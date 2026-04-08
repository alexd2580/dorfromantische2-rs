use super::{Segment, Side, Terrain, HEX_SIDES};

/// The terrain visible at each of the 6 edges of a tile. Computed once from
/// segments and then immutable — the single source of truth for "what does
/// this tile look like from outside."
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EdgeProfile([Terrain; HEX_SIDES]);

#[allow(dead_code)]
impl EdgeProfile {
    /// Build an edge profile from a slice of segments.
    pub fn from_segments(segments: &[Segment]) -> Self {
        let mut edges = [Terrain::Empty; HEX_SIDES];
        for segment in segments {
            for rotation in segment.rotations() {
                edges[rotation] = segment.terrain;
            }
        }
        Self(edges)
    }

    /// What terrain is at the given side?
    pub fn at(&self, side: Side) -> Terrain {
        self.0[side.index()]
    }

    /// Raw array access by index.
    pub fn at_index(&self, index: usize) -> Terrain {
        self.0[index]
    }

    /// The underlying array.
    pub fn as_array(&self) -> &[Terrain; HEX_SIDES] {
        &self.0
    }

    /// Rotate the profile clockwise by `offset` sides.
    pub fn rotated(&self, offset: usize) -> Self {
        Self(std::array::from_fn(|i| {
            self.0[(i + HEX_SIDES - offset) % HEX_SIDES]
        }))
    }

    /// Canonical form: the lexicographically smallest rotation.
    pub fn canonical(&self) -> Self {
        let mut best = self.0;
        for rot in 1..HEX_SIDES {
            let rotated: [Terrain; HEX_SIDES] =
                std::array::from_fn(|i| self.0[(i + rot) % HEX_SIDES]);
            if rotated < best {
                best = rotated;
            }
        }
        Self(best)
    }
}

impl Default for EdgeProfile {
    fn default() -> Self {
        Self([Terrain::Empty; HEX_SIDES])
    }
}

#[cfg(test)]
mod tests {
    use super::super::Form;
    use super::*;

    fn seg(form: Form, terrain: Terrain, rotation: usize) -> Segment {
        Segment {
            pos: glam::IVec2::ZERO,
            form,
            terrain,
            rotation,
            unit_count: 0,
        }
    }

    #[test]
    fn test_empty_profile() {
        let p = EdgeProfile::from_segments(&[]);
        for side in Side::ALL {
            assert_eq!(p.at(side), Terrain::Empty);
        }
    }

    #[test]
    fn test_size6_fills_all() {
        let p = EdgeProfile::from_segments(&[seg(Form::Size6, Terrain::Forest, 0)]);
        for side in Side::ALL {
            assert_eq!(p.at(side), Terrain::Forest);
        }
    }

    #[test]
    fn test_straight_covers_two_opposite() {
        // Straight at rotation 1 covers sides 1 and 4.
        let p = EdgeProfile::from_segments(&[seg(Form::Straight, Terrain::Rail, 1)]);
        assert_eq!(p.at(Side::NE), Terrain::Rail);
        assert_eq!(p.at(Side::SW), Terrain::Rail);
        assert_eq!(p.at(Side::N), Terrain::Empty);
        assert_eq!(p.at(Side::SE), Terrain::Empty);
    }

    #[test]
    fn test_multi_segment() {
        let p = EdgeProfile::from_segments(&[
            seg(Form::Size3, Terrain::Forest, 0), // covers 0,1,2
            seg(Form::Size3, Terrain::House, 3),  // covers 3,4,5
        ]);
        assert_eq!(p.at(Side::N), Terrain::Forest);
        assert_eq!(p.at(Side::NE), Terrain::Forest);
        assert_eq!(p.at(Side::SE), Terrain::Forest);
        assert_eq!(p.at(Side::S), Terrain::House);
        assert_eq!(p.at(Side::SW), Terrain::House);
        assert_eq!(p.at(Side::NW), Terrain::House);
    }

    #[test]
    fn test_rotated_roundtrip() {
        let p = EdgeProfile::from_segments(&[
            seg(Form::Size2, Terrain::Rail, 0),
            seg(Form::Size4, Terrain::Forest, 2),
        ]);
        assert_eq!(p.rotated(0), p);
        assert_eq!(p.rotated(6), p);
        assert_eq!(p.rotated(1).rotated(5), p);
    }

    #[test]
    fn test_canonical_rotation_invariant() {
        let p = EdgeProfile::from_segments(&[
            seg(Form::Straight, Terrain::River, 0),
            seg(Form::Size2, Terrain::Forest, 1),
        ]);
        let canon = p.canonical();
        for rot in 0..6 {
            assert_eq!(p.rotated(rot).canonical(), canon);
        }
    }
}
