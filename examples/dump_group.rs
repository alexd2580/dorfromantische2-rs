//! Dump the composition of a specific group.
//! Run with: cargo run --example dump_group -- <terrain> <rank>
//! e.g.: cargo run --example dump_group -- Forest 44

use dorfromantische2_rs::data::{Form, HexPos, Terrain};
use dorfromantische2_rs::group_assignments::GroupAssignments;
use dorfromantische2_rs::map::Map;
use dorfromantische2_rs::raw_data::SaveGame;
use std::collections::HashMap;
use std::io::Cursor;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let terrain_name = args.get(1).map(|s| s.as_str()).unwrap_or("Forest");
    let target_rank: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(44);
    let save_path = args
        .get(3)
        .map(|s| s.as_str())
        .unwrap_or("calibration/savegame.sav");
    let by_index = args.iter().any(|a| a == "--index");

    let data = std::fs::read(save_path).unwrap();
    let parsed = nrbf_rs::parse_nrbf(&mut Cursor::new(&data));
    let sg = SaveGame::try_from(&parsed).unwrap();
    let map = Map::from(&sg);
    let groups = GroupAssignments::from(&map);

    let target_terrain = match terrain_name {
        "Forest" => Terrain::Forest,
        "House" | "Village" => Terrain::House,
        "Wheat" | "Agriculture" => Terrain::Wheat,
        "Rail" | "Train" => Terrain::Rail,
        "River" | "Water" => Terrain::River,
        "Lake" => Terrain::Lake,
        _ => panic!("Unknown terrain: {terrain_name}"),
    };

    // Rank groups by unit count
    let mut terrain_groups: Vec<(usize, &dorfromantische2_rs::group::Group)> = groups
        .groups
        .iter()
        .enumerate()
        .filter(|(_, g)| g.terrain == target_terrain && !g.is_closed())
        .collect();
    terrain_groups.sort_by(|a, b| b.1.unit_count.cmp(&a.1.unit_count));

    let (group_idx, group) = if by_index {
        // target_rank is actually a group index when --index is used.
        match groups.groups.get(target_rank) {
            Some(g) => (&target_rank, g),
            None => {
                println!("No group at index {target_rank}");
                return;
            }
        }
    } else {
        match terrain_groups.get(target_rank - 1) {
            Some(&(ref idx, group)) => (idx, group),
            None => {
                println!("No group at rank {target_rank} for {terrain_name}");
                println!("Available: {} open groups", terrain_groups.len());
                return;
            }
        }
    };

    println!("=== {terrain_name}#{target_rank} (group index {group_idx}) ===");
    println!("Unit count: {}", group.unit_count);
    println!("Segments: {}", group.segment_indices.len());
    println!("Open edges: {}", group.open_edges.len());
    println!("Closed: {}", group.is_closed());
    println!();

    // Count segments by (Form, unit_count)
    let mut by_form: HashMap<(Form, u32), usize> = HashMap::new();
    let mut tile_positions: HashMap<HexPos, Vec<(Form, Terrain, u32)>> = HashMap::new();

    for &seg_idx in &group.segment_indices {
        let seg = &map.segments[seg_idx];
        *by_form.entry((seg.form, seg.unit_count)).or_default() += 1;
        tile_positions
            .entry(seg.pos)
            .or_default()
            .push((seg.form, seg.terrain, seg.unit_count));
    }

    // Show tile positions
    println!("Tile positions:");
    let mut positions: Vec<_> = tile_positions.keys().collect();
    positions.sort_by_key(|p| (p.x(), p.y()));
    for pos in &positions {
        let segs = &tile_positions[pos];
        let desc: Vec<_> = segs
            .iter()
            .map(|(f, t, u)| format!("{f:?} {t:?} {u}u"))
            .collect();
        // Check if quest tile
        let is_quest = map.quests.contains_key(pos);
        let qt_marker = if is_quest { " [QUEST]" } else { "" };
        println!(
            "  ({}, {}): {}{qt_marker}",
            pos.x(),
            pos.y(),
            desc.join(", ")
        );
    }

    println!("Segment breakdown (form, unit_count -> count):");
    let mut form_list: Vec<_> = by_form.into_iter().collect();
    form_list.sort_by_key(|((form, _), count)| (std::cmp::Reverse(*count), format!("{form:?}")));
    let mut total_units = 0u64;
    for ((form, units), count) in &form_list {
        let subtotal = *units as u64 * *count as u64;
        total_units += subtotal;
        println!("  {form:?} ({units} units) x {count} = {subtotal}");
    }
    println!("\nTotal computed units: {total_units}");
    println!("Group unit_count:    {}", group.unit_count);

    // Quests
    if !group.quests.is_empty() {
        println!("\nQuests:");
        for q in &group.quests {
            let remaining = q.target_value - group.unit_count as i32;
            println!(
                "  {:?} target={} remaining={} type={:?} active={}",
                q.terrain, q.target_value, remaining, q.quest_type, q.active
            );
        }
    }
}
