use egui::{Color32, Label, Pos2, Sense};

use crate::{
    data::Terrain, file_watcher::FileWatcher, game_data::GameData, render::camera::Camera,
};

use super::input_state::InputState;
use super::ui_state::{ClosedGroupStyle, QuestDisplay, SectionStyle, TooltipMode, UiState};

/// Top panel with title and some menus.
fn render_top_panel(ui_state: &mut UiState, file_watcher: &mut FileWatcher, ctx: &egui::Context) {
    egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label("Dorfromantik viewer");
            if ui
                .add_enabled(
                    !file_watcher.file_choose_dialog.is_open(),
                    egui::Button::new("Load file"),
                )
                .clicked()
            {
                file_watcher.file_choose_dialog.open();
            }
            ui.toggle_value(&mut ui_state.sidebar_expanded, "Visual settings");
            ui.toggle_value(&mut ui_state.show_tile_frequencies, "Tile frequencies");
            ui.separator();
            ui.label("Camera:");
            use super::ui_state::CameraMode;
            ui.selectable_value(&mut ui_state.camera_mode, CameraMode::Off, "Off");
            ui.selectable_value(&mut ui_state.camera_mode, CameraMode::TrackGame, "Track");
            ui.selectable_value(&mut ui_state.camera_mode, CameraMode::Duplex, "Duplex");
        });
    });
}

