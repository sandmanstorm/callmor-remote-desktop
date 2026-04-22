use crate::email::EmailConfig;
use crate::jwt::JwtKeys;
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub jwt: JwtKeys,
    pub email: Option<EmailConfig>,
    pub public_url: String,
}
