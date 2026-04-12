use std::{cmp::Ordering, collections::BTreeSet};

use crate::{
    data::{EdgeMatch, HexPos, Rotation, Terrain, HEX_SIDES},
    group::GroupIndex,
    group_assignments::GroupAssignments,
    map::Map,
    tile_frequency::TileFrequencies,
};

pub const MAX_SHOWN_PLACEMENTS: usize = 30;

#[derive(Debug, PartialEq, Eq)]
pub struct GroupEdgeAlteration {
    pub group_size: usize,
    pub diff: i8,
}

/// How a placement affects an active quest on a group.
#[derive(Debug, Clone)]
pub struct QuestEffect {
    pub quest_type: crate::map::QuestType,
    /// Target value the quest requires.
    pub target: i32,
    /// Group segment count before placement.
    pub current_segments: usize,
    /// Group segment count after placement (accounts for merging).
    pub segments_after: usize,
    /// Whether placement would close the group (0 open edges after).
    pub would_close: bool,
}

/// Effect on a group's open edges from a placement.
#[derive(Debug, Clone)]
pub struct GroupEffect {
    pub terrain: Terrain,
    /// Rank among groups of the same terrain (1 = largest).
    pub rank: usize,
    /// Current number of open edges before placement.
    pub open_edges_before: usize,
    /// Change in open edges for this group.
    pub open_edge_delta: i8,
    /// Quest progress info, if any active quest exists on this group.
    pub quest: Option<QuestEffect>,
}

#[derive(Debug)]

pub struct PlacementScore {
    pub pos: HexPos,
    pub rotation: Rotation,
    pub matching_edges: u8,
    /// Bonus for station/lake tiles: count of desirable adjacent terrains.
    pub neighbor_bonus: u8,
    /// How constrained the empty spaces are that Rail/River edges point at.
    pub connection_difficulty: u8,
    /// Rail/River edges from existing tiles pointing at empty neighbors we'd crowd.
    pub crowding: u8,
    /// Effects on groups with >5 tiles.
    pub group_effects: Vec<GroupEffect>,
    pub group_edge_alterations: Vec<GroupEdgeAlteration>,
    /// Probability (0.0-1.0) of finding a tile that fits here based on empirical frequencies.
    pub fit_chance: f32,
    /// Number of unique tile patterns that fit at any rotation.
    pub fit_unique: u16,
    /// How placing here changes the fit chance for each empty neighbor.
    pub neighbor_fit_effects: Vec<NeighborFitEffect>,
}

/// How placing a tile affects the fit chance at one empty neighbor.
#[derive(Debug, Clone)]
pub struct NeighborFitEffect {
    /// Which side of the placement (0-5).
    pub side: usize,
    /// Fit chance at the neighbor position BEFORE placement (current state).
    pub chance_before: f32,
    /// Fit chance at the neighbor position AFTER placement (with new edge constraint).
    pub chance_after: f32,
}

impl PlacementScore {
    fn to_orderable(&self) -> impl Ord {
        // Primary: lower fit_chance = harder to fill = place here first.
        // Convert to fixed-point for Ord (f32 doesn't impl Ord).
        let fit_key = std::cmp::Reverse((self.fit_chance * 1_000_000.0) as u32);
        (
            fit_key,
            self.matching_edges,
            std::cmp::Reverse(self.connection_difficulty),
            std::cmp::Reverse(self.crowding),
            self.neighbor_bonus,
            // Use the following to prevent duplicate removal.
            self.pos.x(),
            self.pos.y(),
        )
    }
}

impl PartialEq for PlacementScore {
    fn eq(&self, other: &Self) -> bool {
        self.to_orderable() == other.to_orderable()
    }
}

impl Eq for PlacementScore {}

impl Ord for PlacementScore {
    fn cmp(&self, other: &Self) -> Ordering {
        self.to_orderable().cmp(&other.to_orderable())
    }
}

