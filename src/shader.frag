#version 460

#define EMPTY_SEGMENT 0
#define HOUSE_SEGMENT 1
#define FOREST_SEGMENT 2
#define WHEAT_SEGMENT 3
#define RAIL_SEGMENT 4
#define WATER_SEGMENT 5

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

/**
 * For reference on shader block layout see:
 * https://registry.khronos.org/OpenGL/specs/gl/glspec45.core.pdf#page=159
 */
struct Tile {
    /**
     * Determines whether this tile is actually present, or just a placeholder.
     */
    // bool exists;

    /**
     * If this is non-zero, then the tile is a waterlogged train station.
     * Currently there are no other special tiles.
     */
    // int special_id;

    // For now we packed the header into a ivec4 due to alignment issues.
    ivec4 header;

    /**
     * Interleaved;
     * - terrain (enum value); 0 means none
     * - form (enum value)
     * - rotation (int)
     * - group (int)
     */
    ivec4 segments[6];
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
    vec2 origin;
    float rotation;
    float inv_scale;
    float time;
    int coloring;
    int hover_index;
    int pad_;
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
layout(binding=6) uniform texture2D water_texture;

const float pi = 3.141592653589793;

// PI / 6
const float deg_30 = pi * 0.166666666;
const float sin_30 = 0.5; // sin(deg_30);
const float cos_30 = 0.8660254037844387; // cos(deg_30);

const vec2 tile_d = vec2(1.5, 2 * cos_30);

vec2 center_coords_of(ivec2 st) {
     return vec2(st.s * tile_d.x, st.t * tile_d.y - 0.5 * (st.s % 2) * tile_d.y);
}

ivec2 grid_coords_at(vec2 pos) {
    // Calculate tile coords in skewed coordinate grid.
    int diagonal_steps = int(round(pos.x / tile_d.x));
    float vertical_offset = pos.y - diagonal_steps * tile_d.y / 2;
    int vertical_steps = int(round(vertical_offset / tile_d.y));

    // Correct edges, otherwise we'd get offset rectangles and not hexes.
    vec2 tile_center = vec2(diagonal_steps * tile_d.x, vertical_steps * tile_d.y + diagonal_steps * tile_d.y / 2);
    vec2 offset_from_center = pos - tile_center;

    vec2 diagonal_to_top_right = vec2(tile_d.x, tile_d.y / 2) / tile_d.y;
    float offset_to_top_right = dot(offset_from_center, diagonal_to_top_right);

    if (offset_to_top_right > cos_30) {
        diagonal_steps += 1;
    } else if (offset_to_top_right < -cos_30) {
        diagonal_steps -= 1;
    }

    vec2 diagonal_to_top_left = vec2(-tile_d.x, tile_d.y / 2) / tile_d.y;
    float offset_to_top_left = dot(offset_from_center, diagonal_to_top_left);

    if (offset_to_top_left > cos_30) {
        diagonal_steps -= 1;
        vertical_steps += 1;
    } else if (offset_to_top_left < -cos_30) {
        diagonal_steps += 1;
        vertical_steps -= 1;
    }

    // Convert to offset coordinate grid.
    return ivec2(
        diagonal_steps,
        vertical_steps + int(ceil((diagonal_steps - 0.5) / 2))
    );
}

int tile_index(ivec2 st) {
    bool violates_s = st.s < index_offset.s || st.s > index_offset.s + index_size.s;
    bool violates_t = st.t < index_offset.t || st.t > index_offset.t + index_size.t;
    if (violates_s || violates_t) {
        return -1;
    }

    return (st.t - index_offset.t) * index_size.s + (st.s - index_offset.s);
}

vec3 color_of_group(int group, float offset) {
    return 0.5 * vec3(sin(group * 0.298347 + offset), cos(group * 0.7834658 + offset), sin(group * 0.123798534 + offset)) + 0.5;
}

vec3 color_of_terrain(int terrain) {
    switch (terrain) {
    case EMPTY_SEGMENT:
        return vec3(0.2);
    case HOUSE_SEGMENT:
        return vec3(0.7, 0.4, 0.4);
    case FOREST_SEGMENT:
        return vec3(0.4, 0.7, 0.0);
    case WHEAT_SEGMENT:
        return vec3(1, 1, 0);
    case RAIL_SEGMENT:
        return vec3(0.8);
    case WATER_SEGMENT:
        return vec3(0, 0, 1);
    default:
        return vec3(1, 0, 1);
    }
}

vec3 color_of_texture(int terrain, vec2 uv) {
    switch (terrain) {
    case FOREST_SEGMENT:
        return textureLod(sampler2D(forest_texture, texture_sampler), uv, 1.0).xyz;
    case HOUSE_SEGMENT:
        return textureLod(sampler2D(city_texture, texture_sampler), uv, 1.0).xyz;
    case WHEAT_SEGMENT:
        return textureLod(sampler2D(wheat_texture, texture_sampler), uv, 1.0).xyz;
    case WATER_SEGMENT:
        return textureLod(sampler2D(water_texture, texture_sampler), uv, 1.0).xyz;
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

bool is_within_form(vec2 pos, int form) {
    // if (pos.y > abs(pos.x * 2 * cos_30)) {
    //     return true;
    // }
    // return false;

    switch (form) {
        case FORM_SIZE1:
            return sqr_dist_of(pos, vec2(0, cos_30)) < single_inner;
        case FORM_SIZE2:
            return sqr_dist_of(pos, vec2(0.5, cos_30)) < double_inner;
            return pos.y > 0 && pos.y > (-pos.x * 2 * cos_30);
        case FORM_BRIDGE: {
            float sqr_dist = sqr_dist_of(pos, vec2(1.5, cos_30));
            return within(single_outer, sqr_dist, triple_inner);
        }
        case FORM_STRAIGHT:
            return abs(pos.x) < 0.35;
        case FORM_SIZE3:
            return sqr_dist_of(pos, vec2(1.5, cos_30)) < triple_inner;
        case FORM_JUNCTION_LEFT: {
            bool bottom_right = sqr_dist_of(pos, vec2(1.5, -cos_30)) > single_outer;
            return pos.x > -0.35 && bottom_right;
        }
        case FORM_JUNCTION_RIGHT: {
            bool left_side = sqr_dist_of(pos, vec2(-1.5, cos_30)) > single_outer;
            bool bottom_right = dot(vec2(sin_30, -cos_30), pos) < 0.35;
            return left_side && bottom_right;
        }
        case FORM_THREE_WAY: {
            float sqr_dist_lr = sqr_dist_of(vec2(abs(pos.x), pos.y), vec2(1.5, cos_30));
            float sqr_dist_b = sqr_dist_of(pos, vec2(0, -2 * cos_30));
            return sqr_dist_lr > single_outer && sqr_dist_b > single_outer;
        }
        case FORM_SIZE4:
            return pos.x > -0.35;
        case FORM_FAN_OUT:
            return sqr_dist_of(vec2(abs(pos.x), pos.y), vec2(1.5, cos_30)) > single_outer;
        case FORM_X: {
            float sqr_dist_tl = sqr_dist_of(pos, vec2(-1.5, cos_30));
            float sqr_dist_br = sqr_dist_of(pos, vec2(1.5, -cos_30));
            return sqr_dist_tl > single_outer && sqr_dist_br > single_outer;
        }
        case FORM_SIZE5:
            return sqr_dist_of(pos, vec2(-1.5, cos_30)) > single_outer;
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

void main() {
    float aspect_ratio = float(size.s) / size.t;
    vec2 coords = origin + vec2(uv.s * aspect_ratio, uv.t) * 0.5 * inv_scale;
    ivec2 st = grid_coords_at(coords);

    // Load tile info.
    int index = tile_index(st);
    // Tile index is outside range. It's empty there.
    if (index == -1) {
        frag_data = vec4(0, 0, 0, 1);
        return;
    }

    Tile tile = tiles[index];
    // Tile is not really present, just placeholder.
    if (tile.header.x == 0) {
        frag_data = vec4(0, 0, 0, 1);
        return;
    }

    vec2 center = center_coords_of(st);
    vec2 offset = coords - center;

    bool close_to_x_border = dot(abs(offset), vec2(cos_30, sin_30)) > 0.95 * cos_30;
    bool close_to_y_border = abs(offset.y) > 0.95 * cos_30;
    if (close_to_x_border || close_to_y_border) {
        frag_data = vec4(0, 0, 0, 1);
        return;
    }

    vec3 color = vec3(0);
    for (int i = 0; i < 6; i++) {
        int terrain = tile.segments[i].x;

        // The segment (and all following segments!) is empty.
        if (terrain == EMPTY_SEGMENT) {
            color = vec3(0.2);
            break;
        }

        // It's a special tile.... TODO
        if (tile.header.y != 0) {
            color = vec3(0.9);
            break;
        }

        int rotation = tile.segments[i].z;
        float angle = rotation * 2 * deg_30;
        float c = cos(angle);
        float s = sin(angle);
        vec2 pos = vec2(
            c * offset.x - s * offset.y,
            s * offset.x + c * offset.y
        );

        int form = tile.segments[i].y;
        if (is_within_form(pos, form)) {
            switch (coloring) {
            case COLOR_BY_TERRAIN:
                color = color_of_terrain(terrain);
                break;
            case COLOR_BY_GROUP_STATIC:
                color = color_of_group(tile.segments[i].w, 0.0);
                break;
            case COLOR_BY_GROUP_DYNAMIC:
                color = color_of_group(tile.segments[i].w, 2 * time);
                break;
            case COLOR_BY_TEXTURE:
                color = color_of_texture(terrain, -0.1 * coords);
                break;
            }
            break;
        }
    }
    // frag_data = vec4(color * (0.5 * sin(time) + 0.5), 1);
    frag_data = vec4(color, 1);
}
