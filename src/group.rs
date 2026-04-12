use std::collections::HashSet;

use glam::Vec2;

use crate::{
    coords::WorldPos,
    data::{GroupKind, HexPos, Segment},
    map::{Quest, SegmentIndex},
};

/// Index into the groups array, identifying a connected terrain group.
pub type GroupIndex = usize;

pub struct Group {
    pub kind: GroupKind,
    /// Kept for backward compatibility with shader/render code.
    pub terrain: crate::data::Terrain,
    pub segment_indices: HashSet<SegmentIndex>,
    pub open_edges: HashSet<HexPos>,
    /// Quests that target this group (placed on tiles belonging to this group).
    pub quests: Vec<Quest>,
    /// Total unit count (houses, trees, fields, etc.) across all segments in this group.
    pub unit_count: u32,
    /// Centroid of unique tile positions in this group (in world coordinates).
    pub centroid: WorldPos,
    /// Max distance from centroid to any tile position (in world coordinates).
    pub radius: f32,
}

impl Group {
    pub fn is_closed(&self) -> bool {
        self.open_edges.is_empty()
    }

    /// Compute the centroid of unique tile positions in the group (in world coordinates).
    pub fn compute_centroid(
        segment_indices: &HashSet<SegmentIndex>,
        segments: &[Segment],
    ) -> WorldPos {
        let mut positions = HashSet::new();
        for &i in segment_indices {
            positions.insert(segments[i].pos);
        }
        let count = positions.len() as f32;
        let sum: Vec2 = positions
            .iter()
            .map(|p| crate::hex::hex_to_world(*p).0)
            .sum();
        WorldPos(sum / count)
    }

    /// Compute max distance from centroid to any tile in the group (in world coordinates).
    pub fn compute_radius(
        centroid: WorldPos,
        segment_indices: &HashSet<SegmentIndex>,
        segments: &[Segment],
    ) -> f32 {
        let mut max_dist_sq: f32 = 0.0;
        for &i in segment_indices {
            let p = crate::hex::hex_to_world(segments[i].pos).0;
            max_dist_sq = max_dist_sq.max(p.distance_squared(centroid.0));
        }
        max_dist_sq.sqrt() + 1.5 // +1.5 to cover the tile itself (hex radius in world coords)
    }

    /// Compute total units from the group's segments.
    pub fn compute_unit_count(
        segment_indices: &HashSet<SegmentIndex>,
        segments: &[Segment],
    ) -> u32 {
        segment_indices
            .iter()
            .map(|&i| segments[i].unit_count)
            .sum()
    }

    /// How many units remain to fulfill each quest. Negative means already exceeded.
    pub fn remaining_per_quest(&self) -> Vec<(&Quest, i32)> {
        self.quests
            .iter()
            .map(|q| (q, q.target_value - self.unit_count as i32))
            .collect()
    }
}
