use axum::{
    extract::{ConnectInfo, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
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

#[derive(Serialize)]
pub struct CreateSessionResponse {
    pub session: Session,
    pub session_token: String,
    pub machine_id: Uuid,
    pub relay_url: String,
}

fn default_permission() -> String {
    "full_control".into()
}

pub async fn create_session(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(req): Json<CreateSessionRequest>,
) -> Result<(StatusCode, Json<CreateSessionResponse>), (StatusCode, String)> {
    // Verify machine belongs to tenant and is online
    let machine: Option<callmor_shared::Machine> =
        sqlx::query_as("SELECT * FROM machines WHERE id = $1 AND tenant_id = $2")
            .bind(req.machine_id)
            .bind(claims.tenant_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    let machine = machine.ok_or((StatusCode::NOT_FOUND, "Machine not found".into()))?;

    // Check access
    let has_access = crate::routes::machines::user_has_access(
        &state.db,
        claims.sub,
        claims.tenant_id,
        machine.id,
        &claims.role,
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;
    if !has_access {
        return Err((StatusCode::FORBIDDEN, "No access to this machine".into()));
    }

    if !machine.is_online {
        return Err((StatusCode::CONFLICT, "Machine is offline".into()));
    }

    // Validate permission
    if !["view_only", "full_control"].contains(&req.permission.as_str()) {
        return Err((StatusCode::BAD_REQUEST, "Invalid permission".into()));
    }

    // Create session row
    let ip = addr.ip().to_string();
    let session: Session = sqlx::query_as(
        "INSERT INTO sessions (tenant_id, machine_id, user_id, permission, ip_address) VALUES ($1, $2, $3, $4, $5) RETURNING *",
    )
    .bind(claims.tenant_id)
    .bind(req.machine_id)
    .bind(claims.sub)
    .bind(&req.permission)
    .bind(&ip)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    crate::audit::log(
        &state.db,
        &crate::audit::AuditContext {
            tenant_id: Some(claims.tenant_id),
            actor_id: Some(claims.sub),
            actor_email: None,
            ip_address: Some(ip),
        },
        "session.started",
        Some("machine"),
        Some(req.machine_id),
        serde_json::json!({
            "machine_name": machine.name,
            "permission": req.permission,
            "session_id": session.id,
        }),
    ).await;

    // Issue session token (2min lifetime)
    let session_token = state
        .jwt
        .create_session_token(
            claims.sub,
            claims.tenant_id,
            req.machine_id,
            session.id,
            &req.permission,
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("JWT error: {e}")))?;

    let relay_url = std::env::var("PUBLIC_RELAY_URL")
        .unwrap_or_else(|_| "wss://relay.callmor.ai".into());

    Ok((
        StatusCode::CREATED,
        Json(CreateSessionResponse {
            session,
            session_token,
            machine_id: req.machine_id,
            relay_url,
        }),
    ))
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
