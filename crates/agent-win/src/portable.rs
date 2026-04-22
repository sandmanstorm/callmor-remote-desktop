//! Portable (no-install) mode: single-click .exe that runs in the user's
//! session, shows a native window with access code + PIN, and runs the
//! WebRTC session loop in the background. The user can share the code with
//! anyone, or click "Install as Service" to persist across reboots.

use anyhow::Result;
use callmor_agent_core::config::{AgentConfig, ConfigLoad};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing::{error, info};

// ─── Shared state between background session thread and egui UI ─────────────

#[derive(Clone, Debug)]
pub enum Status {
    Starting,
    Registering,
    Online,
    ViewerConnected,
    Error(String),
}

impl Default for Status {
    fn default() -> Self {
        Status::Starting
    }
}

#[derive(Default)]
pub struct SharedState {
    pub status: Status,
    pub access_code: String,
    pub pin: String,
    pub hostname: String,
    pub machine_id: String,
    pub frames_sent: u64,
    pub viewer_count: usize,
    pub service_install_message: Option<(bool, String)>, // (success, message)
}

pub type Shared = Arc<Mutex<SharedState>>;

// ─── Entry point ────────────────────────────────────────────────────────────

pub fn run() -> Result<()> {
    info!("Callmor Remote Desktop (portable) v{}", env!("CARGO_PKG_VERSION"));

    let state: Shared = Arc::new(Mutex::new(SharedState::default()));

    // Background thread: tokio runtime running the agent session.
    {
        let state = state.clone();
        std::thread::spawn(move || {
            let rt = match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    set_error(&state, format!("Failed to build tokio runtime: {e}"));
                    return;
                }
            };
            rt.block_on(async move {
                if let Err(e) = run_agent(state.clone()).await {
                    set_error(&state, format!("{e:#}"));
                }
            });
        });
    }

    // Main thread: egui window.
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([460.0, 520.0])
            .with_min_inner_size([380.0, 420.0])
            .with_title("Callmor Remote Desktop")
            .with_resizable(true),
        ..Default::default()
    };

    eframe::run_native(
        "Callmor Remote Desktop",
        options,
        Box::new(move |cc| {
            // Slightly more comfortable default type scaling
            cc.egui_ctx.set_pixels_per_point(1.1);
            Ok(Box::new(CallmorApp { state: state.clone() }))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {e}"))
}

fn set_error(state: &Shared, msg: String) {
    error!("{msg}");
    if let Ok(mut st) = state.lock() {
        st.status = Status::Error(msg);
    }
}

// ─── Background: agent self-register + session loop ─────────────────────────

async fn run_agent(state: Shared) -> Result<()> {
    // Config path in user-writable LOCALAPPDATA so we don't need admin.
    let config_path = portable_config_path();

    set_status(&state, Status::Registering);

    let config = match AgentConfig::load(Some(&config_path))? {
        ConfigLoad::Ready(c) => c,
        ConfigLoad::NeedsEnrollment { .. } => {
            // Portable mode doesn't use tenant enrollment. If someone dropped
            // a per-tenant agent.conf here, fall back to adhoc to keep things
            // simple — they can always Install as Service afterwards.
            register_adhoc(&state, &config_path).await?
        }
        ConfigLoad::NeedsAdhoc { .. } | ConfigLoad::Missing => {
            register_adhoc(&state, &config_path).await?
        }
    };

    info!(
        "Relay: {}, API: {}, Machine: {}",
        config.relay_url, config.api_url, config.machine_id
    );

    // Update shared state so the UI reflects what was persisted/registered
    {
        let mut st = state.lock().unwrap();
        st.machine_id = config.machine_id.clone();
        if st.hostname.is_empty() {
            st.hostname = crate::hostname().unwrap_or_else(|| "this computer".into());
        }
        st.status = Status::Online;
    }

    // Heartbeat
    {
        let api = config.api_url.clone();
        let token = config.agent_token.clone();
        let mid = config.machine_id.clone();
        let host = state.lock().unwrap().hostname.clone();
        tokio::spawn(async move {
            callmor_agent_core::heartbeat::run(api, token, mid, host, "windows", 30).await;
        });
    }

    // Session loop
    loop {
        match crate::session::run(&config).await {
            Ok(()) => info!("Session ended cleanly"),
            Err(e) => error!("Session error: {e:#}"),
        }
        // Between sessions, reflect "online and idle" in the UI
        if let Ok(mut st) = state.lock() {
            if !matches!(st.status, Status::Error(_)) {
                st.status = Status::Online;
                st.viewer_count = 0;
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
}

async fn register_adhoc(state: &Shared, config_path: &std::path::Path) -> Result<AgentConfig> {
    let api_url = std::env::var("API_URL").unwrap_or_else(|_| "https://api.callmor.ai".into());
    let hostname = crate::hostname().unwrap_or_else(|| "unknown".into());

    if let Ok(mut st) = state.lock() {
        st.hostname = hostname.clone();
    }

    info!("Registering with {api_url}...");
    let r =
        callmor_agent_core::enrollment::register_adhoc(&api_url, &hostname, "windows").await?;
    info!("Registered — code {} pin {}", r.access_code, r.pin);
    callmor_agent_core::enrollment::save_adhoc_to_config(config_path, &r)?;

    if let Ok(mut st) = state.lock() {
        st.access_code = r.access_code.clone();
        st.pin = r.pin.clone();
    }

    Ok(AgentConfig {
        relay_url: r.relay_url,
        api_url: r.api_url,
        machine_id: r.machine_id,
        agent_token: r.agent_token,
    })
}

fn set_status(state: &Shared, status: Status) {
    if let Ok(mut st) = state.lock() {
        st.status = status;
    }
}

fn portable_config_path() -> PathBuf {
    #[cfg(windows)]
    {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            return PathBuf::from(local).join("Callmor").join("agent.conf");
        }
    }
    PathBuf::from("agent.conf")
}

// ─── egui UI ────────────────────────────────────────────────────────────────

struct CallmorApp {
    state: Shared,
}

impl eframe::App for CallmorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Keep repainting so status + counters stay live
        ctx.request_repaint_after(std::time::Duration::from_millis(500));

        // Dark theme with a hint of blue accent
        let mut visuals = egui::Visuals::dark();
        visuals.window_fill = egui::Color32::from_rgb(10, 10, 12);
        visuals.panel_fill = egui::Color32::from_rgb(10, 10, 12);
        ctx.set_visuals(visuals);

        egui::CentralPanel::default().show(ctx, |ui| {
            let st = self.state.lock().unwrap();

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.add_space(20.0);
                ui.heading(egui::RichText::new("Callmor").size(26.0).strong());
                ui.label(
                    egui::RichText::new("Remote Desktop")
                        .size(14.0)
                        .color(egui::Color32::from_gray(150)),
                );
            });
            ui.add_space(4.0);
            ui.separator();
            ui.add_space(16.0);

            // ── Status row ──
            ui.horizontal(|ui| {
                ui.add_space(20.0);
                let (text, color) = match &st.status {
                    Status::Starting => ("Starting...", egui::Color32::GRAY),
                    Status::Registering => (
                        "Registering with server...",
                        egui::Color32::from_rgb(234, 179, 8),
                    ),
                    Status::Online => (
                        "Ready — share the code below",
                        egui::Color32::from_rgb(34, 197, 94),
                    ),
                    Status::ViewerConnected => (
                        "Connected — someone is viewing",
                        egui::Color32::from_rgb(59, 130, 246),
                    ),
                    Status::Error(_) => ("Error", egui::Color32::from_rgb(239, 68, 68)),
                };
                // Status dot
                let (rect, _) = ui.allocate_exact_size([10.0, 10.0].into(), egui::Sense::hover());
                ui.painter().circle_filled(rect.center(), 5.0, color);
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(text)
                        .color(color)
                        .size(13.0)
                        .strong(),
                );
            });

            if let Status::Error(msg) = &st.status {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    ui.colored_label(egui::Color32::from_rgb(239, 68, 68), msg);
                });
            }

            ui.add_space(20.0);

            // ── Code + PIN ──
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("Your access code")
                        .color(egui::Color32::from_gray(160))
                        .size(13.0),
                );
                ui.add_space(6.0);
                let code_display = if st.access_code.is_empty() {
                    "— — — —".to_string()
                } else {
                    format_code(&st.access_code)
                };
                ui.label(
                    egui::RichText::new(&code_display)
                        .monospace()
                        .size(34.0)
                        .color(egui::Color32::WHITE),
                );
                ui.add_space(6.0);
                if !st.access_code.is_empty() {
                    if ui
                        .add(egui::Button::new("  Copy code  "))
                        .on_hover_text("Copy to clipboard")
                        .clicked()
                    {
                        ctx.copy_text(st.access_code.clone());
                    }
                }

                ui.add_space(20.0);

                ui.label(
                    egui::RichText::new("PIN")
                        .color(egui::Color32::from_gray(160))
                        .size(13.0),
                );
                ui.add_space(6.0);
                let pin_display = if st.pin.is_empty() { "— — — —".to_string() } else { st.pin.clone() };
                ui.label(
                    egui::RichText::new(&pin_display)
                        .monospace()
                        .size(30.0)
                        .color(egui::Color32::WHITE),
                );
                ui.add_space(6.0);
                if !st.pin.is_empty() {
                    if ui.add(egui::Button::new("  Copy PIN  ")).clicked() {
                        ctx.copy_text(st.pin.clone());
                    }
                }
            });

            ui.add_space(18.0);
            ui.separator();
            ui.add_space(12.0);

            // ── Instructions ──
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("Anyone with this code can connect from")
                        .size(12.0)
                        .color(egui::Color32::from_gray(150)),
                );
                ui.label(
                    egui::RichText::new("https://remote.callmor.ai/connect")
                        .size(13.0)
                        .color(egui::Color32::from_rgb(96, 165, 250))
                        .monospace(),
                );
            });

            ui.add_space(16.0);

            // ── Footer actions ──
            ui.horizontal(|ui| {
                ui.add_space(20.0);
                if ui
                    .add(egui::Button::new("Install as Windows Service"))
                    .on_hover_text(
                        "Copies the agent to Program Files and registers it so it runs at boot.\n\
                         Requires administrator approval.",
                    )
                    .clicked()
                {
                    drop(st);
                    let state = self.state.clone();
                    std::thread::spawn(move || match crate::service_install::launch_self_installer()
                    {
                        Ok(()) => {
                            if let Ok(mut s) = state.lock() {
                                s.service_install_message = Some((
                                    true,
                                    "Service installer launched. Follow the UAC prompt.".into(),
                                ));
                            }
                        }
                        Err(e) => {
                            if let Ok(mut s) = state.lock() {
                                s.service_install_message =
                                    Some((false, format!("Install failed: {e:#}")));
                            }
                        }
                    });
                    return;
                }
                ui.add_space(6.0);
                if ui.add(egui::Button::new("Quit")).clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });

            if let Some((ok, msg)) = &self.state.lock().unwrap().service_install_message.clone() {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    let color = if *ok {
                        egui::Color32::from_rgb(34, 197, 94)
                    } else {
                        egui::Color32::from_rgb(239, 68, 68)
                    };
                    ui.colored_label(color, msg);
                });
            }
        });
    }
}

/// "ABCD1234" -> "ABCD-1234" for easier reading.
fn format_code(code: &str) -> String {
    if code.len() == 8 && code.chars().all(|c| c.is_ascii_alphanumeric()) {
        format!("{}-{}", &code[..4], &code[4..])
    } else {
        code.to_string()
    }
}
