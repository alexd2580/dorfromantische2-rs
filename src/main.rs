use gpu_data::Tile;
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet, VecDeque},
    fs::File,
    iter,
    time::SystemTime,
};
use wgpu::util::DeviceExt;
use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, Event, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};
mod data;
mod gpu_data;

struct GraphicsDevice {
    _instance: wgpu::Instance,
    surface: wgpu::Surface,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl GraphicsDevice {
    async fn new(window: &Window) -> Self {
        let instance = wgpu::Instance::default();

        let surface = unsafe { instance.create_surface(&window) }.unwrap();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                // Request an adapter which can render to our surface
                compatible_surface: Some(&surface),
            })
            .await
            .expect("Failed to find an appropriate adapter");

        // Create the logical device and command queue
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::default(),
                    // Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
                    // limits: wgpu::Limits::downlevel_webgl2_defaults()
                    //     .using_resolution(adapter.limits()),
                    limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .expect("Failed to create device");

        Self {
            _instance: instance,
            surface,
            adapter,
            device,
            queue,
        }
    }

    fn load_texture(&self, path: &str) -> wgpu::Texture {
        let image = image::io::Reader::open(path).unwrap().decode().unwrap();
        let image = image.to_rgba8();
        let dimensions = image.dimensions();

        let texture_size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            label: Some(path),
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &image,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * dimensions.0),
                rows_per_image: Some(dimensions.1),
            },
            texture_size,
        );
        texture
    }
}

struct Ui {
    context: egui::Context,
    state: egui_winit::State,
    screen_descriptor: egui_wgpu::renderer::ScreenDescriptor,
}

impl Ui {
    fn new(window: &Window) -> Self {
        let size = window.inner_size();
        Self {
            context: Default::default(),
            state: egui_winit::State::new(window),
            screen_descriptor: egui_wgpu::renderer::ScreenDescriptor {
                size_in_pixels: [size.width, size.height],
                pixels_per_point: 1.0,
            },
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.screen_descriptor.size_in_pixels = [width, height];
    }

    fn on_event(&mut self, event: &WindowEvent) -> egui_winit::EventResponse {
        self.state.on_event(&self.context, event)
    }

    fn run(&mut self, window: &Window, run_ui: impl FnOnce(&egui::Context)) -> egui::FullOutput {
        self.context.run(self.state.take_egui_input(window), run_ui)
    }
}

struct Graphics {
    graphics_device: GraphicsDevice,
    _pipeline_layout: wgpu::PipelineLayout,
    _swapchain_capabilities: wgpu::SurfaceCapabilities,
    _swapchain_format: wgpu::TextureFormat,
    render_pipeline: wgpu::RenderPipeline,
    surface_config: wgpu::SurfaceConfiguration,

