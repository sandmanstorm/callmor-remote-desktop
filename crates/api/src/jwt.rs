use anyhow::Result;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Build a strict HS256-only validator. Prevents algorithm confusion attacks
/// (e.g., `alg: none` or RS256 swap) that can occur with Validation::default().
fn strict_validation() -> Validation {
    let mut v = Validation::new(Algorithm::HS256);
    v.validate_exp = true;
    v.leeway = 0;
    v
}

/// Access token claims — used for dashboard API requests.
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,        // user_id
    pub tenant_id: Uuid,
    pub role: String,
    #[serde(default)]
    pub is_superadmin: bool,
    pub exp: i64,
    pub iat: i64,
}

/// Session token claims — short-lived, bound to a specific machine.
/// Used by browser to authenticate to the relay.
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionClaims {
    pub sub: Uuid,          // user_id
    pub tenant_id: Uuid,
    pub machine_id: Uuid,
    pub session_id: Uuid,
    pub permission: String, // 'view_only', 'full_control'
    pub exp: i64,
    pub iat: i64,
    #[serde(rename = "type")]
    pub token_type: String, // always "session"
}

#[derive(Clone)]
pub struct JwtKeys {
    encoding: EncodingKey,
    decoding: DecodingKey,
}

impl JwtKeys {
    pub fn from_secret(secret: &[u8]) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret),
            decoding: DecodingKey::from_secret(secret),
        }
    }

    pub fn create_access_token(&self, user_id: Uuid, tenant_id: Uuid, role: &str, is_superadmin: bool) -> Result<String> {
        let now = Utc::now();
        let claims = Claims {
            sub: user_id,
            tenant_id,
            role: role.to_string(),
            is_superadmin,
            iat: now.timestamp(),
            exp: (now + Duration::minutes(15)).timestamp(),
        };
        Ok(encode(&Header::default(), &claims, &self.encoding)?)
    }

    pub fn create_session_token(
        &self,
        user_id: Uuid,
        tenant_id: Uuid,
        machine_id: Uuid,
        session_id: Uuid,
        permission: &str,
    ) -> Result<String> {
        let now = Utc::now();
        let claims = SessionClaims {
            sub: user_id,
            tenant_id,
            machine_id,
            session_id,
            permission: permission.to_string(),
            iat: now.timestamp(),
            exp: (now + Duration::minutes(2)).timestamp(),
            token_type: "session".into(),
        };
        Ok(encode(&Header::default(), &claims, &self.encoding)?)
    }

    pub fn validate_token(&self, token: &str) -> Result<Claims> {
        let data = decode::<Claims>(token, &self.decoding, &strict_validation())?;
        Ok(data.claims)
    }

    pub fn validate_session_token(&self, token: &str) -> Result<SessionClaims> {
        let data = decode::<SessionClaims>(token, &self.decoding, &strict_validation())?;
        if data.claims.token_type != "session" {
            anyhow::bail!("Not a session token");
        }
        Ok(data.claims)
    }
}

/// Generate a random refresh token (64 hex chars)
pub fn generate_refresh_token() -> String {
    use rand::Rng;
    let bytes: [u8; 32] = rand::rng().random();
    hex::encode(bytes)
}

/// Hash a refresh token for storage
pub fn hash_refresh_token(token: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}
