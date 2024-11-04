use std::ops::Range;

use crate::{
    data::{
        segments_from_quest_tile, segments_from_special_tile_id, Pos, Rotation, Segment, Terrain,
    },
    raw_data,
};
use glam::IVec2;

pub type TileKey = usize;

pub type SegmentIndex = usize;
pub type SegmentCount = usize;

pub struct Map {
    /// Defines the smallest possible coordinate in the index.
    pub index_offset: Pos,
    /// Defines the extents of the index structure.
    pub index_size: IVec2,
    /// Maps a tile position key to a set of segment indices.
    pub tile_index: Vec<Option<(SegmentIndex, SegmentCount)>>,
    pub rendered_tiles: Vec<Option<[Terrain; 6]>>,
    /// Maps a segment id (index in this array) to a segment.
    pub segments: Vec<Segment>,

    /// The next tile is also represented by a set of segments.
    pub next_tile: Vec<Segment>,
    pub rendered_next_tile: [Terrain; 6],
}

impl Default for Map {
    fn default() -> Self {
        Self {
            index_offset: Default::default(),
            index_size: Default::default(),
            tile_index: Default::default(),
            rendered_tiles: Default::default(),
            segments: Default::default(),
            next_tile: Default::default(),
            rendered_next_tile: [Terrain::Missing; 6],
        }
    }
}

/// Functions for initialization of map.
impl Map {
    /// Compute the position of tile at `pos` in the index structure.
    fn tile_key_function(offset: IVec2, size: IVec2, pos: IVec2) -> Option<TileKey> {
        let upper = offset + size;
        let valid_s = pos.x >= offset.x && pos.x < upper.x;
        let valid_t = pos.y >= offset.y && pos.y < upper.y;
        (valid_s && valid_t)
            .then(|| usize::try_from((pos.y - offset.y) * size.x + (pos.x - offset.x)).unwrap())
    }

    fn load_tile(raw_tile: &raw_data::Tile) -> (Pos, Vec<Segment>) {
        // Hex grid tutorial:
        // https://www.redblobgames.com/grids/hexagons/#line-drawing
        let pos = IVec2::new(raw_tile.s, raw_tile.t - ((raw_tile.s + 1) & -2i32) / 2);

        // We store the rotation on the segments, not on the tiles.
        let tile_rotation = raw_tile.rotation.try_into().unwrap();
        let rotate = |segment: &mut Segment| {
            segment.rotation = (segment.rotation + tile_rotation) % 6;
        };

        // The game stores segments in different ways, either via predefined quest tiles, some
        // special tiles (currently only the station), or simply as a raw list.
        let segments = if let Some(quest_tile) = raw_tile.quest_tile.as_ref() {
            let mut segments = segments_from_quest_tile(pos, quest_tile);
            segments.iter_mut().for_each(rotate);
            segments
        } else if raw_tile.special_tile_id.0 != 0 {
            let mut segments = segments_from_special_tile_id(pos, &raw_tile.special_tile_id);
            segments.iter_mut().for_each(rotate);
            segments
        } else {
            raw_tile
                .segments
                .iter()
                .map(|raw_segment| Segment::from((raw_segment, pos, tile_rotation)))
                .collect()
        };

        (pos, segments)
    }
}

