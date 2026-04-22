//! Platform settings (SMTP, etc.) — superadmin only.

use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};

use crate::auth_extractor::AuthUser;
use crate::state::AppState;

fn require_superadmin(claims: &crate::jwt::Claims) -> Result<(), (StatusCode, String)> {
    if claims.is_superadmin {
        Ok(())
    } else {
        Err((StatusCode::FORBIDDEN, "Requires platform super-admin".into()))
    }
}

async fn get_setting(pool: &sqlx::PgPool, key: &str) -> Option<String> {
    sqlx::query_scalar("SELECT value FROM app_settings WHERE key = $1")
        .bind(key)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
}

async fn set_setting(
    pool: &sqlx::PgPool,
    key: &str,
    value: &str,
    user_id: uuid::Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO app_settings (key, value, updated_by) VALUES ($1, $2, $3)
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = now(), updated_by = $3",
    )
    .bind(key)
    .bind(value)
    .bind(user_id)
    .execute(pool)
    .await
    .map(|_| ())
}

// =========================================================================
// SMTP settings
// =========================================================================

#[derive(Serialize)]
pub struct SmtpSettingsResponse {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub from_email: String,
    pub from_name: String,
    pub tls: String,
    /// True if a password is configured (we never return the actual password).
    pub has_password: bool,
    pub configured: bool,
}

pub async fn get_smtp(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<SmtpSettingsResponse>, (StatusCode, String)> {
    require_superadmin(&claims)?;

    let host = get_setting(&state.db, "smtp.host").await.unwrap_or_default();
    let port: u16 = get_setting(&state.db, "smtp.port").await
        .and_then(|s| s.parse().ok()).unwrap_or(587);
    let username = get_setting(&state.db, "smtp.username").await.unwrap_or_default();
    let password = get_setting(&state.db, "smtp.password").await.unwrap_or_default();
    let from_email = get_setting(&state.db, "smtp.from_email").await.unwrap_or_default();
    let from_name = get_setting(&state.db, "smtp.from_name").await.unwrap_or_default();
    let tls = get_setting(&state.db, "smtp.tls").await.unwrap_or_else(|| "starttls".into());

    Ok(Json(SmtpSettingsResponse {
        configured: !host.is_empty(),
        host,
        port,
        username,
        from_email,
        from_name,
        tls,
        has_password: !password.is_empty(),
    }))
}

#[derive(Deserialize)]
pub struct UpdateSmtpRequest {
    pub host: String,
    pub port: u16,
    pub username: String,
    /// Only update password if provided. Empty string = keep existing.
    pub password: Option<String>,
    pub from_email: String,
    pub from_name: String,
    pub tls: String, // 'starttls' | 'implicit' | 'none'
}

pub async fn update_smtp(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(req): Json<UpdateSmtpRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    require_superadmin(&claims)?;

    if !["starttls", "implicit", "none"].contains(&req.tls.as_str()) {
        return Err((StatusCode::BAD_REQUEST, "tls must be starttls | implicit | none".into()));
    }

    let save = |key: &'static str, value: String| {
        let pool = state.db.clone();
        let sub = claims.sub;
        async move { set_setting(&pool, key, &value, sub).await }
    };

    save("smtp.host", req.host).await.map_err(db_err)?;
    save("smtp.port", req.port.to_string()).await.map_err(db_err)?;
    save("smtp.username", req.username).await.map_err(db_err)?;
    if let Some(pw) = req.password.filter(|p| !p.is_empty()) {
        save("smtp.password", pw).await.map_err(db_err)?;
    }
    save("smtp.from_email", req.from_email).await.map_err(db_err)?;
    save("smtp.from_name", req.from_name).await.map_err(db_err)?;
    save("smtp.tls", req.tls).await.map_err(db_err)?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn clear_smtp(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<StatusCode, (StatusCode, String)> {
    require_superadmin(&claims)?;
    sqlx::query("DELETE FROM app_settings WHERE key LIKE 'smtp.%'")
        .execute(&state.db)
        .await
        .map_err(db_err)?;
    Ok(StatusCode::NO_CONTENT)
}

fn db_err(e: sqlx::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}"))
}
