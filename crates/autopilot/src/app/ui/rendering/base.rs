pub(crate) const INPUT_HEIGHT: f32 = 40.0;
pub(crate) const INPUT_PADDING: f32 = 12.0;
pub(crate) const OUTPUT_PADDING: f32 = 12.0;
pub(crate) const STATUS_BAR_HEIGHT: f32 = 20.0;
pub(crate) const STATUS_BAR_FONT_SIZE: f32 = 13.0;
pub(crate) const COMPOSER_BAR_HEIGHT: f32 = 28.0;
pub(crate) const COMPOSER_BAR_GAP: f32 = 8.0;
pub(crate) const COMPOSER_PILL_HEIGHT: f32 = 20.0;
pub(crate) const COMPOSER_PILL_GAP: f32 = 8.0;
pub(crate) const COMPOSER_PILL_PADDING_X: f32 = 10.0;
pub(crate) const COMPOSER_SEND_WIDTH: f32 = 58.0;
pub(crate) const COMPOSER_SEND_GAP: f32 = 10.0;
pub(crate) const COMPOSER_MENU_ITEM_HEIGHT: f32 = 22.0;
pub(crate) const COMPOSER_MENU_PADDING: f32 = 6.0;
pub(crate) const COMPOSER_MENU_MIN_WIDTH: f32 = 140.0;
pub(crate) const COMPOSER_MENU_MAX_ITEMS: usize = 8;
pub(crate) const TOPBAR_HEIGHT: f32 = 0.0;
pub(crate) const CONTENT_PADDING_X: f32 = 32.0;
/// Height of input area (input + padding + status bar) for modal positioning
pub(crate) const INPUT_AREA_HEIGHT: f32 = INPUT_HEIGHT
    + INPUT_PADDING * 2.0
    + STATUS_BAR_HEIGHT
    + COMPOSER_BAR_HEIGHT
    + COMPOSER_BAR_GAP;

pub(crate) const SESSION_MODAL_WIDTH: f32 = 760.0;
pub(crate) const SESSION_MODAL_HEIGHT: f32 = 520.0;
pub(crate) const AGENT_BACKENDS_MODAL_WIDTH: f32 = 760.0;
pub(crate) const AGENT_BACKENDS_MODAL_HEIGHT: f32 = 520.0;
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
pub(crate) const GATEWAY_MODAL_WIDTH: f32 = 760.0;
pub(crate) const GATEWAY_MODAL_HEIGHT: f32 = 500.0;
pub(crate) const LM_ROUTER_MODAL_WIDTH: f32 = 760.0;
pub(crate) const LM_ROUTER_MODAL_HEIGHT: f32 = 520.0;
pub(crate) const NEXUS_MODAL_WIDTH: f32 = 760.0;
pub(crate) const NEXUS_MODAL_HEIGHT: f32 = 540.0;
pub(crate) const SPARK_WALLET_MODAL_WIDTH: f32 = 780.0;
pub(crate) const SPARK_WALLET_MODAL_HEIGHT: f32 = 560.0;
pub(crate) const OANIX_MODAL_WIDTH: f32 = 780.0;
pub(crate) const OANIX_MODAL_HEIGHT: f32 = 560.0;
pub(crate) const DIRECTIVES_MODAL_WIDTH: f32 = 760.0;
pub(crate) const DIRECTIVES_MODAL_HEIGHT: f32 = 520.0;
pub(crate) const ISSUES_MODAL_WIDTH: f32 = 760.0;
pub(crate) const ISSUES_MODAL_HEIGHT: f32 = 520.0;
pub(crate) const AUTOPILOT_ISSUES_MODAL_WIDTH: f32 = 760.0;
pub(crate) const AUTOPILOT_ISSUES_MODAL_HEIGHT: f32 = 520.0;
pub(crate) const NIP28_MODAL_WIDTH: f32 = 760.0;
pub(crate) const NIP28_MODAL_HEIGHT: f32 = 520.0;
pub(crate) const NIP90_MODAL_WIDTH: f32 = 780.0;
pub(crate) const NIP90_MODAL_HEIGHT: f32 = 540.0;
pub(crate) const RLM_MODAL_WIDTH: f32 = 780.0;
pub(crate) const RLM_MODAL_HEIGHT: f32 = 540.0;
pub(crate) const RLM_TRACE_MODAL_WIDTH: f32 = 780.0;
pub(crate) const RLM_TRACE_MODAL_HEIGHT: f32 = 540.0;
pub(crate) const PYLON_EARNINGS_MODAL_WIDTH: f32 = 780.0;
pub(crate) const PYLON_EARNINGS_MODAL_HEIGHT: f32 = 540.0;
pub(crate) const PYLON_JOBS_MODAL_WIDTH: f32 = 780.0;
pub(crate) const PYLON_JOBS_MODAL_HEIGHT: f32 = 540.0;
pub(crate) const DVM_MODAL_WIDTH: f32 = 780.0;
pub(crate) const DVM_MODAL_HEIGHT: f32 = 540.0;
pub(crate) const HOOK_MODAL_WIDTH: f32 = 860.0;
pub(crate) const HOOK_MODAL_HEIGHT: f32 = 520.0;
pub(crate) const HOOK_EVENT_ROW_HEIGHT: f32 = 20.0;
pub(crate) const TOOL_PANEL_GAP: f32 = 8.0;

