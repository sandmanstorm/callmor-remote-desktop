use anyhow::Result;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub relay_url: String,
    pub api_url: String,
    pub machine_id: String,
    pub agent_token: String,
}

/// What we found in the config: fully-enrolled, needs-enrollment,
/// ad-hoc (login-less public install), or broken.
pub enum ConfigLoad {
    /// Ready to run. machine_id + agent_token present.
    Ready(AgentConfig),
    /// First run: we have an enrollment_token but no machine credentials yet.
    NeedsEnrollment {
        enrollment_token: String,
        api_url: String,
        relay_url: String,
        config_path: PathBuf,
    },
    /// Public installer, no enrollment token — use the ad-hoc flow. The agent
    /// calls /agent/adhoc/register, gets a code+pin, and shows them on screen.
    NeedsAdhoc {
        api_url: String,
        relay_url: String,
        config_path: PathBuf,
    },
    /// Neither enrollment_token nor machine credentials set.
    Missing,
}

impl AgentConfig {
    /// Load config from env + optional config file.
    /// Returns a ConfigLoad indicating what state we're in.
    pub fn load(config_file: Option<&Path>) -> Result<ConfigLoad> {
        let path = config_file.map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from(""));

        if !path.as_os_str().is_empty() && path.exists() {
            load_kv_file(&path)?;
        }

        let relay_url = std::env::var("RELAY_URL")
            .unwrap_or_else(|_| "wss://relay.callmor.ai".into());
        let api_url = std::env::var("API_URL")
            .unwrap_or_else(|_| "https://api.callmor.ai".into());
        let machine_id = std::env::var("MACHINE_ID").unwrap_or_default();
        let agent_token = std::env::var("AGENT_TOKEN").unwrap_or_default();
        let enrollment_token = std::env::var("ENROLLMENT_TOKEN").unwrap_or_default();

        let have_credentials = !machine_id.is_empty()
            && machine_id != "CHANGE_ME"
            && !agent_token.is_empty()
            && agent_token != "CHANGE_ME";

        if have_credentials {
            return Ok(ConfigLoad::Ready(AgentConfig {
                relay_url,
                api_url,
                machine_id,
                agent_token,
            }));
        }

        if !enrollment_token.is_empty() && enrollment_token != "CHANGE_ME" {
            return Ok(ConfigLoad::NeedsEnrollment {
                enrollment_token,
                api_url,
                relay_url,
                config_path: path,
            });
        }

        // Public installers drop ADHOC=1 into agent.conf so the agent knows
        // to self-register in login-less mode and show a code+pin.
        let adhoc_mode = std::env::var("ADHOC").unwrap_or_default();
        if adhoc_mode == "1" || adhoc_mode.eq_ignore_ascii_case("true") {
            return Ok(ConfigLoad::NeedsAdhoc {
                api_url,
                relay_url,
                config_path: path,
            });
        }

        Ok(ConfigLoad::Missing)
    }
}

/// Read a KEY=VALUE file and set env vars (only if not already set).
fn load_kv_file(path: &Path) -> Result<()> {
    let contents = std::fs::read_to_string(path)?;
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim().trim_matches('"');
            if std::env::var(key).is_err() {
                // SAFETY: called before any threads spawn (in main init).
                unsafe { std::env::set_var(key, value) };
            }
        }
    }
    Ok(())
}
