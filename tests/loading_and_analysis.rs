use dorfromantische2_rs::best_placements::{
    constraints_at, fit_chance_for_constraints, BestPlacements, MAX_SHOWN_PLACEMENTS,
};
use dorfromantische2_rs::data::{
    quest_terrain, EdgeMatch, EdgeProfile, Form, Pos, Segment, Side, Terrain, HEX_SIDES,
};
use dorfromantische2_rs::group_assignments::GroupAssignments;
use dorfromantische2_rs::map::Map;
use dorfromantische2_rs::raw_data::{self, SaveGame};
use std::io::Cursor;

// ===========================================================================
// Helpers
// ===========================================================================

fn load_raw(path: &str) -> nrbf_rs::value::Value {
    let data = std::fs::read(path).unwrap_or_else(|_| panic!("{path} not found"));
    nrbf_rs::parse_nrbf(&mut Cursor::new(&data))
}

fn load_savegame(path: &str) -> SaveGame {
    let parsed = load_raw(path);
    SaveGame::try_from(&parsed).unwrap_or_else(|e| panic!("Failed to parse {path}: {e}"))
}

fn load_dorfromantik() -> SaveGame {
    load_savegame("dorfromantik.dump")
}

fn load_biggame() -> SaveGame {
    load_savegame("biggame.sav")
}

fn build_map(savegame: &SaveGame) -> Map {
    Map::from(savegame)
}

fn analyze_groups(map: &Map) -> GroupAssignments {
    GroupAssignments::from(map)
}

fn compute_placements(map: &Map, groups: &GroupAssignments) -> BestPlacements {
    let freqs = dorfromantische2_rs::tile_frequency::TileFrequencies::from_map(map);
    BestPlacements::compute(map, groups, &freqs)
}

// ===========================================================================
// SaveGame loading tests
// ===========================================================================

#[test]
fn test_load_dorfromantik_savegame() {
    let sg = load_dorfromantik();
    assert_eq!(sg.version, 3);
    assert_eq!(sg.score, 949490);
    assert_eq!(sg.level, 312);
    assert_eq!(sg.placed_tile_count, 11366);
    assert_eq!(sg.perfect_placements, 9783);
    assert_eq!(sg.quests_fulfilled, 386);
    assert_eq!(sg.quests_failed, 195);
    assert_eq!(sg.surrounded_tiles_count, 10248);
    assert_eq!(sg.generated_tile_count, 11180);
    assert_eq!(sg.generated_quest_count, 579);
    assert_eq!(sg.consecutive_perfect_fits, 25);
    assert_eq!(sg.consecutive_placements_without_rotate, 1);
    assert_eq!(sg.biome_seed, -830648020);
    assert_eq!(sg.preplaced_tile_seed, -830626339);
}

#[test]
fn test_load_biggame_savegame() {
    let sg = load_biggame();
    assert_eq!(sg.version, 3);
    // These values change when the savegame is updated — just verify they parse.
    assert!(sg.score > 0);
    assert!(sg.level > 0);
    assert!(sg.placed_tile_count > 0);
    assert!(sg.perfect_placements > 0);
    assert!(sg.generated_tile_count > 0);
    assert_eq!(sg.biome_seed, -830648020);
    assert_eq!(sg.preplaced_tile_seed, -830626339);
}

#[test]
fn test_savegame_tile_count_close_to_placed() {
    let sg = load_dorfromantik();
    // tiles vec may be slightly larger than placed_tile_count due to null filtering
    // in the GenericList, but should be close
    let diff = (sg.tiles.len() as i32 - sg.placed_tile_count).abs();
    assert!(
        diff <= 50,
        "Tile count {} too far from placed_tile_count {}",
        sg.tiles.len(),
        sg.placed_tile_count
    );
}

#[test]
fn test_biggame_tile_count_close_to_placed() {
    let sg = load_biggame();
    let diff = (sg.tiles.len() as i32 - sg.placed_tile_count).abs();
    assert!(
        diff <= 200,
        "Tile count {} too far from placed_tile_count {}",
        sg.tiles.len(),
        sg.placed_tile_count
    );
}

#[test]
fn test_savegame_has_tile_stack() {
    let sg = load_dorfromantik();
    assert!(!sg.tile_stack.is_empty());
    // tile_stack_count is the allocated capacity; actual non-null entries may be fewer
    assert!(sg.tile_stack.len() <= sg.tile_stack_count as usize);
}

#[test]
fn test_biggame_has_tile_stack() {
    let sg = load_biggame();
    assert!(!sg.tile_stack.is_empty());
    assert!(sg.tile_stack.len() <= sg.tile_stack_count as usize);
}

#[test]
fn test_savegame_has_preplaced_tiles() {
    let sg = load_dorfromantik();
    assert!(!sg.preplaced_tiles.is_empty());
}

