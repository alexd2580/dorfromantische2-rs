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

layout(std140, binding=0) uniform Sizes {
    ivec4 quadrant_sizes;
};

layout(std140, binding=1) readonly buffer Quadrant0Buffer {
    Tile quadrant0[];
};

layout(std140, binding=2) readonly buffer Quadrant1Buffer {
    Tile quadrant1[];
};

layout(std140, binding=3) readonly buffer Quadrant2Buffer {
    Tile quadrant2[];
};

layout(std140, binding=4) readonly buffer Quadrant3Buffer {
    Tile quadrant3[];
};

layout(std140, binding=5) uniform View {
    ivec2 size;
    vec2 origin;
    float rotation;
    float inv_scale;
    float time;
    int coloring;
};

layout(binding=6) uniform sampler texture_sampler;
layout(binding=7) uniform texture2D forest_texture;
layout(binding=8) uniform texture2D city_texture;
layout(binding=9) uniform texture2D wheat_texture;
layout(binding=10) uniform texture2D water_texture;

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

ivec2 tile_indices(ivec2 st) {
    int quadrant = st.s >= 0 ? (st.t >= 0 ? 0 : 3) : (st.t >= 0 ? 1 : 2);
    int _s = st.s >= 0 ? st.s : (-1 - st.s);
    int _t = st.t >= 0 ? st.t : (-1 - st.t);

    int index = int((_s + _t + 1) * (_s + _t) / 2.0) + _t;

    if (quadrant == 0 && index < quadrant_sizes.x) {
        return ivec2(0, index);
    } else if (quadrant == 1 && index < quadrant_sizes.y) {
        return ivec2(1, index);
    } else if (quadrant == 2 && index < quadrant_sizes.z) {
        return ivec2(2, index);
    } else if (quadrant == 3 && index < quadrant_sizes.w) {
        return ivec2(3, index);
    }
    return ivec2(-1, -1);
}

Tile get_tile(ivec2 indices) {
    if (indices.s == 0) {
        return quadrant0[indices.t];
    } else if (indices.s == 1) {
        return quadrant1[indices.t];
    } else if (indices.s == 2) {
        return quadrant2[indices.t];
    } else { // if (indices.s == 3) {
        return quadrant3[indices.t];
    }
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

#define BRIDGE_MIN_SQR (1.1 * 1.1)
#define BRIDGE_MAX_SQR (1.8 * 1.8)

bool is_within_form(vec2 pos, int form) {
    // if (pos.y > abs(pos.x * 2 * cos_30)) {
    //     return true;
    // }
    // return false;

    switch (form) {
        case FORM_SIZE1:
            return sqr_dist_of(pos, vec2(0, cos_30)) < (0.4 * 0.4);
        case FORM_SIZE2:
            return pos.y > 0 && pos.y > (-pos.x * 2 * cos_30);
        case FORM_BRIDGE: {
            float sqr_dist = sqr_dist_of(pos, vec2(1.5, cos_30));
            return within(BRIDGE_MIN_SQR, sqr_dist, BRIDGE_MAX_SQR);
        }
        case FORM_STRAIGHT:
            return abs(pos.x) < 0.3;
        case FORM_SIZE3:
            return pos.y > (-pos.x * 2 * cos_30);
        case FORM_JUNCTION_LEFT: {
            bool left_side = sqr_dist_of(pos, vec2(-1, 0)) > 1;
            bool bottom_right = sqr_dist_of(pos, 0.5 * vec2(1.5, -cos_30)) > (0.5 * 0.5);
            return left_side && bottom_right;
        }
        case FORM_JUNCTION_RIGHT: {
            bool left_side = sqr_dist_of(pos, 0.5 * vec2(-1.5, cos_30)) > (0.5 * 0.5);
            bool bottom_right = sqr_dist_of(pos, vec2(0.5, -cos_30)) > 1;
            return left_side && bottom_right;
        }
        case FORM_THREE_WAY: {
            float sqr_dist_lr = sqr_dist_of(vec2(abs(pos.x), pos.y), 0.5 * vec2(1.5, cos_30));
            float sqr_dist_b = sqr_dist_of(pos, vec2(0, -cos_30));
            return sqr_dist_lr > (0.4 * 0.4) && sqr_dist_b > (0.4 * 0.4);
        }
        case FORM_SIZE4:
            return abs(pos.y) > (-pos.x * 2 * cos_30);
        case FORM_FAN_OUT:
            return pos.y < 0 || abs(pos.x) < 0.3;
        case FORM_X: {
            float sqr_dist_tl = sqr_dist_of(pos, 0.5 * vec2(-1.5, cos_30));
            float sqr_dist_br = sqr_dist_of(pos, 0.5 * vec2(1.5, -cos_30));
            return sqr_dist_tl > (0.4 * 0.4) && sqr_dist_br > (0.4 * 0.4);
        }
        case FORM_SIZE5: {
            float sqr_dist_tl = sqr_dist_of(pos, 0.5 * vec2(-1.5, cos_30));
            return sqr_dist_tl > (0.4 * 0.4);
        }
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
    ivec2 indices = tile_indices(st);
    // Tile index is outside range. It's empty there.
    if (indices.s == -1) {
        frag_data = vec4(0, 0, 0, 1);
        return;
    }

    Tile tile = get_tile(indices);
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