/// Main config panel.
fn render_side_panel(
    data: &GameData,
    camera: &mut Camera,
    ui_state: &mut UiState,
    ctx: &egui::Context,
) {
    egui::SidePanel::left("left_panel").show_animated(ctx, ui_state.sidebar_expanded, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.label(egui::RichText::new("Orientation").size(20.0).underline());
            ui.horizontal(|ui| {
                ui.label("Goto");
                let size = ui.available_size();

                let edit_x = egui::TextEdit::singleline(&mut ui_state.goto_x);
                ui.add_sized((size.x / 3.0, size.y), edit_x);

                let edit_y = egui::TextEdit::singleline(&mut ui_state.goto_y);
                let response = ui.add_sized((size.x / 3.0, size.y), edit_y);

                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if let Some(pos) = ui_state.parse_goto() {
                        camera.goto(pos);
                    }
                }
            });
            ui.horizontal(|ui| {
                let slider =
                    egui::Slider::new(&mut camera.inv_scale.target, 5.0..=500.0).text("Zoom out");
                if ui.add(slider).changed() {
                    camera.inv_scale.set(camera.inv_scale.target);
                }
                if ui.button("Zoom fit").clicked() {
                    camera.zoom_fit(&data.map);
                }
            });
            ui.add_space(10.0);

            ui.label(egui::RichText::new("Overlays").size(20.0).underline());
            ui.label("Hover info");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut ui_state.tooltip_mode, TooltipMode::None, "None");
                ui.selectable_value(&mut ui_state.tooltip_mode, TooltipMode::Group, "Group");
                ui.selectable_value(
                    &mut ui_state.tooltip_mode,
                    TooltipMode::Placement,
                    "Placement",
                );
                ui.selectable_value(&mut ui_state.tooltip_mode, TooltipMode::Chance, "Chance");
            });
            ui.checkbox(&mut ui_state.show_biggest_groups, "Show biggest groups");
            ui.checkbox(
                &mut ui_state.show_imperfect_tiles,
                "Highlight imperfect tiles",
            );
            ui.add_space(10.0);

            ui.label(egui::RichText::new("Section style").size(20.0).underline());
            ui.selectable_value(
                &mut ui_state.section_style,
                SectionStyle::Terrain,
                "Color by terrain type",
            );
            ui.selectable_value(
                &mut ui_state.section_style,
                SectionStyle::GroupStatic,
                "Color by group statically",
            );
            ui.selectable_value(
                &mut ui_state.section_style,
                SectionStyle::GroupDynamic,
                "Color by group dynamically",
            );
            ui.selectable_value(
                &mut ui_state.section_style,
                SectionStyle::Texture,
                "Color by texture",
            );
            ui.selectable_value(
                &mut ui_state.section_style,
                SectionStyle::RailRiverOnly,
                "Color rail/river only",
            );
            ui.add_space(10.0);

            ui.label(
                egui::RichText::new("Group display options")
                    .size(20.0)
                    .underline(),
            );
            ui.label("Closed groups");
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut ui_state.closed_group_style,
                    ClosedGroupStyle::Show,
                    "Show",
                );
                ui.selectable_value(
                    &mut ui_state.closed_group_style,
                    ClosedGroupStyle::Dim,
                    "Dim",
                );
                ui.selectable_value(
                    &mut ui_state.closed_group_style,
                    ClosedGroupStyle::Hide,
                    "Hide",
                );
            });
            ui.label("Quest labels");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut ui_state.quest_display, QuestDisplay::None, "None");
                ui.selectable_value(&mut ui_state.quest_display, QuestDisplay::Min, "Min");
                ui.selectable_value(&mut ui_state.quest_display, QuestDisplay::Easy, "Easy");
                ui.selectable_value(&mut ui_state.quest_display, QuestDisplay::All, "All");
            });
            ui.checkbox(
                &mut ui_state.highlight_hovered_group,
                "Highlight hovered group",
            );
            ui.add_space(10.0);

            ui.label(
                egui::RichText::new("Placement display")
                    .size(20.0)
                    .underline(),
            );
            // Select/unselect all.
            let all_count = data
                .best_placements
                .iter_all()
                .len()
                .min(crate::best_placements::MAX_SHOWN_PLACEMENTS);
            let any_selected = ui_state.show_placements[..all_count].iter().any(|&v| v);
            let mut select_all = any_selected;
            if ui.checkbox(&mut select_all, "Select all").changed() {
                for v in &mut ui_state.show_placements[..all_count] {
                    *v = select_all;
                }
            }

            egui::Grid::new("placement_options").show(ui, |ui| {
                ui.label("Show");
                ui.label("Pos");
                ui.label("Edges");
                ui.label("Diff");
                ui.label("Bonus");
                ui.label("Crowd");
                ui.label("Fit%");
                ui.label("Group");
                ui.label("Quest");
                ui.end_row();

                let mut clicked_row = None;
                for (rank, score) in data.best_placements.iter_all() {
                    if rank >= crate::best_placements::MAX_SHOWN_PLACEMENTS {
                        break;
                    }

                    let is_focused = ui_state.focused_placement == Some(score.pos);

                    ui.checkbox(&mut ui_state.show_placements[rank], "");

                    // All remaining cells in this row are clickable for navigation.
                    let row_color = if is_focused {
                        Color32::from_rgb(100, 150, 255)
                    } else {
                        Color32::WHITE
                    };
                    let pos_text = egui::RichText::new(format!("{}", score.pos)).color(row_color);
                    if ui.add(Label::new(pos_text).sense(Sense::click())).clicked() {
                        clicked_row = Some((rank, score.pos));
                    }
                    if ui
                        .add(
                            Label::new(
                                egui::RichText::new(format!("{}", score.matching_edges))
                                    .color(row_color),
                            )
                            .sense(Sense::click()),
                        )
                        .clicked()
                    {
                        clicked_row = Some((rank, score.pos));
                    }
                    // Helper: clickable label that also triggers row navigation.
                    let mut cell = |text: egui::RichText| {
                        if ui.add(Label::new(text).sense(Sense::click())).clicked() {
                            clicked_row = Some((rank, score.pos));
                        }
                    };

                    // Diff
                    let diff_color = if score.connection_difficulty > 0 {
                        Color32::from_rgb(220, 80, 80)
                    } else {
                        row_color
                    };
                    cell(
                        egui::RichText::new(format!("{}", score.connection_difficulty))
                            .color(diff_color),
                    );
                    // Bonus
                    let bonus_color = if score.neighbor_bonus > 0 {
                        Color32::from_rgb(80, 200, 80)
                    } else {
                        row_color
                    };
                    cell(
                        egui::RichText::new(format!("{}", score.neighbor_bonus)).color(bonus_color),
                    );
                    // Crowd
                    let crowd_color = if score.crowding > 0 {
                        Color32::from_rgb(220, 80, 80)
                    } else {
                        row_color
                    };
                    cell(egui::RichText::new(format!("{}", score.crowding)).color(crowd_color));
                    // Fit%
                    let pct = score.fit_chance * 100.0;
                    if pct < 20.0 {
                        let color = if pct > 1.0 {
                            Color32::from_rgb(80, 200, 80)
                        } else if pct > 0.1 {
                            Color32::from_rgb(220, 180, 80)
                        } else {
                            Color32::from_rgb(220, 80, 80)
                        };
                        cell(egui::RichText::new(format!("{pct:.2}")).color(color));
                    } else {
                        cell(egui::RichText::new(""));
                    }

                    // Group/Quest/Progress columns — first effect on this row.
                    let render_effect =
                        |ui: &mut egui::Ui,
                         e: &crate::best_placements::GroupEffect,
                         row_color: Color32,
                         clicked_row: &mut Option<(usize, crate::data::HexPos)>,
                         rank: usize,
                         pos: crate::data::HexPos| {
                            let mut cell = |text: egui::RichText| {
                                if ui.add(Label::new(text).sense(Sense::click())).clicked() {
                                    *clicked_row = Some((rank, pos));
                                }
                            };
                            // Group
                            cell(
                                egui::RichText::new(format!("{:?}#{}", e.terrain, e.rank))
                                    .color(row_color),
                            );

                            if let Some(ref q) = e.quest {
                                let type_label = match q.quest_type {
                                    crate::map::QuestType::MoreThan => ">=",
                                    crate::map::QuestType::Exact => "==",
                                    crate::map::QuestType::Flag => "close",
                                    crate::map::QuestType::Unknown => "?",
                                };
                                let remaining_after = q.target as i64 - q.segments_after as i64;

                                let (text, color) = if q.would_close {
                                    let met = match q.quest_type {
                                        crate::map::QuestType::MoreThan => remaining_after <= 0,
                                        crate::map::QuestType::Exact => remaining_after == 0,
                                        crate::map::QuestType::Flag => true,
                                        _ => false,
                                    };
                                    if met {
                                        (
                                            format!("{type_label} DONE"),
                                            Color32::from_rgb(80, 200, 80),
                                        )
                                    } else {
                                        (
                                            format!("{type_label} CLOSES!"),
                                            Color32::from_rgb(220, 80, 80),
                                        )
                                    }
                                } else if q.quest_type == crate::map::QuestType::Exact
                                    && remaining_after < 0
                                {
                                    (
                                        format!("{type_label} OVER!"),
                                        Color32::from_rgb(220, 80, 80),
                                    )
                                } else {
                                    (format!("{type_label} {remaining_after} left"), row_color)
                                };
                                cell(egui::RichText::new(text).color(color));
                            } else {
                                let edges_after =
                                    e.open_edges_before as i16 + e.open_edge_delta as i16;
                                cell(
                                    egui::RichText::new(format!("{edges_after} edges"))
                                        .color(row_color),
                                );
                            }
                        };

                    // Only show group effects that have quests.
                    let quest_effects: Vec<_> = score
                        .group_effects
                        .iter()
                        .filter(|e| e.quest.is_some())
                        .collect();

                    if let Some(first) = quest_effects.first() {
                        render_effect(ui, first, row_color, &mut clicked_row, rank, score.pos);
                    } else {
                        cell(egui::RichText::new(""));
                        cell(egui::RichText::new(""));
                    }
                    ui.end_row();

                    for e in quest_effects.iter().skip(1) {
                        // Empty cells for Show, Pos, Edges, Diff, Bonus, Crowd, Fit%
                        for _ in 0..7 {
                            ui.label("");
                        }
                        render_effect(ui, e, row_color, &mut clicked_row, rank, score.pos);
                        ui.end_row();
                    }
                }
                if let Some((rank, pos)) = clicked_row {
                    camera.goto(pos);
                    ui_state.show_placements[rank] = true;
                    ui_state.focused_placement = Some(pos);
                }
            });
            ui.add_space(10.0);

            // Groups section.
            egui::CollapsingHeader::new(egui::RichText::new("Groups").size(20.0).underline())
                .default_open(false)
                .show(ui, |ui| {
                    // Collect groups by terrain, sorted by unit count descending.
                    let terrain_order = [
                        Terrain::House,
                        Terrain::Forest,
                        Terrain::Wheat,
                        Terrain::Rail,
                        Terrain::River,
                    ];
                    // Collect groups per terrain.
                    let mut columns: Vec<(Terrain, Vec<(usize, &crate::group::Group)>)> =
                        Vec::new();
                    for &terrain in &terrain_order {
                        let mut groups: Vec<_> = data
                            .group_assignments
                            .groups
                            .iter()
                            .enumerate()
                            .filter(|(_, g)| g.terrain == terrain && !g.is_closed())
                            .collect();
                        groups.sort_by(|a, b| b.1.unit_count.cmp(&a.1.unit_count));
                        if !groups.is_empty() {
                            columns.push((terrain, groups));
                        }
                    }

                    let max_rows = columns.iter().map(|(_, g)| g.len()).max().unwrap_or(0);
                    let mut clicked_group = None;

                    egui::Grid::new("groups_table").show(ui, |ui| {
                        for (terrain, _) in &columns {
                            ui.label(
                                egui::RichText::new(format!("{terrain:?}"))
                                    .color(terrain_color(*terrain))
                                    .strong(),
                            );
                        }
                        ui.end_row();

                        for row in 0..max_rows {
                            for (_, groups) in &columns {
                                if let Some((group_idx, group)) = groups.get(row) {
                                    let is_focused = ui_state.focused_group == Some(*group_idx);
                                    let color = if is_focused {
                                        Color32::from_rgb(100, 150, 255)
                                    } else {
                                        Color32::WHITE
                                    };
                                    let text = egui::RichText::new(format!(
                                        "{} ({})",
                                        group.unit_count,
                                        group.open_edges.len()
                                    ))
                                    .color(color);
                                    if ui.add(Label::new(text).sense(Sense::click())).clicked() {
                                        clicked_group = Some((*group_idx, group.centroid));
                                    }
                                } else {
                                    ui.label("");
                                }
                            }
                            ui.end_row();
                        }
                    });
                    if let Some((group_idx, world)) = clicked_group {
                        camera.goto_world(world);
                        ui_state.focused_group = Some(group_idx);
                    }
                });
            ui.add_space(10.0);
        }); // ScrollArea
    });
}

