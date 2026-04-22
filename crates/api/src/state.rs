use crate::jwt::JwtKeys;
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub jwt: JwtKeys,
    pub public_url: String,
}
