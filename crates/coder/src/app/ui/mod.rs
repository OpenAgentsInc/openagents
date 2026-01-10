pub(crate) mod layout;
pub(crate) mod rendering;
pub(crate) mod theme;

pub(crate) use layout::{split_into_words_for_layout, wrap_text};
pub(crate) use rendering::{
    agent_list_layout, agent_modal_content_top, hook_event_layout, modal_y_in_content,
    new_session_button_bounds, render_app, session_list_layout, sidebar_layout, skill_list_layout,
    skill_modal_content_top, AgentListLayout, HookEventLayout, SessionListLayout, SidebarLayout,
    SkillListLayout, HELP_MODAL_HEIGHT, HELP_MODAL_WIDTH, HOOK_EVENT_ROW_HEIGHT, HOOK_MODAL_HEIGHT,
    HOOK_MODAL_WIDTH, INPUT_AREA_HEIGHT, INPUT_HEIGHT, INPUT_PADDING, OUTPUT_PADDING,
    SESSION_CARD_GAP, SESSION_CARD_HEIGHT, SESSION_MODAL_HEIGHT, SESSION_MODAL_PADDING,
    SESSION_MODAL_WIDTH, SETTINGS_MODAL_HEIGHT, SETTINGS_MODAL_WIDTH, SETTINGS_ROW_HEIGHT,
    SETTINGS_TAB_HEIGHT, STATUS_BAR_HEIGHT, STATUS_BAR_FONT_SIZE, TOOL_PANEL_GAP,
};
pub(crate) use theme::{palette_for, theme_label, ThemeSetting, UiPalette};
