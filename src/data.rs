use glam::IVec2;

use crate::raw_data::{self, QuestTile, SpecialTileId};

pub const PAD_: usize = 4;
pub const _BOOL_: usize = 4;
pub const INT_: usize = 4;
pub const FLOAT_: usize = 4;
pub const VEC2_: usize = 2 * FLOAT_;
pub const _VEC3_: usize = 3 * FLOAT_ + PAD_;
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
    pub fn connects_and_matches(self, other: Terrain) -> Option<bool> {
        use Terrain::{Empty, Lake, Missing, Rail, River, Station};
        match (self, other) {
            // Placing adjacent to missing is ok.
            (Missing, _) | (_, Missing) => Some(true),
            // Empty connects with lake and station.
            (Empty, Lake | Station) => Some(true),
            (Lake | Station, Empty) => Some(true),

            // These two also connect.
            (Lake | Station, Lake | Station) => Some(true),

            // River can connect to waterlike things only.
            // River to river-like is a better choice.
            (River, River | Lake | Station) => Some(true),
            (Lake | Station, River) => Some(true),

            // Rail can connect to rail things only.
            // Rail to rail-like is a better choice.
            (Rail, Rail | Station) => Some(true),
            (Station, Rail) => Some(true),

            // Anything else doesn't conenct with either rail or river.
            (River | Rail, _) => None,
            (_, River | Rail) => None,

            // Station to station, lake to lake.
            (a, b) => Some(a == b),
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
/// x -> 2 o'cock
/// y -> north
/// Offset coordinates are stupid and complex.
pub type Pos = IVec2;
pub type Rotation = usize;

#[derive(Debug, Clone)]
pub struct Segment {
    pub pos: Pos,
    pub form: Form,
    pub terrain: Terrain,
    pub rotation: Rotation,
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
            rotation: (raw_rotation + tile_rotation) % 6,
        }
    }
}

impl Segment {
    #[allow(clippy::match_same_arms)]
    pub fn rotations(&self) -> Vec<Rotation> {
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
        .map(|local| (self.rotation + local) % 6)
        .collect()
    }

    // pub fn iter_rotations(&self) -> impl Iterator<Item = Rotation> {
    //     self.rotations().into_iter()
    // }
}

