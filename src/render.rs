//! wgpu surface setup for plugin child windows.

use crate::embed::ReposeEmbed;
use crate::rwh_compat::RawHandles;
use anyhow::{Context, Result};

/// Owns a wgpu surface + device bound to a plugin child window, plus the raw
/// handles that back it (kept alive for the surface lifetime).
pub struct GpuSurface {
    pub instance: wgpu::Instance,
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    /// Keep the raw handles alive as long as the surface exists.
    _handles: RawHandles,
}

impl GpuSurface {
    pub fn resize(&mut self, width: u32, height: u32) {
        let w = width.max(1);
        let h = height.max(1);
        if self.config.width == w && self.config.height == h {
            return;
        }
        self.config.width = w;
        self.config.height = h;
        self.surface.configure(&self.device, &self.config);
    }

    /// Compose + render + present one frame.
    ///
    /// Handles lost/outdated/timeout surface conditions without panicking so a
    /// DAW can minimize / GPU can reset under a plugin window. Lost/outdated
    /// surfaces are reconfigured and retried a few times within the same frame.
    pub fn present(&mut self, embed: &mut ReposeEmbed) -> Result<()> {
        embed.compose();

        let mut retries = 0u32;
        const MAX_RETRIES: u32 = 4;
        let frame = loop {
            match self.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(f)
                | wgpu::CurrentSurfaceTexture::Suboptimal(f) => break f,
                wgpu::CurrentSurfaceTexture::Lost
                | wgpu::CurrentSurfaceTexture::Outdated
                | wgpu::CurrentSurfaceTexture::Validation => {
                    retries += 1;
                    if retries >= MAX_RETRIES {
                        log::warn!("surface recovery failed after {MAX_RETRIES}; skip frame");
                        return Ok(());
                    }
                    self.surface.configure(&self.device, &self.config);
                }
                wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                    return Ok(());
                }
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("repose-audui encoder"),
            });

        embed.render_to(&mut encoder, &view, None);
        self.queue.submit(Some(encoder.finish()));
        self.queue.present(frame);
        Ok(())
    }
}

pub fn pick_msaa(adapter: &wgpu::Adapter, requested: u32, format: wgpu::TextureFormat) -> u32 {
    repose_render_wgpu::pick_surface_msaa(adapter, format, requested)
}

/// Create a device + surface from rwh 0.6 handles and attach the renderer.
pub fn attach_from_handles(
    embed: &mut ReposeEmbed,
    handles: RawHandles,
    width: u32,
    height: u32,
    msaa: u32,
) -> Result<GpuSurface> {
    let instance = wgpu::Instance::default();

    // SAFETY: `handles` (and the window/display they point at) are kept alive
    // by `GpuSurface` for as long as this surface exists.
    let surface = unsafe {
        instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: handles.display,
            raw_window_handle: handles.window,
        })?
    };

    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
        apply_limit_buckets: false,
    }))
    .context("request_adapter failed")?;

    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("repose-audui device"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::default(),
        trace: wgpu::Trace::Off,
    }))
    .context("request_device failed")?;

    let caps = surface.get_capabilities(&adapter);
    let format = caps
        .formats
        .iter()
        .copied()
        .find(|f| f.is_srgb())
        .unwrap_or(caps.formats[0]);
    let msaa = pick_msaa(&adapter, msaa.max(1), format);
    // Prefer Fifo (vsync; safe default for plugin windows), fall back to
    // Mailbox then Immediate when the host compositor lacks it.
    let present_mode = caps
        .present_modes
        .iter()
        .copied()
        .find(|m| *m == wgpu::PresentMode::Fifo)
        .or_else(|| {
            caps.present_modes
                .iter()
                .copied()
                .find(|m| *m == wgpu::PresentMode::Mailbox)
        })
        .unwrap_or(wgpu::PresentMode::Immediate);

    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: width.max(1),
        height: height.max(1),
        present_mode,
        alpha_mode: caps.alpha_modes[0],
        color_space: wgpu::SurfaceColorSpace::Auto,
        view_formats: vec![],
        desired_maximum_frame_latency: 1,
    };
    surface.configure(&device, &config);

    let surface =
        unsafe { std::mem::transmute::<wgpu::Surface<'_>, wgpu::Surface<'static>>(surface) };

    embed.attach_device(device.clone(), queue.clone(), format, msaa);
    embed.set_size_px(config.width, config.height, embed.scale);

    Ok(GpuSurface {
        instance,
        surface,
        device,
        queue,
        config,
        _handles: handles,
    })
}

/// Convenience when you already have rwh 0.6 window + display handles.
pub fn attach_from_raw_window(
    embed: &mut ReposeEmbed,
    window: raw_window_handle::RawWindowHandle,
    display: raw_window_handle::RawDisplayHandle,
    width: u32,
    height: u32,
    msaa: u32,
) -> Result<GpuSurface> {
    attach_from_handles(
        embed,
        RawHandles::from_rwh06(window, display),
        width,
        height,
        msaa,
    )
}

/// Convenience for baseview windows (rwh 0.5 handles get bridged to 0.6).
#[cfg(feature = "baseview")]
pub fn attach_from_baseview_window(
    embed: &mut ReposeEmbed,
    window: &baseview::Window<'_>,
    width: u32,
    height: u32,
    msaa: u32,
) -> Result<GpuSurface> {
    use raw_window_handle_05::{HasRawDisplayHandle, HasRawWindowHandle};

    let handles = RawHandles::from_rwh05(window.raw_window_handle(), window.raw_display_handle())?;
    attach_from_handles(embed, handles, width, height, msaa)
}