    egui_renderer: egui_wgpu::Renderer,
}

impl Graphics {
    fn new(
        window: &Window,
        graphics_device: GraphicsDevice,
        bind_group_layouts: &[wgpu::BindGroupLayout],
    ) -> Self {
        let size = window.inner_size();

        // Load the shaders from disk
        let vertex_shader =
            graphics_device
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("vertex_shader"),
                    source: wgpu::ShaderSource::Glsl {
                        shader: Cow::Borrowed(include_str!("shader.vert")),
                        stage: naga::ShaderStage::Vertex,
                        defines: Default::default(),
                    },
                });
        let fragment_shader =
            graphics_device
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
        let pipeline_layout =
            graphics_device
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: None,
                    bind_group_layouts: &bind_group_layouts,
                    push_constant_ranges: &[],
                });

        let swapchain_capabilities = graphics_device
            .surface
            .get_capabilities(&graphics_device.adapter);
        let swapchain_format = swapchain_capabilities.formats[0];

        let render_pipeline =
            graphics_device
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

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: swapchain_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: swapchain_capabilities.alpha_modes[0],
            view_formats: vec![],
        };

        graphics_device
            .surface
            .configure(&graphics_device.device, &surface_config);

        let egui_renderer =
            egui_wgpu::Renderer::new(&graphics_device.device, swapchain_format, None, 1);

        Graphics {
            graphics_device,
            _pipeline_layout: pipeline_layout,
            _swapchain_capabilities: swapchain_capabilities,
            _swapchain_format: swapchain_format,
            render_pipeline,
            surface_config,
            egui_renderer,
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.graphics_device
            .surface
            .configure(&self.graphics_device.device, &self.surface_config);
    }

    fn redraw(
        &mut self,
        bind_groups: &[wgpu::BindGroup],
        ui: &mut Ui,
        full_output: egui::FullOutput,
    ) {
        // create encoder
        // # run refresh ui
        // take input
        // run ui
        // tesselate
        // for textures in texture_delta.set
        //   renderer.update_texture
        // cms_buffs = update_buffers
        // encoder begin renderpass
        // egui-renderer.render
        // for texture in textures_delta.free
        //  free_texture
        //
        // get frame and view
        // create render pass
        // run draw
        // run `compuse_by_pass`
        // submit cms bufs chained encider.finiosh()
        // frame present

        // Prepare frame resources.
        let frame = self
            .graphics_device
            .surface
            .get_current_texture()
            .expect("Failed to acquire next swapchain texture");
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .graphics_device
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        // Upload egui data.
        let egui_paint_jobs = ui.context.tessellate(full_output.shapes);
        let texture_sets = full_output.textures_delta.set;
        for (id, image_delta) in texture_sets {
            self.egui_renderer.update_texture(
                &self.graphics_device.device,
                &self.graphics_device.queue,
                id,
                &image_delta,
            );
        }

        let mut command_buffers = self.egui_renderer.update_buffers(
            &self.graphics_device.device,
            &self.graphics_device.queue,
            &mut encoder,
            &egui_paint_jobs,
            &ui.screen_descriptor,
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
            render_pass.set_pipeline(&self.render_pipeline);
            for (index, bind_group) in bind_groups.iter().enumerate() {
                render_pass.set_bind_group(index.try_into().unwrap(), bind_group, &[]);
            }
            render_pass.draw(0..4, 0..1);

            self.egui_renderer
                .render(&mut render_pass, &egui_paint_jobs, &ui.screen_descriptor);
        }

        let texture_frees = full_output.textures_delta.free;
        for id in texture_frees {
            self.egui_renderer.free_texture(&id);
        }

        command_buffers.push(encoder.finish());

        self.graphics_device.queue.submit(command_buffers);
        frame.present();
    }
}

struct App {
    program_start: SystemTime,

    tiles: Vec<Tile>,
    quadrants: [Vec<Option<usize>>; 4],

    sizes_buffer: wgpu::Buffer,
    quadrant_buffers: [wgpu::Buffer; 4],

    view_buffer_size: u64,
    view_buffer: wgpu::Buffer,

    bind_group_layouts: Vec<wgpu::BindGroupLayout>,
    bind_groups: Vec<wgpu::BindGroup>,

    mouse_position: (f32, f32),
    grab_move: bool,
    grab_rotate: bool,

    size: (u32, u32),
    origin: (f32, f32),
    rotation: f32,
    inv_scale: f32,

    coloring: i32,
}

