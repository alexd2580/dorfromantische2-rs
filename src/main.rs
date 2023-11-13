use data::{Rotation, TileId};
use glam::{IVec2, UVec2, Vec2};
use gpu::{Buffer, Gpu, SizeOrContent};
use map::{GroupId, Map};
use std::{borrow::Cow, fs::File, path::PathBuf, thread::JoinHandle, time::SystemTime};
use textures::Textures;
use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, Event, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};
mod data;
mod gpu;
mod index;
mod map;
mod raw_data;
mod textures;

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

struct Pipeline {
    _pipeline_layout: wgpu::PipelineLayout,
    render_pipeline: wgpu::RenderPipeline,

    egui_renderer: egui_wgpu::Renderer,
}

impl Pipeline {
    fn new(gpu: &Gpu, bind_group_layouts: &[wgpu::BindGroupLayout]) -> Self {
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

        Pipeline {
            _pipeline_layout: pipeline_layout,
            render_pipeline,
            egui_renderer,
        }
    }

    fn redraw(
        &mut self,
        gpu: &Gpu,
        bind_groups: Option<&[wgpu::BindGroup]>,
        ui: &mut Ui,
        full_output: egui::FullOutput,
    ) {
        // Prepare frame resources.
        let (frame, view) = gpu.get_current_texture();
        let mut encoder = gpu.create_encoder();

        // Upload egui data.
        let egui_paint_jobs = ui.context.tessellate(full_output.shapes);
        let texture_sets = full_output.textures_delta.set;

        for (id, image_delta) in texture_sets {
            self.egui_renderer
                .update_texture(&gpu.device, &gpu.queue, id, &image_delta);
        }

        let mut command_buffers = self.egui_renderer.update_buffers(
            &gpu.device,
            &gpu.queue,
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

        gpu.queue.submit(command_buffers);
        frame.present();
    }
}

struct BindGroups {
    layouts: [wgpu::BindGroupLayout; 1],
    groups: Option<[wgpu::BindGroup; 1]>,
}

impl BindGroups {
    const UNIFORM: wgpu::BindingType = wgpu::BindingType::Buffer {
        ty: wgpu::BufferBindingType::Uniform,
        has_dynamic_offset: false,
        min_binding_size: None,
    };
    const STORAGE: wgpu::BindingType = wgpu::BindingType::Buffer {
        ty: wgpu::BufferBindingType::Storage { read_only: true },
        has_dynamic_offset: false,
        min_binding_size: None,
    };
    const SAMPLER: wgpu::BindingType =
        wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering);
    const TEXTURE: wgpu::BindingType = wgpu::BindingType::Texture {
        multisampled: false,
        view_dimension: wgpu::TextureViewDimension::D2,
        sample_type: wgpu::TextureSampleType::Float { filterable: true },
    };

    fn new(gpu: &Gpu, entries: &[(u32, wgpu::BindingType)]) -> BindGroups {
        let entries = entries
            .iter()
            .cloned()
            .map(|(binding, ty)| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty,
                count: None,
            })
            .collect::<Vec<_>>();
        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &entries,
                label: Some("bind_group_layout"),
            });

        Self {
            layouts: [layout],
            groups: Default::default(),
        }
    }

    fn generate_bind_groups(&mut self, gpu: &Gpu, entries: &[(u32, wgpu::BindingResource)]) {
        let entries = entries
            .iter()
            .cloned()
            .map(|(binding, resource)| wgpu::BindGroupEntry { binding, resource })
            .collect::<Vec<_>>();
        self.groups = Some([gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.layouts[0],
            entries: &entries,
            label: Some("bind_group"),
        })]);
    }
}

#[derive(Default)]
struct FileChooseDialog {
    handle: Option<JoinHandle<Option<PathBuf>>>,
}

impl FileChooseDialog {
    fn is_open(&self) -> bool {
        self.handle
            .as_ref()
            .is_some_and(|handle| !handle.is_finished())
    }

    fn open(&mut self) {
        if !self.is_open() {
            self.handle = Some(std::thread::spawn(|| {
                rfd::FileDialog::new().set_directory(".").pick_file()
            }))
        }
    }

    fn take_result(&mut self) -> Option<PathBuf> {
        if self
            .handle
            .as_ref()
            .is_some_and(|handle| handle.is_finished())
        {
            self.handle
                .take()
                .unwrap()
                .join()
                .expect("Failed to choose file")
        } else {
            None
        }
    }
}

