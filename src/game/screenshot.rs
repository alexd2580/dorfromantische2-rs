use egui::ColorImage;
use libwayshot::WayshotConnection;
use niri_ipc::socket::Socket;
use niri_ipc::{Request, Response};

/// Find the output (monitor) name that the Dorfromantik game window is on.
fn find_game_output() -> Option<String> {
    let mut socket = Socket::connect().ok()?;

    // Get windows list.
    let reply = socket.send(Request::Windows).ok()?;
    let windows = match reply {
        Ok(Response::Windows(w)) => w,
        _ => return None,
    };

    // Find the Dorfromantik window and its workspace.
    let workspace_id = windows.iter().find_map(|w| {
        let title = w.title.as_deref().unwrap_or("");
        let app_id = w.app_id.as_deref().unwrap_or("");
        if title.contains("Dorfromantik")
            || app_id.contains("dorfromantik")
            || app_id.contains("1455840")
        {
            w.workspace_id
        } else {
            None
        }
    })?;

    // Get workspaces to find the output name for this workspace.
    let reply = socket.send(Request::Workspaces).ok()?;
    let workspaces = match reply {
        Ok(Response::Workspaces(ws)) => ws,
        _ => return None,
    };

    workspaces.iter().find_map(|ws| {
        if ws.id == workspace_id {
            ws.output.clone()
        } else {
            None
        }
    })
}

/// Capture a screenshot of the Dorfromantik game window.
/// Falls back to full screen if the game window can't be found.
pub fn capture_screen() -> Option<ColorImage> {
    let wayshot = WayshotConnection::new().ok()?;

    let img = if let Some(output_name) = find_game_output() {
        // Find the matching output in wayshot's output list.
        let output_info = wayshot
            .get_all_outputs()
            .iter()
            .find(|o| o.name == output_name);
        match output_info {
            Some(info) => wayshot.screenshot_single_output(info, false).ok()?,
            None => {
                log::warn!(
                    "Output '{}' not found in wayshot outputs, capturing all",
                    output_name
                );
                wayshot.screenshot_all(false).ok()?
            }
        }
    } else {
        log::warn!("Could not find Dorfromantik window, capturing full screen");
        wayshot.screenshot_all(false).ok()?
    };

    let rgba = img.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let pixels = rgba.into_raw();
    Some(ColorImage::from_rgba_unmultiplied(size, &pixels))
}