/// Compute the 6 hex vertices for a pointy-top hex centered at `center` with radius `size`.
fn hex_vertices(center: Pos2, size: f32) -> [Pos2; 6] {
    std::array::from_fn(|i| {
        let angle = std::f32::consts::FRAC_PI_3 * i as f32 - std::f32::consts::FRAC_PI_2;
        Pos2::new(center.x + size * angle.cos(), center.y + size * angle.sin())
    })
}

/// Draw a hex tile by filling each side's wedge with its segment's color,
/// then drawing border lines between different segments to show form structure.
fn draw_hex_segments(
    painter: &egui::Painter,
    center: Pos2,
    vertices: &[Pos2; 6],
    segments: &[crate::data::Segment],
    _stroke: egui::Stroke,
) {
    use crate::data::HEX_SIDES;

    let v = |side: usize| vertices[side % HEX_SIDES];

    // Build side→segment index map.
    let mut side_seg: [Option<usize>; 6] = [None; 6];
    for (seg_idx, seg) in segments.iter().enumerate() {
        for side in seg.rotations() {
            side_seg[side] = Some(seg_idx);
        }
    }

    // Pass 1: fill each side's wedge with its segment color (or empty).
    for (side, &seg) in side_seg.iter().enumerate() {
        let color = seg
            .map(|idx| terrain_color(segments[idx].terrain))
            .unwrap_or_else(|| terrain_color(Terrain::Empty));
        painter.add(egui::Shape::convex_polygon(
            vec![center, v(side + 5), v(side)],
            color,
            egui::Stroke::NONE,
        ));
    }

    // Pass 2: for non-contiguous segments, draw a circle at center to show connection.
    for seg in segments {
        let sides: Vec<usize> = seg.rotations().collect();
        if sides.len() <= 1 {
            continue;
        }
        let is_contiguous = sides.windows(2).all(|w| w[1] == (w[0] + 1) % HEX_SIDES);
        if is_contiguous {
            continue;
        }
        let color = terrain_color(seg.terrain);
        // Radius proportional to hex size (distance from center to vertex).
        let dx = vertices[0].x - center.x;
        let dy = vertices[0].y - center.y;
        let hex_radius = (dx * dx + dy * dy).sqrt();
        painter.circle_filled(center, hex_radius * 0.3, color);
    }

    // Pass 3: draw borders between adjacent sides that belong to different segments.
    let border_color = Color32::from_rgb(30, 30, 30);
    let border_width = 1.5;
    for side in 0..HEX_SIDES {
        let next = (side + 1) % HEX_SIDES;
        if side_seg[side] != side_seg[next] {
            painter.add(egui::Shape::line_segment(
                [center, v(side)],
                egui::Stroke::new(border_width, border_color),
            ));
        }
    }
}

