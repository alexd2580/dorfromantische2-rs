//! Print the current quest list with progress and status.
//! Run with: cargo run --example quests -- biggame.sav

use comfy_table::{presets::NOTHING, Cell, CellAlignment, Color, Table};
use dorfromantische2_rs::group_assignments::GroupAssignments;
use dorfromantische2_rs::map::{Map, QuestType};
use dorfromantische2_rs::raw_data::SaveGame;
use std::io::Cursor;

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "biggame.sav".into());
    let data = std::fs::read(&path).unwrap_or_else(|e| {
        eprintln!("Failed to read {path}: {e}");
        std::process::exit(1);
    });
    let parsed = nrbf_rs::parse_nrbf(&mut Cursor::new(&data));
    let savegame = SaveGame::try_from(&parsed).unwrap_or_else(|e| {
        eprintln!("Failed to parse savegame: {e}");
        std::process::exit(1);
    });

    let map = Map::from(&savegame);
    let groups = GroupAssignments::from(&map);

    struct QuestInfo {
        terrain: String,
        target: i32,
        quest_type: QuestType,
        unit_count: u32,
        remaining: i32,
        open_edges: usize,
    }

    let mut quests: Vec<QuestInfo> = Vec::new();
    for group in &groups.groups {
        for (quest, remaining) in group.remaining_per_quest() {
            if !quest.active {
                continue;
            }
            quests.push(QuestInfo {
                terrain: format!("{:?}", quest.terrain),
                target: quest.target_value,
                quest_type: quest.quest_type,
                unit_count: group.unit_count,
                remaining,
                open_edges: group.open_edges.len(),
            });
        }
    }

    let terrain_order = |t: &str| match t {
        "House" => 0,
        "Forest" => 1,
        "Wheat" => 2,
        "Rail" => 3,
        "River" => 4,
        _ => 5,
    };
    quests.sort_by(|a, b| {
        terrain_order(&a.terrain)
            .cmp(&terrain_order(&b.terrain))
            .then(a.remaining.cmp(&b.remaining))
    });

    let mut table = Table::new();
    table.load_preset(NOTHING).set_header(vec![
        Cell::new("Terrain"),
        Cell::new("Target").set_alignment(CellAlignment::Right),
        Cell::new("Type").set_alignment(CellAlignment::Center),
        Cell::new("Units").set_alignment(CellAlignment::Right),
        Cell::new("Left").set_alignment(CellAlignment::Right),
        Cell::new("Edges").set_alignment(CellAlignment::Right),
    ]);

    let mut last_terrain = String::new();
    for q in &quests {
        // Add separator between terrain groups.
        if q.terrain != last_terrain {
            last_terrain = q.terrain.clone();
        }

        let type_str = q.quest_type.label();

        let remaining_cell = if q.quest_type == QuestType::Flag && q.remaining <= 0 {
            Cell::new("close")
                .set_alignment(CellAlignment::Right)
                .fg(Color::Green)
        } else {
            let cell = Cell::new(q.remaining).set_alignment(CellAlignment::Right);
            if q.remaining <= 0 {
                cell.fg(Color::Green)
            } else if q.remaining <= 10 {
                cell.fg(Color::Cyan)
            } else {
                cell
            }
        };

        let terrain_color = match q.terrain.as_str() {
            "House" => Color::Red,
            "Forest" => Color::Green,
            "Wheat" => Color::Yellow,
            "Rail" => Color::White,
            "River" => Color::Blue,
            _ => Color::White,
        };

        table.add_row(vec![
            Cell::new(&q.terrain).fg(terrain_color),
            Cell::new(q.target).set_alignment(CellAlignment::Right),
            Cell::new(type_str).set_alignment(CellAlignment::Center),
            Cell::new(q.unit_count).set_alignment(CellAlignment::Right),
            remaining_cell,
            Cell::new(q.open_edges).set_alignment(CellAlignment::Right),
        ]);
    }

    println!("{table}");

    let total = quests.len();
    let fulfilled = quests.iter().filter(|q| q.remaining <= 0).count();
    let easy = quests
        .iter()
        .filter(|q| q.remaining > 0 && q.remaining <= 10)
        .count();

    println!("\n{total} active quests, {fulfilled} fulfilled, {easy} close to completion");
}
