use std::time::SystemTime;

use bitfield_struct::bitfield;

use crate::{
    best_placements::BestPlacements,
    data::{Terrain, HEX_SIDES, IVEC2_, IVEC4_},
    group_assignments::GroupAssignments,
    map::Map,
    ui::input_state::InputState,
    ui::ui_state::UiState,
};

use super::camera::Camera;
use super::gpu::{Buffer, Gpu};

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
    segments: [PackedSegment; HEX_SIDES],
    placement_rank: i32,
    _pad: u32,
}

/// Write the map data into a GPU buffer at `ptr`.
///
/// # Safety
///
/// `ptr` must point to a buffer of at least `byte_size(map)` bytes,
/// be valid for writes, and be properly aligned for `i32` / `PackedTile`.
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
            if segment_count < HEX_SIDES {
                tile.segments[segment_count].set_terrain(Terrain::Empty as u8);
            }
        } else {
            // Tile doesn't exist.
            tile.segments[0].set_terrain(Terrain::Missing as u8);
            tile.placement_rank = -1;
        }
    }

    for (rank, score) in best_placements.iter_all() {
        if rank >= crate::best_placements::MAX_SHOWN_PLACEMENTS {
            break;
        }
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

pub fn view_buffer_size() -> u64 {
    let size = std::mem::size_of::<PackedView>();
    // std140 requires uniform block size to be a multiple of 16.
    assert!(
        size.is_multiple_of(16),
        "PackedView size {size} is not a multiple of 16 (std140 requirement)"
    );
    size as u64
}

#[repr(C)]
struct PackedView {
    size: (i32, i32),
    aspect_ratio: f32,
    time: f32,

    origin: (f32, f32),
    rotation: f32,
    inv_scale: f32,

    hover_pos: (i32, i32),
    hover_rotation: i32,
    pad_: i32,

    hover_segment: u32,
    hover_group: u32,
    section_style: i32,
    closed_group_style: i32,

    highlight_hovered_group: i32,
    show_placements: u32,

    ghost_pos: (i32, i32),
    ghost_rotation: i32,
    ghost_active: i32,

    /// Padding to align ghost_segments_4 to 16 bytes (std140 uvec4 requirement).
    _pad_align: [i32; 2],
    /// Packed segments of the next tile for ghost rendering.
    /// Layout matches uvec4 ghost_segments_4 + uvec2 ghost_segments_2 in shader.
    ghost_segments_4: (u32, u32, u32, u32),
    ghost_segments_2: (u32, u32),
    _pad2: [i32; 2],
}

#[allow(
    clippy::cast_ptr_alignment,
    clippy::similar_names,
    clippy::too_many_arguments
)]
pub fn write_view(
    buffer: &Buffer,
    gpu: &Gpu,
    camera: &Camera,
    input: &InputState,
    ui_state: &UiState,
    map: &Map,
    best_placements: &BestPlacements,
    program_start: SystemTime,
) {
    let elapsed_secs = SystemTime::now()
        .duration_since(program_start)
        .unwrap()
        .as_secs_f32();

    let mut buffer_view = buffer.write(gpu);
    // SAFETY: The buffer was created with size_of::<PackedView>(), so the cast
    // is within bounds. PackedView is #[repr(C)] ensuring a stable layout.
    unsafe {
        let view = &mut *buffer_view.as_mut_ptr().cast::<PackedView>();
        view.size = (camera.size.x as i32, camera.size.y as i32);
        view.aspect_ratio = camera.aspect_ratio;
        view.time = elapsed_secs;
        view.origin = (camera.origin.x, camera.origin.y);
        view.rotation = camera.rotation;
        view.inv_scale = *camera.inv_scale;
        view.hover_pos = (input.hover_pos.x, input.hover_pos.y);
        view.hover_rotation = input.hover_rotation as i32;
        view.hover_group = input
            .hover_group
            .map_or(u32::MAX, |x| x.try_into().unwrap());
        view.section_style = ui_state.section_style as i32;
        view.closed_group_style = ui_state.closed_group_style as i32;
        view.highlight_hovered_group = i32::from(ui_state.highlight_hovered_group);

        let mut show_score_flags = 0;
        for (index, show) in ui_state.show_placements.iter().enumerate() {
            show_score_flags |= u32::from(*show) << index;
        }
        view.show_placements = show_score_flags;

        // Ghost placement: nearest placement to hover.
        view.ghost_segments_4 = (0, 0, 0, 0);
        view.ghost_segments_2 = (0, 0);
        if ui_state.tooltip_mode == crate::ui::ui_state::TooltipMode::Placement {
            if let Some(score) = best_placements.find_nearest(input.hover_pos, 3) {
                view.ghost_pos = (score.pos.x, score.pos.y);
                view.ghost_rotation = score.rotation as i32;
                view.ghost_active = 1;
                // Pack next tile segments.
                let mut ghost = [0u32; 6];
                for (i, seg) in map.next_tile.iter().enumerate() {
                    if i >= 6 {
                        break;
                    }
                    let mut packed = PackedSegment::new();
                    packed.set_terrain(seg.terrain as u8);
                    packed.set_form(seg.form as u8);
                    let rotated = (seg.rotation + score.rotation) % HEX_SIDES;
                    packed.set_rotation(rotated as u8);
                    packed.set_is_closed(false);
                    packed.set_group_index(0);
                    ghost[i] = packed.into();
                }
                if map.next_tile.len() < 6 {
                    let mut end = PackedSegment::new();
                    end.set_terrain(Terrain::Empty as u8);
                    ghost[map.next_tile.len()] = end.into();
                }
                view.ghost_segments_4 = (ghost[0], ghost[1], ghost[2], ghost[3]);
                view.ghost_segments_2 = (ghost[4], ghost[5]);
            } else {
                view.ghost_pos = (0, 0);
                view.ghost_rotation = 0;
                view.ghost_active = 0;
            }
        } else {
            view.ghost_pos = (0, 0);
            view.ghost_rotation = 0;
            view.ghost_active = 0;
        }
    }
}

pub fn write_tiles(
    buffer: &Buffer,
    gpu: &Gpu,
    map: &Map,
    groups: &GroupAssignments,
    best_placements: &BestPlacements,
) {
    let mut buffer_view = buffer.write(gpu);
    // SAFETY: The buffer was created with shader::byte_size() bytes, which
    // accounts for the header and all tiles. write_map_to writes within those bounds.
    unsafe {
        let ptr = buffer_view.as_mut_ptr();
        write_map_to(ptr, map, groups, best_placements);
    }
}
