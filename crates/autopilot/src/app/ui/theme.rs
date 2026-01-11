use serde::{Deserialize, Serialize};
use wgpui::Hsla;

pub(crate) fn theme_label(theme: ThemeSetting) -> &'static str {
    match theme {
        ThemeSetting::Dark => "Dark",
        ThemeSetting::Light => "Light",
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct UiPalette {
    pub(crate) background: Hsla,
    pub(crate) panel: Hsla,
    pub(crate) panel_border: Hsla,
    pub(crate) panel_highlight: Hsla,
    pub(crate) overlay: Hsla,
    #[allow(dead_code)]
    pub(crate) input_bg: Hsla,
    pub(crate) input_border: Hsla,
    pub(crate) input_border_focused: Hsla,
    pub(crate) text_primary: Hsla,
    pub(crate) text_secondary: Hsla,
    pub(crate) text_muted: Hsla,
    pub(crate) text_dim: Hsla,
    pub(crate) text_faint: Hsla,
    pub(crate) prompt: Hsla,
    #[allow(dead_code)]
    pub(crate) status_left: Hsla,
    pub(crate) status_right: Hsla,
    pub(crate) user_text: Hsla,
    pub(crate) assistant_text: Hsla,
    pub(crate) thinking_text: Hsla,
    pub(crate) selection_bg: Hsla,
    #[allow(dead_code)]
    pub(crate) tool_panel_bg: Hsla,
    #[allow(dead_code)]
    pub(crate) tool_panel_border: Hsla,
    pub(crate) tool_progress_bg: Hsla,
    pub(crate) tool_progress_fg: Hsla,
    pub(crate) code_bg: Hsla,
    pub(crate) inline_code_bg: Hsla,
    pub(crate) link: Hsla,
    pub(crate) blockquote: Hsla,
}

pub(crate) fn palette_for(theme: ThemeSetting) -> UiPalette {
    match theme {
        ThemeSetting::Dark => UiPalette {
            background: Hsla::new(0.0, 0.0, 0.0, 1.0),
            panel: Hsla::new(220.0, 0.15, 0.12, 1.0),
            panel_border: Hsla::new(220.0, 0.15, 0.25, 1.0),
            panel_highlight: Hsla::new(220.0, 0.2, 0.18, 1.0),
            overlay: Hsla::new(0.0, 0.0, 0.0, 0.7),
            input_bg: Hsla::new(220.0, 0.15, 0.08, 1.0),
            input_border: Hsla::new(220.0, 0.15, 0.25, 1.0),
            input_border_focused: Hsla::new(0.0, 0.0, 1.0, 1.0),
            text_primary: Hsla::new(0.0, 0.0, 0.9, 1.0),
            text_secondary: Hsla::new(0.0, 0.0, 0.7, 1.0),
            text_muted: Hsla::new(0.0, 0.0, 0.6, 1.0),
            text_dim: Hsla::new(0.0, 0.0, 0.5, 1.0),
            text_faint: Hsla::new(0.0, 0.0, 0.4, 1.0),
            prompt: Hsla::new(0.0, 0.0, 0.6, 1.0),
            status_left: Hsla::new(35.0, 0.8, 0.65, 1.0),
            status_right: Hsla::new(0.0, 0.0, 0.55, 1.0),
            user_text: Hsla::new(0.0, 0.0, 0.6, 1.0),
            assistant_text: Hsla::new(180.0, 0.5, 0.7, 1.0),
            thinking_text: Hsla::new(0.0, 0.0, 0.5, 1.0),
            selection_bg: Hsla::new(200.0, 0.6, 0.55, 0.35),
            tool_panel_bg: Hsla::new(220.0, 0.15, 0.12, 1.0),
            tool_panel_border: Hsla::new(220.0, 0.15, 0.25, 1.0),
            tool_progress_bg: Hsla::new(220.0, 0.15, 0.20, 1.0),
            tool_progress_fg: Hsla::new(200.0, 0.8, 0.6, 1.0),
            code_bg: Hsla::new(220.0, 0.18, 0.14, 1.0),
            inline_code_bg: Hsla::new(220.0, 0.12, 0.18, 1.0),
            link: Hsla::new(200.0, 0.7, 0.6, 1.0),
            blockquote: Hsla::new(200.0, 0.6, 0.6, 1.0),
        },
        ThemeSetting::Light => UiPalette {
            background: Hsla::new(210.0, 0.2, 0.96, 1.0),
            panel: Hsla::new(0.0, 0.0, 1.0, 1.0),
            panel_border: Hsla::new(210.0, 0.1, 0.78, 1.0),
            panel_highlight: Hsla::new(210.0, 0.4, 0.9, 1.0),
            overlay: Hsla::new(0.0, 0.0, 0.0, 0.3),
            input_bg: Hsla::new(0.0, 0.0, 1.0, 1.0),
            input_border: Hsla::new(210.0, 0.1, 0.72, 1.0),
            input_border_focused: Hsla::new(210.0, 0.8, 0.4, 1.0),
            text_primary: Hsla::new(0.0, 0.0, 0.12, 1.0),
            text_secondary: Hsla::new(0.0, 0.0, 0.25, 1.0),
            text_muted: Hsla::new(0.0, 0.0, 0.35, 1.0),
            text_dim: Hsla::new(0.0, 0.0, 0.45, 1.0),
            text_faint: Hsla::new(0.0, 0.0, 0.55, 1.0),
            prompt: Hsla::new(0.0, 0.0, 0.35, 1.0),
            status_left: Hsla::new(25.0, 0.85, 0.35, 1.0),
            status_right: Hsla::new(0.0, 0.0, 0.4, 1.0),
            user_text: Hsla::new(0.0, 0.0, 0.35, 1.0),
            assistant_text: Hsla::new(200.0, 0.6, 0.35, 1.0),
            thinking_text: Hsla::new(0.0, 0.0, 0.4, 1.0),
            selection_bg: Hsla::new(210.0, 0.7, 0.5, 0.25),
            tool_panel_bg: Hsla::new(0.0, 0.0, 0.98, 1.0),
            tool_panel_border: Hsla::new(210.0, 0.1, 0.82, 1.0),
            tool_progress_bg: Hsla::new(210.0, 0.2, 0.88, 1.0),
            tool_progress_fg: Hsla::new(200.0, 0.8, 0.45, 1.0),
            code_bg: Hsla::new(210.0, 0.15, 0.92, 1.0),
            inline_code_bg: Hsla::new(210.0, 0.15, 0.9, 1.0),
            link: Hsla::new(210.0, 0.7, 0.35, 1.0),
            blockquote: Hsla::new(210.0, 0.5, 0.4, 1.0),
        },
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ThemeSetting {
    Dark,
    Light,
}

impl Default for ThemeSetting {
    fn default() -> Self {
        ThemeSetting::Dark
    }
}
