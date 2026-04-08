use crate::raw_data;

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

#[derive(PartialEq, Eq, Clone, Copy, Debug, Hash, PartialOrd, Ord)]
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
    #[allow(dead_code, clippy::match_same_arms)]
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