struct App {
    program_start: SystemTime,

    // Savegame.
    /// Thread handle for file choose dialog.
    file_choose_dialog: FileChooseDialog,
    /// Loaded savegame path.
    file: Option<PathBuf>,
    /// Mtime of the savegame file.
    mtime: SystemTime,

    // Game data.
    /// The map of tiles.
    map: Map,

    // Gpu resources.
    /// Set of textures.
    textures: Textures,
    /// Constant size info gpu buffer.
    view_buffer: Buffer,
    /// Gpu buffer containing static tile info.
    tiles_buffer: Buffer,
    /// Bind group for textures and buffers (TODO Split into two?).
    bind_groups: BindGroups,

    // Window data.
    /// Size of the window.
    size: UVec2,
    /// Aspect ration of the window.
    aspect_ratio: f32,

    // Mouse state.
    /// Mouse position in window coordinates.
    mouse_position: Vec2,
    /// Whether the left mouse button is held.
    grab_move: bool,
    /// Whether the right mouse button is held.
    grab_rotate: bool,

    // World info.
    /// Origin coordinates (center of screen).
    origin: Vec2,
    /// Rotation relative to origin (TODO unused currently).
    rotation: f32,
    /// World size (how many tiles are visible).
    inv_scale: f32,

    /// World hover position of mouse
    hover_pos: IVec2,
    /// Hovered rotation.
    hover_rotation: Rotation,

    /// Hovered tile id (if present).
    hover_tile: Option<TileId>,
    /// Hovered group id (if present).
    hover_group: Option<GroupId>,

    // Ui.
    /// How to color segments (TODO change to enum).
    coloring: i32,
    /// Whether to highlight hovered groups.
    highlight_hovered_group: bool,
    /// Whether to highlight open groups only.
    highlight_open_groups: bool,
}

const SIN_30: f32 = 0.5;
const COS_30: f32 = 0.8660254;

impl App {
    fn create_view_buffer(gpu: &Gpu) -> Buffer {
        let view_buffer_size = u64::try_from({
            // 2 int + 2 float
            let size = data::IVEC2_;
            let aspect_ratio = data::FLOAT_;
            let time = data::FLOAT_;

            // 4 float
            let origin = data::VEC2_;
            let rotation = data::FLOAT_;
            let inv_scale = data::FLOAT_;

            // 4 int
            let hover_pos = data::IVEC2_;
            let hover_rotation = data::INT_;
            let pad2 = data::PAD_;

            // 4 int
            let hover_tile = data::INT_;
            let hover_group = data::INT_;

            let coloring = data::INT_;
            let highlight_flags = data::INT_;

            let gold = data::IVEC2_;
            let silver = data::IVEC2_;

            let bronze = data::IVEC2_;

            (size + aspect_ratio + time)
                + (origin + rotation + inv_scale)
                + (hover_pos + hover_rotation + pad2)
                + (hover_tile + hover_group + coloring + highlight_flags)
                + (gold + silver)
                + bronze
        })
        .unwrap();
        gpu.create_buffer(
            "view",
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            SizeOrContent::Size(view_buffer_size),
        )
    }

    #[allow(clippy::identity_op)]
    fn create_tiles_buffer(gpu: &Gpu, map: &Map) -> Buffer {
        let tiles_buffer_size = u64::try_from(map.byte_size()).unwrap();
        gpu.create_buffer(
            "tiles",
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            SizeOrContent::Size(tiles_buffer_size),
        )
    }

    /// Create actual bind group.
    fn generate_bind_group(&mut self, gpu: &Gpu) {
        let mut bind_group_entries = vec![
            (0, self.view_buffer.binding()),
            (1, self.tiles_buffer.binding()),
        ];
        bind_group_entries.extend(self.textures.binding_resources());
        self.bind_groups
            .generate_bind_groups(gpu, &bind_group_entries);
    }

