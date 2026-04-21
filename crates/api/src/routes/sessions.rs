use axum::{
    extract::{ConnectInfo, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use std::net::SocketAddr;
use uuid::Uuid;

use crate::auth_extractor::AuthUser;
use crate::state::AppState;
use callmor_shared::Session;

#[derive(Deserialize)]
pub struct CreateSessionRequest {
    pub machine_id: Uuid,
    #[serde(default = "default_permission")]
    pub permission: String,
}

fn default_permission() -> String {
    "full_control".into()
}

pub async fn create_session(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(req): Json<CreateSessionRequest>,
) -> Result<(StatusCode, Json<Session>), (StatusCode, String)> {
    // Verify machine belongs to tenant
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM machines WHERE id = $1 AND tenant_id = $2)",
    )
    .bind(req.machine_id)
    .bind(claims.tenant_id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    if !exists {
        return Err((StatusCode::NOT_FOUND, "Machine not found".into()));
    }

    let session: Session = sqlx::query_as(
        "INSERT INTO sessions (tenant_id, machine_id, user_id, permission, ip_address) VALUES ($1, $2, $3, $4, $5) RETURNING *",
    )
    .bind(claims.tenant_id)
    .bind(req.machine_id)
    .bind(claims.sub)
    .bind(&req.permission)
    .bind(addr.ip().to_string())
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    Ok((StatusCode::CREATED, Json(session)))
}

pub async fn list_sessions(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<Vec<Session>>, (StatusCode, String)> {
    let sessions: Vec<Session> =
        sqlx::query_as("SELECT * FROM sessions WHERE tenant_id = $1 ORDER BY started_at DESC LIMIT 50")
            .bind(claims.tenant_id)
            .fetch_all(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;
    Ok(Json(sessions))
}

pub async fn list_active_sessions(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<Vec<Session>>, (StatusCode, String)> {
    let sessions: Vec<Session> =
        sqlx::query_as("SELECT * FROM sessions WHERE tenant_id = $1 AND ended_at IS NULL ORDER BY started_at DESC")
            .bind(claims.tenant_id)
            .fetch_all(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;
    Ok(Json(sessions))
}
