use glam::IVec2;

use crate::raw_data::{self, QuestTile, SpecialTileId};

/// Number of sides on a hexagonal tile.
pub const HEX_SIDES: usize = 6;

pub const INT_: usize = 4;
pub const IVEC2_: usize = 2 * INT_;
pub const IVEC4_: usize = 4 * INT_;

#[derive(Clone, Copy, Debug)]
pub enum Form {
    Size1 = 0,
    Size2 = 1,
    Bridge = 2,   // 1-skip1-1
    Straight = 3, // 1-skip2-1
    Size3 = 4,
    JunctionLeft = 5,  // 2-skip1-1
    JunctionRight = 6, // 2-skip2-1
    ThreeWay = 7,      // 1-skip1-1-skip1-1
    Size4 = 8,
    FanOut = 9, // 3-skip1-1
    X = 10,     // 2-skip1-2
    Size5 = 11,
    Size6 = 12,

    LakeSize2 = 14,
    LakeSize3 = 15,
    LakeSize4 = 16,
    LakeSize5 = 17,
}

impl Form {
    /// Default unit count for non-quest tiles, based on form and terrain.
    /// Quest tiles override this with per-variant values from the tile segment tables.
    pub fn default_unit_count(self, terrain: Terrain) -> u32 {
        match (self, terrain) {
            // Houses
            (Form::Size1, Terrain::House) => 1,
            (Form::Size2, Terrain::House) => 2,
            (Form::Bridge, Terrain::House) => 3,
            (Form::Straight, Terrain::House) => 3,
            (Form::Size3, Terrain::House) => 3,
            (Form::JunctionLeft, Terrain::House) => 4,
            (Form::JunctionRight, Terrain::House) => 4,
            (Form::ThreeWay, Terrain::House) => 4,
            (Form::Size4, Terrain::House) => 5,
            (Form::FanOut, Terrain::House) => 5,
            (Form::X, Terrain::House) => 5,
            (Form::Size5, Terrain::House) => 7,
            (Form::Size6, Terrain::House) => 7,

            // Forest
            (Form::Size1, Terrain::Forest) => 4,
            (Form::Size2, Terrain::Forest) => 10,
            (Form::Bridge, Terrain::Forest) => 15,
            (Form::Straight, Terrain::Forest) => 17,
            (Form::Size3, Terrain::Forest) => 17,
            // TODO
            (Form::JunctionLeft, Terrain::Forest) => 20,
            (Form::JunctionRight, Terrain::Forest) => 4,
            (Form::ThreeWay, Terrain::Forest) => 20,
            (Form::Size4, Terrain::Forest) => 21,
            (Form::FanOut, Terrain::Forest) => 24,
            (Form::X, Terrain::Forest) => 24,
            (Form::Size5, Terrain::Forest) => 29,
            (Form::Size6, Terrain::Forest) => 37,

            // Wheat
            (Form::Size1, Terrain::Wheat) => 1,
            (Form::Size2, Terrain::Wheat) => 1,
            (Form::Bridge, Terrain::Wheat) => 2,
            (Form::Straight, Terrain::Wheat) => 2,
            (Form::Size3, Terrain::Wheat) => 1,
            (Form::JunctionLeft, Terrain::Wheat) => 2,
            (Form::JunctionRight, Terrain::Wheat) => 2,
            (Form::ThreeWay, Terrain::Wheat) => 3,
            (Form::Size4, Terrain::Wheat) => 2,
            (Form::FanOut, Terrain::Wheat) => 2,
            (Form::X, Terrain::Wheat) => 3,
            (Form::Size5, Terrain::Wheat) => 2,
            (Form::Size6, Terrain::Wheat) => 3,

            // Rail/River: one per segment.
            (_, Terrain::Rail | Terrain::River | Terrain::Lake) => 1,

            _ => 0,
        }
    }
}

impl From<&raw_data::SegmentTypeId> for Form {
    fn from(value: &raw_data::SegmentTypeId) -> Self {
        match value.0 {
            1 => Form::Size1,
            2 => Form::Size2,
            3 => Form::Bridge,
            4 => Form::Straight,
            5 => Form::Size3,
            6 => Form::JunctionLeft,
            7 => Form::JunctionRight,
            8 => Form::ThreeWay,
            9 => Form::Size4,
            10 => Form::FanOut,
            11 => Form::X,
            12 => Form::Size5,
            13 => Form::Size6,
            102 => Form::LakeSize2,
            105 => Form::LakeSize3,
            109 => Form::LakeSize4,
            111 => Form::LakeSize5,
            other => panic!("Unexpected segment type value {other}"),
        }
    }
}

