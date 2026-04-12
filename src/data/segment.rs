use crate::raw_data;

use super::{Form, HexPos, Rotation, Terrain, HEX_SIDES};

#[derive(Debug, Clone)]
pub struct Segment {
    pub pos: HexPos,
    pub form: Form,
    pub terrain: Terrain,
    pub rotation: Rotation,
    /// Number of visual units (houses, trees, fields, etc.) in this segment.
    pub unit_count: u32,
}

/// Lake tiles use segment type IDs offset by +100 from their regular equivalents.
fn is_lake_segment_type(id: &raw_data::SegmentTypeId) -> bool {
    matches!(id.0, 102 | 105 | 109 | 111)
}

impl From<(&raw_data::Segment, HexPos, Rotation)> for Segment {
    fn from(value: (&raw_data::Segment, HexPos, Rotation)) -> Self {
        let (raw_segment, pos, tile_rotation) = value;

        let form = (&raw_segment.segment_type).into();
        let mut terrain: Terrain = (&raw_segment.group_type).into();
        // Lake forms and Size6 rivers become Lake terrain.
        if is_lake_segment_type(&raw_segment.segment_type)
            || (form == Form::Size6 && terrain == Terrain::River)
        {
            terrain = Terrain::Lake;
        }

        let raw_rotation: Rotation = raw_segment.rotation.try_into().unwrap_or_else(|_| {
            log::warn!(
                "Invalid segment rotation value {}, defaulting to 0",
                raw_segment.rotation
            );
            0
        });
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
        }
        .iter()
        .map(|local| (self.rotation + local) % HEX_SIDES)
    }

    pub fn contains_rotation(&self, rotation: Rotation) -> bool {
        self.rotations().any(|r| r == rotation)
    }
}
