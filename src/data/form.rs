use crate::raw_data;

use super::Terrain;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
            (Form::JunctionRight, Terrain::Forest) => 20,
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
            // Lake forms map to their regular equivalents (terrain
            // override to Lake is handled in Segment::from).
            102 => Form::Size2,
            105 => Form::Size3,
            109 => Form::Size4,
            111 => Form::Size5,
            other => {
                log::warn!("Unexpected segment type value {other}, defaulting to Size1");
                Form::Size1
            }
        }
    }
}
