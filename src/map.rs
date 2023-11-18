use glam::IVec2;
use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    iter,
};

use crate::{
    data::{Rotation, SegmentId, Terrain, Tile, TileId, IVEC2_, IVEC4_},
    index::Index,
    raw_data,
};

pub type GroupId = usize;

pub struct Group {
    // Tile/segment.
    pub segments: HashSet<(TileId, SegmentId)>,

    // Tile/rotation.
    pub open_edges: HashSet<(TileId, Rotation)>,
}

pub struct Map {
    /// List of tiles in placement order, as read from the savegame.
    tiles: Vec<Tile>,
    /// Index structure.
    index: Index,
    /// Probability/count of getting a tile. Tiles are canonicalized by converting the terrain enum
    /// into int and choosing the lexicographically smallest rotation.
    probabilities: HashMap<u32, ([Terrain; 6], usize, f32)>,

    /// Groups of segments on tiles.
    assigned_groups: HashMap<(TileId, SegmentId), GroupId>,
    /// List of groups - which segments belong to them and what open edges they have.
    groups: Vec<Group>,
    /// Set of valid positions to place a tile.
    possible_placements: HashSet<IVec2>,

    /// Next tile in the tile stack.
    next_tile: Option<Tile>,
    /// Placement choices, grouped by score.
    best_placements: BTreeMap<i32, BTreeMap<isize, (IVec2, Rotation)>>,
}

impl Default for Map {
    fn default() -> Self {
        let tiles = vec![Tile {
            pos: IVec2::ZERO,
            segments: vec![],
            parts: [Terrain::Empty; 6],
            quest_tile: None,
        }];
        let index = Index::from(&tiles);

        Self {
            tiles,
            index,
            probabilities: HashMap::default(),
            assigned_groups: HashMap::default(),
            groups: Vec::default(),
            possible_placements: HashSet::default(),
            next_tile: Option::default(),
            best_placements: BTreeMap::default(),
        }
    }
}

impl From<&raw_data::SaveGame> for Map {
    fn from(savegame: &raw_data::SaveGame) -> Self {
        let mut quest_tile_ids = HashSet::<i32>::default();
        let mut quest_ids = HashSet::<i32>::default();

        // TODO convert sectionGridPos into gridPos and then into axial coordinates.
        // dbg!(&savegame.preplaced_tiles);
        // int num = Mathf.RoundToInt(worldPos.x / (_tileSize.x * 0.75f));
        // int y = Mathf.RoundToInt((worldPos.z + (float)Mathf.Abs(num % 2) * _tileSize.y / 2f) / _tileSize.y);
        // return new Vector2Int(num, y);

        savegame
            .tiles
            .iter()
            .filter(|tile| tile.quest_tile.is_some())
            .for_each(|tile| {
                let q = tile.quest_tile.as_ref().unwrap();
                quest_ids.insert(q.quest_id.0);
                quest_tile_ids.insert(q.quest_tile_id.0);
            });

        // Prepend tiles list with empty tile (is this necessary when i start parsing special tiles?)
        let tiles = iter::once(Tile::default())
            .chain(savegame.tiles.iter().map(Tile::from))
            .collect::<Vec<_>>();
        let index = Index::from(&tiles);

        let mut probabilities = HashMap::default();
        let num_tiles = tiles.len() as f32;
        for tile in &tiles {
            let canonical_id = tile.canonical_id();
            if !probabilities.contains_key(&canonical_id) {
                probabilities.insert(canonical_id, (tile.parts.clone(), 0, 0.0));
            }
            let entry = probabilities.get_mut(&canonical_id).unwrap();
            entry.1 += 1;
            entry.2 = entry.1 as f32 / num_tiles;
        }

        // let mut probabilities_as_vec = probabilities.values().collect::<Vec<_>>();
        // probabilities_as_vec.sort_by_key(|entry| usize::MAX - entry.1);
        // dbg!(&probabilities_as_vec);

        let next_tile = Some(Tile::from(&savegame.tile_stack[0]));

        let mut map = Self {
            tiles,
            index,
            probabilities,
            assigned_groups: HashMap::default(),
            groups: Vec::default(),
            possible_placements: HashSet::default(),
            next_tile,
            best_placements: BTreeMap::default(),
        };

        map.assign_groups();
        map.evaluate_best_placements();

        map
    }
}

const TILE_: usize = IVEC4_ + IVEC2_ + IVEC2_;

impl Map {
    pub fn tile_id_at(&self, pos: IVec2) -> Option<TileId> {
        self.index.tile_index(pos)
    }

    pub fn tile(&self, id: TileId) -> Option<&Tile> {
        self.tiles.get(id)
    }

    pub fn tile_at(&self, pos: IVec2) -> Option<&Tile> {
        self.index.tile_index(pos).map(|id| &self.tiles[id])
    }

    pub fn tile_and_id_at(&self, pos: IVec2) -> Option<(TileId, &Tile)> {
        self.index.tile_index(pos).map(|id| (id, &self.tiles[id]))
    }

    pub fn neighbor_id_of(&self, pos: IVec2, rotation: Rotation) -> Option<TileId> {
        self.tile_id_at(Tile::neighbor_pos_of(pos, rotation))
    }

