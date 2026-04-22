//! Portable (no-install) mode: single-click .exe that runs in the user's
//! session, shows a native Win32 window with access code + PIN, and runs the
//! WebRTC session loop in the background. No GPU/OpenGL/D3D required — the
//! GUI is drawn by Windows itself with GDI, so it works everywhere:
//! RDP, Hyper-V, Windows Server core, any Windows back to Vista.

use anyhow::Result;
use callmor_agent_core::config::{AgentConfig, ConfigLoad};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing::{error, info};

// ─── Shared state between background session thread and Win32 UI ────────────

#[derive(Clone, Debug, PartialEq, Eq)]
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
    pub viewer_count: usize,
    pub service_install_message: Option<(bool, String)>,
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

    // Main thread: native Win32 UI (GDI).
    #[cfg(windows)]
    {
        ui::run(state)
    }
    #[cfg(not(windows))]
    {
        // Dev/CI: no GUI, just let the agent thread run.
        let _ = state;
        std::thread::park();
        Ok(())
    }
}

fn set_error(state: &Shared, msg: String) {
    error!("{msg}");
    if let Ok(mut st) = state.lock() {
        st.status = Status::Error(msg);
    }
}

// ─── Background: agent self-register + session loop ─────────────────────────

async fn run_agent(state: Shared) -> Result<()> {
    let config_path = portable_config_path();

    set_status(&state, Status::Registering);

    let config = match AgentConfig::load(Some(&config_path))? {
        ConfigLoad::Ready(c) => c,
        ConfigLoad::NeedsEnrollment { .. } => register_adhoc(&state, &config_path).await?,
        ConfigLoad::NeedsAdhoc { .. } | ConfigLoad::Missing => {
            register_adhoc(&state, &config_path).await?
        }
    };

    info!(
        "Relay: {}, API: {}, Machine: {}",
        config.relay_url, config.api_url, config.machine_id
    );

    {
        let mut st = state.lock().unwrap();
        st.machine_id = config.machine_id.clone();
        if st.hostname.is_empty() {
            st.hostname = crate::hostname().unwrap_or_else(|| "this computer".into());
        }
        // If Ready-from-config, we never went through adhoc-register, so pull
        // the code/pin from the on-disk config.
        if st.access_code.is_empty() {
            if let Ok(contents) = std::fs::read_to_string(&config_path) {
                for line in contents.lines() {
                    if let Some((k, v)) = line.split_once('=') {
                        match k.trim() {
                            "ACCESS_CODE" => st.access_code = v.trim().to_string(),
                            "PIN" => st.pin = v.trim().to_string(),
                            _ => {}
                        }
                    }
                }
            }
        }
        st.status = Status::Online;
    }

    {
        let api = config.api_url.clone();
        let token = config.agent_token.clone();
        let mid = config.machine_id.clone();
        let host = state.lock().unwrap().hostname.clone();
        tokio::spawn(async move {
            callmor_agent_core::heartbeat::run(api, token, mid, host, "windows", 30).await;
        });
    }

    loop {
        match crate::session::run(&config).await {
            Ok(()) => info!("Session ended cleanly"),
            Err(e) => error!("Session error: {e:#}"),
        }
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
    let r = callmor_agent_core::enrollment::register_adhoc(&api_url, &hostname, "windows").await?;
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

fn format_code(code: &str) -> String {
    if code.len() == 8 && code.chars().all(|c| c.is_ascii_alphanumeric()) {
        format!("{}-{}", &code[..4], &code[4..])
    } else {
        code.to_string()
    }
}

// ─── Native Win32 UI via native-windows-gui ─────────────────────────────────

#[cfg(windows)]
mod ui {
    use super::*;
    use native_windows_gui as nwg;
    use std::cell::RefCell;
    use std::rc::Rc;

    struct App {
        window: nwg::Window,
        title: nwg::Label,
        status: nwg::Label,
        code_label: nwg::Label,
        code_value: nwg::Label,
        code_copy: nwg::Button,
        pin_label: nwg::Label,
        pin_value: nwg::Label,
        pin_copy: nwg::Button,
        instructions: nwg::Label,
        install_btn: nwg::Button,
        quit_btn: nwg::Button,
        notice: nwg::Notice,
        timer: nwg::Timer,
        fonts: Fonts,
        state: Shared,
        last_rendered: RefCell<Option<Snapshot>>,
    }

    struct Fonts {
        heading: nwg::Font,
        mono_big: nwg::Font,
        body: nwg::Font,
    }

    #[derive(Clone, PartialEq, Eq)]
    struct Snapshot {
        status: Status,
        access_code: String,
        pin: String,
        install_msg: Option<(bool, String)>,
    }

    pub fn run(state: Shared) -> anyhow::Result<()> {
        nwg::init().map_err(|e| anyhow::anyhow!("nwg::init failed: {e:?}"))?;

        let app = Rc::new(build(state.clone())?);

        // Timer: poll SharedState every 500ms and refresh the UI.
        app.timer.start();

        let app_for_events = app.clone();
        let _handler = nwg::full_bind_event_handler(&app.window.handle, move |evt, _data, handle| {
            match evt {
                nwg::Event::OnWindowClose => {
                    if &handle == &app_for_events.window.handle {
                        nwg::stop_thread_dispatch();
                    }
                }
                nwg::Event::OnButtonClick => {
                    if &handle == &app_for_events.code_copy.handle {
                        copy_to_clipboard(&app_for_events.state.lock().unwrap().access_code);
                    } else if &handle == &app_for_events.pin_copy.handle {
                        copy_to_clipboard(&app_for_events.state.lock().unwrap().pin);
                    } else if &handle == &app_for_events.install_btn.handle {
                        let state = app_for_events.state.clone();
                        std::thread::spawn(move || match crate::service_install::launch_self_installer() {
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
                    } else if &handle == &app_for_events.quit_btn.handle {
                        nwg::stop_thread_dispatch();
                    }
                }
                nwg::Event::OnTimerTick => {
                    if &handle == &app_for_events.timer.handle {
                        refresh(&app_for_events);
                    }
                }
                _ => {}
            }
        });

        nwg::dispatch_thread_events();
        Ok(())
    }

    fn build(state: Shared) -> anyhow::Result<App> {
        // Fonts
        let mut heading = nwg::Font::default();
        nwg::Font::builder()
            .family("Segoe UI")
            .size(22)
            .weight(700)
            .build(&mut heading)
            .map_err(|e| anyhow::anyhow!("font heading: {e:?}"))?;

        let mut mono_big = nwg::Font::default();
        nwg::Font::builder()
            .family("Consolas")
            .size(36)
            .weight(600)
            .build(&mut mono_big)
            .map_err(|e| anyhow::anyhow!("font mono: {e:?}"))?;

        let mut body = nwg::Font::default();
        nwg::Font::builder()
            .family("Segoe UI")
            .size(15)
            .build(&mut body)
            .map_err(|e| anyhow::anyhow!("font body: {e:?}"))?;

        // Window
        let mut window = nwg::Window::default();
        nwg::Window::builder()
            .size((460, 520))
            .center(true)
            .title("Callmor Remote Desktop")
            .flags(nwg::WindowFlags::WINDOW | nwg::WindowFlags::VISIBLE)
            .build(&mut window)
            .map_err(|e| anyhow::anyhow!("window: {e:?}"))?;

        // Title
        let mut title = nwg::Label::default();
        nwg::Label::builder()
            .text("Callmor Remote Desktop")
            .position((20, 14))
            .size((420, 30))
            .font(Some(&heading))
            .parent(&window)
            .build(&mut title)
            .map_err(|e| anyhow::anyhow!("title: {e:?}"))?;

        // Status
        let mut status = nwg::Label::default();
        nwg::Label::builder()
            .text("● Starting...")
            .position((20, 52))
            .size((420, 24))
            .font(Some(&body))
            .parent(&window)
            .build(&mut status)
            .map_err(|e| anyhow::anyhow!("status: {e:?}"))?;

        // Code
        let mut code_label = nwg::Label::default();
        nwg::Label::builder()
            .text("Your access code")
            .position((20, 96))
            .size((420, 20))
            .font(Some(&body))
            .parent(&window)
            .build(&mut code_label)
            .map_err(|e| anyhow::anyhow!("code_label: {e:?}"))?;

        let mut code_value = nwg::Label::default();
        nwg::Label::builder()
            .text("— — — —")
            .position((20, 118))
            .size((320, 52))
            .font(Some(&mono_big))
            .parent(&window)
            .build(&mut code_value)
            .map_err(|e| anyhow::anyhow!("code_value: {e:?}"))?;

        let mut code_copy = nwg::Button::default();
        nwg::Button::builder()
            .text("Copy")
            .position((350, 128))
            .size((90, 34))
            .font(Some(&body))
            .parent(&window)
            .build(&mut code_copy)
            .map_err(|e| anyhow::anyhow!("code_copy: {e:?}"))?;

        // PIN
        let mut pin_label = nwg::Label::default();
        nwg::Label::builder()
            .text("PIN")
            .position((20, 186))
            .size((420, 20))
            .font(Some(&body))
            .parent(&window)
            .build(&mut pin_label)
            .map_err(|e| anyhow::anyhow!("pin_label: {e:?}"))?;

        let mut pin_value = nwg::Label::default();
        nwg::Label::builder()
            .text("— — — —")
            .position((20, 208))
            .size((320, 52))
            .font(Some(&mono_big))
            .parent(&window)
            .build(&mut pin_value)
            .map_err(|e| anyhow::anyhow!("pin_value: {e:?}"))?;

        let mut pin_copy = nwg::Button::default();
        nwg::Button::builder()
            .text("Copy")
            .position((350, 218))
            .size((90, 34))
            .font(Some(&body))
            .parent(&window)
            .build(&mut pin_copy)
            .map_err(|e| anyhow::anyhow!("pin_copy: {e:?}"))?;

        // Instructions
        let mut instructions = nwg::Label::default();
        nwg::Label::builder()
            .text("Share this code and PIN. Anyone can connect from\nhttps://remote.callmor.ai/connect  (no account needed).")
            .position((20, 280))
            .size((420, 50))
            .font(Some(&body))
            .parent(&window)
            .build(&mut instructions)
            .map_err(|e| anyhow::anyhow!("instructions: {e:?}"))?;

        // Buttons
        let mut install_btn = nwg::Button::default();
        nwg::Button::builder()
            .text("Install as Windows Service")
            .position((20, 360))
            .size((300, 40))
            .font(Some(&body))
            .parent(&window)
            .build(&mut install_btn)
            .map_err(|e| anyhow::anyhow!("install_btn: {e:?}"))?;

        let mut quit_btn = nwg::Button::default();
        nwg::Button::builder()
            .text("Quit")
            .position((340, 360))
            .size((100, 40))
            .font(Some(&body))
            .parent(&window)
            .build(&mut quit_btn)
            .map_err(|e| anyhow::anyhow!("quit_btn: {e:?}"))?;

        // Notice (placeholder — future cross-thread signalling)
        let mut notice = nwg::Notice::default();
        nwg::Notice::builder()
            .parent(&window)
            .build(&mut notice)
            .map_err(|e| anyhow::anyhow!("notice: {e:?}"))?;

        // Timer: 500ms refresh
        #[allow(deprecated)]
        let mut timer = nwg::Timer::default();
        #[allow(deprecated)]
        nwg::Timer::builder()
            .parent(&window)
            .interval(500) // ms
            .build(&mut timer)
            .map_err(|e| anyhow::anyhow!("timer: {e:?}"))?;

        Ok(App {
            window,
            title,
            status,
            code_label,
            code_value,
            code_copy,
            pin_label,
            pin_value,
            pin_copy,
            instructions,
            install_btn,
            quit_btn,
            notice,
            timer,
            fonts: Fonts { heading, mono_big, body },
            state,
            last_rendered: RefCell::new(None),
        })
    }

    fn refresh(app: &App) {
        let st = app.state.lock().unwrap();
        let snap = Snapshot {
            status: st.status.clone(),
            access_code: st.access_code.clone(),
            pin: st.pin.clone(),
            install_msg: st.service_install_message.clone(),
        };
        drop(st);

        let same = match &*app.last_rendered.borrow() {
            Some(prev) => prev == &snap,
            None => false,
        };
        if same {
            return;
        }

        // Status
        let status_text = match &snap.status {
            Status::Starting => "●  Starting...".to_string(),
            Status::Registering => "●  Registering with server...".to_string(),
            Status::Online => "●  Ready — share the code below".to_string(),
            Status::ViewerConnected => "●  Connected — someone is viewing".to_string(),
            Status::Error(e) => format!("●  Error: {e}"),
        };
        app.status.set_text(&status_text);

        // Code + PIN
        let code_display = if snap.access_code.is_empty() {
            "— — — —".to_string()
        } else {
            format_code(&snap.access_code)
        };
        app.code_value.set_text(&code_display);
        let pin_display = if snap.pin.is_empty() { "— — — —".to_string() } else { snap.pin.clone() };
        app.pin_value.set_text(&pin_display);

        // Install-as-service feedback
        if let Some((_ok, msg)) = &snap.install_msg {
            app.instructions.set_text(msg);
        }

        *app.last_rendered.borrow_mut() = Some(snap);

        // Silence unused-field warnings without removing the references that
        // keep the handles alive for the lifetime of the app.
        let _ = (&app.title, &app.code_label, &app.pin_label, &app.notice, &app.fonts);
    }

    fn copy_to_clipboard(text: &str) {
        if text.is_empty() {
            return;
        }
        let _ = nwg::Clipboard::set_data_text(&nwg::ControlHandle::NoHandle, text);
    }
}
