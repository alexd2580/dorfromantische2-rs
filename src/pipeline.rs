use winit::window::Window;

use crate::gpu::Gpu;
use std::borrow::Cow;

pub struct Pipeline {
    _pipeline_layout: wgpu::PipelineLayout,
    render_pipeline: wgpu::RenderPipeline,

    egui_renderer: egui_wgpu::Renderer,
    egui_screen_descriptor: egui_wgpu::renderer::ScreenDescriptor,
}

impl Pipeline {
    pub fn new(gpu: &Gpu, window: &Window, bind_group_layouts: &[wgpu::BindGroupLayout]) -> Self {
        // Load the shaders from disk
        let vertex_shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("vertex_shader"),
                source: wgpu::ShaderSource::Glsl {
                    shader: Cow::Borrowed(include_str!("shader.vert")),
                    stage: naga::ShaderStage::Vertex,
                    defines: Default::default(),
                },
            });
        let fragment_shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("fragment_shader"),
                source: wgpu::ShaderSource::Glsl {
                    shader: Cow::Borrowed(include_str!("shader.frag")),
                    stage: naga::ShaderStage::Fragment,
                    defines: Default::default(),
                },
            });

        let bind_group_layouts = bind_group_layouts.iter().collect::<Vec<_>>();
        let pipeline_layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &bind_group_layouts,
                push_constant_ranges: &[],
            });

        let swapchain_format = gpu.swapchain_format();
        let render_pipeline = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &vertex_shader,
                    entry_point: "main",
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &fragment_shader,
                    entry_point: "main",
                    targets: &[Some(swapchain_format.into())],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

        let egui_renderer = egui_wgpu::Renderer::new(&gpu.device, swapchain_format, None, 1);
        let size = window.inner_size();
        let egui_screen_descriptor = egui_wgpu::renderer::ScreenDescriptor {
            size_in_pixels: [size.width, size.height],
            pixels_per_point: 1.0,
        };

        Pipeline {
            _pipeline_layout: pipeline_layout,
            render_pipeline,
            egui_renderer,
            egui_screen_descriptor,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.egui_screen_descriptor.size_in_pixels = [width, height];
    }

    pub fn redraw(
        &mut self,
        gpu: &Gpu,
        bind_groups: Option<&[wgpu::BindGroup]>,
        egui_paint_jobs: Vec<egui::ClippedPrimitive>,
        textures_delta: egui::TexturesDelta,
    ) {
        // Prepare frame resources.
        let (frame, view) = gpu.get_current_texture();
        let mut encoder = gpu.create_encoder();

        // Upload egui data.
        for (id, image_delta) in textures_delta.set {
            self.egui_renderer
                .update_texture(&gpu.device, &gpu.queue, id, &image_delta);
        }

        let mut command_buffers = self.egui_renderer.update_buffers(
            &gpu.device,
            &gpu.queue,
            &mut encoder,
            &egui_paint_jobs,
            &self.egui_screen_descriptor,
        );

        // Run renderpass (with egui at the end).
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::GREEN),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            // Run shader.
            if let Some(bind_groups) = bind_groups {
                render_pass.set_pipeline(&self.render_pipeline);
                for (index, bind_group) in bind_groups.iter().enumerate() {
                    render_pass.set_bind_group(index.try_into().unwrap(), bind_group, &[]);
                }
                render_pass.draw(0..4, 0..1);
            }

            // Render GUI.
            self.egui_renderer.render(
                &mut render_pass,
                &egui_paint_jobs,
                &self.egui_screen_descriptor,
            );
        }

        for id in textures_delta.free {
            self.egui_renderer.free_texture(&id);
        }

        command_buffers.push(encoder.finish());

        gpu.queue.submit(command_buffers);
        frame.present();
    }
}
