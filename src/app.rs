use std::{
    fs::File,
    path::{Path, PathBuf},
    thread::JoinHandle,
    time::{Duration, SystemTime},
};

use glam::{IVec2, UVec2, Vec2};
use winit::window::Window;

use crate::{
    best_placements::BestPlacements,
    bind_groups::BindGroups,
    data::{self, Rotation},
    gpu::{Buffer, Gpu, SizeOrContent},
    group::GroupIndex,
    group_assignments::GroupAssignments,
    lerp::Interpolated,
    map::{Map, SegmentIndex},
    opencv, raw_data, shader,
    textures::Textures,
};

#[derive(Default)]
pub struct FileChooseDialog {
    handle: Option<JoinHandle<Option<PathBuf>>>,
}

impl FileChooseDialog {
    pub fn is_open(&self) -> bool {
        self.handle
            .as_ref()
            .is_some_and(|handle| !handle.is_finished())
    }

    pub fn open(&mut self) {
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
pub struct MapLoader {
    handle: Option<JoinHandle<(Map, GroupAssignments, BestPlacements)>>,
}

impl MapLoader {
    pub fn in_progress(&self) -> bool {
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
                println!("NRBF Tree loaded in: {tree_loaded:?}");
                let start = std::time::Instant::now();

                let savegame = raw_data::SaveGame::try_from(&parsed).unwrap();

                let save_loaded = start.elapsed();
                println!("Savegame loaded in: {save_loaded:?}");
                let start = std::time::Instant::now();

                let map = Map::from(&savegame);
                let groups = GroupAssignments::from(&map);
                let best_placements = BestPlacements::from((&map, &groups));
                opencv::map_to_img(&map);

                let map_loaded = start.elapsed();
                println!("Map loaded in: {map_loaded:?}");

                (map, groups, best_placements)
            }));
        }
    }

    fn take_result(&mut self) -> Option<(Map, GroupAssignments, BestPlacements)> {
        if self.handle.as_ref().is_some_and(JoinHandle::is_finished) {
            self.handle.take().unwrap().join().ok()
        } else {
            None
        }
    }
}

#[allow(clippy::struct_excessive_bools)]
pub struct App {
    program_start: SystemTime,

    // Savegame.
    /// Thread handle for file choose dialog.
    pub file_choose_dialog: FileChooseDialog,
    /// Loaded savegame path.
    file: Option<PathBuf>,
    /// Mtime of the savegame file.
    mtime: SystemTime,
    /// Tells whether we noticed that the file changed. We don't reload the file immediately, we
    /// wait for the mtime to stay the same for a second, then reload.
    change_detected: bool,

    // Game data.
    pub map_loader: MapLoader,
    /// Map of tiles.
    map: Map,
    group_assignments: GroupAssignments,
    pub best_placements: BestPlacements,
    pub show_placements: [bool; 30],

    // Gpu resources.
    /// Set of textures.
    textures: Textures,
    /// Constant size info gpu buffer.
    view_buffer: Buffer,
    /// Gpu buffer containing static tile info.
    tiles_buffer: Buffer,
    /// Bind group for textures and buffers (TODO Split into two?).
    pub bind_groups: BindGroups,

    // Window data.
    /// Size of the window.
    size: UVec2,
    /// Aspect ration of the window.
    aspect_ratio: f32,

    // Mouse state.
    /// Mouse position in window coordinates.
    mouse_position: Vec2,
    /// Whether the left mouse button is held.
    pub grab_move: bool,
    /// Whether the right mouse button is held.
    pub grab_rotate: bool,

    // World info.
    /// Current origin coordinates (center of screen).
    origin: Interpolated<Vec2>,
    /// Rotation relative to origin (TODO unused currently).
    rotation: f32,
    /// Current world size (how many tiles are visible).
    pub inv_scale: Interpolated<f32>, // TODO fix scaling.

    /// World hover position of mouse
    pub hover_pos: IVec2,
    /// Hovered rotation.
    hover_rotation: Rotation,
    /// Hovered segment index (if present).
    pub hover_segment: Option<SegmentIndex>,
    hover_group: Option<GroupIndex>,

    // Ui.
    pub goto_x: String,
    pub goto_y: String,

