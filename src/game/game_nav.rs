//! Game navigation: synchronize the solver viewport with the Dorfromantik game
//! by capturing screenshots, determining the game viewport position, and
//! simulating mouse drag to pan the game view.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use enigo::{Button, Coordinate, Direction, Enigo, Mouse, Settings};
use glam::Vec2;
use niri_ipc::socket::Socket;

use crate::data::HexPos;
use niri_ipc::{Action, Request, Response};

use super::game_camera::GameCamera;
use super::viewport_detect::MapSilhouette;
use crate::coords::{CameraMode, UnityCameraState, WorldPos};
use crate::map::Map;

/// Path to the camera position file written by the hardpatched game.
const CAMERA_POS_FILE: &str =
    "/home/sascha/.local/share/Steam/steamapps/common/Dorfromantik/camera_pos.txt";
/// Path to the file the solver writes to move the game camera.
const CAMERA_SET_FILE: &str =
    "/home/sascha/.local/share/Steam/steamapps/common/Dorfromantik/camera_set.txt";

/// State for tracking viewport changes and triggering game navigation.
pub struct GameNav {
    pub camera: GameCamera,
    last_solver_center: WorldPos,
    last_change: Instant,
    pending: bool,
    /// Estimated game viewport center (world coords).
    game_center: Option<WorldPos>,
    settle_time: Duration,
    /// Camera coupling mode.
    pub camera_mode: CameraMode,
    /// Solver's mouse position (absolute screen pixels) for restoration after drag.
    saved_mouse: Option<(i32, i32)>,
    /// Precomputed map silhouette for viewport detection (shared with bg thread).
    map_silhouette: Option<Arc<MapSilhouette>>,
    /// Status message for the UI.
    pub detect_status: String,
    /// Pending map silhouette being built in background.
    pending_silhouette: Option<Arc<Mutex<Option<MapSilhouette>>>>,
    /// Game screen dimensions (width, height) from the last screenshot.
    pub screen_size: (u32, u32),
    /// Last contents of camera_pos.txt to detect changes.
    last_camera_file: String,
    /// Last parsed Unity camera state.
    last_unity_state: Option<UnityCameraState>,
}

impl Default for GameNav {
    fn default() -> Self {
        Self {
            camera: GameCamera::default(),
            last_solver_center: WorldPos::ZERO,
            last_change: Instant::now(),
            pending: false,
            game_center: None,
            settle_time: Duration::from_millis(500),
            camera_mode: CameraMode::Off,
            saved_mouse: None,
            map_silhouette: None,
            detect_status: "Off".into(),
            pending_silhouette: None,
            screen_size: (2560, 1440),
            last_camera_file: String::new(),
            last_unity_state: None,
        }
    }
}

impl GameNav {
    /// Call each frame with the solver viewport center, mouse position,
    /// and whether the mouse is idle (no movement, no buttons held).
    pub fn tick(
        &mut self,
        solver_center: WorldPos,
        mouse_abs: Option<(i32, i32)>,
        mouse_idle: bool,
    ) -> bool {
        self.saved_mouse = mouse_abs;

        if self.camera_mode != CameraMode::Off {
            // Poll camera_pos.txt from the hardpatched game.
            self.poll_camera_file();
        }

        // Check for completed silhouette build.
        let sil_ready = self
            .pending_silhouette
            .as_ref()
            .and_then(|p| p.try_lock().ok().and_then(|mut g| g.take()));
        if let Some(sil) = sil_ready {
            self.map_silhouette = Some(Arc::new(sil));
            self.detect_status = "Map loaded".into();
            self.pending_silhouette = None;
        }

        // Duplex: when solver camera moves, tell the game to follow immediately.
        if self.camera_mode == CameraMode::Duplex {
            let moved = (solver_center.0 - self.last_solver_center.0).length() > 1.0;
            if moved {
                self.last_solver_center = solver_center;
                self.write_camera_set(solver_center);
            }
        }

        if !self.pending || self.last_change.elapsed() < self.settle_time || !mouse_idle {
            return false;
        }

        self.pending = false;
        self.navigate_to(solver_center)
    }

