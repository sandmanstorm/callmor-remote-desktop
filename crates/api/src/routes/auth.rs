use axum::{extract::State, http::StatusCode, Json};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::jwt::{generate_refresh_token, hash_refresh_token};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub display_name: String,
    pub tenant_name: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
    pub tenant_slug: String,
}

#[derive(Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub user: UserInfo,
}

#[derive(Serialize)]
pub struct UserInfo {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub role: String,
    pub is_superadmin: bool,
    pub tenant_id: Uuid,
    pub tenant_name: String,
    pub tenant_slug: String,
}

fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn hash_password(password: &str) -> Result<String, (StatusCode, String)> {
    use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
    let salt = SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Hash error: {e}")))
}

fn verify_password(password: &str, hash: &str) -> bool {
    use argon2::{password_hash::PasswordHash, Argon2, PasswordVerifier};
    let parsed = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

async fn issue_tokens(
    state: &AppState,
    user_id: Uuid,
    tenant_id: Uuid,
    role: &str,
    is_superadmin: bool,
) -> Result<(String, String), (StatusCode, String)> {
    let access_token = state
        .jwt
        .create_access_token(user_id, tenant_id, role, is_superadmin)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("JWT error: {e}")))?;

    let refresh = generate_refresh_token();
    let refresh_hash = hash_refresh_token(&refresh);
    let expires_at = Utc::now() + Duration::days(7);

    sqlx::query("INSERT INTO refresh_tokens (user_id, token_hash, expires_at) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind(&refresh_hash)
        .bind(expires_at)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    Ok((access_token, refresh))
}

pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, String)> {
    let slug = slugify(&req.tenant_name);

    // Check slug uniqueness
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tenants WHERE slug = $1)")
        .bind(&slug)
        .fetch_one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;
    if exists {
        return Err((StatusCode::CONFLICT, "Tenant slug already exists".into()));
    }

    let password_hash = hash_password(&req.password)?;

    // Create tenant
    let tenant_id: Uuid =
        sqlx::query_scalar("INSERT INTO tenants (name, slug) VALUES ($1, $2) RETURNING id")
            .bind(&req.tenant_name)
            .bind(&slug)
            .fetch_one(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    // Create user as owner
    let user_id: Uuid = sqlx::query_scalar(
        "INSERT INTO users (tenant_id, email, password_hash, display_name, role) VALUES ($1, $2, $3, $4, 'owner') RETURNING id",
    )
    .bind(tenant_id)
    .bind(&req.email)
    .bind(&password_hash)
    .bind(&req.display_name)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    let (access_token, refresh_token) = issue_tokens(&state, user_id, tenant_id, "owner", false).await?;

    crate::audit::log(
        &state.db,
        &crate::audit::AuditContext {
            tenant_id: Some(tenant_id),
            actor_id: Some(user_id),
            actor_email: Some(req.email.clone()),
            ..Default::default()
        },
        "tenant.created",
        Some("tenant"),
        Some(tenant_id),
        serde_json::json!({"tenant_name": req.tenant_name, "tenant_slug": slug}),
    ).await;

    Ok(Json(AuthResponse {
        access_token,
        refresh_token,
        user: UserInfo {
            id: user_id,
            email: req.email,
            display_name: req.display_name,
            role: "owner".into(),
            is_superadmin: false,
            tenant_id,
            tenant_name: req.tenant_name,
            tenant_slug: slug,
        },
    }))
}

