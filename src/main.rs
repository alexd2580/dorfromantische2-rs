use glam::{IVec2, UVec2, Vec2};
use gpu_data::Tile;
use std::{
    borrow::Cow,
    collections::{HashSet, VecDeque},
    fs::File,
    iter,
    thread::JoinHandle,
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

    fn upload_texture(
        &self,
        path: &str,
        image: image::ImageBuffer<image::Rgba<u8>, Vec<u8>>,
    ) -> wgpu::Texture {
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
        bind_groups: Option<&[wgpu::BindGroup]>,
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

            // Run shader.
            if let Some(bind_groups) = bind_groups {
                render_pass.set_pipeline(&self.render_pipeline);
                for (index, bind_group) in bind_groups.iter().enumerate() {
                    render_pass.set_bind_group(index.try_into().unwrap(), bind_group, &[]);
                }
                render_pass.draw(0..4, 0..1);
            }

            // Render GUI.
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

struct Group {
    // Tile/segment.
    segments: HashSet<(usize, usize)>,

    // Tile/rotation.
    open_edges: HashSet<(usize, i32)>,
}

struct GameData {
    tiles: Vec<Tile>,
    possible_placements: HashSet<IVec2>,
    groups: Vec<Group>,

    best_placements: Vec<IVec2>,

    index_offset: IVec2,
    index_size: IVec2,
    index: Vec<Option<usize>>,
}

impl Default for GameData {
    fn default() -> Self {
        let tiles = vec![Tile {
            pos: Default::default(),
            segments: vec![],
        }];

        let mut game_data = Self {
            tiles,
            possible_placements: Default::default(),
            groups: Default::default(),
            best_placements: Default::default(),
            index_offset: Default::default(),
            index_size: Default::default(),
            index: Default::default(),
        };
        game_data.recreate_index();
        game_data
    }
}

impl GameData {
    /// Compute the position of tile at `pos` in the index structure.
    fn index_position_of(&self, pos: IVec2) -> Option<usize> {
        let valid_s =
            pos.x >= self.index_offset.x && pos.x < self.index_offset.x + self.index_size.x;
        let valid_t =
            pos.y >= self.index_offset.y && pos.y < self.index_offset.y + self.index_size.y;
        (valid_s && valid_t).then(|| {
            usize::try_from(
                (pos.y - self.index_offset.y) * self.index_size.x + (pos.x - self.index_offset.x),
            )
            .unwrap()
        })
    }

    /// Compute the 2D bounding box and traverse it row-first (in row-major) order.
    fn recreate_index(&mut self) {
        let (min, max) = self.tiles.iter().fold(
            (
                IVec2::new(i32::MAX, i32::MAX),
                IVec2::new(i32::MIN, i32::MIN),
            ),
            |(min, max), tile| {
                (
                    IVec2::new(min.x.min(tile.pos.x), min.y.min(tile.pos.y)),
                    IVec2::new(max.x.max(tile.pos.x), max.y.max(tile.pos.y)),
                )
            },
        );

        self.index_offset = min;
        self.index_size = max - min + IVec2::new(1, 1);
        let mut index = vec![None; usize::try_from(self.index_size.x * self.index_size.y).unwrap()];

        self.tiles
            .iter()
            .enumerate()
            .for_each(|(tile_index, tile)| {
                index[self.index_position_of(tile.pos).unwrap()] = Some(tile_index);
            });

        self.index = index;
    }

    /// Compute the position of tile at `pos` in the tiles' list.
    fn tile_index(&self, pos: IVec2) -> Option<usize> {
        self.index_position_of(pos)
            .and_then(|index| self.index.get(index))
            .cloned()
            .flatten()
    }

    fn connected_groups(&self, tile: usize, segment: usize) -> HashSet<usize> {
        let segment = &self.tiles[tile].segments[segment];
        segment
            .rotations()
            .into_iter()
            .flat_map(|rotation| {
                let neighbor_pos = self.tiles[tile].neighbor_coordinates(rotation);
                let opposite_side = (rotation + 3) % 6;

                // Get neighbor tile at `rotation`.
                self.tile_index(neighbor_pos)
                    // Get its segment which is at the opposite side of `rotation`.
                    // Require that the terrain is the same.
                    .and_then(|neighbor| {
                        self.tiles[neighbor].connecting_segment_at(segment.terrain, opposite_side)
                    })
                    .into_iter()
                    // Get the group id.
                    .map(|neighbor_segment| neighbor_segment.group)
            })
            .collect::<HashSet<_>>()
    }

    fn assign_groups(&mut self) {
        // Assign groups.
        let mut groups = Vec::<HashSet<(usize, usize)>>::from([Default::default()]);
        let mut next_group_index = 1;
        let mut processed = HashSet::<usize>::default();
        let mut queue = VecDeque::from([0]);
        let mut open_tiles = HashSet::<IVec2>::default();

        // Process all tiles, breadth first.
        while !queue.is_empty() {
            let tile = queue.pop_front().unwrap();

            // Check if an index was processed and enqueue neighbor otherwise.
            for rotation in 0..6 {
                let pos = self.tiles[tile].neighbor_coordinates(rotation);
                if let Some(tile) = self.tile_index(pos) {
                    if !processed.contains(&tile) {
                        processed.insert(tile);
                        queue.push_back(tile);
                    }
                } else {
                    open_tiles.insert(pos);
                }
            }

            // For each segment, aka each separate part of a tile...
            for segment in 0..self.tiles[tile].segments.len() {
                if self.tiles[tile].segments[segment].group != 0 {
                    continue;
                }

                let mut group_ids = self.connected_groups(tile, segment);
                group_ids.remove(&0);

                // Choose the new group id from the collected ids.
                let group_id = if group_ids.is_empty() {
                    groups.push(Default::default());
                    next_group_index += 1;
                    next_group_index - 1
                } else if group_ids.len() == 1 {
                    group_ids.drain().next().unwrap()
                } else {
                    let min_id = group_ids.iter().fold(usize::max_value(), |a, b| a.min(*b));
                    group_ids.remove(&min_id);
                    min_id
                };

                // Assign the group to the current segment.
                self.tiles[tile].segments[segment].group = group_id;
                // Register the current segment with `group_id`.
                let mut group = std::mem::take(&mut groups[group_id]);
                group.insert((tile, segment));
                // Remap all connected groups to the chosen one (TODO Expensive!).
                for other_id in group_ids.into_iter() {
                    group.extend(groups[other_id].drain().inspect(|(tile, segment)| {
                        self.tiles[*tile].segments[*segment].group = group_id;
                    }));
                }
                groups[group_id] = group;
            }
        }

        let mut groups = groups
            .into_iter()
            .map(|segments| Group {
                segments,
                open_edges: Default::default(),
            })
            .collect::<Vec<_>>();

        open_tiles
            .iter()
            .flat_map(|pos| {
                (0..6).map(|rotation| {
                    (
                        Tile::neighbor_coordinates_of(*pos, rotation),
                        Tile::opposite_side(rotation),
                    )
                })
            })
            .filter_map(|(position, rotation)| {
                self.tile_index(position).map(|index| (index, rotation))
            })
            .flat_map(|(tile, rotation)| {
                self.tiles[tile]
                    .segments_at(rotation)
                    .map(move |segment| (tile, rotation, segment))
            })
            .for_each(|(tile, rotation, segment)| {
                groups[segment.group].open_edges.insert((tile, rotation));
            });

        self.possible_placements = open_tiles;
        self.groups = groups;
    }

    fn load_file(&mut self, path: &std::path::Path) {
        // Load savegame.
        let mut stream = File::open(path).expect("Failed to open file");
        let parsed = nrbf_rs::parse_nrbf(&mut stream);
        let savegame = data::SaveGame::try_from(&parsed).unwrap();

        // TODO here
        dbg!(Tile::from(&savegame.tile_stack[0]));

        // let mut quest_tile_ids = HashSet::<i32>::default();
        // let mut quest_ids = HashSet::<i32>::default();
        //
        // savegame.tiles.iter().filter(|tile| tile.quest_tile.is_some()).for_each(|tile| {
        //     let q = tile.quest_tile.as_ref().unwrap();
        //     quest_ids.insert(q.quest_id.0);
        //     quest_tile_ids.insert(q.quest_tile_id.0);
        // });
        //
        // dbg!(&quest_tile_ids);
        // dbg!(&quest_ids);

        // Prepend tiles list with empty tile (is this necessary when i start parsing special tiles?)
        let empty_tile = Tile {
            pos: IVec2::new(0, 0),
            segments: vec![],
        };
        self.tiles = iter::once(empty_tile)
            .chain(savegame.tiles.iter().map(Tile::from))
            .collect::<Vec<_>>();

        // Group into quadrants for indexing.
        self.recreate_index();
        self.assign_groups();
    }
}

struct GraphicsResources {
    // Textures.
    _forest_texture: wgpu::Texture,
    forest_view: wgpu::TextureView,
    _city_texture: wgpu::Texture,
    city_view: wgpu::TextureView,
    _wheat_texture: wgpu::Texture,
    wheat_view: wgpu::TextureView,
    _river_texture: wgpu::Texture,
    river_view: wgpu::TextureView,

    // Texture access.
    texture_sampler: wgpu::Sampler,

    // Generic info (changes every frame).
    view_buffer_size: u64,
    view_buffer: wgpu::Buffer,

    // Organized tiles list.
    tiles_buffer_size: u64,
    tiles_buffer: wgpu::Buffer,

    bind_group_layouts: Vec<wgpu::BindGroupLayout>,
    bind_groups: Vec<wgpu::BindGroup>,
}

enum SizeOrContent<'a> {
    Size(u64),
    _Content(&'a [u8]),
}

impl GraphicsResources {
    fn load_texture(path: &str, graphics_device: &GraphicsDevice) -> wgpu::Texture {
        let image = image::io::Reader::open(path).unwrap().decode().unwrap();
        let image = image.to_rgba8();
        graphics_device.upload_texture(path, image)
    }

    fn create_buffer(
        label: &str,
        usage: wgpu::BufferUsages,
        size_or_content: SizeOrContent,
        graphics_device: &GraphicsDevice,
    ) -> wgpu::Buffer {
        match size_or_content {
            SizeOrContent::Size(size) => {
                graphics_device
                    .device
                    .create_buffer(&wgpu::BufferDescriptor {
                        label: Some(label),
                        usage,
                        size,
                        mapped_at_creation: false,
                    })
            }
            SizeOrContent::_Content(contents) => {
                graphics_device
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some(label),
                        usage,
                        contents,
                    })
            }
        }
    }

    fn create_tiles_buffer(
        graphics_device: &GraphicsDevice,
        game_data: &GameData,
    ) -> (u64, wgpu::Buffer) {
        let tiles_buffer_size = u64::try_from(
            // Offset
            1 * gpu_data::IVEC2_
            // Size
            + 1 * gpu_data::IVEC2_
            // Tiles (at least one...)
            + game_data.index.len().max(1) * gpu_data::TILE_,
        )
        .unwrap();
        let tiles_buffer = Self::create_buffer(
            "tiles",
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            SizeOrContent::Size(tiles_buffer_size),
            graphics_device,
        );

        (tiles_buffer_size, tiles_buffer)
    }

    fn generate_bind_group(&mut self, graphics_device: &GraphicsDevice) {
        // Create actual bind group.
        let bind_group_entries = [
            (0, self.view_buffer.as_entire_binding()),
            (1, self.tiles_buffer.as_entire_binding()),
            (2, wgpu::BindingResource::Sampler(&self.texture_sampler)),
            (3, wgpu::BindingResource::TextureView(&self.forest_view)),
            (4, wgpu::BindingResource::TextureView(&self.city_view)),
            (5, wgpu::BindingResource::TextureView(&self.wheat_view)),
            (6, wgpu::BindingResource::TextureView(&self.river_view)),
        ]
        .map(|(binding, resource)| wgpu::BindGroupEntry { binding, resource });
        let bind_group = graphics_device
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &self.bind_group_layouts[0],
                entries: &bind_group_entries,
                label: Some("bind_group"),
            });

        self.bind_groups.push(bind_group);
    }

    fn new(graphics_device: &GraphicsDevice, game_data: &GameData) -> Self {
        // Textures tutorial:
        // https://sotrh.github.io/learn-wgpu/beginner/tutorial5-textures/#the-bindgroup
        let forest_texture = Self::load_texture("seamless-forest.jpg", graphics_device);
        let forest_view = forest_texture.create_view(&Default::default());
        let city_texture = Self::load_texture("seamless-city.jpg", graphics_device);
        let city_view = city_texture.create_view(&Default::default());
        let river_texture = Self::load_texture("seamless-river.jpg", graphics_device);
        let river_view = river_texture.create_view(&Default::default());
        let wheat_texture = Self::load_texture("seamless-wheat.jpg", graphics_device);
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

        let view_buffer_size = u64::try_from({
            // 2 int + 2 float
            let size = gpu_data::IVEC2_;
            let aspect_ratio = gpu_data::FLOAT_;
            let time = gpu_data::FLOAT_;

            // 4 float
            let origin = gpu_data::VEC2_;
            let rotation = gpu_data::FLOAT_;
            let inv_scale = gpu_data::FLOAT_;

            // 4 int
            let hover_pos = gpu_data::IVEC2_;
            let hover_rotation = gpu_data::INT_;
            let pad2 = gpu_data::PAD_;

            // 4 int
            let hover_tile = gpu_data::INT_;
            let hover_group = gpu_data::INT_;

            let coloring = gpu_data::INT_;
            let highlight_selected = gpu_data::INT_; // actually bool

            (size + aspect_ratio + time)
                + (origin + rotation + inv_scale)
                + (hover_pos + hover_rotation + pad2)
                + (hover_tile + hover_group + coloring + highlight_selected)
        })
        .unwrap();
        let view_buffer = Self::create_buffer(
            "view",
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            SizeOrContent::Size(view_buffer_size),
            graphics_device,
        );

        let (tiles_buffer_size, tiles_buffer) =
            Self::create_tiles_buffer(graphics_device, game_data);

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
            (0, uniform_type),
            (1, storage_type),
            (2, sampler_type),
            (3, texture_type),
            (4, texture_type),
            (5, texture_type),
            (6, texture_type),
        ]
        .map(|(binding, ty)| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty,
            count: None,
        });
        let bind_group_layout =
            graphics_device
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    entries: &bind_group_layout_entries,
                    label: Some("bind_group_layout"),
                });

        let mut graphics_resources = Self {
            _forest_texture: forest_texture,
            forest_view,
            _city_texture: city_texture,
            city_view,
            _wheat_texture: wheat_texture,
            wheat_view,
            _river_texture: river_texture,
            river_view,
            texture_sampler,
            view_buffer_size,
            view_buffer,
            tiles_buffer_size,
            tiles_buffer,
            bind_group_layouts: vec![bind_group_layout],
            bind_groups: vec![],
        };

        graphics_resources.generate_bind_group(graphics_device);
        graphics_resources
    }

    fn update(&mut self, graphics_device: &GraphicsDevice, game_data: &GameData) {
        let (tiles_buffer_size, tiles_buffer) =
            Self::create_tiles_buffer(graphics_device, game_data);

        self.tiles_buffer_size = tiles_buffer_size;
        self.tiles_buffer = tiles_buffer;

        self.bind_groups.clear();
        self.generate_bind_group(graphics_device);
    }
}

