mod capture;
mod input;
mod session;

use anyhow::Result;
use callmor_agent_core::config::AgentConfig;
use std::path::PathBuf;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    info!("Callmor Agent (macOS) v{}", env!("CARGO_PKG_VERSION"));

    // Config at /Library/Application Support/Callmor/agent.conf (system) or user's
    // ~/Library/Application Support/Callmor/agent.conf. Installer writes to system.
    let config_path: PathBuf = std::env::var("CALLMOR_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_config_path());

    let config = match AgentConfig::load(Some(&config_path)) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to load config: {e}");
            error!("Edit {} and set MACHINE_ID and AGENT_TOKEN from the Callmor dashboard.", config_path.display());
            std::process::exit(1);
        }
    };

    info!("Relay: {}, API: {}, Machine: {}", config.relay_url, config.api_url, config.machine_id);

    // Heartbeat task
    let hostname = hostname().unwrap_or_else(|| "unknown".into());
    {
        let api = config.api_url.clone();
        let token = config.agent_token.clone();
        let mid = config.machine_id.clone();
        let host = hostname.clone();
        tokio::spawn(async move {
            callmor_agent_core::heartbeat::run(api, token, mid, host, "macos", 30).await;
        });
    }

    // Session loop
    loop {
        match session::run(&config).await {
            Ok(()) => info!("Session ended cleanly"),
            Err(e) => error!("Session error: {e:#}"),
        }
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
}

#[cfg(target_os = "macos")]
fn default_config_path() -> PathBuf {
    PathBuf::from("/Library/Application Support/Callmor/agent.conf")
}

#[cfg(not(target_os = "macos"))]
fn default_config_path() -> PathBuf {
    PathBuf::from("/etc/callmor-agent/agent.conf")
}

fn hostname() -> Option<String> {
    std::process::Command::new("hostname")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
}
