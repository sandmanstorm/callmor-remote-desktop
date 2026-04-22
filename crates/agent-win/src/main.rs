//! Callmor Remote Desktop — Windows agent.
//!
//! One binary, three modes depending on how it's invoked:
//!
//!   * Default (user double-click): portable GUI mode. Runs in the user's
//!     session, shows a native window with code + PIN, session loop in
//!     background. Config lives under %LOCALAPPDATA%\Callmor\.
//!
//!   * `--install-service`: self-elevates, copies the running .exe to
//!     Program Files, registers a Windows service pointing at it.
//!
//!   * `--service` (invoked by Windows Service Control Manager): headless
//!     service mode. No GUI. Config under C:\ProgramData\Callmor\.
//!
//!   * `--uninstall-service`: stops + removes the service.
//!
//! The GUI-vs-service split matters because services run in Session 0 and
//! can't display windows to logged-in users — but portable mode is exactly
//! what you want for ad-hoc remote support.

// Hide the Windows console window for GUI builds (portable mode).
// Service mode doesn't have a console anyway.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod capture;
mod input;
mod portable;
mod service_install;
mod session;

use anyhow::Result;
use callmor_agent_core::config::AgentConfig;
use std::path::PathBuf;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

enum Mode {
    Portable,
    InstallService,
    UninstallService,
    ServiceMain,
}

fn parse_mode() -> Mode {
    let args: Vec<String> = std::env::args().collect();
    for a in &args[1..] {
        match a.as_str() {
            "--install-service" | "/install" => return Mode::InstallService,
            "--uninstall-service" | "/uninstall" => return Mode::UninstallService,
            "--service" | "/service" => return Mode::ServiceMain,
            _ => {}
        }
    }
    Mode::Portable
}

fn main() {
    let mode = parse_mode();
    init_tracing(&mode);

    let result = match mode {
        Mode::Portable => portable::run(),
        Mode::InstallService => service_install::install(),
        Mode::UninstallService => service_install::uninstall(),
        Mode::ServiceMain => run_service_headless(),
    };

    if let Err(e) = result {
        let msg = format!("Callmor failed to start:\n\n{e:#}\n\nSee %LOCALAPPDATA%\\Callmor\\agent.log for details.");
        error!("Fatal: {e:#}");
        show_fatal_dialog(&msg);
        std::process::exit(1);
    }
}

/// Tracing subscriber that writes both to stderr (for service/dev mode) and
/// to a log file (crucial for portable mode where windows_subsystem = windows
/// means there is no console). Without the file, any early failure — failed
/// DLL load, eframe OpenGL init error, anything — is completely silent.
fn init_tracing(mode: &Mode) {
    use tracing_subscriber::fmt;

    let log_path = match mode {
        Mode::Portable | Mode::InstallService | Mode::UninstallService => {
            std::env::var("LOCALAPPDATA")
                .ok()
                .map(|p| std::path::PathBuf::from(p).join("Callmor").join("agent.log"))
        }
        Mode::ServiceMain => Some(std::path::PathBuf::from(
            r"C:\ProgramData\Callmor\agent.log",
        )),
    };

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into());

    if let Some(path) = log_path {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        // Open append-mode; truncate would lose crash info between launches.
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            Ok(f) => {
                fmt()
                    .with_env_filter(env_filter)
                    .with_writer(std::sync::Mutex::new(f))
                    .with_ansi(false)
                    .init();
                tracing::info!("--- Callmor agent starting ---");
                return;
            }
            Err(_) => {
                // Fall through to stderr-only subscriber
            }
        }
    }

    fmt().with_env_filter(env_filter).init();
}

/// Show an error dialog so the user at least sees *something* when the agent
/// can't start. Without this, portable mode fails silently (no console).
#[cfg(windows)]
fn show_fatal_dialog(message: &str) {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

    let title: Vec<u16> = "Callmor — Startup Error\0".encode_utf16().collect();
    let body: Vec<u16> = OsStr::new(message).encode_wide().chain(Some(0)).collect();
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(body.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}

#[cfg(not(windows))]
fn show_fatal_dialog(message: &str) {
    eprintln!("{message}");
}

/// Run the agent as a Windows service (headless). Config path defaults to
/// ProgramData. No GUI. This is the existing pre-GUI code path, preserved
/// for service installs.
#[tokio::main]
async fn run_service_headless() -> Result<()> {
    info!("Callmor Agent (service mode) v{}", env!("CARGO_PKG_VERSION"));

    let config_path: PathBuf = std::env::var("CALLMOR_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_service_config_path());

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
            let result =
                callmor_agent_core::enrollment::register_adhoc(&api_url, &hostname, "windows").await?;
            info!(
                "Registered as machine {} — code {}, pin {}",
                result.machine_id, result.access_code, result.pin
            );
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

    info!(
        "Relay: {}, API: {}, Machine: {}",
        config.relay_url, config.api_url, config.machine_id
    );

    // Heartbeat
    let hostname_str = hostname().unwrap_or_else(|| "unknown".into());
    {
        let api = config.api_url.clone();
        let token = config.agent_token.clone();
        let mid = config.machine_id.clone();
        let host = hostname_str.clone();
        tokio::spawn(async move {
            callmor_agent_core::heartbeat::run(api, token, mid, host, "windows", 30).await;
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

#[cfg(windows)]
fn default_service_config_path() -> PathBuf {
    PathBuf::from(r"C:\ProgramData\Callmor\agent.conf")
}

#[cfg(not(windows))]
fn default_service_config_path() -> PathBuf {
    PathBuf::from("/etc/callmor-agent/agent.conf")
}

pub(crate) fn hostname() -> Option<String> {
    std::process::Command::new("hostname")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
}