const SIDEBAR_WIDTH: f32 = 280.0;
const RIGHT_PANEL_WIDTH: f32 = 230.0;
const SIDEBAR_MIN_MAIN: f32 = 320.0;
const WORKSPACE_LIST_TOP: f32 = 56.0;
const WORKSPACE_ROW_HEIGHT: f32 = 30.0;
const WORKSPACE_ROW_GAP: f32 = 8.0;
const WORKSPACE_ROW_PADDING_X: f32 = 12.0;
const WORKSPACE_CONNECT_PILL_WIDTH: f32 = 58.0;
const WORKSPACE_CONNECT_PILL_HEIGHT: f32 = 16.0;
const GIT_PANEL_PADDING: f32 = 12.0;
const GIT_PANEL_ROW_HEIGHT: f32 = 36.0;
const GIT_PANEL_ROW_GAP: f32 = 6.0;
const GIT_PANEL_HEADER_HEIGHT: f32 = 14.0;
const GIT_PANEL_LINE_GAP: f32 = 8.0;
const TOPBAR_BACK_BUTTON_SIZE: f32 = 20.0;

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

pub(crate) struct WorkspaceListLayout {
    pub(crate) rows: Vec<Bounds>,
    pub(crate) connect_pills: Vec<Bounds>,
    pub(crate) empty_bounds: Option<Bounds>,
}

