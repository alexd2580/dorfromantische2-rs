use crate::data;

pub const _PAD_: usize = 4;
pub const BOOL_: usize = 4;
pub const INT_: usize = 4;
pub const FLOAT_: usize = 4;
pub const VEC2_: usize = 2 * FLOAT_;
pub const _VEC3_: usize = 3 * FLOAT_ + _PAD_;
pub const IVEC2_: usize = 2 * INT_;
pub const IVEC4_: usize = 4 * INT_;

// pub const TILE_: usize = BOOL_ + INT_ + 18 * INT_ + 4 * INT_;
pub const TILE_: usize = 1 * IVEC4_ + 6 * IVEC4_;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Vec2<T> {
    pub s: T,
    pub t: T,
}

impl<T: Default> Default for Vec2<T> {
    fn default() -> Self {
        Self {
            s: Default::default(),
            t: Default::default(),
        }
    }
}

impl<T: std::ops::Add<Output = T>> std::ops::Add for Vec2<T> {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            s: self.s + other.s,
            t: self.t + other.t,
        }
    }
}

impl<T: std::ops::Sub<Output = T>> std::ops::Sub for Vec2<T> {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self {
            s: self.s - other.s,
            t: self.t - other.t,
        }
    }
}

impl<T> Vec2<T> {
    pub fn new(s: T, t: T) -> Self {
        Self { s, t }
    }
}

pub type IVec2 = Vec2<i32>;

#[derive(Clone, Copy)]
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

    Unknown102 = 14,
    Unknown105 = 15,
    WaterSize4 = 16, // wtf?
    Unknown111 = 17,
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
            102 => Form::Unknown102,
            105 => Form::Unknown105,
            109 => Form::WaterSize4,
            111 => Form::Unknown111,
            other => panic!("Unexpected segment type value {other}"),
        }
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Terrain {
    Empty = 0,
    House = 1,
    Forest = 2,
    Wheat = 3,
    Rail = 4,
    Water = 5,
}

impl From<&data::GroupTypeId> for Terrain {
    fn from(value: &data::GroupTypeId) -> Self {
        match value.0 {
            -1 => Terrain::Empty,
            0 => Terrain::House,
            1 => Terrain::Forest,
            2 => Terrain::Wheat,
            3 => Terrain::Rail,
            4 => Terrain::Water,
            other => panic!("Unexpected terrain type value {other}"),
        }
    }
}

pub struct Segment {
    pub form: Form,
    pub terrain: Terrain,
    pub rotation: i32,
    pub group: usize,
}

impl From<&data::Segment> for Segment {
    fn from(value: &data::Segment) -> Self {
        Self {
            form: (&value.segment_type).into(),
            terrain: (&value.group_type).into(),
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
            Form::Unknown102 => &[],
            Form::Unknown105 => &[],
            Form::WaterSize4 => &[0, 1, 2, 3],
            Form::Unknown111 => &[],
        };
        local_rotations
            .iter()
            .map(|local| (self.rotation + local) % 6)
            .collect()
    }
}

pub struct Tile {
    pub pos: IVec2,
    pub special: i32,
    pub segments: Vec<Segment>,
}

impl From<&data::Tile> for Tile {
    fn from(value: &data::Tile) -> Self {
        let segments = value
            .segments
            .iter()
            .map(|segment| {
                let mut segment = Segment::from(segment);
                segment.rotation += value.rotation;
                segment
            })
            .collect();

        Self {
            pos: IVec2 {
                s: value.s,
                t: value.t,
            },
            special: value.special_tile_id.0,
            segments,
        }
    }
}

impl Tile {
    pub fn neighbor_coordinates(&self, rotation: i32) -> IVec2 {
        let s_even = self.pos.s % 2 == 0;
        let b_to_i = |b| if b { 1 } else { 0 };
        match rotation {
            0 => self.pos + IVec2::new(0, 1),
            3 => self.pos + IVec2::new(0, -1),
            1 => self.pos + IVec2::new(1, b_to_i(s_even)),
            2 => self.pos + IVec2::new(1, -b_to_i(!s_even)),
            4 => self.pos + IVec2::new(-1, -b_to_i(!s_even)),
            5 => self.pos + IVec2::new(-1, b_to_i(s_even)),
            _ => panic!("Rotation should be 0-5, got {rotation}"),
        }
    }

    pub fn segment_at(&self, rotation: i32) -> Option<&Segment> {
        self.segments
            .iter()
            .find(|segment| segment.rotations().into_iter().any(|r| r == rotation))
    }
}
