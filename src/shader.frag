#version 460

#define TERRAIN_MISSING 0
#define TERRAIN_EMPTY 1
#define TERRAIN_VILLAGE 2
#define TERRAIN_FOREST 3
#define TERRAIN_FIELD 4
#define TERRAIN_RAIL 5
#define TERRAIN_RIVER 6
#define TERRAIN_LAKE 7
#define TERRAIN_RAIL_STATION 8
#define TERRAIN_LAKE_STATION 9

#define FORM_SIZE1 0
#define FORM_SIZE2 1
#define FORM_BRIDGE 2 // 1-skip1-1
#define FORM_STRAIGHT 3 // 1-skip2-1
#define FORM_SIZE3 4
#define FORM_JUNCTION_LEFT 5 // 2-skip1-1
#define FORM_JUNCTION_RIGHT 6 // 2-skip2-1
#define FORM_THREE_WAY 7 // 1-skip1-1-skip1-1
#define FORM_SIZE4 8
#define FORM_FAN_OUT 9 // 3-skip1-1
#define FORM_X 10 // 2-skip1-2
#define FORM_SIZE5 11
#define FORM_SIZE6 12

#define FORM_UNKNOWN_102 13
#define FORM_UNKNOWN_105 14
#define FORM_WATER_SIZE4 15
#define FORM_UNKNOWN_111 16

#define COLOR_BY_TERRAIN 0
#define COLOR_BY_GROUP_STATIC 1
#define COLOR_BY_GROUP_DYNAMIC 2
#define COLOR_BY_TEXTURE 3

#define CLOSED_GROUPS_SHOW 0
#define CLOSED_GROUPS_DIM 1
#define CLOSED_GROUPS_HIDE 2

/**
 * For reference on shader block layout see:
 * https://registry.khronos.org/OpenGL/specs/gl/glspec45.core.pdf#page=159
 */
struct Tile {
    /**
     * See `inflate_segment` for reference:
     * - terrain: 0-9 => 4 bit
     * - form: 0-16 => 5 bit
     * - rotation: 0-5 => 3 bit
     * - group is closed: bool => 1 bit
     * - group: => 19 bit
     * Due to an a bug in wgpu/friends which assume that structs are
     * std140-aligned, we need to do packing the ugly way if we want to reduce
     * the memory footprint. This is not particularly bad, because per
     * invocation we do the unpacking only once.
     * `segments_4` holds the first 4 segments, `segments_2` the latter two.
     * See `unpack_segment` for more info.
     */
    uvec4 segments_4;
    uvec2 segments_2;
    int placement_score;
    int pad_;
};

// Segment struct for internal use.
struct Segment {
    // See `TERRAIN_*` defines above.
    uint terrain;
    // See `FORM_*` defines above.
    uint form;
    // Clockwise rotation.
    uint rotation;
    // Whether the group is closed.
    bool group_is_closed;
    // Assigned group.
    uint group;
};

in vec2 uv;

layout(location=0) out vec4 frag_data;

// Push constants have a limit of 0 bytes on my machine.
// Using uniform buffers for compatibility.
// layout(std140) uniform PushConstants {
//   /**
//    * Time in float seconds.
//    */
//   layout(offset=0) float time;
// };

layout(std140, binding=0) uniform View {
    ivec2 size;
    float aspect_ratio;
    float time;

    vec2 origin;
    float rotation;
    float inv_scale;

    ivec2 hover_pos;
    int hover_rotation;
    int pad_;

    int hover_tile;
    int hover_group;
    int section_style;
    int closed_group_style;

    int highlight_hovered_group;
    uint show_placements;
};

layout(std140, binding=1) readonly buffer Tiles {
    ivec2 index_offset;
    ivec2 index_size;
    Tile tiles[];
};

layout(binding=2) uniform sampler texture_sampler;
layout(binding=3) uniform texture2D forest_texture;
layout(binding=4) uniform texture2D city_texture;
layout(binding=5) uniform texture2D wheat_texture;
layout(binding=6) uniform texture2D river_texture;

const float PI = 3.141592653589793;

// PI / 6
const float DEG_30 = PI * 0.166666666;
const float SIN_30 = 0.5;
const float COS_30 = 0.8660254037844387;

vec2 center_coords_of(ivec2 st) {
     return vec2(st.s * 1.5, (st.s + st.t * 2) * COS_30);
}

ivec2 grid_coords_at(vec2 pos) {
    // Calculate tile coords in skewed coordinate grid.
    float x = round(pos.x / 1.5);
    float y_rest = pos.y - x * COS_30;
    float y = round(y_rest / (2 * COS_30));

    ivec2 prelim = ivec2(x, y);
    pos = pos - center_coords_of(prelim);
    float xc = int(round(0.5 * dot(pos, vec2(COS_30, SIN_30)) / COS_30));
    float xyc = int(round(0.5 * dot(pos, vec2(-COS_30, SIN_30)) / COS_30));

    return prelim + ivec2(xc - xyc, xyc);
}

