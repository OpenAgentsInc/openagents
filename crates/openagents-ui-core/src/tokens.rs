use wgpui::Hsla;

pub const APP_DISPLAY_NAME: &str = "OpenAgents";
pub const DESKTOP_WINDOW_TITLE: &str = "Autopilot";

pub mod palette {
    use super::*;

    pub const CANVAS_BG_HEX: u32 = 0x080A10;
    pub const SURFACE_CARD_HEX: u32 = 0x111827;
    pub const BORDER_SUBTLE_HEX: u32 = 0x1F2937;

    pub fn canvas_bg() -> Hsla {
        Hsla::from_hex(CANVAS_BG_HEX)
    }

    pub fn surface_card() -> Hsla {
        Hsla::from_hex(SURFACE_CARD_HEX)
    }

    pub fn border_subtle() -> Hsla {
        Hsla::from_hex(BORDER_SUBTLE_HEX)
    }
}

pub mod spacing {
    pub const EDGE_MARGIN: f32 = 24.0;
    pub const CARD_HEIGHT: f32 = 180.0;
    pub const CARD_MAX_WIDTH: f32 = 680.0;
}
