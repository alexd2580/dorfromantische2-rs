use glam::IVec2;
use std::{
    collections::{HashMap, HashSet, VecDeque},
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

    /// Groups of segments on tiles.
    assigned_groups: HashMap<(TileId, SegmentId), GroupId>,
    /// List of groups - which segments belong to them and what open edges they have.
    groups: Vec<Group>,

    /// Probability/count of getting a tile. Tiles are canonicalized by converting the terrain enum
    /// into int and choosing the lexicographically smallest rotation.
    probabilities: HashMap<[Terrain; 6], (usize, f32)>,
    /// Set of valid positions to place a tile.
    possible_placements: HashSet<IVec2>,

    /// Next tile in the tile stack.
    next_tile: Option<Tile>,
    /// Placement choices, grouped by score.
    best_placements: Vec<(i32, Vec<IVec2>)>,
}

impl Default for Map {
    fn default() -> Self {
        let tiles = vec![Tile {
            pos: Default::default(),
            segments: vec![],
            parts: [Terrain::Empty; 6],
            quest_tile: None,
        }];
        let index = Index::from(&tiles);

        Self {
            tiles,
            index,
            assigned_groups: Default::default(),
            groups: Default::default(),
            probabilities: Default::default(),
            possible_placements: Default::default(),
            next_tile: Default::default(),
            best_placements: Default::default(),
        }
    }
}

impl From<&raw_data::SaveGame> for Map {
    fn from(savegame: &raw_data::SaveGame) -> Self {
        let mut quest_tile_ids = HashSet::<i32>::default();
        let mut quest_ids = HashSet::<i32>::default();

        //     dbg!(&savegame.preplaced_tiles);
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
        let tiles = iter::once(Default::default())
            .chain(savegame.tiles.iter().map(Tile::from))
            .collect::<Vec<_>>();
        let index = Index::from(&tiles);

        let (assigned_groups, groups, possible_placements) = Self::assign_groups(&tiles, &index);
        let next_tile = Some(Tile::from(&savegame.tile_stack[0]));

        let mut map = Self {
            tiles,
            index,
            assigned_groups,
            groups,
            probabilities: Default::default(),
            possible_placements,
            next_tile,
            best_placements: Default::default(),
        };

        let next_tile = map.next_tile.as_ref().unwrap();
        let mut best_placements = HashMap::<i32, Vec<IVec2>>::default();
        for pos in &map.possible_placements {
            let score = (0..6)
                .map(|rotation| map.score_of(&next_tile.moved_to(*pos, rotation)))
                .max()
                .unwrap();

            let mut previous = best_placements.remove(&score).unwrap_or(vec![]);
            previous.push(*pos);
            best_placements.insert(score, previous);
        }
        let mut best_placements = best_placements.into_iter().collect::<Vec<_>>();
        best_placements.sort_by_key(|elem| std::cmp::Reverse(elem.0));

        map.best_placements = best_placements;

        map
    }
}

const TILE_: usize = IVEC4_ + IVEC2_ + IVEC2_;

impl Map {
    // pub fn tiles(&self) -> &[Tile] {
    //     &self.tiles
    // }

    pub fn best_placements(&self) -> &Vec<(i32, Vec<IVec2>)> {
        &self.best_placements
    }

    pub fn tile_index(&self, pos: IVec2) -> Option<TileId> {
        self.index.tile_index(pos)
    }

    pub fn tile(&self, tile_id: TileId) -> &Tile {
        &self.tiles[tile_id]
    }

    pub fn group_of(&self, tile_id: TileId, segment_id: SegmentId) -> GroupId {
        self.assigned_groups[&(tile_id, segment_id)]
    }

    pub fn group(&self, group_id: GroupId) -> &Group {
        &self.groups[group_id]
    }

    pub fn next_tile(&self) -> &Option<Tile> {
        &self.next_tile
    }