uint unpack_segment(Tile tile, uint segment_id) {
    switch(segment_id) {
        case 0: return tile.segments_4.x;
        case 1: return tile.segments_4.y;
        case 2: return tile.segments_4.z;
        case 3: return tile.segments_4.w;
        case 4: return tile.segments_2.x;
        case 5: return tile.segments_2.y;
    }
    return uint(0);
}

Segment inflate_segment(uint segment) {
    // bits 0-3 // 4 bytes
    uint terrain = segment & 0x0F;
    // bits 4-8 // skip 4 take 5
    uint form = (segment & (0x1F << 4)) >> 4;
    // bits 9-11 // skip 9 take 3
    uint rotation = (segment & (0x07 << 9)) >> 9;
    // bit 12 // skip 12 take 1
    bool group_is_closed = (segment & (0x01 << 12)) != 0;
    // bits 13-32 // skip 13
    uint group = (segment & (0x07FFFF << 13)) >> 13;

    return Segment(terrain, form, rotation, group_is_closed, group);
}

uint tile_id_at(ivec2 st) {
    bool violates_s = st.s < index_offset.s || st.s >= index_offset.s + index_size.s;
    bool violates_t = st.t < index_offset.t || st.t >= index_offset.t + index_size.t;
    if (violates_s || violates_t) {
        return uint(1) << 31;
    }

    return uint((st.t - index_offset.t) * index_size.s + (st.s - index_offset.s));
}

vec3 color_of_group(uint group, float offset) {
    return 0.5 * vec3(sin(group * 0.298347 + offset), cos(group * 0.7834658 + offset), sin(group * 0.123798534 + offset)) + 0.5;
}

vec3 color_of_terrain(uint terrain) {
    switch (terrain) {
    case TERRAIN_EMPTY:
        return vec3(0.2);
    case TERRAIN_VILLAGE:
        return vec3(0.7, 0.4, 0.4);
    case TERRAIN_FOREST:
        return vec3(0.5, 0.3, 0.2);
    case TERRAIN_FIELD:
        return vec3(1, 1, 0);
    case TERRAIN_RAIL:
        return vec3(0.8);
    case TERRAIN_RIVER:
        return vec3(0, 0, 1);
    case TERRAIN_LAKE:
        return vec3(0.2, 0.2, 1);
    case TERRAIN_RAIL_STATION:
    case TERRAIN_LAKE_STATION:
        return vec3(0.5, 0.5, 1);
    default:
        return vec3(1, 0, 1);
    }
}

vec3 color_of_texture(uint terrain, vec2 uv) {
    switch (terrain) {
    case TERRAIN_FOREST:
        return textureLod(sampler2D(forest_texture, texture_sampler), uv, 1.0).xyz;
    case TERRAIN_VILLAGE:
        return textureLod(sampler2D(city_texture, texture_sampler), uv, 1.0).xyz;
    case TERRAIN_FIELD:
        return textureLod(sampler2D(wheat_texture, texture_sampler), uv, 1.0).xyz;
    case TERRAIN_RIVER:
        return textureLod(sampler2D(river_texture, texture_sampler), uv, 1.0).xyz;
    default:
        return color_of_terrain(terrain);
    }
}

float sqr_dist_of(vec2 a, vec2 b) {
    vec2 s = a - b;
    return dot(s, s);
}

bool within(float lower, float value, float upper) {
    return lower <= value && value <= upper;
}

const float single_inner = 0.35 * 0.35;
const float single_outer = 1.15 * 1.15;
const float double_inner = 0.85 * 0.85;
const float triple_inner = 1.85 * 1.85;

bool is_within_form(vec2 pos, uint form) {
    // if (pos.y > abs(pos.x * 2 * cos_30)) {
    //     return true;
    // }
    // return false;

    switch (form) {
        case FORM_SIZE1:
            return sqr_dist_of(pos, vec2(0, COS_30)) < single_inner;
        case FORM_SIZE2:
            return sqr_dist_of(pos, vec2(0.5, COS_30)) < double_inner;
            return pos.y > 0 && pos.y > (-pos.x * 2 * COS_30);
        case FORM_BRIDGE: {
            float sqr_dist = sqr_dist_of(pos, vec2(1.5, COS_30));
            return within(single_outer, sqr_dist, triple_inner);
        }
        case FORM_STRAIGHT:
            return abs(pos.x) < 0.35;
        case FORM_SIZE3:
            return sqr_dist_of(pos, vec2(1.5, COS_30)) < triple_inner;
        case FORM_JUNCTION_LEFT: {
            bool bottom_right = sqr_dist_of(pos, vec2(1.5, -COS_30)) > single_outer;
            return pos.x > -0.35 && bottom_right;
        }
        case FORM_JUNCTION_RIGHT: {
            bool left_side = sqr_dist_of(pos, vec2(-1.5, COS_30)) > single_outer;
            bool bottom_right = dot(vec2(SIN_30, -COS_30), pos) < 0.35;
            return left_side && bottom_right;
        }
        case FORM_THREE_WAY: {
            float sqr_dist_lr = sqr_dist_of(vec2(abs(pos.x), pos.y), vec2(1.5, COS_30));
            float sqr_dist_b = sqr_dist_of(pos, vec2(0, -2 * COS_30));
            return sqr_dist_lr > single_outer && sqr_dist_b > single_outer;
        }
        case FORM_SIZE4:
            return pos.x > -0.35;
        case FORM_FAN_OUT: {
            float sqr_dist_tl = sqr_dist_of(pos, vec2(-1.5, COS_30));
            float sqr_dist_b = sqr_dist_of(pos, vec2(0, -2 * COS_30));
            return sqr_dist_tl > single_outer && sqr_dist_b > single_outer;
        }
        case FORM_X: {
            float sqr_dist_tl = sqr_dist_of(pos, vec2(-1.5, COS_30));
            float sqr_dist_br = sqr_dist_of(pos, vec2(1.5, -COS_30));
            return sqr_dist_tl > single_outer && sqr_dist_br > single_outer;
        }
        case FORM_SIZE5:
            return sqr_dist_of(pos, vec2(-1.5, COS_30)) > single_outer;
        case FORM_SIZE6:
            return true;

        case FORM_UNKNOWN_102:
        case FORM_UNKNOWN_105:
        case FORM_WATER_SIZE4:
        case FORM_UNKNOWN_111:
        default:
            return false;
    }
}

