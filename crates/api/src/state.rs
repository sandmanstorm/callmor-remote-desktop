use crate::jwt::JwtKeys;
use crate::storage::Storage;
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub jwt: JwtKeys,
    pub storage: Storage,
    /// User-facing web URL (e.g. https://remote.callmor.ai). Used for invite
    /// links, viewer redirects, and anything else a browser opens.
    pub public_url: String,
    /// Agent-facing API URL (e.g. https://api.callmor.ai). This is what we
    /// hand back to agents in enroll/register responses so they know where
    /// to POST heartbeats and fetch config. Must be distinct from the web
    /// URL when the two are served from different hostnames.
    pub api_url: String,
}
