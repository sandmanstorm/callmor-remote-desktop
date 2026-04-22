use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth_extractor::AuthUser;
use crate::state::AppState;

#[derive(Serialize, sqlx::FromRow)]
pub struct UserListItem {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub role: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn list_users(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<Vec<UserListItem>>, (StatusCode, String)> {
    if !["owner", "admin"].contains(&claims.role.as_str()) {
        return Err((StatusCode::FORBIDDEN, "Requires admin or owner".into()));
    }
    let users: Vec<UserListItem> = sqlx::query_as(
        "SELECT id, email, display_name, role, created_at FROM users WHERE tenant_id = $1 ORDER BY created_at",
    )
    .bind(claims.tenant_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    Ok(Json(users))
}

#[derive(Deserialize)]
pub struct UpdateUserRequest {
    pub role: Option<String>,
}

pub async fn update_user(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(user_id): Path<Uuid>,
    Json(req): Json<UpdateUserRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    if claims.role != "owner" {
        return Err((StatusCode::FORBIDDEN, "Only owners can change roles".into()));
    }

    // Can't change own role
    if user_id == claims.sub {
        return Err((StatusCode::CONFLICT, "Cannot change your own role".into()));
    }

    if let Some(role) = req.role {
        if !["owner", "admin", "member"].contains(&role.as_str()) {
            return Err((StatusCode::BAD_REQUEST, "Invalid role".into()));
        }

        let result = sqlx::query("UPDATE users SET role = $1 WHERE id = $2 AND tenant_id = $3")
            .bind(&role)
            .bind(user_id)
            .bind(claims.tenant_id)
            .execute(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

        if result.rows_affected() == 0 {
            return Err((StatusCode::NOT_FOUND, "User not found".into()));
        }

        crate::audit::log(
            &state.db,
            &crate::audit::ctx_from_claims(&claims),
            "user.role_changed",
            Some("user"),
            Some(user_id),
            serde_json::json!({"new_role": role}),
        ).await;
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_user(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(user_id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, String)> {
    if claims.role != "owner" {
        return Err((StatusCode::FORBIDDEN, "Only owners can remove users".into()));
    }

    if user_id == claims.sub {
        return Err((StatusCode::CONFLICT, "Cannot remove yourself".into()));
    }

    let email: Option<String> = sqlx::query_scalar("SELECT email FROM users WHERE id = $1 AND tenant_id = $2")
        .bind(user_id).bind(claims.tenant_id)
        .fetch_optional(&state.db).await.ok().flatten();

    let result = sqlx::query("DELETE FROM users WHERE id = $1 AND tenant_id = $2 AND role != 'owner'")
        .bind(user_id)
        .bind(claims.tenant_id)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    if result.rows_affected() == 0 {
        Err((StatusCode::NOT_FOUND, "User not found or cannot remove owner".into()))
    } else {
        crate::audit::log(
            &state.db,
            &crate::audit::ctx_from_claims(&claims),
            "user.removed",
            Some("user"),
            Some(user_id),
            serde_json::json!({"email": email}),
        ).await;
        Ok(StatusCode::NO_CONTENT)
    }
}