    fn new(window: &Window, gpu: &Gpu) -> Self {
        let map = Map::default();

        let textures = Textures::new(gpu);
        let view_buffer = Self::create_view_buffer(gpu);
        let tiles_buffer = Self::create_tiles_buffer(gpu, &map);
        let bind_groups = BindGroups::new(
            gpu,
            &[
                (0, BindGroups::UNIFORM),
                (1, BindGroups::STORAGE),
                (2, BindGroups::SAMPLER),
                (3, BindGroups::TEXTURE),
                (4, BindGroups::TEXTURE),
                (5, BindGroups::TEXTURE),
                (6, BindGroups::TEXTURE),
            ],
        );

        let mut app = Self {
            program_start: SystemTime::now(),

            // Savegame.
            file_choose_dialog: Default::default(),
            file: None,
            mtime: SystemTime::now(),

            // Game data.
            map,

            // Gpu resources.
            textures,
            view_buffer,
            tiles_buffer,
            bind_groups,

            // Window data.
            size: UVec2::ZERO,
            aspect_ratio: 0.0,

            // Mouse state.
            mouse_position: Vec2::ZERO,
            grab_move: false,
            grab_rotate: false,

            // World info.
            origin: Vec2::ZERO,
            rotation: 0.0,
            inv_scale: 20.0,

            hover_pos: IVec2::ZERO,
            hover_rotation: 0,

            hover_tile: None,
            hover_group: None,

            // Ui.
            coloring: 0,
            highlight_hovered_group: false,
            highlight_open_groups: true,
        };

        let size = window.inner_size();
        app.resize(UVec2::new(size.width, size.height));
        app.generate_bind_group(gpu);
        app.write_tiles(gpu);
        app
    }

    fn elapsed_secs(&self) -> f32 {
        SystemTime::now()
            .duration_since(self.program_start)
            .unwrap()
            .as_secs_f32()
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
        pos -= Self::hex_to_world(&prelim);
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

        self.hover_tile = self.map.tile_index(self.hover_pos);
        self.hover_group = self.hover_tile.and_then(|tile_id| {
            self.map
                .tile(tile_id)
                .segments_at(self.hover_rotation)
                .next()
                .map(|(segment_id, _)| self.map.group_of(tile_id, segment_id))
        });
    }

    fn on_scroll(&mut self, y: f32) {
        self.inv_scale = 5f32.max(self.inv_scale - y).min(500.0);
    }

    fn write_view(&self, gpu: &Gpu) {
        let mut buffer_view = self.view_buffer.write(gpu);
        unsafe {
            let ptr = buffer_view.as_mut_ptr();

            let uptr = ptr.cast::<u32>();
            let iptr = ptr.cast::<i32>();
            let fptr = ptr.cast::<f32>();

            *uptr.add(0) = self.size.x;
            *uptr.add(1) = self.size.y;
            *fptr.add(2) = self.aspect_ratio;
            *fptr.add(3) = self.elapsed_secs();
            *fptr.add(4) = self.origin.x;
            *fptr.add(5) = self.origin.y;
            *fptr.add(6) = self.rotation;
            *fptr.add(7) = self.inv_scale;
            *iptr.add(8) = self.hover_pos.x;
            *iptr.add(9) = self.hover_pos.y;
            *uptr.add(10) = self.hover_rotation as u32;
            // pad
            *uptr.add(12) = self.hover_tile.map_or(u32::MAX, |x| x.try_into().unwrap());
            *uptr.add(13) = self.hover_group.map_or(u32::MAX, |x| x.try_into().unwrap());
            *iptr.add(14) = self.coloring;
            let highlight_flags = if self.highlight_hovered_group { 1 } else { 0 }
                | if self.highlight_open_groups { 2 } else { 0 };
            *iptr.add(15) = highlight_flags;

            let best_placements = self.map.best_placements();
            if best_placements.len() >= 3 {
                let gold = best_placements[0];
                *iptr.add(16) = gold.0.x;
                *iptr.add(17) = gold.0.y;
                let silver = best_placements[1];
                *iptr.add(18) = silver.0.x;
                *iptr.add(19) = silver.0.y;
                let bronze = best_placements[2];
                *iptr.add(20) = bronze.0.x;
                *iptr.add(21) = bronze.0.y;
            }
        }
    }

    fn write_tiles(&self, gpu: &Gpu) {
        let mut buffer_view = self.tiles_buffer.write(gpu);
        unsafe {
            let ptr = buffer_view.as_mut_ptr();
            self.map.write_to(ptr);
        }
    }

    fn load_file(&mut self, file: PathBuf, gpu: &Gpu) {
        self.file = Some(file.clone());
        self.mtime = file
            .metadata()
            .and_then(|md| md.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);

        // Load savegame.
        let mut stream = File::open(file).expect("Failed to open file");
        let parsed = nrbf_rs::parse_nrbf(&mut stream);
        let savegame = raw_data::SaveGame::try_from(&parsed).unwrap();

        self.map = Map::from(&savegame);
        self.tiles_buffer = Self::create_tiles_buffer(gpu, &self.map);
        self.generate_bind_group(gpu);
        self.write_tiles(gpu);
    }

