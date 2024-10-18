use data::{Rotation, TileId};
use glam::{IVec2, UVec2, Vec2};
use gpu::{Buffer, Gpu, SizeOrContent};
use map::{GroupId, Map};
use pipeline::Pipeline;
use std::{
    env,
    fs::File,
    path::{Path, PathBuf},
    thread::JoinHandle,
    time::{Duration, SystemTime},
};
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
mod pipeline;
mod raw_data;
mod textures;

mod opencv;
mod xlib;

struct Ui {
    context: egui::Context,
    state: egui_winit::State,
}

impl Ui {
    fn new(window: &Window) -> Self {
        Self {
            context: egui::Context::default(),
            state: egui_winit::State::new(window),
        }
    }

    fn on_event(&mut self, event: &WindowEvent) -> egui_winit::EventResponse {
        self.state.on_event(&self.context, event)
    }

    fn run(
        &mut self,
        window: &Window,
        run_ui: impl FnOnce(&egui::Context),
    ) -> (Vec<egui::ClippedPrimitive>, egui::TexturesDelta) {
        let egui::FullOutput {
            shapes,
            textures_delta,
            ..
        } = self.context.run(self.state.take_egui_input(window), run_ui);
        (self.context.tessellate(shapes), textures_delta)
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
            .copied()
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
            groups: Option::default(),
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
            }));
        }
    }

    fn take_result(&mut self) -> Option<PathBuf> {
        if self.handle.as_ref().is_some_and(JoinHandle::is_finished) {
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
#[derive(Default)]
struct MapLoader {
    handle: Option<JoinHandle<Map>>,
}

impl MapLoader {
    fn in_progress(&self) -> bool {
        self.handle
            .as_ref()
            .is_some_and(|handle| !handle.is_finished())
    }

    fn load(&mut self, path: &Path) {
        if !self.in_progress() {
            let path = path.to_owned();
            self.handle = Some(std::thread::spawn(|| {
                // Load savegame.
                let path = path;

                let start = std::time::Instant::now();

                // Fugly retry mechanism.
                let parsed = std::panic::catch_unwind(|| {
                    let mut stream = File::open(&path).expect("Failed to open file");
                    nrbf_rs::parse_nrbf(&mut stream)
                })
                .or_else(|_| {
                    std::panic::catch_unwind(|| {
                        let mut stream = File::open(&path).expect("Failed to open file");
                        nrbf_rs::parse_nrbf(&mut stream)
                    })
                })
                .or_else(|_| {
                    std::panic::catch_unwind(|| {
                        let mut stream = File::open(&path).expect("Failed to open file");
                        nrbf_rs::parse_nrbf(&mut stream)
                    })
                })
                .unwrap();

                let tree_loaded = start.elapsed();
                println!("NRBF Tree loaded in: {:?}", tree_loaded);
                let start = std::time::Instant::now();

                let savegame = raw_data::SaveGame::try_from(&parsed).unwrap();

                let save_loaded = start.elapsed();
                println!("Savegame loaded in: {:?}", save_loaded);
                let start = std::time::Instant::now();

                let map = Map::from(&savegame);

                let map_loaded = start.elapsed();
                println!("Map loaded in: {:?}", map_loaded);

                map
            }));
        }
    }

    fn take_result(&mut self) -> Option<Map> {
        if self.handle.as_ref().is_some_and(JoinHandle::is_finished) {
            self.handle.take().unwrap().join().ok()
        } else {
            None
        }
    }
}

#[allow(clippy::struct_excessive_bools)]
struct App {
    program_start: SystemTime,

    // Savegame.
    /// Thread handle for file choose dialog.
    file_choose_dialog: FileChooseDialog,
    /// Loaded savegame path.
    file: Option<PathBuf>,
    /// Mtime of the savegame file.
    mtime: SystemTime,
    /// Tells whether we noticed that the file changed. We don't reload the file immediately, we
    /// wait for the mtime to stay the same for a second, then reload.
    change_detected: bool,

    // Game data.
    map_loader: MapLoader,
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
    /// Current origin coordinates (center of screen).
    origin: Vec2,
    /// Origin coordinates to move from.
    source_origin: Vec2,
    /// Origin coordinates to move to.
    target_origin: Vec2,
    /// Mix between source and target.
    origin_mix: f32,

    /// Rotation relative to origin (TODO unused currently).
    rotation: f32,
    /// Current world size (how many tiles are visible).
    inv_scale: f32,
    source_inv_scale: f32,
    target_inv_scale: f32,
    inv_scale_mix: f32,

    /// World hover position of mouse
    hover_pos: IVec2,
    /// Hovered rotation.
    hover_rotation: Rotation,

    /// Hovered tile id (if present).
    hover_tile: Option<TileId>,
    /// Hovered group id (if present).
    hover_group: Option<GroupId>,

    // Ui.
    goto_x: String,
    goto_y: String,

    /// How to color segments (TODO change to enum).
    section_style: i32,
    /// How to display closed groups (TODO change to enum).
    closed_group_style: i32,
    /// Whether to highlight hovered groups.
    highlight_hovered_group: bool,

    show_placements: [bool; 7],
}

const SIN_30: f32 = 0.5;
const COS_30: f32 = 0.866_025_4;

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
            let pad1 = data::PAD_;

            // 4 int
            let hover_tile = data::INT_;
            let hover_group = data::INT_;
            let section_style = data::INT_;
            let closed_group_style = data::INT_;

            // 2 int
            let highlight_hovered_group = data::INT_;
            let show_placements = data::INT_;

            (size + aspect_ratio + time)
                + (origin + rotation + inv_scale)
                + (hover_pos + hover_rotation + pad1)
                + (hover_tile + hover_group + section_style + closed_group_style)
                + (highlight_hovered_group + show_placements)
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
            file_choose_dialog: FileChooseDialog::default(),
            file: None,
            mtime: SystemTime::now(),
            change_detected: false,

            // Game data.
            map_loader: MapLoader::default(),
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
            source_origin: Vec2::ZERO,
            target_origin: Vec2::ZERO,
            origin_mix: 1.0,

            rotation: 0.0,

            inv_scale: 20.0,
            source_inv_scale: 20.0,
            target_inv_scale: 20.0,
            inv_scale_mix: 1.0,

            hover_pos: IVec2::ZERO,
            hover_rotation: 0,

            hover_tile: None,
            hover_group: None,

            // Ui.
            goto_x: String::new(),
            goto_y: String::new(),
            section_style: 0,
            closed_group_style: 1,
            highlight_hovered_group: false,

            show_placements: [true; 7],
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
        let f32_size = size.as_vec2();
        self.aspect_ratio = f32_size.x / f32_size.y;
    }

    fn hex_to_world(pos: IVec2) -> Vec2 {
        Vec2::new(pos.x as f32 * 1.5, (pos.x + pos.y * 2) as f32 * COS_30)
    }

    fn world_to_hex(mut pos: Vec2) -> IVec2 {
        let x = (pos.x / 1.5).round();
        let y_rest = pos.y - x * COS_30;
        let y = (y_rest / (2.0 * COS_30)).round();

        let prelim = IVec2::new(x as i32, y as i32);
        pos -= Self::hex_to_world(prelim);
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
            let new_origin =
                self.origin + Vec2::new(-1.0 * self.aspect_ratio, 1.0) * delta * self.inv_scale;
            self.origin = new_origin;
            self.target_origin = new_origin;
            self.origin_mix = 1.0;
        }

        if self.grab_rotate {
            self.rotation += delta.x;
        }

        self.mouse_position = pos;
        let world_pos = self.pixel_to_world(pos);
        self.hover_pos = Self::world_to_hex(world_pos);
        let offset = world_pos - App::hex_to_world(self.hover_pos);

        let gradient = 2.0 * COS_30 * offset.x;
        self.hover_rotation = match (offset.y > 0.0, offset.y > gradient, offset.y > -gradient) {
            (true, true, true) => 0,
            (true, false, _) => 1,
            (false, _, true) => 2,
            (false, false, false) => 3,
            (false, true, _) => 4,
            (true, _, false) => 5,
        };

        self.hover_tile = self.map.tile_id_at(self.hover_pos);
        self.hover_group = self.hover_tile.and_then(|tile_id| {
            self.map
                .tile(tile_id)
                .unwrap()
                .segments_at(self.hover_rotation)
                .next()
                .map(|(segment_id, _)| self.map.group_of(tile_id, segment_id))
        });
    }

    fn on_scroll(&mut self, y: f32) {
        let inv_scale = 5f32.max(self.inv_scale - y).min(500.0);
        self.inv_scale = inv_scale;
        self.target_inv_scale = inv_scale;
        self.inv_scale_mix = 1.0;
    }

    #[allow(clippy::cast_ptr_alignment, clippy::similar_names)]
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
            *iptr.add(14) = self.section_style;
            *iptr.add(15) = self.closed_group_style;

            *iptr.add(16) = i32::from(self.highlight_hovered_group);

            let mut show_score_flags = 0;
            for score in 0..7 {
                show_score_flags |= i32::from(self.show_placements[score]) << score;
                // TODO i32
            }
            *iptr.add(17) = show_score_flags;
        }
    }

    fn write_tiles(&self, gpu: &Gpu) {
        let mut buffer_view = self.tiles_buffer.write(gpu);
        unsafe {
            let ptr = buffer_view.as_mut_ptr();
            self.map.write_to(ptr);
        }
    }

    fn previous_file_path_cache_path(&self) -> PathBuf {
        let mut previous_file_path =
            dirs::cache_dir().expect("There is no cache directory on this system");
        previous_file_path.push("dorfromantische2-rs/previous_file_path");
        let _ = std::fs::create_dir_all(previous_file_path.parent().unwrap());
        previous_file_path
    }

    pub fn set_file_path(&mut self, file: PathBuf) {
        self.file = Some(file.clone());
        self.mtime = SystemTime::UNIX_EPOCH;

        let cache_path = self.previous_file_path_cache_path();
        std::fs::write(cache_path, file.to_str().unwrap())
            .expect("Failed to write file path to cache");
    }

    pub fn use_previous_file_path(&mut self) {
        let cache_path = self.previous_file_path_cache_path();
        if let Ok(file_path) = std::fs::read_to_string(cache_path) {
            self.set_file_path(file_path.into());
        }
    }

    fn handle_file_dialog(&mut self) {
        if let Some(file) = self.file_choose_dialog.take_result() {
            self.set_file_path(file)
        }
    }

    fn reload_file_if_changed(&mut self) {
        if let Some(file) = self.file.as_ref() {
            let actual_mtime = file.metadata().ok().and_then(|md| md.modified().ok());
            if let Some(actual_mtime) = actual_mtime {
                if actual_mtime > self.mtime {
                    self.change_detected = true;
                    self.mtime = actual_mtime;
                } else if self.change_detected
                    && actual_mtime == self.mtime
                    && SystemTime::now() > actual_mtime + Duration::from_secs(1)
                {
                    self.change_detected = false;
                    self.map_loader.load(file);
                }
            }
        }
    }

    fn handle_map_loader(&mut self, gpu: &Gpu) {
        if let Some(map) = self.map_loader.take_result() {
            self.map = map;

            self.tiles_buffer = Self::create_tiles_buffer(gpu, &self.map);
            self.generate_bind_group(gpu);
            self.write_tiles(gpu);
            self.show_placements = [false; 7];
            if let Some((score, _)) = self.map.best_placements().iter().last() {
                self.show_placements[*score as usize] = true;
            }
            self.hover_tile = None;
            self.hover_group = None;
        }
    }

    fn submit_goto(&mut self) {
        let x = self.goto_x.parse::<i32>();
        let y = self.goto_y.parse::<i32>();
        if let (Ok(x), Ok(y)) = (x, y) {
            self.source_origin = self.origin;
            self.target_origin = Self::hex_to_world(IVec2::new(x, y));
            self.origin_mix = 0.0;

            self.source_inv_scale = self.inv_scale;
            self.target_inv_scale = 30.0;
            self.inv_scale_mix = 0.0;
        }
    }

    fn tick(&mut self, gpu: &Gpu) {
        let smoothstep = |x: f32| -2.0 * x.powi(3) + 3.0 * x.powi(2);

        self.origin_mix = 1f32.min(self.origin_mix + 1.0 / 60.0);
        self.origin = self
            .source_origin
            .lerp(self.target_origin, smoothstep(self.origin_mix));

        self.inv_scale_mix = 1f32.min(self.inv_scale_mix + 1.0 / 60.0);
        self.inv_scale = self.source_inv_scale
            + (self.target_inv_scale - self.source_inv_scale) * smoothstep(self.inv_scale_mix);

        self.handle_file_dialog();
        self.reload_file_if_changed();
        self.handle_map_loader(&gpu);
        self.write_view(&gpu);
    }
}

