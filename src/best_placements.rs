use std::{cmp::Ordering, collections::BTreeSet};

use crate::{
    data::{EdgeMatch, Pos, Rotation, Terrain, HEX_SIDES},
    group::GroupIndex,
    group_assignments::GroupAssignments,
    map::Map,
};

pub const MAX_SHOWN_PLACEMENTS: usize = 30;

#[derive(Debug, PartialEq, Eq)]
pub struct GroupEdgeAlteration {
    pub group_size: usize,
    pub diff: i8,
}

#[derive(Debug, PartialEq, Eq)]
pub struct PlacementScore {
    pub pos: Pos,
    pub rotation: Rotation,
    pub matching_edges: u8,
    /// Bonus for station/lake tiles: count of desirable adjacent terrains.
    pub neighbor_bonus: u8,
    /// How constrained the empty spaces are that Rail/River edges point at.
    pub connection_difficulty: u8,
    /// Rail/River edges from existing tiles pointing at empty neighbors we'd crowd.
    pub crowding: u8,
    /// Net change in group open ends: new open ends created minus existing ones closed.
    pub open_end_delta: i8,
    pub group_edge_alterations: Vec<GroupEdgeAlteration>,
}

impl PlacementScore {
    fn to_orderable(&self) -> impl Ord {
        (
            self.matching_edges,
            std::cmp::Reverse(self.connection_difficulty),
            std::cmp::Reverse(self.crowding),
            std::cmp::Reverse(self.open_end_delta),
            self.neighbor_bonus,
            // Use the following to prevent duplicate removal.
            self.pos.x,
            self.pos.y,
            // Don't use rotation here, we DO drop duplicates with the same rotation.
        )
    }
}

impl Ord for PlacementScore {
    fn cmp(&self, other: &Self) -> Ordering {
        self.to_orderable().cmp(&other.to_orderable())
    }
}

impl PartialOrd for PlacementScore {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Default)]
pub struct BestPlacements {
    best_placements: BTreeSet<PlacementScore>,
}

#[derive(Default)]
#[allow(dead_code)]
struct SegmentEffects {
    connects_to: Vec<GroupIndex>,
    blocks: Vec<GroupIndex>,
    opens_edges: i32,
}

// /// Evaluate one side of a potential placement. Returns `None` if the placement is invalid.
// fn evaluate_side(
//     map: &Map,
//     groups: &GroupAssignments,
//     pos: Pos,
//     rotation: Rotation,
//     side: usize,
//     mismatched_edges: &mut u8,
//     matching_edges: &mut u8,
//     segment_effects: &mut [SegmentEffects; 6],
//     empty_segment_blocks: &mut Vec<GroupIndex>,
// ) -> Option<()> {
//     let my_segment = map
//         .next_tile
//         .iter()
//         .enumerate()
//         .find(|(_, seg)| seg.contains_rotation((side + HEX_SIDES - rotation) % HEX_SIDES));
//
//     let neighbor_pos = Map::neighbor_pos_of(pos, side);
//     let other_side = Map::opposite_side(side);
//     let other_tile = map
//         .tile_key(neighbor_pos)
//         .and_then(|key| map.rendered_tiles[key].map(|segments| segments[other_side]));
//
//     match (my_segment, other_tile) {
//         // Place empty next to empty or missing.
//         (None, None | Some(None)) => {}
//         // Place empty next to some tile with a segment.
//         (None, Some(Some(segment_index))) => {
//             let group = groups.assigned_groups[segment_index];
//             empty_segment_blocks.push(group);
//         }
//         // Place something next to nothing.
//         (Some((i, _)), None) => {
//             segment_effects[i].opens_edges += 1;
//         }
//         // Place something next to empty tile (check if matches).
//         (Some((_, my_segment)), Some(None)) => {
//             match my_segment.terrain.connects_and_matches(Terrain::Empty) {
//                 None => return None,
//                 Some(true) => *matching_edges += 1,
//                 Some(false) => *mismatched_edges += 1,
//             }
//         }
//         // Place something next to something (check if matches).
//         (Some((i, my_segment)), Some(Some(other_index))) => {
//             let other_terrain = map.segments[other_index].terrain;
//             match my_segment.terrain.connects_and_matches(other_terrain) {
//                 None => return None,
//                 Some(true) => {
//                     let other_group = groups.assigned_groups[other_index];
//                     segment_effects[i].connects_to.push(other_group);
//                     *matching_edges += 1;
//                 }
//                 Some(false) => {
//                     let other_group = groups.assigned_groups[other_index];
//                     segment_effects[i].blocks.push(other_group);
//                     *mismatched_edges += 1;
//                 }
//             }
//         }
//     }
//
//     Some(())
// }
//
// /// Check whether placing a tile at `pos` would create a split (hole) in the map.
// fn detect_split(map: &Map, pos: Pos) -> bool {
//     let mut split_groups = Vec::new();
//     for side in 0..HEX_SIDES {
//         let neighbor_pos = Map::neighbor_pos_of(pos, side);
//         let other_side = Map::opposite_side(side);
//         let other_tile = map
//             .tile_key(neighbor_pos)
//             .and_then(|key| map.rendered_tiles[key].map(|segments| segments[other_side]));
//
//         let is_free = other_tile.is_none();
//         if split_groups.last() != Some(&is_free) {
//             split_groups.push(is_free);
//         }
//     }
//     split_groups.len() > 3
// }