/// Result of comparing two terrain types on adjacent tile edges.
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum EdgeMatch {
    /// Terrains are compatible and score points (e.g. forest-forest, river-lake).
    Matching,
    /// Terrains are compatible but don't score (e.g. wheat next to forest).
    Suboptimal,
    /// Terrains cannot be placed adjacent (e.g. rail next to river).
    Illegal,
    /// One or both sides have no neighbor tile (Missing). Always allowed, not scored.
    Missing,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug, Hash)]
pub enum Terrain {
    Missing = 0,
    Empty = 1,
    House = 2,
    Forest = 3,
    Wheat = 4,
    Rail = 5,
    River = 6,
    Lake = 7,
    Station = 8,
}

impl Terrain {
    #[allow(clippy::match_same_arms)]
    /// Check whether `self` would connect to a group through a `terrain` edge.
    pub fn extends_group_of(self, terrain: Terrain) -> bool {
        use Terrain::{Empty, Lake, Missing, Rail, River, Station};
        match (self, terrain) {
            // Station and lake can't be used as group-type.
            (_, Station | Lake | Empty | Missing) => unreachable!(),
            (Lake | Station, River) => true,
            (Station, Rail) => true,
            (a, b) => a == b,
        }
    }

    #[allow(clippy::match_same_arms)]
    pub fn connects_and_matches(self, other: Terrain) -> EdgeMatch {
        use EdgeMatch::{Illegal, Matching, Suboptimal};
        use Terrain::{Empty, Lake, Missing, Rail, River, Station};
        match (self, other) {
            // Placing adjacent to missing — no neighbor tile exists.
            (Missing, _) | (_, Missing) => EdgeMatch::Missing,
            // Empty connects with lake and station.
            (Empty, Lake | Station) => Matching,
            (Lake | Station, Empty) => Matching,

            // These two also connect.
            (Lake | Station, Lake | Station) => Matching,

            // River can connect to waterlike things only.
            (River, River | Lake | Station) => Matching,
            (Lake | Station, River) => Matching,

            // Rail can connect to rail things only.
            (Rail, Rail | Station) => Matching,
            (Station, Rail) => Matching,

            // Anything else doesn't connect with either rail or river.
            (River | Rail, _) => Illegal,
            (_, River | Rail) => Illegal,

            // Same terrain type matches.
            (a, b) if a == b => Matching,

            // Different non-special terrains are suboptimal but allowed.
            _ => Suboptimal,
        }
    }
}

impl From<&raw_data::GroupTypeId> for Terrain {
    fn from(value: &raw_data::GroupTypeId) -> Self {
        match value.0 {
            -1 => Terrain::Empty,
            0 => Terrain::House,
            1 => Terrain::Forest,
            2 => Terrain::Wheat,
            3 => Terrain::Rail,
            4 => Terrain::River,
            other => panic!("Unexpected terrain type value {other}"),
        }
    }
}

/// Use flat-top axial coordinates.
/// x -> 2 o'clock
/// y -> north
/// Offset coordinates are stupid and complex.
pub type Pos = IVec2;

/// Hex tile rotation index (0..HEX_SIDES). 0 = north, increasing clockwise.
pub type Rotation = usize;

#[derive(Debug, Clone)]
pub struct Segment {
    pub pos: Pos,
    pub form: Form,
    pub terrain: Terrain,
    pub rotation: Rotation,
    /// Number of visual units (houses, trees, fields, etc.) in this segment.
    pub unit_count: u32,
}

