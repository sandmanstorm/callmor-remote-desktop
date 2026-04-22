use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::state::AppState;

#[derive(Deserialize)]
pub struct HeartbeatRequest {
    pub machine_id: Uuid,
    pub hostname: Option<String>,
    pub os: Option<String>,
}

#[derive(Serialize)]
pub struct HeartbeatResponse {
    pub ok: bool,
}

/// Validates the X-Agent-Token header against a machine's stored agent_token.
/// Returns (machine_id, tenant_id) on success.
pub async fn validate_agent_token(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(Uuid, Uuid), (StatusCode, String)> {
    let token = headers
        .get("X-Agent-Token")
        .and_then(|v| v.to_str().ok())
        .ok_or((StatusCode::UNAUTHORIZED, "Missing X-Agent-Token header".into()))?;

    let row: Option<(Uuid, Uuid)> =
        sqlx::query_as("SELECT id, tenant_id FROM machines WHERE agent_token = $1")
            .bind(token)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    row.ok_or((StatusCode::UNAUTHORIZED, "Invalid agent token".into()))
}

/// Heartbeat endpoint: agents POST here every 30s to report online status.
pub async fn heartbeat(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<HeartbeatRequest>,
) -> Result<Json<HeartbeatResponse>, (StatusCode, String)> {
    let token = headers
        .get("X-Agent-Token")
        .and_then(|v| v.to_str().ok())
        .ok_or((StatusCode::UNAUTHORIZED, "Missing X-Agent-Token header".into()))?;

    // Try tenant machines first
    let tenant_row: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM machines WHERE agent_token = $1")
            .bind(token)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    if let Some((machine_id,)) = tenant_row {
        if machine_id != req.machine_id {
            return Err((StatusCode::FORBIDDEN, "Token/machine mismatch".into()));
        }
        sqlx::query(
            "UPDATE machines SET last_seen = now(), is_online = true, \
             hostname = COALESCE($2, hostname), os = COALESCE($3, os) WHERE id = $1",
        )
        .bind(machine_id)
        .bind(req.hostname.as_deref())
        .bind(req.os.as_deref())
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;
        return Ok(Json(HeartbeatResponse { ok: true }));
    }

    // Fall through to adhoc machines — same agent, different table
    let adhoc_row: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM adhoc_machines WHERE agent_token = $1")
            .bind(token)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    if let Some((machine_id,)) = adhoc_row {
        if machine_id != req.machine_id {
            return Err((StatusCode::FORBIDDEN, "Token/machine mismatch".into()));
        }
        sqlx::query(
            "UPDATE adhoc_machines SET last_seen = now(), online = true, \
             hostname = COALESCE($2, hostname) WHERE id = $1",
        )
        .bind(machine_id)
        .bind(req.hostname.as_deref())
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;
        return Ok(Json(HeartbeatResponse { ok: true }));
    }

    Err((StatusCode::UNAUTHORIZED, "Invalid agent token".into()))
}

/// Called periodically by the API to mark stale machines as offline.
pub async fn sweep_stale(pool: &sqlx::PgPool) {
    let _ = sqlx::query(
        "UPDATE machines SET is_online = false WHERE is_online = true AND (last_seen IS NULL OR last_seen < now() - interval '90 seconds')",
    )
    .execute(pool)
    .await;
}
