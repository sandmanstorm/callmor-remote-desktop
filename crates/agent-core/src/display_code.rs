//! Displays the ad-hoc access code + PIN to the user on the local machine.
//!
//! Strategy per OS:
//!   - ALL: write `<home>/Desktop/Callmor-Code.txt` so the user can always
//!     find it, even if they dismissed the dialog. Also write to a predictable
//!     path (`/tmp/callmor-code.txt` / `%PROGRAMDATA%\Callmor\code.txt`) as a
//!     fallback for service-mode installs with no interactive desktop.
//!   - Windows: pop a MessageBox (blocks until dismissed — that's fine, it
//!     runs in a spawned thread so the session loop keeps going).
//!   - macOS: `osascript -e 'display dialog ...'` shows a native AppleScript
//!     dialog. No build deps needed.
//!   - Linux: `notify-send` if available (common on GNOME/KDE/Cinnamon);
//!     otherwise just the file is enough.
//!
//! This is intentionally low-fidelity — the Desktop file is the source of
//! truth. A richer tray UI can come later.

use std::path::PathBuf;

pub fn show(access_code: &str, pin: &str) {
    let message = format!(
        "Callmor Remote Desktop\n\n\
         Share this code and PIN with the person who will connect to this computer:\n\n\
         Code: {}\n\
         PIN:  {}\n\n\
         Go to https://remote.callmor.ai/connect and enter both.",
        format_code(access_code),
        pin,
    );

    // Always try writing the file first — if the dialog call fails (no display,
    // headless service, etc.) the user still has somewhere to read the code.
    write_code_file(&message);

    // On Windows a service runs in Session 0 with no user desktop, so a
    // MessageBox there is invisible and just wastes a thread. Only show the
    // dialog when we clearly have an interactive session.
    #[cfg(windows)]
    if is_interactive_session() {
        show_windows_dialog(&message);
    }

    #[cfg(target_os = "macos")]
    show_macos_dialog(&message);

    #[cfg(all(unix, not(target_os = "macos")))]
    show_linux_notification(access_code, pin);
}

/// Best-effort: detect whether we're running in a user session (vs Session 0
/// as a service). Services under LocalSystem have USERPROFILE pointing at
/// `C:\Windows\system32\config\systemprofile`, which is a reliable signal.
#[cfg(windows)]
fn is_interactive_session() -> bool {
    match std::env::var("USERPROFILE") {
        Ok(p) => {
            let lower = p.to_ascii_lowercase();
            !lower.contains(r"system32\config\systemprofile")
                && !lower.contains(r"system32/config/systemprofile")
        }
        Err(_) => false,
    }
}

/// "ABCD1234" -> "ABCD-1234" for easier reading-aloud.
fn format_code(code: &str) -> String {
    if code.len() == 8 && code.chars().all(|c| c.is_ascii_alphanumeric()) {
        format!("{}-{}", &code[..4], &code[4..])
    } else {
        code.to_string()
    }
}

fn write_code_file(content: &str) {
    // Target list: write to every plausible location so at least one is
    // visible regardless of whether we're running as a service or a user
    // process. A failed write on any one path is silent and non-fatal.
    let paths = code_file_targets();
    for path in &paths {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, content);
    }
}

fn code_file_targets() -> Vec<PathBuf> {
    let mut out = Vec::new();

    #[cfg(windows)]
    {
        // Public Desktop — merges into every user's actual visible desktop.
        // This works from a Session 0 service since we write to C:\Users\Public.
        if let Ok(p) = std::env::var("PUBLIC") {
            out.push(PathBuf::from(p).join("Desktop").join("Callmor-Code.txt"));
        } else {
            out.push(PathBuf::from(r"C:\Users\Public\Desktop\Callmor-Code.txt"));
        }
        // Current-user Desktop — only meaningful when run interactively. When
        // the service runs as SYSTEM this path would hit
        // system32\config\systemprofile, which is useless to the user; skip
        // it there.
        if let Ok(p) = std::env::var("USERPROFILE") {
            let lower = p.to_ascii_lowercase();
            if !lower.contains(r"system32\config\systemprofile") {
                out.push(PathBuf::from(p).join("Desktop").join("Callmor-Code.txt"));
            }
        }
        // Stable machine-readable copy for a UI helper to pick up later.
        out.push(PathBuf::from(r"C:\ProgramData\Callmor\code.txt"));
    }
    #[cfg(unix)]
    {
        if let Ok(home) = std::env::var("HOME") {
            out.push(PathBuf::from(home).join("Desktop").join("Callmor-Code.txt"));
        }
        out.push(PathBuf::from("/tmp/callmor-code.txt"));
    }

    out
}

#[cfg(windows)]
fn show_windows_dialog(message: &str) {
    // Spawn in a thread so the blocking MessageBox doesn't hold up main.
    let msg = message.to_string();
    std::thread::spawn(move || {
        use std::os::windows::ffi::OsStrExt;
        use std::ffi::OsStr;
        // Inline MessageBoxW FFI to avoid dragging in another crate.
        #[link(name = "user32")]
        unsafe extern "system" {
            fn MessageBoxW(hwnd: *mut core::ffi::c_void, text: *const u16, caption: *const u16, typ: u32) -> i32;
        }
        const MB_OK: u32 = 0;
        const MB_ICONINFORMATION: u32 = 0x40;
        const MB_SETFOREGROUND: u32 = 0x10000;
        const MB_TOPMOST: u32 = 0x40000;
        const MB_SERVICE_NOTIFICATION: u32 = 0x00200000;

        let wide: Vec<u16> = OsStr::new(&msg).encode_wide().chain(Some(0)).collect();
        let title: Vec<u16> = OsStr::new("Callmor Remote Desktop").encode_wide().chain(Some(0)).collect();
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                wide.as_ptr(),
                title.as_ptr(),
                MB_OK | MB_ICONINFORMATION | MB_SETFOREGROUND | MB_TOPMOST | MB_SERVICE_NOTIFICATION,
            );
        }
    });
}

#[cfg(target_os = "macos")]
fn show_macos_dialog(message: &str) {
    // Run in a thread so the blocking osascript call doesn't hold up main.
    let msg = message.to_string();
    std::thread::spawn(move || {
        let script = format!(
            "display dialog \"{}\" with title \"Callmor Remote Desktop\" buttons {{\"OK\"}} default button \"OK\" with icon note",
            msg.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n")
        );
        let _ = std::process::Command::new("osascript")
            .arg("-e")
            .arg(script)
            .status();
    });
}

#[cfg(all(unix, not(target_os = "macos")))]
fn show_linux_notification(access_code: &str, pin: &str) {
    // Best-effort: notify-send, libnotify is on most desktop Linuxes.
    let code = format!("{} / PIN {}", format_code(access_code), pin);
    let _ = std::process::Command::new("notify-send")
        .arg("-u")
        .arg("critical")
        .arg("-t")
        .arg("0")
        .arg("Callmor Remote Desktop")
        .arg(format!("Share this code to let someone connect:\n\n{code}\n\nGo to https://remote.callmor.ai/connect"))
        .status();
}
