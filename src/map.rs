use std::ops::Range;

use std::collections::HashMap;

use crate::{
    data::{
        quest_terrain, segments_from_quest_tile, segments_from_special_tile_id, HexPos, Rotation,
        Segment, Terrain, HEX_SIDES,
    },
    raw_data,
};
use glam::IVec2;

/// Index into the spatial tile index (derived from tile position).
pub type TileKey = usize;

/// Index into the flat segments array.
pub type SegmentIndex = usize;
/// Number of segments belonging to a single tile.
pub type SegmentCount = usize;

/// Whether the quest requires exactly the target count or at least the target count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestType {
    /// Group must have at least target_value units.
    MoreThan,
    /// Group must have exactly target_value units.
    Exact,
    /// Flag quest: close the group (0 open edges).
    Flag,
    /// Unknown quest type (quest_id not recognized).
    Unknown,
}

impl QuestType {
    /// Derive the quest type from the quest_id.
    /// Pattern: base ID = more than, base+1 = exact.
    fn from_quest_id(quest_id: i32) -> Self {
        match quest_id {
            // Flag quest (close the group).
            1 => QuestType::Flag,
            30 => QuestType::MoreThan,
            31 => QuestType::Exact,
            // House
            10 => QuestType::MoreThan,
            11 => QuestType::Exact,
            // Forest
            20 => QuestType::MoreThan,
            21 => QuestType::Exact,
            // Rail
            41 => QuestType::MoreThan,
            42 => QuestType::Exact,
            // River/Lake
            50 => QuestType::MoreThan,
            51 => QuestType::Exact,
            _ => QuestType::Unknown,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            QuestType::MoreThan => ">=",
            QuestType::Exact => "==",
            QuestType::Flag => "flag",
            QuestType::Unknown => "??",
        }
    }
}

/// A quest placed on a tile, targeting a specific terrain group size.

#[derive(Debug, Clone)]
pub struct Quest {
    pub terrain: Terrain,
    pub target_value: i32,
    pub active: bool,
    pub quest_type: QuestType,
    pub quest_id: i32,
    pub quest_level: i32,
    pub quest_queue_index: i32,
    pub unlocked_challenge_id: i32,
}

type TileData = (
    Vec<(HexPos, usize, usize)>,
    Vec<Segment>,
    HashMap<HexPos, Quest>,
    IVec2,
    IVec2,
    IVec2,
);
type IndexData = (
    Vec<Option<(SegmentIndex, SegmentCount)>>,
    Vec<Option<[Option<SegmentIndex>; HEX_SIDES]>>,
);

pub struct Map {
    /// Defines the smallest possible coordinate in the index.
    pub index_offset: IVec2,
    /// Defines the extents of the index structure.
    pub index_size: IVec2,

    pub world_y_extents: IVec2,

    /// Maps a tile position key to a set of segment indices.
    pub tile_index: Vec<Option<(SegmentIndex, SegmentCount)>>,
    pub rendered_tiles: Vec<Option<[Option<SegmentIndex>; HEX_SIDES]>>,
    /// Maps a segment id (index in this array) to a segment.
    pub segments: Vec<Segment>,

    /// Quest info keyed by tile position.
    pub quests: HashMap<HexPos, Quest>,

    /// The next tile is also represented by a set of segments.
    pub next_tile: Vec<Segment>,

    pub rendered_next_tile: [Terrain; HEX_SIDES],
    /// Quest attached to the next tile (if any).
    pub next_tile_quest: Option<Quest>,
}

impl Default for Map {
    fn default() -> Self {
        Self {
            index_offset: IVec2::default(),
            index_size: IVec2::default(),
            world_y_extents: IVec2::default(),
            tile_index: Vec::default(),
            rendered_tiles: Vec::default(),
            segments: Vec::default(),
            quests: HashMap::default(),
            next_tile: Vec::default(),
            rendered_next_tile: [Terrain::Missing; HEX_SIDES],
            next_tile_quest: None,
        }
    }
}

/// Functions for initialization of map.
impl Map {
    /// Compute the position of tile at `pos` in the index structure.
    fn tile_key_function(offset: IVec2, size: IVec2, pos: HexPos) -> Option<TileKey> {
        let upper = offset + size;
        let valid_s = pos.x() >= offset.x && pos.x() < upper.x;
        let valid_t = pos.y() >= offset.y && pos.y() < upper.y;
        (valid_s && valid_t)
            .then(|| usize::try_from((pos.y() - offset.y) * size.x + (pos.x() - offset.x)).unwrap())
    }

