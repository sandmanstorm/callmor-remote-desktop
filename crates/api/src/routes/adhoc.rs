//! Ad-hoc (login-less) machine flow — ScreenConnect-style.
//!
//! Anyone can download a public installer, run it, and the agent will
//! self-register without any tenant. The agent displays a 9-char access code
//! + 4-digit PIN on the remote screen. Anyone with both can connect via
//! `/connect` — no user account required.
//!
//! A tenant user can later call `/machines/claim` with code+pin to move the
//! machine into their tenant, at which point it becomes a regular managed
//! machine and drops out of the adhoc table.

use axum::{extract::State, http::StatusCode, Json};
use rand::Rng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth_extractor::AuthUser;
use crate::state::AppState;

// --- Agent-side: self-registration with no auth ---

#[derive(Deserialize)]
pub struct AdhocRegisterRequest {
    pub hostname: String,
    pub os: String,
}

#[derive(Serialize)]
pub struct AdhocRegisterResponse {
    pub machine_id: Uuid,
    pub agent_token: String,
    pub access_code: String,
    pub pin: String,
    pub relay_url: String,
    pub api_url: String,
}

/// Generate a base32 code (no O/0/I/1/L to avoid confusion), 8 chars, then
/// display it with a hyphen after 4 for readability on the remote screen.
fn generate_access_code() -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789"; // 31 chars
    let mut rng = rand::rng();
    (0..8)
        .map(|_| ALPHABET[rng.random_range(0..ALPHABET.len())] as char)
        .collect()
}

fn generate_pin() -> String {
    let mut rng = rand::rng();
    format!("{:04}", rng.random_range(0..10_000))
}

fn generate_agent_token() -> String {
    let mut rng = rand::rng();
    let bytes: [u8; 32] = rng.random();
    format!("cmt_{}", hex::encode(bytes))
}

pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<AdhocRegisterRequest>,
) -> Result<Json<AdhocRegisterResponse>, (StatusCode, String)> {
    let os = match req.os.to_lowercase().as_str() {
        "linux" | "windows" | "macos" => req.os.to_lowercase(),
        _ => return Err((StatusCode::BAD_REQUEST, "Invalid os".into())),
    };

    // Retry up to 5x in the cosmically unlikely event of access_code collision
    let mut attempts = 0;
    let row = loop {
        attempts += 1;
        let access_code = generate_access_code();
        let pin = generate_pin();
        let agent_token = generate_agent_token();
        let result = sqlx::query_as::<_, (Uuid, String, String, String)>(
            "INSERT INTO adhoc_machines (access_code, pin, agent_token, hostname, os) \
             VALUES ($1, $2, $3, $4, $5) \
             RETURNING id, access_code, pin, agent_token",
        )
        .bind(&access_code)
        .bind(&pin)
        .bind(&agent_token)
        .bind(&req.hostname)
        .bind(&os)
        .fetch_one(&state.db)
        .await;
        match result {
            Ok(r) => break r,
            Err(e) if attempts < 5 && e.to_string().contains("duplicate") => continue,
            Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}"))),
        }
    };

    let relay_url = std::env::var("PUBLIC_RELAY_URL")
        .unwrap_or_else(|_| "wss://relay.callmor.ai".into());

    Ok(Json(AdhocRegisterResponse {
        machine_id: row.0,
        access_code: row.1,
        pin: row.2,
        agent_token: row.3,
        relay_url,
        api_url: state.api_url.clone(),
    }))
}

// --- Viewer-side: /connect with code+pin, no login ---

#[derive(Deserialize)]
pub struct ConnectRequest {
    pub access_code: String,
    pub pin: String,
}

#[derive(Serialize)]
pub struct ConnectResponse {
    pub machine_id: Uuid,
    pub session_token: String,
    pub relay_url: String,
    pub hostname: String,
}