struct App {
    program_start: SystemTime,

    game_data: GameData,
    graphics_resources: GraphicsResources,

    mouse_position: Vec2,
    grab_move: bool,
    grab_rotate: bool,

    size: UVec2,
    aspect_ratio: f32,

    origin: Vec2,
    rotation: f32,
    inv_scale: f32,

    hover_pos: IVec2,
    hover_rotation: i32,

    hover_tile: Option<usize>,
    hover_group: Option<usize>,

    coloring: i32,
    highlight_hovered_group: bool,
    highlight_open_groups: bool,

    file_choose_dialog: Option<JoinHandle<Option<std::path::PathBuf>>>,
}

const SIN_30: f32 = 0.5;
const COS_30: f32 = 0.8660254037844387;

impl App {
    fn new(window: &Window, graphics_device: &GraphicsDevice) -> Self {
        // Load data
        // "dorfromantik.dump"
        let game_data = GameData::default();
        let graphics_resources = GraphicsResources::new(graphics_device, &game_data);

        let mut app = Self {
            program_start: SystemTime::now(),
            game_data,
            graphics_resources,
            mouse_position: Vec2::ZERO,
            grab_move: false,
            grab_rotate: false,
            size: UVec2::ZERO,
            aspect_ratio: 0.0,
            origin: Vec2::ZERO,
            rotation: 0.0,
            inv_scale: 20.0,
            coloring: 0,
            highlight_hovered_group: false,
            highlight_open_groups: true,
            hover_pos: IVec2::ZERO,
            hover_rotation: 0,
            hover_tile: None,
            hover_group: None,
            file_choose_dialog: None,
        };

        let size = window.inner_size();
        app.resize(UVec2::new(size.width, size.height));
        app.write_tiles(graphics_device);
        app
    }