    fn load_tile(raw_tile: &raw_data::Tile) -> (HexPos, Vec<Segment>) {
        // Hex grid tutorial:
        // https://www.redblobgames.com/grids/hexagons/#line-drawing
        let pos = HexPos::new(raw_tile.s, raw_tile.t - ((raw_tile.s + 1) & -2i32) / 2);

        // We store the rotation on the segments, not on the tiles.
        let tile_rotation = raw_tile.rotation.try_into().unwrap();
        let rotate = |segment: &mut Segment| {
            segment.rotation = (segment.rotation + tile_rotation) % HEX_SIDES;
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

    /// Load all tiles from the savegame, accumulating positions, segments, quests, and map bounds.
    fn load_all_tiles(savegame: &raw_data::SaveGame) -> TileData {
        let mut index_min = IVec2::ZERO;
        let mut index_max = IVec2::ZERO;
        let mut world_y_extents = IVec2::new(i32::MAX, i32::MIN);

        // Pos                          -- tile_key_function()  --> IndexKey
        // IndexKey                     -- tile_index[]         --> Option<(SegmentIndex, SegmentCount)>
        // (SegmentIndex, SegmentCount) -- segments[]           --> &[Segment]

        let mut pos_map = Vec::<(HexPos, usize, usize)>::default();
        pos_map.push((HexPos::new(0, 0), 0, 0));

        let mut segments = Vec::with_capacity(savegame.tiles.len() * 3);
        let mut quests = HashMap::new();

        for raw_tile in &savegame.tiles {
            let segment_base_index = segments.len();
            let (pos, tile_segments) = Map::load_tile(raw_tile);

            index_min.x = index_min.x.min(pos.x());
            index_min.y = index_min.y.min(pos.y());
            index_max.x = index_max.x.max(pos.x());
            index_max.y = index_max.y.max(pos.y());
            let world_y = pos.y() + (pos.x() + 1) / 2;
            world_y_extents.x = world_y_extents.x.min(world_y);
            world_y_extents.y = world_y_extents.y.max(world_y);

            // Extract quest info if present.
            if let Some(quest) = Map::extract_quest(raw_tile) {
                quests.insert(pos, quest);
            }

            let segment_count = tile_segments.len();
            segments.extend(tile_segments);
            pos_map.push((pos, segment_base_index, segment_count));
        }

        (
            pos_map,
            segments,
            quests,
            index_min,
            index_max,
            world_y_extents,
        )
    }

    /// Build the spatial index and per-rotation rendered tile lookups.
    fn build_index(
        pos_map: &[(HexPos, usize, usize)],
        segments: &[Segment],
        index_offset: IVec2,
        index_size: IVec2,
    ) -> IndexData {
        let index_length = usize::try_from(index_size.x * index_size.y).unwrap();
        let mut tile_index = vec![None; index_length];
        let mut rendered_tiles = vec![None; index_length];

        for &(pos, segment_base_index, segment_count) in pos_map {
            let position_key = Map::tile_key_function(index_offset, index_size, pos).unwrap();
            tile_index[position_key] = Some((segment_base_index, segment_count));

            let mut rendered = [None; HEX_SIDES];
            for (segment_index, segment) in segments
                .iter()
                .enumerate()
                .skip(segment_base_index)
                .take(segment_count)
            {
                for rotation in segment.rotations() {
                    rendered[rotation] = Some(segment_index);
                }
            }
            rendered_tiles[position_key] = Some(rendered);
        }

        (tile_index, rendered_tiles)
    }

    /// Extract quest info from a raw tile (if present).
    fn extract_quest(raw_tile: &raw_data::Tile) -> Option<Quest> {
        let qt = raw_tile.quest_tile.as_ref()?;
        let terrain = quest_terrain(qt.quest_tile_id)?;
        Some(Quest {
            terrain,
            target_value: qt.target_value,
            active: qt.quest_active,
            quest_type: QuestType::from_quest_id(qt.quest_id.0),
            quest_id: qt.quest_id.0,
            quest_level: qt.quest_level,
            quest_queue_index: qt.quest_queue_index,
            unlocked_challenge_id: qt.unlocked_challenge_id.0,
        })
    }

    /// Render the next tile from the tile stack into a per-rotation terrain array.
    fn render_next_tile(next_tile: &[Segment]) -> [Terrain; 6] {
        let mut rendered = [Terrain::Empty; HEX_SIDES];
        for segment in next_tile {
            for rotation in segment.rotations() {
                rendered[rotation] = segment.terrain;
            }
        }
        rendered
    }
}

impl From<&raw_data::SaveGame> for Map {
    fn from(savegame: &raw_data::SaveGame) -> Self {
        let (pos_map, segments, quests, index_min, index_max, world_y_extents) =
            Map::load_all_tiles(savegame);

        let index_offset = index_min - IVec2::ONE;
        let index_size = index_max + IVec2::ONE - index_offset + IVec2::ONE;

        let (tile_index, rendered_tiles) =
            Map::build_index(&pos_map, &segments, index_offset, index_size);

        let (next_tile, rendered_next_tile, next_tile_quest) =
            if let Some(next_raw_tile) = savegame.tile_stack.first() {
                let (_, next_tile) = Map::load_tile(next_raw_tile);
                let rendered_next_tile = Map::render_next_tile(&next_tile);
                let next_tile_quest = Map::extract_quest(next_raw_tile);
                (next_tile, rendered_next_tile, next_tile_quest)
            } else {
                log::warn!("Savegame has empty tile_stack, no next tile available");
                (Vec::new(), [Terrain::Missing; HEX_SIDES], None)
            };

        Self {
            world_y_extents,
            index_offset,
            index_size,
            tile_index,
            rendered_tiles,
            segments,
            quests,
            next_tile,
            rendered_next_tile,
            next_tile_quest,
        }
    }
}

impl Map {
    /// Compute the position of tile at `pos` in the index structure.
    pub fn tile_key(&self, pos: HexPos) -> Option<TileKey> {
        Map::tile_key_function(self.index_offset, self.index_size, pos)
    }

    fn tile_position(&self, index_key: TileKey) -> HexPos {
        let i32_key = i32::try_from(index_key).unwrap();
        let x = i32_key % self.index_size.x;
        let y = (i32_key - x) / self.index_size.x;
        HexPos::new(x + self.index_offset.x, y + self.index_offset.y)
    }

    pub fn iter_tile_positions(&self) -> impl Iterator<Item = HexPos> + '_ {
        self.tile_index
            .iter()
            .enumerate()
            .filter_map(|(key, content)| content.map(|_| self.tile_position(key)))
    }

    pub fn has(&self, pos: HexPos) -> bool {
        self.tile_key(pos)
            .is_some_and(|key| self.tile_index[key].is_some())
    }

    pub fn segment_indices_at(&self, pos: HexPos) -> Option<Range<SegmentIndex>> {
        self.tile_index[self.tile_key(pos)?].map(|(index, count)| index..index + count)
    }

    /// Important: returns None if either there is no tile there
    /// or if the tile that is present has no segment at this rotation.
    /// If that is a concern, check with `has` beforehand.
    pub fn segment_at(&self, pos: HexPos, rotation: Rotation) -> Option<(SegmentIndex, &Segment)> {
        self.segment_indices_at(pos)?.find_map(|index| {
            let segment = self.segment(index);
            segment
                .contains_rotation(rotation)
                .then_some((index, segment))
        })
    }

    pub fn segment(&self, segment_index: SegmentIndex) -> &Segment {
        &self.segments[segment_index]
    }

    /// The rotation pointing in the opposite direction.
    pub fn opposite_side(rotation: Rotation) -> Rotation {
        crate::hex::opposite_side(rotation)
    }

    pub fn neighbor_pos_of(pos: HexPos, rotation: Rotation) -> HexPos {
        crate::hex::neighbor_pos_of(pos, rotation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{Form, HexPos, Terrain, HEX_SIDES};
    use crate::hex::neighbor_pos_of;
    use glam::IVec2;
    use std::collections::HashSet;

    /// Build a minimal Map with a hex flower: center tile at origin plus 6 neighbors.
    /// Each tile gets a single Size6 segment (covers all 6 rotations) with the given terrain.
    fn make_hex_flower() -> Map {
        let center = HexPos::ZERO;
        let positions: Vec<HexPos> = std::iter::once(center)
            .chain((0..HEX_SIDES).map(|r| neighbor_pos_of(center, r)))
            .collect();

        // Determine index bounds.
        let min_x = positions.iter().map(|p| p.x()).min().unwrap();
        let min_y = positions.iter().map(|p| p.y()).min().unwrap();
        let max_x = positions.iter().map(|p| p.x()).max().unwrap();
        let max_y = positions.iter().map(|p| p.y()).max().unwrap();

        let index_offset = IVec2::new(min_x - 1, min_y - 1);
        let index_size = IVec2::new(
            max_x + 1 - index_offset.x + 1,
            max_y + 1 - index_offset.y + 1,
        );
        let index_length = (index_size.x * index_size.y) as usize;

        let terrains = [
            Terrain::House,
            Terrain::Forest,
            Terrain::Wheat,
            Terrain::River,
            Terrain::Forest,
            Terrain::House,
            Terrain::Wheat,
        ];

        let mut segments = Vec::new();
        let mut tile_index = vec![None; index_length];
        let mut rendered_tiles = vec![None; index_length];

        for (i, &pos) in positions.iter().enumerate() {
            let seg_base = segments.len();
            let terrain = terrains[i];
            segments.push(Segment {
                pos,
                form: Form::Size6,
                terrain,
                rotation: 0,
                unit_count: Form::Size6.default_unit_count(terrain),
            });

            let key = Map::tile_key_function(index_offset, index_size, pos).unwrap();
            tile_index[key] = Some((seg_base, 1));

            // Size6 covers all 6 rotations from base rotation 0.
            let rendered = [Some(seg_base); HEX_SIDES];
            rendered_tiles[key] = Some(rendered);
        }

        Map {
            index_offset,
            index_size,
            world_y_extents: IVec2::new(-1, 1),
            tile_index,
            rendered_tiles,
            segments,
            quests: HashMap::new(),
            next_tile: Vec::new(),
            rendered_next_tile: [Terrain::Missing; HEX_SIDES],
            next_tile_quest: None,
        }
    }

    /// Build a map with two adjacent tiles that have multi-segment layouts,
    /// to test segment_at with partial coverage.
    fn make_two_tile_map() -> Map {
        let pos_a = HexPos::ZERO;
        let pos_b = neighbor_pos_of(pos_a, 1); // neighbor at rotation 1

        let index_offset = IVec2::new(-1, -1);
        let index_size = IVec2::new(4, 4);
        let index_length = (index_size.x * index_size.y) as usize;

        let mut segments = Vec::new();
        let mut tile_index = vec![None; index_length];
        let mut rendered_tiles = vec![None; index_length];

        // Tile A: Size3 Forest at rotation 0 (covers rotations 0,1,2)
        //       + Size3 Wheat at rotation 3 (covers rotations 3,4,5)
        let seg_a0 = segments.len();
        segments.push(Segment {
            pos: pos_a,
            form: Form::Size3,
            terrain: Terrain::Forest,
            rotation: 0,
            unit_count: 17,
        });
        segments.push(Segment {
            pos: pos_a,
            form: Form::Size3,
            terrain: Terrain::Wheat,
            rotation: 3,
            unit_count: 1,
        });

        let key_a = Map::tile_key_function(index_offset, index_size, pos_a).unwrap();
        tile_index[key_a] = Some((seg_a0, 2));
        let mut rendered_a = [None; HEX_SIDES];
        // Size3 at rotation 0: covers 0,1,2
        rendered_a[..3].fill(Some(seg_a0));
        // Size3 at rotation 3: covers 3,4,5
        rendered_a[3..].fill(Some(seg_a0 + 1));
        rendered_tiles[key_a] = Some(rendered_a);

        // Tile B: Size6 River at rotation 0 (covers all)
        let seg_b0 = segments.len();
        segments.push(Segment {
            pos: pos_b,
            form: Form::Size6,
            terrain: Terrain::River,
            rotation: 0,
            unit_count: 1,
        });

        let key_b = Map::tile_key_function(index_offset, index_size, pos_b).unwrap();
        tile_index[key_b] = Some((seg_b0, 1));
        let rendered_b = [Some(seg_b0); HEX_SIDES];
        rendered_tiles[key_b] = Some(rendered_b);

        Map {
            index_offset,
            index_size,
            world_y_extents: IVec2::new(-1, 1),
            tile_index,
            rendered_tiles,
            segments,
            quests: HashMap::new(),
            next_tile: Vec::new(),
            rendered_next_tile: [Terrain::Missing; HEX_SIDES],
            next_tile_quest: None,
        }
    }

    #[test]
    fn test_iter_tile_positions_hex_flower() {
        let map = make_hex_flower();
        let positions: HashSet<HexPos> = map.iter_tile_positions().collect();
        assert_eq!(positions.len(), 7, "Hex flower should have 7 tiles");
        assert!(positions.contains(&HexPos::ZERO));
        for r in 0..HEX_SIDES {
            let neighbor = neighbor_pos_of(HexPos::ZERO, r);
            assert!(
                positions.contains(&neighbor),
                "Missing neighbor at rotation {r}: {neighbor:?}"
            );
        }
    }

    #[test]
    fn test_tile_at_present_and_absent() {
        let map = make_hex_flower();
        // Center and all neighbors should be present.
        assert!(map.has(HexPos::ZERO));
        for r in 0..HEX_SIDES {
            assert!(map.has(neighbor_pos_of(HexPos::ZERO, r)));
        }
        // A position two steps away should not be present.
        let far = neighbor_pos_of(neighbor_pos_of(HexPos::ZERO, 0), 0);
        assert!(!map.has(far));
    }

    #[test]
    fn test_segment_at_center() {
        let map = make_hex_flower();
        // Center tile is House (Size6, rotation 0), covers all rotations.
        for r in 0..HEX_SIDES {
            let result = map.segment_at(HexPos::ZERO, r);
            assert!(
                result.is_some(),
                "Center should have segment at rotation {r}"
            );
            let (_, seg) = result.unwrap();
            assert_eq!(seg.terrain, Terrain::House);
            assert_eq!(seg.form, Form::Size6);
        }
    }

    #[test]
    fn test_segment_at_neighbors_terrain() {
        let map = make_hex_flower();
        let expected = [
            Terrain::Forest,
            Terrain::Wheat,
            Terrain::River,
            Terrain::Forest,
            Terrain::House,
            Terrain::Wheat,
        ];
        for (r, &expected_terrain) in expected.iter().enumerate() {
            let pos = neighbor_pos_of(HexPos::ZERO, r);
            let (_, seg) = map.segment_at(pos, 0).unwrap();
            assert_eq!(
                seg.terrain, expected_terrain,
                "Neighbor at rotation {r} has wrong terrain"
            );
        }
    }

    #[test]
    fn test_segment_at_absent_tile_returns_none() {
        let map = make_hex_flower();
        let far = HexPos::new(10, 10);
        assert!(map.segment_at(far, 0).is_none());
    }

    #[test]
    fn test_multi_segment_tile() {
        let map = make_two_tile_map();
        let pos_a = HexPos::ZERO;

        // Rotations 0,1,2 should be Forest.
        for r in 0..3 {
            let (_, seg) = map.segment_at(pos_a, r).unwrap();
            assert_eq!(
                seg.terrain,
                Terrain::Forest,
                "Rotation {r} should be Forest"
            );
        }
        // Rotations 3,4,5 should be Wheat.
        for r in 3..6 {
            let (_, seg) = map.segment_at(pos_a, r).unwrap();
            assert_eq!(seg.terrain, Terrain::Wheat, "Rotation {r} should be Wheat");
        }
    }

    #[test]
    fn test_edge_connectivity_adjacent_tiles() {
        let map = make_two_tile_map();
        let pos_a = HexPos::ZERO;
        let pos_b = neighbor_pos_of(pos_a, 1);

        // From tile A looking at rotation 1, the neighbor is tile B.
        // Tile A at rotation 1 is Forest (Size3 covers 0,1,2).
        let (_, seg_a) = map.segment_at(pos_a, 1).unwrap();
        assert_eq!(seg_a.terrain, Terrain::Forest);

        // Tile B at opposite rotation (4) is River.
        let opp = crate::hex::opposite_side(1);
        assert_eq!(opp, 4);
        let (_, seg_b) = map.segment_at(pos_b, opp).unwrap();
        assert_eq!(seg_b.terrain, Terrain::River);

        // Both tiles exist and have segments at the shared edge.
        assert!(map.has(pos_a));
        assert!(map.has(pos_b));
    }

    #[test]
    fn test_hex_flower_edge_connectivity_all_directions() {
        let map = make_hex_flower();
        let center = HexPos::ZERO;

        for r in 0..HEX_SIDES {
            let neighbor = neighbor_pos_of(center, r);
            // Center has a segment at rotation r.
            assert!(map.segment_at(center, r).is_some());
            // Neighbor has a segment at the opposite rotation.
            let opp = crate::hex::opposite_side(r);
            assert!(
                map.segment_at(neighbor, opp).is_some(),
                "Neighbor at direction {r} should have segment at opposite rotation {opp}"
            );
        }
    }

    #[test]
    fn test_segment_indices_at() {
        let map = make_two_tile_map();
        // Tile A has 2 segments.
        let range = map.segment_indices_at(HexPos::ZERO).unwrap();
        assert_eq!(range.len(), 2);

        // Tile B has 1 segment.
        let pos_b = neighbor_pos_of(HexPos::ZERO, 1);
        let range = map.segment_indices_at(pos_b).unwrap();
        assert_eq!(range.len(), 1);

        // Absent tile returns None.
        assert!(map.segment_indices_at(HexPos::new(5, 5)).is_none());
    }

    #[test]
    fn test_default_map_is_empty() {
        let map = Map::default();
        assert_eq!(map.iter_tile_positions().count(), 0);
        assert_eq!(map.segments.len(), 0);
    }
}