    fn handle_file_dialog(&mut self, gpu: &Gpu) {
        if let Some(file) = self.file_choose_dialog.take_result() {
            self.load_file(file, gpu);
        }
    }
}

fn run(
    event_loop: EventLoop<()>,
    window: Window,
    mut gpu: Gpu,
    mut pipeline: Pipeline,
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
                    } => app.on_scroll(y),
                    WindowEvent::CloseRequested => {
                        *control_flow = ControlFlow::Exit;
                    }
                    WindowEvent::Resized(size) => {
                        // Window has been resized. Adjust render pipeline settings.
                        gpu.resize(size.width, size.height);
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
                    egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Dorfromantik viewer");
                            if ui
                                .add_enabled(
                                    !app.file_choose_dialog.is_open(),
                                    egui::Button::new("Load file"),
                                )
                                .clicked()
                            {
                                app.file_choose_dialog.open();
                            }
                            ui.toggle_value(&mut sidebar_expanded, "Visual settings");
                        });
                    });
                    egui::SidePanel::left("left_panel").show_animated(
                        ctx,
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
                        if let Some(tile_id) = app.hover_tile {
                            let tile = &app.map.tile(tile_id);
                            let response = egui::Window::new(format!("Tile {tile_id}"))
                                .movable(false)
                                .collapsible(false)
                                .resizable(false)
                                .current_pos((
                                    app.mouse_position.x + 50.0,
                                    app.mouse_position.y - 50.0,
                                ))
                                .show(ctx, |ui| {
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

                                            for (segment_id, segment) in
                                                tile.segments.iter().enumerate()
                                            {
                                                ui.label("Terrain");
                                                ui.label(format!(
                                                    "{:?} {:?}",
                                                    segment.terrain, segment.form
                                                ));
                                                ui.end_row();

                                                ui.label("Group");
                                                ui.label(format!(
                                                    "{}",
                                                    app.map.group_of(tile_id, segment_id)
                                                ));
                                                ui.end_row();
                                            }
                                        }
                                    });
                                });

                            if let Some(group_id) = app.hover_group {
                                let group = &app.map.group(group_id);
                                let tile_rect = response.unwrap().response.rect;
                                let pos = (tile_rect.min.x, tile_rect.max.y + 10.0);
                                egui::Window::new(format!("Group {group_id}"))
                                    .movable(false)
                                    .collapsible(false)
                                    .resizable(false)
                                    .current_pos(pos)
                                    .show(ctx, |ui| {
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
                        } else if let Some(tile) = app.map.next_tile().as_ref() {
                            let rotation_scores = (0..6)
                                .map(|rotation| {
                                    let next = tile.moved_to(app.hover_pos, rotation);
                                    app.map.score_of(&next)
                                })
                                .max();
                            egui::Window::new("Placement score")
                                .movable(false)
                                .collapsible(false)
                                .resizable(false)
                                .current_pos((
                                    app.mouse_position.x + 50.0,
                                    app.mouse_position.y - 50.0,
                                ))
                                .show(ctx, |ui| {
                                    for score in rotation_scores.into_iter() {
                                        ui.label(format!("{score}"));
                                    }
                                });
                        }
                    }
                });

                app.handle_file_dialog(&gpu);
                if app.file.as_ref().is_some_and(|file| {
                    file.metadata()
                        .and_then(|md| md.modified())
                        .is_ok_and(|mtime| mtime > app.mtime)
                }) {
                    app.load_file(app.file.as_ref().unwrap().clone(), &gpu);
                }
                app.write_view(&gpu);
                let bind_groups = app
                    .bind_groups
                    .groups
                    .as_ref()
                    .map(|array| array.as_slice());
                pipeline.redraw(&gpu, bind_groups, &mut ui, full_output);
            }
            _ => {}
        }
    });
}

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new();
    let window = winit::window::Window::new(&event_loop).unwrap();
    let gpu = pollster::block_on(Gpu::new(&window));
    let app = App::new(&window, &gpu);
    let pipeline = Pipeline::new(&gpu, &app.bind_groups.layouts);
    let ui = Ui::new(&window);

    run(event_loop, window, gpu, pipeline, ui, app);
}