impl PartialOrd for PlacementScore {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Default)]
pub struct BestPlacements {
    best_placements: BTreeSet<PlacementScore>,
}

/// Count matching edges for placing the next tile at `pos` with `rotation`.
/// Returns `None` if any edge is illegal (e.g. rail next to river).
fn count_matching_edges(map: &Map, pos: HexPos, rotation: Rotation) -> Option<u8> {
    let mut matching = 0;
    for side in 0..HEX_SIDES {
        let neighbor_pos = Map::neighbor_pos_of(pos, side);
        let other_side = Map::opposite_side(side);
        let neighbor_segments = match map
            .tile_key(neighbor_pos)
            .and_then(|key| map.rendered_tiles[key])
        {
            Some(segments) => segments,
            // No neighbor tile at this side — no constraint.
            None => continue,
        };
        let other_terrain =
            neighbor_segments[other_side].map_or(Terrain::Empty, |idx| map.segments[idx].terrain);

        // Resolve terrains: no segment on a side means Empty.
        let my_terrain = map
            .next_tile
            .iter()
            .find(|seg| seg.contains_rotation((side + HEX_SIDES - rotation) % HEX_SIDES))
            .map_or(Terrain::Empty, |s| s.terrain);

        match my_terrain.connects_and_matches(other_terrain) {
            EdgeMatch::Matching => matching += 1,
            EdgeMatch::Missing => {}
            EdgeMatch::Suboptimal | EdgeMatch::Illegal => return None,
        }
    }
    Some(matching)
}

/// Count how many neighboring edges at `pos` have one of the given terrains.
fn count_neighbor_terrains(map: &Map, pos: HexPos, wanted: &[Terrain]) -> u8 {
    let mut count = 0;
    for side in 0..HEX_SIDES {
        let neighbor_pos = Map::neighbor_pos_of(pos, side);
        let other_side = Map::opposite_side(side);
        let terrain = map
            .tile_key(neighbor_pos)
            .and_then(|key| map.rendered_tiles[key])
            .and_then(|segments| segments[other_side])
            .map(|idx| map.segments[idx].terrain);
        if terrain.is_some_and(|t| wanted.contains(&t)) {
            count += 1;
        }
    }
    count
}

/// Sum of occupied neighbors for each empty space a Rail/River edge points at.
/// Higher = harder to connect later. An edge pointing at open space (few neighbors) is fine;
/// an edge pointing at a nearly-surrounded empty hex is bad.
fn connection_difficulty(map: &Map, pos: HexPos, rotation: Rotation) -> u8 {
    let mut difficulty = 0;
    for side in 0..HEX_SIDES {
        let neighbor_pos = Map::neighbor_pos_of(pos, side);
        let has_neighbor = map
            .tile_key(neighbor_pos)
            .and_then(|key| map.rendered_tiles[key])
            .is_some();
        if has_neighbor {
            continue;
        }
        let my_terrain = map
            .next_tile
            .iter()
            .find(|seg| seg.contains_rotation((side + HEX_SIDES - rotation) % HEX_SIDES))
            .map(|s| s.terrain);
        if matches!(my_terrain, Some(Terrain::Rail | Terrain::River)) {
            // Count how many of the empty neighbor's sides are already occupied
            // (including the tile we're about to place).
            let occupied = (0..HEX_SIDES)
                .filter(|&s| {
                    let p = Map::neighbor_pos_of(neighbor_pos, s);
                    if p == pos {
                        return true; // the tile we're placing
                    }
                    map.tile_key(p)
                        .and_then(|key| map.rendered_tiles[key])
                        .is_some()
                })
                .count() as u8;
            difficulty += occupied;
        }
    }
    difficulty
}