vec3 color_of_score(int score) {
    if ((show_placements & (uint(1) << score)) == 0) {
        return vec3(0);
    }

    float twice_per_sec = 0.5 * sin(2 * 2 * PI * time) + 0.5;
    switch (score) {
        case 6: return twice_per_sec * vec3(0.9, 0.85, 0);
        case 5: return twice_per_sec * vec3(0.85);
        case 4: return twice_per_sec * vec3(0.8, 0.5, 0.2);
        case 3: return twice_per_sec * vec3(0, 1, 0);
        case 2: return twice_per_sec * vec3(0, 0, 1);
        case 1: return twice_per_sec * vec3(1, 0, 0);
        // case 0: return twice_per_sec * vec3(0.5);
    }
    return vec3(0);
}

float group_color_factor(bool is_closed) {
    switch (closed_group_style) {
    case CLOSED_GROUPS_SHOW:
        return 1.0;
    case CLOSED_GROUPS_DIM:
        return is_closed ? 0.1 : 1.0;
    case CLOSED_GROUPS_HIDE:
        return is_closed ? 0.0 : 1.0;
        break;
    }
    return 1.0;
}

void main() {
    vec2 coords = origin + vec2(uv.s * aspect_ratio, uv.t) * 0.5 * inv_scale;
    ivec2 st = grid_coords_at(coords);

    // Load tile info.
    uint tile_id = tile_id_at(st);
    // Tile is outside range. It's empty there.
    if (tile_id == (uint(1) << 31)) {
        frag_data = vec4(0, 0, 0, 1);
        return;
    }

    Tile tile = tiles[tile_id];

    vec2 center = center_coords_of(st);
    vec2 offset = coords - center;

    bool close_to_x_border = dot(abs(offset), vec2(COS_30, SIN_30)) > 0.95 * COS_30;
    bool close_to_y_border = abs(offset.y) > 0.95 * COS_30;
    if (close_to_x_border || close_to_y_border) {
        frag_data = vec4(0, 0, 0, 1);
        return;
    }

    vec3 color = vec3(0.2);
    for (uint segment_id = 0; segment_id < 6; segment_id++) {
        Segment segment = inflate_segment(unpack_segment(tile, segment_id));

        // The segment and the entire tile is empty.
        if (segment.terrain == TERRAIN_MISSING) {
            color = color_of_score(tile.placement_score);
            break;
        }

        // This segment and all following segments are empty.
        if (segment.terrain == TERRAIN_EMPTY) {
            break;
        }

        float angle = segment.rotation * 2 * DEG_30;
        float c = cos(angle);
        float s = sin(angle);
        vec2 pos = vec2(
            c * offset.x - s * offset.y,
            s * offset.x + c * offset.y
        );

        if (is_within_form(pos, segment.form)) {
            switch (section_style) {
            case COLOR_BY_TERRAIN:
                color = color_of_terrain(segment.terrain);
                break;
            case COLOR_BY_GROUP_STATIC:
                color = color_of_group(segment.group, 0.0);
                break;
            case COLOR_BY_GROUP_DYNAMIC:
                color = color_of_group(segment.group, 2 * time);
                break;
            case COLOR_BY_TEXTURE:
                color = color_of_texture(segment.terrain, -0.1 * coords);
                break;
            }

            if (closed_group_style == CLOSED_GROUPS_HIDE && segment.group_is_closed) {
                color = vec3(0.2);
                break;
            }

            float factor = group_color_factor(segment.group_is_closed);

            // bool highlight_hovered = highlight_hovered_group != 0;
            // if (highlight_hovered) {
            //     bool hovered_group_is_visible = factor > 0.01;
            //     bool group_is_currently_hovered = hover_group == segment.group;
            //     if (group_is_currently_hovered && hovered_group_is_visible) {
            //         factor = 1.0;
            //     } else if(
            // }

            color *= factor;
            break;
        }
    }
    // frag_data = vec4(color * (0.5 * sin(time) + 0.5), 1);
    frag_data = vec4(color, 1);
}