/// Trade a code+pin for a short-lived session JWT the browser can use to
/// connect through the relay. No user account involved.
pub async fn connect(
    State(state): State<AppState>,
    Json(req): Json<ConnectRequest>,
) -> Result<Json<ConnectResponse>, (StatusCode, String)> {
    // Normalize: accept "K7F3-9QPZ" or "K7F39QPZ", case-insensitive
    let code = req
        .access_code
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_uppercase();

    if code.is_empty() || req.pin.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Code and PIN are required".into()));
    }

    let row: Option<(Uuid, String, String, bool)> = sqlx::query_as(
        "SELECT id, pin, hostname, online FROM adhoc_machines \
         WHERE access_code = $1 AND expires_at > now()",
    )
    .bind(&code)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    let (machine_id, stored_pin, hostname, _online) = row.ok_or((
        StatusCode::UNAUTHORIZED,
        "Invalid or expired code".into(),
    ))?;

    // Constant-time compare would be nicer, but PINs are short-lived and
    // rate-limiting is the real defense here; tokio::time + axum middleware
    // can add that later if needed.
    if stored_pin != req.pin {
        return Err((StatusCode::UNAUTHORIZED, "Invalid code or PIN".into()));
    }

    // Mint a 2-minute session token. We use Uuid::nil() sentinels for
    // tenant_id/user_id since this connection isn't owned by any tenant user.
    // The relay only cares that machine_id matches, so this is safe.
    let session_id = Uuid::new_v4();
    let session_token = state
        .jwt
        .create_session_token(Uuid::nil(), Uuid::nil(), machine_id, session_id, "full_control")
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("JWT error: {e}")))?;

    let relay_url = std::env::var("PUBLIC_RELAY_URL")
        .unwrap_or_else(|_| "wss://relay.callmor.ai".into());

    Ok(Json(ConnectResponse {
        machine_id,
        session_token,
        relay_url,
        hostname,
    }))
}

// --- Tenant-side: claim an adhoc machine into your tenant ---

#[derive(Deserialize)]
pub struct ClaimRequest {
    pub access_code: String,
    pub pin: String,
    /// Optional display name; if omitted we use the hostname.
    pub name: Option<String>,
}

#[derive(Serialize)]
pub struct ClaimResponse {
    pub machine_id: Uuid,
    pub name: String,
}

/// Move an adhoc machine into the caller's tenant. The machine keeps the same
/// machine_id and agent_token so the already-running agent doesn't need to
/// re-enroll — we just migrate the row between tables in one transaction.
pub async fn claim(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(req): Json<ClaimRequest>,
) -> Result<Json<ClaimResponse>, (StatusCode, String)> {
    if !["owner", "admin"].contains(&claims.role.as_str()) {
        return Err((StatusCode::FORBIDDEN, "Requires admin or owner".into()));
    }

    let code = req
        .access_code
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_uppercase();

    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    let row: Option<(Uuid, String, String, String, String, bool, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as(
            "SELECT id, pin, agent_token, hostname, os, online, last_seen \
             FROM adhoc_machines WHERE access_code = $1 AND claimed_at IS NULL \
             FOR UPDATE",
        )
        .bind(&code)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    let (adhoc_id, stored_pin, agent_token, hostname, os, online, last_seen) = row.ok_or((
        StatusCode::NOT_FOUND,
        "Code not found or already claimed".into(),
    ))?;

    if stored_pin != req.pin {
        return Err((StatusCode::UNAUTHORIZED, "Invalid code or PIN".into()));
    }

    let name = req
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&hostname)
        .to_string();

    // Insert into machines with the same agent_token so the agent's websocket
    // connection stays valid after the claim — the relay just starts finding
    // it in `machines` instead of `adhoc_machines`.
    let machine_id: Uuid = sqlx::query_scalar(
        "INSERT INTO machines (tenant_id, name, hostname, os, agent_token, is_online, last_seen) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
    )
    .bind(claims.tenant_id)
    .bind(&name)
    .bind(&hostname)
    .bind(&os)
    .bind(&agent_token)
    .bind(online)
    .bind(last_seen)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    // Mark the adhoc row claimed (kept for audit, swept later). We also clear
    // agent_token + access_code so re-use is impossible.
    sqlx::query(
        "UPDATE adhoc_machines \
         SET claimed_at = now(), claimed_into_tenant = $1, \
             access_code = 'CLAIMED:' || id::text, \
             agent_token = 'CLAIMED:' || id::text \
         WHERE id = $2",
    )
    .bind(claims.tenant_id)
    .bind(adhoc_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    tx.commit()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    crate::audit::log(
        &state.db,
        &crate::audit::ctx_from_claims(&claims),
        "machine.claimed",
        Some("machine"),
        Some(machine_id),
        serde_json::json!({"name": name, "hostname": hostname, "os": os}),
    )
    .await;

    Ok(Json(ClaimResponse { machine_id, name }))
}

// Background sweep — called from the API's periodic task loop.

pub async fn sweep_expired(pool: &sqlx::PgPool) {
    // Mark stale-but-not-expired offline
    let _ = sqlx::query(
        "UPDATE adhoc_machines SET online = false \
         WHERE online = true AND last_seen < now() - interval '90 seconds'",
    )
    .execute(pool)
    .await;

    // Purge anything past expiry (24h from creation by default)
    let _ = sqlx::query("DELETE FROM adhoc_machines WHERE expires_at < now() AND claimed_at IS NULL")
        .execute(pool)
        .await;
}
