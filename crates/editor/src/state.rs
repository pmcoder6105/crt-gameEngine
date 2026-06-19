//! The egui integration: window-event input, per-frame UI, and wgpu painting.
//!
//! [`EditorState`] owns the three pieces egui needs to live inside a winit +
//! wgpu app — an [`egui::Context`], an [`egui_winit::State`] (turns winit events
//! into egui input and applies egui's platform output), and an
//! [`egui_wgpu::Renderer`] (uploads egui's meshes/textures and draws them) — plus
//! the panel [`Editor`](crate::Editor) that produces the actual UI.
//!
//! The flow each frame is: [`integrate_event`](EditorState::integrate_event) for
//! every window event → [`run_frame`](EditorState::run_frame) once to build the
//! UI and tessellate it into paint jobs → [`paint`](EditorState::paint) to record
//! those jobs into the frame's command encoder.

use egui_winit::winit;
use elderforge_scene::Scene;

use crate::Editor;

/// The tessellated result of one editor frame, handed from
/// [`run_frame`](EditorState::run_frame) to [`paint`](EditorState::paint).
pub struct EditorFrame {
    /// egui's draw commands for this frame.
    pub paint_jobs: Vec<egui::ClippedPrimitive>,
    /// Textures egui created or freed this frame.
    pub textures_delta: egui::TexturesDelta,
    /// Logical-to-physical pixel ratio the jobs were tessellated at.
    pub pixels_per_point: f32,
}

/// Timings and counters the [stats panel](crate::panels) shows; the app fills
/// these in each frame from its own measurements.
#[derive(Default, Clone, Copy)]
pub struct EditorStats {
    pub frame_time_ms: f32,
    pub physics_time_ms: f32,
}

pub struct EditorState {
    context: egui::Context,
    winit_state: egui_winit::State,
    renderer: egui_wgpu::Renderer,
    /// The panels themselves (hierarchy, inspector, sim controls, stats, …).
    pub editor: Editor,
}

impl EditorState {
    /// Create the editor's egui integration. `output_format` must be the surface
    /// format the editor will paint into; `window` seeds the input state with
    /// the current scale factor and display handle.
    pub fn new(
        device: &wgpu::Device,
        output_format: wgpu::TextureFormat,
        window: &winit::window::Window,
    ) -> Self {
        let context = egui::Context::default();
        let winit_state = egui_winit::State::new(
            context.clone(),
            context.viewport_id(),
            window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );
        // No depth buffer, single sample, no extra dithering: the editor draws
        // flat over the finished 3D frame.
        let renderer = egui_wgpu::Renderer::new(device, output_format, None, 1, false);
        Self { context, winit_state, renderer, editor: Editor::new() }
    }

    /// Forward one winit window event to egui. Returns `true` if egui consumed
    /// it (e.g. a click landed on a panel), so the caller can suppress it from
    /// the camera / picking logic.
    pub fn integrate_event(
        &mut self,
        window: &winit::window::Window,
        event: &winit::event::WindowEvent,
    ) -> bool {
        self.winit_state.on_window_event(window, event).consumed
    }

    /// Run one editor frame: feed accumulated input to egui, lay out every panel
    /// over `scene`, and tessellate the result into paint jobs. `stats` populates
    /// the read-only stats panel.
    pub fn run_frame(
        &mut self,
        window: &winit::window::Window,
        scene: &mut Scene,
        stats: EditorStats,
    ) -> EditorFrame {
        let Self { context, winit_state, editor, .. } = self;
        editor.set_stats(stats);

        let raw_input = winit_state.take_egui_input(window);
        let full_output = context.run(raw_input, |ctx| editor.ui(ctx, scene));
        winit_state.handle_platform_output(window, full_output.platform_output);

        let paint_jobs = context.tessellate(full_output.shapes, full_output.pixels_per_point);
        EditorFrame {
            paint_jobs,
            textures_delta: full_output.textures_delta,
            pixels_per_point: full_output.pixels_per_point,
        }
    }

    /// Record the editor's paint jobs into `encoder`, drawing over `view`
    /// (loading, not clearing, so the 3D scene shows through). `size_in_pixels`
    /// is the physical surface size.
    pub fn paint(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        size_in_pixels: [u32; 2],
        frame: &EditorFrame,
    ) {
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels,
            pixels_per_point: frame.pixels_per_point,
        };
        for (id, image_delta) in &frame.textures_delta.set {
            self.renderer.update_texture(device, queue, *id, image_delta);
        }
        self.renderer
            .update_buffers(device, queue, encoder, &frame.paint_jobs, &screen_descriptor);

        {
            let mut render_pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("egui"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                })
                // egui-wgpu's `render` wants a `'static` pass; the encoder still
                // outlives it because the pass is dropped at the end of this block.
                .forget_lifetime();
            self.renderer.render(&mut render_pass, &frame.paint_jobs, &screen_descriptor);
        }

        for id in &frame.textures_delta.free {
            self.renderer.free_texture(id);
        }
    }
}
