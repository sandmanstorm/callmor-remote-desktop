//! Audit log query endpoints.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth_extractor::AuthUser;
use crate::state::AppState;

#[derive(Serialize, sqlx::FromRow)]
pub struct AuditEvent {
    pub id: Uuid,
    pub tenant_id: Option<Uuid>,
    pub actor_id: Option<Uuid>,
    pub actor_email: Option<String>,
    pub actor_display: Option<String>, // joined from users
    pub event_type: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
    pub metadata: serde_json::Value,
    pub ip_address: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
pub struct AuditQuery {
    #[serde(default)]
    pub event_type: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub before: Option<chrono::DateTime<chrono::Utc>>,
}

fn default_limit() -> i64 { 100 }

/// Tenant-scoped audit log (visible to owner/admin of that tenant).
pub async fn list_tenant_audit(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(q): Query<AuditQuery>,
) -> Result<Json<Vec<AuditEvent>>, (StatusCode, String)> {
    if !["owner", "admin"].contains(&claims.role.as_str()) {
        return Err((StatusCode::FORBIDDEN, "Requires admin or owner".into()));
    }

    let limit = q.limit.clamp(1, 500);
    let before = q.before.unwrap_or_else(chrono::Utc::now);

    let events: Vec<AuditEvent> = if let Some(et) = q.event_type {
        sqlx::query_as(
            "SELECT a.id, a.tenant_id, a.actor_id, a.actor_email, u.display_name AS actor_display,
                    a.event_type, a.entity_type, a.entity_id, a.metadata, a.ip_address, a.created_at
             FROM audit_events a
             LEFT JOIN users u ON a.actor_id = u.id
             WHERE a.tenant_id = $1 AND a.event_type = $2 AND a.created_at < $3
             ORDER BY a.created_at DESC LIMIT $4",
        )
        .bind(claims.tenant_id).bind(et).bind(before).bind(limit)
        .fetch_all(&state.db).await
    } else {
        sqlx::query_as(
            "SELECT a.id, a.tenant_id, a.actor_id, a.actor_email, u.display_name AS actor_display,
                    a.event_type, a.entity_type, a.entity_id, a.metadata, a.ip_address, a.created_at
             FROM audit_events a
             LEFT JOIN users u ON a.actor_id = u.id
             WHERE a.tenant_id = $1 AND a.created_at < $2
             ORDER BY a.created_at DESC LIMIT $3",
        )
        .bind(claims.tenant_id).bind(before).bind(limit)
        .fetch_all(&state.db).await
    }
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    Ok(Json(events))
}

/// Platform-wide audit log (superadmin only).
pub async fn list_platform_audit(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(q): Query<AuditQuery>,
) -> Result<Json<Vec<AuditEvent>>, (StatusCode, String)> {
    if !claims.is_superadmin {
        return Err((StatusCode::FORBIDDEN, "Requires super-admin".into()));
    }

    let limit = q.limit.clamp(1, 500);
    let before = q.before.unwrap_or_else(chrono::Utc::now);

    let events: Vec<AuditEvent> = if let Some(et) = q.event_type {
        sqlx::query_as(
            "SELECT a.id, a.tenant_id, a.actor_id, a.actor_email, u.display_name AS actor_display,
                    a.event_type, a.entity_type, a.entity_id, a.metadata, a.ip_address, a.created_at
             FROM audit_events a
             LEFT JOIN users u ON a.actor_id = u.id
             WHERE a.event_type = $1 AND a.created_at < $2
             ORDER BY a.created_at DESC LIMIT $3",
        )
        .bind(et).bind(before).bind(limit)
        .fetch_all(&state.db).await
    } else {
        sqlx::query_as(
            "SELECT a.id, a.tenant_id, a.actor_id, a.actor_email, u.display_name AS actor_display,
                    a.event_type, a.entity_type, a.entity_id, a.metadata, a.ip_address, a.created_at
             FROM audit_events a
             LEFT JOIN users u ON a.actor_id = u.id
             WHERE a.created_at < $1
             ORDER BY a.created_at DESC LIMIT $2",
        )
        .bind(before).bind(limit)
        .fetch_all(&state.db).await
    }
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    Ok(Json(events))
}
