//! baseview `WindowHandler` hosting `ReposeEmbed` + wgpu.

use crate::clipboard::install_clipboard_hooks;
use crate::cursor::map_cursor;
use crate::embed::ReposeEmbed;
use crate::input::KeyCapture;
use crate::input::baseview_map;
use crate::render::{GpuSurface, attach_from_baseview_window};
use baseview::MouseCursor;
use baseview::{
    Event, EventStatus, Size, Window, WindowHandle, WindowHandler, WindowOpenOptions,
    WindowScalePolicy,
};
use raw_window_handle_05::HasRawWindowHandle;
use repose_core::{RenderContext, Scheduler, View};

pub struct ReposeBaseviewConfig {
    pub title: String,
    /// Logical size (pre-DPI) of the window.
    pub logical_size: (f64, f64),
    pub scale: WindowScalePolicy,
    pub msaa: u32,
    pub clear_color: [f64; 4],
    /// Whether keys the UI does not consume are let through to the DAW parent.
    /// Safe default: `true` so DAW transport keeps working while focused.
    pub key_fallthrough: bool,
}

impl Default for ReposeBaseviewConfig {
    fn default() -> Self {
        Self {
            title: "Repose".into(),
            logical_size: (480.0, 360.0),
            scale: WindowScalePolicy::SystemScaleFactor,
            msaa: 4,
            clear_color: [0.12, 0.12, 0.14, 1.0],
            key_fallthrough: true,
        }
    }
}

/// A thin host that owns the wgpu surface + embed and renders every frame.
pub struct ReposeWindowHandler {
    pub embed: ReposeEmbed,
    gpu: Option<GpuSurface>,
    logical: (f64, f64),
    scale: f32,
    cursor_logical: (f64, f64),
    msaa: u32,
    last_cursor: Option<MouseCursor>,
    key_capture: KeyCapture,
    key_fallthrough: bool,
    /// Optional per-frame hook (pull meters, params, resize requests, …).
    pub on_frame:
        Option<Box<dyn for<'a> FnMut(&mut ReposeEmbed, &mut Window<'a>) + Send + 'static>>,
}

impl ReposeWindowHandler {
    fn new<F>(
        window: &mut Window,
        config: &ReposeBaseviewConfig,
        root: F,
        on_frame: Option<
            Box<dyn for<'a> FnMut(&mut ReposeEmbed, &mut Window<'a>) + Send + 'static>,
        >,
    ) -> Self
    where
        F: FnMut(&mut Scheduler, &RenderContext) -> View + Send + 'static,
    {
        install_clipboard_hooks();

        let scale = match config.scale {
            WindowScalePolicy::ScaleFactor(s) => s as f32,
            WindowScalePolicy::SystemScaleFactor => 1.0,
        };
        let phy_w = (config.logical_size.0 * scale as f64).round().max(1.0) as u32;
        let phy_h = (config.logical_size.1 * scale as f64).round().max(1.0) as u32;

        let mut embed = ReposeEmbed::new(root)
            .with_clear_color(config.clear_color)
            .with_msaa(config.msaa);
        embed.set_size_px(phy_w, phy_h, scale);

        let gpu = attach_from_baseview_window(&mut embed, window, phy_w, phy_h, config.msaa)
            .map_err(|e| log::warn!("repose-audui: wgpu init failed: {e:#}"))
            .ok();

        Self {
            embed,
            gpu,
            logical: config.logical_size,
            scale,
            cursor_logical: (0.0, 0.0),
            msaa: config.msaa,
            last_cursor: None,
            key_capture: KeyCapture::CaptureAll,
            key_fallthrough: config.key_fallthrough,
            on_frame,
        }
    }

    fn apply_resize(&mut self, window: &mut Window, info: baseview::WindowInfo) {
        let s = info.scale() as f32;
        let phy = info.physical_size();
        self.logical = (info.logical_size().width, info.logical_size().height);
        self.scale = s;
        self.embed
            .set_size_px(phy.width.max(1), phy.height.max(1), s);
        if let Some(gpu) = &mut self.gpu {
            gpu.resize(phy.width.max(1), phy.height.max(1));
        } else {
            let gpu = attach_from_baseview_window(
                &mut self.embed,
                window,
                phy.width.max(1),
                phy.height.max(1),
                self.msaa,
            );
            match gpu {
                Ok(g) => self.gpu = Some(g),
                Err(e) => log::debug!("repose-audui: surface attach retry failed: {e:#}"),
            }
        }
    }

    fn apply_cursor(&mut self, window: &mut Window) {
        //HACK: baseview's cursor support on macOS is unreliable. Remove when stable.
        if cfg!(target_os = "macos") {
            return;
        }
        let icon = self
            .embed
            .platform()
            .and_then(|p| p.cursor)
            .or_else(|| self.embed.runtime.cursor_suggestion());
        let next = icon.map(map_cursor);
        if next != self.last_cursor {
            if let Some(c) = next {
                window.set_mouse_cursor(c);
            }
            self.last_cursor = next;
        }
    }
}

