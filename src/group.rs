use std::collections::HashSet;

use crate::{data::Pos, map::SegmentIndex};

pub type GroupIndex = usize;

pub struct Group {
    pub segment_indices: HashSet<SegmentIndex>,
    pub open_edges: HashSet<Pos>,
}

impl Group {
    pub fn size(&self) -> usize {
        self.segment_indices.len()
    }

    pub fn is_closed(&self) -> bool {
        self.open_edges.is_empty()
    }
}
