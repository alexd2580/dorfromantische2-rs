use glam::Vec2;

use crate::coords::WorldPos;
use crate::data::{HexPos, Rotation, HEX_SIDES};

pub const SIN_30: f32 = 0.5;
pub const COS_30: f32 = 0.866_025_4;

/// The rotation pointing in the opposite direction.
pub fn opposite_side(rotation: Rotation) -> Rotation {
    (rotation + HEX_SIDES / 2) % HEX_SIDES
}

pub fn neighbor_pos_of(pos: HexPos, rotation: Rotation) -> HexPos {
    pos + match rotation {
        0 => HexPos::new(0, 1),
        1 => HexPos::new(1, 0),
        2 => HexPos::new(1, -1),
        3 => HexPos::new(0, -1),
        4 => HexPos::new(-1, 0),
        5 => HexPos::new(-1, 1),
        _ => panic!("Rotation should be 0..{HEX_SIDES}, got {rotation}"),
    }
}

pub fn hex_to_world(pos: HexPos) -> WorldPos {
    WorldPos(Vec2::new(
        pos.x() as f32 * 1.5,
        (pos.x() + pos.y() * 2) as f32 * COS_30,
    ))
}

pub fn world_to_hex(pos: WorldPos) -> HexPos {
    let mut raw = pos.0;
    let x = (raw.x / 1.5).round();
    let y_rest = raw.y - x * COS_30;
    let y = (y_rest / (2.0 * COS_30)).round();

    let prelim = HexPos::new(x as i32, y as i32);
    raw -= hex_to_world(prelim).0;
    let xc = (0.5 * Vec2::new(COS_30, SIN_30).dot(raw) / COS_30).round() as i32;
    let xyc = (0.5 * Vec2::new(-COS_30, SIN_30).dot(raw) / COS_30).round() as i32;

    prelim + HexPos::new(xc - xyc, xyc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opposite_side_all() {
        for r in 0..HEX_SIDES {
            assert_eq!(opposite_side(r), (r + 3) % 6);
            assert_eq!(opposite_side(opposite_side(r)), r);
        }
    }

    #[test]
    fn test_neighbor_pos_of_six_unique() {
        let origin = HexPos::ZERO;
        let neighbors: std::collections::HashSet<HexPos> =
            (0..HEX_SIDES).map(|r| neighbor_pos_of(origin, r)).collect();
        assert_eq!(neighbors.len(), HEX_SIDES);
    }

    #[test]
    fn test_neighbor_roundtrip() {
        for x in -3..=3 {
            for y in -3..=3 {
                let pos = HexPos::new(x, y);
                for r in 0..HEX_SIDES {
                    let n = neighbor_pos_of(pos, r);
                    let back = neighbor_pos_of(n, opposite_side(r));
                    assert_eq!(back, pos, "Failed at ({x},{y}) rot {r}");
                }
            }
        }
    }

    #[test]
    #[should_panic(expected = "Rotation should be 0..6")]
    fn test_neighbor_pos_of_panics_on_invalid() {
        neighbor_pos_of(HexPos::ZERO, 6);
    }

    #[test]
    fn test_hex_to_world_origin() {
        assert_eq!(hex_to_world(HexPos::ZERO), WorldPos::ZERO);
    }

    #[test]
    fn test_hex_to_world_known() {
        let w = hex_to_world(HexPos::new(1, 0));
        assert!((w.x() - 1.5).abs() < 1e-5);
        assert!((w.y() - COS_30).abs() < 1e-4);

        let w = hex_to_world(HexPos::new(0, 1));
        assert!(w.x().abs() < 1e-5);
        assert!((w.y() - 2.0 * COS_30).abs() < 1e-4);
    }

    #[test]
    fn test_world_to_hex_origin() {
        assert_eq!(world_to_hex(WorldPos::ZERO), HexPos::ZERO);
    }

    #[test]
    fn test_world_to_hex_roundtrip_grid() {
        for x in -15..=15 {
            for y in -15..=15 {
                let pos = HexPos::new(x, y);
                let world = hex_to_world(pos);
                let back = world_to_hex(world);
                assert_eq!(back, pos, "Roundtrip failed for ({x}, {y})");
            }
        }
    }

    #[test]
    fn test_world_to_hex_perturbation_stable() {
        // Small perturbations around hex center should resolve to same hex.
        for x in -5..=5 {
            for y in -5..=5 {
                let pos = HexPos::new(x, y);
                let center = hex_to_world(pos);
                for dx in [-0.1f32, 0.0, 0.1] {
                    for dy in [-0.1, 0.0, 0.1] {
                        let result = world_to_hex(WorldPos(center.0 + Vec2::new(dx, dy)));
                        assert_eq!(result, pos, "Perturbed ({dx},{dy}) at ({x},{y})");
                    }
                }
            }
        }
    }

    #[test]
    fn test_world_to_hex_non_trivial() {
        // Verify specific non-origin conversions.
        let cases = [
            HexPos::new(3, -2),
            HexPos::new(-5, 7),
            HexPos::new(10, -10),
            HexPos::new(-1, 1),
        ];
        for pos in cases {
            let world = hex_to_world(pos);
            assert_eq!(world_to_hex(world), pos, "Failed for {pos:?}");
            // Also test slightly off-center
            assert_eq!(
                world_to_hex(WorldPos(world.0 + Vec2::new(0.05, -0.05))),
                pos,
                "Perturbed failed for {pos:?}"
            );
        }
    }
}
