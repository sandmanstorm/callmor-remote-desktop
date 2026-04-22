use serde::Deserialize;

/// Input event received from the browser over the data channel.
/// Uses JS conventions: button 0 = left, 1 = middle, 2 = right.
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
    Scroll {
        #[allow(dead_code)]
        x: i32,
        #[allow(dead_code)]
        y: i32,
        #[serde(rename = "deltaY")]
        delta_y: f64,
    },

    #[serde(rename = "keydown")]
    KeyDown { code: String },

    #[serde(rename = "keyup")]
    KeyUp { code: String },
}