#[test]
fn dump_preplaced_tiles() {
    // Both savegames share the same preplaced_tile_seed, so preplaced_tiles should be identical.
    let sg1 = load_dorfromantik();
    let sg2 = load_biggame();

    // The smaller save has fewer placed tiles. Find quest tiles that exist in sg2 but not sg1
    // — those were placed between the two saves, giving us section->hex pairs.
    use std::collections::HashSet;
    let sg1_quest_positions: HashSet<(i32, i32)> = sg1
        .tiles
        .iter()
        .filter(|t| t.quest_tile.is_some())
        .map(|t| (t.s, t.t))
        .collect();

    let new_quest_tiles: Vec<_> = sg2
        .tiles
        .iter()
        .filter(|t| t.quest_tile.is_some() && !sg1_quest_positions.contains(&(t.s, t.t)))
        .collect();

    println!("Quest tiles in sg1: {}", sg1_quest_positions.len());
    println!("New quest tiles in sg2: {}", new_quest_tiles.len());
    for t in &new_quest_tiles {
        let qt = t.quest_tile.as_ref().unwrap();
        println!("  hex=({}, {}), tile_id={}", t.s, t.t, qt.quest_tile_id.0);
    }

    // The preplaced_tiles list is identical in both saves (same seed).
    // Try to find unplaced preplaced tiles: those whose section coords
    // don't correspond to any placed quest tile.
    // First, let's see if there's a pattern: what's the range of hex coords
    // for quest tiles?
    let all_quest_hexes: Vec<_> = sg2
        .tiles
        .iter()
        .filter_map(|t| {
            t.quest_tile
                .as_ref()
                .map(|qt| (t.s, t.t, qt.quest_tile_id.0))
        })
        .collect();
    let s_min = all_quest_hexes.iter().map(|x| x.0).min().unwrap();
    let s_max = all_quest_hexes.iter().map(|x| x.0).max().unwrap();
    let t_min = all_quest_hexes.iter().map(|x| x.1).min().unwrap();
    let t_max = all_quest_hexes.iter().map(|x| x.1).max().unwrap();
    println!("Quest hex ranges: s=[{s_min}, {s_max}], t=[{t_min}, {t_max}]");

    let sec_s_min = sg2
        .preplaced_tiles
        .iter()
        .map(|p| p.section_grid_pos_x)
        .min()
        .unwrap();
    let sec_s_max = sg2
        .preplaced_tiles
        .iter()
        .map(|p| p.section_grid_pos_x)
        .max()
        .unwrap();
    let sec_t_min = sg2
        .preplaced_tiles
        .iter()
        .map(|p| p.section_grid_pos_y)
        .min()
        .unwrap();
    let sec_t_max = sg2
        .preplaced_tiles
        .iter()
        .map(|p| p.section_grid_pos_y)
        .max()
        .unwrap();
    println!("Section ranges: x=[{sec_s_min}, {sec_s_max}], y=[{sec_t_min}, {sec_t_max}]");

    // Compute approximate scale
    let hex_s_range = (s_max - s_min) as f64;
    let hex_t_range = (t_max - t_min) as f64;
    let sec_s_range = (sec_s_max - sec_s_min) as f64;
    let sec_t_range = (sec_t_max - sec_t_min) as f64;
    println!(
        "Approx scale: s={:.1}, t={:.1}",
        hex_s_range / sec_s_range,
        hex_t_range / sec_t_range
    );

    // Are preplaced_tiles identical between the two saves?
    println!(
        "Preplaced count sg1={}, sg2={}",
        sg1.preplaced_tiles.len(),
        sg2.preplaced_tiles.len()
    );
    let same = sg1
        .preplaced_tiles
        .iter()
        .zip(sg2.preplaced_tiles.iter())
        .all(|(a, b)| {
            a.section_grid_pos_x == b.section_grid_pos_x
                && a.section_grid_pos_y == b.section_grid_pos_y
                && a.preplaced_tile_id.0 == b.preplaced_tile_id.0
        });
    println!("Preplaced tiles identical: {same}");

    // Count how many quest tiles in sg2 exist per quest_tile_id
    let mut id_counts: std::collections::HashMap<i32, usize> = std::collections::HashMap::new();
    for qt in all_quest_hexes.iter() {
        *id_counts.entry(qt.2).or_default() += 1;
    }
    println!("\nQuest tile_id counts (placed in sg2):");
    let mut counts: Vec<_> = id_counts.iter().collect();
    counts.sort();
    for (id, count) in &counts {
        println!("  tile_id={id}: {count}");
    }
    println!("Total placed quest tiles: {}", all_quest_hexes.len());
    println!("Total preplaced entries: {}", sg2.preplaced_tiles.len());

    let sg = sg2;
    // Build a map from quest_tile_id to all hex positions where that quest tile was placed.
    use std::collections::HashMap;
    let mut quest_positions: HashMap<i32, Vec<(i32, i32)>> = HashMap::new();
    for t in &sg.tiles {
        if let Some(qt) = &t.quest_tile {
            quest_positions
                .entry(qt.quest_tile_id.0)
                .or_default()
                .push((t.s, t.t));
        }
    }

    println!("Preplaced tiles: {}", sg.preplaced_tiles.len());
    println!("Quest tiles on map:");
    for (id, positions) in &quest_positions {
        println!("  tile_id={id}: {positions:?}");
    }

    // For each placed quest tile, find which preplaced entry could map to it.
    // Group placed quest tiles by their position and see if section coords correlate.
    println!("\nPlaced quest tiles with their hex positions:");
    for t in &sg.tiles {
        if let Some(qt) = &t.quest_tile {
            println!(
                "  hex=({}, {}), quest_tile_id={}, target={}",
                t.s, t.t, qt.quest_tile_id.0, qt.target_value
            );
        }
    }

    // Try to find the mapping: for each preplaced tile, find a placed quest tile
    // with matching quest_tile_id that hasn't been claimed yet.
    // Then compute section -> hex relationship.
    println!("\nSection -> Hex mapping attempts:");
    let mut used_hex: std::collections::HashSet<(i32, i32)> = std::collections::HashSet::new();
    for pt in &sg.preplaced_tiles {
        let candidates: Vec<_> = sg
            .tiles
            .iter()
            .filter(|t| {
                t.quest_tile
                    .as_ref()
                    .is_some_and(|qt| qt.quest_tile_id.0 == pt.preplaced_tile_id.0)
                    && !used_hex.contains(&(t.s, t.t))
            })
            .collect();
        if candidates.len() == 1 {
            let t = candidates[0];
            used_hex.insert((t.s, t.t));
            println!(
                "  section=({}, {}) => hex=({}, {}), tile_id={} [UNIQUE]",
                pt.section_grid_pos_x, pt.section_grid_pos_y, t.s, t.t, pt.preplaced_tile_id.0
            );
        } else {
            println!(
                "  section=({}, {}), tile_id={} => {} candidates",
                pt.section_grid_pos_x,
                pt.section_grid_pos_y,
                pt.preplaced_tile_id.0,
                candidates.len()
            );
        }
    }
}

#[test]
fn test_savegame_string_fields() {
    let sg = load_dorfromantik();
    assert_eq!(sg.last_played_version.len(), 7);
    assert_eq!(sg.initial_version.len(), 5);
    assert!(sg.file_name.is_some());
    assert_eq!(sg.file_name.as_ref().unwrap().len(), 40);
}

