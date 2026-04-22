//! Super-admin endpoints (cross-tenant).
//! All routes require `is_superadmin = true` in the JWT claims.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth_extractor::AuthUser;
use crate::state::AppState;

fn require_superadmin(claims: &crate::jwt::Claims) -> Result<(), (StatusCode, String)> {
    if claims.is_superadmin {
        Ok(())
    } else {
        Err((StatusCode::FORBIDDEN, "Requires platform super-admin".into()))
    }
}

// --- Tenants overview ---

#[derive(Serialize, sqlx::FromRow)]
pub struct TenantOverview {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub user_count: i64,
    pub machine_count: i64,
    pub online_machines: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn list_tenants(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<Vec<TenantOverview>>, (StatusCode, String)> {
    require_superadmin(&claims)?;

    let rows: Vec<TenantOverview> = sqlx::query_as(
        r#"
        SELECT
            t.id,
            t.name,
            t.slug,
            (SELECT COUNT(*) FROM users WHERE tenant_id = t.id)   AS user_count,
            (SELECT COUNT(*) FROM machines WHERE tenant_id = t.id) AS machine_count,
            (SELECT COUNT(*) FROM machines WHERE tenant_id = t.id AND is_online = true) AS online_machines,
            t.created_at
        FROM tenants t
        ORDER BY t.created_at DESC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    Ok(Json(rows))
}

pub async fn delete_tenant(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(tenant_id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, String)> {
    require_superadmin(&claims)?;

    // Cascade: delete machines, sessions, users, invitations, then tenant
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    for stmt in [
        "DELETE FROM machine_access WHERE tenant_id = $1",
        "DELETE FROM sessions WHERE tenant_id = $1",
        "DELETE FROM machines WHERE tenant_id = $1",
        "DELETE FROM refresh_tokens WHERE user_id IN (SELECT id FROM users WHERE tenant_id = $1)",
        "DELETE FROM invitations WHERE tenant_id = $1",
        "DELETE FROM users WHERE tenant_id = $1",
        "DELETE FROM tenants WHERE id = $1",
    ] {
        sqlx::query(stmt)
            .bind(tenant_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;
    }

    tx.commit()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Commit: {e}")))?;

    Ok(StatusCode::NO_CONTENT)
}

// --- Users (cross-tenant) ---

#[derive(Serialize, sqlx::FromRow)]
pub struct GlobalUser {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub role: String,
    pub is_superadmin: bool,
    pub tenant_id: Uuid,
    pub tenant_name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn list_users(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<Vec<GlobalUser>>, (StatusCode, String)> {
    require_superadmin(&claims)?;

    let rows: Vec<GlobalUser> = sqlx::query_as(
        "SELECT u.id, u.email, u.display_name, u.role, u.is_superadmin,
                u.tenant_id, t.name AS tenant_name, u.created_at
         FROM users u
         JOIN tenants t ON u.tenant_id = t.id
         ORDER BY u.created_at DESC",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    Ok(Json(rows))
}

#[derive(Deserialize)]
pub struct SetSuperadminRequest {
    pub is_superadmin: bool,
}

pub async fn set_superadmin(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(user_id): Path<Uuid>,
    Json(req): Json<SetSuperadminRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    require_superadmin(&claims)?;

    // Prevent removing your own superadmin (lockout safety)
    if user_id == claims.sub && !req.is_superadmin {
        return Err((StatusCode::CONFLICT, "Cannot revoke your own super-admin".into()));
    }

    let result = sqlx::query("UPDATE users SET is_superadmin = $1 WHERE id = $2")
        .bind(req.is_superadmin)
        .bind(user_id)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    if result.rows_affected() == 0 {
        Err((StatusCode::NOT_FOUND, "User not found".into()))
    } else {
        Ok(StatusCode::NO_CONTENT)
    }
}

// --- Machines (cross-tenant) ---

#[derive(Serialize, sqlx::FromRow)]
pub struct GlobalMachine {
    pub id: Uuid,
    pub name: String,
    pub hostname: Option<String>,
    pub os: Option<String>,
    pub is_online: bool,
    pub last_seen: Option<chrono::DateTime<chrono::Utc>>,
    pub tenant_id: Uuid,
    pub tenant_name: String,
}

pub async fn list_machines(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<Vec<GlobalMachine>>, (StatusCode, String)> {
    require_superadmin(&claims)?;

    let rows: Vec<GlobalMachine> = sqlx::query_as(
        "SELECT m.id, m.name, m.hostname, m.os, m.is_online, m.last_seen,
                m.tenant_id, t.name AS tenant_name
         FROM machines m
         JOIN tenants t ON m.tenant_id = t.id
         ORDER BY m.is_online DESC, m.name",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    Ok(Json(rows))
}

// --- Platform stats summary ---

#[derive(Serialize)]
pub struct PlatformStats {
    pub total_tenants: i64,
    pub total_users: i64,
    pub total_machines: i64,
    pub online_machines: i64,
    pub active_sessions: i64,
}

pub async fn get_stats(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<PlatformStats>, (StatusCode, String)> {
    require_superadmin(&claims)?;

    let (total_tenants,): (i64,) = sqlx::query_as("SELECT COUNT(*)::bigint FROM tenants")
        .fetch_one(&state.db).await.map_err(db_err)?;
    let (total_users,): (i64,) = sqlx::query_as("SELECT COUNT(*)::bigint FROM users")
        .fetch_one(&state.db).await.map_err(db_err)?;
    let (total_machines,): (i64,) = sqlx::query_as("SELECT COUNT(*)::bigint FROM machines")
        .fetch_one(&state.db).await.map_err(db_err)?;
    let (online_machines,): (i64,) = sqlx::query_as("SELECT COUNT(*)::bigint FROM machines WHERE is_online = true")
        .fetch_one(&state.db).await.map_err(db_err)?;
    let (active_sessions,): (i64,) = sqlx::query_as("SELECT COUNT(*)::bigint FROM sessions WHERE ended_at IS NULL")
        .fetch_one(&state.db).await.map_err(db_err)?;

    Ok(Json(PlatformStats {
        total_tenants,
        total_users,
        total_machines,
        online_machines,
        active_sessions,
    }))
}

fn db_err(e: sqlx::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}"))
}
