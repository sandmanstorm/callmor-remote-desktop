use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth_extractor::AuthUser;
use crate::jwt::{generate_refresh_token, hash_refresh_token};
use crate::state::AppState;
use callmor_shared::Invitation;

#[derive(Deserialize)]
pub struct CreateInvitationRequest {
    pub email: String,
    #[serde(default = "default_role")]
    pub role: String,
}

fn default_role() -> String {
    "member".into()
}

#[derive(Serialize)]
pub struct CreateInvitationResponse {
    pub id: Uuid,
    pub email: String,
    pub role: String,
    pub token: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub email_sent: bool,
}

fn require_admin_or_owner(role: &str) -> Result<(), (StatusCode, String)> {
    match role {
        "owner" | "admin" => Ok(()),
        _ => Err((StatusCode::FORBIDDEN, "Requires admin or owner role".into())),
    }
}

fn require_owner(role: &str) -> Result<(), (StatusCode, String)> {
    if role == "owner" {
        Ok(())
    } else {
        Err((StatusCode::FORBIDDEN, "Requires owner role".into()))
    }
}

pub async fn create_invitation(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(req): Json<CreateInvitationRequest>,
) -> Result<(StatusCode, Json<CreateInvitationResponse>), (StatusCode, String)> {
    require_admin_or_owner(&claims.role)?;

    // Only owners can invite other owners/admins
    if req.role != "member" {
        require_owner(&claims.role)?;
    }

    let token = generate_refresh_token();
    let token_hash = hash_refresh_token(&token);
    let expires_at = Utc::now() + Duration::days(7);

    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO invitations (tenant_id, email, role, token_hash, invited_by, expires_at) VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(claims.tenant_id)
    .bind(&req.email)
    .bind(&req.role)
    .bind(&token_hash)
    .bind(claims.sub)
    .bind(expires_at)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    // Lookup inviter name + tenant name for the email
    let (inviter_name, tenant_name): (String, String) = sqlx::query_as(
        "SELECT u.display_name, t.name FROM users u JOIN tenants t ON u.tenant_id = t.id WHERE u.id = $1",
    )
    .bind(claims.sub)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    let invite_link = format!("{}/invite/{}", state.public_url, token);
    let mut email_sent = false;

    if let Some(smtp) = crate::email::EmailConfig::load(&state.db).await {
        let (subject, html, text) = crate::email::invitation_email(
            &req.email,
            &inviter_name,
            &tenant_name,
            &req.role,
            &invite_link,
        );
        match smtp.send(&req.email, &subject, &html, &text).await {
            Ok(()) => {
                tracing::info!("Invitation email sent to {}", req.email);
                email_sent = true;
            }
            Err(e) => {
                tracing::warn!("Failed to send invitation email to {}: {e:#}", req.email);
            }
        }
    }

    Ok((
        StatusCode::CREATED,
        Json(CreateInvitationResponse {
            id,
            email: req.email,
            role: req.role,
            token,
            expires_at,
            email_sent,
        }),
    ))
}

pub async fn list_invitations(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<Vec<Invitation>>, (StatusCode, String)> {
    require_admin_or_owner(&claims.role)?;

    let invitations: Vec<Invitation> = sqlx::query_as(
        "SELECT * FROM invitations WHERE tenant_id = $1 AND accepted_at IS NULL AND expires_at > now() ORDER BY created_at DESC",
    )
    .bind(claims.tenant_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    Ok(Json(invitations))
}

pub async fn delete_invitation(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, String)> {
    require_admin_or_owner(&claims.role)?;

    let result = sqlx::query("DELETE FROM invitations WHERE id = $1 AND tenant_id = $2")
        .bind(id)
        .bind(claims.tenant_id)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    if result.rows_affected() == 0 {
        Err((StatusCode::NOT_FOUND, "Invitation not found".into()))
    } else {
        Ok(StatusCode::NO_CONTENT)
    }
}

// --- Public invitation accept flow ---

#[derive(Serialize)]
pub struct InvitationDetails {
    pub email: String,
    pub role: String,
    pub tenant_name: String,
    pub tenant_slug: String,
    pub invited_by_name: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

pub async fn get_invitation(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<Json<InvitationDetails>, (StatusCode, String)> {
    let token_hash = hash_refresh_token(&token);

    let row: Option<(String, String, String, String, String, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as(
            "SELECT i.email, i.role, t.name, t.slug, u.display_name, i.expires_at
             FROM invitations i
             JOIN tenants t ON i.tenant_id = t.id
             JOIN users u ON i.invited_by = u.id
             WHERE i.token_hash = $1 AND i.accepted_at IS NULL AND i.expires_at > now()",
        )
        .bind(&token_hash)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    let (email, role, tenant_name, tenant_slug, invited_by_name, expires_at) =
        row.ok_or((StatusCode::NOT_FOUND, "Invitation not found or expired".into()))?;

    Ok(Json(InvitationDetails {
        email,
        role,
        tenant_name,
        tenant_slug,
        invited_by_name,
        expires_at,
    }))
}

#[derive(Deserialize)]
pub struct AcceptInvitationRequest {
    pub password: String,
    pub display_name: String,
}

pub async fn accept_invitation(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Json(req): Json<AcceptInvitationRequest>,
) -> Result<Json<crate::routes::auth::AuthResponse>, (StatusCode, String)> {
    use argon2::{password_hash::SaltString, Argon2, PasswordHasher};

    let token_hash = hash_refresh_token(&token);

    // Find and mark accepted
    let row: Option<(Uuid, Uuid, String, String)> = sqlx::query_as(
        "UPDATE invitations SET accepted_at = now()
         WHERE token_hash = $1 AND accepted_at IS NULL AND expires_at > now()
         RETURNING id, tenant_id, email, role",
    )
    .bind(&token_hash)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    let (_inv_id, tenant_id, email, role) =
        row.ok_or((StatusCode::NOT_FOUND, "Invitation not found or expired".into()))?;

    // Hash password
    let salt = SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
    let password_hash = Argon2::default()
        .hash_password(req.password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Hash: {e}")))?;

    // Create user
    let user_id: Uuid = sqlx::query_scalar(
        "INSERT INTO users (tenant_id, email, password_hash, display_name, role) VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(tenant_id)
    .bind(&email)
    .bind(&password_hash)
    .bind(&req.display_name)
    .bind(&role)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    // Issue tokens (reuse auth logic)
    let tenant: callmor_shared::Tenant = sqlx::query_as("SELECT * FROM tenants WHERE id = $1")
        .bind(tenant_id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    let access_token = state
        .jwt
        .create_access_token(user_id, tenant_id, &role, false)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("JWT: {e}")))?;

    let refresh = generate_refresh_token();
    let refresh_hash_val = hash_refresh_token(&refresh);
    let expires = Utc::now() + Duration::days(7);

    sqlx::query("INSERT INTO refresh_tokens (user_id, token_hash, expires_at) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind(&refresh_hash_val)
        .bind(expires)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    Ok(Json(crate::routes::auth::AuthResponse {
        access_token,
        refresh_token: refresh,
        user: crate::routes::auth::UserInfo {
            id: user_id,
            email,
            display_name: req.display_name,
            role,
            is_superadmin: false,
            tenant_id,
            tenant_name: tenant.name,
            tenant_slug: tenant.slug,
        },
    }))
}