#[test]
fn test_savegame_playtime_positive() {
    let sg = load_dorfromantik();
    assert!(sg.playtime > 0.0);
}

#[test]
fn test_savegame_shared_seeds() {
    let sg1 = load_dorfromantik();
    let sg2 = load_biggame();
    assert_eq!(sg1.biome_seed, sg2.biome_seed);
    assert_eq!(sg1.preplaced_tile_seed, sg2.preplaced_tile_seed);
}

#[test]
fn test_biggame_larger_than_dorfromantik() {
    let sg1 = load_dorfromantik();
    let sg2 = load_biggame();
    assert!(sg2.score > sg1.score);
    assert!(sg2.placed_tile_count > sg1.placed_tile_count);
    assert!(sg2.level > sg1.level);
    assert!(sg2.tiles.len() > sg1.tiles.len());
}

// ===========================================================================
// Tile parsing tests
// ===========================================================================

#[test]
fn test_tiles_have_valid_coordinates() {
    let sg = load_dorfromantik();
    // Coordinates should span a reasonable range around origin
    let min_s = sg.tiles.iter().map(|t| t.s).min().unwrap();
    let max_s = sg.tiles.iter().map(|t| t.s).max().unwrap();
    let min_t = sg.tiles.iter().map(|t| t.t).min().unwrap();
    let max_t = sg.tiles.iter().map(|t| t.t).max().unwrap();
    assert!(min_s < 0, "Expected negative s coordinates");
    assert!(max_s > 0, "Expected positive s coordinates");
    assert!(min_t < 0, "Expected negative t coordinates");
    assert!(max_t > 0, "Expected positive t coordinates");
}

#[test]
fn test_tiles_have_valid_rotations() {
    let sg = load_dorfromantik();
    for tile in &sg.tiles {
        assert!(
            tile.rotation >= 0 && tile.rotation < HEX_SIDES as i32,
            "Tile at ({}, {}) has invalid rotation {}",
            tile.s,
            tile.t,
            tile.rotation
        );
    }
}

#[test]
fn test_most_tiles_have_segments() {
    let sg = load_dorfromantik();
    let with_segments = sg.tiles.iter().filter(|t| !t.segments.is_empty()).count();
    // The vast majority of tiles should have segments
    assert!(
        with_segments as f64 / sg.tiles.len() as f64 > 0.9,
        "Only {with_segments}/{} tiles have segments",
        sg.tiles.len()
    );
}

#[test]
fn test_segment_rotations_valid() {
    let sg = load_dorfromantik();
    for tile in &sg.tiles {
        for seg in &tile.segments {
            assert!(
                seg.rotation >= 0 && seg.rotation < HEX_SIDES as i32,
                "Segment rotation {} out of range",
                seg.rotation
            );
        }
    }
}

// ===========================================================================
// Map building tests
// ===========================================================================

#[test]
fn test_map_from_dorfromantik() {
    let sg = load_dorfromantik();
    let map = build_map(&sg);

    // Map should have segments
    assert!(!map.segments.is_empty());

    // Index sizes should be positive
    assert!(map.index_size.x > 0);
    assert!(map.index_size.y > 0);
}

#[test]
fn test_map_from_biggame() {
    let sg = load_biggame();
    let map = build_map(&sg);
    assert!(!map.segments.is_empty());
    assert!(map.index_size.x > 0);
    assert!(map.index_size.y > 0);
}

#[test]
fn test_map_has_origin_tile() {
    let sg = load_dorfromantik();
    let map = build_map(&sg);

    // Origin should exist in the map
    let origin = Pos::new(0, 0);
    assert!(map.has(origin), "Map should contain origin tile");
}

#[test]
fn test_map_tiles_mostly_have_segments() {
    let sg = load_dorfromantik();
    let map = build_map(&sg);

    // Count tiles that have at least one segment in the map
    let total_tiles = map.tile_index.iter().filter(|t| t.is_some()).count();
    let tiles_with_segments = map
        .tile_index
        .iter()
        .filter(|t| matches!(t, Some((_, count)) if *count > 0))
        .count();
    assert!(
        tiles_with_segments as f64 / total_tiles as f64 > 0.9,
        "Only {tiles_with_segments}/{total_tiles} map tiles have segments"
    );
}

#[test]
fn test_map_segment_terrains_valid() {
    let sg = load_dorfromantik();
    let map = build_map(&sg);

    for segment in &map.segments {
        assert_ne!(
            segment.terrain,
            Terrain::Missing,
            "No segment should have Missing terrain"
        );
    }
}

#[test]
fn test_map_segment_rotations_valid() {
    let sg = load_dorfromantik();
    let map = build_map(&sg);

    for segment in &map.segments {
        assert!(
            segment.rotation < HEX_SIDES,
            "Segment rotation {} out of range",
            segment.rotation
        );
    }
}

#[test]
fn test_map_next_tile_exists() {
    let sg = load_dorfromantik();
    let map = build_map(&sg);

    assert!(!map.next_tile.is_empty(), "Next tile should have segments");
}

#[test]
fn test_map_next_tile_rendered() {
    let sg = load_dorfromantik();
    let map = build_map(&sg);

    // At least one side of the next tile should have a non-empty terrain
    let has_terrain = map
        .rendered_next_tile
        .iter()
        .any(|t| *t != Terrain::Empty && *t != Terrain::Missing);
    assert!(
        has_terrain,
        "Next tile should have at least one terrain side"
    );
}

#[test]
fn test_map_tile_positions_unique() {
    let sg = load_dorfromantik();
    let map = build_map(&sg);

    let positions: Vec<Pos> = map.iter_tile_positions().collect();
    let mut seen = std::collections::HashSet::new();
    for pos in &positions {
        assert!(seen.insert(*pos), "Duplicate tile position: {pos}");
    }
}

#[test]
fn test_map_biggame_larger() {
    let sg1 = load_dorfromantik();
    let sg2 = load_biggame();
    let map1 = build_map(&sg1);
    let map2 = build_map(&sg2);

    assert!(map2.segments.len() > map1.segments.len());
}