/// Draw a mini hex from segments (for tile frequency display).
fn draw_mini_hex_segments(ui: &mut egui::Ui, segments: &[crate::data::Segment], size: f32) {
    let (response, painter) =
        ui.allocate_painter(egui::vec2(size * 2.0, size * 2.0), Sense::hover());
    let center = response.rect.center();
    let verts = hex_vertices(center, size);
    let stroke = egui::Stroke::new(1.0, Color32::from_rgb(40, 40, 40));
    draw_hex_segments(&painter, center, &verts, segments, stroke);

    // Hex outline.
    let mut outline = verts.to_vec();
    outline.push(verts[0]);
    painter.add(egui::Shape::line(
        outline,
        egui::Stroke::new(1.5, Color32::from_rgb(40, 40, 40)),
    ));
}

fn render_tile_frequencies(data: &GameData, ui_state: &mut UiState, ctx: &egui::Context) {
    if !ui_state.show_tile_frequencies {
        return;
    }
    egui::Window::new("Tile Frequencies")
        .open(&mut ui_state.show_tile_frequencies)
        .default_width(500.0)
        .default_height(600.0)
        .vscroll(true)
        .show(ctx, |ui| {
            let freqs = &data.tile_frequencies;
            ui.label(format!(
                "Total: {} tiles, {} distinct patterns",
                freqs.total_tiles,
                freqs.entries.len()
            ));
            ui.add_space(5.0);

            let hex_size = 20.0;
            egui::Grid::new("tile_freq_grid")
                .min_col_width(hex_size * 2.0 + 4.0)
                .show(ui, |ui| {
                    ui.label("");
                    ui.label("Count");
                    ui.label("%");
                    ui.end_row();

                    for entry in &freqs.entries {
                        draw_mini_hex_segments(ui, &entry.segments, hex_size);
                        ui.label(format!("{}", entry.count));
                        ui.label(format!("{:.1}%", entry.fraction * 100.0));
                        ui.end_row();
                    }
                });
        });
}

fn render_tooltip(data: &GameData, input: &InputState, ui_state: &UiState, ctx: &egui::Context) {
    if ui_state.tooltip_mode != TooltipMode::Group {
        return;
    }

    let segment_index = match input.hover_segment {
        Some(i) => i,
        None => return,
    };

    let segment = data.map.segment(segment_index);
    let group_index = data.group_assignments.group_of(segment_index);

    egui::Window::new("Tile info")
        .movable(false)
        .collapsible(false)
        .resizable(false)
        .current_pos((input.mouse_position.x + 30.0, input.mouse_position.y - 30.0))
        .show(ctx, |ui| {
            egui::Grid::new("tooltip_grid").show(ui, |ui| {
                ui.label("Position");
                ui.label(format!("{}, {}", input.hover_pos.x(), input.hover_pos.y()));
                ui.end_row();

                ui.label("Terrain");
                ui.label(format!("{:?}", segment.terrain));
                ui.end_row();

                if let Some(group_index) = group_index {
                    let group = &data.group_assignments.groups[group_index];
                    ui.label("Group");
                    ui.label(format!(
                        "{} units, {}",
                        group.unit_count,
                        if group.is_closed() { "closed" } else { "open" }
                    ));
                    ui.end_row();

                    for (quest, remaining) in group.remaining_per_quest() {
                        use crate::map::QuestType;
                        let remaining_str = if quest.quest_type == QuestType::Flag && remaining <= 0
                        {
                            "close".to_string()
                        } else {
                            let suffix = match quest.quest_type {
                                QuestType::MoreThan => "+",
                                QuestType::Exact | QuestType::Flag => "",
                                QuestType::Unknown => "?",
                            };
                            format!("{remaining}{suffix}")
                        };
                        ui.label("Quest");
                        ui.label(format!(
                            "{:?} {}/{} ({remaining_str})",
                            quest.terrain, group.unit_count, quest.target_value,
                        ));
                        ui.end_row();
                    }
                }
            });
        });
}

fn terrain_color(terrain: Terrain) -> Color32 {
    // Matches shader.frag color_of_terrain(). MAX saturation.
    match terrain {
        Terrain::Missing => Color32::from_rgb(0x32, 0x32, 0x32),
        Terrain::Empty => Color32::from_rgb(0x32, 0x32, 0x32),
        Terrain::House => Color32::from_rgb(0xFF, 0x7A, 0x2F),
        Terrain::Forest => Color32::from_rgb(0x59, 0x40, 0x24),
        Terrain::Wheat => Color32::from_rgb(0xFF, 0xF2, 0x26),
        Terrain::Rail => Color32::from_rgb(0xB0, 0xB5, 0xBD), // silver
        Terrain::River => Color32::from_rgb(0x1A, 0x80, 0xE6), // blue
        Terrain::Lake => Color32::from_rgb(0x1A, 0x56, 0xC9),
        Terrain::Station => Color32::from_rgb(0xB0, 0xB5, 0xBD), // same as rail
    }
}