impl App {
    fn assign_groups(tiles: &mut Vec<Tile>, quadrants: &[Vec<Option<usize>>; 4]) {
        // Assign groups.
        let mut groups = HashMap::<usize, HashSet<(i32, i32, usize)>>::default();
        let mut next_group_index = 1;
        let mut processed = HashSet::<(i32, i32)>::default();
        let mut queue = VecDeque::from([(0, 0)]);

        let tile_index = |s, t| {
            let quadrant = &quadrants[Tile::quadrant_of(s, t)];
            let index = Tile::index_of(s, t);
            if index < quadrant.len() {
                quadrant[index]
            } else {
                None
            }
        };

        while !queue.is_empty() {
            let (s, t) = queue.pop_front().unwrap();
            let tile = match tile_index(s, t) {
                Some(tile) => tile,
                None => {
                    continue;
                }
            };

            // Enqueue neighbors.
            for rotation in 0..6 {
                let (ns, nt) = tiles[tile].neighbor_coordinates(rotation);
                if !processed.contains(&(ns, nt)) {
                    processed.insert((ns, nt));
                    queue.push_back((ns, nt));
                }
            }

            // For each segment, aka each separate part of a tile.
            for segment_index in 0..tiles[tile].segments.len() {
                // Collect neighbor group ids.
                let mut group_ids = HashSet::<usize>::default();
                {
                    let segment = &tiles[tile].segments[segment_index];
                    if segment.group != 0 {
                        continue;
                    }

                    for rotation in segment.rotations().into_iter() {
                        let (ns, nt) = tiles[tile].neighbor_coordinates(rotation);
                        let neighbor = match tile_index(ns, nt) {
                            Some(tile) => tile,
                            None => {
                                continue;
                            }
                        };

                        let opposite_side = (rotation + 3) % 6;
                        let neighbor_segment = match tiles[neighbor].segment_at(opposite_side) {
                            Some(segment) => segment,
                            None => {
                                continue;
                            }
                        };

                        if neighbor_segment.terrain == segment.terrain {
                            group_ids.insert(neighbor_segment.group);
                        }
                    }
                }

                // TODO why can this happen?
                group_ids.remove(&0);

                // Create, assign and merge.
                if group_ids.is_empty() {
                    tiles[tile].segments[segment_index].group = next_group_index;
                    groups.insert(next_group_index, HashSet::from([(s, t, segment_index)]));
                    next_group_index += 1;
                } else if group_ids.len() == 1 {
                    let id = group_ids.into_iter().next().unwrap();
                    tiles[tile].segments[segment_index].group = id;
                    groups.get_mut(&id).unwrap().insert((s, t, segment_index));
                } else {
                    let min_id = group_ids.iter().fold(usize::max_value(), |a, b| a.min(*b));
                    tiles[tile].segments[segment_index].group = min_id;
                    group_ids.remove(&min_id);
                    groups
                        .get_mut(&min_id)
                        .unwrap()
                        .insert((s, t, segment_index));

                    for other_id in group_ids.into_iter() {
                        for (other_s, other_t, other_index) in
                            groups.remove(&other_id).unwrap().into_iter()
                        {
                            let other_tile = match tile_index(other_s, other_t) {
                                Some(tile) => tile,
                                None => {
                                    continue;
                                }
                            };

                            tiles[other_tile].segments[other_index].group = min_id;
                            groups.get_mut(&min_id).unwrap().insert((
                                other_s,
                                other_t,
                                other_index,
                            ));
                        }
                    }
                }
            }
        }
    }

    fn new(window: &Window, graphics_device: &GraphicsDevice) -> Self {
        let forest_texture = graphics_device.load_texture("seamless-forest.jpg");
        let forest_view = forest_texture.create_view(&Default::default());
        let city_texture = graphics_device.load_texture("seamless-city.jpg");
        let city_view = city_texture.create_view(&Default::default());
        let water_texture = graphics_device.load_texture("seamless-water.jpg");
        let water_view = water_texture.create_view(&Default::default());
        let wheat_texture = graphics_device.load_texture("seamless-wheat.jpg");
        let wheat_view = wheat_texture.create_view(&Default::default());

        let texture_sampler = graphics_device
            .device
            .create_sampler(&wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: wgpu::AddressMode::Repeat,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Nearest,
                mipmap_filter: wgpu::FilterMode::Nearest,
                ..Default::default()
            });

        // Load savegame.
        let mut stream = File::open("dorfromantik.dump").expect("Failed to open file");
        // let mut stream = File::open("mini.sav").expect("Failed to open file");
        let parsed = nrbf_rs::parse_nrbf(&mut stream);
        let savegame = data::SaveGame::try_from(&parsed).unwrap();
        let empty_tile = Tile {
            s: 0,
            t: 0,
            special: 0,
            segments: vec![],
        };
        let mut tiles = iter::once(empty_tile)
            .chain(savegame.tiles.iter().map(Tile::from))
            .collect::<Vec<_>>();

        // Group into quadrants for indexing.
        let mut quadrants = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        for (index, tile) in tiles.iter().enumerate() {
            let quadrant = &mut quadrants[tile.quadrant()];
            let t_index = tile.index();
            if quadrant.len() < t_index + 1 {
                quadrant.resize(t_index + 1, None);
            }
            quadrant[t_index] = Some(index);
        }

        Self::assign_groups(&mut tiles, &quadrants);

        let quadrant_sizes = [0, 1, 2, 3].map(|i| u32::try_from(quadrants[i].len()).unwrap());