fn render_ui(
    app: &mut App,
    ctx: &egui::Context,
    sidebar_expanded: &mut bool,
    show_tooltip: &mut bool,
) {
    // Top panel with title and some menus.
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
            ui.toggle_value(sidebar_expanded, "Visual settings");
        });
    });

    // Main config panel.
    egui::SidePanel::left("left_panel").show_animated(ctx, *sidebar_expanded, |ui| {
        ui.label(egui::RichText::new("Orientation").size(20.0).underline());
        ui.horizontal(|ui| {
            ui.label("Goto");
            let size = ui.available_size();

            let edit_x = egui::TextEdit::singleline(&mut app.goto_x);
            ui.add_sized((size.x / 3.0, size.y), edit_x);

            let edit_y = egui::TextEdit::singleline(&mut app.goto_y);
            let response = ui.add_sized((size.x / 3.0, size.y), edit_y);

            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                app.submit_goto();
            }
        });
        ui.horizontal(|ui| {
            let slider = egui::Slider::new(&mut app.target_inv_scale, 5.0..=500.0).text("Zoom out");
            if ui.add(slider).changed() {
                app.inv_scale = app.target_inv_scale;
                app.inv_scale_mix = 1.0;
            }
            if ui.button("Zoom fit").clicked() {
                let (offset, size) = app.map.offset_and_size();

                app.source_origin = app.origin;
                app.target_origin = App::hex_to_world(offset + size / 2);
                app.origin_mix = 0.0;

                app.source_inv_scale = app.inv_scale;
                app.target_inv_scale =
                    (0.5 + size.x as f32 * 1.5).max((1 + size.y) as f32 * COS_30);
                app.inv_scale_mix = 0.0;
            }
        });
        ui.add_space(10.0);

        ui.label(egui::RichText::new("Tooltip").size(20.0).underline());
        ui.checkbox(show_tooltip, "Show tooltip");
        ui.add_space(10.0);

        ui.label(egui::RichText::new("Section style").size(20.0).underline());
        ui.selectable_value(&mut app.section_style, 0, "Color by terrain type");
        ui.selectable_value(&mut app.section_style, 1, "Color by group statically");
        ui.selectable_value(&mut app.section_style, 2, "Color by group dynamically");
        ui.selectable_value(&mut app.section_style, 3, "Color by texture");
        ui.add_space(10.0);

        ui.label(
            egui::RichText::new("Group display options")
                .size(20.0)
                .underline(),
        );
        ui.label("Closed groups");
        ui.horizontal(|ui| {
            ui.selectable_value(&mut app.closed_group_style, 0, "Show");
            ui.selectable_value(&mut app.closed_group_style, 1, "Dim");
            ui.selectable_value(&mut app.closed_group_style, 2, "Hide");
        });
        ui.checkbox(&mut app.highlight_hovered_group, "Highlight hovered group");
        ui.add_space(10.0);

        ui.label(
            egui::RichText::new("Placement display")
                .size(20.0)
                .underline(),
        );
        egui::Grid::new("placement_options").show(ui, |ui| {
            ui.label("Score");
            ui.label("Count");
            ui.label("Show");
            ui.end_row();

            for (score, placements) in app.map.best_placements() {
                if *score < 0 {
                    continue;
                }
                ui.label(format!("{score}"));
                ui.label(format!("{}", placements.len()));
                ui.checkbox(&mut app.show_placements[*score as usize], "");
                ui.end_row();
            }
            // ui.label(format!("x: {}, y: {}", app.hover_pos.x, app.hover_pos.y));
        });
        ui.add_space(10.0);
    });

    // Tooltip (hovering close to mouse)
    if *show_tooltip {
        if let Some(tile_id) = app.hover_tile {
            let tile = app.map.tile(tile_id).unwrap();
            let response = egui::Window::new(format!("Tile {tile_id}"))
                .movable(false)
                .collapsible(false)
                .resizable(false)
                .current_pos((app.mouse_position.x + 50.0, app.mouse_position.y - 50.0))
                .show(ctx, |ui| {
                    egui::Grid::new("tile_data").show(ui, |ui| {
                        ui.label("Axial position");
                        ui.label(format!("x: {}, y: {}", app.hover_pos.x, app.hover_pos.y));
                        ui.end_row();

                        if !tile.segments.is_empty() {
                            ui.label("Segments");
                            ui.end_row();

                            ui.label("Terrain");
                            ui.label("Form");
                            ui.label("Group");
                            ui.end_row();

                            for (segment_id, segment) in tile.segments.iter().enumerate() {
                                ui.label(format!("{:?}", segment.terrain));
                                ui.label(format!("{:?}", segment.form));
                                let group = app.map.group_of(tile_id, segment_id);
                                ui.label(format!("{group}",));
                                ui.end_row();
                            }
                        }

                        ui.label("Quest");
                        ui.label(format!("{:?}", tile.quest_tile));
                        ui.end_row();
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
            let rotation_scores = (0..6).map(|rotation| {
                let next = tile.moved_to(app.hover_pos, rotation);
                app.map.score_of(&next)
            });
            egui::Window::new("Placement score")
                .movable(false)
                .collapsible(false)
                .resizable(false)
                .current_pos((app.mouse_position.x + 50.0, app.mouse_position.y - 50.0))
                .show(ctx, |ui| {
                    for (matching_edges, probability_score) in rotation_scores {
                        ui.label(format!("{matching_edges} {probability_score}",));
                    }
                });
        }
    }

    if app.map_loader.in_progress() {
        egui::Area::new("my_area")
            .anchor(egui::Align2::RIGHT_BOTTOM, (-50.0, -50.0))
            .show(ctx, |ui| {
                ui.add(egui::Spinner::default().size(40.0));
            });
    }
}

#[allow(for_loops_over_fallibles)]
fn run(
    event_loop: EventLoop<()>,
    window: Window,
    mut gpu: Gpu,
    mut pipeline: Pipeline,
    mut ui: Ui,
    mut app: App,
) {
    let mut show_tooltip = false;
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
                        pipeline.resize(size.width, size.height);
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
                let (paint_jobs, textures_delta) = ui.run(&window, |ctx| {
                    // TODO move these bools somewhere....
                    render_ui(&mut app, ctx, &mut sidebar_expanded, &mut show_tooltip)
                });

                app.tick(&gpu);
                let bind_groups = app.bind_groups.groups.as_ref().map(<[_; 1]>::as_slice);
                pipeline.redraw(&gpu, bind_groups, &paint_jobs, textures_delta);
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
    let mut app = App::new(&window, &gpu);
    let pipeline = Pipeline::new(&gpu, &window, &app.bind_groups.layouts);
    let ui = Ui::new(&window);

    // Load the specified or previous file.
    let arguments = env::args().collect::<Vec<_>>();
    if arguments.len() > 1 {
        let file = arguments[1].clone().into();
        app.set_file_path(file);
    } else {
        app.use_previous_file_path();
    }

    run(event_loop, window, gpu, pipeline, ui, app);
    dbg!("Exiting");
}
