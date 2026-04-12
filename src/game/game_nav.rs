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
use super::screenshot;
use super::viewport_detect::{self, DetectResult, MapSilhouette};
use crate::coords::WorldPos;
use crate::map::Map;

/// State for tracking viewport changes and triggering game navigation.
pub struct GameNav {
    pub camera: GameCamera,
    last_solver_center: WorldPos,
    last_change: Instant,
    pending: bool,
    /// Estimated game viewport center (world coords).
    game_center: Option<WorldPos>,
    settle_time: Duration,
    pub enabled: bool,
    /// Whether periodic screenshot-based viewport detection is active.
    pub detect_enabled: bool,
    /// Solver's mouse position (absolute screen pixels) for restoration after drag.
    saved_mouse: Option<(i32, i32)>,
    /// Precomputed map silhouette for viewport detection (shared with bg thread).
    map_silhouette: Option<Arc<MapSilhouette>>,
    /// Last time viewport detection was kicked off.
    last_detect: Instant,
    /// Interval between automatic viewport detections.
    detect_interval: Duration,
    /// Result from background detection thread.
    detect_result: Arc<Mutex<Option<DetectResult>>>,
    /// Whether a detection is currently running.
    detect_running: Arc<Mutex<bool>>,
    /// Status message for the UI.
    pub detect_status: String,
    /// Pending map silhouette being built in background.
    pending_silhouette: Option<Arc<Mutex<Option<MapSilhouette>>>>,
    /// Game screen dimensions (width, height) from the last screenshot.
    pub screen_size: (u32, u32),
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
            enabled: false,
            detect_enabled: false,
            saved_mouse: None,
            map_silhouette: None,
            last_detect: Instant::now(),
            detect_interval: Duration::from_secs(5),
            detect_result: Arc::new(Mutex::new(None)),
            detect_running: Arc::new(Mutex::new(false)),
            detect_status: "No map loaded".into(),
            pending_silhouette: None,
            screen_size: (2560, 1440),
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

        // Check for completed silhouette build.
        let sil_ready = self
            .pending_silhouette
            .as_ref()
            .and_then(|p| p.try_lock().ok().and_then(|mut g| g.take()));
        if let Some(sil) = sil_ready {
            self.map_silhouette = Some(Arc::new(sil));
            self.detect_status = "Map loaded, waiting to detect...".into();
            self.pending_silhouette = None;
        }

        // Check for completed background detection.
        if let Some(result) = self.detect_result.lock().unwrap().take() {
            self.game_center = Some(result.center);
            self.camera.look_at = result.center;
            self.screen_size = result.screen_size;
            self.detect_status = format!(
                "Viewport: ({:.0}, {:.0})",
                result.center.x(),
                result.center.y()
            );
        }

        // Kick off periodic detection in background (only when enabled).
        let is_running = *self.detect_running.lock().unwrap();
        if self.detect_enabled
            && self.map_silhouette.is_some()
            && self.last_detect.elapsed() >= self.detect_interval
            && !is_running
        {
            self.last_detect = Instant::now();
            self.detect_status = "Detecting...".into();
            self.spawn_detect();
        }

        if !self.enabled {
            return false;
        }

        let moved = (solver_center.0 - self.last_solver_center.0).length() > 1.0;
        if moved {
            self.last_solver_center = solver_center;
            self.last_change = Instant::now();
            self.pending = true;
            return false;
        }

        if !self.pending || self.last_change.elapsed() < self.settle_time || !mouse_idle {
            return false;
        }

        self.pending = false;
        self.navigate_to(solver_center)
    }

    fn spawn_detect(&self) {
        let sil = match &self.map_silhouette {
            Some(s) => Arc::clone(s),
            None => return,
        };
        let mut cam = self.camera.clone();
        cam.look_at = WorldPos::ZERO; // Unprojection must be origin-relative.
        let result = Arc::clone(&self.detect_result);
        let running = Arc::clone(&self.detect_running);

        *running.lock().unwrap() = true;

        std::thread::spawn(move || {
            let center = std::panic::catch_unwind(|| {
                (|| {
                    let color_image = screenshot::capture_screen()?;
                    let w = color_image.size[0] as u32;
                    let h = color_image.size[1] as u32;
                    let mut rgb = image::RgbImage::new(w, h);
                    for y in 0..h {
                        for x in 0..w {
                            let c = color_image[(x as usize, y as usize)];
                            rgb.put_pixel(x, y, image::Rgb([c.r(), c.g(), c.b()]));
                        }
                    }
                    viewport_detect::detect_viewport(&rgb, &sil, &cam)
                })()
            });

            match center {
                Ok(Some(c)) => {
                    *result.lock().unwrap() = Some(c);
                }
                Ok(None) => {
                    log::warn!("GameNav: detection returned None");
                }
                Err(e) => {
                    log::error!("GameNav: detection panicked: {:?}", e);
                }
            }
            *running.lock().unwrap() = false;
        });
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

    /// Capture a screenshot and detect the game viewport position.
    /// Returns the detected world center, or None.
    pub fn detect_game_viewport(&mut self) -> Option<WorldPos> {
        let sil = self.map_silhouette.as_ref()?;
        let color_image = screenshot::capture_screen()?;
        let w = color_image.size[0] as u32;
        let h = color_image.size[1] as u32;
        let mut rgb = image::RgbImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let c = color_image[(x as usize, y as usize)];
                rgb.put_pixel(x, y, image::Rgb([c.r(), c.g(), c.b()]));
            }
        }
        let mut cam = self.camera.clone();
        cam.look_at = WorldPos::ZERO;
        let result = viewport_detect::detect_viewport(&rgb, sil, &cam)?;
        self.game_center = Some(result.center);
        self.camera.look_at = result.center;
        self.screen_size = result.screen_size;
        Some(result.center)
    }

    pub fn game_center(&self) -> Option<WorldPos> {
        self.game_center
    }

    pub fn set_game_center(&mut self, center: WorldPos) {
        self.game_center = Some(center);
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