/// Count Rail/River edges from existing tiles that point into empty neighbors of `pos`.
/// Placing our tile next to such a space constrains it further, making it harder to
/// later fill with a tile that connects those rails/rivers.
fn crowding(map: &Map, pos: HexPos) -> u8 {
    let mut total = 0;
    for side in 0..HEX_SIDES {
        let empty_pos = Map::neighbor_pos_of(pos, side);
        // Only consider empty neighbors.
        let is_empty = map
            .tile_key(empty_pos)
            .and_then(|key| map.rendered_tiles[key])
            .is_none();
        if !is_empty {
            continue;
        }
        // Check how many Rail/River edges from other tiles already point at this empty hex.
        for s in 0..HEX_SIDES {
            let p = Map::neighbor_pos_of(empty_pos, s);
            if p == pos {
                continue; // skip the tile we're placing
            }
            let other_side = Map::opposite_side(s);
            let terrain = map
                .tile_key(p)
                .and_then(|key| map.rendered_tiles[key])
                .and_then(|segments| segments[other_side])
                .map(|idx| map.segments[idx].terrain);
            if matches!(terrain, Some(Terrain::Rail | Terrain::River)) {
                total += 1;
            }
        }
    }
    total
}

/// Check whether placing a tile at `pos` would split a contiguous empty region into
/// multiple holes. Walks the 6 neighbors and counts runs of occupied/empty tiles around
/// the hex (wrapping around). More than one run of empty neighbors means the placement
/// creates a split.
fn would_create_split(map: &Map, pos: HexPos) -> bool {
    let occupied: [bool; HEX_SIDES] = std::array::from_fn(|side| {
        let neighbor_pos = Map::neighbor_pos_of(pos, side);
        map.tile_key(neighbor_pos)
            .and_then(|key| map.rendered_tiles[key])
            .is_some()
    });

    // Count the number of contiguous runs of empty neighbors, wrapping around.
    let mut empty_runs = 0;
    for side in 0..HEX_SIDES {
        let prev = (side + HEX_SIDES - 1) % HEX_SIDES;
        // Start of a new empty run: current is empty, previous is occupied (or wraps).
        if !occupied[side] && occupied[prev] {
            empty_runs += 1;
        }
    }
    empty_runs > 1
}

impl BestPlacements {
    /// Test a single placement option.
    fn score_of_next_at(
        map: &Map,
        _groups: &GroupAssignments,
        pos: HexPos,
        rotation: Rotation,
    ) -> Option<PlacementScore> {
        if would_create_split(map, pos) {
            return None;
        }
        let matching_edges = count_matching_edges(map, pos, rotation)?;

        let has_terrain = |t| map.next_tile.iter().any(|s| s.terrain == t);
        let neighbor_bonus = if has_terrain(Terrain::Station) {
            count_neighbor_terrains(map, pos, &[Terrain::Rail, Terrain::River])
        } else if has_terrain(Terrain::Lake) {
            count_neighbor_terrains(map, pos, &[Terrain::River])
        } else {
            0
        };
        let connection_difficulty = connection_difficulty(map, pos, rotation);
        let crowding = crowding(map, pos);
        Some(PlacementScore {
            pos,
            rotation,
            matching_edges,
            connection_difficulty,
            crowding,
            neighbor_bonus,
            group_effects: Vec::new(),
            group_edge_alterations: Vec::new(),
            fit_chance: 0.0,
            fit_unique: 0,
            neighbor_fit_effects: Vec::new(),
        })
    }

    /// Find the placement closest to `pos` within `max_dist` hex distance.
    pub fn find_nearest(&self, pos: HexPos, max_dist: i32) -> Option<&PlacementScore> {
        self.best_placements
            .iter()
            .filter(|s| {
                (s.pos.x() - pos.x()).abs() <= max_dist && (s.pos.y() - pos.y()).abs() <= max_dist
            })
            .min_by_key(|s| (s.pos.x() - pos.x()).pow(2) + (s.pos.y() - pos.y()).pow(2))
    }

