//! Self-install as a Windows service.
//!
//! Called three ways:
//!   * `launch_self_installer()` — from the portable GUI's "Install as Service"
//!     button. Re-launches the current .exe with `--install-service`, via
//!     ShellExecuteW + "runas" verb so Windows prompts for UAC elevation.
//!   * `install()` — runs (elevated) after the UAC prompt: copies the .exe
//!     to Program Files, registers a Windows service that will run it in
//!     `--service` mode, and starts it.
//!   * `uninstall()` — stops + deletes the service, removes the installed
//!     binary.
//!
//! The portable agent keeps running in the user session while the service
//! takes over persistence at boot — this matches AnyDesk's model.

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use tracing::info;

const SERVICE_NAME: &str = "CallmorAgent";
const SERVICE_DISPLAY: &str = "Callmor Remote Desktop Agent";
const INSTALL_DIR: &str = r"C:\Program Files\Callmor";
const INSTALL_EXE: &str = r"C:\Program Files\Callmor\callmor-agent.exe";

/// Non-elevated: ask Windows to re-launch us with --install-service under UAC.
pub fn launch_self_installer() -> Result<()> {
    let exe = std::env::current_exe().context("current_exe")?;

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows::core::PCWSTR;
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

        let verb: Vec<u16> = "runas\0".encode_utf16().collect();
        let file: Vec<u16> = exe.as_os_str().encode_wide().chain(Some(0)).collect();
        let params: Vec<u16> = "--install-service\0".encode_utf16().collect();

        let result = unsafe {
            ShellExecuteW(
                None,
                PCWSTR(verb.as_ptr()),
                PCWSTR(file.as_ptr()),
                PCWSTR(params.as_ptr()),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        // ShellExecuteW returns a HINSTANCE; values <= 32 indicate failure.
        if result.0 as usize <= 32 {
            bail!("ShellExecuteW failed (code {})", result.0 as usize);
        }
    }

    #[cfg(not(windows))]
    {
        let _ = exe;
        bail!("Service install only supported on Windows");
    }

    Ok(())
}

/// Elevated: actually perform the install.
pub fn install() -> Result<()> {
    info!("Installing Callmor service to {}", INSTALL_DIR);

    // 1. Copy the current exe to Program Files\Callmor\callmor-agent.exe
    let src = std::env::current_exe().context("current_exe")?;
    let dest = PathBuf::from(INSTALL_EXE);
    std::fs::create_dir_all(INSTALL_DIR).context("create install dir")?;

    // If the running .exe is already the install target (someone manually
    // ran the installed copy with --install-service), skip the copy — but
    // still re-register the service below.
    if src.canonicalize().ok() != dest.canonicalize().ok() {
        // If destination is currently held open by a running service, stop
        // it first so we can overwrite.
        let _ = run_sc(&["stop", SERVICE_NAME]);
        std::thread::sleep(std::time::Duration::from_millis(1500));
        std::fs::copy(&src, &dest).with_context(|| format!("copy {src:?} -> {dest:?}"))?;
    }

    // 2. Write a starter ADHOC config so the service also shows up as an
    //    ad-hoc machine once running. The portable user already has their
    //    own config under LOCALAPPDATA; the service gets a fresh one under
    //    ProgramData so the two don't share credentials.
    let cfg_dir = Path::new(r"C:\ProgramData\Callmor");
    std::fs::create_dir_all(cfg_dir).context("create ProgramData\\Callmor")?;
    let cfg_path = cfg_dir.join("agent.conf");
    if !cfg_path.exists() {
        let contents = "# Callmor Remote Desktop Agent — Service mode\n\
                        # On first run the agent self-registers; do not hand-edit.\n\n\
                        RELAY_URL=wss://relay.callmor.ai\n\
                        API_URL=https://api.callmor.ai\n\
                        ADHOC=1\n";
        std::fs::write(&cfg_path, contents).context("write service agent.conf")?;
    }

    // 3. Register the Windows service.
    let _ = run_sc(&["stop", SERVICE_NAME]);
    std::thread::sleep(std::time::Duration::from_millis(500));
    let _ = run_sc(&["delete", SERVICE_NAME]);
    std::thread::sleep(std::time::Duration::from_millis(500));

    let bin_path_arg = format!("binPath= \"\\\"{INSTALL_EXE}\\\" --service\"");
    run_sc(&[
        "create",
        SERVICE_NAME,
        &bin_path_arg,
        "start=",
        "auto",
        "DisplayName=",
        SERVICE_DISPLAY,
    ])?;
    run_sc(&["description", SERVICE_NAME, "Enables remote access via Callmor."])?;
    run_sc(&["start", SERVICE_NAME])?;

    info!("Callmor service installed and started.");
    show_info_dialog("Callmor service installed and started. It will run automatically at boot.");
    Ok(())
}

/// Stop + delete the service, remove the installed binary.
pub fn uninstall() -> Result<()> {
    info!("Uninstalling Callmor service");
    let _ = run_sc(&["stop", SERVICE_NAME]);
    std::thread::sleep(std::time::Duration::from_millis(1000));
    let _ = run_sc(&["delete", SERVICE_NAME]);

    let installed = Path::new(INSTALL_EXE);
    if installed.exists() {
        let _ = std::fs::remove_file(installed);
    }

    show_info_dialog("Callmor service removed.");
    Ok(())
}

fn run_sc(args: &[&str]) -> Result<()> {
    let output = std::process::Command::new("sc")
        .args(args)
        .output()
        .with_context(|| format!("spawn sc {args:?}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        bail!(
            "sc {args:?} failed ({}): {}{}",
            output.status,
            stderr,
            stdout
        );
    }
    Ok(())
}

fn show_info_dialog(message: &str) {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows::core::PCWSTR;
        use windows::Win32::UI::WindowsAndMessaging::{
            MessageBoxW, MB_ICONINFORMATION, MB_OK,
        };
        let title: Vec<u16> = "Callmor\0".encode_utf16().collect();
        let body: Vec<u16> = std::ffi::OsStr::new(message)
            .encode_wide()
            .chain(Some(0))
            .collect();
        unsafe {
            MessageBoxW(
                None,
                PCWSTR(body.as_ptr()),
                PCWSTR(title.as_ptr()),
                MB_OK | MB_ICONINFORMATION,
            );
        }
    }
    #[cfg(not(windows))]
    {
        let _ = message;
    }
}