pub async fn login(
    State(state): State<AppState>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, String)> {
    let ip = addr.ip().to_string();

    // Find tenant
    let tenant: Option<callmor_shared::Tenant> =
        sqlx::query_as("SELECT * FROM tenants WHERE slug = $1")
            .bind(&req.tenant_slug)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;
    let tenant = match tenant {
        Some(t) => t,
        None => {
            crate::audit::log(
                &state.db,
                &crate::audit::AuditContext { ip_address: Some(ip.clone()), ..Default::default() },
                "auth.login_failed",
                Some("email"),
                None,
                serde_json::json!({"email": req.email, "reason": "tenant_not_found"}),
            ).await;
            return Err((StatusCode::UNAUTHORIZED, "Invalid credentials".into()));
        }
    };

    // Find user
    let user: Option<callmor_shared::User> =
        sqlx::query_as("SELECT * FROM users WHERE tenant_id = $1 AND email = $2")
            .bind(tenant.id)
            .bind(&req.email)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;
    let user = match user {
        Some(u) => u,
        None => {
            crate::audit::log(
                &state.db,
                &crate::audit::AuditContext { tenant_id: Some(tenant.id), ip_address: Some(ip.clone()), ..Default::default() },
                "auth.login_failed",
                Some("email"),
                None,
                serde_json::json!({"email": req.email, "reason": "user_not_found"}),
            ).await;
            return Err((StatusCode::UNAUTHORIZED, "Invalid credentials".into()));
        }
    };

    if !verify_password(&req.password, &user.password_hash) {
        crate::audit::log(
            &state.db,
            &crate::audit::AuditContext {
                tenant_id: Some(tenant.id),
                actor_id: Some(user.id),
                actor_email: Some(user.email.clone()),
                ip_address: Some(ip),
            },
            "auth.login_failed",
            Some("user"),
            Some(user.id),
            serde_json::json!({"reason": "wrong_password"}),
        ).await;
        return Err((StatusCode::UNAUTHORIZED, "Invalid credentials".into()));
    }

    let (access_token, refresh_token) =
        issue_tokens(&state, user.id, tenant.id, &user.role, user.is_superadmin).await?;

    crate::audit::log(
        &state.db,
        &crate::audit::AuditContext {
            tenant_id: Some(tenant.id),
            actor_id: Some(user.id),
            actor_email: Some(user.email.clone()),
            ip_address: Some(ip),
        },
        "auth.login",
        Some("user"),
        Some(user.id),
        serde_json::json!({}),
    ).await;

    Ok(Json(AuthResponse {
        access_token,
        refresh_token,
        user: UserInfo {
            id: user.id,
            email: user.email,
            display_name: user.display_name,
            role: user.role,
            is_superadmin: user.is_superadmin,
            tenant_id: tenant.id,
            tenant_name: tenant.name,
            tenant_slug: tenant.slug,
        },
    }))
}

pub async fn refresh(
    State(state): State<AppState>,
    Json(req): Json<RefreshRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, String)> {
    let token_hash = hash_refresh_token(&req.refresh_token);

    // Find and delete the old refresh token (rotation)
    let row: Option<callmor_shared::RefreshToken> =
        sqlx::query_as("DELETE FROM refresh_tokens WHERE token_hash = $1 AND expires_at > now() RETURNING *")
            .bind(&token_hash)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;
    let old_token = row.ok_or((StatusCode::UNAUTHORIZED, "Invalid refresh token".into()))?;

    // Get user + tenant
    let user: callmor_shared::User = sqlx::query_as("SELECT * FROM users WHERE id = $1")
        .bind(old_token.user_id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    let tenant: callmor_shared::Tenant = sqlx::query_as("SELECT * FROM tenants WHERE id = $1")
        .bind(user.tenant_id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    let (access_token, refresh_token) =
        issue_tokens(&state, user.id, user.tenant_id, &user.role, user.is_superadmin).await?;

    Ok(Json(AuthResponse {
        access_token,
        refresh_token,
        user: UserInfo {
            id: user.id,
            email: user.email,
            display_name: user.display_name,
            role: user.role,
            is_superadmin: user.is_superadmin,
            tenant_id: tenant.id,
            tenant_name: tenant.name,
            tenant_slug: tenant.slug,
        },
    }))
}

pub async fn logout(
    State(state): State<AppState>,
    Json(req): Json<RefreshRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let token_hash = hash_refresh_token(&req.refresh_token);
    sqlx::query("DELETE FROM refresh_tokens WHERE token_hash = $1")
        .bind(&token_hash)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;
    Ok(StatusCode::NO_CONTENT)
}
