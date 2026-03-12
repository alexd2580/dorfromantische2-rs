use egui::{Color32, Label, Pos2, Sense};

use crate::{data::Terrain, App};

/// Top panel with title and some menus.
fn render_top_panel(app: &mut App, ctx: &egui::Context, sidebar_expanded: &mut bool) {
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
}

/// Main config panel.
fn render_side_panel(
    app: &mut App,
    ctx: &egui::Context,
    sidebar_expanded: &mut bool,
    show_tooltip: &mut bool,
    show_groups: &mut bool,
    show_biggest_groups: &mut bool,
) {
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

        ui.label(egui::RichText::new("Overlays").size(20.0).underline());
        ui.checkbox(show_tooltip, "Show tooltip");
        ui.checkbox(show_groups, "Show quest labels");
        ui.checkbox(show_biggest_groups, "Show biggest groups");
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
            ui.label("Pos");
            ui.label("Edges");
            ui.label("Bonus");
            ui.label("Diff");
            ui.label("Crowd");
            ui.label("Ends");
            ui.label("Groups");
            ui.end_row();

            let max_edges = app
                .best_placements
                .iter_usable()
                .next()
                .map(|(_, s)| s.matching_edges);
            let mut clicked_row = None;
            for (rank, score) in app.best_placements.iter_all() {
                let is_best = max_edges == Some(score.matching_edges);
                let color = if is_best {
                    Color32::WHITE
                } else {
                    Color32::from_rgb(120, 120, 120)
                };

                ui.checkbox(&mut app.show_placements[rank], "");
                let text = egui::RichText::new(format!("{}", score.pos)).color(color);
                if ui.add(Label::new(text).sense(Sense::click())).clicked() {
                    clicked_row = Some(score.pos);
                }
                ui.label(egui::RichText::new(format!("{}", score.matching_edges)).color(color));
                let bonus_color = if score.neighbor_bonus > 0 {
                    Color32::from_rgb(80, 200, 80)
                } else {
                    color
                };
                ui.label(
                    egui::RichText::new(format!("{}", score.neighbor_bonus)).color(bonus_color),
                );
                ui.label(
                    egui::RichText::new(format!("{}", score.connection_difficulty)).color(color),
                );
                let crowd_color = if score.crowding > 0 {
                    Color32::from_rgb(220, 80, 80)
                } else {
                    color
                };
                ui.label(egui::RichText::new(format!("{}", score.crowding)).color(crowd_color));
                let end_color = if score.open_end_delta > 0 {
                    Color32::from_rgb(220, 80, 80)
                } else if score.open_end_delta < 0 {
                    Color32::from_rgb(80, 200, 80)
                } else {
                    color
                };
                ui.label(
                    egui::RichText::new(format!("{:+}", score.open_end_delta)).color(end_color),
                );
                let group_diffs = score
                    .group_edge_alterations
                    .iter()
                    .map(|g| format!("{} => {}", g.group_size, g.diff))
                    .collect::<Vec<_>>();
                ui.label(egui::RichText::new(format!("{group_diffs:?}")).color(color));
                ui.end_row();
            }
            if let Some(pos) = clicked_row {
                app.goto(pos);
            }
        });
        ui.add_space(10.0);
    });
}

fn render_tooltip(app: &App, ctx: &egui::Context, show_tooltip: &mut bool) {
    if !*show_tooltip {
        return;
    }

    let segment_index = match app.hover_segment {
        Some(i) => i,
        None => return,
    };

    let segment = app.map.segment(segment_index);
    let group_index = app.group_assignments.group_of(segment_index);

    egui::Window::new("Tile info")
        .movable(false)
        .collapsible(false)
        .resizable(false)
        .current_pos((app.mouse_position.x + 30.0, app.mouse_position.y - 30.0))
        .show(ctx, |ui| {
            egui::Grid::new("tooltip_grid").show(ui, |ui| {
                ui.label("Position");
                ui.label(format!("{}, {}", app.hover_pos.x, app.hover_pos.y));
                ui.end_row();

                ui.label("Terrain");
                ui.label(format!("{:?}", segment.terrain));
                ui.end_row();

                if let Some(group_index) = group_index {
                    let group = &app.group_assignments.groups[group_index];
                    ui.label("Group");
                    ui.label(format!(
                        "{} units, {}",
                        group.unit_count,
                        if group.is_closed() { "closed" } else { "open" }
                    ));
                    ui.end_row();

                    for (quest, remaining) in group.remaining_per_quest() {
                        use crate::map::QuestType;
                        let suffix = match quest.quest_type {
                            QuestType::MoreThan => "+",
                            QuestType::Exact => "",
                            QuestType::Unknown => "?",
                        };
                        ui.label("Quest");
                        ui.label(format!(
                            "{:?} {}/{} ({remaining}{suffix})",
                            quest.terrain, group.unit_count, quest.target_value,
                        ));
                        ui.end_row();
                    }
                }
            });
        });
}

