#![allow(dead_code)]

use glam::{IVec2, Vec2};

use crate::data::{Pos, Rotation, Side, HEX_SIDES};

pub const SIN_30: f32 = 0.5;
pub const COS_30: f32 = 0.866_025_4;

/// The rotation pointing in the opposite direction.
pub fn opposite_side(rotation: Rotation) -> Rotation {
    (rotation + HEX_SIDES / 2) % HEX_SIDES
}

pub fn neighbor_pos_of(pos: Pos, rotation: Rotation) -> Pos {
    pos + match rotation {
        0 => Pos::new(0, 1),
        1 => Pos::new(1, 0),
        2 => Pos::new(1, -1),
        3 => Pos::new(0, -1),
        4 => Pos::new(-1, 0),
        5 => Pos::new(-1, 1),
        _ => panic!("Rotation should be 0..{HEX_SIDES}, got {rotation}"),
    }
}

/// `neighbor_pos_of` accepting `Side` instead of bare `Rotation`.
pub fn neighbor(pos: Pos, side: Side) -> Pos {
    neighbor_pos_of(pos, side.index())
}

pub fn hex_to_world(pos: IVec2) -> Vec2 {
    Vec2::new(pos.x as f32 * 1.5, (pos.x + pos.y * 2) as f32 * COS_30)
}

pub fn world_to_hex(mut pos: Vec2) -> IVec2 {
    let x = (pos.x / 1.5).round();
    let y_rest = pos.y - x * COS_30;
    let y = (y_rest / (2.0 * COS_30)).round();

    let prelim = IVec2::new(x as i32, y as i32);
    pos -= hex_to_world(prelim);
    let xc = (0.5 * Vec2::new(COS_30, SIN_30).dot(pos) / COS_30).round() as i32;
    let xyc = (0.5 * Vec2::new(-COS_30, SIN_30).dot(pos) / COS_30).round() as i32;

    prelim + IVec2::new(xc - xyc, xyc)
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
        let origin = Pos::ZERO;
        let neighbors: std::collections::HashSet<Pos> =
            (0..HEX_SIDES).map(|r| neighbor_pos_of(origin, r)).collect();
        assert_eq!(neighbors.len(), HEX_SIDES);
    }

    #[test]
    fn test_neighbor_roundtrip() {
        for x in -3..=3 {
            for y in -3..=3 {
                let pos = Pos::new(x, y);
                for r in 0..HEX_SIDES {
                    let n = neighbor_pos_of(pos, r);
                    let back = neighbor_pos_of(n, opposite_side(r));
                    assert_eq!(back, pos, "Failed at ({x},{y}) rot {r}");
                }
            }
        }
    }

    #[test]
    fn test_neighbor_side_matches_rotation() {
        for x in -2..=2 {
            for y in -2..=2 {
                let pos = Pos::new(x, y);
                for side in Side::ALL {
                    assert_eq!(neighbor(pos, side), neighbor_pos_of(pos, side.index()),);
                }
            }
        }
    }

    #[test]
    #[should_panic(expected = "Rotation should be 0..6")]
    fn test_neighbor_pos_of_panics_on_invalid() {
        neighbor_pos_of(Pos::ZERO, 6);
    }

    #[test]
    fn test_hex_to_world_origin() {
        assert_eq!(hex_to_world(IVec2::ZERO), Vec2::ZERO);
    }

    #[test]
    fn test_hex_to_world_known() {
        let w = hex_to_world(IVec2::new(1, 0));
        assert!((w.x - 1.5).abs() < 1e-5);
        assert!((w.y - COS_30).abs() < 1e-4);

        let w = hex_to_world(IVec2::new(0, 1));
        assert!(w.x.abs() < 1e-5);
        assert!((w.y - 2.0 * COS_30).abs() < 1e-4);
    }

    #[test]
    fn test_world_to_hex_origin() {
        assert_eq!(world_to_hex(Vec2::ZERO), IVec2::ZERO);
    }

    #[test]
    fn test_world_to_hex_roundtrip_grid() {
        for x in -15..=15 {
            for y in -15..=15 {
                let pos = IVec2::new(x, y);
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
                let pos = IVec2::new(x, y);
                let center = hex_to_world(pos);
                for dx in [-0.1f32, 0.0, 0.1] {
                    for dy in [-0.1, 0.0, 0.1] {
                        let result = world_to_hex(center + Vec2::new(dx, dy));
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
            IVec2::new(3, -2),
            IVec2::new(-5, 7),
            IVec2::new(10, -10),
            IVec2::new(-1, 1),
        ];
        for pos in cases {
            let world = hex_to_world(pos);
            assert_eq!(world_to_hex(world), pos, "Failed for {pos:?}");
            // Also test slightly off-center
            assert_eq!(
                world_to_hex(world + Vec2::new(0.05, -0.05)),
                pos,
                "Perturbed failed for {pos:?}"
            );
        }
    }
}
