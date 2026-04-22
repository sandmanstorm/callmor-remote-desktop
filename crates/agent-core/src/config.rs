use std::path::Path;

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub relay_url: String,
    pub api_url: String,
    pub machine_id: String,
    pub agent_token: String,
}

impl AgentConfig {
    /// Load config from env vars + optional config file.
    ///
    /// Order of precedence: env vars > config_file > defaults.
    pub fn load(config_file: Option<&Path>) -> anyhow::Result<Self> {
        // Load config file first (if present) — env vars take precedence
        if let Some(path) = config_file {
            if path.exists() {
                load_kv_file(path)?;
            }
        }

        let relay_url = std::env::var("RELAY_URL")
            .unwrap_or_else(|_| "wss://relay.callmor.ai".into());
        let api_url = std::env::var("API_URL")
            .unwrap_or_else(|_| "https://api.callmor.ai".into());
        let machine_id = std::env::var("MACHINE_ID")
            .map_err(|_| anyhow::anyhow!("MACHINE_ID not set (edit config file or env)"))?;
        let agent_token = std::env::var("AGENT_TOKEN")
            .map_err(|_| anyhow::anyhow!("AGENT_TOKEN not set (edit config file or env)"))?;

        if agent_token == "CHANGE_ME" || machine_id == "CHANGE_ME" {
            anyhow::bail!("Config file still has CHANGE_ME placeholders");
        }

        Ok(AgentConfig { relay_url, api_url, machine_id, agent_token })
    }
}

/// Read a KEY=VALUE file and set env vars (only if not already set).
fn load_kv_file(path: &Path) -> anyhow::Result<()> {
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