impl From<(&raw_data::Segment, Pos, Rotation)> for Segment {
    fn from(value: (&raw_data::Segment, Pos, Rotation)) -> Self {
        let (raw_segment, pos, tile_rotation) = value;

        let mut form = (&raw_segment.segment_type).into();
        let mut terrain = (&raw_segment.group_type).into();
        match (form, terrain) {
            // There are no 6-sided rivers.
            (Form::LakeSize2, Terrain::River) => {
                terrain = Terrain::Lake;
                form = Form::Size2;
            }
            (Form::LakeSize3, Terrain::River) => {
                terrain = Terrain::Lake;
                form = Form::Size3;
            }
            (Form::LakeSize4, Terrain::River) => {
                terrain = Terrain::Lake;
                form = Form::Size4;
            }
            (Form::LakeSize5, Terrain::River) => {
                terrain = Terrain::Lake;
                form = Form::Size5;
            }
            (Form::Size6, Terrain::River) => terrain = Terrain::Lake,
            (Form::LakeSize2 | Form::LakeSize3 | Form::LakeSize4 | Form::LakeSize5, _) => {
                unreachable!()
            }
            _ => {}
        }

        let raw_rotation: Rotation = raw_segment.rotation.try_into().unwrap();
        Self {
            pos,
            form,
            terrain,
            rotation: (raw_rotation + tile_rotation) % HEX_SIDES,
            unit_count: form.default_unit_count(terrain),
        }
    }
}

impl Segment {
    #[allow(clippy::match_same_arms)]
    pub fn rotations(&self) -> impl Iterator<Item = Rotation> + '_ {
        match self.form {
            Form::Size1 => [0].as_slice(),
            Form::Size2 => &[0, 1],
            Form::Bridge => &[0, 2],
            Form::Straight => &[0, 3],
            Form::Size3 => &[0, 1, 2],
            Form::JunctionLeft => &[0, 1, 3],
            Form::JunctionRight => &[0, 1, 4],
            Form::ThreeWay => &[0, 2, 4],
            Form::Size4 => &[0, 1, 2, 3],
            Form::FanOut => &[0, 1, 2, 4],
            Form::X => &[0, 1, 3, 4],
            Form::Size5 => &[0, 1, 2, 3, 4],
            Form::Size6 => &[0, 1, 2, 3, 4, 5],
            Form::LakeSize2 => &[],
            Form::LakeSize3 => &[],
            Form::LakeSize4 => &[0, 1, 2, 3],
            Form::LakeSize5 => &[],
        }
        .iter()
        .map(|local| (self.rotation + local) % HEX_SIDES)
    }

    pub fn contains_rotation(&self, rotation: Rotation) -> bool {
        self.rotations().find(|r| r == &rotation).is_some()
    }
}

/// (form, terrain, rotation, unit_count)
type SegmentDef = (Form, Terrain, usize, u32);

#[allow(clippy::match_same_arms)]
fn wheat_tile_segments(id: i32) -> Option<Vec<SegmentDef>> {
    Some(match id {
        // 2AA_4AF (Normal, BigTree, Granary, Windmill)
        2 | 3 | 4 | 5 => vec![
            (Form::Size2, Terrain::Wheat, 5, 1),
            (Form::Size4, Terrain::Forest, 1, 21),
        ],
        // 2AA
        92 => vec![(Form::Size2, Terrain::Wheat, 0, 1)],
        // 2AA_2AV_1AV
        1 => vec![
            (Form::Size2, Terrain::Wheat, 0, 1),
            (Form::Size2, Terrain::House, 2, 2),
            (Form::Size1, Terrain::House, 5, 1),
        ],
        // 3AA_1AV (Normal, Granary, Windmill)
        6 | 7 | 8 => vec![
            (Form::Size3, Terrain::Wheat, 3, 1),
            (Form::Size1, Terrain::House, 0, 1),
        ],
        // 4AA_2AF (Normal, Granary)
        9 | 10 => vec![
            (Form::Size4, Terrain::Wheat, 0, 2),
            (Form::Size2, Terrain::Forest, 4, 7),
        ],
        // 4BA_1AF_1AF (Normal, BigTree)
        11 | 12 => vec![
            (Form::X, Terrain::Wheat, 0, 3),
            (Form::Size1, Terrain::Forest, 2, 1),
            (Form::Size1, Terrain::Forest, 5, 1),
        ],
        // 6AA (Normal, BigTree, Windmill)
        13 | 14 | 15 => vec![(Form::Size6, Terrain::Wheat, 0, 3)],
        _ => return None,
    })
}