    fn navigate_to(&mut self, target_world: WorldPos) -> bool {
        let game_center = self.game_center.unwrap_or(WorldPos::ZERO);
        let delta = target_world.0 - game_center.0;
        if delta.length() < 2.0 {
            return false;
        }

        let ss = self.screen_size;
        let screen_center = Vec2::new(ss.0 as f32 / 2.0, ss.1 as f32 / 2.0);
        let current_pixel = self.camera.world_to_pixel(game_center, ss);
        let target_pixel = self.camera.world_to_pixel(target_world, ss);
        let (current_pixel, target_pixel) = match (current_pixel, target_pixel) {
            (Some(c), Some(t)) => (c, t),
            _ => return false,
        };

        // Total drag vector (drag opposite to desired view movement).
        let total_delta = current_pixel.0 - target_pixel.0;

        let win_info = match find_windows() {
            Some(info) => info,
            None => {
                log::warn!("GameNav: can't find game window");
                return false;
            }
        };

        let goff = win_info.game_output_offset;
        let soff = win_info.solver_output_offset;
        let restore_pos = self.saved_mouse.map(|(mx, my)| (mx + soff.0, my + soff.1));

        // Focus the game window.
        focus_window(win_info.game_window_id);
        std::thread::sleep(Duration::from_millis(100));

        // Maximum drag distance per stroke (stay within window margins).
        let margin = 100.0;
        let max_dx = ss.0 as f32 / 2.0 - margin;
        let max_dy = ss.1 as f32 / 2.0 - margin;

        let mut remaining = total_delta;
        let mut success = true;

        while remaining.length() > 10.0 {
            // Clamp this stroke to fit within the window.
            let stroke = Vec2::new(
                remaining.x.clamp(-max_dx, max_dx),
                remaining.y.clamp(-max_dy, max_dy),
            );

            let from = screen_center;
            let to = screen_center + stroke;

            let from_x = from.x as i32 + goff.0;
            let from_y = from.y as i32 + goff.1;
            let to_x = to.x as i32 + goff.0;
            let to_y = to.y as i32 + goff.1;

            log::debug!(
                "GameNav: drag ({from_x},{from_y})->({to_x},{to_y}), remaining=({:.0},{:.0})",
                remaining.x,
                remaining.y
            );

            if !mouse_drag(from_x, from_y, to_x, to_y, 500) {
                success = false;
                break;
            }

            remaining -= stroke;

            // Pause between strokes.
            if remaining.length() > 10.0 {
                std::thread::sleep(Duration::from_millis(200));
            }
        }

        // Restore mouse to solver window.
        if let Some((rx, ry)) = restore_pos {
            std::thread::sleep(Duration::from_millis(100));
            if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
                if let Err(e) = enigo.move_mouse(rx, ry, Coordinate::Abs) {
                    log::error!("Failed to restore mouse position: {e}");
                }
            }
        }

        // Don't assume we arrived — let the next detection cycle determine
        // where we actually ended up.
        success
    }

    /// Rebuild the map silhouette in a background thread (call when map changes).
    pub fn update_map(&mut self, map: &Map) {
        self.detect_status = "Building map silhouette...".into();
        let sil_result = Arc::new(Mutex::new(None::<MapSilhouette>));
        let sil_result_clone = Arc::clone(&sil_result);
        // Clone the positions we need — Map isn't Send.
        let positions: Vec<HexPos> = map.iter_tile_positions().collect();
        std::thread::spawn(move || {
            let sil = MapSilhouette::from_positions(&positions);
            *sil_result_clone.lock().unwrap() = Some(sil);
        });
        self.pending_silhouette = Some(sil_result);
    }

    pub fn game_center(&self) -> Option<WorldPos> {
        self.game_center
    }

    pub fn set_game_center(&mut self, center: WorldPos) {
        self.game_center = Some(center);
    }

    /// Read camera_pos.txt and update camera state if changed.
    fn poll_camera_file(&mut self) {
        let contents = match std::fs::read_to_string(CAMERA_POS_FILE) {
            Ok(c) => c,
            Err(_) => return,
        };
        if contents == self.last_camera_file {
            return;
        }
        self.last_camera_file = contents.clone();

        if let Some(state) = UnityCameraState::parse(&contents) {
            // camera_pos.txt reports CameraParent position (ground look-at point).
            // Unity coordinates are half our world coordinates (game hex spacing = 0.75, ours = 1.5).
            let look_at = WorldPos::new(state.pos.x * 2.0, state.pos.z * 2.0);

            // CameraParent eulerAngles.x is the pitch from horizontal.
            let pitch = std::f32::consts::FRAC_PI_2 - state.pitch_deg.to_radians(); // from vertical
            let yaw = state.yaw_deg.to_radians();
            let fov_y = state.fov_deg.to_radians();

            // anchor_z is CameraAnchor.localPosition.z (negative, e.g. -10 at default zoom).
            // GameCamera.distance is a projection parameter: at anchor_z=-10, distance=96.
            let distance = state.anchor_z.abs() * (96.0 / 10.0);

            self.camera.look_at = look_at;
            self.camera.distance = distance;
            self.camera.pitch = pitch;
            self.camera.yaw = yaw;
            self.camera.fov_y = fov_y;
            self.game_center = Some(look_at);
            self.last_unity_state = Some(state);
            self.detect_status = format!("Camera: ({:.0}, {:.0})", look_at.x(), look_at.y());
        }
    }

    /// Write camera_set.txt to move the game's CameraParent to a world position.
    /// Unity coordinates are half our world coordinates.
    fn write_camera_set(&self, target: WorldPos) {
        let line = format!("{:.4} 0.0 {:.4}", target.x() / 2.0, target.y() / 2.0);
        if let Err(e) = std::fs::write(CAMERA_SET_FILE, line) {
            log::warn!("Failed to write camera_set.txt: {e}");
        }
    }
}

