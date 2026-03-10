use std::{cmp::Ordering, collections::BTreeSet};

use crate::{
    data::{Pos, Rotation, Terrain},
    group::GroupIndex,
    group_assignments::GroupAssignments,
    map::Map,
};

#[derive(Debug, PartialEq, Eq)]
pub struct GroupEdgeAlteration {
    pub group_size: usize,
    pub diff: i8,
}

#[derive(Debug, PartialEq, Eq)]
pub struct PlacementScore {
    pub pos: Pos,
    pub rotation: Rotation,
    pub split: bool,
    pub mismatched_edges: u8,
    pub matching_edges: u8,
    pub group_edge_alterations: Vec<GroupEdgeAlteration>,
}

impl PlacementScore {
    fn to_orderable(&self) -> impl Ord {
        (
            !self.split,
            -i16::from(self.mismatched_edges),
            self.matching_edges,
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
struct SegmentEffects {
    connects_to: Vec<GroupIndex>,
    blocks: Vec<GroupIndex>,
    opens_edges: i32,
}

/// Result of evaluating all six sides of a potential tile placement.
struct SideEvaluation {
    mismatched_edges: u8,
    matching_edges: u8,
    split: bool,
    segment_effects: [SegmentEffects; 6],
    empty_segment_blocks: Vec<GroupIndex>,
}

/// Evaluate one side of a potential placement. Returns `None` if the placement is invalid.
fn evaluate_side(
    map: &Map,
    groups: &GroupAssignments,
    pos: Pos,
    rotation: Rotation,
    side: usize,
    mismatched_edges: &mut u8,
    matching_edges: &mut u8,
    segment_effects: &mut [SegmentEffects; 6],
    empty_segment_blocks: &mut Vec<GroupIndex>,
) -> Option<()> {
    let my_segment = map
        .next_tile
        .iter()
        .enumerate()
        .find(|(_, seg)| seg.contains_rotation((side + 6 - rotation) % 6));

    let neighbor_pos = Map::neighbor_pos_of(pos, side);
    let other_side = (side + 3) % 6;
    let other_tile = map
        .tile_key(neighbor_pos)
        .and_then(|key| map.rendered_tiles[key].map(|segments| segments[other_side]));

    match (my_segment, other_tile) {
        // Place empty next to empty or missing.
        (None, None | Some(None)) => {}
        // Place empty next to some tile with a segment.
        (None, Some(Some(segment_index))) => {
            let group = groups.assigned_groups[segment_index];
            empty_segment_blocks.push(group);
        }
        // Place something next to nothing.
        (Some((i, _)), None) => {
            segment_effects[i].opens_edges += 1;
        }
        // Place something next to empty tile (check if matches).
        (Some((_, my_segment)), Some(None)) => {
            match my_segment.terrain.connects_and_matches(Terrain::Empty) {
                None => return None,
                Some(true) => *matching_edges += 1,
                Some(false) => *mismatched_edges += 1,
            }
        }
        // Place something next to something (check if matches).
        (Some((i, my_segment)), Some(Some(other_index))) => {
            let other_terrain = map.segments[other_index].terrain;
            match my_segment.terrain.connects_and_matches(other_terrain) {
                None => return None,
                Some(true) => {
                    let other_group = groups.assigned_groups[other_index];
                    segment_effects[i].connects_to.push(other_group);
                    *matching_edges += 1;
                }
                Some(false) => {
                    let other_group = groups.assigned_groups[other_index];
                    segment_effects[i].blocks.push(other_group);
                    *mismatched_edges += 1;
                }
            }
        }
    }

    Some(())
}

/// Check whether placing a tile at `pos` would create a split (hole) in the map.
fn detect_split(map: &Map, pos: Pos) -> bool {
    let mut split_groups = Vec::new();
    for side in 0..6 {
        let neighbor_pos = Map::neighbor_pos_of(pos, side);
        let other_side = (side + 3) % 6;
        let other_tile = map
            .tile_key(neighbor_pos)
            .and_then(|key| map.rendered_tiles[key].map(|segments| segments[other_side]));

        let is_free = other_tile.is_none();
        if split_groups.last() != Some(&is_free) {
            split_groups.push(is_free);
        }
    }
    split_groups.len() > 3
}

impl BestPlacements {
    /// Test a single placement option.
    fn score_of_next_at(
        map: &Map,
        groups: &GroupAssignments,
        pos: Pos,
        rotation: Rotation,
    ) -> Option<PlacementScore> {
        let mut mismatched_edges = 0;
        let mut matching_edges = 0;
        let mut empty_segment_blocks = Vec::new();
        let mut segment_effects = <[SegmentEffects; 6]>::default();

        for side in 0..6 {
            evaluate_side(
                map,
                groups,
                pos,
                rotation,
                side,
                &mut mismatched_edges,
                &mut matching_edges,
                &mut segment_effects,
                &mut empty_segment_blocks,
            )?;
        }

        let split = detect_split(map, pos);
        let group_edge_alterations = Vec::new();

        Some(PlacementScore {
            pos,
            rotation,
            split,
            mismatched_edges,
            matching_edges,
            group_edge_alterations,
        })
    }

    pub fn iter_usable(&self) -> impl Iterator<Item = (usize, &PlacementScore)> {
        self.best_placements
            .iter()
            .filter(|score| score.mismatched_edges == 0)
            .rev()
            .take(30)
            .enumerate()
    }
}

impl From<(&Map, &GroupAssignments)> for BestPlacements {
    fn from(data: (&Map, &GroupAssignments)) -> Self {
        let map = data.0;
        let groups = data.1;
        let mut best_placements = BTreeSet::default();

        for pos in &groups.possible_placements {
            for rotation in 0..6 {
                if let Some(score) = BestPlacements::score_of_next_at(map, groups, *pos, rotation) {
                    best_placements.insert(score);
                }
            }
        }

        Self { best_placements }
    }
}
