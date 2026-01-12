pub(crate) mod layout;
pub(crate) mod rendering;
pub(crate) mod theme;

pub(crate) use layout::{split_into_words_for_layout, wrap_text};
pub(crate) use rendering::{
    agent_list_layout, agent_modal_content_top, diff_back_button_bounds, git_diff_panel_layout,
    hook_event_layout, modal_y_in_content, new_session_button_bounds, render_app,
    session_list_layout, sidebar_layout, skill_list_layout, skill_modal_content_top,
    workspace_list_layout, CONTENT_PADDING_X, INPUT_PADDING, SESSION_MODAL_HEIGHT,
    STATUS_BAR_HEIGHT,
};
pub(crate) use theme::{palette_for, theme_label, ThemeSetting, UiPalette};
