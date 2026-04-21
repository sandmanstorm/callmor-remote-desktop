use anyhow::{Context, Result};
use serde::Deserialize;
use tracing::debug;
use x11rb::connection::Connection;
use x11rb::protocol::xproto;
use x11rb::protocol::xtest::ConnectionExt as XTestExt;

/// Input event received from the browser over the data channel.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum InputEvent {
    #[serde(rename = "mousemove")]
    MouseMove { x: i32, y: i32 },

    #[serde(rename = "mousedown")]
    MouseDown { x: i32, y: i32, button: u8 },

    #[serde(rename = "mouseup")]
    MouseUp { x: i32, y: i32, button: u8 },

    #[serde(rename = "scroll")]
    #[allow(dead_code)]
    Scroll { x: i32, y: i32, #[serde(rename = "deltaY")] delta_y: f64 },

    #[serde(rename = "keydown")]
    KeyDown { code: String },

    #[serde(rename = "keyup")]
    KeyUp { code: String },
}

/// Injects mouse and keyboard events into an X11 display via XTest.
pub struct InputInjector {
    conn: x11rb::rust_connection::RustConnection,
    root: u32,
    screen_width: u16,
    screen_height: u16,
}

impl InputInjector {
    pub fn new() -> Result<Self> {
        let (conn, screen_num) = x11rb::connect(None)
            .context("Failed to connect to X11 display")?;
        let screen = &conn.setup().roots[screen_num];
        let root = screen.root;
        let screen_width = screen.width_in_pixels;
        let screen_height = screen.height_in_pixels;

        tracing::info!("InputInjector connected to X11 display ({}x{})", screen_width, screen_height);
        Ok(Self { conn, root, screen_width, screen_height })
    }

    pub fn screen_size(&self) -> (u16, u16) {
        (self.screen_width, self.screen_height)
    }

    pub fn handle_event(&self, event: &InputEvent) -> Result<()> {
        match event {
            InputEvent::MouseMove { x, y } => {
                self.conn.xtest_fake_input(
                    xproto::MOTION_NOTIFY_EVENT,
                    0, // detail (unused for motion)
                    x11rb::CURRENT_TIME,
                    self.root,
                    *x as i16,
                    *y as i16,
                    0,
                )?;
                self.conn.flush()?;
            }

            InputEvent::MouseDown { x, y, button } => {
                let x_button = js_button_to_x11(*button);
                // Move first, then press
                self.conn.xtest_fake_input(
                    xproto::MOTION_NOTIFY_EVENT, 0, x11rb::CURRENT_TIME,
                    self.root, *x as i16, *y as i16, 0,
                )?;
                self.conn.xtest_fake_input(
                    xproto::BUTTON_PRESS_EVENT, x_button, x11rb::CURRENT_TIME,
                    self.root, 0, 0, 0,
                )?;
                self.conn.flush()?;
            }

            InputEvent::MouseUp { x, y, button } => {
                let x_button = js_button_to_x11(*button);
                self.conn.xtest_fake_input(
                    xproto::MOTION_NOTIFY_EVENT, 0, x11rb::CURRENT_TIME,
                    self.root, *x as i16, *y as i16, 0,
                )?;
                self.conn.xtest_fake_input(
                    xproto::BUTTON_RELEASE_EVENT, x_button, x11rb::CURRENT_TIME,
                    self.root, 0, 0, 0,
                )?;
                self.conn.flush()?;
            }

            InputEvent::Scroll { delta_y, .. } => {
                // X11: button 4 = scroll up, button 5 = scroll down
                let button = if *delta_y < 0.0 { 4u8 } else { 5u8 };
                // Send multiple scroll events for larger deltas
                let clicks = (delta_y.abs() / 53.0).max(1.0).min(10.0) as u32;
                for _ in 0..clicks {
                    self.conn.xtest_fake_input(
                        xproto::BUTTON_PRESS_EVENT, button, x11rb::CURRENT_TIME,
                        self.root, 0, 0, 0,
                    )?;
                    self.conn.xtest_fake_input(
                        xproto::BUTTON_RELEASE_EVENT, button, x11rb::CURRENT_TIME,
                        self.root, 0, 0, 0,
                    )?;
                }
                self.conn.flush()?;
            }

            InputEvent::KeyDown { code } => {
                if let Some(keycode) = js_code_to_x11_keycode(code) {
                    self.conn.xtest_fake_input(
                        xproto::KEY_PRESS_EVENT, keycode, x11rb::CURRENT_TIME,
                        self.root, 0, 0, 0,
                    )?;
                    self.conn.flush()?;
                } else {
                    debug!("Unmapped key code: {code}");
                }
            }

            InputEvent::KeyUp { code } => {
                if let Some(keycode) = js_code_to_x11_keycode(code) {
                    self.conn.xtest_fake_input(
                        xproto::KEY_RELEASE_EVENT, keycode, x11rb::CURRENT_TIME,
                        self.root, 0, 0, 0,
                    )?;
                    self.conn.flush()?;
                }
            }
        }
        Ok(())
    }
}

