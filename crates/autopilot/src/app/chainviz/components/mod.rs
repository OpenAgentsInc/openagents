mod chain_node;
mod connector;
mod prompt_card;

use crate::app::ui::UiPalette;
use wgpui::Hsla;

#[derive(Clone, Copy)]
pub struct ChainTheme {
    pub card_bg: Hsla,
    pub card_border: Hsla,
    pub text_primary: Hsla,
    pub text_muted: Hsla,
    pub text_accent: Hsla,
    pub text_desc: Hsla,
    pub status_pending: Hsla,
    pub status_running: Hsla,
    pub status_complete: Hsla,
    pub status_failed: Hsla,
    pub prompt_bg: Hsla,
    pub prompt_border: Hsla,
    pub prompt_label: Hsla,
    pub prompt_text: Hsla,
    pub connector: Hsla,
}

impl ChainTheme {
    pub fn from_palette(palette: &UiPalette) -> Self {
        Self {
            card_bg: palette.panel,
            card_border: palette.panel_border,
            text_primary: palette.text_primary,
            text_muted: palette.text_muted,
            text_accent: palette.link,
            text_desc: palette.text_secondary,
            status_pending: palette.text_dim,
            status_running: palette.tool_progress_fg,
            status_complete: Hsla::new(120.0 / 360.0, 0.6, 0.45, 1.0),
            status_failed: Hsla::new(0.0, 0.6, 0.5, 1.0),
            prompt_bg: palette.panel_highlight,
            prompt_border: palette.panel_border,
            prompt_label: palette.text_dim,
            prompt_text: palette.text_primary,
            connector: palette.panel_border,
        }
    }
}

pub use chain_node::{ChainNode, NodeState};
pub use connector::Connector;
pub use prompt_card::PromptCard;
