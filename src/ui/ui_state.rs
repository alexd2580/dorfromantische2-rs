use crate::best_placements::MAX_SHOWN_PLACEMENTS;
use crate::data::HexPos;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TooltipMode {
    None,
    Group,
    Placement,
    Chance,
}

pub use crate::coords::CameraMode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuestDisplay {
    None,
    Min,
    Easy,
    All,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum ClosedGroupStyle {
    Show = 0,
    Dim = 1,
    Hide = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum SectionStyle {
    Terrain = 0,
    GroupStatic = 1,
    GroupDynamic = 2,
    Texture = 3,
    RailRiverOnly = 4,
}

#[allow(clippy::struct_excessive_bools)]
pub struct UiState {
    pub goto_x: String,
    pub goto_y: String,
    pub section_style: SectionStyle,
    pub closed_group_style: ClosedGroupStyle,
    pub highlight_hovered_group: bool,
    pub show_placements: [bool; MAX_SHOWN_PLACEMENTS],
    pub tooltip_mode: TooltipMode,
    pub show_biggest_groups: bool,
    /// Currently focused/highlighted group (from clicking in the groups overlay).
    pub focused_group: Option<usize>,
    pub show_tile_frequencies: bool,
    pub show_imperfect_tiles: bool,
    pub quest_display: QuestDisplay,
    pub sidebar_expanded: bool,
    /// The currently focused/highlighted placement position (from clicking a row).
    pub focused_placement: Option<HexPos>,
    pub camera_mode: CameraMode,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            goto_x: String::new(),
            goto_y: String::new(),
            section_style: SectionStyle::Terrain,
            closed_group_style: ClosedGroupStyle::Hide,
            highlight_hovered_group: false,
            show_placements: [false; MAX_SHOWN_PLACEMENTS],
            tooltip_mode: TooltipMode::Placement,
            show_biggest_groups: false,
            show_tile_frequencies: false,
            show_imperfect_tiles: false,
            quest_display: QuestDisplay::Min,
            sidebar_expanded: true,
            focused_placement: None,
            focused_group: None,
            camera_mode: CameraMode::Off,
        }
    }
}

impl UiState {
    pub fn parse_goto(&self) -> Option<HexPos> {
        let x = self.goto_x.parse::<i32>().ok()?;
        let y = self.goto_y.parse::<i32>().ok()?;
        Some(HexPos::new(x, y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ui_state_defaults() {
        let ui = UiState::default();
        assert_eq!(ui.section_style, SectionStyle::Terrain);
        assert_eq!(ui.closed_group_style, ClosedGroupStyle::Hide);
        assert!(!ui.highlight_hovered_group);
        assert!(ui.goto_x.is_empty());
        assert!(ui.goto_y.is_empty());
        assert_eq!(ui.tooltip_mode, TooltipMode::Placement);
        assert_eq!(ui.quest_display, QuestDisplay::Easy);
        assert!(ui.show_biggest_groups);
        assert!(!ui.show_tile_frequencies);
        assert!(ui.sidebar_expanded);
        assert!(ui.show_placements.iter().all(|&v| !v));
    }

    #[test]
    fn test_parse_goto_valid() {
        let ui = UiState {
            goto_x: "42".to_string(),
            goto_y: "-7".to_string(),
            ..Default::default()
        };
        let pos = ui.parse_goto();
        assert_eq!(pos, Some(HexPos::new(42, -7)));
    }

    #[test]
    fn test_parse_goto_invalid() {
        let ui = UiState {
            goto_x: "abc".to_string(),
            goto_y: "10".to_string(),
            ..Default::default()
        };
        assert_eq!(ui.parse_goto(), None);

        let ui = UiState {
            goto_x: "10".to_string(),
            goto_y: String::new(),
            ..Default::default()
        };
        assert_eq!(ui.parse_goto(), None);
    }
}