    fn elapsed_secs(&self) -> f32 {
        SystemTime::now()
            .duration_since(self.program_start)
            .unwrap()
            .as_secs_f32()
    }

    fn bind_group_layouts(&self) -> &[wgpu::BindGroupLayout] {
        &self.graphics_resources.bind_group_layouts
    }

    fn bind_groups(&self) -> Option<&[wgpu::BindGroup]> {
        Some(&self.graphics_resources.bind_groups)
    }

    fn resize(&mut self, size: UVec2) {
        self.size = size;
        let fsize = size.as_vec2();
        self.aspect_ratio = fsize.x / fsize.y;
    }

    fn hex_to_world(pos: &IVec2) -> Vec2 {
        Vec2::new(pos.x as f32 * 1.5, (pos.x + pos.y * 2) as f32 * COS_30)
    }

    fn world_to_hex(mut pos: Vec2) -> IVec2 {
        let x = (pos.x / 1.5).round();
        let y_rest = pos.y - x * COS_30;
        let y = (y_rest / (2.0 * COS_30)).round();

        let prelim = IVec2::new(x as i32, y as i32);
        pos = pos - Self::hex_to_world(&prelim);
        let xc = (0.5 * Vec2::new(COS_30, SIN_30).dot(pos) / COS_30).round() as i32;
        let xyc = (0.5 * Vec2::new(-COS_30, SIN_30).dot(pos) / COS_30).round() as i32;

        prelim + IVec2::new(xc - xyc, xyc)
    }

