//! Predict quest target values based on current group sizes.
//! Run with: cargo run --example quest_predict

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
    println!("Score: {}", sg.score);
    println!("Tiles placed: {}", sg.placed_tile_count);
    println!("Quests fulfilled: {}", sg.quests_fulfilled);
    println!();

    let map = Map::from(&sg);
    let groups = GroupAssignments::from(&map);

    // Collect open groups by terrain type, sorted by size (segment count).
    let mut terrain_groups: HashMap<Terrain, Vec<(usize, usize, bool)>> = HashMap::new();

    for (group_idx, group) in groups.groups.iter().enumerate() {
        let terrain = group.terrain;
        let seg_count = group.segment_indices.len();
        let is_closed = group.is_closed();
        terrain_groups
            .entry(terrain)
            .or_default()
            .push((group_idx, seg_count, is_closed));
    }

    // Sort each terrain's groups by size descending
    for groups in terrain_groups.values_mut() {
        groups.sort_by(|a, b| b.1.cmp(&a.1));
    }

    let quest_terrains = [
        Terrain::Forest,
        Terrain::House, // Village
        Terrain::Wheat, // Agriculture
        Terrain::Rail,  // Train
        Terrain::River, // Water (River + Lake)
    ];

    println!("=== Current Group Sizes by Terrain ===\n");

    for &terrain in &quest_terrains {
        let groups = terrain_groups.get(&terrain).cloned().unwrap_or_default();
        let open: Vec<_> = groups.iter().filter(|g| !g.2).collect();
        let closed: Vec<_> = groups.iter().filter(|g| g.2).collect();

        println!(
            "{terrain:?}: {} open groups, {} closed",
            open.len(),
            closed.len()
        );

        // Show top 10 open groups
        println!("  Top 10 open groups (segment count):");
        for (i, &&(_, count, _)) in open.iter().take(10).enumerate() {
            let marker = if i == 0 {
                " ← MoreThan reference"
            } else if i < 4 {
                " ← Exactly candidate"
            } else {
                ""
            };
            println!("    #{}: {} segments{}", i + 1, count, marker);
        }
        println!();
    }

    // For River/Water quests, we should also count Lake groups since they share
    println!("=== Lake groups (count toward Water quests) ===");
    if let Some(lake_groups) = terrain_groups.get(&Terrain::Lake) {
        let open: Vec<_> = lake_groups.iter().filter(|g| !g.2).collect();
        println!("  {} open lake groups", open.len());
        for (i, &&(_, count, _)) in open.iter().take(5).enumerate() {
            println!("    #{}: {} segments", i + 1, count);
        }
    }
    println!();

    // Predict quest targets
    println!("=== Predicted Quest Targets ===");
    println!("(assuming base_target=0, difficulty_increase=0 for illustration)\n");
    println!(
        "{:<12} {:>12} {:>12} {:>20}",
        "Terrain", "MoreThan ref", "Exactly ref", "After closing top 10"
    );

    for &terrain in &quest_terrains {
        let groups = terrain_groups.get(&terrain).cloned().unwrap_or_default();
        let open: Vec<_> = groups.iter().filter(|g| !g.2).map(|g| g.1).collect();

        let more_than_ref = open.first().copied().unwrap_or(0);
        let exactly_ref = if open.len() >= 4 {
            // Average of top 4 (it's random, so show range)
            format!("{}-{}", open[3], open[0])
        } else if !open.is_empty() {
            format!("{}-{}", open[open.len() - 1], open[0])
        } else {
            "0".to_string()
        };

        // After closing top 10: the reference becomes the 11th largest
        let after_close = open.get(10).copied().unwrap_or(0);

        println!(
            "{:<12} {:>12} {:>12} {:>20}",
            format!("{terrain:?}"),
            more_than_ref,
            exactly_ref,
            after_close,
        );
    }

    // Estimate difficulty increase for various levelsNeededPerIncrease / targetValueIncrease combos
    let level = sg.level as f32;
    // With default expFactor=1.0, globalDiffMult=1.0, questDiffMult=1.0:
    // extra = level / levelsNeededPerIncrease * targetValueIncrease
    println!(
        "\n=== Difficulty Increase Estimates (level={}) ===",
        sg.level
    );
    println!("Formula: round(level / levelsNeededPerIncrease * targetValueIncrease)");
    println!("With default expFactor=1.0, all multipliers=1.0\n");
    println!("{:>8} {:>20} {:>10}", "needed", "valueIncrease", "extra");
    for &levels_needed in &[1, 2, 3, 5, 10] {
        for &value_inc in &[1.0_f32, 2.0, 3.0, 5.0] {
            let extra = (level / levels_needed as f32 * value_inc).round() as i32;
            println!("{:>8} {:>20.1} {:>10}", levels_needed, value_inc, extra);
        }
    }
    println!("\nNote: actual target = max(reference, minTargetCount) + condition.targetValue + DifficultyIncrease");
    println!("We don't know the per-quest levelsNeededPerIncrease and targetValueIncrease");
    println!("values without BepInEx inspection of the ScriptableObject assets.");
}