fn terrain_color(terrain: Terrain) -> Color32 {
    match terrain {
        Terrain::Missing => Color32::from_rgb(80, 80, 80),
        Terrain::Empty => Color32::from_rgb(180, 180, 160),
        Terrain::House => Color32::from_rgb(200, 100, 80),
        Terrain::Forest => Color32::from_rgb(50, 140, 50),
        Terrain::Wheat => Color32::from_rgb(220, 200, 80),
        Terrain::Rail => Color32::from_rgb(100, 100, 100),
        Terrain::River => Color32::from_rgb(60, 120, 220),
        Terrain::Lake => Color32::from_rgb(80, 150, 240),
        Terrain::Station => Color32::from_rgb(160, 130, 100),
    }
}

fn render_next_tile(app: &App, ctx: &egui::Context) {
    if app.map.next_tile.is_empty() {
        return;
    }

    let size = 80.0;
    egui::Area::new("next_tile")
        .anchor(egui::Align2::RIGHT_BOTTOM, (-20.0, -20.0))
        .show(ctx, |ui| {
            let has_quest = app.map.next_tile_quest.is_some();
            let height = size * 2.0 + if has_quest { 50.0 } else { 30.0 };
            let (response, painter) =
                ui.allocate_painter(egui::vec2(size * 2.0 + 10.0, height), Sense::hover());
            let center = Pos2::new(response.rect.center().x, response.rect.min.y + 15.0 + size);

            // Draw label.
            painter.text(
                Pos2::new(center.x, response.rect.min.y + 8.0),
                egui::Align2::CENTER_TOP,
                "Next tile",
                egui::FontId::proportional(14.0),
                Color32::WHITE,
            );

            // Hex vertices for a pointy-top hex (rotation 0 = north).
            // Vertices go from top, clockwise.
            let hex_vertices: Vec<Pos2> = (0..6)
                .map(|i| {
                    let angle =
                        std::f32::consts::FRAC_PI_3 * i as f32 - std::f32::consts::FRAC_PI_2;
                    Pos2::new(center.x + size * angle.cos(), center.y + size * angle.sin())
                })
                .collect();

            // Draw each side as a triangle wedge from center to two adjacent vertices.
            // Rotation 0 = north = between vertex 5 and vertex 0 (top-left to top-right).
            for side in 0..6 {
                let terrain = app.map.rendered_next_tile[side];
                let color = terrain_color(terrain);

                // Vertex indices: side 0 (north) spans from vertex 5 to vertex 0,
                // side 1 spans vertex 0 to vertex 1, etc.
                let v0 = hex_vertices[(side + 5) % 6];
                let v1 = hex_vertices[side % 6];

                let triangle = egui::Shape::convex_polygon(
                    vec![center, v0, v1],
                    color,
                    egui::Stroke::new(1.5, Color32::from_rgb(40, 40, 40)),
                );
                painter.add(triangle);
            }

            // Draw hex outline on top.
            let mut outline = hex_vertices.clone();
            outline.push(hex_vertices[0]);
            painter.add(egui::Shape::line(
                outline,
                egui::Stroke::new(2.0, Color32::from_rgb(40, 40, 40)),
            ));

            // Draw quest info below the hex if present.
            if let Some(quest) = &app.map.next_tile_quest {
                use crate::map::QuestType;
                let suffix = match quest.quest_type {
                    QuestType::MoreThan => "+",
                    QuestType::Exact => "",
                    QuestType::Unknown => "?",
                };
                let text = format!("{:?} {}{}", quest.terrain, quest.target_value, suffix);
                painter.text(
                    Pos2::new(center.x, response.rect.max.y - 8.0),
                    egui::Align2::CENTER_BOTTOM,
                    text,
                    egui::FontId::proportional(13.0),
                    Color32::WHITE,
                );
            }
        });
}

