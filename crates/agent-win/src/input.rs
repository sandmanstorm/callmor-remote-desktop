//! Windows input injection using SendInput.
//!
//! Maps JS KeyboardEvent.code strings to Virtual Keys, and JS MouseEvent.button
//! indices to MOUSEEVENTF_* flags.

use callmor_agent_core::input::InputEvent;

#[cfg(windows)]
pub use windows_impl::*;

#[cfg(not(windows))]
pub use stub_impl::*;

#[cfg(windows)]
mod windows_impl {
    use super::*;
    use windows::Win32::UI::Input::KeyboardAndMouse::*;
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

    pub struct Injector {
        pub screen_width: i32,
        pub screen_height: i32,
    }

    impl Injector {
        pub fn new() -> Self {
            unsafe {
                let screen_width = GetSystemMetrics(SM_CXSCREEN).max(1);
                let screen_height = GetSystemMetrics(SM_CYSCREEN).max(1);
                Injector { screen_width, screen_height }
            }
        }

        pub fn screen_size(&self) -> (u32, u32) {
            (self.screen_width as u32, self.screen_height as u32)
        }

        pub fn handle(&self, event: &InputEvent) {
            match event {
                InputEvent::MouseMove { x, y } => self.mouse_move(*x, *y),
                InputEvent::MouseDown { x, y, button } => {
                    self.mouse_move(*x, *y);
                    self.mouse_button(*button, true);
                }
                InputEvent::MouseUp { x, y, button } => {
                    self.mouse_move(*x, *y);
                    self.mouse_button(*button, false);
                }
                InputEvent::Scroll { delta_y, .. } => {
                    self.scroll(*delta_y);
                }
                InputEvent::KeyDown { code } => {
                    if let Some(vk) = map_code(code) {
                        self.key(vk, true);
                    }
                }
                InputEvent::KeyUp { code } => {
                    if let Some(vk) = map_code(code) {
                        self.key(vk, false);
                    }
                }
            }
        }

        fn mouse_move(&self, x: i32, y: i32) {
            // SendInput absolute coordinates: 0..65535 across primary monitor
            let abs_x = (x as i64 * 65535 / self.screen_width as i64) as i32;
            let abs_y = (y as i64 * 65535 / self.screen_height as i64) as i32;
            let input = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dx: abs_x,
                        dy: abs_y,
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };
            send(&[input]);
        }

        fn mouse_button(&self, button: u8, down: bool) {
            let flags = match (button, down) {
                (0, true) => MOUSEEVENTF_LEFTDOWN,
                (0, false) => MOUSEEVENTF_LEFTUP,
                (1, true) => MOUSEEVENTF_MIDDLEDOWN,
                (1, false) => MOUSEEVENTF_MIDDLEUP,
                (2, true) => MOUSEEVENTF_RIGHTDOWN,
                (2, false) => MOUSEEVENTF_RIGHTUP,
                _ => return,
            };
            let input = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dx: 0,
                        dy: 0,
                        mouseData: 0,
                        dwFlags: flags,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };
            send(&[input]);
        }

        fn scroll(&self, delta_y: f64) {
            // Negative delta_y in JS = scroll up; Windows WHEEL_DELTA = +120 for up
            let wheel_delta = (-delta_y / 100.0 * 120.0).clamp(-1200.0, 1200.0) as i32;
            let input = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dx: 0,
                        dy: 0,
                        mouseData: wheel_delta as u32,
                        dwFlags: MOUSEEVENTF_WHEEL,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };
            send(&[input]);
        }

        fn key(&self, vk: VIRTUAL_KEY, down: bool) {
            let flags = if down { KEYBD_EVENT_FLAGS(0) } else { KEYEVENTF_KEYUP };
            let input = INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: vk,
                        wScan: 0,
                        dwFlags: flags,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };
            send(&[input]);
        }
    }

    fn send(inputs: &[INPUT]) {
        unsafe {
            SendInput(inputs, std::mem::size_of::<INPUT>() as i32);
        }
    }

    /// Map JS KeyboardEvent.code to Windows Virtual Key.
    fn map_code(code: &str) -> Option<VIRTUAL_KEY> {
        Some(match code {
            // Letters
            "KeyA" => VK_A, "KeyB" => VK_B, "KeyC" => VK_C, "KeyD" => VK_D,
            "KeyE" => VK_E, "KeyF" => VK_F, "KeyG" => VK_G, "KeyH" => VK_H,
            "KeyI" => VK_I, "KeyJ" => VK_J, "KeyK" => VK_K, "KeyL" => VK_L,
            "KeyM" => VK_M, "KeyN" => VK_N, "KeyO" => VK_O, "KeyP" => VK_P,
            "KeyQ" => VK_Q, "KeyR" => VK_R, "KeyS" => VK_S, "KeyT" => VK_T,
            "KeyU" => VK_U, "KeyV" => VK_V, "KeyW" => VK_W, "KeyX" => VK_X,
            "KeyY" => VK_Y, "KeyZ" => VK_Z,

            // Digits
            "Digit0" => VK_0, "Digit1" => VK_1, "Digit2" => VK_2, "Digit3" => VK_3,
            "Digit4" => VK_4, "Digit5" => VK_5, "Digit6" => VK_6, "Digit7" => VK_7,
            "Digit8" => VK_8, "Digit9" => VK_9,

            // Function keys
            "F1" => VK_F1, "F2" => VK_F2, "F3" => VK_F3, "F4" => VK_F4,
            "F5" => VK_F5, "F6" => VK_F6, "F7" => VK_F7, "F8" => VK_F8,
            "F9" => VK_F9, "F10" => VK_F10, "F11" => VK_F11, "F12" => VK_F12,

            // Modifiers
            "ShiftLeft" => VK_LSHIFT, "ShiftRight" => VK_RSHIFT,
            "ControlLeft" => VK_LCONTROL, "ControlRight" => VK_RCONTROL,
            "AltLeft" => VK_LMENU, "AltRight" => VK_RMENU,
            "MetaLeft" => VK_LWIN, "MetaRight" => VK_RWIN,
            "CapsLock" => VK_CAPITAL,

            // Special
            "Escape" => VK_ESCAPE,
            "Backspace" => VK_BACK,
            "Tab" => VK_TAB,
            "Enter" => VK_RETURN,
            "Space" => VK_SPACE,
            "Delete" => VK_DELETE,
            "Insert" => VK_INSERT,
            "Home" => VK_HOME,
            "End" => VK_END,
            "PageUp" => VK_PRIOR,
            "PageDown" => VK_NEXT,

            "ArrowUp" => VK_UP,
            "ArrowDown" => VK_DOWN,
            "ArrowLeft" => VK_LEFT,
            "ArrowRight" => VK_RIGHT,

            "Minus" => VK_OEM_MINUS,
            "Equal" => VK_OEM_PLUS,
            "BracketLeft" => VK_OEM_4,
            "BracketRight" => VK_OEM_6,
            "Backslash" => VK_OEM_5,
            "Semicolon" => VK_OEM_1,
            "Quote" => VK_OEM_7,
            "Backquote" => VK_OEM_3,
            "Comma" => VK_OEM_COMMA,
            "Period" => VK_OEM_PERIOD,
            "Slash" => VK_OEM_2,

            _ => return None,
        })
    }
}

#[cfg(not(windows))]
mod stub_impl {
    use super::*;

    pub struct Injector;

    impl Injector {
        pub fn new() -> Self { Injector }
        pub fn screen_size(&self) -> (u32, u32) { (1920, 1080) }
        pub fn handle(&self, _event: &InputEvent) {}
    }
}