        // Create sizes buffer with quadrant sizes.
        let sizes_buffer =
            graphics_device
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("sizes"),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    contents: bytemuck::cast_slice(&quadrant_sizes),
                });

        // Create quadrants themselves.
        let create_quadrant_buffer = |index: usize, tile_count: u32| {
            graphics_device
                .device
                .create_buffer(&wgpu::BufferDescriptor {
                    label: Some(&format!("quadrant_buffer_{index}")),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    size: (tile_count * u32::try_from(gpu_data::TILE_).unwrap()).into(),
                    mapped_at_creation: false,
                })
        };
        let quadrant_buffers = [
            create_quadrant_buffer(0, quadrant_sizes[0]),
            create_quadrant_buffer(1, quadrant_sizes[1]),
            create_quadrant_buffer(2, quadrant_sizes[2]),
            create_quadrant_buffer(3, quadrant_sizes[3]),
        ];

        for ((quadrant, &quadrant_size), buffer) in quadrants
            .iter()
            .zip(quadrant_sizes.iter())
            .zip(quadrant_buffers.iter())
        {
            let buffer_size = u64::from(quadrant_size) * u64::try_from(gpu_data::TILE_).unwrap();
            let mut buffer_view = graphics_device
                .queue
                .write_buffer_with(buffer, 0, buffer_size.try_into().unwrap())
                .expect("Failed to create buffer view");

            unsafe {
                let ptr = buffer_view.as_mut_ptr();
                for (index, tile) in quadrant.iter().enumerate() {
                    let tile_ptr = ptr.add(index * gpu_data::TILE_).cast::<i32>();

                    if let Some(tile_index) = tile {
                        let tile = &tiles[*tile_index];
                        *tile_ptr = 1;
                        *tile_ptr.add(1) = tile.special;

                        let segments_ptr = tile_ptr.add(4);
                        for (index, segment) in tile.segments.iter().enumerate() {
                            let segment_ptr = segments_ptr.add(index * 4);
                            *segment_ptr.add(0) = segment.terrain as i32;
                            *segment_ptr.add(1) = segment.form as i32;
                            *segment_ptr.add(2) = segment.rotation;
                            *segment_ptr.add(3) = segment.group as i32;
                        }
                        for index in tile.segments.len()..6 {
                            let segment_ptr = segments_ptr.add(index * 4);
                            // Mark empty.
                            *segment_ptr.add(0) = 0;
                        }
                    } else {
                        *tile_ptr = 0;
                    }
                }
            }
        }

        // Create view buffer.
        let view_buffer_size = (
            // Size
            1 * gpu_data::IVEC2_
            // Origin
            + 1 * gpu_data::VEC2_
            // Rotation
            + 1 * gpu_data::FLOAT_
             // InvScale
            + 1 * gpu_data::FLOAT_
            // Time
            + 1 * gpu_data::FLOAT_
            // Color bu group
            + 1 * gpu_data::BOOL_
        )
            .try_into()
            .unwrap();
        let view_buffer = graphics_device
            .device
            .create_buffer(&wgpu::BufferDescriptor {
                label: Some("view"),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                size: view_buffer_size,
                mapped_at_creation: false,
            });

        // Create bind group layout.
        let layout_entry = |binding, stage, binding_type| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: stage,
            ty: binding_type,
            count: None,
        };

        let fragment = wgpu::ShaderStages::FRAGMENT;
        let uniform_type = wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        };
        let storage_type = wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        };
        let sampler_type = wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering);
        let texture_type = wgpu::BindingType::Texture {
            multisampled: false,
            view_dimension: wgpu::TextureViewDimension::D2,
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
        };

        let bind_group_layout_entries = [
            layout_entry(0, fragment, uniform_type),
            layout_entry(1, fragment, storage_type),
            layout_entry(2, fragment, storage_type),
            layout_entry(3, fragment, storage_type),
            layout_entry(4, fragment, storage_type),
            layout_entry(5, fragment, uniform_type),
            layout_entry(6, fragment, sampler_type),
            layout_entry(7, fragment, texture_type),
            layout_entry(8, fragment, texture_type),
            layout_entry(9, fragment, texture_type),
            layout_entry(10, fragment, texture_type),
        ];
        let bind_group_layout =
            graphics_device
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    entries: &bind_group_layout_entries,
                    label: Some("bind_group_layout"),
                });

        // Create actual bind group.
        let bind_group_entries = [
            sizes_buffer.as_entire_binding(),
            quadrant_buffers[0].as_entire_binding(),
            quadrant_buffers[1].as_entire_binding(),
            quadrant_buffers[2].as_entire_binding(),
            quadrant_buffers[3].as_entire_binding(),
            view_buffer.as_entire_binding(),
            wgpu::BindingResource::Sampler(&texture_sampler),
            wgpu::BindingResource::TextureView(&forest_view),
            wgpu::BindingResource::TextureView(&city_view),
            wgpu::BindingResource::TextureView(&wheat_view),
            wgpu::BindingResource::TextureView(&water_view),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, resource)| wgpu::BindGroupEntry {
            binding: index as u32,
            resource,
        })
        .collect::<Vec<_>>();

        let bind_group = graphics_device
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &bind_group_layout,
                entries: &bind_group_entries,
                label: Some("bind_group"),
            });

        let size = window.inner_size();
        let size = (size.width, size.height);

        Self {
            program_start: SystemTime::now(),
            tiles,
            quadrants,
            sizes_buffer,
            quadrant_buffers,
            view_buffer_size,
            view_buffer,
            bind_group_layouts: vec![bind_group_layout],
            bind_groups: vec![bind_group],
            mouse_position: (0.0, 0.0),
            grab_move: false,
            grab_rotate: false,
            size,
            origin: (0.0, 0.0),
            rotation: 0.0,
            inv_scale: 20.0,
            coloring: 0,
        }
    }

    fn elapsed_secs(&self) -> f32 {
        SystemTime::now()
            .duration_since(self.program_start)
            .unwrap()
            .as_secs_f32()
    }

    fn bind_group_layouts(&self) -> &[wgpu::BindGroupLayout] {
        &self.bind_group_layouts
    }

    fn bind_groups(&self) -> &[wgpu::BindGroup] {
        &self.bind_groups
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.size = (width, height);
    }

    fn write_view(&self, graphics_device: &GraphicsDevice) {
        let view_buffer_size = self.view_buffer_size.try_into().unwrap();
        let mut buffer_view = graphics_device
            .queue
            .write_buffer_with(&self.view_buffer, 0, view_buffer_size)
            .expect("Failed to create buffer view");

        unsafe {
            let ptr = buffer_view.as_mut_ptr();
            let uptr = ptr.cast::<u32>();
            *uptr.add(0) = self.size.0;
            *uptr.add(1) = self.size.1;
            let fptr = uptr.add(2).cast::<f32>();
            *fptr.add(0) = self.origin.0;
            *fptr.add(1) = self.origin.1;
            *fptr.add(2) = self.rotation;
            *fptr.add(3) = self.inv_scale;
            *fptr.add(4) = self.elapsed_secs();
            let iptr = fptr.add(5).cast::<i32>();
            *iptr.add(0) = self.coloring;
        }
    }
}

