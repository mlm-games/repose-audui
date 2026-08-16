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
use repose_core::cursor::CursorIcon;
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
    /// Optional keyboard filter (space → DAW, etc.). Only consulted while the
    /// UI wants the keyboard (focused textfield / IME) when `key_fallthrough`.
    pub key_capture: KeyCapture,
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
            key_capture: KeyCapture::CaptureAll,
        }
    }
}

/// A thin host that owns the wgpu surface + embed and renders every frame.
pub struct ReposeWindowHandler {
    pub embed: ReposeEmbed,
    gpu: Option<GpuSurface>,
    logical: (f64, f64),
    scale: f32,
    /// When true, Resized events update scale from the system.
    use_system_scaling: bool,
    cursor_logical: (f64, f64),
    msaa: u32,
    last_cursor: Option<MouseCursor>,
    key_capture: KeyCapture,
    key_fallthrough: bool,
    /// Force one dummy resize after first frame (vizia pattern) so
    /// `SystemScaleFactor` is corrected on HiDPI before the user interacts.
    needs_scale_kick: bool,
    /// Optional logical size the UI wants (host/on_frame can set). Applied next frame.
    pending_logical_resize: Option<(f64, f64)>,
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

        let (use_system_scaling, scale) = match config.scale {
            WindowScalePolicy::ScaleFactor(s) => (false, s as f32),
            // Start at 1.0; first Resized (or the scale kick) fixes it on HiDPI
            // before the user interacts.
            WindowScalePolicy::SystemScaleFactor => (true, 1.0),
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
            use_system_scaling,
            cursor_logical: (0.0, 0.0),
            msaa: config.msaa,
            last_cursor: None,
            key_capture: config.key_capture.clone(),
            key_fallthrough: config.key_fallthrough,
            needs_scale_kick: use_system_scaling,
            pending_logical_resize: None,
            on_frame,
        }
    }

    /// Request a logical resize (plugin UI or host). Applied in `on_frame`.
    pub fn request_logical_resize(&mut self, w: f64, h: f64) {
        self.pending_logical_resize = Some((w.max(1.0), h.max(1.0)));
    }

    pub fn set_key_capture(&mut self, c: KeyCapture) {
        self.key_capture = c;
    }

    fn apply_resize(&mut self, window: &mut Window, info: baseview::WindowInfo) {
        if self.use_system_scaling {
            self.scale = info.scale() as f32;
        }
        let phy = info.physical_size();
        // Prefer physical from baseview; logical tracked for host size APIs.
        self.logical = (info.logical_size().width, info.logical_size().height);
        self.embed
            .set_size_px(phy.width.max(1), phy.height.max(1), self.scale);
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
        // HACK: baseview's cursor support on macOS is unreliable. Remove when stable.
        if cfg!(target_os = "macos") {
            return;
        }
        let icon = self
            .embed
            .platform()
            .and_then(|p| p.cursor)
            .or(Some(CursorIcon::Default));
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
        if self.needs_scale_kick {
            self.needs_scale_kick = false;
            let (w, h) = self.logical;
            window.resize(Size::new(w + 1.0, h + 1.0));
            window.resize(Size::new(w, h));
        }

        if let Some((w, h)) = self.pending_logical_resize.take() {
            self.logical = (w, h);
            window.resize(Size::new(w, h));
        }

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
                self.embed.set_modifiers(Default::default());
                self.embed.focus_lost();
                EventStatus::Captured
            }
            Event::Window(baseview::WindowEvent::WillClose) => {
                self.embed.focus_lost();
                EventStatus::Ignored
            }
            Event::Window(baseview::WindowEvent::Focused) => EventStatus::Ignored,
            Event::Mouse(_) | Event::Keyboard(_) => baseview_map::dispatch(
                &mut self.embed,
                event,
                &mut self.cursor_logical,
                &self.key_capture,
                self.key_fallthrough,
            ),
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