pub(crate) struct GitDiffPanelLayout {
    pub(crate) header_bounds: Bounds,
    pub(crate) status_bounds: Bounds,
    pub(crate) branch_bounds: Bounds,
    pub(crate) list_bounds: Bounds,
    pub(crate) row_bounds: Vec<(usize, Bounds)>,
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

pub(crate) struct InputLayout {
    pub(crate) area_bounds: Bounds,
    pub(crate) input_bounds: Bounds,
    pub(crate) send_bounds: Bounds,
    pub(crate) bar_bounds: Bounds,
}

pub(crate) struct ComposerBarLayout {
    pub(crate) model_bounds: Bounds,
    pub(crate) effort_bounds: Bounds,
    pub(crate) access_bounds: Bounds,
    pub(crate) skill_bounds: Bounds,
}

pub(crate) struct ComposerMenuLayout {
    pub(crate) bounds: Bounds,
    pub(crate) item_bounds: Vec<(usize, Bounds)>,
}

pub(crate) struct ApprovalsLayout {
    pub(crate) panel_bounds: Bounds,
    pub(crate) card_bounds: Vec<(usize, Bounds)>,
    pub(crate) approve_bounds: Vec<(usize, Bounds)>,
    pub(crate) decline_bounds: Vec<(usize, Bounds)>,
}

pub(crate) fn modal_y_in_content(logical_height: f32, modal_height: f32) -> f32 {
    let content_height = logical_height - INPUT_AREA_HEIGHT - TOPBAR_HEIGHT;
    TOPBAR_HEIGHT + (content_height - modal_height) / 2.0
}

pub(crate) fn sidebar_layout(
    logical_width: f32,
    logical_height: f32,
    left_open: bool,
    right_open: bool,
) -> SidebarLayout {
    let mut left_width = if left_open { SIDEBAR_WIDTH } else { 0.0 };
    let mut right_width = if right_open { RIGHT_PANEL_WIDTH } else { 0.0 };
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
    let size = 22.0;
    Bounds::new(
        sidebar_bounds.origin.x + sidebar_bounds.size.width - size - 16.0,
        sidebar_bounds.origin.y + 12.0,
        size,
        size,
    )
}

pub(crate) fn diff_back_button_bounds(sidebar_layout: &SidebarLayout) -> Bounds {
    Bounds::new(
        sidebar_layout.main.origin.x + 10.0,
        sidebar_layout.main.origin.y + 10.0,
        TOPBAR_BACK_BUTTON_SIZE,
        TOPBAR_BACK_BUTTON_SIZE,
    )
}

pub(crate) fn workspace_list_layout(
    sidebar_bounds: Bounds,
    workspace_count: usize,
) -> WorkspaceListLayout {
    let row_width = (sidebar_bounds.size.width - WORKSPACE_ROW_PADDING_X * 2.0).max(0.0);
    let mut rows = Vec::with_capacity(workspace_count);
    let mut connect_pills = Vec::with_capacity(workspace_count);
    let mut y = sidebar_bounds.origin.y + WORKSPACE_LIST_TOP;

    for _ in 0..workspace_count {
        let row_bounds = Bounds::new(
            sidebar_bounds.origin.x + WORKSPACE_ROW_PADDING_X,
            y,
            row_width,
            WORKSPACE_ROW_HEIGHT,
        );
        let pill_x = row_bounds.origin.x + row_bounds.size.width - WORKSPACE_CONNECT_PILL_WIDTH - 6.0;
        let pill_y =
            row_bounds.origin.y + (WORKSPACE_ROW_HEIGHT - WORKSPACE_CONNECT_PILL_HEIGHT) / 2.0;
        rows.push(row_bounds);
        connect_pills.push(Bounds::new(
            pill_x,
            pill_y,
            WORKSPACE_CONNECT_PILL_WIDTH,
            WORKSPACE_CONNECT_PILL_HEIGHT,
        ));
        y += WORKSPACE_ROW_HEIGHT + WORKSPACE_ROW_GAP;
    }

    let empty_bounds = if workspace_count == 0 {
        Some(Bounds::new(
            sidebar_bounds.origin.x + WORKSPACE_ROW_PADDING_X,
            y,
            row_width,
            20.0,
        ))
    } else {
        None
    };

    WorkspaceListLayout {
        rows,
        connect_pills,
        empty_bounds,
    }
}

pub(crate) fn git_diff_panel_layout(
    sidebar_bounds: Bounds,
    file_count: usize,
) -> GitDiffPanelLayout {
    let panel_x = sidebar_bounds.origin.x + GIT_PANEL_PADDING;
    let panel_width = (sidebar_bounds.size.width - GIT_PANEL_PADDING * 2.0).max(0.0);
    let mut y = sidebar_bounds.origin.y + GIT_PANEL_PADDING;

    let header_bounds = Bounds::new(panel_x, y, panel_width, GIT_PANEL_HEADER_HEIGHT);
    y += GIT_PANEL_HEADER_HEIGHT + GIT_PANEL_LINE_GAP;
    let status_bounds = Bounds::new(panel_x, y, panel_width, 14.0);
    y += 14.0 + GIT_PANEL_LINE_GAP;
    let branch_bounds = Bounds::new(panel_x, y, panel_width, 14.0);
    y += 14.0 + GIT_PANEL_LINE_GAP;

    let available_height =
        (sidebar_bounds.origin.y + sidebar_bounds.size.height - y - GIT_PANEL_PADDING).max(0.0);
    let min_approvals = 140.0;
    let diff_list_height = if available_height > min_approvals + 120.0 {
        available_height - min_approvals
    } else {
        available_height * 0.55
    }
    .max(0.0)
    .min(available_height);
    let list_bounds = Bounds::new(panel_x, y, panel_width, diff_list_height);

    let max_rows = if GIT_PANEL_ROW_HEIGHT + GIT_PANEL_ROW_GAP <= 0.0 {
        0
    } else {
        ((diff_list_height + GIT_PANEL_ROW_GAP) / (GIT_PANEL_ROW_HEIGHT + GIT_PANEL_ROW_GAP))
            .floor()
            .max(0.0) as usize
    };
    let visible_count = file_count.min(max_rows);
    let mut row_bounds = Vec::new();
    if visible_count > 0 {
        let mut row_y = list_bounds.origin.y;
        for index in 0..visible_count {
            row_bounds.push((
                index,
                Bounds::new(panel_x, row_y, panel_width, GIT_PANEL_ROW_HEIGHT),
            ));
            row_y += GIT_PANEL_ROW_HEIGHT + GIT_PANEL_ROW_GAP;
        }
    }

    GitDiffPanelLayout {
        header_bounds,
        status_bounds,
        branch_bounds,
        list_bounds,
        row_bounds,
    }
}

#[allow(dead_code)]
fn format_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.0}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

#[allow(dead_code)]
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