impl From<&raw_data::SaveGame> for Map {
    fn from(savegame: &raw_data::SaveGame) -> Self {
        // TODO ENABLE THIS
        // let mut quest_tile_ids = HashSet::<i32>::default();
        // let mut quest_ids = HashSet::<i32>::default();

        // TODO convert sectionGridPos into gridPos and then into axial coordinates.
        // dbg!(&savegame.preplaced_tiles);
        // int num = Mathf.RoundToInt(worldPos.x / (_tileSize.x * 0.75f));
        // int y = Mathf.RoundToInt((worldPos.z + (float)Mathf.Abs(num % 2) * _tileSize.y / 2f) / _tileSize.y);
        // return new Vector2Int(num, y);

        // TODO ENABLE THIS
        // savegame
        //     .tiles
        //     .iter()
        //     .filter(|tile| tile.quest_tile.is_some())
        //     .for_each(|tile| {
        //         let q = tile.quest_tile.as_ref().unwrap();
        //         quest_ids.insert(q.quest_id.0);
        //         quest_tile_ids.insert(q.quest_tile_id.0);
        //     });

        // Accumulate min and max borders of map.
        let mut index_min = IVec2::ZERO;
        let mut index_max = IVec2::ZERO;

        // Pos                          -- tile_key_function()  --> IndexKey
        // IndexKey                     -- tile_index[]         --> Option<(SegmentIndex, SegmentCount)>
        // (SegmentIndex, SegmentCount) -- segments[]           --> &[Segment]

        // Cache the positions of tiles. Can't compute the index keys yet.
        // I'm using a primitive non-expanding scanrow indexing.
        let mut pos_map = Vec::<(Pos, usize, usize)>::default();
        pos_map.push((IVec2::new(0, 0), 0, 0));

        // Create segments vector (estimate the number of segments at three per tile).
        let mut segments = Vec::new();
        segments.reserve(savegame.tiles.len() * 3);

        // Prepend tiles list with empty tile (is this necessary when I start parsing special tiles?)
        for raw_tile in &savegame.tiles {
            // The base index is the index of the first segment in the large `segments` vec.
            let segment_base_index = segments.len();

            let (pos, tile_segments) = Map::load_tile(raw_tile);
            index_min.x = index_min.x.min(pos.x);
            index_min.y = index_min.y.min(pos.y);
            index_max.x = index_max.x.max(pos.x);
            index_max.y = index_max.y.max(pos.y);

            let segment_count = tile_segments.len();
            segments.extend(tile_segments.into_iter());
            pos_map.push((pos, segment_base_index, segment_count));
        }

        // Compute the cache size/offset.
        let lower = index_min - IVec2::ONE;
        let upper = index_max + IVec2::ONE;

        let index_offset = lower;
        let index_size = upper - lower + IVec2::ONE;
        let index_length = usize::try_from(index_size.x * index_size.y).unwrap();
        let mut tile_index = vec![None; index_length];
        let mut rendered_tiles = vec![None; index_length];

        // Insert the cached segment ids per position into the cache.
        for (pos, segment_base_index, segment_count) in pos_map {
            let position_key = Map::tile_key_function(index_offset, index_size, pos).unwrap();
            tile_index[position_key] = Some((segment_base_index, segment_count));

            let mut rendered = [Terrain::Empty; 6];
            for segment in &segments[segment_base_index..segment_base_index + segment_count] {
                for rotation in segment.rotations() {
                    rendered[rotation] = segment.terrain;
                }
            }
            rendered_tiles[position_key] = Some(rendered);
        }

        let (_, next_tile) = Map::load_tile(&savegame.tile_stack[0]);
        let mut rendered_next_tile = [Terrain::Empty; 6];
        for segment in &next_tile {
            for rotation in segment.rotations() {
                rendered_next_tile[rotation] = segment.terrain;
            }
        }

        Self {
            index_offset,
            index_size,
            tile_index,
            rendered_tiles,
            segments,
            next_tile,
            rendered_next_tile,
        }

        // ASLKDJASLDKJ
        // let mut probabilities = HashMap::default();
        // let num_tiles = tiles.len() as f32;
        // for tile in &tiles {
        //     let canonical_id = tile.canonical_id();
        //     if !probabilities.contains_key(&canonical_id) {
        //         probabilities.insert(canonical_id, (tile.parts.clone(), 0, 0.0));
        //     }
        //     let entry = probabilities.get_mut(&canonical_id).unwrap();
        //     entry.1 += 1;
        //     entry.2 = entry.1 as f32 / num_tiles;
        // }

        // // let mut probabilities_as_vec = probabilities.values().collect::<Vec<_>>();
        // // probabilities_as_vec.sort_by_key(|entry| usize::MAX - entry.1);
        // // dbg!(&probabilities_as_vec);
    }
}

impl Map {
    /// Compute the position of tile at `pos` in the index structure.
    pub fn tile_key(&self, pos: Pos) -> Option<TileKey> {
        Map::tile_key_function(self.index_offset, self.index_size, pos)
    }

    fn tile_position(&self, index_key: TileKey) -> Pos {
        let x = index_key as i32 % self.index_size.x;
        let y = (index_key as i32 - x) / self.index_size.x;
        Pos::new(x + self.index_offset.x, y + self.index_offset.y)
    }

