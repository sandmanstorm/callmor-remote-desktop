//! Agent self-enrollment endpoint.
//!
//! Agent calls this on first run with the tenant's enrollment_token
//! (baked into the installer). We create a machine record and return
//! a permanent agent_token that the agent uses for all future auth.

use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth_extractor::AuthUser;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct EnrollRequest {
    pub enrollment_token: String,
    pub hostname: String,
    pub os: String,
}

#[derive(Serialize)]
pub struct EnrollResponse {
    pub machine_id: Uuid,
    pub agent_token: String,
    pub relay_url: String,
    pub api_url: String,
}

fn generate_agent_token() -> String {
    use rand::Rng;
    let bytes: [u8; 32] = rand::rng().random();
    format!("cmt_{}", hex::encode(bytes))
}

pub async fn enroll(
    State(state): State<AppState>,
    Json(req): Json<EnrollRequest>,
) -> Result<Json<EnrollResponse>, (StatusCode, String)> {
    // Look up tenant by enrollment token
    let row: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM tenants WHERE enrollment_token = $1")
            .bind(&req.enrollment_token)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    let tenant_id = row
        .ok_or((StatusCode::UNAUTHORIZED, "Invalid enrollment token".into()))?
        .0;

    // Validate os
    let os = match req.os.to_lowercase().as_str() {
        "linux" | "windows" | "macos" => req.os.to_lowercase(),
        _ => return Err((StatusCode::BAD_REQUEST, "Invalid os".into())),
    };

    // Use the hostname as the default machine name (user can rename later)
    let name = if req.hostname.is_empty() {
        format!("{}-agent", &os)
    } else {
        req.hostname.clone()
    };

    // Generate agent token for this new machine
    let agent_token = generate_agent_token();

    let machine_id: Uuid = sqlx::query_scalar(
        "INSERT INTO machines (tenant_id, name, hostname, os, agent_token) VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(tenant_id)
    .bind(&name)
    .bind(&req.hostname)
    .bind(&os)
    .bind(&agent_token)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    crate::audit::log(
        &state.db,
        &crate::audit::AuditContext {
            tenant_id: Some(tenant_id),
            ..Default::default()
        },
        "machine.enrolled",
        Some("machine"),
        Some(machine_id),
        serde_json::json!({"name": name, "hostname": req.hostname, "os": os}),
    )
    .await;

    let relay_url = std::env::var("PUBLIC_RELAY_URL")
        .unwrap_or_else(|_| "wss://relay.callmor.ai".into());
    let api_url = state.api_url.clone();

    Ok(Json(EnrollResponse {
        machine_id,
        agent_token,
        relay_url,
        api_url,
    }))
}

// --- Tenant-owner endpoints ---

#[derive(Serialize)]
pub struct TenantEnrollmentInfo {
    pub enrollment_token: String,
}

pub async fn get_enrollment_token(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<TenantEnrollmentInfo>, (StatusCode, String)> {
    if !["owner", "admin"].contains(&claims.role.as_str()) {
        return Err((StatusCode::FORBIDDEN, "Requires admin or owner".into()));
    }
    let token: String =
        sqlx::query_scalar("SELECT enrollment_token FROM tenants WHERE id = $1")
            .bind(claims.tenant_id)
            .fetch_one(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;
    Ok(Json(TenantEnrollmentInfo { enrollment_token: token }))
}

pub async fn rotate_enrollment_token(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<TenantEnrollmentInfo>, (StatusCode, String)> {
    if claims.role != "owner" {
        return Err((StatusCode::FORBIDDEN, "Only owners can rotate".into()));
    }

    let new_token = {
        use rand::Rng;
        let bytes: [u8; 16] = rand::rng().random();
        format!("cle_{}", hex::encode(bytes))
    };

    sqlx::query("UPDATE tenants SET enrollment_token = $1 WHERE id = $2")
        .bind(&new_token)
        .bind(claims.tenant_id)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    crate::audit::log(
        &state.db,
        &crate::audit::ctx_from_claims(&claims),
        "tenant.enrollment_rotated",
        Some("tenant"),
        Some(claims.tenant_id),
        serde_json::json!({}),
    )
    .await;

    Ok(Json(TenantEnrollmentInfo { enrollment_token: new_token }))
}
