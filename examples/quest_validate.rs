//! Validate quest target values against reference group sizes.
//! Extracts active quest targets from the savegame and compares them
//! to what ReferenceGroupCount would return, to isolate the difficulty increase.

use dorfromantische2_rs::data::Terrain;
use dorfromantische2_rs::group_assignments::GroupAssignments;
use dorfromantische2_rs::map::Map;
use dorfromantische2_rs::raw_data::SaveGame;
use std::collections::HashMap;
use std::io::Cursor;

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "calibration/savegame.sav".into());
    let data = std::fs::read(&path).unwrap();
    let parsed = nrbf_rs::parse_nrbf(&mut Cursor::new(&data));
    let sg = SaveGame::try_from(&parsed).unwrap();

    println!("Level: {}", sg.level);
    println!("Tiles placed: {}", sg.placed_tile_count);
    println!();

    let map = Map::from(&sg);
    let groups = GroupAssignments::from(&map);

    // Build open group sizes by terrain
    let mut open_groups: HashMap<Terrain, Vec<usize>> = HashMap::new();
    for group in &groups.groups {
        if !group.is_closed() {
            open_groups
                .entry(group.terrain)
                .or_default()
                .push(group.segment_indices.len());
        }
    }
    for sizes in open_groups.values_mut() {
        sizes.sort_unstable_by(|a, b| b.cmp(a));
    }

    // Map quest terrain IDs to our Terrain enum
    // From decompiled QuestId: forest=20, village=10/11, agriculture=30/31, train=41/42, water=50/51
    // QuestTileId names tell us the terrain: Agriculture_, Forest_, Village_, Train_, Water_

    // Extract active quests from savegame tiles
    println!(
        "{:<6} {:<15} {:<10} {:<8} {:<8} {:<10} {:<10}",
        "Level", "QuestTileId", "QuestId", "Target", "Active", "RefGroup?", "Diff?"
    );
    println!("{}", "-".repeat(75));

    let mut quest_data: Vec<(i32, i32, i32, i32, bool, String)> = Vec::new();

    for tile in &sg.tiles {
        if let Some(qt) = &tile.quest_tile {
            if qt.target_value > 0 && qt.quest_active {
                let tile_id = qt.quest_tile_id.0;
                let quest_id = qt.quest_id.0;
                let target = qt.target_value;
                let level = qt.quest_level;

                let terrain_from_quest = match quest_id {
                    10 | 11 => "Village",
                    20 => "Forest",
                    30 | 31 => "Wheat",
                    41 | 42 => "Rail",
                    50 | 51 => "Water",
                    1 => "Closing",
                    _ => "Unknown",
                };

                // Find reference group size for this terrain
                let solver_terrain = match terrain_from_quest {
                    "Forest" => Some(Terrain::Forest),
                    "Village" => Some(Terrain::House),
                    "Wheat" => Some(Terrain::Wheat),
                    "Rail" => Some(Terrain::Rail),
                    "Water" => Some(Terrain::River),
                    _ => None,
                };

                let ref_size = solver_terrain
                    .and_then(|t| open_groups.get(&t).and_then(|sizes| sizes.first().copied()))
                    .unwrap_or(0);

                let diff = target - ref_size as i32;

                println!(
                    "{:<6} {:<15} {:<10} {:<8} {:<8} {:<10} {:<10}",
                    level,
                    format!("tile_{}", tile_id),
                    format!("{}_{}", terrain_from_quest, quest_id),
                    target,
                    qt.quest_active,
                    ref_size,
                    diff,
                );

                quest_data.push((
                    level,
                    quest_id,
                    target,
                    ref_size as i32,
                    qt.quest_active,
                    terrain_from_quest.to_string(),
                ));
            }
        }
    }

    println!();
    println!("=== Analysis ===\n");

    // For each active quest, show: target - referenceGroupSize = base + difficulty
    // This tells us how much the difficulty formula adds on top of the group reference
    let mut by_terrain: HashMap<String, Vec<(i32, i32, i32, i32)>> = HashMap::new(); // terrain -> (level, target, ref, diff)
    for (level, _qid, target, ref_size, _active, terrain) in &quest_data {
        by_terrain.entry(terrain.clone()).or_default().push((
            *level,
            *target,
            *ref_size,
            target - ref_size,
        ));
    }

    for (terrain, entries) in &by_terrain {
        println!("{terrain}:");
        for (level, target, ref_size, diff) in entries {
            let pct = if *target > 0 {
                *ref_size as f32 / *target as f32 * 100.0
            } else {
                0.0
            };
            println!("  level={level}, target={target}, refGroup={ref_size}, base+difficulty={diff}, refGroup is {pct:.0}% of target");
        }
        println!();
    }

    // Also dump ALL quest tiles (including fulfilled ones) to see historical targets
    println!("=== All quest tiles (including completed) ===\n");
    println!(
        "{:<6} {:<10} {:<8} {:<8} {:<10}",
        "Level", "QuestId", "Target", "Active", "Terrain"
    );

    let mut all_targets: Vec<(i32, i32)> = Vec::new(); // (level, target)
    for tile in &sg.tiles {
        if let Some(qt) = &tile.quest_tile {
            if qt.target_value > 0 {
                all_targets.push((qt.quest_level, qt.target_value));
                // Only print a sample
            }
        }
    }

    // Stats
    all_targets.sort_by_key(|t| t.0);
    println!("Total quest tiles with targets: {}", all_targets.len());

    // Group by level ranges and show average target
    let ranges = [
        (0, 50),
        (50, 100),
        (100, 150),
        (150, 200),
        (200, 250),
        (250, 300),
        (300, 360),
    ];
    println!("\nAverage target by level range:");
    for (lo, hi) in ranges {
        let in_range: Vec<_> = all_targets
            .iter()
            .filter(|(l, _)| *l >= lo && *l < hi)
            .collect();
        if !in_range.is_empty() {
            let avg: f32 =
                in_range.iter().map(|(_, t)| *t as f32).sum::<f32>() / in_range.len() as f32;
            let max = in_range.iter().map(|(_, t)| *t).max().unwrap();
            let min = in_range.iter().map(|(_, t)| *t).min().unwrap();
            println!("  level {lo:>3}-{hi:>3}: {count:>3} quests, avg target={avg:>6.1}, min={min:>4}, max={max:>4}",
                count = in_range.len());
        }
    }
}