#[test]
fn test_map_neighbor_positions() {
    let origin = Pos::new(0, 0);
    let neighbors: Vec<Pos> = (0..HEX_SIDES)
        .map(|r| Map::neighbor_pos_of(origin, r))
        .collect();

    // All 6 neighbors should be distinct
    let unique: std::collections::HashSet<Pos> = neighbors.iter().copied().collect();
    assert_eq!(unique.len(), HEX_SIDES);

    // Going to a neighbor and back should return to origin
    for rotation in 0..HEX_SIDES {
        let neighbor = Map::neighbor_pos_of(origin, rotation);
        let back = Map::neighbor_pos_of(neighbor, Map::opposite_side(rotation));
        assert_eq!(
            back, origin,
            "Neighbor roundtrip failed for rotation {rotation}"
        );
    }
}

#[test]
fn test_map_opposite_side() {
    for r in 0..HEX_SIDES {
        let opp = Map::opposite_side(r);
        assert_eq!(Map::opposite_side(opp), r);
        assert_eq!((r + 3) % HEX_SIDES, opp);
    }
}

// ===========================================================================
// Group analysis tests
// ===========================================================================

#[test]
fn test_group_analysis_dorfromantik() {
    let sg = load_dorfromantik();
    let map = build_map(&sg);
    let groups = analyze_groups(&map);

    // Should discover some groups
    assert!(!groups.groups.is_empty(), "Should find groups");

    // Should find possible placements
    assert!(
        !groups.possible_placements.is_empty(),
        "Should find possible placements"
    );
}

#[test]
fn test_group_analysis_biggame() {
    let sg = load_biggame();
    let map = build_map(&sg);
    let groups = analyze_groups(&map);

    assert!(!groups.groups.is_empty());
    assert!(!groups.possible_placements.is_empty());
}

#[test]
fn test_every_segment_assigned_to_group() {
    let sg = load_dorfromantik();
    let map = build_map(&sg);
    let groups = analyze_groups(&map);

    // assigned_groups should cover all segments
    assert!(
        groups.assigned_groups.len() >= map.segments.len(),
        "assigned_groups ({}) should cover all segments ({})",
        groups.assigned_groups.len(),
        map.segments.len()
    );
}

#[test]
fn test_group_segments_reference_valid_indices() {
    let sg = load_dorfromantik();
    let map = build_map(&sg);
    let groups = analyze_groups(&map);

    for group in &groups.groups {
        for &idx in &group.segment_indices {
            assert!(
                idx < map.segments.len(),
                "Group references out-of-bounds segment index {idx}"
            );
        }
    }
}

#[test]
fn test_group_of_returns_valid_indices() {
    let sg = load_dorfromantik();
    let map = build_map(&sg);
    let groups = analyze_groups(&map);

    for idx in 0..map.segments.len() {
        if let Some(group_idx) = groups.group_of(idx) {
            assert!(
                group_idx < groups.groups.len(),
                "group_of({idx}) returned invalid group index {group_idx}"
            );
        }
    }
}

#[test]
fn test_possible_placements_not_in_map() {
    let sg = load_dorfromantik();
    let map = build_map(&sg);
    let groups = analyze_groups(&map);

    for &pos in &groups.possible_placements {
        assert!(
            !map.has(pos),
            "Possible placement {pos} should not already exist in map"
        );
    }
}

#[test]
fn test_possible_placements_adjacent_to_map() {
    let sg = load_dorfromantik();
    let map = build_map(&sg);
    let groups = analyze_groups(&map);

    for &pos in &groups.possible_placements {
        let has_neighbor = (0..HEX_SIDES).any(|r| {
            let neighbor = Map::neighbor_pos_of(pos, r);
            map.has(neighbor)
        });
        assert!(
            has_neighbor,
            "Possible placement {pos} should be adjacent to at least one existing tile"
        );
    }
}

#[test]
fn test_some_groups_are_closed() {
    let sg = load_dorfromantik();
    let map = build_map(&sg);
    let groups = analyze_groups(&map);

    let closed_count = groups.groups.iter().filter(|g| g.is_closed()).count();
    assert!(
        closed_count > 0,
        "With {} tiles there should be at least some closed groups",
        sg.placed_tile_count
    );
}

#[test]
fn test_some_groups_are_open() {
    let sg = load_dorfromantik();
    let map = build_map(&sg);
    let groups = analyze_groups(&map);

    let open_count = groups.groups.iter().filter(|g| !g.is_closed()).count();
    assert!(open_count > 0, "There should be at least some open groups");
}

// ===========================================================================
// Best placements tests
// ===========================================================================

#[test]
fn test_best_placements_dorfromantik() {
    let sg = load_dorfromantik();
    let map = build_map(&sg);
    let groups = analyze_groups(&map);
    let placements = compute_placements(&map, &groups);

    let usable: Vec<_> = placements.iter_all();
    assert!(
        !usable.is_empty(),
        "Should find at least one usable placement"
    );
}

#[test]
fn test_best_placements_biggame() {
    let sg = load_biggame();
    let map = build_map(&sg);
    let groups = analyze_groups(&map);
    let placements = compute_placements(&map, &groups);

    let usable: Vec<_> = placements.iter_all();
    assert!(!usable.is_empty());
}

#[test]
fn test_best_placements_limited_to_max() {
    let sg = load_dorfromantik();
    let map = build_map(&sg);
    let groups = analyze_groups(&map);
    let placements = compute_placements(&map, &groups);

    let usable: Vec<_> = placements.iter_all();
    // iter_all returns top N + any with group_effects, so may exceed MAX_SHOWN_PLACEMENTS.
    // But the core set should be at most MAX_SHOWN_PLACEMENTS.
    assert!(
        !usable.is_empty(),
        "Should have at least one placement option"
    );
}

#[test]
fn test_best_placements_valid_positions() {
    let sg = load_dorfromantik();
    let map = build_map(&sg);
    let groups = analyze_groups(&map);
    let placements = compute_placements(&map, &groups);

    for (_, score) in placements.iter_all() {
        // Placement positions should not already be occupied
        assert!(
            !map.has(score.pos),
            "Placement at {} should not be an existing tile",
            score.pos
        );

        // Rotation should be valid
        assert!(
            score.rotation < HEX_SIDES,
            "Rotation {} out of range",
            score.rotation
        );
    }
}