struct WindowInfo {
    game_window_id: u64,
    game_output_offset: (i32, i32),
    solver_output_offset: (i32, i32),
}

fn find_windows() -> Option<WindowInfo> {
    let mut socket = Socket::connect().ok()?;

    // Get windows list.
    let reply = socket.send(Request::Windows).ok()?;
    let windows = match reply {
        Ok(Response::Windows(w)) => w,
        _ => return None,
    };

    // Find game window.
    let (game_id, game_ws) = windows.iter().find_map(|w| {
        let app_id = w.app_id.as_deref().unwrap_or("");
        if app_id.contains("1455840") {
            Some((w.id, w.workspace_id?))
        } else {
            None
        }
    })?;

    // Find solver window (winit).
    let solver_ws = windows.iter().find_map(|w| {
        let title = w.title.as_deref().unwrap_or("");
        if title.contains("winit") || title.contains("Dorfromantik viewer") {
            w.workspace_id
        } else {
            None
        }
    });

    // Get workspaces to map workspace_id -> output name.
    let reply = socket.send(Request::Workspaces).ok()?;
    let workspaces = match reply {
        Ok(Response::Workspaces(ws)) => ws,
        _ => return None,
    };

    let ws_to_output = |ws_id: u64| -> Option<String> {
        workspaces.iter().find_map(|ws| {
            if ws.id == ws_id {
                ws.output.clone()
            } else {
                None
            }
        })
    };

    // Get outputs to map output name -> logical position.
    let reply = socket.send(Request::Outputs).ok()?;
    let outputs = match reply {
        Ok(Response::Outputs(o)) => o,
        _ => return None,
    };

    let output_offset = |name: &str| -> (i32, i32) {
        outputs
            .get(name)
            .and_then(|o| o.logical.as_ref())
            .map(|l| (l.x, l.y))
            .unwrap_or((0, 0))
    };

    let game_output = ws_to_output(game_ws)?;
    let solver_output = solver_ws.and_then(&ws_to_output);

    Some(WindowInfo {
        game_window_id: game_id,
        game_output_offset: output_offset(&game_output),
        solver_output_offset: solver_output
            .map(|name| output_offset(&name))
            .unwrap_or((0, 0)),
    })
}

fn focus_window(id: u64) {
    if let Ok(mut socket) = Socket::connect() {
        let action = Action::FocusWindow { id };
        if let Err(e) = socket.send(Request::Action(action)) {
            log::error!("Failed to focus window {id}: {e}");
        }
    }
}

fn mouse_drag(from_x: i32, from_y: i32, to_x: i32, to_y: i32, duration_ms: u32) -> bool {
    let mut enigo = match Enigo::new(&Settings::default()) {
        Ok(e) => e,
        Err(e) => {
            log::error!("Failed to create enigo instance: {e}");
            return false;
        }
    };

    // Move to start.
    if enigo.move_mouse(from_x, from_y, Coordinate::Abs).is_err() {
        return false;
    }
    std::thread::sleep(Duration::from_millis(50));

    // Middle button down.
    if enigo.button(Button::Middle, Direction::Press).is_err() {
        return false;
    }
    std::thread::sleep(Duration::from_millis(50));

    // Interpolated move.
    let steps = 20;
    let step_delay = Duration::from_millis(duration_ms as u64 / steps as u64);
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let x = from_x as f32 + (to_x - from_x) as f32 * t;
        let y = from_y as f32 + (to_y - from_y) as f32 * t;
        let _ = enigo.move_mouse(x as i32, y as i32, Coordinate::Abs);
        std::thread::sleep(step_delay);
    }

    std::thread::sleep(Duration::from_millis(50));

    // Middle button up.
    if enigo.button(Button::Middle, Direction::Release).is_err() {
        return false;
    }

    std::thread::sleep(Duration::from_millis(50));
    true
}