    pub fn neighbor_of(&self, pos: IVec2, rotation: Rotation) -> Option<&Tile> {
        self.tile_at(Tile::neighbor_pos_of(pos, rotation))
    }

    pub fn neighbor_and_id_of(&self, pos: IVec2, rotation: Rotation) -> Option<(TileId, &Tile)> {
        self.neighbor_id_of(pos, rotation)
            .map(|id| (id, &self.tiles[id]))
    }

    pub fn terrain_at(&self, pos: IVec2, rotation: Rotation) -> Terrain {
        self.tile_at(pos)
            .map_or(Terrain::Missing, |tile| tile.parts[rotation])
    }

    pub fn terrain_of_neighbor_at(&self, pos: IVec2, rotation: Rotation) -> Terrain {
        self.terrain_at(
            Tile::neighbor_pos_of(pos, rotation),
            Tile::opposite_side(rotation),
        )
    }

    pub fn group_of(&self, tile_id: TileId, segment_id: SegmentId) -> GroupId {
        self.assigned_groups[&(tile_id, segment_id)]
    }

    pub fn group(&self, group_id: GroupId) -> &Group {
        &self.groups[group_id]
    }

    pub fn next_tile(&self) -> Option<&Tile> {
        self.next_tile.as_ref()
    }

    pub fn best_placements(&self) -> &BTreeMap<i32, BTreeMap<isize, (IVec2, Rotation)>> {
        &self.best_placements
    }

    fn assign_groups(&mut self) {
        let mut assigned_groups = HashMap::<(TileId, SegmentId), GroupId>::default();
        let mut groups = Vec::<HashSet<(TileId, SegmentId)>>::default();
        let mut possible_placements = HashSet::<IVec2>::default();

        let mut processed = HashSet::<TileId>::default();
        let mut queue = VecDeque::from([0]);

        // Process all tiles, breadth first.
        while !queue.is_empty() {
            let tile_id = queue.pop_front().unwrap();
            let tile = &self.tiles[tile_id];

            // Check if an index was processed and enqueue neighbor otherwise.
            for rotation in 0..6 {
                let neighbor_pos = tile.neighbor_pos(rotation);
                if let Some(neighbor_id) = self.tile_id_at(neighbor_pos) {
                    if !processed.contains(&neighbor_id) {
                        processed.insert(neighbor_id);
                        queue.push_back(neighbor_id);
                    }
                } else {
                    possible_placements.insert(neighbor_pos);
                }
            }

            // For each segment, aka each separate part of a tile...
            for (segment_id, segment) in tile.segments.iter().enumerate() {
                if assigned_groups.contains_key(&(tile_id, segment_id)) {
                    continue;
                }

                // Collect connected neighbor group ids.
                let mut group_ids = segment
                    .rotations()
                    .into_iter()
                    .filter_map(|rotation| {
                        // Get its segment which is at the opposite side of `rotation`.
                        // Require that the terrain is the same.
                        let (neighbor_id, neighbor) =
                            self.neighbor_and_id_of(tile.pos, rotation)?;
                        let (segment_id, _) = neighbor.connecting_segment_at(
                            segment.terrain,
                            Tile::opposite_side(rotation),
                        )?;
                        assigned_groups.get(&(neighbor_id, segment_id)).copied()
                    })
                    .collect::<HashSet<_>>();

                // Choose the new group id from the collected ids.
                let group_id = if group_ids.is_empty() {
                    groups.push(HashSet::default());
                    groups.len() - 1
                } else if group_ids.len() == 1 {
                    group_ids.drain().next().unwrap()
                } else {
                    let min_id = group_ids
                        .iter()
                        .fold(GroupId::max_value(), |a, b| a.min(*b));
                    group_ids.remove(&min_id);
                    min_id
                };

                // Assign the group to the current segment.
                let _ = assigned_groups.insert((tile_id, segment_id), group_id);
                // Register the current segment with `group_id`.
                let mut group = std::mem::take(&mut groups[group_id]);
                group.insert((tile_id, segment_id));
                // Remap all connected groups to the chosen one (TODO Expensive!).
                for other_id in group_ids {
                    group.extend(groups[other_id].drain().inspect(|(tile_id, segment_id)| {
                        assigned_groups.insert((*tile_id, *segment_id), group_id);
                    }));
                }
                groups[group_id] = group;
            }
        }

        let mut groups = groups
            .into_iter()
            .map(|segments| Group {
                segments,
                open_edges: HashSet::default(),
            })
            .collect::<Vec<_>>();

        possible_placements
            .iter()
            // Get all pairs of position/rotation that border this open tile.
            .flat_map(|pos| {
                (0..6).map(|rotation| {
                    (
                        Tile::neighbor_pos_of(*pos, rotation),
                        Tile::opposite_side(rotation),
                    )
                })
            })
            // Filter all tiles at these positions that actually exist.
            .filter_map(|(position, rotation)| {
                let (id, tile) = self.tile_and_id_at(position)?;
                Some((id, tile, rotation))
            })
            // For all these, add this rotation as an open edge.
            .for_each(|(tile_id, tile, rotation)| {
                tile.segments_at(rotation).for_each(|(segment_id, _)| {
                    let group_of_segment = assigned_groups[&(tile_id, segment_id)];
                    groups[group_of_segment]
                        .open_edges
                        .insert((tile_id, rotation));
                });
            });

        self.assigned_groups = assigned_groups;
        self.groups = groups;
        self.possible_placements = possible_placements;
    }