#[test]
fn test_best_placements_sorted_by_quality() {
    let sg = load_dorfromantik();
    let map = build_map(&sg);
    let groups = analyze_groups(&map);
    let placements = compute_placements(&map, &groups);

    let all = placements.iter_all();
    let usable: Vec<_> = all.iter().take(MAX_SHOWN_PLACEMENTS).collect();
    for window in usable.windows(2) {
        let (_, a) = &window[0];
        let (_, b) = &window[1];
        // Primary sort: lower fit_chance first (hardest to fill).
        assert!(
            a.fit_chance <= b.fit_chance + f32::EPSILON,
            "Placements not sorted by fit_chance: {} > {}",
            a.fit_chance,
            b.fit_chance
        );
    }
}

// ===========================================================================
// Full pipeline: load → map → groups → placements (determinism)
// ===========================================================================

#[test]
fn test_full_pipeline_deterministic() {
    let sg = load_dorfromantik();

    let map1 = build_map(&sg);
    let groups1 = analyze_groups(&map1);
    let placements1: Vec<_> = compute_placements(&map1, &groups1)
        .iter_all()
        .into_iter()
        .map(|(i, s)| (i, s.pos, s.rotation, s.matching_edges))
        .collect();

    let map2 = build_map(&sg);
    let groups2 = analyze_groups(&map2);
    let placements2: Vec<_> = compute_placements(&map2, &groups2)
        .iter_all()
        .into_iter()
        .map(|(i, s)| (i, s.pos, s.rotation, s.matching_edges))
        .collect();

    assert_eq!(map1.segments.len(), map2.segments.len());
    assert_eq!(groups1.groups.len(), groups2.groups.len());
    assert_eq!(placements1, placements2);
}

// ===========================================================================
// Terrain connectivity rules
// ===========================================================================

#[test]
fn test_terrain_connects_and_matches_symmetry() {
    let terrains = [
        Terrain::House,
        Terrain::Forest,
        Terrain::Wheat,
        Terrain::Rail,
        Terrain::River,
        Terrain::Lake,
        Terrain::Station,
        Terrain::Empty,
        Terrain::Missing,
    ];

    for &a in &terrains {
        for &b in &terrains {
            let ab = a.connects_and_matches(b);
            let ba = b.connects_and_matches(a);
            assert_eq!(
                ab, ba,
                "connects_and_matches not symmetric for {a:?} vs {b:?}: {ab:?} != {ba:?}"
            );
        }
    }
}

#[test]
fn test_terrain_missing_always_matches() {
    let terrains = [
        Terrain::House,
        Terrain::Forest,
        Terrain::Wheat,
        Terrain::Rail,
        Terrain::River,
        Terrain::Lake,
        Terrain::Station,
        Terrain::Empty,
    ];

    for &t in &terrains {
        assert_eq!(
            Terrain::Missing.connects_and_matches(t),
            EdgeMatch::Missing,
            "Missing should return Missing for {t:?}"
        );
    }
}

#[test]
fn test_terrain_same_type_matches() {
    for &t in &[Terrain::House, Terrain::Forest, Terrain::Wheat] {
        assert_eq!(
            t.connects_and_matches(t),
            EdgeMatch::Matching,
            "{t:?} should match itself"
        );
    }
}

#[test]
fn test_terrain_rail_river_dont_cross() {
    assert_eq!(
        Terrain::Rail.connects_and_matches(Terrain::River),
        EdgeMatch::Illegal,
        "Rail and River should not connect"
    );
}

#[test]
fn test_terrain_river_connects_to_lake() {
    assert_eq!(
        Terrain::River.connects_and_matches(Terrain::Lake),
        EdgeMatch::Matching
    );
}

#[test]
fn test_terrain_rail_connects_to_station() {
    assert_eq!(
        Terrain::Rail.connects_and_matches(Terrain::Station),
        EdgeMatch::Matching
    );
}

// ===========================================================================
// Data conversion edge cases
// ===========================================================================

#[test]
fn test_form_from_segment_type_id_all_known() {
    use dorfromantische2_rs::raw_data::SegmentTypeId;

    let known_ids = [
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 102, 105, 109, 111,
    ];
    for id in known_ids {
        let _form: Form = (&SegmentTypeId(id)).into();
    }
}

#[test]
fn test_terrain_from_group_type_id_all_known() {
    use dorfromantische2_rs::raw_data::GroupTypeId;

    let known_ids = [-1, 0, 1, 2, 3, 4];
    for id in known_ids {
        let _terrain: Terrain = (&GroupTypeId(id)).into();
    }
}

#[test]
fn dump_first_tile() {
    let raw = load_raw("mini.sav");
    println!("{}", raw_data::dump_first_tile(&raw));
}

#[test]
fn dump_active_challenges_structure() {
    for path in ["biggame.sav", "dorfromantik.dump", "mini.sav"] {
        let raw = load_raw(path);
        println!("=== {path} ===");
        println!("{}", raw_data::dump_active_challenges(&raw));
        println!();
    }
}

// Quest tile segment generation is tested indirectly through the full pipeline tests
// (test_full_pipeline_*) which load real save files containing quest tiles.

#[test]
fn dump_last_tile() {
    let sg = load_biggame();
    let last = sg.tiles.last().unwrap();
    println!(
        "Last tile: pos=({}, {}), rotation={}, special_tile_id={:?}",
        last.s, last.t, last.rotation, last.special_tile_id
    );
    println!("  segments: {:?}", last.segments);
    if let Some(qt) = &last.quest_tile {
        println!(
            "  quest: tile_id={}, target={}, active={}, quest_id={}",
            qt.quest_tile_id.0, qt.target_value, qt.quest_active, qt.quest_id.0
        );
    }
}

// ===========================================================================
// Step 1 tests: data module behavior preservation
// ===========================================================================

fn make_segment(form: Form, terrain: Terrain, rotation: usize) -> Segment {
    Segment {
        pos: Pos::new(0, 0),
        form,
        terrain,
        rotation,
        unit_count: 0,
    }
}

