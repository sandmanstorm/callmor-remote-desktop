use anyhow::{Context, Result};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct SessionClaims {
    pub sub: Uuid,
    pub tenant_id: Uuid,
    pub machine_id: Uuid,
    pub permission: String,
    pub exp: i64,
    #[serde(rename = "type")]
    pub token_type: String,
}

/// Validates agent_token against the DB.
/// Returns the machine_id if valid.
pub async fn validate_agent_token(
    pool: &PgPool,
    token: &str,
    claimed_machine_id: &str,
) -> Result<Uuid> {
    let row: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM machines WHERE agent_token = $1")
            .bind(token)
            .fetch_optional(pool)
            .await?;

    let machine_id = row.ok_or_else(|| anyhow::anyhow!("Invalid agent token"))?.0;

    // Ensure agent claims the same machine_id
    let claimed: Uuid = claimed_machine_id.parse()
        .context("machine_id must be a UUID")?;
    if machine_id != claimed {
        anyhow::bail!("Agent token/machine_id mismatch");
    }
    Ok(machine_id)
}

/// Validates a browser session JWT.
/// Returns the machine_id if valid and matches the claimed machine_id.
pub fn validate_session_token(
    key: &DecodingKey,
    token: &str,
    claimed_machine_id: &str,
) -> Result<Uuid> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    validation.leeway = 0;
    let data = decode::<SessionClaims>(token, key, &validation)
        .context("Invalid session token")?;

    if data.claims.token_type != "session" {
        anyhow::bail!("Not a session token");
    }

    let claimed: Uuid = claimed_machine_id.parse()
        .context("machine_id must be a UUID")?;
    if data.claims.machine_id != claimed {
        anyhow::bail!("Session token/machine_id mismatch");
    }
    Ok(data.claims.machine_id)
}