/// Count matching edges for placing the next tile at `pos` with `rotation`.
/// Returns `None` if any edge is illegal (e.g. rail next to river).
fn count_matching_edges(map: &Map, pos: Pos, rotation: Rotation) -> Option<u8> {
    let mut matching = 0;
    for side in 0..HEX_SIDES {
        let neighbor_pos = Map::neighbor_pos_of(pos, side);
        let other_side = Map::opposite_side(side);
        let neighbor_segments = match map
            .tile_key(neighbor_pos)
            .and_then(|key| map.rendered_tiles[key])
        {
            Some(segments) => segments,
            // No neighbor tile at this side — no constraint.
            None => continue,
        };
        let other_terrain =
            neighbor_segments[other_side].map_or(Terrain::Empty, |idx| map.segments[idx].terrain);

        // Resolve terrains: no segment on a side means Empty.
        let my_terrain = map
            .next_tile
            .iter()
            .find(|seg| seg.contains_rotation((side + HEX_SIDES - rotation) % HEX_SIDES))
            .map_or(Terrain::Empty, |s| s.terrain);

        match my_terrain.connects_and_matches(other_terrain) {
            EdgeMatch::Matching => matching += 1,
            EdgeMatch::Missing => {}
            EdgeMatch::Suboptimal | EdgeMatch::Illegal => return None,
        }
    }
    Some(matching)
}

/// Count how many neighboring edges at `pos` have one of the given terrains.
fn count_neighbor_terrains(map: &Map, pos: Pos, wanted: &[Terrain]) -> u8 {
    let mut count = 0;
    for side in 0..HEX_SIDES {
        let neighbor_pos = Map::neighbor_pos_of(pos, side);
        let other_side = Map::opposite_side(side);
        let terrain = map
            .tile_key(neighbor_pos)
            .and_then(|key| map.rendered_tiles[key])
            .and_then(|segments| segments[other_side])
            .map(|idx| map.segments[idx].terrain);
        if terrain.is_some_and(|t| wanted.contains(&t)) {
            count += 1;
        }
    }
    count
}

/// Sum of occupied neighbors for each empty space a Rail/River edge points at.
/// Higher = harder to connect later. An edge pointing at open space (few neighbors) is fine;
/// an edge pointing at a nearly-surrounded empty hex is bad.
fn connection_difficulty(map: &Map, pos: Pos, rotation: Rotation) -> u8 {
    let mut difficulty = 0;
    for side in 0..HEX_SIDES {
        let neighbor_pos = Map::neighbor_pos_of(pos, side);
        let has_neighbor = map
            .tile_key(neighbor_pos)
            .and_then(|key| map.rendered_tiles[key])
            .is_some();
        if has_neighbor {
            continue;
        }
        let my_terrain = map
            .next_tile
            .iter()
            .find(|seg| seg.contains_rotation((side + HEX_SIDES - rotation) % HEX_SIDES))
            .map(|s| s.terrain);
        if matches!(my_terrain, Some(Terrain::Rail | Terrain::River)) {
            // Count how many of the empty neighbor's sides are already occupied
            // (including the tile we're about to place).
            let occupied = (0..HEX_SIDES)
                .filter(|&s| {
                    let p = Map::neighbor_pos_of(neighbor_pos, s);
                    if p == pos {
                        return true; // the tile we're placing
                    }
                    map.tile_key(p)
                        .and_then(|key| map.rendered_tiles[key])
                        .is_some()
                })
                .count() as u8;
            difficulty += occupied;
        }
    }
    difficulty
}

/// Count Rail/River edges from existing tiles that point into empty neighbors of `pos`.
/// Placing our tile next to such a space constrains it further, making it harder to
/// later fill with a tile that connects those rails/rivers.
fn crowding(map: &Map, pos: Pos) -> u8 {
    let mut total = 0;
    for side in 0..HEX_SIDES {
        let empty_pos = Map::neighbor_pos_of(pos, side);
        // Only consider empty neighbors.
        let is_empty = map
            .tile_key(empty_pos)
            .and_then(|key| map.rendered_tiles[key])
            .is_none();
        if !is_empty {
            continue;
        }
        // Check how many Rail/River edges from other tiles already point at this empty hex.
        for s in 0..HEX_SIDES {
            let p = Map::neighbor_pos_of(empty_pos, s);
            if p == pos {
                continue; // skip the tile we're placing
            }
            let other_side = Map::opposite_side(s);
            let terrain = map
                .tile_key(p)
                .and_then(|key| map.rendered_tiles[key])
                .and_then(|segments| segments[other_side])
                .map(|idx| map.segments[idx].terrain);
            if matches!(terrain, Some(Terrain::Rail | Terrain::River)) {
                total += 1;
            }
        }
    }
    total
}