    /// Compute world coordinates of pixel.
    fn pixel_to_world(&self, pos: Vec2) -> Vec2 {
        // First, get world-coordinates of pixel.
        let relative = pos / self.size.as_vec2();
        let uv_2 = Vec2::new(1.0, -1.0) * (relative - 0.5);
        self.origin + uv_2 * Vec2::new(self.aspect_ratio, 1.0) * self.inv_scale
    }

    fn on_cursor_move(&mut self, pos: Vec2) {
        let delta = (pos - self.mouse_position) / self.size.as_vec2();

        if self.grab_move {
            self.origin += Vec2::new(-1.0 * self.aspect_ratio, 1.0) * delta * self.inv_scale;
        }

        if self.grab_rotate {
            self.rotation += delta.x;
        }

        self.mouse_position = pos;
        let world_pos = self.pixel_to_world(pos);
        self.hover_pos = Self::world_to_hex(world_pos);
        let offset = world_pos - App::hex_to_world(&self.hover_pos);

        let gradient = 2.0 * COS_30 * offset.x;
        self.hover_rotation = match (offset.y > 0.0, offset.y > gradient, offset.y > -gradient) {
            (true, true, true) => 0,
            (true, false, _) => 1,
            (false, _, true) => 2,
            (false, false, false) => 3,
            (false, true, _) => 4,
            (true, _, false) => 5,
        };

        self.hover_tile = self.game_data.tile_index(self.hover_pos);
        self.hover_group = self
            .hover_tile
            .clone()
            .and_then(|tile| {
                self.game_data.tiles[tile]
                    .segments_at(self.hover_rotation)
                    .next()
            })
            .map(|segment| segment.group);
    }