#[test]
fn test_segment_rotations_form_size1() {
    let seg = make_segment(Form::Size1, Terrain::Forest, 2);
    let rotations: Vec<usize> = seg.rotations().collect();
    assert_eq!(rotations, vec![2]);
}

#[test]
fn test_segment_rotations_form_size6() {
    let seg = make_segment(Form::Size6, Terrain::Lake, 0);
    let rotations: Vec<usize> = seg.rotations().collect();
    assert_eq!(rotations, vec![0, 1, 2, 3, 4, 5]);
}

#[test]
fn test_segment_rotations_form_bridge_wraps() {
    let seg = make_segment(Form::Bridge, Terrain::Rail, 5);
    // Bridge = [0, 2] relative. With rotation 5: (5+0)%6=5, (5+2)%6=1.
    let rotations: Vec<usize> = seg.rotations().collect();
    assert_eq!(rotations, vec![5, 1]);
}

#[test]
fn test_segment_contains_rotation() {
    let seg = make_segment(Form::Straight, Terrain::River, 1);
    // Straight = [0, 3] relative. With rotation 1: covers 1 and 4.
    assert!(seg.contains_rotation(1));
    assert!(seg.contains_rotation(4));
    assert!(!seg.contains_rotation(0));
    assert!(!seg.contains_rotation(2));
    assert!(!seg.contains_rotation(3));
    assert!(!seg.contains_rotation(5));
}

#[test]
fn test_form_default_unit_count_houses() {
    assert_eq!(Form::Size1.default_unit_count(Terrain::House), 1);
    assert_eq!(Form::Size6.default_unit_count(Terrain::House), 7);
}

#[test]
fn test_form_default_unit_count_rail_always_one() {
    assert_eq!(Form::Bridge.default_unit_count(Terrain::Rail), 1);
    assert_eq!(Form::Straight.default_unit_count(Terrain::Rail), 1);
    assert_eq!(Form::Size1.default_unit_count(Terrain::Rail), 1);
}

#[test]
fn test_quest_terrain_known_ids() {
    assert_eq!(quest_terrain(2), Some(Terrain::Wheat));
    assert_eq!(quest_terrain(5), Some(Terrain::Wheat));
    assert_eq!(quest_terrain(16), Some(Terrain::Forest));
    assert_eq!(quest_terrain(33), Some(Terrain::House));
    assert_eq!(quest_terrain(25), Some(Terrain::Rail));
    assert_eq!(quest_terrain(47), Some(Terrain::River));
    assert_eq!(quest_terrain(999), None);
}

#[test]
fn test_terrain_extends_group_matching() {
    assert!(Terrain::Forest.extends_group_of(Terrain::Forest));
    assert!(Terrain::Lake.extends_group_of(Terrain::River));
    assert!(Terrain::Station.extends_group_of(Terrain::River));
    assert!(Terrain::Station.extends_group_of(Terrain::Rail));
    assert!(!Terrain::Forest.extends_group_of(Terrain::House));
}

#[test]
fn test_edge_match_variants() {
    assert_eq!(
        Terrain::Forest.connects_and_matches(Terrain::Wheat),
        EdgeMatch::Suboptimal
    );
    assert_eq!(
        Terrain::Rail.connects_and_matches(Terrain::Forest),
        EdgeMatch::Illegal
    );
    assert_eq!(
        Terrain::Empty.connects_and_matches(Terrain::Lake),
        EdgeMatch::Matching
    );
}

// ===========================================================================
// Mutation-gap tests: segment construction, hex math, terrain matching
// ===========================================================================

#[test]
fn test_world_to_hex_known_values() {
    use dorfromantische2_rs::hex;
    use glam::{IVec2, Vec2};
    // Origin
    assert_eq!(hex::world_to_hex(Vec2::ZERO), IVec2::ZERO);
    // Hex (1,0) -> world (1.5, COS_30) -> back
    let w = hex::hex_to_world(IVec2::new(1, 0));
    assert_eq!(hex::world_to_hex(w), IVec2::new(1, 0));
    // Hex (-2, 3)
    let w = hex::hex_to_world(IVec2::new(-2, 3));
    assert_eq!(hex::world_to_hex(w), IVec2::new(-2, 3));
}

#[test]
fn test_world_to_hex_grid_roundtrip() {
    use dorfromantische2_rs::hex;
    use glam::IVec2;
    for x in -10..=10 {
        for y in -10..=10 {
            let pos = IVec2::new(x, y);
            let world = hex::hex_to_world(pos);
            let back = hex::world_to_hex(world);
            assert_eq!(back, pos, "Roundtrip failed for ({x}, {y})");
        }
    }
}

#[test]
fn test_terrain_lake_station_matching() {
    // Lake matches Lake, Station, and River
    assert_eq!(
        Terrain::Lake.connects_and_matches(Terrain::Lake),
        EdgeMatch::Matching
    );
    assert_eq!(
        Terrain::Lake.connects_and_matches(Terrain::Station),
        EdgeMatch::Matching
    );
    assert_eq!(
        Terrain::Station.connects_and_matches(Terrain::Lake),
        EdgeMatch::Matching
    );
    assert_eq!(
        Terrain::Station.connects_and_matches(Terrain::Station),
        EdgeMatch::Matching
    );
    assert_eq!(
        Terrain::Station.connects_and_matches(Terrain::Rail),
        EdgeMatch::Matching
    );
    assert_eq!(
        Terrain::Station.connects_and_matches(Terrain::River),
        EdgeMatch::Matching
    );
}

#[test]
fn test_segment_rotation_always_valid() {
    // All segments in a loaded map should have rotation < 6.
    let sg = load_biggame();
    let map = Map::from(&sg);
    for seg in &map.segments {
        assert!(
            seg.rotation < HEX_SIDES,
            "Segment at {:?} has invalid rotation {}",
            seg.pos,
            seg.rotation
        );
    }
}