    pub fn iter_tile_positions<'a>(&'a self) -> impl Iterator<Item = Pos> + 'a {
        self.tile_index
            .iter()
            .enumerate()
            .filter_map(|(key, content)| content.map(|_| self.tile_position(key)))
    }

    pub fn has(&self, pos: Pos) -> bool {
        self.tile_key(pos)
            .is_some_and(|key| self.tile_index[key].is_some())
    }

    pub fn segment_indices_at(&self, pos: Pos) -> Option<Range<SegmentIndex>> {
        self.tile_index[self.tile_key(pos)?].map(|(index, count)| (index..index + count))
    }

    pub fn segment_index_at(&self, pos: Pos, rotation: Rotation) -> Option<SegmentIndex> {
        self.segment_indices_at(pos)?
            .filter(|index| self.segment(*index).rotations().contains(&rotation))
            .next()
    }

    pub fn segment(&self, segment_index: SegmentIndex) -> &Segment {
        &self.segments[segment_index]
    }

    // fn segments_at(&self, pos: Pos) -> Option<(Range<SegmentIndex>, &[Segment])> {
    //     self.tile_key(pos)
    //         .and_then(|key| self.tile_index[key])
    //         .map(|(index, count)| ((index..index + count), &self.segments[index..index + count]))
    // }

    pub fn neighbor_pos_of(pos: Pos, rotation: Rotation) -> Pos {
        pos + match rotation {
            0 => Pos::new(0, 1),
            1 => Pos::new(1, 0),
            2 => Pos::new(1, -1),
            3 => Pos::new(0, -1),
            4 => Pos::new(-1, 0),
            5 => Pos::new(-1, 1),
            _ => panic!("Rotation should be 0-5, got {rotation}"),
        }
    }
}

/*
pub struct Map {
    /// Probability/count of getting a tile. Tiles are canonicalized by converting the terrain enum
    /// into int and choosing the lexicographically smallest rotation.
    // probabilities: HashMap<u32, ([Terrain; 6], usize, f32)>,
    //
    // /// Groups of segments on tiles.
    // assigned_groups: HashMap<(TileId, SegmentId), GroupId>,
}

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

//
// impl Map {
//     pub fn segment(&self, tile_id: TileId, segment_id: SegmentId) -> Option<&Segment> {
//         self.tile(tile_id).and_then(|tile| tile.segment(segment_id))
//     }
//
//     pub fn group_of(&self, tile_id: TileId, segment_id: SegmentId) -> GroupId {
//         self.assigned_groups[&(tile_id, segment_id)]
//     }
//
//     pub fn group(&self, group_id: GroupId) -> &Group {
//         &self.groups[group_id]
//     }
//
//     pub fn best_placements(&self) -> &BTreeMap<i32, BTreeMap<isize, (IVec2, Rotation)>> {
//         &self.best_placements
//     }
//
//     fn iter_neighbor_cells<'a>(
//         &'a self,
//         tile: &'a Tile,
//     ) -> impl Iterator<Item = (IVec2, Option<TileId>)> + 'a {
//         (0..6).map(|rotation| {
//             let neighbor_pos = tile.neighbor_pos(rotation);
//             (neighbor_pos, self.tile_id_at(neighbor_pos))
//         })
//     }
//
//     pub fn evaluate_best_placements(&mut self) {
//     }
//
//     /// Chance returned as number of previously used tiles that would have matched.
//     pub fn chance_of_finding_tile_for(&self, outer_edges: &[Terrain; 6]) -> usize {
//         let mut total_matching_count = 0;
//         for (inner_edges, tile_count, _) in self.probabilities.values() {
//             let matches = Tile::is_perfect_placement(inner_edges, outer_edges);
//             if matches {
//                 total_matching_count += tile_count;
//             }
//         }
//         total_matching_count
//     }
//     /// Collect the edge requirements for a tile at `pos`.
//     pub fn outer_edges(&self, pos: IVec2) -> [Terrain; 6] {
//         [0, 1, 2, 3, 4, 5].map(|side| self.terrain_of_neighbor_at(pos, side))
//     }
//
// }

*/