/// Net change in group open ends from placing the next tile at `pos` with `rotation`.
/// Counts new open ends (tile segments pointing at empty neighbors) minus
/// existing open ends that get closed (neighbor segments pointing at this slot).
fn open_end_delta(map: &Map, pos: Pos, rotation: Rotation) -> i8 {
    let mut created: i8 = 0;
    let mut closed: i8 = 0;
    for side in 0..HEX_SIDES {
        let neighbor_pos = Map::neighbor_pos_of(pos, side);
        let other_side = Map::opposite_side(side);
        let neighbor_tile = map
            .tile_key(neighbor_pos)
            .and_then(|key| map.rendered_tiles[key]);

        let my_terrain = map
            .next_tile
            .iter()
            .find(|seg| seg.contains_rotation((side + HEX_SIDES - rotation) % HEX_SIDES))
            .map(|s| s.terrain);

        match neighbor_tile {
            None => {
                // Empty neighbor: our segment creates a new open end.
                if my_terrain.is_some_and(|t| t != Terrain::Empty) {
                    created += 1;
                }
            }
            Some(segments) => {
                // Occupied neighbor: their segment pointing at us was an open end, now closed.
                let other_terrain = segments[other_side].map(|idx| map.segments[idx].terrain);
                if other_terrain.is_some_and(|t| t != Terrain::Empty) {
                    closed += 1;
                }
            }
        }
    }
    created - closed
}

/// Check whether placing a tile at `pos` would split a contiguous empty region into
/// multiple holes. Walks the 6 neighbors and counts runs of occupied/empty tiles around
/// the hex (wrapping around). More than one run of empty neighbors means the placement
/// creates a split.
fn would_create_split(map: &Map, pos: Pos) -> bool {
    let occupied: [bool; HEX_SIDES] = std::array::from_fn(|side| {
        let neighbor_pos = Map::neighbor_pos_of(pos, side);
        map.tile_key(neighbor_pos)
            .and_then(|key| map.rendered_tiles[key])
            .is_some()
    });

    // Count the number of contiguous runs of empty neighbors, wrapping around.
    let mut empty_runs = 0;
    for side in 0..HEX_SIDES {
        let prev = (side + HEX_SIDES - 1) % HEX_SIDES;
        // Start of a new empty run: current is empty, previous is occupied (or wraps).
        if !occupied[side] && occupied[prev] {
            empty_runs += 1;
        }
    }
    empty_runs > 1
}

impl BestPlacements {
    /// Test a single placement option.
    fn score_of_next_at(
        map: &Map,
        _groups: &GroupAssignments,
        pos: Pos,
        rotation: Rotation,
    ) -> Option<PlacementScore> {
        if would_create_split(map, pos) {
            return None;
        }
        let matching_edges = count_matching_edges(map, pos, rotation)?;

        let has_terrain = |t| map.next_tile.iter().any(|s| s.terrain == t);
        let neighbor_bonus = if has_terrain(Terrain::Station) {
            count_neighbor_terrains(map, pos, &[Terrain::Rail, Terrain::River])
        } else if has_terrain(Terrain::Lake) {
            count_neighbor_terrains(map, pos, &[Terrain::River])
        } else {
            0
        };
        let connection_difficulty = connection_difficulty(map, pos, rotation);
        let crowding = crowding(map, pos);
        let open_end_delta = open_end_delta(map, pos, rotation);

        Some(PlacementScore {
            pos,
            rotation,
            matching_edges,
            connection_difficulty,
            crowding,
            open_end_delta,
            neighbor_bonus,
            group_edge_alterations: Vec::new(),
        })
    }

    pub fn iter_usable(&self) -> impl Iterator<Item = (usize, &PlacementScore)> {
        let max_edges = self
            .best_placements
            .iter()
            .next_back()
            .map_or(0, |s| s.matching_edges);
        self.best_placements
            .iter()
            .rev()
            .take_while(move |s| s.matching_edges == max_edges)
            .take(MAX_SHOWN_PLACEMENTS)
            .enumerate()
    }

    pub fn iter_all(&self) -> impl Iterator<Item = (usize, &PlacementScore)> {
        self.best_placements
            .iter()
            .rev()
            .take(MAX_SHOWN_PLACEMENTS)
            .enumerate()
    }
}

impl From<(&Map, &GroupAssignments)> for BestPlacements {
    fn from(data: (&Map, &GroupAssignments)) -> Self {
        let map = data.0;
        let groups = data.1;
        let mut best_placements = BTreeSet::default();

        for pos in &groups.possible_placements {
            let best_rotation = (0..HEX_SIDES)
                .filter_map(|rotation| {
                    BestPlacements::score_of_next_at(map, groups, *pos, rotation)
                })
                .max();
            if let Some(score) = best_rotation {
                best_placements.insert(score);
            }
        }

        Self { best_placements }
    }
}
