#![allow(dead_code)]

use std::process::Command;

use egui::ColorImage;

/// Find the output (monitor) that the Dorfromantik game window is on.
fn find_game_output() -> Option<String> {
    // Get windows list.
    let win_output = Command::new("niri")
        .args(["msg", "-j", "windows"])
        .output()
        .ok()?;
    if !win_output.status.success() {
        return None;
    }
    let windows: Vec<serde_json::Value> = serde_json::from_slice(&win_output.stdout).ok()?;

    // Find the Dorfromantik window and its workspace.
    let workspace_id = windows.iter().find_map(|w| {
        let title = w.get("title")?.as_str()?;
        let app_id = w.get("app_id").and_then(|a| a.as_str()).unwrap_or("");
        if title.contains("Dorfromantik")
            || app_id.contains("dorfromantik")
            || app_id.contains("1455840")
        {
            w.get("workspace_id")?.as_u64()
        } else {
            None
        }
    })?;

    // Get workspaces to find the output name for this workspace.
    let ws_output = Command::new("niri")
        .args(["msg", "-j", "workspaces"])
        .output()
        .ok()?;
    if !ws_output.status.success() {
        return None;
    }
    let workspaces: Vec<serde_json::Value> = serde_json::from_slice(&ws_output.stdout).ok()?;

    workspaces.iter().find_map(|ws| {
        let id = ws.get("id")?.as_u64()?;
        if id == workspace_id {
            ws.get("output")?.as_str().map(|s| s.to_string())
        } else {
            None
        }
    })
}

/// Capture a screenshot of the Dorfromantik game window.
/// Falls back to full screen if the game window can't be found.
pub fn capture_screen() -> Option<ColorImage> {
    let mut cmd = Command::new("grim");
    cmd.args(["-t", "png"]);

    if let Some(output_name) = find_game_output() {
        cmd.args(["-o", &output_name]);
    } else {
        eprintln!("Could not find Dorfromantik window, capturing full screen");
    }

    cmd.arg("-");

    let output = cmd.output().ok()?;

    if !output.status.success() {
        eprintln!("grim failed: {}", String::from_utf8_lossy(&output.stderr));
        return None;
    }

    let img = image::load_from_memory_with_format(&output.stdout, image::ImageFormat::Png)
        .ok()?
        .to_rgba8();

    let size = [img.width() as usize, img.height() as usize];
    let pixels = img.into_raw();
    Some(ColorImage::from_rgba_unmultiplied(size, &pixels))
}