    fn on_scroll(&mut self, y: f32) {
        self.inv_scale = 5f32.max(self.inv_scale - y as f32).min(500.0);
    }

    fn write_view(&self, graphics_device: &GraphicsDevice) {
        let view_buffer_size = self.graphics_resources.view_buffer_size.try_into().unwrap();
        let mut buffer_view = graphics_device
            .queue
            .write_buffer_with(&self.graphics_resources.view_buffer, 0, view_buffer_size)
            .expect("Failed to create buffer view");

        // 4 int

        unsafe {
            let ptr = buffer_view.as_mut_ptr();
            let uptr = ptr.cast::<u32>();
            *uptr.add(0) = self.size.x;
            *uptr.add(1) = self.size.y;
            let fptr = uptr.add(2).cast::<f32>();
            *fptr.add(0) = self.aspect_ratio;
            *fptr.add(1) = self.elapsed_secs();
            *fptr.add(2) = self.origin.x;
            *fptr.add(3) = self.origin.y;
            *fptr.add(4) = self.rotation;
            *fptr.add(5) = self.inv_scale;
            let iptr = fptr.add(6).cast::<i32>();
            *iptr.add(0) = self.hover_pos.x;
            *iptr.add(1) = self.hover_pos.y;
            *iptr.add(2) = self.hover_rotation;
            // pad
            *iptr.add(4) = self.hover_tile.map_or(-1, |v| v.try_into().unwrap());
            *iptr.add(5) = self.hover_group.map_or(-1, |v| v.try_into().unwrap());
            *iptr.add(6) = self.coloring;

            let highlight_flags = if self.highlight_hovered_group { 1 } else { 0 }
                | if self.highlight_open_groups { 2 } else { 0 };

            *iptr.add(7) = highlight_flags;
        }
    }