#[allow(clippy::match_same_arms)]
fn forest_tile_segments(id: i32) -> Option<Vec<SegmentDef>> {
    Some(match id {
        // 1AF (Normal, Deer, Bear, Boar)
        16 | 19 | 65 | 66 => vec![(Form::Size1, Terrain::Forest, 0, 1)],
        // 2AF (Normal, Deer, Bear, Boar)
        67 | 68 | 69 | 70 => vec![(Form::Size2, Terrain::Forest, 0, 2)],
        // 3AF (Normal, Deer, Bear, Boar)
        20 | 21 | 71 | 72 => vec![(Form::Size3, Terrain::Forest, 0, 17)],
        // 4AF (Normal, Ruin)
        22 | 73 => vec![(Form::Size4, Terrain::Forest, 0, 21)],
        // 6AF (Normal, Bear, Boar, Ruin)
        23 | 74 | 75 | 76 => vec![(Form::Size6, Terrain::Forest, 0, 37)],
        // 6AF (Deer)
        24 => vec![(Form::Size6, Terrain::Forest, 0, 26)],
        // 1AF_2AW (Normal, Deer)
        17 | 18 => vec![
            (Form::Size1, Terrain::Forest, 3, 1),
            (Form::Size2, Terrain::Lake, 0, 1),
        ],
        _ => return None,
    })
}

#[allow(clippy::match_same_arms)]
fn village_tile_segments(id: i32) -> Option<Vec<SegmentDef>> {
    Some(match id {
        // 2AV
        33 => vec![(Form::Size2, Terrain::House, 0, 2)],
        // 3AV (Normal, Fountain)
        35 | 38 => vec![(Form::Size3, Terrain::House, 0, 3)],
        // 3AV_3AF (Normal, Fountain, Tower, Fox)
        34 | 36 | 37 | 80 => vec![
            (Form::Size3, Terrain::House, 0, 3),
            (Form::Size3, Terrain::Forest, 3, 17),
        ],
        // 4BV_1AF_1AF (Normal, Fountain, Tower, Fox)
        39 | 40 | 41 | 84 => vec![
            (Form::FanOut, Terrain::House, 4, 5),
            (Form::Size1, Terrain::Forest, 1, 1),
            (Form::Size1, Terrain::Forest, 3, 1),
        ],
        // 5AV_1AF (Normal, Fox)
        85 | 86 => vec![
            (Form::Size5, Terrain::House, 0, 7),
            (Form::Size1, Terrain::Forest, 5, 1),
        ],
        // 6AV (Normal)
        42 => vec![(Form::Size6, Terrain::House, 0, 7)],
        // 6AV (Fountain, Tower)
        43 | 44 => vec![(Form::Size6, Terrain::House, 0, 6)],
        _ => return None,
    })
}

#[allow(clippy::match_same_arms)]
fn rail_tile_segments(id: i32) -> Option<Vec<SegmentDef>> {
    Some(match id {
        // 2BT_3AA_1AA (BUG: says 3AA but tile only has size2 agriculture)
        25 => vec![
            (Form::Bridge, Terrain::Rail, 0, 1),
            (Form::Size1, Terrain::Wheat, 1, 1),
            (Form::Size2, Terrain::Wheat, 4, 1),
        ],
        // 2BT_3AF_1AF
        26 => vec![
            (Form::Bridge, Terrain::Rail, 0, 1),
            (Form::Size1, Terrain::Forest, 1, 1),
            (Form::Size3, Terrain::Forest, 3, 17),
        ],
        // 2BT_3AV_1AV
        27 => vec![
            (Form::Bridge, Terrain::Rail, 0, 1),
            (Form::Size1, Terrain::House, 1, 1),
            (Form::Size3, Terrain::House, 3, 3),
        ],
        // 2CT_1AF_1AV (Normal, Locomotive)
        28 | 29 => vec![
            (Form::Straight, Terrain::Rail, 0, 1),
            (Form::Size1, Terrain::Forest, 1, 1),
            (Form::Size1, Terrain::House, 4, 1),
        ],
        // 2CT (Normal, Locomotive)
        30 | 31 => vec![(Form::Straight, Terrain::Rail, 0, 1)],
        _ => return None,
    })
}

