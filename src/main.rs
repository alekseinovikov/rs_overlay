use std::{sync::Arc, time::Instant};

mod platform;

use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{WindowBuilder, WindowLevel},
};

struct RenderState {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
}

struct EguiState {
    ctx: egui::Context,
    state: egui_winit::State,
    renderer: egui_wgpu::Renderer,
}

struct FpsTracker {
    last_instant: Instant,
    frame_count: u32,
    fps: f32,
}

impl FpsTracker {
    fn new() -> Self {
        Self {
            last_instant: Instant::now(),
            frame_count: 0,
            fps: 0.0,
        }
    }

    fn tick(&mut self) {
        self.frame_count = self.frame_count.saturating_add(1);
        let elapsed = self.last_instant.elapsed();
        if elapsed.as_secs_f32() >= 1.0 {
            self.fps = self.frame_count as f32 / elapsed.as_secs_f32();
            self.frame_count = 0;
            self.last_instant = Instant::now();
        }
    }

    fn fps(&self) -> f32 {
        self.fps
    }
}

fn main() {
    pollster::block_on(run());
}

async fn run() {
    let event_loop = EventLoop::new().expect("create event loop");
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("rs_overlay")
            .with_decorations(false)
            .with_resizable(false)
            .with_transparent(true)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .build(&event_loop)
            .expect("create window"),
    );

    platform::configure_overlay(&window);

    if let Some(monitor) = window.primary_monitor() {
        let position: PhysicalPosition<i32> = monitor.position();
        let size: PhysicalSize<u32> = monitor.size();
        window.as_ref().set_outer_position(position);
        let _ = window.as_ref().request_inner_size(size);
    }

    let size = window.inner_size();
    let instance = wgpu::Instance::default();
    let surface = instance
        .create_surface(window.clone())
        .expect("create surface");
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        })
        .await
        .expect("find adapter");
    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
            },
            None,
        )
        .await
        .expect("create device");
    let surface_caps = surface.get_capabilities(&adapter);
    let surface_format = surface_caps.formats[0];
    let surface_alpha_mode = pick_alpha_mode(&surface_caps.alpha_modes);
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface_format,
        width: size.width.max(1),
        height: size.height.max(1),
        present_mode: wgpu::PresentMode::Fifo,
        alpha_mode: surface_alpha_mode,
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&device, &config);

    let ctx = egui::Context::default();
    let state =
        egui_winit::State::new(ctx.clone(), egui::ViewportId::ROOT, &event_loop, None, None);
    let renderer = egui_wgpu::Renderer::new(&device, surface_format, None, 1);

    let mut render_state = RenderState {
        device,
        queue,
        surface,
        config,
    };
    let mut egui_state = EguiState {
        ctx,
        state,
        renderer,
    };
    let mut fps_tracker = FpsTracker::new();

    let _ = event_loop.run(move |event, target| {
        target.set_control_flow(ControlFlow::Poll);

        match event {
            Event::WindowEvent { event, window_id } if window_id == window.id() => {
                match event {
                    WindowEvent::CloseRequested => {
                        target.exit();
                        return;
                    }
                    WindowEvent::Resized(size) => {
                        if size.width > 0 && size.height > 0 {
                            render_state.config.width = size.width;
                            render_state.config.height = size.height;
                            render_state
                                .surface
                                .configure(&render_state.device, &render_state.config);
                        }
                    }
                    WindowEvent::RedrawRequested => {
                        fps_tracker.tick();

                        let raw_input = egui_state.state.take_egui_input(&window);
                        let full_output = egui_state.ctx.run(raw_input, |ctx| {
                            let fps_text = format!("FPS: {:.1}", fps_tracker.fps());
                            egui::Area::new(egui::Id::new("fps_overlay"))
                                .fixed_pos(egui::pos2(12.0, 12.0))
                                .show(ctx, |ui| {
                                    egui::Frame::none().show(ui, |ui| {
                                        ui.label(egui::RichText::new(fps_text).size(18.0));
                                    });
                                });
                        });

                        egui_state
                            .state
                            .handle_platform_output(&window, full_output.platform_output);

                        let paint_jobs = egui_state
                            .ctx
                            .tessellate(full_output.shapes, full_output.pixels_per_point);
                        let screen_descriptor = egui_wgpu::ScreenDescriptor {
                            size_in_pixels: [render_state.config.width, render_state.config.height],
                            pixels_per_point: egui_state.ctx.pixels_per_point(),
                        };

                        for (id, image_delta) in &full_output.textures_delta.set {
                            egui_state.renderer.update_texture(
                                &render_state.device,
                                &render_state.queue,
                                *id,
                                image_delta,
                            );
                        }

                        for id in &full_output.textures_delta.free {
                            egui_state.renderer.free_texture(id);
                        }

                        let output_frame = match render_state.surface.get_current_texture() {
                            Ok(frame) => frame,
                            Err(wgpu::SurfaceError::Lost) => {
                                render_state
                                    .surface
                                    .configure(&render_state.device, &render_state.config);
                                return;
                            }
                            Err(wgpu::SurfaceError::OutOfMemory) => {
                                target.exit();
                                return;
                            }
                            Err(_) => return,
                        };
                        let view = output_frame
                            .texture
                            .create_view(&wgpu::TextureViewDescriptor::default());
                        let mut encoder = render_state.device.create_command_encoder(
                            &wgpu::CommandEncoderDescriptor { label: None },
                        );
                        egui_state.renderer.update_buffers(
                            &render_state.device,
                            &render_state.queue,
                            &mut encoder,
                            &paint_jobs,
                            &screen_descriptor,
                        );

                        {
                            let mut rpass =
                                encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                    label: None,
                                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                        view: &view,
                                        resolve_target: None,
                                        ops: wgpu::Operations {
                                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                                r: 0.0,
                                                g: 0.0,
                                                b: 0.0,
                                                a: 0.0,
                                            }),
                                            store: wgpu::StoreOp::Store,
                                        },
                                    })],
                                    depth_stencil_attachment: None,
                                    occlusion_query_set: None,
                                    timestamp_writes: None,
                                });

                            egui_state
                                .renderer
                                .render(&mut rpass, &paint_jobs, &screen_descriptor);
                        }

                        render_state.queue.submit(Some(encoder.finish()));
                        output_frame.present();
                    }
                    _ => {}
                }

                let response = egui_state.state.on_window_event(&window, &event);
                if response.repaint {
                    window.request_redraw();
                }
            }
            Event::AboutToWait => {
                window.request_redraw();
            }
            _ => {}
        }
    });
}

fn pick_alpha_mode(modes: &[wgpu::CompositeAlphaMode]) -> wgpu::CompositeAlphaMode {
    let preferred = [
        wgpu::CompositeAlphaMode::PreMultiplied,
        wgpu::CompositeAlphaMode::PostMultiplied,
        wgpu::CompositeAlphaMode::Inherit,
        wgpu::CompositeAlphaMode::Opaque,
    ];

    preferred
        .into_iter()
        .find(|mode| modes.contains(mode))
        .unwrap_or(wgpu::CompositeAlphaMode::Opaque)
}
