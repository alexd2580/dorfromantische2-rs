//! Find small open forest groups where each segment has a unique form,
//! so each form's unit count can be isolated.
//! Run with: cargo run --example find_simple_groups -- <save_path>

use dorfromantische2_rs::data::{Form, Terrain};
use dorfromantische2_rs::group_assignments::GroupAssignments;
use dorfromantische2_rs::map::Map;
use dorfromantische2_rs::raw_data::SaveGame;
use std::collections::HashMap;
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

    println!("=== Small open forest groups with all-unique forms ===\n");

    let mut results = Vec::new();
    for (group_idx, group) in groups.groups.iter().enumerate() {
        if group.terrain != Terrain::Forest || group.is_closed() {
            continue;
        }
        if group.segment_indices.len() > 6 || group.segment_indices.len() < 2 {
            continue;
        }

        let mut by_form: HashMap<Form, Vec<u32>> = HashMap::new();
        for &seg_idx in &group.segment_indices {
            let seg = &map.segments[seg_idx];
            by_form.entry(seg.form).or_default().push(seg.unit_count);
        }

        // All forms must be unique (no duplicates).
        let all_unique = by_form.values().all(|v| v.len() == 1);
        if !all_unique {
            continue;
        }

        let breakdown: Vec<(Form, u32)> = by_form.into_iter().map(|(f, v)| (f, v[0])).collect();

        results.push((group_idx, group, breakdown));
    }

    results.sort_by_key(|(_, g, _)| g.segment_indices.len());

    for (group_idx, group, breakdown) in &results {
        let has_quest = group.quests.iter().any(|q| q.active);
        let mut forms: Vec<_> = breakdown.clone();
        forms.sort_by_key(|(f, _)| format!("{f:?}"));

        let parts: String = forms
            .iter()
            .map(|(f, u)| format!("{f:?}={u}"))
            .collect::<Vec<_>>()
            .join(", ");

        let quest_info = if has_quest {
            let q = group.quests.iter().find(|q| q.active).unwrap();
            let rem = q.target_value - group.unit_count as i32;
            format!("  quest: tgt={} rem={}", q.target_value, rem)
        } else {
            String::new()
        };

        println!(
            "Group {group_idx} | {} segs | units={} | {parts}{quest_info}",
            group.segment_indices.len(),
            group.unit_count,
        );
    }
}