    pub fn iter_all(&self) -> Vec<(usize, &PlacementScore)> {
        // Top N by score (stable order from BTreeSet).
        let top: Vec<&PlacementScore> = self
            .best_placements
            .iter()
            .rev()
            .take(MAX_SHOWN_PLACEMENTS)
            .collect();

        // Append any with group_effects not already in top N.
        let mut result = top.clone();
        for score in &self.best_placements {
            if !score.group_effects.is_empty() && !result.iter().any(|s| std::ptr::eq(*s, score)) {
                result.push(score);
            }
        }

        result.into_iter().enumerate().collect()
    }
}

/// A group with >5 tiles, tracked for placement effect computation.
struct LargeGroup {
    group_idx: GroupIndex,
    /// Rank among groups of same terrain (1 = largest by unit count).
    rank: usize,
}

/// Compute effects on open groups from placing next tile at `pos` with `rotation`.
/// Accounts for merging: if multiple groups of the same terrain touch this position,
/// they'd merge into one group.
///
/// Reports the number of unique open-edge POSITIONS (= tiles needed to close) before
/// and after placement. "Before" is the union of all merging groups' open edges.
/// "After" removes the placed position and adds any new open edges from the placed tile.
fn large_group_effects(
    map: &Map,
    groups: &GroupAssignments,
    large_groups: &[LargeGroup],
    pos: HexPos,
    rotation: Rotation,
) -> Vec<GroupEffect> {
    use std::collections::HashMap;
    use std::collections::HashSet;

    let mut by_terrain: HashMap<Terrain, Vec<&LargeGroup>> = HashMap::new();
    for lg in large_groups {
        let group = &groups.groups[lg.group_idx];
        if group.open_edges.contains(&pos) {
            by_terrain.entry(group.terrain).or_default().push(lg);
        }
    }

    let mut effects = Vec::new();
    for (terrain, touching) in &by_terrain {
        let best_rank = touching.iter().map(|lg| lg.rank).min().unwrap_or(1);
        let kind = groups.groups[touching[0].group_idx].kind;

        // Before: union of all merging groups' open edge positions.
        let mut open_before: HashSet<HexPos> = HashSet::new();
        for lg in touching {
            open_before.extend(&groups.groups[lg.group_idx].open_edges);
        }
        let before = open_before.len();

        // After: start from the before set.
        let mut open_after = open_before;

        // The placed position is no longer open.
        open_after.remove(&pos);

        // For each side of the placed tile: check if it creates a new open edge.
        for side in 0..HEX_SIDES {
            let neighbor_pos = Map::neighbor_pos_of(pos, side);

            // Does the placed tile have a matching segment on this side?
            let my_terrain = map
                .next_tile
                .iter()
                .find(|seg| seg.contains_rotation((side + HEX_SIDES - rotation) % HEX_SIDES))
                .map(|s| s.terrain);
            let has_matching_segment = my_terrain.is_some_and(|t| kind.accepts(t));

            if !has_matching_segment {
                continue;
            }

            // If neighbor is occupied, this edge connects to something (not open).
            let neighbor_occupied = map
                .tile_key(neighbor_pos)
                .and_then(|key| map.rendered_tiles[key])
                .is_some();
            if neighbor_occupied {
                // This side connects to an existing tile — the neighbor's facing edge
                // was an open edge of that group, now it's closed.
                // (It's already in open_after from the before set if it was open.)
                // Actually: the neighbor position was open if the NEIGHBOR had an open
                // edge pointing at us. But open_edges tracks the EMPTY positions, not
                // the neighbor tile. Since we're filling `pos`, we already removed it.
                // No further action needed.
                continue;
            }

            // Neighbor is empty and our tile points there: new open edge.
            open_after.insert(neighbor_pos);
        }

        let after = open_after.len();
        let delta = after as i32 - before as i32;

        let merge_label = if touching.len() > 1 {
            // Show the best rank with a merge indicator.
            best_rank
        } else {
            touching[0].rank
        };

        // Compute quest effect if any touching group has an active quest.
        let quest_effect = {
            // Unit count before: union of all merging groups' units.
            let current_segments: usize = touching
                .iter()
                .map(|lg| groups.groups[lg.group_idx].unit_count as usize)
                .sum();

            // Units contributed by the next tile that match this group's terrain.
            let new_segments: usize = map
                .next_tile
                .iter()
                .filter(|seg| kind.accepts(seg.terrain))
                .map(|seg| seg.unit_count as usize)
                .sum();

            let segments_after = current_segments + new_segments;
            let would_close = open_after.is_empty();

            // Collect active quests from all touching groups, pick the one with smallest remaining.
            let mut best_quest: Option<&crate::map::Quest> = None;
            for lg in touching {
                for q in &groups.groups[lg.group_idx].quests {
                    if !q.active {
                        continue;
                    }
                    let remaining = q.target_value - segments_after as i32;
                    match best_quest {
                        None => best_quest = Some(q),
                        Some(prev) => {
                            let prev_remaining = prev.target_value - segments_after as i32;
                            if remaining.abs() < prev_remaining.abs() {
                                best_quest = Some(q);
                            }
                        }
                    }
                }
            }

            best_quest.map(|q| QuestEffect {
                quest_type: q.quest_type,
                target: q.target_value,
                current_segments,
                segments_after,
                would_close,
            })
        };

        effects.push(GroupEffect {
            terrain: *terrain,
            rank: merge_label,
            open_edges_before: before,
            open_edge_delta: delta.clamp(-128, 127) as i8,
            quest: quest_effect,
        });
    }
    effects
}

