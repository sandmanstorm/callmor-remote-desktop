//! macOS input injection via Quartz Event Services (CGEventPost).

use callmor_agent_core::input::InputEvent;

#[cfg(target_os = "macos")]
pub use macos_impl::*;

#[cfg(not(target_os = "macos"))]
pub use stub_impl::*;

#[cfg(target_os = "macos")]
mod macos_impl {
    use super::*;
    use core_graphics::display::CGDisplay;
    use core_graphics::event::{
        CGEvent, CGEventTapLocation, CGEventType, CGKeyCode, CGMouseButton, ScrollEventUnit,
    };
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    use core_graphics::geometry::CGPoint;

    pub struct Injector {
        pub screen_width: u32,
        pub screen_height: u32,
    }

    impl Injector {
        pub fn new() -> Self {
            let main = CGDisplay::main();
            Injector {
                screen_width: main.pixels_wide() as u32,
                screen_height: main.pixels_high() as u32,
            }
        }

        pub fn screen_size(&self) -> (u32, u32) {
            (self.screen_width, self.screen_height)
        }

        pub fn handle(&self, event: &InputEvent) {
            let source = match CGEventSource::new(CGEventSourceStateID::HIDSystemState) {
                Ok(s) => s,
                Err(_) => return,
            };

            match event {
                InputEvent::MouseMove { x, y } => {
                    post_mouse(
                        &source,
                        CGEventType::MouseMoved,
                        CGPoint::new(*x as f64, *y as f64),
                        CGMouseButton::Left,
                    );
                }
                InputEvent::MouseDown { x, y, button } => {
                    let (evt, btn) = mouse_type(*button, true);
                    post_mouse(&source, evt, CGPoint::new(*x as f64, *y as f64), btn);
                }
                InputEvent::MouseUp { x, y, button } => {
                    let (evt, btn) = mouse_type(*button, false);
                    post_mouse(&source, evt, CGPoint::new(*x as f64, *y as f64), btn);
                }
                InputEvent::Scroll { delta_y, .. } => {
                    let lines = (-delta_y / 40.0).clamp(-10.0, 10.0) as i32;
                    if let Ok(e) = CGEvent::new_scroll_event(
                        source,
                        ScrollEventUnit::LINE,
                        2,
                        lines,
                        0,
                        0,
                    ) {
                        e.post(CGEventTapLocation::HID);
                    }
                }
                InputEvent::KeyDown { code } => {
                    if let Some(kc) = map_code(code) {
                        if let Ok(e) = CGEvent::new_keyboard_event(source, kc, true) {
                            e.post(CGEventTapLocation::HID);
                        }
                    }
                }
                InputEvent::KeyUp { code } => {
                    if let Some(kc) = map_code(code) {
                        if let Ok(e) = CGEvent::new_keyboard_event(source, kc, false) {
                            e.post(CGEventTapLocation::HID);
                        }
                    }
                }
            }
        }
    }

    fn mouse_type(js_button: u8, down: bool) -> (CGEventType, CGMouseButton) {
        match (js_button, down) {
            (0, true) => (CGEventType::LeftMouseDown, CGMouseButton::Left),
            (0, false) => (CGEventType::LeftMouseUp, CGMouseButton::Left),
            (1, true) => (CGEventType::OtherMouseDown, CGMouseButton::Center),
            (1, false) => (CGEventType::OtherMouseUp, CGMouseButton::Center),
            (2, true) => (CGEventType::RightMouseDown, CGMouseButton::Right),
            (2, false) => (CGEventType::RightMouseUp, CGMouseButton::Right),
            _ => (CGEventType::LeftMouseDown, CGMouseButton::Left),
        }
    }

    fn post_mouse(
        source: &CGEventSource,
        event_type: CGEventType,
        point: CGPoint,
        button: CGMouseButton,
    ) {
        if let Ok(e) = CGEvent::new_mouse_event(source.clone(), event_type, point, button) {
            e.post(CGEventTapLocation::HID);
        }
    }

    /// Map JS KeyboardEvent.code → macOS CGKeyCode.
    fn map_code(code: &str) -> Option<CGKeyCode> {
        Some(match code {
            // Letters (ANSI layout keycodes)
            "KeyA" => 0, "KeyS" => 1, "KeyD" => 2, "KeyF" => 3,
            "KeyH" => 4, "KeyG" => 5, "KeyZ" => 6, "KeyX" => 7,
            "KeyC" => 8, "KeyV" => 9, "KeyB" => 11, "KeyQ" => 12,
            "KeyW" => 13, "KeyE" => 14, "KeyR" => 15, "KeyY" => 16,
            "KeyT" => 17, "KeyO" => 31, "KeyU" => 32, "KeyI" => 34,
            "KeyP" => 35, "KeyL" => 37, "KeyJ" => 38, "KeyK" => 40,
            "KeyN" => 45, "KeyM" => 46,

            // Digits
            "Digit0" => 29, "Digit1" => 18, "Digit2" => 19, "Digit3" => 20,
            "Digit4" => 21, "Digit5" => 23, "Digit6" => 22, "Digit7" => 26,
            "Digit8" => 28, "Digit9" => 25,

            // Function
            "F1" => 122, "F2" => 120, "F3" => 99,  "F4" => 118,
            "F5" => 96,  "F6" => 97,  "F7" => 98,  "F8" => 100,
            "F9" => 101, "F10" => 109, "F11" => 103, "F12" => 111,

            // Modifiers
            "ShiftLeft" => 56, "ShiftRight" => 60,
            "ControlLeft" => 59, "ControlRight" => 62,
            "AltLeft" => 58, "AltRight" => 61,
            "MetaLeft" => 55, "MetaRight" => 54,
            "CapsLock" => 57,

            "Escape" => 53,
            "Backspace" => 51,
            "Tab" => 48,
            "Enter" => 36,
            "Space" => 49,
            "Delete" => 117,
            "Home" => 115,
            "End" => 119,
            "PageUp" => 116,
            "PageDown" => 121,

            "ArrowUp" => 126,
            "ArrowDown" => 125,
            "ArrowLeft" => 123,
            "ArrowRight" => 124,

            "Minus" => 27,
            "Equal" => 24,
            "BracketLeft" => 33,
            "BracketRight" => 30,
            "Backslash" => 42,
            "Semicolon" => 41,
            "Quote" => 39,
            "Backquote" => 50,
            "Comma" => 43,
            "Period" => 47,
            "Slash" => 44,

            _ => return None,
        })
    }
}

#[cfg(not(target_os = "macos"))]
mod stub_impl {
    use super::*;

    pub struct Injector;

    impl Injector {
        pub fn new() -> Self { Injector }
        pub fn screen_size(&self) -> (u32, u32) { (1920, 1080) }
        pub fn handle(&self, _event: &InputEvent) {}
    }
}
