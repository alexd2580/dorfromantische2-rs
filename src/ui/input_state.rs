use glam::Vec2;

use crate::{
    data::{HexPos, Rotation},
    group::GroupIndex,
    map::SegmentIndex,
};

/// Input and mouse state extracted from App.
#[derive(Default)]
pub struct InputState {
    /// Mouse position in window coordinates.
    pub mouse_position: Vec2,
    /// Whether the left mouse button is held.
    pub grab_move: bool,
    /// Whether the right mouse button is held.
    pub grab_rotate: bool,
    /// World hover position of mouse.
    pub hover_pos: HexPos,
    /// Hovered rotation.
    pub hover_rotation: Rotation,
    /// Hovered segment index (if present).
    pub hover_segment: Option<SegmentIndex>,
    /// Hovered group index (if present).
    pub hover_group: Option<GroupIndex>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_state_defaults() {
        let state = InputState::default();
        assert_eq!(state.mouse_position, Vec2::ZERO);
        assert!(!state.grab_move);
        assert!(!state.grab_rotate);
        assert_eq!(state.hover_pos, HexPos::ZERO);
        assert_eq!(state.hover_rotation, 0);
        assert!(state.hover_segment.is_none());
        assert!(state.hover_group.is_none());
    }
}