    fn write_tiles(&self, graphics_device: &GraphicsDevice) {
        let view_buffer_size = self
            .graphics_resources
            .tiles_buffer_size
            .try_into()
            .unwrap();
        let mut buffer_view = graphics_device
            .queue
            .write_buffer_with(&self.graphics_resources.tiles_buffer, 0, view_buffer_size)
            .expect("Failed to create buffer view");

        unsafe {
            let ptr = buffer_view.as_mut_ptr();
            let iptr = ptr.cast::<i32>();
            *iptr.add(0) = self.game_data.index_offset.x;
            *iptr.add(1) = self.game_data.index_offset.y;
            *iptr.add(2) = self.game_data.index_size.x;
            *iptr.add(3) = self.game_data.index_size.y;
            let bptr = iptr.add(4).cast::<u8>();

            for (index, tile) in self.game_data.index.iter().enumerate() {
                let mut tptr = bptr.add(index * gpu_data::TILE_).cast::<i32>();

                if let Some(index) = tile {
                    let tile = &self.game_data.tiles[*index];

                    for segment in tile.segments.iter() {
                        *tptr.add(0) = segment.terrain as i32;
                        *tptr.add(1) = segment.form as i32;
                        *tptr.add(2) = segment.rotation;

                        let group = segment.group;
                        let is_closed = self.game_data.groups[group].open_edges.is_empty();
                        let group_bytes = group as i32 | if is_closed { 2 << 30 } else { 0 };

                        *tptr.add(3) = group_bytes;
                        tptr = tptr.add(4);
                    }
                    if tile.segments.len() < 6 {
                        *tptr.add(0) = gpu_data::Terrain::Empty as i32
                    }
                } else {
                    *tptr = gpu_data::Terrain::Missing as i32;
                }
            }
        }
    }

    fn is_file_dialog_open(&self) -> bool {
        self.file_choose_dialog.is_some()
    }

    fn open_file_dialog(&mut self) {
        if !self.is_file_dialog_open() {
            self.file_choose_dialog = Some(std::thread::spawn(|| {
                rfd::FileDialog::new().set_directory(".").pick_file()
            }))
        }
    }