fn run(
    event_loop: EventLoop<()>,
    window: Window,
    mut graphics: Graphics,
    mut ui: Ui,
    mut app: App,
) {
    let mut sidebar_expanded = false;
    event_loop.run(move |event, _, control_flow| {
        // What the actual??
        // Have the closure take ownership of the resources.
        // `event_loop.run` never returns, therefore we must do this to ensure
        // the resources are properly cleaned up.
        // let _ = (&instance, &adapter, &shader, &pipeline_layout);

        *control_flow = ControlFlow::Poll;

        match event {
            Event::WindowEvent { event, .. } => {
                let event_response = ui.on_event(&event);

                if event_response.repaint {
                    window.request_redraw();
                }

                if event_response.consumed {
                    return;
                }

                match event {
                    // WindowEvent::CursorMoved { position, .. } => {}
                    WindowEvent::MouseInput { button, state, .. } => {
                        match (button, state) {
                            (MouseButton::Left, ElementState::Pressed) => {
                                app.grab_move = true;
                            }
                            (MouseButton::Left, ElementState::Released) => {
                                app.grab_move = false;
                            }
                            (MouseButton::Right, ElementState::Pressed) => {
                                app.grab_rotate = true;
                            }
                            (MouseButton::Right, ElementState::Released) => {
                                app.grab_rotate = false;
                            }
                            _ => {}
                        }

                        // Lock the mouse so that we can't leave the window while dragging and
                        // enter a crooked button state.
                        let grab_mode = if !app.grab_move && !app.grab_rotate {
                            winit::window::CursorGrabMode::None
                        } else {
                            winit::window::CursorGrabMode::Confined
                        };
                        window.set_cursor_grab(grab_mode).unwrap();
                    }
                    WindowEvent::CursorMoved {
                        position: PhysicalPosition { x, y },
                        ..
                    } => {
                        let x = x as f32;
                        let y = y as f32;
                        let dx = (x - app.mouse_position.0) / app.size.0 as f32;
                        let dy = (y - app.mouse_position.1) / app.size.1 as f32;

                        if app.grab_move {
                            let aspect_ratio = app.size.0 as f32 / app.size.1 as f32;
                            app.origin = (
                                app.origin.0 - dx * aspect_ratio * app.inv_scale,
                                app.origin.1 + dy * app.inv_scale,
                            );
                        }

                        if app.grab_rotate {
                            app.rotation += dx;
                        }

                        app.mouse_position = (x, y);
                    }
                    WindowEvent::MouseWheel {
                        delta: MouseScrollDelta::LineDelta(_, y),
                        ..
                    } => {
                        app.inv_scale = 5f32.max(app.inv_scale - y as f32).min(500.0);
                    }
                    WindowEvent::CloseRequested => {
                        *control_flow = ControlFlow::Exit;
                    }
                    WindowEvent::Resized(size) => {
                        // Window has been resized. Adjust render pipeline settings.
                        graphics.resize(size.width, size.height);
                        ui.resize(size.width, size.height);
                        app.resize(size.width, size.height);

                        // On macos the window needs to be redrawn manually after resizing
                        window.request_redraw();
                    }
                    _ => {}
                }
            }
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            Event::RedrawRequested(_) => {
                let full_output = ui.run(&window, |ctx| {
                    // egui::CentralPanel::default()
                    //     .frame(egui::Frame::none().fill(egui::Color32::TRANSPARENT))
                    egui::TopBottomPanel::top("top_panel").show(&ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Dorfromantik viewer");
                            ui.toggle_value(&mut sidebar_expanded, "Visual settings");
                        });
                    });
                    egui::SidePanel::left("left_panel").show_animated(
                        &ctx,
                        sidebar_expanded,
                        |ui| {
                            ui.label("Visual settings");
                            ui.add(
                                egui::Slider::new(&mut app.inv_scale, 5.0..=500.0).text("Zoom out"),
                            );
                            ui.label("Coloring of sections:");
                            ui.selectable_value(&mut app.coloring, 0, "Color by terrain type");
                            ui.selectable_value(&mut app.coloring, 1, "Color by group statically");
                            ui.selectable_value(&mut app.coloring, 2, "Color by group dynamically");
                            ui.selectable_value(&mut app.coloring, 3, "Color by texture");
                        },
                    );
                    egui::Window::new("Select file").show(&ctx, |ui| {
                        ui.label("Select file");
                    });
                });

                // let egui_paint_jobs = ui.context.tessellate(full_output.shapes);
                app.write_view(&graphics.graphics_device);
                graphics.redraw(app.bind_groups(), &mut ui, full_output);
            }
            _ => {}
        }
    });
}

fn main() {
    let event_loop = EventLoop::new();
    let window = winit::window::Window::new(&event_loop).unwrap();
    let graphics_device = pollster::block_on(GraphicsDevice::new(&window));
    let app = App::new(&window, &graphics_device);

    let graphics = Graphics::new(&window, graphics_device, app.bind_group_layouts());
    let ui = Ui::new(&window);

    #[cfg(not(target_arch = "wasm32"))]
    {
        env_logger::init();
        run(event_loop, window, graphics, ui, app);
    }
    #[cfg(target_arch = "wasm32")]
    {
        // TODO
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
        console_log::init().expect("could not initialize logger");
        use winit::platform::web::WindowExtWebSys;
        // On wasm, append the canvas to the document body
        web_sys::window()
            .and_then(|win| win.document())
            .and_then(|doc| doc.body())
            .and_then(|body| {
                body.append_child(&web_sys::Element::from(window.canvas()))
                    .ok()
            })
            .expect("couldn't append canvas to document body");
        wasm_bindgen_futures::spawn_local(run(event_loop, window));
    }
}