#[allow(clippy::match_same_arms)]
fn water_tile_segments(id: i32) -> Option<Vec<SegmentDef>> {
    Some(match id {
        // 2BW_3AF_1AF (Normal, Boat)
        45 | 46 => vec![
            (Form::Bridge, Terrain::River, 0, 1),
            (Form::Size1, Terrain::Forest, 1, 1),
            (Form::Size3, Terrain::Forest, 3, 17),
        ],
        // 2CW (Normal, Boat, Beaver)
        47 | 54 | 58 => vec![(Form::Straight, Terrain::River, 0, 1)],
        // 2CW_2AA_1AV (Normal, Watermill)
        49 | 50 => vec![
            (Form::Straight, Terrain::River, 0, 1),
            (Form::Size2, Terrain::Wheat, 1, 1),
            (Form::Size1, Terrain::House, 5, 1),
        ],
        // 2CW_2AF_1AA (Normal, Watermill)
        51 | 52 => vec![
            (Form::Straight, Terrain::River, 0, 1),
            (Form::Size2, Terrain::Forest, 1, 2),
            (Form::Size1, Terrain::Wheat, 4, 1),
        ],
        // 2CW_2AF_2AA (Normal, Beaver)
        87 | 88 => vec![
            (Form::Straight, Terrain::River, 0, 1),
            (Form::Size2, Terrain::Forest, 1, 2),
            (Form::Size2, Terrain::Wheat, 4, 1),
        ],
        // 2CW_2AV_1AV
        48 => vec![
            (Form::Straight, Terrain::River, 0, 1),
            (Form::Size2, Terrain::House, 1, 2),
            (Form::Size1, Terrain::House, 5, 1),
        ],
        // 2CW_2AV_2AV_Watermill
        53 => vec![
            (Form::Straight, Terrain::River, 0, 1),
            (Form::Size2, Terrain::House, 1, 2),
            (Form::Size2, Terrain::House, 4, 2),
        ],
        // 3AW_3AF (Normal, SwanGoose, Beaver)
        59 | 89 | 60 => vec![
            (Form::Size3, Terrain::Lake, 0, 1),
            (Form::Size3, Terrain::Forest, 3, 17),
        ],
        // 4AW_2AF (Normal, Beaver, SwanGoose)
        61 | 62 | 90 => vec![
            (Form::Size4, Terrain::Lake, 0, 1),
            (Form::Size2, Terrain::Forest, 4, 2),
        ],
        // 6AW (Normal, Beaver, Ruin, Boat, SwanGoose)
        55 | 63 | 64 | 56 | 91 => vec![(Form::Size6, Terrain::Lake, 0, 1)],
        // 6AW_6AT (WaterTrainStation)
        57 => vec![(Form::Size6, Terrain::Station, 0, 1)],
        _ => return None,
    })
}

fn raw_segments_for_quest_tile(id: i32) -> Option<Vec<SegmentDef>> {
    wheat_tile_segments(id)
        .or_else(|| forest_tile_segments(id))
        .or_else(|| village_tile_segments(id))
        .or_else(|| rail_tile_segments(id))
        .or_else(|| water_tile_segments(id))
}

/// Get the primary terrain type for a quest tile (the terrain with the most segments).
pub fn quest_terrain(quest_tile_id: i32) -> Option<Terrain> {
    let segments = raw_segments_for_quest_tile(quest_tile_id)?;
    // The first segment's terrain is the quest's target terrain.
    segments.first().map(|(_, terrain, _, _)| *terrain)
}

pub fn segments_from_quest_tile(pos: IVec2, quest_tile: &QuestTile) -> Vec<Segment> {
    let id = quest_tile.quest_tile_id.0;
    let segments = raw_segments_for_quest_tile(id).unwrap_or_else(|| {
        println!("{}\t{}\t=> {}", pos.x, pos.y, id);
        todo!("Unhandled quest tile id {id}");
    });

    segments
        .into_iter()
        .map(|(form, terrain, rotation, unit_count)| Segment {
            pos,
            form,
            terrain,
            rotation,
            unit_count,
        })
        .collect()
}

#[allow(clippy::single_match_else)]
pub fn segments_from_special_tile_id(pos: IVec2, special_tile_id: &SpecialTileId) -> Vec<Segment> {
    match special_tile_id.0 {
        1 => {
            vec![Segment {
                pos,
                form: Form::Size6,
                terrain: Terrain::Station,
                rotation: 0,
                unit_count: 1,
            }]
        }
        other => {
            println!("{}\t{}\t=> {}", pos.x, pos.y, other);
            todo!("Unhandled special tile id {other}");
        }
    }
}