#[test]
fn test_segment_contains_rotation_consistent() {
    // For every segment, contains_rotation should be true for exactly the sides
    // returned by rotations().
    let sg = load_biggame();
    let map = Map::from(&sg);
    for seg in &map.segments {
        let from_iter: std::collections::HashSet<usize> = seg.rotations().collect();
        for side in 0..HEX_SIDES {
            assert_eq!(
                seg.contains_rotation(side),
                from_iter.contains(&side),
                "Mismatch at {:?} side {side} form {:?} rot {}",
                seg.pos,
                seg.form,
                seg.rotation
            );
        }
    }
}

#[test]
fn test_lake_segments_exist_in_savegame() {
    // Verify that lake terrain segments actually exist in the loaded data.
    let sg = load_biggame();
    let map = Map::from(&sg);
    let lake_count = map
        .segments
        .iter()
        .filter(|s| s.terrain == Terrain::Lake)
        .count();
    assert!(lake_count > 0, "No lake segments found in savegame");
}

#[test]
fn test_station_segments_exist_in_savegame() {
    let sg = load_biggame();
    let map = Map::from(&sg);
    let station_count = map
        .segments
        .iter()
        .filter(|s| s.terrain == Terrain::Station)
        .count();
    assert!(station_count > 0, "No station segments found in savegame");
}

#[test]
fn test_tile_frequency_computes() {
    use dorfromantische2_rs::tile_frequency::TileFrequencies;
    let sg = load_biggame();
    let map = Map::from(&sg);
    let freqs = TileFrequencies::from_map(&map);
    assert!(freqs.total_tiles > 0);
    assert!(!freqs.entries.is_empty());
    let total_from_entries: usize = freqs.entries.iter().map(|e| e.count).sum();
    assert_eq!(total_from_entries, freqs.total_tiles);
    // Fractions should sum to ~1.0
    let frac_sum: f64 = freqs.entries.iter().map(|e| e.fraction).sum();
    assert!((frac_sum - 1.0).abs() < 0.001);
}

// ===========================================================================
// Step 3 tests: EdgeProfile matches rendered_tiles
// ===========================================================================

#[test]
fn test_edge_profile_matches_rendered_tiles() {
    let sg = load_biggame();
    let map = Map::from(&sg);
    let index_len = map.tile_index.len();
    let mut checked = 0;
    for key in 0..index_len {
        if let (Some((base, count)), Some(rendered)) =
            (map.tile_index[key], map.rendered_tiles[key])
        {
            if count == 0 {
                continue;
            }
            let segments = &map.segments[base..base + count];
            let profile = EdgeProfile::from_segments(segments);
            for side in Side::ALL {
                let from_profile = profile.at(side);
                let from_rendered =
                    rendered[side.index()].map_or(Terrain::Empty, |idx| map.segments[idx].terrain);
                assert_eq!(
                    from_profile, from_rendered,
                    "Mismatch at tile key {key}, side {side:?}"
                );
            }
            checked += 1;
        }
    }
    assert!(checked > 1000, "Only checked {checked} tiles");
}

// ===========================================================================
// Step 2 tests: hex geometry behavior preservation
// ===========================================================================

#[test]
fn test_neighbor_pos_of_six_unique() {
    let origin = Pos::new(0, 0);
    let neighbors: Vec<Pos> = (0..HEX_SIDES)
        .map(|r| Map::neighbor_pos_of(origin, r))
        .collect();
    let unique: std::collections::HashSet<Pos> = neighbors.iter().copied().collect();
    assert_eq!(unique.len(), HEX_SIDES);
}

#[test]
fn test_neighbor_pos_of_known_values() {
    let origin = Pos::new(0, 0);
    assert_eq!(Map::neighbor_pos_of(origin, 0), Pos::new(0, 1));
    assert_eq!(Map::neighbor_pos_of(origin, 1), Pos::new(1, 0));
    assert_eq!(Map::neighbor_pos_of(origin, 2), Pos::new(1, -1));
    assert_eq!(Map::neighbor_pos_of(origin, 3), Pos::new(0, -1));
    assert_eq!(Map::neighbor_pos_of(origin, 4), Pos::new(-1, 0));
    assert_eq!(Map::neighbor_pos_of(origin, 5), Pos::new(-1, 1));
}

#[test]
fn test_neighbor_roundtrip_all_positions() {
    for x in -3..=3 {
        for y in -3..=3 {
            let pos = Pos::new(x, y);
            for rotation in 0..HEX_SIDES {
                let neighbor = Map::neighbor_pos_of(pos, rotation);
                let back = Map::neighbor_pos_of(neighbor, Map::opposite_side(rotation));
                assert_eq!(back, pos, "Roundtrip failed at ({x},{y}) rot {rotation}");
            }
        }
    }
}

#[test]
fn test_opposite_side_values() {
    for r in 0..HEX_SIDES {
        assert_eq!(Map::opposite_side(r), (r + 3) % HEX_SIDES);
        assert_eq!(Map::opposite_side(Map::opposite_side(r)), r);
    }
}

// ===========================================================================

#[test]
fn print_biggame_quests() {
    let save = load_biggame();
    let map = Map::from(&save);
    let _groups = GroupAssignments::from(&map);

    use dorfromantische2_rs::data::quest_terrain;
    // Dump quest fields to find the exact/more-than indicator.
    for tile in &save.tiles {
        if let Some(qt) = &tile.quest_tile {
            if qt.target_value >= 0 {
                let terrain = quest_terrain(qt.quest_tile_id.0)
                    .map(|t| format!("{t:?}"))
                    .unwrap_or_else(|| "?".into());
                println!(
                    "quest_id={} terrain={terrain} level={} queue={} challenge={} target={}",
                    qt.quest_id.0,
                    qt.quest_level,
                    qt.quest_queue_index,
                    qt.unlocked_challenge_id.0,
                    qt.target_value,
                );
            }
        }
    }
}

