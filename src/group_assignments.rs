use std::collections::{HashSet, VecDeque};

use crate::{
    data::{Pos, Terrain},
    group::{Group, GroupIndex},
    map::{Map, SegmentIndex},
};

pub struct GroupAnalyzer<'a> {
    /// Reference to the used map.
    map: &'a Map,

    /// List of valid tile positions to be processed.
    pos_queue: VecDeque<Pos>,
    /// All tiles that have been discovered.
    discovered_pos: HashSet<Pos>,

    segment_queue: Vec<SegmentIndex>,
    discovered_segments: HashSet<(SegmentIndex, Terrain)>,

    pub possible_placements: HashSet<Pos>,
    pub groups: Vec<Group>,
    pub assigned_groups: Vec<GroupIndex>,
}

impl<'a> GroupAnalyzer<'a> {
    /// Register a new position for either discovery or as a possible placement.
    /// Returns whether the tile exists in the map.
    fn handle_new_pos(&mut self, pos: Pos) -> bool {
        if self.is_discovered(pos) {
            return true;
        }

        if self.map.has(pos) {
            // If it exists, then queue it.
            self.pos_queue.push_back(pos);
            self.discovered_pos.insert(pos);
            true
        } else {
            // If it doesn't, then it's a possible placement.
            self.possible_placements.insert(pos);
            false
        }
    }

    fn discover_groups_of_segment(&mut self, segment_index: SegmentIndex) {
        match self.map.segment(segment_index).terrain {
            Terrain::Lake => self.discover_group(segment_index, Terrain::River),
            Terrain::Station => {
                self.discover_group(segment_index, Terrain::River);
                self.discover_group(segment_index, Terrain::Rail);
            }
            a => self.discover_group(segment_index, a),
        }
    }

    fn discover_group(&mut self, segment_index: SegmentIndex, group_terrain: Terrain) {
        // Ignore if already processed.
        if self
            .discovered_segments
            .contains(&(segment_index, group_terrain))
        {
            return;
        }

        // Collect all members belonging to the same group as `segment`.
        let mut segment_indices = HashSet::new();
        let mut open_edges = HashSet::new();

        // Start the
        self.segment_queue.push(segment_index);
        self.discovered_segments
            .insert((segment_index, group_terrain));
        while !self.segment_queue.is_empty() {
            let segment_index = self.segment_queue.pop().unwrap();
            segment_indices.insert(segment_index);

            let segment = self.map.segment(segment_index);
            let connecting_rotations = segment.rotations();

            for rotation in 0..6 {
                let neighbor_pos = Map::neighbor_pos_of(segment.pos, rotation);
                let neighbor_exists = self.handle_new_pos(neighbor_pos);

                // If the rotation does not extend that way, then ignore.
                let edge_is_relevant = connecting_rotations.contains(&rotation);
                if !edge_is_relevant {
                    continue;
                }

                if !neighbor_exists {
                    open_edges.insert(neighbor_pos);
                    continue;
                }

                // Neighbor exists and it's relevant (pos is already registered).
                let back_rotation = (rotation + 3) % 6;
                self.map
                    .segment_indices_at(neighbor_pos)
                    .unwrap()
                    .for_each(|index| {
                        if self.discovered_segments.contains(&(index, group_terrain)) {
                            return;
                        }
                        let neighbor = self.map.segment(index);

                        if neighbor.rotations().contains(&back_rotation)
                            && neighbor.terrain.extends_group_of(group_terrain)
                        {
                            self.discovered_segments.insert((index, group_terrain));
                            self.segment_queue.push(index);
                        }
                    });
            }
        }
        self.groups.push(Group {
            segment_indices,
            open_edges,
        });
    }

    fn is_discovered(&self, pos: Pos) -> bool {
        self.discovered_pos.contains(&pos)
    }

    pub fn run(&mut self) {
        // Discovery queue, breadth first.
        while !self.pos_queue.is_empty() {
            let pos = self.pos_queue.pop_front().unwrap();
            let segment_indices = self.map.segment_indices_at(pos).unwrap();

            // If we don't discover new tiles via the group-discovery, then we need to manually do
            // so. This case happens when there are no segments on a tile, so it won't be used in
            // `discover_group`.
            if segment_indices.is_empty() {
                // Add all yet undiscovered neighbors to the queue.
                for rotation in 0..6 {
                    let neighbor_pos = Map::neighbor_pos_of(pos, rotation);
                    self.handle_new_pos(neighbor_pos);
                }
            } else {
                // Discover the groups of the segments on this tile.
                segment_indices
                    // Discover all other segments.
                    .for_each(|index| self.discover_groups_of_segment(index));
            }
        }

        self.assigned_groups
            .resize(self.discovered_segments.len(), usize::MAX);
        for (group_index, group) in self.groups.iter().enumerate() {
            for segment_index in &group.segment_indices {
                self.assigned_groups[*segment_index] = group_index;
            }
        }
    }
}

#[derive(Default)]
pub struct GroupAssignments {
    /// Positions where tiles can be placed.
    pub possible_placements: HashSet<Pos>,
    /// List of groups, with each being a set of segments.
    pub groups: Vec<Group>,
    /// Mapping of segment index to group index.
    pub assigned_groups: Vec<GroupIndex>,
}

impl From<&Map> for GroupAssignments {
    fn from(map: &Map) -> Self {
        let mut analyzer = GroupAnalyzer {
            map,
            pos_queue: VecDeque::from([Pos::new(0, 0)]),
            discovered_pos: HashSet::from([Pos::new(0, 0)]),

            segment_queue: Vec::default(),
            discovered_segments: HashSet::default(),

            possible_placements: HashSet::default(),
            groups: Vec::default(),
            assigned_groups: Vec::default(),
        };
        analyzer.run();
        Self {
            possible_placements: analyzer.possible_placements,
            groups: analyzer.groups,
            assigned_groups: analyzer.assigned_groups,
        }
    }
}

impl GroupAssignments {
    pub fn group_of(&self, segment_index: SegmentIndex) -> Option<GroupIndex> {
        let group_index = self.assigned_groups[segment_index];
        if group_index == usize::MAX {
            None
        } else {
            Some(group_index)
        }
    }
}