impl WindowHandler for ReposeWindowHandler {
    fn on_frame(&mut self, window: &mut Window) {
        if let Some(cb) = self.on_frame.as_mut() {
            cb(&mut self.embed, window);
        }

        if let Some(gpu) = self.gpu.as_mut() {
            if let Err(e) = gpu.present(&mut self.embed) {
                log::warn!("repose-audui: present failed: {e:#}");
            }
        } else {
            let _ = self.embed.compose();
        }

        self.apply_cursor(window);
    }

    fn on_event(&mut self, window: &mut Window, event: Event) -> EventStatus {
        match event {
            Event::Window(baseview::WindowEvent::Resized(info)) => {
                self.apply_resize(window, info);
                EventStatus::Captured
            }
            Event::Window(baseview::WindowEvent::Unfocused) => {
                self.embed.focus_lost();
                EventStatus::Captured
            }
            Event::Window(baseview::WindowEvent::Focused | baseview::WindowEvent::WillClose) => {
                EventStatus::Ignored
            }
            Event::Mouse(_) | Event::Keyboard(_) => {
                let status = baseview_map::dispatch(
                    &mut self.embed,
                    event,
                    &mut self.cursor_logical,
                    &self.key_capture,
                    self.key_fallthrough,
                );
                // Let unhandled keys fall through to the host when enabled.
                if self.key_fallthrough && status == EventStatus::Captured {
                    // Consuming the key is enough for the DAW not to see it.
                }
                status
            }
        }
    }
}

/// Open a parented plugin window. `parent` must implement rwh 0.5
/// `HasRawWindowHandle` (nih-plug `ParentWindowHandle`, baseview handles, …).
pub fn open_parented<P, F>(
    parent: &P,
    config: ReposeBaseviewConfig,
    root: F,
    on_frame: Option<Box<dyn for<'a> FnMut(&mut ReposeEmbed, &mut Window<'a>) + Send + 'static>>,
) -> WindowHandle
where
    P: HasRawWindowHandle,
    F: FnMut(&mut Scheduler, &RenderContext) -> View + Send + 'static,
{
    let options = WindowOpenOptions {
        title: config.title.clone(),
        size: Size::new(config.logical_size.0, config.logical_size.1),
        scale: config.scale,
        gl_config: None,
    };

    Window::open_parented(parent, options, move |window: &mut Window| {
        ReposeWindowHandler::new(window, &config, root, on_frame)
    })
}

/// Blocking window for examples / offline UI tests.
pub fn open_blocking<F>(config: ReposeBaseviewConfig, root: F)
where
    F: FnMut(&mut Scheduler, &RenderContext) -> View + Send + 'static,
{
    let options = WindowOpenOptions {
        title: config.title.clone(),
        size: Size::new(config.logical_size.0, config.logical_size.1),
        scale: config.scale,
        gl_config: None,
    };

    Window::open_blocking(options, move |window: &mut Window| {
        ReposeWindowHandler::new(window, &config, root, None)
    });
}
