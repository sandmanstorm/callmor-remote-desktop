//! Audit log helper.
//!
//! Fire-and-forget: we never fail a user request because audit insert failed;
//! we just log the error.

use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone, Debug, Default)]
pub struct AuditContext {
    pub tenant_id: Option<Uuid>,
    pub actor_id: Option<Uuid>,
    pub actor_email: Option<String>,
    pub ip_address: Option<String>,
}

pub async fn log(
    db: &PgPool,
    ctx: &AuditContext,
    event_type: &str,
    entity_type: Option<&str>,
    entity_id: Option<Uuid>,
    metadata: serde_json::Value,
) {
    let result = sqlx::query(
        "INSERT INTO audit_events
           (tenant_id, actor_id, actor_email, event_type, entity_type, entity_id, metadata, ip_address)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(ctx.tenant_id)
    .bind(ctx.actor_id)
    .bind(ctx.actor_email.as_deref())
    .bind(event_type)
    .bind(entity_type)
    .bind(entity_id)
    .bind(metadata)
    .bind(ctx.ip_address.as_deref())
    .execute(db)
    .await;

    if let Err(e) = result {
        tracing::warn!("audit log insert failed for {event_type}: {e}");
    }
}

/// Convenience helper: build context from JWT claims.
pub fn ctx_from_claims(claims: &crate::jwt::Claims) -> AuditContext {
    AuditContext {
        tenant_id: Some(claims.tenant_id),
        actor_id: Some(claims.sub),
        actor_email: None, // populated by callers that have it
        ip_address: None,  // populated by callers that have it
    }
}
