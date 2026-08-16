//! Shared host object.

use repose_app::{FrameOutput, PlatformOutput, ReposeRuntime};
use repose_core::input::{ImeEvent, KeyEvent, PointerButton};
use repose_core::{RenderContext, Scheduler, Vec2, View, ViewKind};
use repose_render_wgpu::{WgpuSceneRenderer, apply_render_commands};

/// Embeddable Repose host. Owns runtime + scene cache + GPU renderer.
pub struct ReposeEmbed {
    pub runtime: ReposeRuntime,
    pub render_ctx: RenderContext,
    pub root: Box<dyn FnMut(&mut Scheduler, &RenderContext) -> View + 'static>,
    pub last: Option<FrameOutput>,
    renderer: Option<WgpuSceneRenderer>,
    /// Device pixel ratio used to convert logical to physical positions.
    pub scale: f32,
    /// Physical (device pixel) size of the render target.
    pub size_px: (u32, u32),
    pub clear_color: [f64; 4],
    pub msaa_samples: u32,
}

impl ReposeEmbed {
    pub fn new<F>(root: F) -> Self
    where
        F: FnMut(&mut Scheduler, &RenderContext) -> View + 'static,
    {
        Self {
            runtime: ReposeRuntime::new(),
            render_ctx: RenderContext::new(),
            root: Box::new(root),
            last: None,
            renderer: None,
            scale: 1.0,
            size_px: (1, 1),
            clear_color: [0.12, 0.12, 0.14, 1.0],
            msaa_samples: 4,
        }
    }

    pub fn with_clear_color(mut self, rgba: [f64; 4]) -> Self {
        self.clear_color = rgba;
        self
    }

    pub fn with_msaa(mut self, samples: u32) -> Self {
        self.msaa_samples = samples.max(1);
        self
    }

    /// Attach an already created wgpu device (shared-device hosts). Repose does
    /// the rest: pipelines, ATLAS upload, image decode, MSAA target.
    pub fn attach_device(
        &mut self,
        device: wgpu::Device,
        queue: wgpu::Queue,
        format: wgpu::TextureFormat,
        msaa: u32,
    ) {
        self.msaa_samples = msaa.max(1);
        self.renderer = Some(WgpuSceneRenderer::from_device(
            device,
            queue,
            format,
            self.msaa_samples,
        ));
        if self.size_px.0 > 1 || self.size_px.1 > 1 {
            if let Some(r) = &mut self.renderer {
                r.resize(self.size_px.0, self.size_px.1);
            }
        }
    }

    pub fn has_renderer(&self) -> bool {
        self.renderer.is_some()
    }

    pub fn renderer_mut(&mut self) -> Option<&mut WgpuSceneRenderer> {
        self.renderer.as_mut()
    }

    /// Physical size + DPI. Repose layout and input run in device pixels.
    pub fn set_size_px(&mut self, w: u32, h: u32, scale: f32) {
        self.size_px = (w.max(1), h.max(1));
        self.scale = scale.max(1e-6);
        self.runtime
            .set_viewport_and_scale(self.size_px.0, self.size_px.1, self.scale);
        if let Some(r) = &mut self.renderer {
            r.resize(self.size_px.0, self.size_px.1);
        }
    }

    /// Compose the UI, cache the frame, and drain image-upload commands into the
    /// GPU renderer. Returns the composed frame output.
    pub fn compose(&mut self) -> &FrameOutput {
        // The root must be temporarily "borrowed out" so we can hand a &mut F to
        // compose_frame_output (same hack as repose-bevy).
        let mut root = std::mem::replace(
            &mut self.root,
            Box::new(|_: &mut Scheduler, _: &RenderContext| View::new(0, ViewKind::Box)),
        );

        let out = self
            .runtime
            .compose_frame_output(&mut root, &self.render_ctx);

        self.root = root;

        self.runtime.ensure_focused_textfield_state();

        if let Some(r) = &mut self.renderer {
            apply_render_commands(r, self.render_ctx.drain());
        }

        self.last = Some(out);
        self.last.as_ref().unwrap()
    }

    /// Render the last composed scene into an external texture view. The host
    /// submits the encoder and presents.
    pub fn render_to(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        clear: Option<[f64; 4]>,
    ) {
        let scene = &self
            .last
            .as_ref()
            .expect("compose() before render_to()")
            .scene;
        let r = self
            .renderer
            .as_mut()
            .expect("attach a device / surface before render_to()");
        let clear = Some(clear.unwrap_or(self.clear_color));
        r.render_to_view(scene, encoder, view, self.size_px.0, self.size_px.1, clear);
    }

    pub fn pointer_move(&mut self, pos_px: Vec2) {
        let _ = self.runtime.handle_pointer_move(pos_px);
    }

    pub fn pointer_down(&mut self, pos_px: Vec2, b: PointerButton) {
        let _ = self.runtime.handle_pointer_press(pos_px, b);
    }

    pub fn pointer_up(&mut self, pos_px: Vec2, b: PointerButton) {
        let _ = self.runtime.handle_pointer_release(pos_px, b);
    }

    pub fn pointer_cancel(&mut self) {
        self.runtime.handle_pointer_cancel();
    }

    pub fn scroll(&mut self, delta: Vec2) {
        let _ = self.runtime.handle_scroll(delta);
    }

    pub fn key(&mut self, e: &KeyEvent) -> bool {
        self.runtime.handle_key(e)
    }

    pub fn key_with_text(&mut self, e: &KeyEvent, composed: Option<&str>) -> bool {
        self.runtime.handle_key_with_text(e, composed)
    }

    pub fn text(&mut self, s: &str) -> bool {
        self.runtime.insert_text(s)
    }

    pub fn ime(&mut self, e: &ImeEvent) {
        self.runtime.handle_ime(e);
    }

    pub fn focus_lost(&mut self) {
        self.runtime.handle_focus_lost();
    }

    pub fn clear_hover(&mut self) {
        self.runtime.clear_hover();
    }

    pub fn set_modifiers(&mut self, m: repose_core::input::Modifiers) {
        self.runtime.modifiers = m;
    }

    pub fn set_pointer_inside(&mut self, inside: bool) {
        self.runtime.pointer_inside = inside;
    }

    pub fn platform(&self) -> Option<&PlatformOutput> {
        self.last.as_ref().map(|f| &f.platform)
    }

    pub fn last_frame(&self) -> Option<&FrameOutput> {
        self.last.as_ref()
    }
}
