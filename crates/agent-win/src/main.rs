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

    info!("Callmor Agent (Windows) v{}", env!("CARGO_PKG_VERSION"));

    // Load config from C:\ProgramData\Callmor\agent.conf or env
    let config_path: PathBuf = std::env::var("CALLMOR_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_config_path());

    let config = match AgentConfig::load(Some(&config_path))? {
        callmor_agent_core::config::ConfigLoad::Ready(c) => c,
        callmor_agent_core::config::ConfigLoad::NeedsEnrollment {
            enrollment_token,
            api_url,
            relay_url: _,
            config_path,
        } => {
            info!("First run: enrolling with API {api_url}...");
            let hostname = hostname().unwrap_or_else(|| "unknown".into());
            let result = callmor_agent_core::enrollment::enroll(
                &api_url,
                &enrollment_token,
                &hostname,
                "windows",
            )
            .await?;
            info!("Enrolled as machine {}", result.machine_id);
            callmor_agent_core::enrollment::save_to_config(&config_path, &result)?;
            AgentConfig {
                relay_url: result.relay_url,
                api_url: result.api_url,
                machine_id: result.machine_id,
                agent_token: result.agent_token,
            }
        }
        callmor_agent_core::config::ConfigLoad::NeedsAdhoc {
            api_url,
            relay_url: _,
            config_path,
        } => {
            info!("Ad-hoc mode: self-registering with API {api_url}...");
            let hostname = hostname().unwrap_or_else(|| "unknown".into());
            let result = callmor_agent_core::enrollment::register_adhoc(&api_url, &hostname, "windows").await?;
            info!("Registered as machine {} — code {}, pin {}", result.machine_id, result.access_code, result.pin);
            callmor_agent_core::display_code::show(&result.access_code, &result.pin);
            callmor_agent_core::enrollment::save_adhoc_to_config(&config_path, &result)?;
            AgentConfig {
                relay_url: result.relay_url,
                api_url: result.api_url,
                machine_id: result.machine_id,
                agent_token: result.agent_token,
            }
        }
        callmor_agent_core::config::ConfigLoad::Missing => {
            error!("No config at {}. Agent cannot start.", config_path.display());
            std::process::exit(1);
        }
    };

    info!("Relay: {}, API: {}, Machine: {}", config.relay_url, config.api_url, config.machine_id);

    // Start heartbeat task
    let hostname = hostname().unwrap_or_else(|| "unknown".into());
    {
        let api = config.api_url.clone();
        let token = config.agent_token.clone();
        let mid = config.machine_id.clone();
        let host = hostname.clone();
        tokio::spawn(async move {
            callmor_agent_core::heartbeat::run(api, token, mid, host, "windows", 30).await;
        });
    }

    // Run session loop: connect to relay, wait for browser, stream. Reconnect on error.
    loop {
        match session::run(&config).await {
            Ok(()) => info!("Session ended cleanly"),
            Err(e) => error!("Session error: {e:#}"),
        }
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
}

#[cfg(windows)]
fn default_config_path() -> PathBuf {
    PathBuf::from(r"C:\ProgramData\Callmor\agent.conf")
}

#[cfg(not(windows))]
fn default_config_path() -> PathBuf {
    PathBuf::from("/etc/callmor-agent/agent.conf")
}

fn hostname() -> Option<String> {
    std::process::Command::new(if cfg!(windows) { "hostname" } else { "hostname" })
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
}