#[allow(clippy::match_same_arms, clippy::too_many_lines)]
pub fn segments_from_quest_tile(pos: IVec2, quest_tile: &QuestTile) -> Vec<Segment> {
    let segments = match quest_tile.quest_tile_id.0 {
        // Wheat
        // 2AA_4AF (Normal,  BigTree, Granary, Windmill)
        2 | 3 | 4 | 5 => vec![
            (Form::Size2, Terrain::Wheat, 5),
            (Form::Size4, Terrain::Forest, 1),
        ],
        // 2AA
        92 => vec![(Form::Size2, Terrain::Wheat, 0)],
        // 2AA_2AV_1AV TODO?
        1 => vec![
            (Form::Size2, Terrain::Wheat, 0),
            (Form::Size2, Terrain::House, 2),
            (Form::Size1, Terrain::House, 5),
        ],
        // 3AA_1AV (Normal, Granary, Windmill)
        6 | 7 | 8 => vec![
            (Form::Size3, Terrain::Wheat, 3),
            (Form::Size1, Terrain::House, 0),
        ],
        // 4AA_2AF (Normal, Granary)
        9 | 10 => vec![
            (Form::Size4, Terrain::Wheat, 0),
            (Form::Size2, Terrain::Forest, 4),
        ],
        // 4BA_1AF_1AF (Normal, BigTree)
        11 | 12 => vec![
            (Form::X, Terrain::Wheat, 0),
            (Form::Size1, Terrain::Forest, 2),
            (Form::Size1, Terrain::Forest, 5),
        ],
        // 6AA (Normal, BigTree, Windmill)
        13 | 14 | 15 => vec![(Form::Size6, Terrain::Wheat, 0)],

        // Forest
        // 1AF (Normal, Dear, Bear, Boar)
        16 | 19 | 65 | 66 => vec![(Form::Size1, Terrain::Forest, 0)],
        // 2AF (Normal, Dear, Bear, Boar)
        67 | 68 | 69 | 70 => vec![(Form::Size2, Terrain::Forest, 0)],
        // 3AF (Normal, Dear, Bear, Boar)
        20 | 21 | 71 | 72 => vec![(Form::Size3, Terrain::Forest, 0)],
        // 4AF (Normal, Ruin)
        22 | 73 => vec![(Form::Size4, Terrain::Forest, 0)],
        // 6AF (Normal, Deer, Bear, Boar, Ruin)
        23 | 24 | 74 | 75 | 76 => vec![(Form::Size6, Terrain::Forest, 0)],
        // 1AF_2AW (Normal, Deer) TODO?
        17 | 18 => vec![
            (Form::Size1, Terrain::Forest, 3),
            (Form::Size2, Terrain::Lake, 0),
        ],

        // Village
        // 2AV
        33 => vec![(Form::Size2, Terrain::House, 0)],
        // 3AV (Normal, Fountain)
        35 | 38 => vec![(Form::Size3, Terrain::House, 0)],
        // 3AV_3AF (Normal, Fountain, Tower, Fox)
        34 | 36 | 37 | 80 => vec![
            (Form::Size3, Terrain::House, 0),
            (Form::Size3, Terrain::Forest, 3),
        ],
        // 4BV_1AF_1AF (Notmal, Fountain, Tower, Fox)
        39 | 40 | 41 | 84 => vec![
            (Form::FanOut, Terrain::House, 4),
            (Form::Size1, Terrain::Forest, 1),
            (Form::Size1, Terrain::Forest, 3),
        ],
        // 4AV_1AF_1AF
        // 81 => todo!(),
        // 4AV_2AA (Normal, Fox)
        // 82 | 83 => todo!(),
        // 5AV_1AF (Normal, Fox)
        85 | 86 => vec![
            (Form::Size5, Terrain::House, 0),
            (Form::Size1, Terrain::Forest, 5),
        ],
        // 6AV (Normal, Fountain, Tower)
        42 | 43 | 44 => vec![(Form::Size6, Terrain::House, 0)],

        // Train
        // 2BT_3AA_1AA is this a bug? it says 3AA, but the tile only has size2 agriculture.
        // BUG!
        25 => vec![
            (Form::Bridge, Terrain::Rail, 0),
            (Form::Size1, Terrain::Wheat, 1),
            (Form::Size2, Terrain::Wheat, 4),
        ],
        // 2BT_3AF_1AF
        26 => vec![
            (Form::Bridge, Terrain::Rail, 0),
            (Form::Size1, Terrain::Forest, 1),
            (Form::Size3, Terrain::Forest, 3),
        ],
        // 2BT_3AV_1AV
        27 => vec![
            (Form::Bridge, Terrain::Rail, 0),
            (Form::Size1, Terrain::House, 1),
            (Form::Size3, Terrain::House, 3),
        ],
        // 2CT_1AF_1AV (Normal, Locomotive)
        28 | 29 => vec![
            (Form::Straight, Terrain::Rail, 0),
            (Form::Size1, Terrain::Forest, 1),
            (Form::Size1, Terrain::House, 4),
        ],
        // 2CT (Normal, Locomotive)
        30 | 31 => vec![(Form::Straight, Terrain::Rail, 0)],
        // 32 => todo!(), // 4CT_1AF_1AF

        // Water
        // 2BW_3AF_1AF (Normal, Boat) TODO?
        45 | 46 => vec![
            (Form::Bridge, Terrain::River, 0),
            (Form::Size1, Terrain::Forest, 1),
            (Form::Size3, Terrain::Forest, 3),
        ],
        // 2CW (Normal, Boat, Beaver)
        47 | 54 | 58 => vec![(Form::Straight, Terrain::River, 0)],

        // 2CW_2AA_1AV (Normal, Watermill) // Very weird tile...
        49 | 50 => vec![
            (Form::Straight, Terrain::River, 0),
            (Form::Size2, Terrain::Wheat, 1),
            (Form::Size1, Terrain::House, 5),
        ],
        // 2CW_2AF_1AA (Normal, Watermill)
        51 | 52 => vec![
            (Form::Straight, Terrain::River, 0),
            (Form::Size2, Terrain::Forest, 1),
            (Form::Size1, Terrain::Wheat, 4),
        ],
        // 2CW_2AF_2AA (Normal, Beaver)
        87 | 88 => vec![
            (Form::Straight, Terrain::River, 0),
            (Form::Size2, Terrain::Forest, 1),
            (Form::Size2, Terrain::Wheat, 4),
        ],
        // 2CW_2AV_1AV
        48 => vec![
            (Form::Straight, Terrain::River, 0),
            (Form::Size2, Terrain::House, 1),
            (Form::Size1, Terrain::House, 5),
        ],
        // 2CW_2AV_2AV_Watermill TODO? more weird tiles....
        53 => vec![
            (Form::Straight, Terrain::River, 0),
            (Form::Size2, Terrain::House, 1),
            (Form::Size2, Terrain::House, 4),
        ],
        // 3AW_3AF (Normal, SwanGoose, Beaver)
        59 | 89 | 60 => vec![
            (Form::Size3, Terrain::Lake, 0),
            (Form::Size3, Terrain::Forest, 3),
        ],

        // 4AW_2AF (Normal, Beaver, SwanGoose)
        61 | 62 | 90 => vec![
            (Form::Size4, Terrain::Lake, 0),
            (Form::Size2, Terrain::Forest, 4),
        ],
        // 6AW (Normal, Beaver, Ruin, Boat, SwanGoose)
        55 | 63 | 64 | 56 | 91 => vec![(Form::Size6, Terrain::Lake, 0)],

        // WaterTrainStation
        // 6AW_6AT
        57 => vec![(Form::Size6, Terrain::Station, 0)],

        // Tutorial
        // 77 => todo!(), // Tutorial_Agriculture_6AA
        // 78 => todo!(), // Tutorial_Agriculture_6AA_Windmill
        // 79 => todo!(), // Tutorial_Village_2AV

        // 0 => todo!(), // Undefined
        other => {
            println!("{}\t{}\t=> {}", pos.x, pos.y, other);
            todo!();
        }
    };

    segments
        .into_iter()
        .map(|(form, terrain, rotation)| Segment {
            pos,
            form,
            terrain,
            rotation,
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
            }]
        }
        _ => {
            println!("{}\t{}\t=> {}", pos.x, pos.y, special_tile_id.0);
            todo!();
        }
    }
}

