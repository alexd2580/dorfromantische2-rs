use crate::{
    best_placements::BestPlacements,
    data::{Terrain, IVEC2_, IVEC4_},
    group_assignments::GroupAssignments,
    map::Map,
};

const TILE_: usize = IVEC4_ + IVEC2_ + IVEC2_;

#[allow(clippy::similar_names, clippy::cast_ptr_alignment)]
pub unsafe fn write_map_to(
    ptr: *mut u8,
    map: &Map,
    groups: &GroupAssignments,
    best_placements: &BestPlacements,
) {
    let iptr = ptr.cast::<i32>();

    *iptr.add(0) = map.index_offset.x;
    *iptr.add(1) = map.index_offset.y;
    *iptr.add(2) = map.index_size.x;
    *iptr.add(3) = map.index_size.y;

    let bptr = iptr.add(4).cast::<u8>();
    for (index, maybe_segments) in map.tile_index.iter().enumerate() {
        let tptr = bptr.add(index * TILE_).cast::<u32>();

        if let &Some((base_index, segment_count)) = maybe_segments {
            // Tile exists.
            for nth_segment in 0..segment_count {
                let segment_index = base_index + nth_segment;
                let segment = &map.segments[segment_index];
                let group_index = groups.assigned_groups[segment_index];
                let group = &groups.groups[group_index];
                let is_closed = if group.is_closed() { 1 } else { 0 };

                // Each segment is a uint32.
                *tptr.add(nth_segment) = segment.terrain as u32
                    | (segment.form as u32) << 4
                    | (segment.rotation as u32) << 9
                    | is_closed << 12
                    | (group_index as u32) << 13;
            }
            if segment_count < 6 {
                *tptr.add(segment_count) = Terrain::Empty as u32;
            }
        } else {
            // Tile doesn't exist.
            *tptr = Terrain::Missing as u32;
            *tptr.add(6) = 0;
        }
    }

    for (rank, score) in best_placements.iter_usable() {
        let tile_index = map.tile_key(score.pos).unwrap();
        let tptr = bptr.add(tile_index * TILE_).cast::<u32>();
        *tptr.add(6) = rank as u32;
        // *tptr.add(7).cast::<f32>() = *probability_score as f32 / self.tiles.len() as f32;
    }
}

pub fn byte_size_for_n_tiles(num_tiles: usize) -> usize {
    // Offset + Size + Tiles (at least one...)
    1 * IVEC2_ + 1 * IVEC2_ + num_tiles.max(1) * TILE_
}

pub fn byte_size(map: &Map) -> usize {
    byte_size_for_n_tiles(map.tile_index.len())
}
