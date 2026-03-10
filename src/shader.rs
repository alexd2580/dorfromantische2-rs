use bitfield_struct::bitfield;

use crate::{
    best_placements::BestPlacements,
    data::{Terrain, IVEC2_, IVEC4_},
    group_assignments::GroupAssignments,
    map::Map,
};

const TILE_: usize = IVEC4_ + IVEC2_ + IVEC2_;

#[bitfield(u32, order=Lsb)]
struct PackedSegment {
    #[bits(4)] // bits 0..3
    pub terrain: u8,
    #[bits(5)]
    pub form: u8,
    #[bits(3)]
    pub rotation: u8,
    #[bits(1)]
    pub is_closed: bool,
    #[bits(19)]
    pub group_index: u32,
}

#[repr(C)]
struct PackedTile {
    segments: [PackedSegment; 6],
    placement_rank: i32,
    _pad: u32,
}

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

    let tiles_ptr = iptr.add(4).cast::<PackedTile>();
    for (index, maybe_segments) in map.tile_index.iter().enumerate() {
        let tile = &mut *tiles_ptr.add(index);

        if let &Some((base_index, segment_count)) = maybe_segments {
            // Tile exists.
            for nth_segment in 0..segment_count {
                let segment_index = base_index + nth_segment;
                let segment = &map.segments[segment_index];
                let group_index = groups.assigned_groups[segment_index];
                let group = &groups.groups[group_index];

                let packed_segment = &mut tile.segments[nth_segment];
                packed_segment.set_terrain(segment.terrain as u8);
                packed_segment.set_form(segment.form as u8);
                packed_segment.set_rotation(segment.rotation as u8);
                packed_segment.set_is_closed(group.is_closed());
                packed_segment.set_group_index(group_index as u32);
            }
            if segment_count < 6 {
                tile.segments[segment_count].set_terrain(Terrain::Empty as u8);
            }
        } else {
            // Tile doesn't exist.
            tile.segments[0].set_terrain(Terrain::Missing as u8);
            tile.placement_rank = -1;
        }
    }

    for (rank, score) in best_placements.iter_usable() {
        let tile_index = map.tile_key(score.pos).unwrap();
        let tile = &mut *tiles_ptr.add(tile_index);
        tile.placement_rank = rank as i32;
    }
}

#[allow(clippy::identity_op)]
// We use `* 1` to be explicitly explicit.
pub fn byte_size_for_n_tiles(num_tiles: usize) -> usize {
    // Offset + Size + Tiles (at least one...)
    1 * IVEC2_ + 1 * IVEC2_ + num_tiles.max(1) * TILE_
}

pub fn byte_size(map: &Map) -> usize {
    byte_size_for_n_tiles(map.tile_index.len())
}
