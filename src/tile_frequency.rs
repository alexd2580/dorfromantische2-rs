use std::collections::HashMap;

use crate::data::{Segment, Terrain, HEX_SIDES};
use crate::map::Map;

/// An edge pattern is the 6-terrain array of a tile, canonicalized by picking
/// the lexicographically smallest rotation. This means tiles with different
/// segment forms but identical edge layouts are counted as the same pattern.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EdgePattern(pub [Terrain; HEX_SIDES]);

impl EdgePattern {
    /// Render segments into a 6-edge terrain array, then canonicalize rotation.
    pub fn from_segments(segments: &[Segment]) -> Self {
        let mut edges = [Terrain::Empty; HEX_SIDES];
        for segment in segments {
            for rotation in segment.rotations() {
                edges[rotation] = segment.terrain;
            }
        }
        Self::canonical(edges)
    }

    /// Find the lexicographically smallest rotation of the edge array.
    fn canonical(edges: [Terrain; HEX_SIDES]) -> Self {
        let mut best = edges;
        for rot in 1..HEX_SIDES {
            let rotated: [Terrain; HEX_SIDES] =
                std::array::from_fn(|i| edges[(i + rot) % HEX_SIDES]);
            if rotated < best {
                best = rotated;
            }
        }
        Self(best)
    }
}

pub struct TileFrequency {
    pub edges: EdgePattern,
    pub count: usize,
    pub fraction: f64,
}

#[derive(Default)]
pub struct TileFrequencies {
    pub entries: Vec<TileFrequency>,
    pub total_tiles: usize,
}

impl TileFrequencies {
    pub fn from_map(map: &Map) -> Self {
        let mut counts: HashMap<EdgePattern, usize> = HashMap::new();
        let mut total_tiles = 0;

        let index_len = map.tile_index.len();
        for key in 0..index_len {
            if let Some((base, count)) = map.tile_index[key] {
                if count == 0 {
                    continue;
                }
                let segments = &map.segments[base..base + count];
                let pattern = EdgePattern::from_segments(segments);
                *counts.entry(pattern).or_default() += 1;
                total_tiles += 1;
            }
        }

        // Also count the next tile.
        if !map.next_tile.is_empty() {
            let pattern = EdgePattern::from_segments(&map.next_tile);
            *counts.entry(pattern).or_default() += 1;
            total_tiles += 1;
        }

        let mut entries: Vec<TileFrequency> = counts
            .into_iter()
            .map(|(edges, count)| TileFrequency {
                edges,
                count,
                fraction: count as f64 / total_tiles as f64,
            })
            .collect();

        entries.sort_by(|a, b| b.count.cmp(&a.count));

        Self {
            entries,
            total_tiles,
        }
    }
}
