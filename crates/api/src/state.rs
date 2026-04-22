use crate::jwt::JwtKeys;
use crate::storage::Storage;
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub jwt: JwtKeys,
    pub storage: Storage,
    pub public_url: String,
}
