use std::time::SystemTime;

use glam::{UVec2, Vec2};
use winit::window::Window;

use crate::{
    best_placements::MAX_SHOWN_PLACEMENTS,
    file_watcher::FileWatcher,
    game_data::GameData,
    render::bind_groups::BindGroups,
    render::camera::Camera,
    render::gpu::{Buffer, Gpu, SizeOrContent},
    render::shader,
    render::textures::Textures,
    tile_frequency,
    ui::input_state::InputState,
    ui::ui_state::UiState,
};

#[allow(dead_code)]
const INITIAL_SHOWN_PLACEMENTS: usize = 5;

#[allow(clippy::struct_excessive_bools)]
pub struct App {
    program_start: SystemTime,

    // Savegame / file watching.
    pub file_watcher: FileWatcher,

    // Game data.
    pub data: GameData,
    /// Countdown frames until zoom_fit runs (0 = inactive, 2 = wait one frame).
    pub pending_zoom_fit: u8,

    // Gpu resources.
    /// Set of textures.
    textures: Textures,
    /// Constant size info gpu buffer.
    view_buffer: Buffer,
    /// Gpu buffer containing static tile info.
    tiles_buffer: Buffer,
    /// Bind group for textures and buffers (TODO Split into two?).
    pub bind_groups: BindGroups,

    // Camera / viewport.
    pub camera: Camera,

    // Input/mouse state.
    pub input: InputState,

    // Game navigation.
    pub game_nav: crate::game::game_nav::GameNav,

    // Ui.
    pub ui_state: UiState,

    /// Area not covered by UI panels.
    pub visible_rect: egui::Rect,
}

use crate::hex::COS_30;

impl App {
    fn create_view_buffer(gpu: &Gpu) -> Buffer {
        gpu.create_buffer(
            "view",
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            &SizeOrContent::Size(shader::view_buffer_size()),
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

            // Savegame / file watching.
            file_watcher: FileWatcher::default(),

            // Game data.
            data: GameData::default(),
            pending_zoom_fit: 0,

            // Gpu resources.
            textures,
            view_buffer,
            tiles_buffer,
            bind_groups,

            // Camera / viewport.
            camera: Camera::default(),

            // Input/mouse state.
            input: InputState::default(),

            // Game navigation.
            game_nav: crate::game::game_nav::GameNav::default(),

            // Ui.
            ui_state: UiState::default(),
            visible_rect: egui::Rect::EVERYTHING,
        };

        let size = window.inner_size();
        app.camera.resize(UVec2::new(size.width, size.height));
        app.generate_bind_group(gpu);
        app.write_tiles(gpu);
        app
    }

    fn hex_to_world(pos: glam::IVec2) -> Vec2 {
        crate::hex::hex_to_world(pos)
    }

    fn world_to_hex(pos: Vec2) -> glam::IVec2 {
        crate::hex::world_to_hex(pos)
    }

    pub fn on_cursor_move(&mut self, pos: Vec2) {
        let delta = (pos - self.input.mouse_position) / self.camera.size.as_vec2();

        if self.input.grab_move {
            self.camera.origin.set(
                *self.camera.origin
                    + Vec2::new(-self.camera.aspect_ratio, 1.0) * delta * *self.camera.inv_scale,
            );
        }

        if self.input.grab_rotate {
            self.camera.rotation += delta.x;
        }

        self.input.mouse_position = pos;

        // Only compute hover when mouse is in the map area.
        let egui_pos = egui::Pos2::new(pos.x, pos.y);
        if !self.visible_rect.contains(egui_pos) {
            self.input.hover_segment = None;
            self.input.hover_group = None;
            return;
        }

        let world_pos = self.camera.pixel_to_world(pos);
        self.input.hover_pos = Self::world_to_hex(world_pos);
        let offset = world_pos - App::hex_to_world(self.input.hover_pos);

        let gradient = 2.0 * COS_30 * offset.x;
        self.input.hover_rotation =
            match (offset.y > 0.0, offset.y > gradient, offset.y > -gradient) {
                (true, true, true) => 0,
                (true, false, _) => 1,
                (false, _, true) => 2,
                (false, false, false) => 3,
                (false, true, _) => 4,
                (true, _, false) => 5,
            };

        self.input.hover_segment = self
            .data
            .map
            .segment_at(self.input.hover_pos, self.input.hover_rotation)
            .map(|(i, _)| i);
        self.input.hover_group = self
            .input
            .hover_segment
            .and_then(|segment_index| self.data.group_assignments.group_of(segment_index));
    }

    fn write_view(&self, gpu: &Gpu) {
        shader::write_view(
            &self.view_buffer,
            gpu,
            &self.camera,
            &self.input,
            &self.ui_state,
            &self.data.map,
            &self.data.best_placements,
            self.program_start,
        );
    }

    fn write_tiles(&self, gpu: &Gpu) {
        shader::write_tiles(
            &self.tiles_buffer,
            gpu,
            &self.data.map,
            &self.data.group_assignments,
            &self.data.best_placements,
        );
    }

    fn handle_map_loader(&mut self, gpu: &Gpu) {
        if let Some((map, groups, best_placements)) = self.file_watcher.map_loader.take_result() {
            self.data.tile_frequencies = tile_frequency::TileFrequencies::from_map(&map);
            self.data.map = map;
            self.data.group_assignments = groups;
            self.data.best_placements = best_placements;
            self.data.invalidate_cache();
            self.ui_state.show_placements = [false; MAX_SHOWN_PLACEMENTS];
            self.ui_state.focused_placement = None;
            // Pre-select placements within 5% of the lowest fit chance.
            let best_fit = self
                .data
                .best_placements
                .iter_all()
                .first()
                .map(|(_, s)| s.fit_chance)
                .unwrap_or(1.0);
            let threshold = best_fit + 0.05;
            for (rank, score) in self.data.best_placements.iter_all() {
                if rank >= MAX_SHOWN_PLACEMENTS {
                    break;
                }
                if score.fit_chance <= threshold {
                    self.ui_state.show_placements[rank] = true;
                }
            }
            self.pending_zoom_fit = 2; // Wait 1 frame for sidebar to settle.

            let map_byte_size = shader::byte_size(&self.data.map);
            self.tiles_buffer = Self::create_tiles_buffer(gpu, map_byte_size);
            self.generate_bind_group(gpu);
            self.write_tiles(gpu);

            self.input.hover_segment = None;

            // Rebuild map silhouette for viewport detection.
            self.game_nav.update_map(&self.data.map);
        }
    }

    pub fn tick(&mut self, gpu: &Gpu) {
        self.camera.tick();

        // Game navigation: sync game viewport with solver viewport.
        self.game_nav.enabled = self.ui_state.game_nav_enabled;
        let solver_center = *self.camera.origin;
        let mouse_abs = Some((
            self.input.mouse_position.x as i32,
            self.input.mouse_position.y as i32,
        ));
        let mouse_idle = !self.input.grab_move && !self.input.grab_rotate;
        self.game_nav.tick(solver_center, mouse_abs, mouse_idle);

        self.file_watcher.handle_file_dialog();
        self.file_watcher.reload_file_if_changed();
        self.handle_map_loader(gpu);
        self.write_view(gpu);
    }
}
