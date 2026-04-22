//! Agent self-enrollment on first run.
//!
//! When the installer is downloaded, the API injects a per-tenant
//! ENROLLMENT_TOKEN into the agent's config file. On first run the
//! agent calls /agent/enroll with this token, receives a permanent
//! MACHINE_ID + AGENT_TOKEN, and persists them (replacing the config).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize)]
struct EnrollRequest<'a> {
    enrollment_token: &'a str,
    hostname: &'a str,
    os: &'a str,
}

#[derive(Deserialize)]
pub struct EnrollResponse {
    pub machine_id: String,
    pub agent_token: String,
    pub relay_url: String,
    pub api_url: String,
}

/// Call the enroll endpoint. Returns machine credentials.
pub async fn enroll(
    api_url: &str,
    enrollment_token: &str,
    hostname: &str,
    os: &str,
) -> Result<EnrollResponse> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;

    let resp = client
        .post(format!("{api_url}/agent/enroll"))
        .json(&EnrollRequest {
            enrollment_token,
            hostname,
            os,
        })
        .send()
        .await
        .context("enroll request")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Enroll failed ({status}): {body}");
    }

    Ok(resp.json().await.context("parse enroll response")?)
}

/// Persist enrollment result to the agent config file.
/// Rewrites the file atomically. Clears the one-time ENROLLMENT_TOKEN.
pub fn save_to_config(
    config_path: &Path,
    result: &EnrollResponse,
) -> Result<()> {
    let contents = format!(
        "# Callmor Remote Desktop Agent Configuration\n\
         # Auto-written by agent on first run. Do not hand-edit unless you know what you're doing.\n\
         \n\
         RELAY_URL={}\n\
         API_URL={}\n\
         MACHINE_ID={}\n\
         AGENT_TOKEN={}\n",
        result.relay_url, result.api_url, result.machine_id, result.agent_token,
    );

    // Ensure parent directory exists
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    // Atomic write: temp file + rename
    let tmp = config_path.with_extension("conf.tmp");
    std::fs::write(&tmp, &contents).context("write config tmp")?;
    // chmod 600 where supported
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600)).ok();
    }
    std::fs::rename(&tmp, config_path).context("rename config")?;

    Ok(())
}
