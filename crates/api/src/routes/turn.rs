//! Short-lived TURN credentials for WebRTC viewers.
//!
//! coturn runs with `use-auth-secret`, which expects clients to present a
//! time-limited username (`<expiry-unix-ts>:<anything>`) and a password that
//! is HMAC-SHA1(shared_secret, username) base64-encoded. We mint those here
//! and hand them to the browser at session start.
//!
//! Without this, the viewer only has public STUN and can't traverse
//! symmetric NATs (most home/corporate/mobile networks). With TURN it has a
//! guaranteed relay fallback.

use axum::{extract::State, http::StatusCode, Json};
use base64::Engine;
use hmac::{Hmac, Mac};
use serde::Serialize;
use sha1::Sha1;

use crate::state::AppState;

#[derive(Serialize)]
pub struct IceServer {
    pub urls: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential: Option<String>,
}

#[derive(Serialize)]
pub struct TurnResponse {
    pub ice_servers: Vec<IceServer>,
    pub ttl_seconds: i64,
}

/// Public — this endpoint is intentionally unauthenticated. The credentials
/// it mints are time-limited (1 hour), bound to coturn's shared secret, and
/// rate-limited in front of coturn itself. Exposing it anonymously is the
/// normal pattern: both Twilio and coturn's own docs do it this way because
/// blocking access to TURN blocks anyone who isn't yet logged in (e.g. the
/// /connect adhoc flow).
pub async fn get_turn_config(
    State(_state): State<AppState>,
) -> Result<Json<TurnResponse>, (StatusCode, String)> {
    let secret = std::env::var("TURN_SECRET")
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "TURN_SECRET not configured".into()))?;

    let turn_host = std::env::var("PUBLIC_TURN_HOST").unwrap_or_else(|_| "turn.callmor.ai".into());
    let turn_port: u16 = std::env::var("PUBLIC_TURN_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3478);

    // Credentials expire in 1 hour. coturn's check is `username prefix <= now`.
    let ttl_seconds: i64 = 3600;
    let expiry = chrono::Utc::now().timestamp() + ttl_seconds;
    // Username can have anything after the colon; adding a nonce makes it
    // unique per request which helps coturn's rate-limiting telemetry.
    let nonce: [u8; 6] = rand::random();
    let username = format!("{}:{}", expiry, hex::encode(nonce));

    let mut mac = Hmac::<Sha1>::new_from_slice(secret.as_bytes())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("HMAC init: {e}")))?;
    mac.update(username.as_bytes());
    let credential = base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes());

    // We advertise both STUN and TURN. The client tries STUN/direct first;
    // TURN is only used when direct/srflx paths fail.
    let stun_url = format!("stun:{turn_host}:{turn_port}");
    let turn_udp = format!("turn:{turn_host}:{turn_port}?transport=udp");
    let turn_tcp = format!("turn:{turn_host}:{turn_port}?transport=tcp");

    Ok(Json(TurnResponse {
        ice_servers: vec![
            IceServer {
                urls: vec![stun_url, "stun:stun.l.google.com:19302".into()],
                username: None,
                credential: None,
            },
            IceServer {
                urls: vec![turn_udp, turn_tcp],
                username: Some(username),
                credential: Some(credential),
            },
        ],
        ttl_seconds,
    }))
}
