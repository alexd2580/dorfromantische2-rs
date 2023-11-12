use glam::IVec2;

use crate::data;

pub const PAD_: usize = 4;
pub const _BOOL_: usize = 4;
pub const INT_: usize = 4;
pub const FLOAT_: usize = 4;
pub const VEC2_: usize = 2 * FLOAT_;
pub const _VEC3_: usize = 3 * FLOAT_ + PAD_;
pub const IVEC2_: usize = 2 * INT_;
pub const IVEC4_: usize = 4 * INT_;

pub const TILE_: usize = 6 * IVEC4_;

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

impl From<&data::SegmentTypeId> for Form {
    fn from(value: &data::SegmentTypeId) -> Self {
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

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum Terrain {
    Missing = -1,
    Empty = 0,
    House = 1,
    Forest = 2,
    Wheat = 3,
    Rail = 4,
    River = 5,
    Lake = 6,
    RailStation = 7,
    LakeStation = 8,
}

impl Terrain {
    pub fn connects_to(&self, terrain: Terrain) -> bool {
        if self == &terrain {
            return true;
        }
        match (*self, terrain) {
            (Terrain::Lake, Terrain::River) => true,
            (Terrain::Lake, Terrain::LakeStation) => true,
            (Terrain::River, Terrain::Lake) => true,
            (Terrain::River, Terrain::LakeStation) => true,
            (Terrain::LakeStation, Terrain::Lake) => true,
            (Terrain::LakeStation, Terrain::River) => true,
            (Terrain::Rail, Terrain::RailStation) => true,
            (Terrain::RailStation, Terrain::Rail) => true,
            (Terrain::Empty, _) => false,
            (Terrain::Missing, _) => false,
            (a, b) => a == b,
        }
    }
}

impl From<&data::GroupTypeId> for Terrain {
    fn from(value: &data::GroupTypeId) -> Self {
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

#[derive(Debug)]
pub struct Segment {
    pub form: Form,
    pub terrain: Terrain,
    pub rotation: i32,
    pub group: usize,
}

impl From<&data::Segment> for Segment {
    fn from(value: &data::Segment) -> Self {
        let mut form = (&value.segment_type).into();
        let mut terrain = (&value.group_type).into();
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
        Self {
            form,
            terrain,
            rotation: value.rotation,
            group: 0,
        }
    }
}

impl Segment {
    pub fn rotations(&self) -> Vec<i32> {
        let local_rotations = match self.form {
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
        };
        local_rotations
            .iter()
            .map(|local| (self.rotation + local) % 6)
            .collect()
    }
}

#[derive(Debug)]
pub struct Tile {
    /// Use flat-top axial coordinates.
    /// x -> 2 o'cock
    /// y -> north
    /// Offset coordinates are stupid and complex.
    pub pos: IVec2,
    pub segments: Vec<Segment>,
}

impl From<&data::Tile> for Tile {
    fn from(value: &data::Tile) -> Self {
        let special = value.special_tile_id.0;
        let mut segments = value
            .segments
            .iter()
            .map(|segment| {
                let mut segment = Segment::from(segment);
                segment.rotation += value.rotation;
                segment
            })
            .collect();

        match special {
            0 => {}
            1 => {
                segments = vec![
                    Segment {
                        form: Form::Size6,
                        terrain: Terrain::RailStation,
                        rotation: 0,
                        group: 0,
                    },
                    Segment {
                        form: Form::Size6,
                        terrain: Terrain::LakeStation,
                        rotation: 0,
                        group: 0,
                    },
                ]
            }
            _ => unreachable!(),
        }

        // Hex grid tutorial:
        // https://www.redblobgames.com/grids/hexagons/#line-drawing
        let pos = IVec2::new(value.s, value.t - ((value.s + 1) & -2i32) / 2);
        Self { pos, segments }
    }
}

impl Tile {
    pub fn opposite_side(rotation: i32) -> i32 {
        (rotation + 3) % 6
    }

    pub fn neighbor_coordinates_of(pos: IVec2, rotation: i32) -> IVec2 {
        match rotation {
            0 => pos + IVec2::new(0, 1),
            1 => pos + IVec2::new(1, 0),
            2 => pos + IVec2::new(1, -1),
            3 => pos + IVec2::new(0, -1),
            4 => pos + IVec2::new(-1, 0),
            5 => pos + IVec2::new(-1, 1),
            _ => panic!("Rotation should be 0-5, got {rotation}"),
        }
    }

    pub fn neighbor_coordinates(&self, rotation: i32) -> IVec2 {
        Tile::neighbor_coordinates_of(self.pos, rotation)
    }

    /// There can be multiple segments at a single rotation due to the station tile.
    pub fn segments_at(&self, rotation: i32) -> impl Iterator<Item = &Segment> {
        self.segments
            .iter()
            .filter(move |segment| segment.rotations().into_iter().any(|r| r == rotation))
    }

    pub fn connecting_segment_at(&self, terrain: Terrain, rotation: i32) -> Option<&Segment> {
        self.segments_at(rotation)
            .find(|segment| segment.terrain.connects_to(terrain))
    }
}
