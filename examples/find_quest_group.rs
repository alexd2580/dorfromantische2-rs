//! Find groups by quest remaining count.
//! Run with: cargo run --example find_quest_group -- Forest 1857

use dorfromantische2_rs::data::Terrain;
use dorfromantische2_rs::group_assignments::GroupAssignments;
use dorfromantische2_rs::map::Map;
use dorfromantische2_rs::raw_data::SaveGame;
use std::io::Cursor;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let terrain_name = args.get(1).map(|s| s.as_str()).unwrap_or("Forest");
    let target_remaining: i32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1857);

    let data = std::fs::read("calibration/savegame.sav").unwrap();
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
        _ => panic!("Unknown terrain: {terrain_name}"),
    };

    // Rank by unit count
    let mut ranked: Vec<(usize, usize, &dorfromantische2_rs::group::Group)> = groups
        .groups
        .iter()
        .enumerate()
        .filter(|(_, g)| g.terrain == target_terrain && !g.is_closed())
        .collect::<Vec<_>>()
        .into_iter()
        .enumerate()
        .map(|(rank, (idx, g))| (rank, idx, g))
        .collect();
    ranked.sort_by(|a, b| b.2.unit_count.cmp(&a.2.unit_count));

    println!("Looking for {terrain_name} quest with ~{target_remaining} remaining\n");

    for (i, &(_, group_idx, group)) in ranked.iter().enumerate() {
        let rank = i + 1;
        for q in &group.quests {
            if !q.active || q.terrain != target_terrain {
                continue;
            }
            let remaining = q.target_value - group.unit_count as i32;
            let diff = (remaining - target_remaining).abs();
            if diff < 20 {
                println!("MATCH: {terrain_name}#{rank} (group {group_idx})");
                println!(
                    "  units={}, target={}, remaining={}, diff from searched={diff}",
                    group.unit_count, q.target_value, remaining
                );
                println!(
                    "  segments={}, open_edges={}",
                    group.segment_indices.len(),
                    group.open_edges.len()
                );
                println!("  quest_type={:?}", q.quest_type);

                // Segment breakdown
                use std::collections::HashMap;
                let mut by_form: HashMap<(dorfromantische2_rs::data::Form, u32), usize> =
                    HashMap::new();
                for &seg_idx in &group.segment_indices {
                    let seg = &map.segments[seg_idx];
                    *by_form.entry((seg.form, seg.unit_count)).or_default() += 1;
                }
                let mut form_list: Vec<_> = by_form.into_iter().collect();
                form_list.sort_by_key(|((_, _), count)| std::cmp::Reverse(*count));
                println!("  Segment breakdown:");
                for ((form, units), count) in &form_list {
                    println!(
                        "    {form:?} ({units}u) x{count} = {}",
                        *units as u64 * *count as u64
                    );
                }
                println!();
            }
        }
    }
}
