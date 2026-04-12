//! Inspect tiles and segments at and around a hex position.
//! Run with: cargo run --example inspect_tile -- <x> <y> [save_path]

use dorfromantische2_rs::data::{HexPos, HEX_SIDES};
use dorfromantische2_rs::group_assignments::GroupAssignments;
use dorfromantische2_rs::hex;
use dorfromantische2_rs::map::Map;
use dorfromantische2_rs::raw_data::SaveGame;
use std::io::Cursor;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let x: i32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let y: i32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let save_path = args
        .get(3)
        .map(|s| s.as_str())
        .unwrap_or("calibration/savegame.sav");

    let data = std::fs::read(save_path).unwrap();
    let parsed = nrbf_rs::parse_nrbf(&mut Cursor::new(&data));
    let sg = SaveGame::try_from(&parsed).unwrap();
    let map = Map::from(&sg);
    let groups = GroupAssignments::from(&map);

    let center = HexPos::new(x, y);
    // Show center + neighbors.
    let mut positions = vec![center];
    for r in 0..HEX_SIDES {
        positions.push(hex::neighbor_pos_of(center, r));
    }

    for pos in &positions {
        let is_center = *pos == center;
        let marker = if is_center { " <<<" } else { "" };

        if !map.has(*pos) {
            if is_center {
                println!("({}, {}) — empty{marker}", pos.x(), pos.y());
            }
            continue;
        }

        println!("({}, {}){marker}", pos.x(), pos.y());

        // Show all segments on this tile.
        if let Some(indices) = map.segment_indices_at(*pos) {
            for seg_idx in indices {
                let seg = &map.segments[seg_idx];
                // Find which group this segment belongs to.
                let group_idx = groups.group_of(seg_idx);
                let group_info = group_idx
                    .map(|gi| {
                        let g = &groups.groups[gi];
                        let quest_info = g
                            .quests
                            .iter()
                            .find(|q| q.active)
                            .map(|q| {
                                format!(
                                    " quest:tgt={} rem={}",
                                    q.target_value,
                                    q.target_value - g.unit_count as i32
                                )
                            })
                            .unwrap_or_default();
                        format!(
                            "group {} ({:?}, {} units, {} segs{quest_info})",
                            gi,
                            g.terrain,
                            g.unit_count,
                            g.segment_indices.len()
                        )
                    })
                    .unwrap_or_else(|| "no group".to_string());

                println!(
                    "  seg[{seg_idx}]: {:?} {:?} rot={} units={} | {group_info}",
                    seg.terrain, seg.form, seg.rotation, seg.unit_count,
                );
            }
        }

        // Show quest on this tile.
        if let Some(quest) = map.quests.get(pos) {
            println!(
                "  QUEST: {:?} {:?} target={} active={} id={} level={}",
                quest.terrain,
                quest.quest_type,
                quest.target_value,
                quest.active,
                quest.quest_id,
                quest.quest_level,
            );
        }

        // Show raw tile info if available.
        for raw_tile in &sg.tiles {
            let raw_pos = HexPos::new(raw_tile.s, raw_tile.t - ((raw_tile.s + 1) & -2i32) / 2);
            if raw_pos == *pos {
                if let Some(qt) = &raw_tile.quest_tile {
                    println!(
                        "  RAW QUEST TILE: quest_tile_id={} rotation={}",
                        qt.quest_tile_id.0, raw_tile.rotation,
                    );
                }
                break;
            }
        }
    }
}