fn render_placement_detail(
    data: &GameData,
    input: &InputState,
    camera: &Camera,
    ctx: &egui::Context,
) {
    use crate::data::HEX_SIDES;
    use crate::map::Map;

    // Find nearest placement within 3 hex tiles of hover position.
    let nearby = data.best_placements.find_nearest(input.hover_pos, 3);
    let score = match nearby {
        Some(s) => s,
        None => return,
    };

    // Position the window near the placement, offset to the right.
    let pixel_pos = camera.hex_to_pixel(score.pos);
    let window_pos = Pos2::new(pixel_pos.x() + 30.0, pixel_pos.y() - 100.0);

    egui::Area::new(egui::Id::new("placement_detail"))
        .fixed_pos(window_pos)
        .order(egui::Order::Tooltip)
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style())
                .fill(Color32::from_black_alpha(220))
                .show(ui, |ui| {
                    egui::Grid::new("placement_detail_grid")
                        .spacing([10.0, 4.0])
                        .show(ui, |ui| {
                            ui.label("Matching edges");
                            ui.label(
                                egui::RichText::new(format!("{}", score.matching_edges))
                                    .color(Color32::WHITE),
                            );
                            ui.end_row();

                            ui.label("Conn. difficulty");
                            let diff_color = if score.connection_difficulty > 0 {
                                Color32::from_rgb(220, 80, 80)
                            } else {
                                Color32::WHITE
                            };
                            ui.label(
                                egui::RichText::new(format!("{}", score.connection_difficulty))
                                    .color(diff_color),
                            );
                            ui.end_row();

                            ui.label("Neighbor bonus");
                            let bonus_color = if score.neighbor_bonus > 0 {
                                Color32::from_rgb(80, 200, 80)
                            } else {
                                Color32::WHITE
                            };
                            ui.label(
                                egui::RichText::new(format!("{}", score.neighbor_bonus))
                                    .color(bonus_color),
                            );
                            ui.end_row();

                            ui.label("Crowding");
                            let crowd_color = if score.crowding > 0 {
                                Color32::from_rgb(220, 80, 80)
                            } else {
                                Color32::WHITE
                            };
                            ui.label(
                                egui::RichText::new(format!("{}", score.crowding))
                                    .color(crowd_color),
                            );
                            ui.end_row();
                        });

                    // Neighbor fit effects.
                    if !score.neighbor_fit_effects.is_empty() {
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new("Neighbor fit chance")
                                .color(Color32::WHITE)
                                .strong(),
                        );
                        let side_names = ["N", "NE", "SE", "S", "SW", "NW"];
                        for effect in &score.neighbor_fit_effects {
                            let before_pct = effect.chance_before * 100.0;
                            let after_pct = effect.chance_after * 100.0;
                            let drop = before_pct - after_pct;
                            let color = if drop < 5.0 {
                                Color32::from_rgb(80, 200, 80)
                            } else if drop < 15.0 {
                                Color32::from_rgb(220, 180, 80)
                            } else {
                                Color32::from_rgb(220, 80, 80)
                            };
                            let name = side_names[effect.side];
                            ui.label(
                                egui::RichText::new(format!(
                                    "  {name}: {before_pct:.1}% -> {after_pct:.1}%",
                                ))
                                .color(color),
                            );
                        }
                    }

                    // Collect all groups whose open_edges contain this position.
                    let mut touched_groups = Vec::new();
                    for group in data.group_assignments.groups.iter() {
                        if group.open_edges.contains(&score.pos) {
                            touched_groups.push(group);
                        }
                    }
                    if !touched_groups.is_empty() {
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new("Groups").color(Color32::WHITE).strong());
                        for group in &touched_groups {
                            // Compute open-edge delta for this group.
                            let mut delta: i8 = -1; // Placing here closes this open edge.
                            for side in 0..HEX_SIDES {
                                let npos = Map::neighbor_pos_of(score.pos, side);
                                let neighbor_occupied = data
                                    .map
                                    .tile_key(npos)
                                    .and_then(|key| data.map.rendered_tiles[key])
                                    .is_some();
                                if neighbor_occupied {
                                    continue;
                                }
                                // Check if our tile has a matching segment at this side.
                                let my_terrain = data
                                    .map
                                    .next_tile
                                    .iter()
                                    .find(|seg| {
                                        seg.contains_rotation(
                                            (side + HEX_SIDES - score.rotation) % HEX_SIDES,
                                        )
                                    })
                                    .map(|s| s.terrain);
                                if my_terrain.is_some_and(|t| group.kind.accepts(t)) {
                                    delta += 1;
                                }
                            }
                            let delta_color = if delta > 0 {
                                Color32::from_rgb(220, 80, 80)
                            } else if delta < 0 {
                                Color32::from_rgb(80, 200, 80)
                            } else {
                                Color32::WHITE
                            };
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(format!("{:?}", group.terrain,))
                                        .color(terrain_color(group.terrain)),
                                );
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{} units, {} open",
                                        group.unit_count,
                                        group.open_edges.len(),
                                    ))
                                    .color(Color32::GRAY),
                                );
                                ui.label(
                                    egui::RichText::new(format!("{:+}", delta,)).color(delta_color),
                                );
                            });
                            for (quest, remaining) in group.remaining_per_quest() {
                                if quest.active {
                                    let qcolor = if remaining <= 0 {
                                        Color32::from_rgb(80, 200, 80)
                                    } else {
                                        Color32::from_rgb(220, 180, 80)
                                    };
                                    use crate::map::QuestType;
                                    let quest_text =
                                        if quest.quest_type == QuestType::Flag && remaining <= 0 {
                                            "  Quest: close".to_string()
                                        } else {
                                            format!("  Quest: {remaining} remaining")
                                        };
                                    ui.label(egui::RichText::new(quest_text).color(qcolor));
                                }
                            }
                        }
                    }
                });
        });
}

