//! Desktop Duplication screen capture (Windows DXGI).
//!
//! Captures frames from the primary monitor as BGRA pixel buffers. Delivers
//! them via an mpsc channel to the encoder.

use anyhow::{bail, Context, Result};
use bytes::Bytes;

pub struct Frame {
    pub width: u32,
    pub height: u32,
    /// BGRA pixels (4 bytes per pixel), tightly packed.
    pub data: Bytes,
}

#[cfg(windows)]
pub use windows_impl::*;

#[cfg(not(windows))]
pub use stub_impl::*;

// =========================================================================
// Windows implementation: DXGI Desktop Duplication
// =========================================================================
#[cfg(windows)]
mod windows_impl {
    use super::*;
    use windows::core::Interface;
    use windows::Win32::Graphics::Direct3D::{
        D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_10_0,
    };
    use windows::Win32::Graphics::Direct3D11::*;
    use windows::Win32::Graphics::Dxgi::*;
    use windows::Win32::Graphics::Dxgi::Common::*;

    pub struct Capturer {
        device: ID3D11Device,
        context: ID3D11DeviceContext,
        duplication: IDXGIOutputDuplication,
        staging: Option<ID3D11Texture2D>,
        pub width: u32,
        pub height: u32,
    }

    impl Capturer {
        pub fn new() -> Result<Self> {
            unsafe {
                // Create D3D11 device
                let mut device: Option<ID3D11Device> = None;
                let mut context: Option<ID3D11DeviceContext> = None;
                let feature_levels = [D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_10_0];
                let mut out_feature_level = Default::default();

                D3D11CreateDevice(
                    None,
                    D3D_DRIVER_TYPE_HARDWARE,
                    Default::default(),
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                    Some(&feature_levels),
                    D3D11_SDK_VERSION,
                    Some(&mut device),
                    Some(&mut out_feature_level),
                    Some(&mut context),
                )
                .context("D3D11CreateDevice failed")?;

                let device = device.context("D3D11 device is None")?;
                let context = context.context("D3D11 context is None")?;

                // Get DXGI adapter and output
                let dxgi_device: IDXGIDevice = device.cast().context("Device -> DXGIDevice")?;
                let adapter: IDXGIAdapter = dxgi_device.GetAdapter().context("GetAdapter")?;
                let output: IDXGIOutput = adapter.EnumOutputs(0).context("EnumOutputs")?;
                let output1: IDXGIOutput1 = output.cast().context("IDXGIOutput1 cast")?;

                let desc = output.GetDesc().context("GetDesc")?;
                let width = (desc.DesktopCoordinates.right - desc.DesktopCoordinates.left) as u32;
                let height = (desc.DesktopCoordinates.bottom - desc.DesktopCoordinates.top) as u32;

                let duplication = output1.DuplicateOutput(&device).context("DuplicateOutput")?;

                Ok(Capturer { device, context, duplication, staging: None, width, height })
            }
        }

        /// Capture one frame. Returns None if no new frame available within the timeout.
        pub fn grab(&mut self, timeout_ms: u32) -> Result<Option<Frame>> {
            unsafe {
                let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
                let mut resource: Option<IDXGIResource> = None;

                let result = self
                    .duplication
                    .AcquireNextFrame(timeout_ms, &mut info as *mut _, &mut resource as *mut _);
                match result {
                    Ok(()) => {}
                    Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => return Ok(None),
                    Err(e) => bail!("AcquireNextFrame: {e}"),
                }

                let resource = resource.context("frame resource is None")?;
                let desktop: ID3D11Texture2D = resource.cast().context("cast to Texture2D")?;

                // Create staging texture on first frame (or if size changes)
                if self.staging.is_none() {
                    let mut desc = D3D11_TEXTURE2D_DESC::default();
                    desktop.GetDesc(&mut desc);
                    desc.Usage = D3D11_USAGE_STAGING;
                    desc.BindFlags = 0;
                    desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
                    desc.MiscFlags = 0;

                    let mut staging: Option<ID3D11Texture2D> = None;
                    self.device
                        .CreateTexture2D(&desc, None, Some(&mut staging))
                        .context("CreateTexture2D staging")?;
                    self.staging = staging;
                }
                let staging = self.staging.as_ref().unwrap();

                // Copy desktop -> staging
                self.context.CopyResource(staging, &desktop);

                // Map and read pixels
                let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
                self.context
                    .Map(staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                    .context("Map staging")?;

                let row_pitch = mapped.RowPitch as usize;
                let expected = (self.width * 4) as usize;

                // Copy row-by-row to tight packed BGRA
                let mut out = vec![0u8; self.width as usize * self.height as usize * 4];
                let src = mapped.pData as *const u8;
                for y in 0..self.height as usize {
                    std::ptr::copy_nonoverlapping(
                        src.add(y * row_pitch),
                        out.as_mut_ptr().add(y * expected),
                        expected,
                    );
                }

                self.context.Unmap(staging, 0);
                let _ = self.duplication.ReleaseFrame();

                Ok(Some(Frame {
                    width: self.width,
                    height: self.height,
                    data: Bytes::from(out),
                }))
            }
        }
    }

    // Silence unused warnings on non-Windows builds
    #[allow(dead_code)]
    fn _unused(_c: &ID3D11DeviceContext) {}
}

// =========================================================================
// Non-Windows stub (so this crate at least compiles on Linux for dev)
// =========================================================================
#[cfg(not(windows))]
mod stub_impl {
    use super::*;

    pub struct Capturer {
        pub width: u32,
        pub height: u32,
    }

    impl Capturer {
        pub fn new() -> Result<Self> {
            anyhow::bail!("Windows agent does not run on this platform")
        }
        pub fn grab(&mut self, _timeout_ms: u32) -> Result<Option<Frame>> {
            anyhow::bail!("Windows agent does not run on this platform")
        }
    }
}
