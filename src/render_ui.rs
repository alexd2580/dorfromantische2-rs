use crate::App;

pub fn render_ui(
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
            let slider = egui::Slider::new(&mut app.inv_scale.target, 5.0..=500.0).text("Zoom out");
            if ui.add(slider).changed() {
                app.inv_scale.set(app.inv_scale.target);
            }
            if ui.button("Zoom fit").clicked() {
                app.zoom_fit();
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
            ui.label("Show");
            ui.label("Split");
            ui.label("Matched");
            ui.label("Group diffs");
            ui.end_row();

            for (rank, score) in app.best_placements.iter_usable() {
                ui.checkbox(&mut app.show_placements[rank], "");
                ui.label(format!("{}", score.split));
                ui.label(format!("{}", score.matching_edges));
                let group_diffs = score
                    .group_edge_alterations
                    .iter()
                    .map(|g| format!("{} => {}", g.group_size, g.diff))
                    .collect::<Vec<_>>();
                ui.label(format!("{:?}", group_diffs));
                ui.end_row();
            }
        });
        ui.add_space(10.0);
    });

    // Tooltip (hovering close to mouse)
    if *show_tooltip {
        if let Some(segment_index) = app.hover_segment {
            // let tile = app.map.tile(tile_id).unwrap();
            // let response = egui::Window::new(format!("Tile {tile_id}"))
            //     .movable(false)
            //     .collapsible(false)
            //     .resizable(false)
            //     .current_pos((app.mouse_position.x + 50.0, app.mouse_position.y - 50.0))
            //     .show(ctx, |ui| {
            //         egui::Grid::new("tile_data").show(ui, |ui| {
            //             ui.label("Axial position");
            //             ui.label(format!("x: {}, y: {}", app.hover_pos.x, app.hover_pos.y));
            //             ui.end_row();
            //
            //             if !tile.segments.is_empty() {
            //                 ui.label("Segments");
            //                 ui.end_row();
            //
            //                 ui.label("Terrain");
            //                 ui.label("Form");
            //                 ui.label("Group");
            //                 ui.end_row();
            //
            //                 for (segment_id, segment) in tile.segments.iter().enumerate() {
            //                     ui.label(format!("{:?}", segment.terrain));
            //                     ui.label(format!("{:?}", segment.form));
            //                     let group = app.map.group_of(tile_id, segment_id);
            //                     ui.label(format!("{group}",));
            //                     ui.end_row();
            //                 }
            //             }
            //
            //             ui.label("Quest");
            //             ui.label(format!("{:?}", tile.quest_tile));
            //             ui.end_row();
            //         });
            //     });

            // if let Some(group_id) = app.hover_group {
            //     let group = &app.map.group(group_id);
            //     let tile_rect = response.unwrap().response.rect;
            //     let pos = (tile_rect.min.x, tile_rect.max.y + 10.0);
            //     egui::Window::new(format!("Group {group_id}"))
            //         .movable(false)
            //         .collapsible(false)
            //         .resizable(false)
            //         .current_pos(pos)
            //         .show(ctx, |ui| {
            //             egui::Grid::new("group_data").show(ui, |ui| {
            //                 ui.label("Segment count");
            //                 ui.label(format!("{}", group.segments.len()));
            //                 ui.end_row();
            //                 ui.label("Closed");
            //                 ui.label(if group.open_edges.is_empty() {
            //                     "Yes"
            //                 } else {
            //                     "No"
            //                 });
            //                 ui.end_row();
            //             });
            //         });
            // }
        } else {
            // let rotation_scores = (0..6).map(|rotation| app.map.score_at(app.hover_pos, rotation));
            // egui::Window::new("Placement score")
            //     .movable(false)
            //     .collapsible(false)
            //     .resizable(false)
            //     .current_pos((app.mouse_position.x + 50.0, app.mouse_position.y - 50.0))
            //     .show(ctx, |ui| {
            //         for res in rotation_scores {
            //             if let Some((matching_edges, probability_score)) = res {
            //                 ui.label(format!("{matching_edges} {probability_score}",));
            //             }
            //         }
            //     });
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