    fn assign_groups(
        tiles: &[Tile],
        index: &Index,
    ) -> (
        HashMap<(TileId, SegmentId), GroupId>,
        Vec<Group>,
        HashSet<IVec2>,
    ) {
        let mut assigned_groups = HashMap::<(TileId, SegmentId), GroupId>::default();
        let mut groups = Vec::<HashSet<(TileId, SegmentId)>>::default();
        let mut possible_placements = HashSet::<IVec2>::default();

        let mut processed = HashSet::<TileId>::default();
        let mut queue = VecDeque::from([0]);

        // Process all tiles, breadth first.
        while !queue.is_empty() {
            let tile_id = queue.pop_front().unwrap();
            let tile = &tiles[tile_id];

            // Check if an index was processed and enqueue neighbor otherwise.
            for rotation in 0..6 {
                let neighbor_pos = tile.neighbor_coordinates(rotation);
                if let Some(neighbor_id) = index.tile_index(neighbor_pos) {
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
                    .flat_map(|rotation| {
                        let neighbor_pos = tile.neighbor_coordinates(rotation);
                        let opposite_side = Tile::opposite_side(rotation);

                        // Get neighbor tile at `rotation`.
                        index
                            .tile_index(neighbor_pos)
                            // Get its segment which is at the opposite side of `rotation`.
                            // Require that the terrain is the same.
                            .and_then(|neighbor_id| {
                                tiles[neighbor_id]
                                    .connecting_segment_at(segment.terrain, opposite_side)
                                    // Get the group id.
                                    .and_then(|(segment_id, _)| {
                                        assigned_groups.get(&(neighbor_id, segment_id))
                                    })
                                    .cloned()
                            })
                    })
                    .collect::<HashSet<_>>();

                // Choose the new group id from the collected ids.
                let group_id = if group_ids.is_empty() {
                    groups.push(Default::default());
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
                for other_id in group_ids.into_iter() {
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
                open_edges: Default::default(),
            })
            .collect::<Vec<_>>();

        possible_placements
            .iter()
            // Get all pairs of position/rotation that border this open tile.
            .flat_map(|pos| {
                (0..6).map(|rotation| {
                    (
                        Tile::neighbor_coordinates_of(*pos, rotation),
                        Tile::opposite_side(rotation),
                    )
                })
            })
            // Filter all tiles at these positions that actually exist.
            .filter_map(|(position, rotation)| {
                index
                    .tile_index(position)
                    .map(|tile_id| (tile_id, rotation))
            })
            // For all these, add this rotation as an open edge.
            .for_each(|(tile_id, rotation)| {
                let tile = &tiles[tile_id];
                tile.segments_at(rotation).for_each(|(segment_id, _)| {
                    let group_of_segment = assigned_groups[&(tile_id, segment_id)];
                    groups[group_of_segment]
                        .open_edges
                        .insert((tile_id, rotation));
                })
            });

        (assigned_groups, groups, possible_placements)
    }

    /// Compute the quality of the placement of `tile`. `tile` can be both places and new tiles.
    /// Rotation is ignored when computing quality. The highest value is chosen.
    pub fn score_of(&self, tile: &Tile) -> i32 {
        (0..6)
            .map(|side| {
                // Get the neighbor at that side.
                self.index
                    .tile_index(Tile::neighbor_coordinates_of(tile.pos, side))
                    .map(|neighbor| tile.placement_score(side, &self.tiles[neighbor]))
                    .unwrap_or(0)
            })
            .sum()
    }

    pub fn byte_size(&self) -> usize {
        let num_tiles = self.index.index_data().len();
        #[allow(unused_parens)]
        (
            // Offset
            1 * IVEC2_
            // Size
            + 1 * IVEC2_
            // Tiles (at least one...)
            + num_tiles.max(1) * TILE_
        )
    }

    pub unsafe fn write_to(&self, ptr: *mut u8) {
        let iptr = ptr.cast::<i32>();

        let (offset, size) = self.index.offset_and_size();
        *iptr.add(0) = offset.x;
        *iptr.add(1) = offset.y;
        *iptr.add(2) = size.x;
        *iptr.add(3) = size.y;

        let bptr = iptr.add(4).cast::<u8>();
        for (index, maybe_tile_id) in self.index.index_data().iter().enumerate() {
            let tptr = bptr.add(index * TILE_).cast::<u32>();

            if let &Some(tile_id) = maybe_tile_id {
                // Tile exists.
                let segments = &self.tile(tile_id).segments;
                for (segment_id, segment) in segments.iter().enumerate() {
                    let group = self.group_of(tile_id, segment_id);
                    let is_closed = self.groups[group].open_edges.is_empty() as u32;

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

        for (score, positions) in &self.best_placements {
            for pos in positions {
                let index = self.index.tile_key(*pos).unwrap();
                let tptr = bptr.add(index * TILE_).cast::<i32>();
                *tptr.add(6) = *score;
            }
        }
    }
}