    /// How to color segments (TODO change to enum).
    pub section_style: i32,
    /// How to display closed groups (TODO change to enum).
    pub closed_group_style: i32,
    /// Whether to highlight hovered groups.
    pub highlight_hovered_group: bool,
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
            &SizeOrContent::Size(view_buffer_size),
        )
    }

    #[allow(clippy::identity_op)]
    fn create_tiles_buffer(gpu: &Gpu, byte_size: usize) -> Buffer {
        let tiles_buffer_size = u64::try_from(byte_size).unwrap();
        gpu.create_buffer(
            "tiles",
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            &SizeOrContent::Size(tiles_buffer_size),
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

    pub fn new(window: &Window, gpu: &Gpu) -> Self {
        let textures = Textures::new(gpu);
        let view_buffer = Self::create_view_buffer(gpu);
        let map_byte_size = shader::byte_size_for_n_tiles(0);
        let tiles_buffer = Self::create_tiles_buffer(gpu, map_byte_size);
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
            origin: Interpolated::new(Vec2::ZERO),
            rotation: 0.0,
            inv_scale: Interpolated::new(20.0),

            hover_pos: IVec2::ZERO,
            hover_rotation: 0,
            hover_segment: None,
            hover_group: None,

            // Ui.
            goto_x: String::new(),
            goto_y: String::new(),
            section_style: 0,
            closed_group_style: 1,
            highlight_hovered_group: false,

            map: Map::default(),
            group_assignments: GroupAssignments::default(),
            best_placements: BestPlacements::default(),
            show_placements: Default::default(),
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

    pub fn resize(&mut self, size: UVec2) {
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
        *self.origin + uv_2 * Vec2::new(self.aspect_ratio, 1.0) * *self.inv_scale
    }

    pub fn on_cursor_move(&mut self, pos: Vec2) {
        let delta = (pos - self.mouse_position) / self.size.as_vec2();

        if self.grab_move {
            self.origin.set(
                *self.origin + Vec2::new(-1.0 * self.aspect_ratio, 1.0) * delta * *self.inv_scale,
            );
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

        self.hover_segment = self
            .map
            .segment_index_at(self.hover_pos, self.hover_rotation);
        self.hover_group = self
            .hover_segment
            .and_then(|segment_index| self.group_assignments.group_of(segment_index));
    }

    pub fn on_scroll(&mut self, y: f32) {
        self.inv_scale.set(5f32.max(*self.inv_scale - y).min(500.0));
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
            *fptr.add(7) = *self.inv_scale;
            *iptr.add(8) = self.hover_pos.x;
            *iptr.add(9) = self.hover_pos.y;
            *uptr.add(10) = self.hover_rotation as u32;
            // pad
            // *uptr.add(12) = self.hover_tile.map_or(u32::MAX, |x| x.try_into().unwrap());
            *uptr.add(13) = self.hover_group.map_or(u32::MAX, |x| x.try_into().unwrap());
            *iptr.add(14) = self.section_style;
            *iptr.add(15) = self.closed_group_style;

            *iptr.add(16) = i32::from(self.highlight_hovered_group);

            let mut show_score_flags = 0;
            for (index, show) in self.show_placements.iter().enumerate().take(15) {
                show_score_flags |= i32::from(*show) << index;
                // TODO i32
            }
            *iptr.add(17) = show_score_flags;
        }
    }

    fn write_tiles(&self, gpu: &Gpu) {
        let mut buffer_view = self.tiles_buffer.write(gpu);
        unsafe {
            let ptr = buffer_view.as_mut_ptr();
            shader::write_map_to(
                ptr,
                &self.map,
                &self.group_assignments,
                &self.best_placements,
            );
        }
    }

    #[allow(clippy::unused_self)]
    fn previous_file_path_cache_path(&self) -> PathBuf {
        let mut previous_file_path =
            dirs::cache_dir().expect("There is no cache directory on this system");
        previous_file_path.push("dorfromantische2-rs/previous_file_path");
        let _ = std::fs::create_dir_all(previous_file_path.parent().unwrap());
        previous_file_path
    }

    pub fn set_file_path(&mut self, file: &Path) {
        self.file = Some(file.to_path_buf());
        self.mtime = SystemTime::UNIX_EPOCH;

        let cache_path = self.previous_file_path_cache_path();
        std::fs::write(cache_path, file.to_str().unwrap())
            .expect("Failed to write file path to cache");
    }

    pub fn use_previous_file_path(&mut self) {
        let cache_path = self.previous_file_path_cache_path();
        if let Ok(file_path) = std::fs::read_to_string(cache_path) {
            self.set_file_path(&PathBuf::from(file_path));
        }
    }

    fn handle_file_dialog(&mut self) {
        if let Some(file) = self.file_choose_dialog.take_result() {
            self.set_file_path(&file);
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
        if let Some((map, groups, best_placements)) = self.map_loader.take_result() {
            self.map = map;
            self.group_assignments = groups;
            self.best_placements = best_placements;
            self.show_placements = [true; 30];
            self.zoom_fit();

            let map_byte_size = shader::byte_size(&self.map);
            self.tiles_buffer = Self::create_tiles_buffer(gpu, map_byte_size);
            self.generate_bind_group(gpu);
            self.write_tiles(gpu);

            self.hover_segment = None;
        }
    }

    pub fn submit_goto(&mut self) {
        let x = self.goto_x.parse::<i32>();
        let y = self.goto_y.parse::<i32>();
        if let (Ok(x), Ok(y)) = (x, y) {
            self.origin.set_target(Self::hex_to_world(IVec2::new(x, y)));
            self.inv_scale.set_target(30.0);
        }
    }

    pub fn tick(&mut self, gpu: &Gpu) {
        self.origin.tick();
        self.inv_scale.tick();

        self.handle_file_dialog();
        self.reload_file_if_changed();
        self.handle_map_loader(gpu);
        self.write_view(gpu);
    }

    #[allow(clippy::similar_names)]
    // Similar names yfix, xfit
    pub fn zoom_fit(&mut self) {
        let offset = self.map.index_offset;
        let size = self.map.index_size;

        self.origin.set_target(App::hex_to_world(offset + size / 2));

        let xfit = 0.5 + size.x as f32 * 1.5;
        let yfit = (1 + size.y) as f32 * COS_30;

        self.inv_scale.set_target(xfit.max(yfit));
    }
}
