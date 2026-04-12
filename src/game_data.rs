use std::collections::HashSet;

use crate::{
    best_placements::BestPlacements,
    data::{EdgeMatch, HexPos, Terrain},
    group_assignments::GroupAssignments,
    map::Map,
    tile_frequency::TileFrequencies,
};

#[derive(Default)]
pub struct GameData {
    pub map: Map,
    pub group_assignments: GroupAssignments,
    pub best_placements: BestPlacements,
    pub tile_frequencies: TileFrequencies,
    /// Tiles with at least one non-matching edge. Computed lazily.
    imperfect_tiles: Option<HashSet<HexPos>>,
}

impl GameData {
    /// Get or compute the set of tiles that have at least one non-matching neighbor edge.
    pub fn imperfect_tiles(&mut self) -> &HashSet<HexPos> {
        if self.imperfect_tiles.is_none() {
            self.imperfect_tiles = Some(compute_imperfect_tiles(&self.map));
        }
        self.imperfect_tiles.as_ref().unwrap()
    }

    /// Invalidate cached computations (call after map reload).
    pub fn invalidate_cache(&mut self) {
        self.imperfect_tiles = None;
    }
}

fn compute_imperfect_tiles(map: &Map) -> HashSet<HexPos> {
    let mut result = HashSet::new();
    for pos in map.iter_tile_positions() {
        let key = match map.tile_key(pos) {
            Some(k) => k,
            None => continue,
        };
        let rendered = match map.rendered_tiles[key] {
            Some(r) => r,
            None => continue,
        };

        for (side, rendered_side) in rendered.iter().enumerate() {
            let my_terrain = rendered_side
                .map(|idx| map.segments[idx].terrain)
                .unwrap_or(Terrain::Empty);

            let neighbor_pos = Map::neighbor_pos_of(pos, side);
            let other_side = Map::opposite_side(side);
            let neighbor_terrain = map
                .tile_key(neighbor_pos)
                .and_then(|k| map.rendered_tiles[k])
                .and_then(|r| r[other_side])
                .map(|idx| map.segments[idx].terrain);

            // Only check edges that face an existing neighbor tile.
            if let Some(other) = neighbor_terrain {
                match my_terrain.connects_and_matches(other) {
                    EdgeMatch::Matching | EdgeMatch::Missing => {}
                    EdgeMatch::Suboptimal | EdgeMatch::Illegal => {
                        result.insert(pos);
                        break;
                    }
                }
            }
        }
    }
    result
}
