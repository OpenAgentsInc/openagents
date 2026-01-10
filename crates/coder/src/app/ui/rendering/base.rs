pub(crate) const INPUT_HEIGHT: f32 = 40.0;
pub(crate) const INPUT_PADDING: f32 = 12.0;
pub(crate) const OUTPUT_PADDING: f32 = 12.0;
pub(crate) const STATUS_BAR_HEIGHT: f32 = 20.0;
pub(crate) const STATUS_BAR_FONT_SIZE: f32 = 13.0;
/// Height of input area (input + padding + status bar) for modal positioning
pub(crate) const INPUT_AREA_HEIGHT: f32 = INPUT_HEIGHT + INPUT_PADDING + STATUS_BAR_HEIGHT;

pub(crate) const SESSION_MODAL_WIDTH: f32 = 760.0;
pub(crate) const SESSION_MODAL_HEIGHT: f32 = 520.0;
pub(crate) const SESSION_CARD_HEIGHT: f32 = 100.0;
pub(crate) const SESSION_CARD_GAP: f32 = 12.0;
pub(crate) const SESSION_MODAL_PADDING: f32 = 16.0;
pub(crate) const SKILL_CARD_HEIGHT: f32 = 110.0;
pub(crate) const SETTINGS_MODAL_WIDTH: f32 = 760.0;
pub(crate) const SETTINGS_MODAL_HEIGHT: f32 = 480.0;
pub(crate) const SETTINGS_ROW_HEIGHT: f32 = 24.0;
pub(crate) const SETTINGS_TAB_HEIGHT: f32 = 22.0;
pub(crate) const HELP_MODAL_WIDTH: f32 = 760.0;
pub(crate) const HELP_MODAL_HEIGHT: f32 = 520.0;
pub(crate) const DSPY_MODAL_WIDTH: f32 = 720.0;
pub(crate) const DSPY_MODAL_HEIGHT: f32 = 480.0;
pub(crate) const OANIX_MODAL_WIDTH: f32 = 780.0;
pub(crate) const OANIX_MODAL_HEIGHT: f32 = 560.0;
pub(crate) const NIP28_MODAL_WIDTH: f32 = 760.0;
pub(crate) const NIP28_MODAL_HEIGHT: f32 = 520.0;
pub(crate) const HOOK_MODAL_WIDTH: f32 = 860.0;
pub(crate) const HOOK_MODAL_HEIGHT: f32 = 520.0;
pub(crate) const HOOK_EVENT_ROW_HEIGHT: f32 = 20.0;
pub(crate) const TOOL_PANEL_GAP: f32 = 8.0;

const SIDEBAR_WIDTH: f32 = 220.0;
const SIDEBAR_MIN_MAIN: f32 = 320.0;

#[derive(Clone, Debug)]
struct LinePrefix {
    text: String,
    x: f32,
    content_x: f32,
    font_size: f32,
}

pub(crate) struct SidebarLayout {
    pub(crate) left: Option<Bounds>,
    pub(crate) right: Option<Bounds>,
    pub(crate) main: Bounds,
}

pub(crate) struct SessionListLayout {
    pub(crate) modal_bounds: Bounds,
    pub(crate) card_bounds: Vec<(usize, Bounds)>,
    pub(crate) checkpoint_bounds: Option<Bounds>,
}

pub(crate) struct AgentListLayout {
    pub(crate) card_bounds: Vec<(usize, Bounds)>,
}

pub(crate) struct SkillListLayout {
    pub(crate) card_bounds: Vec<(usize, Bounds)>,
}

pub(crate) struct HookEventLayout {
    pub(crate) list_bounds: Bounds,
    pub(crate) inspector_bounds: Bounds,
    pub(crate) row_bounds: Vec<(usize, Bounds)>,
}

pub(crate) fn modal_y_in_content(logical_height: f32, modal_height: f32) -> f32 {
    let content_height = logical_height - INPUT_AREA_HEIGHT;
    (content_height - modal_height) / 2.0
}

pub(crate) fn sidebar_layout(
    logical_width: f32,
    logical_height: f32,
    left_open: bool,
    right_open: bool,
) -> SidebarLayout {
    let mut left_width = if left_open { SIDEBAR_WIDTH } else { 0.0 };
    let mut right_width = if right_open { SIDEBAR_WIDTH } else { 0.0 };
    let available_main = logical_width - left_width - right_width;
    if available_main < SIDEBAR_MIN_MAIN {
        let overflow = SIDEBAR_MIN_MAIN - available_main;
        if left_width > 0.0 && right_width > 0.0 {
            let reduce = overflow / 2.0;
            left_width = (left_width - reduce).max(120.0);
            right_width = (right_width - reduce).max(120.0);
        } else if left_width > 0.0 {
            left_width = (left_width - overflow).max(120.0);
        } else if right_width > 0.0 {
            right_width = (right_width - overflow).max(120.0);
        }
    }
    let main_width = (logical_width - left_width - right_width).max(1.0);
    let main = Bounds::new(left_width, 0.0, main_width, logical_height);
    let left = if left_width > 0.0 {
        Some(Bounds::new(0.0, 0.0, left_width, logical_height))
    } else {
        None
    };
    let right = if right_width > 0.0 {
        Some(Bounds::new(
            logical_width - right_width,
            0.0,
            right_width,
            logical_height,
        ))
    } else {
        None
    };

    SidebarLayout { left, right, main }
}

pub(crate) fn new_session_button_bounds(sidebar_bounds: Bounds) -> Bounds {
    Bounds::new(
        sidebar_bounds.origin.x + 12.0,
        sidebar_bounds.origin.y + 12.0,
        sidebar_bounds.size.width - 24.0,
        32.0,
    )
}

fn format_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.0}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn format_duration_ms(ms: u64) -> String {
    if ms < 1_000 {
        format!("{}ms", ms)
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1_000.0)
    } else {
        let mins = ms / 60_000;
        let secs = (ms % 60_000) / 1_000;
        format!("{}m{}s", mins, secs)
    }
}
