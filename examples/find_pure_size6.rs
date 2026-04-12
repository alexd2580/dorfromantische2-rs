//! Find the smallest open forest groups that contain ONLY Size6 segments.
//! Ideally a group with just one Size6 segment and nothing else.
//! Run with: cargo run --example find_pure_size6 -- <save_path>

use dorfromantische2_rs::data::{Form, Terrain};
use dorfromantische2_rs::group_assignments::GroupAssignments;
use dorfromantische2_rs::map::Map;
use dorfromantische2_rs::raw_data::SaveGame;
use std::io::Cursor;

fn main() {
    let save_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "calibration/savegame.sav".into());
    let data = std::fs::read(&save_path).unwrap();
    let parsed = nrbf_rs::parse_nrbf(&mut Cursor::new(&data));
    let sg = SaveGame::try_from(&parsed).unwrap();
    let map = Map::from(&sg);
    let groups = GroupAssignments::from(&map);

    println!("=== Open forest groups with ONLY Size6(37u) non-quest segments ===\n");
    find_pure(&map, &groups, 37);

    println!("\n=== Open forest groups with ONLY Size6(26u) quest segments ===\n");
    find_pure(&map, &groups, 26);

    println!("\n=== Single-segment Size6 forest groups (any unit count) ===\n");
    for (group_idx, group) in groups.groups.iter().enumerate() {
        if group.terrain != Terrain::Forest || group.is_closed() {
            continue;
        }
        if group.segment_indices.len() != 1 {
            continue;
        }
        let seg_idx = *group.segment_indices.iter().next().unwrap();
        let seg = &map.segments[seg_idx];
        if seg.form != Form::Size6 {
            continue;
        }
        let has_quest = group.quests.iter().any(|q| q.active);
        println!(
            "  Group {} | 1 seg | Size6 ({}u) | units={} | pos={} | quest={}",
            group_idx,
            seg.unit_count,
            group.unit_count,
            seg.pos,
            if has_quest { "yes" } else { "no" }
        );
    }
}

fn find_pure(
    map: &dorfromantische2_rs::map::Map,
    groups: &GroupAssignments,
    target_unit_count: u32,
) {
    let mut results = Vec::new();
    for (group_idx, group) in groups.groups.iter().enumerate() {
        if group.terrain != Terrain::Forest || group.is_closed() {
            continue;
        }
        let all_size6 = group.segment_indices.iter().all(|&si| {
            let seg = &map.segments[si];
            seg.form == Form::Size6 && seg.unit_count == target_unit_count
        });
        if !all_size6 {
            continue;
        }
        results.push((group_idx, group));
    }
    results.sort_by_key(|(_, g)| g.segment_indices.len());
    for (group_idx, group) in results.iter().take(10) {
        let has_quest = group.quests.iter().any(|q| q.active);
        println!(
            "  Group {} | {} segs | units={} | open_edges={} | quest={}",
            group_idx,
            group.segment_indices.len(),
            group.unit_count,
            group.open_edges.len(),
            if has_quest { "yes" } else { "no" }
        );
    }
    if results.is_empty() {
        println!("  (none found)");
    }
}