fn render_next_tile(data: &GameData, ctx: &egui::Context) {
    if data.map.next_tile.is_empty() {
        return;
    }

    let size = 80.0;
    egui::Area::new("next_tile")
        .anchor(egui::Align2::RIGHT_BOTTOM, (-20.0, -20.0))
        .show(ctx, |ui| {
            let has_quest = data.map.next_tile_quest.is_some();
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

            let verts = hex_vertices(center, size);
            let stroke = egui::Stroke::new(1.5, Color32::from_rgb(40, 40, 40));
            draw_hex_segments(&painter, center, &verts, &data.map.next_tile, stroke);

            // Draw hex outline on top.
            let mut outline = verts.to_vec();
            outline.push(verts[0]);
            painter.add(egui::Shape::line(
                outline,
                egui::Stroke::new(2.0, Color32::from_rgb(40, 40, 40)),
            ));

            // Draw quest info below the hex if present.
            if let Some(quest) = &data.map.next_tile_quest {
                use crate::map::QuestType;
                let text = if quest.quest_type == QuestType::Flag {
                    format!("{:?} flag", quest.terrain)
                } else {
                    let suffix = match quest.quest_type {
                        QuestType::MoreThan => "+",
                        QuestType::Exact => "",
                        QuestType::Flag => "",
                        QuestType::Unknown => "?",
                    };
                    format!("{:?} {}{}", quest.terrain, quest.target_value, suffix)
                };
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
/// Threshold for "easy" quests per terrain.
fn easy_quest_threshold(terrain: Terrain) -> i32 {
    match terrain {
        Terrain::Rail | Terrain::River => 3,
        Terrain::House => 10,
        Terrain::Wheat => 8,
        Terrain::Forest => 30,
        _ => 5,
    }
}

fn render_group_quest_labels(
    data: &GameData,
    camera: &Camera,
    ui_state: &mut UiState,
    ctx: &egui::Context,
    visible_rect: egui::Rect,
) {
    if visible_rect.width() <= 0.0 || visible_rect.height() <= 0.0 {
        return;
    }

    let mode = ui_state.quest_display;

    for (group_idx, group) in data.group_assignments.groups.iter().enumerate() {
        let mut active_quests: Vec<_> = group
            .remaining_per_quest()
            .into_iter()
            .filter(|(q, remaining)| {
                q.active
                    && match mode {
                        QuestDisplay::Min => true,
                        QuestDisplay::Easy => *remaining <= easy_quest_threshold(q.terrain),
                        QuestDisplay::All => true,
                        QuestDisplay::None => false,
                    }
            })
            .collect();
        if active_quests.is_empty() {
            continue;
        }

        // Min mode: only show the quest with the smallest target value.
        if mode == QuestDisplay::Min {
            if let Some(min_quest) = active_quests.iter().min_by_key(|(q, _)| q.target_value) {
                active_quests = vec![*min_quest];
            }
        }

        let centroid = camera.world_to_pixel(group.centroid);

        let margin = 50.0;
        if centroid.x() < visible_rect.min.x - margin
            || centroid.y() < visible_rect.min.y - margin
            || centroid.x() > visible_rect.max.x + margin
            || centroid.y() > visible_rect.max.y + margin
        {
            continue;
        }

        let text = active_quests
            .iter()
            .map(|(quest, remaining)| {
                use crate::map::QuestType;
                if quest.quest_type == QuestType::Flag && *remaining <= 0 {
                    format!("{:?} close", quest.terrain)
                } else {
                    let suffix = match quest.quest_type {
                        QuestType::MoreThan => "+",
                        QuestType::Exact | QuestType::Flag => "",
                        QuestType::Unknown => "?",
                    };
                    format!("{:?} {remaining}{suffix}", quest.terrain)
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        let id = egui::Id::new(("group_quest_label", group_idx));
        egui::Area::new(id)
            .order(egui::Order::Background)
            .fixed_pos(Pos2::new(centroid.x(), centroid.y()))
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

fn render_biggest_groups(
    data: &GameData,
    camera: &Camera,
    ctx: &egui::Context,
    visible_rect: egui::Rect,
) {
    if visible_rect.width() <= 0.0 || visible_rect.height() <= 0.0 {
        return;
    }

    use std::collections::HashMap;
    let mut top3: HashMap<Terrain, Vec<(usize, &crate::group::Group)>> = HashMap::new();
    for (idx, group) in data.group_assignments.groups.iter().enumerate() {
        if group.is_closed() {
            continue;
        }
        let list = top3.entry(group.terrain).or_default();
        list.push((idx, group));
        list.sort_by(|a, b| b.1.unit_count.cmp(&a.1.unit_count));
        list.truncate(3);
    }

    let rank_colors = [
        Color32::from_rgb(255, 215, 0),
        Color32::from_rgb(192, 192, 192),
        Color32::from_rgb(205, 127, 50),
    ];

    // Collect visible groups with their screen positions.
    struct GroupLabel {
        center: Pos2,
        radius_px: f32,
        text: String,
        color: Color32,
    }
    let mut labels = Vec::new();

    for (terrain, groups) in &top3 {
        for (rank, (_group_idx, group)) in groups.iter().enumerate() {
            let centroid_px = camera.world_to_pixel(group.centroid);

            let margin = 50.0;
            if centroid_px.x() < visible_rect.min.x - margin
                || centroid_px.y() < visible_rect.min.y - margin
                || centroid_px.x() > visible_rect.max.x + margin
                || centroid_px.y() > visible_rect.max.y + margin
            {
                continue;
            }

            let radius_px = camera.world_dist_to_pixels(group.radius);
            let text = format!("{terrain:?} {}", group.unit_count);
            labels.push(GroupLabel {
                center: Pos2::new(centroid_px.x(), centroid_px.y()),
                radius_px,
                text,
                color: rank_colors[rank],
            });
        }
    }

    // Pass 1: draw all circles.
    let mut circle_painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("biggest_groups_circles"),
    ));
    circle_painter.set_clip_rect(visible_rect);
    for label in &labels {
        circle_painter.circle_stroke(
            label.center,
            label.radius_px,
            egui::Stroke::new(2.0, label.color.linear_multiply(0.6)),
        );
    }

    // Pass 2: draw all outlines, then all colored text, on ONE painter
    // so draw order is strictly: all outlines first, all text on top.
    let mut painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new("biggest_groups_text"),
    ));
    painter.set_clip_rect(visible_rect);

    // All outlines first.
    for label in &labels {
        let font = egui::FontId::proportional(18.0);
        for dx in [-2.0_f32, -1.0, 0.0, 1.0, 2.0] {
            for dy in [-2.0_f32, -1.0, 0.0, 1.0, 2.0] {
                if dx == 0.0 && dy == 0.0 {
                    continue;
                }
                painter.text(
                    Pos2::new(label.center.x + dx, label.center.y + dy),
                    egui::Align2::CENTER_CENTER,
                    &label.text,
                    font.clone(),
                    Color32::BLACK,
                );
            }
        }
    }

    // All colored text on top of all outlines.
    for label in &labels {
        let font = egui::FontId::proportional(18.0);
        painter.text(
            label.center,
            egui::Align2::CENTER_CENTER,
            &label.text,
            font,
            label.color,
        );
    }
}

fn render_status_bar(
    file_watcher: &FileWatcher,
    game_nav: &crate::game::game_nav::GameNav,
    ctx: &egui::Context,
) {
    egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            if file_watcher.map_loader.in_progress() {
                ui.add(egui::Spinner::default().size(14.0));
                ui.label("Loading map...");
                ui.separator();
            }
            ui.label(&game_nav.detect_status);
        });
    });
}

#[allow(clippy::too_many_arguments)]
pub fn render_ui(
    data: &mut GameData,
    camera: &mut Camera,
    ui_state: &mut UiState,
    file_watcher: &mut FileWatcher,
    input: &InputState,
    game_nav: &crate::game::game_nav::GameNav,
    pending_zoom_fit: &mut u8,
    ctx: &egui::Context,
) -> egui::Rect {
    render_top_panel(ui_state, file_watcher, ctx);
    render_side_panel(data, camera, ui_state, ctx);
    render_status_bar(file_watcher, game_nav, ctx);
    // Available rect after all panels have claimed their space.
    let visible_rect = ctx.available_rect();
    let full_rect = ctx.screen_rect();
    if full_rect.width() > 0.0 && full_rect.height() > 0.0 {
        camera.visible_fraction = glam::Vec2::new(
            visible_rect.width() / full_rect.width(),
            visible_rect.height() / full_rect.height(),
        );
    }
    // Delay zoom_fit by one frame so visible_fraction reflects the new sidebar size.
    // pending_zoom_fit counts down: 2 → 1 → zoom → 0.
    if *pending_zoom_fit > 0 {
        *pending_zoom_fit -= 1;
        if *pending_zoom_fit == 0 {
            // Zoom to best placement if available, otherwise fit the whole map.
            if let Some((_, score)) = data.best_placements.iter_all().first() {
                camera.goto(score.pos);
            } else {
                camera.zoom_fit(&data.map);
            }
        }
    }
    render_tooltip(data, input, ui_state, ctx);
    if ui_state.quest_display != QuestDisplay::None {
        render_group_quest_labels(data, camera, ui_state, ctx, visible_rect);
    }
    if ui_state.show_biggest_groups {
        render_biggest_groups(data, camera, ctx, visible_rect);
    }
    if ui_state.tooltip_mode == TooltipMode::Placement {
        render_placement_detail(data, input, camera, ctx);
    }
    if ui_state.tooltip_mode == TooltipMode::Chance {
        render_placement_chance(data, input, camera, ctx);
    }
    if ui_state.show_imperfect_tiles {
        render_imperfect_tiles(data, camera, ctx, visible_rect);
    }
    // Highlight focused placement.
    if let Some(pos) = ui_state.focused_placement {
        let px = camera.hex_to_pixel(pos);
        let radius = camera.world_dist_to_pixels(1.2);
        let mut painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Middle,
            egui::Id::new("focused_placement_highlight"),
        ));
        painter.set_clip_rect(visible_rect);
        painter.circle_stroke(
            Pos2::new(px.x(), px.y()),
            radius,
            egui::Stroke::new(3.0, Color32::from_rgb(80, 140, 255)),
        );
    }
    // Highlight focused group.
    if let Some(group_idx) = ui_state.focused_group {
        if let Some(group) = data.group_assignments.groups.get(group_idx) {
            let centroid_px = camera.world_to_pixel(group.centroid);
            let radius_px = camera.world_dist_to_pixels(group.radius).max(20.0);
            let mut painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Middle,
                egui::Id::new("focused_group_highlight"),
            ));
            painter.set_clip_rect(visible_rect);
            painter.circle_stroke(
                Pos2::new(centroid_px.x(), centroid_px.y()),
                radius_px,
                egui::Stroke::new(3.0, Color32::from_rgb(255, 200, 50)),
            );
        }
    }
    render_tile_frequencies(data, ui_state, ctx);
    render_next_tile(data, ctx);
    render_game_camera_marker(game_nav, camera, ctx, visible_rect);
    visible_rect
}