/// Convert JS MouseEvent.button (0=left, 1=middle, 2=right) to X11 button (1=left, 2=middle, 3=right)
fn js_button_to_x11(js_button: u8) -> u8 {
    match js_button {
        0 => 1, // left
        1 => 2, // middle
        2 => 3, // right
        n => n + 1,
    }
}

/// Map JavaScript KeyboardEvent.code to X11 keycode.
/// These are standard US keyboard keycodes for Xvfb/Xorg.
fn js_code_to_x11_keycode(code: &str) -> Option<u8> {
    Some(match code {
        // Letters
        "KeyA" => 38, "KeyB" => 56, "KeyC" => 54, "KeyD" => 40,
        "KeyE" => 26, "KeyF" => 41, "KeyG" => 42, "KeyH" => 43,
        "KeyI" => 31, "KeyJ" => 44, "KeyK" => 45, "KeyL" => 46,
        "KeyM" => 58, "KeyN" => 57, "KeyO" => 32, "KeyP" => 33,
        "KeyQ" => 24, "KeyR" => 27, "KeyS" => 39, "KeyT" => 28,
        "KeyU" => 30, "KeyV" => 55, "KeyW" => 25, "KeyX" => 53,
        "KeyY" => 29, "KeyZ" => 52,

        // Numbers
        "Digit0" => 19, "Digit1" => 10, "Digit2" => 11, "Digit3" => 12,
        "Digit4" => 13, "Digit5" => 14, "Digit6" => 15, "Digit7" => 16,
        "Digit8" => 17, "Digit9" => 18,

        // Function keys
        "F1" => 67, "F2" => 68, "F3" => 69, "F4" => 70,
        "F5" => 71, "F6" => 72, "F7" => 73, "F8" => 74,
        "F9" => 75, "F10" => 76, "F11" => 95, "F12" => 96,

        // Modifiers
        "ShiftLeft" => 50, "ShiftRight" => 62,
        "ControlLeft" => 37, "ControlRight" => 105,
        "AltLeft" => 64, "AltRight" => 108,
        "MetaLeft" => 133, "MetaRight" => 134,
        "CapsLock" => 66,

        // Special keys
        "Escape" => 9,
        "Backspace" => 22,
        "Tab" => 23,
        "Enter" => 36,
        "Space" => 65,
        "Delete" => 119,
        "Insert" => 118,
        "Home" => 110,
        "End" => 115,
        "PageUp" => 112,
        "PageDown" => 117,

        // Arrow keys
        "ArrowUp" => 111,
        "ArrowDown" => 116,
        "ArrowLeft" => 113,
        "ArrowRight" => 114,

        // Punctuation
        "Minus" => 20,
        "Equal" => 21,
        "BracketLeft" => 34,
        "BracketRight" => 35,
        "Backslash" => 51,
        "Semicolon" => 47,
        "Quote" => 48,
        "Backquote" => 49,
        "Comma" => 59,
        "Period" => 60,
        "Slash" => 61,

        _ => return None,
    })
}