// TODO
// // Compute parts.
// let parts = [0, 1, 2, 3, 4, 5].map(|rotation| {
//     segments
//         .iter()
//         .find(|segment| segment.rotations().into_iter().any(|r| r == rotation))
//         .map_or(Terrain::Empty, |segment| segment.terrain)
// });

//
// impl Tile {
//     pub fn opposite_side(rotation: Rotation) -> Rotation {
//         (rotation + 3) % 6
//     }
//
//     pub fn neighbor_pos(&self, rotation: Rotation) -> IVec2 {
//         Tile::neighbor_pos_of(self.pos, rotation)
//     }
//
//     pub fn segment(&self, id: SegmentId) -> Option<&Segment> {
//         self.segments.get(id)
//     }
//
//     /// There can be multiple segments at a single rotation due to the station tile.
//     pub fn segments_at(&self, rotation: Rotation) -> impl Iterator<Item = (SegmentId, &Segment)> {
//         self.segments
//             .iter()
//             .enumerate()
//             .filter(move |(_, segment)| segment.rotations().into_iter().any(|r| r == rotation))
//     }
//
//     /// There can only be one segment that connects to `terrain`.
//     /// Station tiles uphold that rule.
//     pub fn connecting_segment_at(
//         &self,
//         terrain: Terrain,
//         rotation: Rotation,
//     ) -> Option<(SegmentId, &Segment)> {
//         self.segments_at(rotation)
//             .find(|(_, segment)| segment.terrain.connects_to_group_of(terrain))
//     }
//
//     /// Get a tile as if moved to `pos` and rotated by `rotation`.
//     pub fn moved_to(&self, pos: IVec2, rotation: Rotation) -> Self {
//         Self {
//             pos,
//             segments: self
//                 .segments
//                 .iter()
//                 .map(|segment| Segment {
//                     rotation: (segment.rotation + rotation) % 6,
//                     ..*segment
//                 })
//                 .collect(),
//             parts: [0, 1, 2, 3, 4, 5].map(|index| self.parts[(index + rotation) % 6]),
//             quest_tile: self.quest_tile,
//         }
//     }
//
//     pub fn canonical_id(&self) -> u32 {
//         (0..6)
//             .map(|rotation| {
//                 self.parts[rotation..]
//                     .iter()
//                     .chain(self.parts[0..rotation].iter())
//                     .fold(0, |accum, part| (accum << 4) | *part as u32)
//             })
//             .min()
//             .unwrap()
//     }
//
//     /// Checks every rotation!
//     pub fn is_perfect_placement(inner: &[Terrain; 6], outer: &[Terrain; 6]) -> bool {
//         for rotation_offset in 0..6 {
//             let offset_inner = inner[rotation_offset..]
//                 .iter()
//                 .chain(inner[0..rotation_offset].iter());
//
//             if offset_inner
//                 .zip(outer.iter())
//                 .all(|(i, o)| i.neighbor_score(*o) >= 0)
//             {
//                 return true;
//             }
//         }
//         return false;
//     }
// }