    fn handle_file_dialog(&mut self, graphics_device: &GraphicsDevice) {
        if self
            .file_choose_dialog
            .as_ref()
            .is_some_and(|handle| handle.is_finished())
        {
            let maybe_file = self
                .file_choose_dialog
                .take()
                .unwrap()
                .join()
                .expect("Failed to choose file");
            if let Some(file) = maybe_file {
                self.game_data.load_file(&file);
                self.graphics_resources
                    .update(graphics_device, &self.game_data);
                self.write_tiles(graphics_device);
            }
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
    let mut show_tooltip = true;
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
                    // window.request_redraw();
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
                    } => app.on_cursor_move(Vec2::new(x as f32, y as f32)),
                    WindowEvent::MouseWheel {
                        delta: MouseScrollDelta::LineDelta(_, y),
                        ..
                    } => app.on_scroll(y as f32),
                    WindowEvent::CloseRequested => {
                        *control_flow = ControlFlow::Exit;
                    }
                    WindowEvent::Resized(size) => {
                        // Window has been resized. Adjust render pipeline settings.
                        graphics.resize(size.width, size.height);
                        ui.resize(size.width, size.height);
                        app.resize(UVec2::new(size.width, size.height));

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
                            if ui
                                .add_enabled(
                                    !app.is_file_dialog_open(),
                                    egui::Button::new("Load file"),
                                )
                                .clicked()
                            {
                                app.open_file_dialog();
                            }
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
                            ui.checkbox(&mut show_tooltip, "Show tooltip");
                            ui.label("Coloring of sections:");
                            ui.selectable_value(&mut app.coloring, 0, "Color by terrain type");
                            ui.selectable_value(&mut app.coloring, 1, "Color by group statically");
                            ui.selectable_value(&mut app.coloring, 2, "Color by group dynamically");
                            ui.selectable_value(&mut app.coloring, 3, "Color by texture");
                            ui.checkbox(
                                &mut app.highlight_hovered_group,
                                "Highlight hovered group",
                            );
                            ui.checkbox(&mut app.highlight_open_groups, "Highlight open groups");
                        },
                    );
                    if show_tooltip {
                        if let Some(tile_index) = app.hover_tile {
                            let tile = &app.game_data.tiles[tile_index];
                            let response = egui::Window::new(format!("Tile {tile_index}"))
                                .movable(false)
                                .collapsible(false)
                                .resizable(false)
                                .current_pos((
                                    app.mouse_position.x + 50.0,
                                    app.mouse_position.y - 50.0,
                                ))
                                .show(&ctx, |ui| {
                                    egui::Grid::new("tile_data").show(ui, |ui| {
                                        ui.label("Axial position");
                                        ui.label(format!(
                                            "x: {}, y: {}",
                                            app.hover_pos.x, app.hover_pos.y
                                        ));
                                        ui.end_row();

                                        if !tile.segments.is_empty() {
                                            ui.label("Segments");
                                            ui.end_row();

                                            for segment in &tile.segments {
                                                ui.label("Terrain");
                                                ui.label(format!(
                                                    "{:?} {:?}",
                                                    segment.terrain, segment.form
                                                ));
                                                ui.end_row();

                                                ui.label("Group");
                                                ui.label(format!("{}", segment.group));
                                                ui.end_row();
                                            }
                                        }
                                    });
                                });

                            if let Some(group_index) = app.hover_group {
                                let group = &app.game_data.groups[group_index];
                                let tile_rect = response.unwrap().response.rect;
                                let pos = (tile_rect.min.x, tile_rect.max.y + 10.0);
                                egui::Window::new(format!("Group {group_index}"))
                                    .movable(false)
                                    .collapsible(false)
                                    .resizable(false)
                                    .current_pos(pos)
                                    .show(&ctx, |ui| {
                                        egui::Grid::new("group_data").show(ui, |ui| {
                                            ui.label("Segment count");
                                            ui.label(format!("{}", group.segments.len()));
                                            ui.end_row();
                                            ui.label("Closed");
                                            ui.label(if group.open_edges.is_empty() {
                                                "Yes"
                                            } else {
                                                "No"
                                            });
                                            ui.end_row();
                                        });
                                    });
                            }
                        }
                    }
                });

                app.handle_file_dialog(&graphics.graphics_device);
                app.write_view(&graphics.graphics_device);
                graphics.redraw(app.bind_groups(), &mut ui, full_output);
            }
            _ => {}
        }
    });
}

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new();
    let window = winit::window::Window::new(&event_loop).unwrap();
    let graphics_device = pollster::block_on(GraphicsDevice::new(&window));
    let app = App::new(&window, &graphics_device);
    let graphics = Graphics::new(&window, graphics_device, app.bind_group_layouts());
    let ui = Ui::new(&window);

    run(event_loop, window, graphics, ui, app);
}
