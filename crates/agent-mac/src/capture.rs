//! macOS screen capture via ScreenCaptureKit (through the `scap` crate).

use anyhow::{bail, Context, Result};
use bytes::Bytes;

pub struct Frame {
    pub width: u32,
    pub height: u32,
    /// BGRA pixels, tightly packed (width*4 bytes per row).
    pub data: Bytes,
}

#[cfg(target_os = "macos")]
pub use macos_impl::*;

#[cfg(not(target_os = "macos"))]
pub use stub_impl::*;

#[cfg(target_os = "macos")]
mod macos_impl {
    use super::*;
    use scap::capturer::{Area, Capturer as ScapCapturer, Options, Point, Size};
    use scap::frame::{Frame as ScapFrame, FrameType};

    pub struct Capturer {
        inner: ScapCapturer,
        pub width: u32,
        pub height: u32,
    }

    impl Capturer {
        pub fn new() -> Result<Self> {
            if !scap::is_supported() {
                bail!("Screen capture not supported on this macOS version");
            }
            if !scap::has_permission() {
                // Will open System Settings prompt. The agent can re-run after user grants.
                if !scap::request_permission() {
                    bail!("Screen Recording permission denied. Enable in System Settings → Privacy & Security → Screen Recording.");
                }
            }

            let options = Options {
                fps: 30,
                target: None,
                show_cursor: true,
                show_highlight: true,
                excluded_targets: None,
                output_type: FrameType::BGRAFrame,
                output_resolution: scap::capturer::Resolution::Captured,
                crop_area: None,
                ..Default::default()
            };

            let mut inner = ScapCapturer::build(options).context("scap build capturer")?;
            inner.start_capture();

            // Capture one frame to learn the resolution
            let first = inner.get_next_frame().context("first frame")?;
            let (width, height) = match &first {
                ScapFrame::BGRA(f) => (f.width as u32, f.height as u32),
                _ => bail!("Expected BGRA frame"),
            };

            Ok(Capturer { inner, width, height })
        }

        pub fn grab(&mut self, _timeout_ms: u32) -> Result<Option<Frame>> {
            match self.inner.get_next_frame() {
                Ok(ScapFrame::BGRA(f)) => Ok(Some(Frame {
                    width: f.width as u32,
                    height: f.height as u32,
                    data: Bytes::from(f.data),
                })),
                Ok(_) => Ok(None),
                Err(e) => bail!("scap grab: {e:?}"),
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod stub_impl {
    use super::*;

    pub struct Capturer {
        pub width: u32,
        pub height: u32,
    }

    impl Capturer {
        pub fn new() -> Result<Self> {
            bail!("macOS agent does not run on this platform")
        }
        pub fn grab(&mut self, _timeout_ms: u32) -> Result<Option<Frame>> {
            bail!("macOS agent does not run on this platform")
        }
    }
}
