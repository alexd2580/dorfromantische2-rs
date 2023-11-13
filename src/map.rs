use glam::IVec2;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    iter,
};

use crate::{
    data::{Rotation, SegmentId, Terrain, Tile, TileId, TILE_},
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
    tiles: Vec<Tile>,
    index: Index,

    assigned_groups: HashMap<(TileId, SegmentId), GroupId>,
    groups: Vec<Group>,

    possible_placements: HashSet<IVec2>,

    next_tile: Option<Tile>,
    best_placements: Vec<(IVec2, i32)>,
}

impl Default for Map {
    fn default() -> Self {
        let tiles = vec![Tile {
            pos: Default::default(),
            segments: vec![],
            parts: [Terrain::Empty; 6],
        }];
        let index = Index::from(&tiles);

        Self {
            tiles,
            index,
            assigned_groups: Default::default(),
            groups: Default::default(),
            possible_placements: Default::default(),
            next_tile: Default::default(),
            best_placements: Default::default(),
        }
    }
}

impl From<&raw_data::SaveGame> for Map {
    fn from(savegame: &raw_data::SaveGame) -> Self {
        // let mut quest_tile_ids = HashSet::<i32>::default();
        // let mut quest_ids = HashSet::<i32>::default();
        //
        // savegame.tiles.iter().filter(|tile| tile.quest_tile.is_some()).for_each(|tile| {
        //     let q = tile.quest_tile.as_ref().unwrap();
        //     quest_ids.insert(q.quest_id.0);
        //     quest_tile_ids.insert(q.quest_tile_id.0);
        // });
        //
        // dbg!(&quest_tile_ids);
        // dbg!(&quest_ids);

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
            possible_placements,
            next_tile,
            best_placements: Default::default(),
        };

        let next_tile = map.next_tile.as_ref().unwrap();
        let mut best_placements = map
            .possible_placements
            .iter()
            .map(|pos| {
                let score = (0..6)
                    .map(|rotation| map.score_of(&next_tile.moved_to(*pos, rotation)))
                    .max()
                    .unwrap();
                (*pos, score)
            })
            .collect::<Vec<_>>();

        best_placements.sort_by_key(|elem| std::cmp::Reverse(elem.1));
        map.best_placements = best_placements;

        map
    }
}

impl Map {
    pub fn tiles(&self) -> &[Tile] {
        &self.tiles
    }

    pub fn best_placements(&self) -> &Vec<(IVec2, i32)> {
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
                                    .map(|(segment_id, _)| {
                                        assigned_groups[&(neighbor_id, segment_id)]
                                    })
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

    #[allow(clippy::identity_op)]
    pub unsafe fn write_to(&self, ptr: *mut u8) {
        let iptr = ptr.cast::<i32>();

        let (offset, size) = self.index.offset_and_size();
        *iptr.add(0) = offset.x;
        *iptr.add(1) = offset.y;
        *iptr.add(2) = size.x;
        *iptr.add(3) = size.y;

        let bptr = iptr.add(4).cast::<u8>();
        for (index, maybe_tile_id) in self.index.index_data().iter().enumerate() {
            let tptr = bptr.add(index * TILE_).cast::<i32>();

            if let &Some(tile_id) = maybe_tile_id {
                let segments = &self.tile(tile_id).segments;
                for (segment_id, segment) in segments.iter().enumerate() {
                    *tptr.add(segment_id * 4 + 0) = segment.terrain as i32;
                    *tptr.add(segment_id * 4 + 1) = segment.form as i32;
                    *tptr.add(segment_id * 4 + 2) = segment.rotation as i32; // TODO better primitive types.

                    let group = self.group_of(tile_id, segment_id);
                    let is_closed = self.groups[group].open_edges.is_empty();
                    let group_bytes = group as i32 | if is_closed { 2 << 30 } else { 0 };

                    *tptr.add(segment_id * 4 + 3) = group_bytes;
                }
                if segments.len() < 6 {
                    *tptr.add(segments.len() * 4 + 0) = Terrain::Empty as i32;
                }
            } else {
                *tptr = Terrain::Missing as i32;
            }
        }
    }
}