/// Render always-visible quest labels centered on each group that has active quests.
/// `visible_rect` is the area not covered by panels (sidebar, top bar).
fn render_group_quest_labels(app: &App, ctx: &egui::Context, visible_rect: egui::Rect) {
    if visible_rect.width() <= 0.0 || visible_rect.height() <= 0.0 {
        return;
    }

    for (group_idx, group) in app.group_assignments.groups.iter().enumerate() {
        // Only show labels for groups with active quests.
        let active_quests: Vec<_> = group
            .remaining_per_quest()
            .into_iter()
            .filter(|(q, _)| q.active)
            .collect();
        if active_quests.is_empty() {
            continue;
        }

        // Convert precomputed hex centroid to pixel coordinates.
        let centroid_hex = glam::IVec2::new(
            group.centroid.x.round() as i32,
            group.centroid.y.round() as i32,
        );
        let centroid = app.hex_to_pixel(centroid_hex);

        // Skip if centroid is outside the visible (non-panel) area.
        let margin = 50.0;
        if centroid.x < visible_rect.min.x - margin
            || centroid.y < visible_rect.min.y - margin
            || centroid.x > visible_rect.max.x + margin
            || centroid.y > visible_rect.max.y + margin
        {
            continue;
        }

        // Build the label text showing remaining count per quest.
        let text = active_quests
            .iter()
            .map(|(quest, remaining)| {
                use crate::map::QuestType;
                let suffix = match quest.quest_type {
                    QuestType::MoreThan => "+",
                    QuestType::Exact => "",
                    QuestType::Unknown => "?",
                };
                format!("{:?} {remaining}{suffix}", quest.terrain)
            })
            .collect::<Vec<_>>()
            .join("\n");

        let id = egui::Id::new(("group_quest_label", group_idx));
        egui::Area::new(id)
            .order(egui::Order::Background)
            .fixed_pos(Pos2::new(centroid.x, centroid.y))
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .fill(Color32::from_black_alpha(180))
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(text).color(Color32::WHITE));
                    });
            });
    }
}

fn render_biggest_groups(app: &App, ctx: &egui::Context, visible_rect: egui::Rect) {
    if visible_rect.width() <= 0.0 || visible_rect.height() <= 0.0 {
        return;
    }

    use std::collections::HashMap;
    // Find the biggest group (by segment count) for each terrain type.
    let mut biggest: HashMap<Terrain, (usize, &crate::group::Group)> = HashMap::new();
    for (idx, group) in app.group_assignments.groups.iter().enumerate() {
        if group.is_closed() {
            continue;
        }
        let size = group.segment_indices.len();
        let entry = biggest.entry(group.terrain).or_insert((idx, group));
        if size > entry.1.segment_indices.len() {
            *entry = (idx, group);
        }
    }

    for (terrain, (group_idx, group)) in &biggest {
        let centroid_hex = glam::IVec2::new(
            group.centroid.x.round() as i32,
            group.centroid.y.round() as i32,
        );
        let centroid = app.hex_to_pixel(centroid_hex);

        let margin = 50.0;
        if centroid.x < visible_rect.min.x - margin
            || centroid.y < visible_rect.min.y - margin
            || centroid.x > visible_rect.max.x + margin
            || centroid.y > visible_rect.max.y + margin
        {
            continue;
        }

        let size = group.segment_indices.len();
        let text = format!("{terrain:?}: {size} segments, {} units", group.unit_count);

        let id = egui::Id::new(("biggest_group", *group_idx));
        egui::Area::new(id)
            .order(egui::Order::Background)
            .fixed_pos(Pos2::new(centroid.x, centroid.y))
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .fill(Color32::from_black_alpha(200))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(text)
                                .color(Color32::from_rgb(255, 215, 0))
                                .size(13.0),
                        );
                    });
            });
    }
}

fn render_map_loader(app: &mut App, ctx: &egui::Context) {
    if app.map_loader.in_progress() {
        egui::Area::new("my_area")
            .anchor(egui::Align2::RIGHT_BOTTOM, (-50.0, -50.0))
            .show(ctx, |ui| {
                ui.add(egui::Spinner::default().size(40.0));
            });
    }
}

pub fn render_ui(
    app: &mut App,
    ctx: &egui::Context,
    sidebar_expanded: &mut bool,
    show_tooltip: &mut bool,
    show_groups: &mut bool,
    show_biggest_groups: &mut bool,
) {
    render_top_panel(app, ctx, sidebar_expanded);
    render_side_panel(
        app,
        ctx,
        sidebar_expanded,
        show_tooltip,
        show_groups,
        show_biggest_groups,
    );
    // Available rect after panels have claimed their space.
    let visible_rect = ctx.available_rect();
    let full_rect = ctx.screen_rect();
    if full_rect.width() > 0.0 && full_rect.height() > 0.0 {
        app.visible_fraction = glam::Vec2::new(
            visible_rect.width() / full_rect.width(),
            visible_rect.height() / full_rect.height(),
        );
    }
    render_tooltip(app, ctx, show_tooltip);
    if *show_groups {
        render_group_quest_labels(app, ctx, visible_rect);
    }
    if *show_biggest_groups {
        render_biggest_groups(app, ctx, visible_rect);
    }
    render_map_loader(app, ctx);
    render_next_tile(app, ctx);
}
