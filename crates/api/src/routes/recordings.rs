//! Session recording endpoints.

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::Response,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth_extractor::AuthUser;
use crate::state::AppState;

// =========================================================================
// Agent config endpoint (agent-token auth)
// Agent calls this to learn whether recording is enabled for its tenant.
// =========================================================================

#[derive(Serialize)]
pub struct AgentConfigResponse {
    pub recording_enabled: bool,
    pub api_url: String,
}

pub async fn agent_get_config(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AgentConfigResponse>, (StatusCode, String)> {
    let (_machine_id, tenant_id) = crate::routes::agent::validate_agent_token(&state, &headers).await?;

    let recording_enabled: bool = sqlx::query_scalar(
        "SELECT recording_enabled FROM tenants WHERE id = $1",
    )
    .bind(tenant_id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    Ok(Json(AgentConfigResponse {
        recording_enabled,
        api_url: state.public_url.clone(),
    }))
}

// =========================================================================
// Agent upload endpoint (agent-token auth)
// Agent POSTs the recording file as raw body.
// =========================================================================

#[derive(Deserialize)]
pub struct UploadParams {
    pub session_id: Uuid,
    #[serde(default)]
    pub duration_ms: Option<i64>,
}

pub async fn agent_upload_recording(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<UploadParams>,
    body: axum::body::Bytes,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let (machine_id, tenant_id) = crate::routes::agent::validate_agent_token(&state, &headers).await?;

    // Verify the session belongs to this machine
    let session_valid: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sessions WHERE id = $1 AND machine_id = $2 AND tenant_id = $3)",
    )
    .bind(params.session_id)
    .bind(machine_id)
    .bind(tenant_id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    if !session_valid {
        return Err((StatusCode::FORBIDDEN, "Session does not belong to this machine".into()));
    }

    let size_bytes = body.len() as i64;
    let object_key = format!("{}/{}.mp4", tenant_id, params.session_id);

    state
        .storage
        .put_recording(&object_key, body.to_vec(), "video/mp4")
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Upload: {e:#}")))?;

    // Record metadata in DB
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO recordings (tenant_id, session_id, machine_id, object_key, size_bytes, duration_ms, content_type)
         VALUES ($1, $2, $3, $4, $5, $6, 'video/mp4')
         ON CONFLICT DO NOTHING
         RETURNING id",
    )
    .bind(tenant_id)
    .bind(params.session_id)
    .bind(machine_id)
    .bind(&object_key)
    .bind(size_bytes)
    .bind(params.duration_ms)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    crate::audit::log(
        &state.db,
        &crate::audit::AuditContext { tenant_id: Some(tenant_id), ..Default::default() },
        "recording.uploaded",
        Some("recording"),
        Some(id),
        serde_json::json!({"size_bytes": size_bytes, "session_id": params.session_id}),
    ).await;

    Ok(Json(serde_json::json!({"id": id, "size_bytes": size_bytes})))
}

// =========================================================================
// User endpoints
// =========================================================================

#[derive(Serialize, sqlx::FromRow)]
pub struct RecordingInfo {
    pub id: Uuid,
    pub session_id: Uuid,
    pub machine_id: Uuid,
    pub machine_name: String,
    pub size_bytes: i64,
    pub duration_ms: Option<i64>,
    pub content_type: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub started_by: Option<String>,
}

pub async fn list_recordings(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<Vec<RecordingInfo>>, (StatusCode, String)> {
    // Owners/admins see everything in the tenant.
    // Members only see recordings of sessions they started.
    let rows: Vec<RecordingInfo> = if claims.role == "owner" || claims.role == "admin" {
        sqlx::query_as(
            "SELECT r.id, r.session_id, r.machine_id, m.name AS machine_name,
                    r.size_bytes, r.duration_ms, r.content_type, r.created_at,
                    u.display_name AS started_by
             FROM recordings r
             JOIN machines m ON r.machine_id = m.id
             JOIN sessions s ON r.session_id = s.id
             LEFT JOIN users u ON s.user_id = u.id
             WHERE r.tenant_id = $1
             ORDER BY r.created_at DESC
             LIMIT 200",
        )
        .bind(claims.tenant_id)
        .fetch_all(&state.db)
        .await
    } else {
        sqlx::query_as(
            "SELECT r.id, r.session_id, r.machine_id, m.name AS machine_name,
                    r.size_bytes, r.duration_ms, r.content_type, r.created_at,
                    u.display_name AS started_by
             FROM recordings r
             JOIN machines m ON r.machine_id = m.id
             JOIN sessions s ON r.session_id = s.id
             LEFT JOIN users u ON s.user_id = u.id
             WHERE r.tenant_id = $1 AND s.user_id = $2
             ORDER BY r.created_at DESC
             LIMIT 200",
        )
        .bind(claims.tenant_id)
        .bind(claims.sub)
        .fetch_all(&state.db)
        .await
    }
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    Ok(Json(rows))
}

pub async fn playback_recording(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(recording_id): Path<Uuid>,
) -> Result<Response<Body>, (StatusCode, String)> {
    // Authorize: must be in tenant and (owner/admin OR user who started the session)
    let row: Option<(String, String, Uuid, Uuid)> = sqlx::query_as(
        "SELECT r.object_key, r.content_type, r.tenant_id, s.user_id
         FROM recordings r
         JOIN sessions s ON r.session_id = s.id
         WHERE r.id = $1",
    )
    .bind(recording_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    let (object_key, content_type, tenant_id, session_user_id) =
        row.ok_or((StatusCode::NOT_FOUND, "Recording not found".into()))?;

    if tenant_id != claims.tenant_id {
        return Err((StatusCode::NOT_FOUND, "Recording not found".into()));
    }
    if claims.role == "member" && session_user_id != claims.sub {
        return Err((StatusCode::FORBIDDEN, "You cannot view this recording".into()));
    }

    let bytes = state
        .storage
        .get_recording(&object_key)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Storage: {e:#}")))?;

    Response::builder()
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, bytes.len())
        .header(header::ACCEPT_RANGES, "bytes")
        .body(Body::from(bytes))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Response: {e}")))
}

pub async fn delete_recording(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(recording_id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, String)> {
    if !["owner", "admin"].contains(&claims.role.as_str()) {
        return Err((StatusCode::FORBIDDEN, "Only owner/admin can delete recordings".into()));
    }

    let row: Option<(String,)> = sqlx::query_as(
        "SELECT object_key FROM recordings WHERE id = $1 AND tenant_id = $2",
    )
    .bind(recording_id)
    .bind(claims.tenant_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    let (object_key,) = row.ok_or((StatusCode::NOT_FOUND, "Recording not found".into()))?;

    let _ = state.storage.delete_recording(&object_key).await;

    sqlx::query("DELETE FROM recordings WHERE id = $1 AND tenant_id = $2")
        .bind(recording_id)
        .bind(claims.tenant_id)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    crate::audit::log(
        &state.db,
        &crate::audit::ctx_from_claims(&claims),
        "recording.deleted",
        Some("recording"),
        Some(recording_id),
        serde_json::json!({}),
    ).await;

    Ok(StatusCode::NO_CONTENT)
}

// =========================================================================
// Tenant settings — recording toggle
// =========================================================================

#[derive(Deserialize)]
pub struct UpdateTenantSettingsRequest {
    pub recording_enabled: bool,
}

pub async fn update_tenant_recording_setting(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(req): Json<UpdateTenantSettingsRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    if claims.role != "owner" {
        return Err((StatusCode::FORBIDDEN, "Only owners can change recording setting".into()));
    }

    sqlx::query("UPDATE tenants SET recording_enabled = $1 WHERE id = $2")
        .bind(req.recording_enabled)
        .bind(claims.tenant_id)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    crate::audit::log(
        &state.db,
        &crate::audit::ctx_from_claims(&claims),
        if req.recording_enabled { "tenant.recording_enabled" } else { "tenant.recording_disabled" },
        Some("tenant"),
        Some(claims.tenant_id),
        serde_json::json!({}),
    ).await;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize)]
pub struct TenantSettingsResponse {
    pub recording_enabled: bool,
}

pub async fn get_tenant_settings(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<TenantSettingsResponse>, (StatusCode, String)> {
    let recording_enabled: bool = sqlx::query_scalar(
        "SELECT recording_enabled FROM tenants WHERE id = $1",
    )
    .bind(claims.tenant_id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    Ok(Json(TenantSettingsResponse { recording_enabled }))
}
