use dorfromantische2_rs::data::Terrain;
use dorfromantische2_rs::map::Map;
use dorfromantische2_rs::raw_data::SaveGame;
use std::io::Cursor;

fn main() {
    let data = std::fs::read("calibration/savegame.sav").unwrap();
    let parsed = nrbf_rs::parse_nrbf(&mut Cursor::new(&data));
    let sg = SaveGame::try_from(&parsed).unwrap();
    let map = Map::from(&sg);

    // Find all lake segments and their positions (deduplicated)
    let mut lake_positions: std::collections::HashSet<(i32, i32)> =
        std::collections::HashSet::new();
    for seg in &map.segments {
        if seg.terrain == Terrain::Lake {
            lake_positions.insert((seg.pos.x(), seg.pos.y()));
        }
    }
    let mut lake_positions: Vec<_> = lake_positions.into_iter().collect();
    lake_positions.sort_by_key(|p| (p.1, p.0));

    println!("Total lake tiles: {}", lake_positions.len());
    println!("\nSouthernmost 30 lake tiles:");
    for (x, y) in lake_positions.iter().take(30) {
        println!("  hex=({x}, {y})");
    }

    // Find the map extents
    let mut min_y = i32::MAX;
    let mut max_y = i32::MIN;
    for pos in map.iter_tile_positions() {
        min_y = min_y.min(pos.y());
        max_y = max_y.max(pos.y());
    }
    println!("\nMap Y range: {min_y} to {max_y}");

    // Find southern boundary shape (tiles at y < min_y + 20)
    let south_bound = min_y + 15;
    println!("\nSouthern tiles (y < {south_bound}):");
    let mut southern: Vec<_> = map
        .iter_tile_positions()
        .filter(|p| p.y() < south_bound)
        .collect();
    southern.sort_by_key(|p| (p.y(), p.x()));
    for pos in &southern {
        let has_lake = map.segments[map.tile_index[map.tile_key(*pos).unwrap()].unwrap().0..]
            .iter()
            .take(6)
            .any(|s| s.terrain == Terrain::Lake);
        let marker = if has_lake { "LAKE" } else { "    " };
        println!("  ({}, {}) {marker}", pos.x(), pos.y());
    }
}