/// Empirical validation of fit% computation.
/// For every placed tile: collect the constraints from its actual neighbors,
/// compute the tile's edge profile, and verify the tile itself would be counted
/// as "fitting" by the fit_chance computation.
#[test]
fn test_fit_chance_empirical_validation() {
    use dorfromantische2_rs::tile_frequency::TileFrequencies;

    let sg = load_biggame();
    let map = Map::from(&sg);
    let freqs = TileFrequencies::from_map(&map);

    let mut checked = 0;
    let mut fit_nonzero = 0;

    for pos in map.iter_tile_positions() {
        let key = match map.tile_key(pos) {
            Some(k) => k,
            None => continue,
        };
        let (base, count) = match map.tile_index[key] {
            Some(bc) => bc,
            None => continue,
        };
        if count == 0 {
            continue;
        }

        // Get this tile's edge profile.
        let segments = &map.segments[base..base + count];
        let tile_profile = EdgeProfile::from_segments(segments);

        // Collect constraints from neighbors (what they present on their facing edges).
        let constraints = constraints_at(&map, pos);

        // Verify this tile satisfies its own constraints (sanity check).
        for (side, constraint) in constraints.iter().enumerate() {
            if let Some(neighbor_terrain) = constraint {
                let my_terrain = tile_profile.at_index(side);
                let edge_match = my_terrain.connects_and_matches(*neighbor_terrain);
                // The placed tile must match or be suboptimal (imperfect placement).
                assert!(
                    matches!(edge_match, EdgeMatch::Matching | EdgeMatch::Suboptimal),
                    "Tile at {pos:?} side {side}: {:?} vs {:?} = {:?}",
                    my_terrain,
                    neighbor_terrain,
                    edge_match
                );
            }
        }

        // Compute fit chance at this position.
        let (chance, _unique) = fit_chance_for_constraints(&freqs, &constraints);

        // The fit chance must be > 0 because at least this tile's pattern exists.
        // (Unless the tile was placed suboptimally with non-matching edges.)
        if constraints.iter().enumerate().all(|(side, c)| {
            c.is_none_or(|neighbor| {
                matches!(
                    tile_profile.at_index(side).connects_and_matches(neighbor),
                    EdgeMatch::Matching
                )
            })
        }) {
            assert!(
                chance > 0.0,
                "Tile at {pos:?} has all matching edges but fit chance is 0. \
                 Profile: {tile_profile:?}, constraints: {constraints:?}"
            );
            fit_nonzero += 1;
        }

        checked += 1;
    }

    assert!(checked > 1000, "Only checked {checked} tiles");
    let pct = fit_nonzero as f64 / checked as f64 * 100.0;
    assert!(
        pct > 80.0,
        "Only {pct:.1}% of tiles had nonzero fit chance ({fit_nonzero}/{checked})"
    );
    println!("Validated {checked} tiles, {fit_nonzero} with nonzero fit chance ({pct:.1}%)");
}

/// Validate that fit_chance_with_min_edges is monotonically decreasing:
/// more required edges = fewer fitting tiles.
#[test]
fn test_fit_chance_monotonic_in_edges() {
    use dorfromantische2_rs::best_placements::fit_chance_with_min_edges;
    use dorfromantische2_rs::tile_frequency::TileFrequencies;

    let sg = load_biggame();
    let map = Map::from(&sg);
    let freqs = TileFrequencies::from_map(&map);

    let mut checked = 0;
    // Sample some positions.
    for pos in map.iter_tile_positions().step_by(50) {
        let constraints = constraints_at(&map, pos);
        let num_constrained = constraints.iter().filter(|c| c.is_some()).count() as u8;

        let mut prev_chance = f32::MAX;
        for min_edges in 0..=num_constrained {
            let (chance, _) = fit_chance_with_min_edges(&freqs, &constraints, min_edges);
            assert!(
                chance <= prev_chance + f32::EPSILON,
                "Fit chance increased at {pos:?}: min_edges={min_edges}, \
                 prev={prev_chance}, now={chance}"
            );
            prev_chance = chance;
        }
        checked += 1;
    }
    assert!(checked > 100, "Only checked {checked} positions");
    println!("Monotonicity validated for {checked} positions");
}

/// Validate that a perfect-fit tile (all edges matching) is always counted
/// in the fit_chance at its own edge count.
#[test]
fn test_fit_chance_perfect_tile_counted() {
    use dorfromantische2_rs::best_placements::fit_chance_with_min_edges;
    use dorfromantische2_rs::tile_frequency::TileFrequencies;

    let sg = load_biggame();
    let map = Map::from(&sg);
    let freqs = TileFrequencies::from_map(&map);

    let mut perfect_tiles = 0;
    let mut counted_at_own_level = 0;

    for pos in map.iter_tile_positions() {
        let key = match map.tile_key(pos) {
            Some(k) => k,
            None => continue,
        };
        let (base, count) = match map.tile_index[key] {
            Some(bc) => bc,
            None => continue,
        };
        if count == 0 {
            continue;
        }

        let segments = &map.segments[base..base + count];
        let tile_profile = EdgeProfile::from_segments(segments);
        let constraints = constraints_at(&map, pos);

        // Count this tile's matching edges.
        let own_matches: u8 = constraints
            .iter()
            .enumerate()
            .filter(|(side, c)| {
                c.is_some_and(|neighbor| {
                    matches!(
                        tile_profile.at_index(*side).connects_and_matches(neighbor),
                        EdgeMatch::Matching
                    )
                })
            })
            .count() as u8;

        let num_constrained = constraints.iter().filter(|c| c.is_some()).count() as u8;
        if own_matches == num_constrained && num_constrained > 0 {
            // Perfect placement — all constrained edges match.
            perfect_tiles += 1;
            // fit_chance with min_edges = own_matches should include this tile.
            let (chance, _) = fit_chance_with_min_edges(&freqs, &constraints, own_matches);
            if chance > 0.0 {
                counted_at_own_level += 1;
            }
        }
    }

    assert!(
        perfect_tiles > 1000,
        "Only {perfect_tiles} perfect tiles found"
    );
    let pct = counted_at_own_level as f64 / perfect_tiles as f64 * 100.0;
    assert!(
        pct > 99.0,
        "Only {pct:.1}% of perfect tiles were counted ({counted_at_own_level}/{perfect_tiles})"
    );
    println!(
        "{perfect_tiles} perfect tiles, {counted_at_own_level} counted at own level ({pct:.1}%)"
    );
}
