use std::collections::HashMap;

use crate::data::{EdgeProfile, Segment};
use crate::map::Map;

/// A canonicalized edge profile (rotation-normalized) used as a frequency key.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EdgePattern(pub EdgeProfile);

impl EdgePattern {
    pub fn from_segments(segments: &[Segment]) -> Self {
        Self(EdgeProfile::from_segments(segments).canonical())
    }
}

pub struct TileFrequency {
    #[allow(dead_code)]
    pub edges: EdgePattern,
    /// A representative set of segments for rendering this pattern.
    pub segments: Vec<Segment>,
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
        let mut counts: HashMap<EdgePattern, (usize, Vec<Segment>)> = HashMap::new();
        let mut total_tiles = 0;

        let index_len = map.tile_index.len();
        for key in 0..index_len {
            if let Some((base, count)) = map.tile_index[key] {
                if count == 0 {
                    continue;
                }
                let segments = &map.segments[base..base + count];
                let pattern = EdgePattern::from_segments(segments);
                counts
                    .entry(pattern)
                    .or_insert_with(|| (0, segments.to_vec()))
                    .0 += 1;
                total_tiles += 1;
            }
        }

        if !map.next_tile.is_empty() {
            let pattern = EdgePattern::from_segments(&map.next_tile);
            counts
                .entry(pattern)
                .or_insert_with(|| (0, map.next_tile.clone()))
                .0 += 1;
            total_tiles += 1;
        }

        let mut entries: Vec<TileFrequency> = counts
            .into_iter()
            .map(|(edges, (count, segments))| TileFrequency {
                edges,
                segments,
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
