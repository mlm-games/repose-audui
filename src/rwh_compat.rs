//! Bridge between `raw-window-handle` **0.6** (wgpu 30) and **0.5** (baseview /
//! nih-plug). The wgpu surface holds these handles, so the window/display must
//! outlive the `GpuSurface`.

use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

/// Owned rwh 0.6 handles for surface creation.
pub struct RawHandles {
    pub window: RawWindowHandle,
    pub display: Option<RawDisplayHandle>,
}

impl RawHandles {
    pub fn from_rwh06(window: RawWindowHandle, display: RawDisplayHandle) -> Self {
        Self {
            window,
            display: Some(display),
        }
    }

    #[cfg(feature = "baseview")]
    pub fn from_rwh05(
        window: raw_window_handle_05::RawWindowHandle,
        display: raw_window_handle_05::RawDisplayHandle,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            window: convert_window_05_to_06(window)?,
            display: Some(convert_display_05_to_06(display)?),
        })
    }
}

#[cfg(feature = "baseview")]
fn convert_window_05_to_06(
    h: raw_window_handle_05::RawWindowHandle,
) -> anyhow::Result<RawWindowHandle> {
    use std::num::{NonZeroIsize, NonZeroU32};
    use std::ptr::NonNull;

    Ok(match h {
        raw_window_handle_05::RawWindowHandle::Win32(w) => {
            let mut out = raw_window_handle::Win32WindowHandle::new(
                NonZeroIsize::new(w.hwnd as isize)
                    .ok_or_else(|| anyhow::anyhow!("null HWND"))?,
            );
            out.hinstance = NonZeroIsize::new(w.hinstance as isize);
            RawWindowHandle::Win32(out)
        }
        raw_window_handle_05::RawWindowHandle::AppKit(w) => RawWindowHandle::AppKit(
            raw_window_handle::AppKitWindowHandle::new(
                NonNull::new(w.ns_view).ok_or_else(|| anyhow::anyhow!("null ns_view"))?,
            ),
        ),
        raw_window_handle_05::RawWindowHandle::Xlib(w) => {
            let mut out = raw_window_handle::XlibWindowHandle::new(w.window);
            out.visual_id = w.visual_id;
            RawWindowHandle::Xlib(out)
        }
        raw_window_handle_05::RawWindowHandle::Xcb(w) => {
            let mut out = raw_window_handle::XcbWindowHandle::new(
                NonZeroU32::new(w.window).ok_or_else(|| anyhow::anyhow!("null xcb window"))?,
            );
            out.visual_id = NonZeroU32::new(w.visual_id);
            RawWindowHandle::Xcb(out)
        }
        other => anyhow::bail!("unsupported rwh 0.5 window handle on this build: {other:?}"),
    })
}

#[cfg(feature = "baseview")]
fn convert_display_05_to_06(
    h: raw_window_handle_05::RawDisplayHandle,
) -> anyhow::Result<RawDisplayHandle> {
    use std::ptr::NonNull;

    Ok(match h {
        raw_window_handle_05::RawDisplayHandle::Windows(_) => {
            RawDisplayHandle::Windows(raw_window_handle::WindowsDisplayHandle::new())
        }
        raw_window_handle_05::RawDisplayHandle::AppKit(_) => {
            RawDisplayHandle::AppKit(raw_window_handle::AppKitDisplayHandle::new())
        }
        raw_window_handle_05::RawDisplayHandle::Xlib(d) => {
            RawDisplayHandle::Xlib(raw_window_handle::XlibDisplayHandle::new(
                NonNull::new(d.display),
                d.screen,
            ))
        }
        raw_window_handle_05::RawDisplayHandle::Xcb(d) => {
            RawDisplayHandle::Xcb(raw_window_handle::XcbDisplayHandle::new(
                NonNull::new(d.connection),
                d.screen,
            ))
        }
        other => anyhow::bail!("unsupported rwh 0.5 display handle on this build: {other:?}"),
    })
}