    pub fn evaluate_best_placements(&mut self) {
        let next_tile = self.next_tile.as_ref().unwrap();
        let mut best_placements = BTreeMap::<i32, BTreeMap<isize, (IVec2, Rotation)>>::default();
        for pos in &self.possible_placements {
            for rotation in 0..6 {
                let placement = next_tile.moved_to(*pos, rotation);
                let (matching_edge_score, probability_score) = self.score_of(&placement);
                if matching_edge_score > 0 {
                    let mut previous = best_placements
                        .remove(&matching_edge_score)
                        .unwrap_or_default();
                    previous.insert(probability_score, (*pos, rotation));
                    best_placements.insert(matching_edge_score, previous);
                }
            }
        }
        self.best_placements = best_placements;
    }

    /// Chance returned as number of previously used tiles that would have matched.
    pub fn chance_of_finding_tile_for(&self, outer_edges: &[Terrain; 6]) -> usize {
        let mut total_matching_count = 0;
        for (inner_edges, tile_count, _) in self.probabilities.values() {
            let matches = Tile::is_perfect_placement(inner_edges, outer_edges);
            if matches {
                total_matching_count += tile_count;
            }
        }
        total_matching_count
    }

    /// Compute the quality of the placement of `tile`. `tile` can be both places and new tiles.
    /// This method does NOT ignore the rotation of `tile`.
    /// Returns a tuple of the matching edge score and a delta of how many previously seen tiles
    /// would be placeable after this move.
    pub fn score_of(&self, tile: &Tile) -> (i32, isize) {
        let matching_edge_score = (0..6)
            .map(|side| {
                let my_terrain = tile.parts[side];
                let other_terrain = self.terrain_of_neighbor_at(tile.pos, side);
                Terrain::neighbor_score(my_terrain, other_terrain)
            })
            .sum();

        if matching_edge_score <= 0 {
            return (matching_edge_score, 0);
        }

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
        let probability_score = self.index.tile_key(tile.pos).unwrap() as isize;
        (matching_edge_score, probability_score)
    }

    /// Collect the edge requirements for a tile at `pos`.
    pub fn outer_edges(&self, pos: IVec2) -> [Terrain; 6] {
        [0, 1, 2, 3, 4, 5].map(|side| self.terrain_of_neighbor_at(pos, side))
    }

    pub fn offset_and_size(&self) -> (IVec2, IVec2) {
        self.index.offset_and_size()
    }

    pub fn byte_size(&self) -> usize {
        let num_tiles = self.index.index_data().len();
        #[allow(unused_parens, clippy::identity_op)]
        (
            // Offset
            1 * IVEC2_
            // Size
            + 1 * IVEC2_
            // Tiles (at least one...)
            + num_tiles.max(1) * TILE_
        )
    }

    #[allow(clippy::similar_names, clippy::cast_ptr_alignment)]
    pub unsafe fn write_to(&self, ptr: *mut u8) {
        let iptr = ptr.cast::<i32>();

        let (offset, size) = self.offset_and_size();
        *iptr.add(0) = offset.x;
        *iptr.add(1) = offset.y;
        *iptr.add(2) = size.x;
        *iptr.add(3) = size.y;

        let bptr = iptr.add(4).cast::<u8>();
        for (index, maybe_tile_id) in self.index.index_data().iter().enumerate() {
            let tptr = bptr.add(index * TILE_).cast::<u32>();

            if let &Some(tile_id) = maybe_tile_id {
                // Tile exists.
                let segments = &self.tiles[tile_id].segments;
                for (segment_id, segment) in segments.iter().enumerate() {
                    let group = self.group_of(tile_id, segment_id);
                    let is_closed = u32::from(self.groups[group].open_edges.is_empty());

                    // Each segment is a uint32.
                    *tptr.add(segment_id) = segment.terrain as u32
                        | (segment.form as u32) << 4
                        | (segment.rotation as u32) << 9
                        | is_closed << 12
                        | (group as u32) << 13;
                }
                if segments.len() < 6 {
                    *tptr.add(segments.len()) = Terrain::Empty as u32;
                }
            } else {
                // Tile doesn't exist.
                *tptr = Terrain::Missing as u32;
                *tptr.add(6) = 0;
            }
        }

        for (matching_edge_score, probability_scores) in &self.best_placements {
            if matching_edge_score <= &0 {
                continue;
            }
            for (probability_score, (pos, rotation)) in probability_scores {
                let index = self.index.tile_key(*pos).unwrap();
                let tptr = bptr.add(index * TILE_).cast::<i32>();
                *tptr.add(6) = *matching_edge_score;
                *tptr.add(7).cast::<f32>() = *probability_score as f32 / self.tiles.len() as f32;
            }
        }
    }
}
