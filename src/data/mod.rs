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

use glam::IVec2;

/// Number of sides on a hexagonal tile.
pub const HEX_SIDES: usize = 6;

pub const INT_: usize = 4;
pub const IVEC2_: usize = 2 * INT_;
pub const IVEC4_: usize = 4 * INT_;

/// Use flat-top axial coordinates.
/// x -> 2 o'clock
/// y -> north
/// Offset coordinates are stupid and complex.
pub type Pos = IVec2;

/// Hex tile rotation index (0..HEX_SIDES). 0 = north, increasing clockwise.
pub type Rotation = usize;

/// (form, terrain, rotation, unit_count)
pub(crate) type SegmentDef = (Form, Terrain, usize, u32);