/// Highlight tiles that have non-matching edges with a red overlay.
fn render_imperfect_tiles(
    data: &mut GameData,
    camera: &Camera,
    ctx: &egui::Context,
    visible_rect: egui::Rect,
) {
    let imperfect = data.imperfect_tiles().clone();
    let mut painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("imperfect_tiles"),
    ));
    painter.set_clip_rect(visible_rect);

    let highlight = Color32::from_rgba_premultiplied(255, 40, 40, 100);
    let dx = camera.world_dist_to_pixels(1.0);
    let radius = dx * 0.5;

    for &pos in &imperfect {
        let px = camera.hex_to_pixel(pos);
        if px.x() < visible_rect.min.x - radius
            || px.y() < visible_rect.min.y - radius
            || px.x() > visible_rect.max.x + radius
            || px.y() > visible_rect.max.y + radius
        {
            continue;
        }
        painter.circle_filled(Pos2::new(px.x(), px.y()), radius, highlight);
    }
}

/// Show placement chance: for each valid placement near hover, compute
/// how many known tile patterns would fit and the probability.
/// Show placement chance: for the hovered empty position, compute
/// how many known tile patterns would fit (matching all occupied neighbor edges).
fn render_placement_chance(
    data: &GameData,
    input: &InputState,
    camera: &Camera,
    ctx: &egui::Context,
) {
    use crate::data::{EdgeMatch, HEX_SIDES};
    use crate::map::Map;

    let pos = input.hover_pos;

    // Only show for empty positions (no tile placed there).
    let is_empty = data
        .map
        .tile_key(pos)
        .is_none_or(|key| data.map.tile_index[key].is_none());
    if !is_empty {
        return;
    }

    // Must have at least one occupied neighbor to be interesting.
    let mut has_any_neighbor = false;

    // Collect edge constraints: for each side with an occupied neighbor,
    // record what terrain that neighbor presents on the facing edge.
    let mut constraints: [Option<Terrain>; 6] = [None; 6];
    for (side, constraint) in constraints.iter_mut().enumerate() {
        let npos = Map::neighbor_pos_of(pos, side);
        let other_side = Map::opposite_side(side);
        let neighbor_tile = data
            .map
            .tile_key(npos)
            .and_then(|key| data.map.rendered_tiles[key]);
        if let Some(rendered) = neighbor_tile {
            let terrain = rendered[other_side]
                .map(|idx| data.map.segments[idx].terrain)
                .unwrap_or(Terrain::Empty);
            *constraint = Some(terrain);
            has_any_neighbor = true;
        }
    }

    if !has_any_neighbor {
        return;
    }

    // Check each tile pattern at all 6 rotations against constraints.
    // A tile fits at a rotation if every constrained edge matches (no Suboptimal/Illegal).
    let mut matching_count: usize = 0;
    let mut matching_unique: usize = 0;
    let total_tiles = data.tile_frequencies.total_tiles;

    for entry in &data.tile_frequencies.entries {
        let profile = crate::data::EdgeProfile::from_segments(&entry.segments);
        let mut counted = false;
        for rot in 0..HEX_SIDES {
            let rotated = profile.rotated(rot);
            let fits = constraints
                .iter()
                .enumerate()
                .all(|(side, constraint)| match constraint {
                    None => true,
                    Some(neighbor_terrain) => {
                        let my_terrain = rotated.at_index(side);
                        matches!(
                            my_terrain.connects_and_matches(*neighbor_terrain),
                            EdgeMatch::Matching
                        )
                    }
                });
            if fits && !counted {
                matching_unique += 1;
                matching_count += entry.count;
                counted = true;
                // Don't break — but don't double-count either.
            }
        }
    }

    let chance = if total_tiles > 0 {
        matching_count as f64 / total_tiles as f64 * 100.0
    } else {
        0.0
    };

    let pixel_pos = camera.hex_to_pixel(pos);
    let window_pos = Pos2::new(pixel_pos.x() + 30.0, pixel_pos.y() - 60.0);

    egui::Area::new(egui::Id::new("placement_chance"))
        .fixed_pos(window_pos)
        .order(egui::Order::Tooltip)
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style())
                .fill(Color32::from_black_alpha(220))
                .show(ui, |ui| {
                    let chance_color = if chance > 50.0 {
                        Color32::from_rgb(80, 200, 80)
                    } else if chance > 20.0 {
                        Color32::from_rgb(220, 180, 80)
                    } else {
                        Color32::from_rgb(220, 80, 80)
                    };
                    ui.label(
                        egui::RichText::new(format!("{chance:.1}%"))
                            .color(chance_color)
                            .size(18.0)
                            .strong(),
                    );
                    ui.label(
                        egui::RichText::new(format!("{matching_count} of {total_tiles} tiles"))
                            .color(Color32::WHITE),
                    );
                    ui.label(
                        egui::RichText::new(format!("{matching_unique} unique patterns"))
                            .color(Color32::GRAY),
                    );
                });
        });
}

/// Render the estimated game viewport as a quad on the solver map.
fn render_game_camera_marker(
    game_nav: &crate::game::game_nav::GameNav,
    camera: &Camera,
    ctx: &egui::Context,
    visible_rect: egui::Rect,
) {
    let center = match game_nav.game_center() {
        Some(c) => c,
        None => return,
    };

    let px = camera.world_to_pixel(center);
    let mut painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new("game_camera_marker"),
    ));
    painter.set_clip_rect(visible_rect);

    let color = Color32::from_rgba_premultiplied(255, 255, 0, 180);
    painter.circle_stroke(
        Pos2::new(px.x(), px.y()),
        20.0,
        egui::Stroke::new(3.0, color),
    );
}
