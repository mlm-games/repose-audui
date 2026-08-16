//! nih-plug [`Editor`] adapter (same spawn model as `nih_plug_egui`).

use crate::baseview_host::{ReposeBaseviewConfig, open_parented};
use crate::embed::ReposeEmbed;
use baseview::{WindowHandle, WindowScalePolicy};
use crossbeam::atomic::AtomicCell;
use nih_plug::prelude::{Editor, GuiContext, ParentWindowHandle};
use raw_window_handle_05::{HasRawWindowHandle, RawWindowHandle};
use repose_core::{RenderContext, Scheduler, View};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Persistent editor chrome state (size, open flag, scale).
pub struct ReposeEditorState {
    pub width: AtomicCell<u32>,
    pub height: AtomicCell<u32>,
    pub open: AtomicBool,
    pub scaling_factor: AtomicCell<Option<f32>>,
    /// UI-thread-only request; consumed in on_frame.
    pub requested_size: AtomicCell<Option<(u32, u32)>>,
}

impl ReposeEditorState {
    pub fn new(width: u32, height: u32) -> Arc<Self> {
        Arc::new(Self {
            width: AtomicCell::new(width),
            height: AtomicCell::new(height),
            open: AtomicBool::new(false),
            scaling_factor: AtomicCell::new(None),
            requested_size: AtomicCell::new(None),
        })
    }

    /// Ask the host to resize the editor to `width` x `height` logical pixels.
    pub fn request_resize(&self, width: u32, height: u32) {
        self.requested_size
            .store(Some((width.max(1), height.max(1))));
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width.load(), self.height.load())
    }

    pub fn is_open(&self) -> bool {
        self.open.load(Ordering::Acquire)
    }
}

struct ParentWindowHandleAdapter(ParentWindowHandle);

unsafe impl HasRawWindowHandle for ParentWindowHandleAdapter {
    fn raw_window_handle(&self) -> RawWindowHandle {
        self.0.raw_window_handle()
    }
}

type RootFactory =
    dyn Fn(&mut Scheduler, &RenderContext, &Arc<dyn GuiContext>) -> View + Send + Sync + 'static;

struct ReposeEditor {
    state: Arc<ReposeEditorState>,
    title: String,
    /// Build root view. Called every compose; read params from Arc inside.
    root: Arc<RootFactory>,
    /// Optional: called each frame before compose (meters, etc.).
    on_frame: Option<Arc<dyn Fn(&mut ReposeEmbed, &Arc<dyn GuiContext>) + Send + Sync + 'static>>,
}

impl Editor for ReposeEditor {
    fn spawn(
        &self,
        parent: ParentWindowHandle,
        context: Arc<dyn GuiContext>,
    ) -> Box<dyn std::any::Any + Send> {
        let (w, h) = self.state.size();
        let scaling = self.state.scaling_factor.load();
        let root = self.root.clone();
        let on_frame = self.on_frame.clone();
        let ctx_frame = context.clone();
        let ctx_resize = context.clone();
        let state_frame = self.state.clone();

        let config = ReposeBaseviewConfig {
            title: self.title.clone(),
            logical_size: (w as f64, h as f64),
            scale: scaling
                .map(|f| WindowScalePolicy::ScaleFactor(f as f64))
                .unwrap_or(WindowScalePolicy::SystemScaleFactor),
            msaa: 4,
            clear_color: [0.10, 0.10, 0.12, 1.0],
            key_fallthrough: true,
            key_capture: crate::input::KeyCapture::CaptureAll,
        };

        let window = open_parented(
            &ParentWindowHandleAdapter(parent),
            config,
            {
                let root = root.clone();
                let context = context.clone();
                move |sched, rctx| (root)(sched, rctx, &context)
            },
            Some(Box::new(move |embed, window| {
                if let Some(cb) = &on_frame {
                    cb(embed, &ctx_frame);
                }

                if let Some((nw, nh)) = state_frame.requested_size.swap(None)
                    && ctx_resize.request_resize()
                {
                    window.resize(baseview::Size::new(nw as f64, nh as f64));
                    state_frame.width.store(nw);
                    state_frame.height.store(nh);
                }
            })),
        );

        self.state.open.store(true, Ordering::Release);

        Box::new(ReposeEditorHandle {
            state: self.state.clone(),
            window,
        })
    }

    fn size(&self) -> (u32, u32) {
        if let Some(s) = self.state.requested_size.load() {
            s
        } else {
            self.state.size()
        }
    }

    fn set_scale_factor(&self, factor: f32) -> bool {
        if self.state.is_open() {
            return false;
        }
        self.state.scaling_factor.store(Some(factor));
        true
    }

    fn param_value_changed(&self, _id: &str, _normalized_value: f32) {
        // Always redraw while open (meters / automation). baseview on_frame always paints.
    }

    fn param_modulation_changed(&self, _id: &str, _modulation_offset: f32) {}

    fn param_values_changed(&self) {}
}

struct ReposeEditorHandle {
    state: Arc<ReposeEditorState>,
    window: WindowHandle,
}

// baseview WindowHandle is !Send; same escape hatch as nih_plug_egui.
unsafe impl Send for ReposeEditorHandle {}

impl Drop for ReposeEditorHandle {
    fn drop(&mut self) {
        self.state.open.store(false, Ordering::Release);
        self.window.close();
    }
}

/// Create a nih-plug [`Editor`] that hosts Repose.
///
/// `root` is invoked every compose frame. Use `ParamSetter::new(gui.as_ref())`
/// inside gesture callbacks (not during pure compose if you can avoid it).
pub fn create_repose_editor<F>(
    state: Arc<ReposeEditorState>,
    title: impl Into<String>,
    root: F,
    on_frame: Option<Arc<dyn Fn(&mut ReposeEmbed, &Arc<dyn GuiContext>) + Send + Sync + 'static>>,
) -> Box<dyn Editor>
where
    F: Fn(&mut Scheduler, &RenderContext, &Arc<dyn GuiContext>) -> View + Send + Sync + 'static,
{
    Box::new(ReposeEditor {
        state,
        title: title.into(),
        root: Arc::new(root),
        on_frame,
    })
}
