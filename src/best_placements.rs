use std::{cmp::Ordering, collections::BTreeSet};

use crate::{
    data::{Pos, Rotation, Terrain},
    group_assignments::GroupAssignments,
    map::Map,
};

#[derive(Debug, PartialEq, Eq)]
pub struct GroupEdgeAlteration {
    pub group_size: usize,
    open_edges: usize,
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
    // pub matched_complex_edges: u8,
}

impl PlacementScore {
    fn to_orderable(&self) -> impl Ord {
        (
            !self.split,
            -(self.mismatched_edges as i8),
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

pub struct BestPlacements {
    best_placements: BTreeSet<PlacementScore>,
}

impl Default for BestPlacements {
    fn default() -> Self {
        Self {
            best_placements: Default::default(),
        }
    }
}

impl BestPlacements {
    fn score_of_next_at(
        map: &Map,
        groups: &GroupAssignments,
        pos: Pos,
        rotation: Rotation,
    ) -> Option<PlacementScore> {
        let mut split_groups = Vec::new();
        let mut mismatched_edges = 0;
        let mut matching_edges = 0;
        let group_edge_alterations = Vec::new();

        for side in 0..6 {
            let my_terrain = map.rendered_next_tile[(side + rotation) % 6];
            let neighbor_pos = Map::neighbor_pos_of(pos, side);
            let other_side = (side + 3) % 6;
            let other_terrain = map
                .tile_key(neighbor_pos)
                .and_then(|key| map.rendered_tiles[key])
                .map(|neighbor| neighbor[other_side])
                .unwrap_or(Terrain::Missing);

            let is_free = other_terrain == Terrain::Missing;
            match my_terrain.connects_and_matches(other_terrain) {
                None => {
                    return None;
                }
                Some(true) => {
                    if !is_free {
                        matching_edges += 1;
                    }
                }
                Some(false) => {
                    mismatched_edges += 1;
                }
            }

            // Prevent splitting/holes.
            if split_groups.is_empty() {
                split_groups.push(is_free);
            } else if *split_groups.last().unwrap() == is_free {
                continue;
            } else {
                split_groups.push(is_free);
            }
        }

        let split = split_groups.len() > 3;

        // What was the probability of finding any tile in the vicinity of `tile.pos` before
        // actually placing `tile`?
        // let mut old_chance = self.chance_of_finding_tile_for(&self.outer_edges(tile.pos));
        // for side in 0..6 {
        //     let neighbor_pos = tile.neighbor_pos(side);
        //     if self.tile_at(neighbor_pos).is_some() {
        //         continue;
        //     }
        //
        //     let edges = self.outer_edges(neighbor_pos);
        //     // Any tile matches an empty space with no restrictions.
        //     // Preemptively check this case to avoid iterating through `probabilities`.
        //     if edges.iter().all(|edge| *edge == Terrain::Missing) {
        //         old_chance += self.tiles.len();
        //     } else {
        //         old_chance += self.chance_of_finding_tile_for(&edges);
        //     }
        // }
        //
        // let mut new_chance = 0;
        // for side in 0..6 {
        //     let neighbor_pos = tile.neighbor_pos(side);
        //     if self.tile_at(neighbor_pos).is_some() {
        //         continue;
        //     }
        //
        //     // There can no longer be the case that we get a space with no restrictions because we
        //     // only check the neighbors of `tile` as if it were placed.
        //     let mut edges = self.outer_edges(neighbor_pos);
        //     edges[Tile::opposite_side(side)] = tile.parts[side];
        //
        //     new_chance += self.chance_of_finding_tile_for(&edges);
        // }
        //
        // let probability_score = -(old_chance as isize) + new_chance as isize;
        // let probability_score = self.index.tile_key(tile.pos).unwrap() as isize;
        // (matching_edge_score, probability_score)

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
