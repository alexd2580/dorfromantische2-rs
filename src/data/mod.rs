mod edge_profile;
mod form;
mod group_kind;
mod segment;
mod side;
mod terrain;
mod tile_table;

pub use edge_profile::EdgeProfile;
pub use form::Form;
pub use group_kind::GroupKind;
pub use segment::Segment;
pub use side::Side;
pub use terrain::{EdgeMatch, Terrain};
pub use tile_table::{quest_terrain, segments_from_quest_tile, segments_from_special_tile_id};

pub use crate::coords::HexPos;

/// Number of sides on a hexagonal tile.
pub const HEX_SIDES: usize = 6;

/// Hex tile rotation index (0..HEX_SIDES). 0 = north, increasing clockwise.
pub type Rotation = usize;

/// (form, terrain, rotation, unit_count)
pub(crate) type SegmentDef = (Form, Terrain, usize, u32);