/// Collect edge constraints at `pos` from the map (occupied neighbors).
pub fn constraints_at(map: &Map, pos: HexPos) -> [Option<Terrain>; HEX_SIDES] {
    let mut constraints: [Option<Terrain>; HEX_SIDES] = [None; HEX_SIDES];
    for (side, constraint) in constraints.iter_mut().enumerate() {
        let npos = Map::neighbor_pos_of(pos, side);
        let other_side = Map::opposite_side(side);
        if let Some(rendered) = map.tile_key(npos).and_then(|key| map.rendered_tiles[key]) {
            let terrain = rendered[other_side]
                .map(|idx| map.segments[idx].terrain)
                .unwrap_or(Terrain::Empty);
            *constraint = Some(terrain);
        }
    }
    constraints
}

/// Count matching edges for a tile profile at constraints.
fn count_matches(
    profile: &crate::data::EdgeProfile,
    constraints: &[Option<Terrain>; HEX_SIDES],
) -> (u8, bool) {
    use crate::data::EdgeMatch;
    let mut matches = 0u8;
    let mut legal = true;
    for (side, c) in constraints.iter().enumerate() {
        if let Some(neighbor) = c {
            match profile.at_index(side).connects_and_matches(*neighbor) {
                EdgeMatch::Matching => matches += 1,
                EdgeMatch::Missing => {}
                EdgeMatch::Suboptimal | EdgeMatch::Illegal => legal = false,
            }
        }
    }
    (matches, legal)
}

/// Compute the chance that a random tile from the frequency table fits given constraints.
/// "Fits" means legal (no Suboptimal/Illegal edges).
/// Returns (chance 0.0-1.0, number of unique fitting patterns).
pub fn fit_chance_for_constraints(
    freqs: &TileFrequencies,
    constraints: &[Option<Terrain>; HEX_SIDES],
) -> (f32, u16) {
    let mut matching_count: usize = 0;
    let mut matching_unique: u16 = 0;

    for entry in &freqs.entries {
        let profile = crate::data::EdgeProfile::from_segments(&entry.segments);
        let mut counted = false;
        for rot in 0..HEX_SIDES {
            let rotated = profile.rotated(rot);
            let (_matches, legal) = count_matches(&rotated, constraints);
            if legal && !counted {
                matching_unique += 1;
                matching_count += entry.count;
                counted = true;
            }
        }
    }

    let chance = if freqs.total_tiles > 0 {
        matching_count as f32 / freqs.total_tiles as f32
    } else {
        0.0
    };
    (chance, matching_unique)
}

