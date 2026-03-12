use std::collections::HashSet;

use glam::Vec2;

use crate::{
    data::{Pos, Segment},
    map::{Quest, SegmentIndex},
};

/// Index into the groups array, identifying a connected terrain group.
pub type GroupIndex = usize;

pub struct Group {
    pub terrain: crate::data::Terrain,
    pub segment_indices: HashSet<SegmentIndex>,
    pub open_edges: HashSet<Pos>,
    /// Quests that target this group (placed on tiles belonging to this group).
    pub quests: Vec<Quest>,
    /// Total unit count (houses, trees, fields, etc.) across all segments in this group.
    pub unit_count: u32,
    /// Centroid of unique tile positions in this group (in hex coordinates as f32).
    pub centroid: Vec2,
}

impl Group {
    pub fn is_closed(&self) -> bool {
        self.open_edges.is_empty()
    }

    /// Compute the centroid of unique tile positions in the group (in hex coordinates).
    pub fn compute_centroid(segment_indices: &HashSet<SegmentIndex>, segments: &[Segment]) -> Vec2 {
        let mut positions = HashSet::new();
        for &i in segment_indices {
            positions.insert(segments[i].pos);
        }
        let count = positions.len() as f32;
        let sum: Vec2 = positions
            .iter()
            .map(|p| Vec2::new(p.x as f32, p.y as f32))
            .sum();
        sum / count
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
