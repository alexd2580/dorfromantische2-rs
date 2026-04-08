/// A side of a hexagonal tile. Invariant: value is always 0..6.
/// 0 = North, increasing clockwise: 1=NE, 2=SE, 3=S, 4=SW, 5=NW.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Side(u8);

impl Side {
    pub const N: Side = Side(0);
    pub const NE: Side = Side(1);
    pub const SE: Side = Side(2);
    pub const S: Side = Side(3);
    pub const SW: Side = Side(4);
    pub const NW: Side = Side(5);
    pub const COUNT: usize = 6;
    pub const ALL: [Side; 6] = [Side::N, Side::NE, Side::SE, Side::S, Side::SW, Side::NW];

    /// Create a Side from a raw value. Panics if value >= 6.
    pub fn new(value: usize) -> Self {
        assert!(value < Self::COUNT, "Side value {value} out of range 0..6");
        Self(value as u8)
    }

    /// The numeric index (0..6) for use in arrays.
    pub fn index(self) -> usize {
        self.0 as usize
    }

    /// Rotate clockwise by `offset` sides.
    pub fn rotate(self, offset: usize) -> Side {
        Side(((self.0 as usize + offset) % Self::COUNT) as u8)
    }

    /// The side pointing in the opposite direction.
    pub fn opposite(self) -> Side {
        self.rotate(3)
    }
}

impl From<Side> for usize {
    fn from(side: Side) -> usize {
        side.index()
    }
}

impl Default for Side {
    fn default() -> Self {
        Side::N
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_side_all_values() {
        for (i, side) in Side::ALL.iter().enumerate() {
            assert_eq!(side.index(), i);
        }
    }

    #[test]
    fn test_side_rotate_identity() {
        for side in Side::ALL {
            assert_eq!(side.rotate(0), side);
            assert_eq!(side.rotate(6), side);
        }
    }

    #[test]
    fn test_side_rotate_wraps() {
        assert_eq!(Side::NW.rotate(1), Side::N); // 5 + 1 = 0
        assert_eq!(Side::SW.rotate(3), Side::NE); // 4 + 3 = 1
    }

    #[test]
    fn test_side_opposite() {
        assert_eq!(Side::N.opposite(), Side::S);
        assert_eq!(Side::NE.opposite(), Side::SW);
        assert_eq!(Side::SE.opposite(), Side::NW);
        assert_eq!(Side::S.opposite(), Side::N);
        assert_eq!(Side::SW.opposite(), Side::NE);
        assert_eq!(Side::NW.opposite(), Side::SE);
    }

    #[test]
    fn test_side_opposite_involution() {
        for side in Side::ALL {
            assert_eq!(side.opposite().opposite(), side);
        }
    }

    #[test]
    fn test_side_rotate_all_distinct() {
        for side in Side::ALL {
            let rotated: Vec<Side> = (0..6).map(|i| side.rotate(i)).collect();
            let unique: std::collections::HashSet<Side> = rotated.iter().copied().collect();
            assert_eq!(unique.len(), 6);
        }
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_side_new_rejects_6() {
        Side::new(6);
    }

    #[test]
    fn test_side_into_usize() {
        let s = Side::SE;
        let i: usize = s.into();
        assert_eq!(i, 2);
    }
}