/// Compute fit chance at `pos` from current map state.
fn compute_fit_chance(map: &Map, freqs: &TileFrequencies, pos: HexPos) -> (f32, u16) {
    fit_chance_for_constraints(freqs, &constraints_at(map, pos))
}

/// Compute how placing the next tile at `pos` with `rotation` changes the fit chance
/// for each empty neighbor.
fn compute_neighbor_fit_effects(
    map: &Map,
    freqs: &TileFrequencies,
    pos: HexPos,
    rotation: Rotation,
) -> Vec<NeighborFitEffect> {
    let next_profile = crate::data::EdgeProfile::from_segments(&map.next_tile).rotated(rotation);
    let mut effects = Vec::new();

    for side in 0..HEX_SIDES {
        let neighbor_pos = Map::neighbor_pos_of(pos, side);
        // Only care about empty neighbors.
        let neighbor_occupied = map
            .tile_key(neighbor_pos)
            .and_then(|key| map.rendered_tiles[key])
            .is_some();
        if neighbor_occupied {
            continue;
        }

        // Before: current constraints at the neighbor.
        let constraints_before = constraints_at(map, neighbor_pos);
        let (chance_before, _) = fit_chance_for_constraints(freqs, &constraints_before);

        // After: same constraints plus the placed tile's edge on the facing side.
        let facing_side = Map::opposite_side(side);
        let mut constraints_after = constraints_before;
        constraints_after[facing_side] = Some(next_profile.at_index(side));

        let (chance_after, _) = fit_chance_for_constraints(freqs, &constraints_after);

        // Normally adding a constraint can only reduce options.
        // Edge case: neighbor at map boundary may have incomplete constraints.
        let chance_after = chance_after.min(chance_before);

        effects.push(NeighborFitEffect {
            side,
            chance_before,
            chance_after,
        });
    }
    effects
}

impl BestPlacements {
    pub fn compute(map: &Map, groups: &GroupAssignments, freqs: &TileFrequencies) -> Self {
        // Collect all open groups with more than MIN_GROUP_SIZE tiles,
        // ranked per terrain by unit count (1 = largest).
        use std::collections::HashMap;
        let mut per_terrain: HashMap<Terrain, Vec<(usize, u32)>> = HashMap::new();
        for (idx, group) in groups.groups.iter().enumerate() {
            if group.is_closed() {
                continue;
            }
            per_terrain
                .entry(group.terrain)
                .or_default()
                .push((idx, group.unit_count));
        }
        let mut large_groups = Vec::new();
        for (_, mut entries) in per_terrain {
            entries.sort_by(|a, b| b.1.cmp(&a.1));
            for (rank_0, &(group_idx, _)) in entries.iter().enumerate() {
                large_groups.push(LargeGroup {
                    group_idx,
                    rank: rank_0 + 1,
                });
            }
        }

        let mut best_placements = BTreeSet::default();

        for pos in &groups.possible_placements {
            let best_rotation = (0..HEX_SIDES)
                .filter_map(|rotation| {
                    let mut score = BestPlacements::score_of_next_at(map, groups, *pos, rotation)?;
                    score.group_effects =
                        large_group_effects(map, groups, &large_groups, *pos, rotation);
                    Some(score)
                })
                .max();
            if let Some(mut score) = best_rotation {
                let (chance, unique) = compute_fit_chance(map, freqs, *pos);
                score.fit_chance = chance;
                score.fit_unique = unique;
                score.neighbor_fit_effects =
                    compute_neighbor_fit_effects(map, freqs, *pos, score.rotation);
                best_placements.insert(score);
            }
        }

        Self { best_placements }
    }
}
