use crate::app_state::{
    AutopilotChatState, AutopilotDiffArtifact, AutopilotMessageStatus,
    AutopilotProjectIdentity, AutopilotReviewArtifact, AutopilotRole, AutopilotTerminalSession,
    AutopilotTerminalSessionStatus, AutopilotThreadMetadata, CodingAgentPaneState,
    CodingAgentPaneInputs, CodingAgentRailTab, PaneKind, RenderState,
};
use crate::pane_renderer::{
    paint_disabled_button, paint_mission_control_section_panel, paint_primary_button,
    paint_secondary_button, paint_standard_input_frame,
};
use codex_client::{AskForApproval, SandboxMode};
use wgpui::{Bounds, Component, Hsla, InputEvent, PaintContext, Point, Quad, SvgQuad, theme};

const PADDING: f32 = 12.0;
const GAP: f32 = 12.0;
const MINI_GAP: f32 = 6.0;
const MINI_BUTTON_WIDTH: f32 = 28.0;
const HEADER_ACTION_HEIGHT: f32 = 32.0;
const APPROVAL_BAR_HEIGHT: f32 = 58.0;
const APPROVAL_BUTTON_HEIGHT: f32 = 30.0;
const APPROVAL_BUTTON_INSPECT_WIDTH: f32 = 76.0;
const APPROVAL_BUTTON_DENY_WIDTH: f32 = 76.0;
const APPROVAL_BUTTON_APPROVE_WIDTH: f32 = 92.0;
const APPROVAL_DETAIL_CARD_GAP: f32 = 10.0;
const APPROVAL_DETAIL_HERO_HEIGHT: f32 = 124.0;
const APPROVAL_DETAIL_QUEUE_HEIGHT: f32 = 72.0;
const APPROVAL_DETAIL_NOTE_HEIGHT: f32 = 64.0;
const APPROVAL_DETAIL_ROW_HEIGHT: f32 = 34.0;
const CARD_RADIUS: f32 = 8.0;
const SECTION_HEADER_HEIGHT: f32 = 34.0;
const SUMMARY_HEIGHT: f32 = 96.0;
const BADGE_GAP: f32 = 8.0;
const BADGE_HEIGHT: f32 = 32.0;
const BADGE_BRANCH_ACCENT: u32 = 0x4AA3FF;
const BADGE_MODE_ACCENT: u32 = 0x66B2FF;
const BADGE_MUTED_ACCENT: u32 = 0x7E8794;
const BADGE_WARNING_ACCENT: u32 = 0xF6B756;
const BADGE_RUNNING_ACCENT: u32 = 0x39C6FF;
const BADGE_SUCCESS_ACCENT: u32 = 0x5DD39E;
const BADGE_ERROR_ACCENT: u32 = 0xF56B6B;
const TIMELINE_ROW_HEIGHT: f32 = 48.0;
const TIMELINE_ROW_GAP: f32 = 8.0;
const TIMELINE_VIEWPORT_INSET_X: f32 = 12.0;
const TIMELINE_VIEWPORT_INSET_Y: f32 = 8.0;
const TERMINAL_LINE_HEIGHT: f32 = 14.0;
const TERMINAL_META_HEIGHT: f32 = 32.0;
const TERMINAL_FOOTER_HEIGHT: f32 = 34.0;
const TERMINAL_VIEWPORT_INSET_X: f32 = 12.0;
const TERMINAL_VIEWPORT_INSET_Y: f32 = 8.0;
const TERMINAL_PROMPT_PREFIX_WIDTH: f32 = 16.0;
const REVIEW_SUMMARY_HEIGHT: f32 = 74.0;
const REVIEW_VIEWPORT_INSET_X: f32 = 12.0;
const REVIEW_VIEWPORT_INSET_Y: f32 = 10.0;
const REVIEW_FILE_ROW_HEIGHT: f32 = 40.0;
const REVIEW_FILE_ROW_GAP: f32 = 8.0;
const REVIEW_TAB_HEIGHT: f32 = 28.0;
const DIFF_LINE_HEIGHT: f32 = 14.0;
const COMPOSER_BAR_HEIGHT: f32 = 56.0;
const COMPOSER_ROW_HEIGHT: f32 = 34.0;
const COMPOSER_FOLDER_HEIGHT: f32 = 28.0;
const COMPOSER_FOLDER_WIDTH: f32 = 300.0;
const COMPOSER_SEND_WIDTH: f32 = 34.0;
const TERMINAL_SEND_WIDTH: f32 = 72.0;
const THREAD_RAIL_WIDTH: f32 = 220.0;
const THREAD_RAIL_MIN_WIDTH: f32 = 180.0;
const THREAD_RAIL_MAX_WIDTH: f32 = 236.0;
const THREAD_RAIL_NEW_BUTTON_SIZE: f32 = 22.0;
const THREAD_RAIL_ROW_HEIGHT: f32 = 44.0;
const THREAD_RAIL_ROW_GAP: f32 = 8.0;
const THREAD_RAIL_VIEWPORT_INSET_X: f32 = 10.0;
const THREAD_RAIL_VIEWPORT_INSET_Y: f32 = 8.0;
const EMPTY_STATE_CARD_HEIGHT: f32 = 84.0;
const TERMINAL_PANEL_MAX_HEIGHT: f32 = 200.0;
const TASK_THREAD_ROW_GAP: f32 = 12.0;
const TASK_THREAD_LINE_HEIGHT: f32 = 14.0;
const TASK_THREAD_BOTTOM_PADDING: f32 = 28.0;
const TASK_THREAD_BUBBLE_MAX_WIDTH_RATIO: f32 = 0.74;
const TASK_THREAD_BUBBLE_PAD_X: f32 = 12.0;
const TASK_THREAD_BUBBLE_PAD_Y: f32 = 10.0;
const TASK_THREAD_STATUS_LINE_HEIGHT: f32 = 10.0;
const CODING_AGENT_SEND_ICON_SVG_RAW: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 640 640"><path fill="#FFFFFF" d="M342.6 73.4C330.1 60.9 309.8 60.9 297.3 73.4L137.3 233.4C124.8 245.9 124.8 266.2 137.3 278.7C149.8 291.2 170.1 291.2 182.6 278.7L288 173.3L288 544C288 561.7 302.3 576 320 576C337.7 576 352 561.7 352 544L352 173.3L457.4 278.7C469.9 291.2 490.2 291.2 502.7 278.7C515.2 266.2 515.2 245.9 502.7 233.4L342.7 73.4z"/></svg>"##;

#[derive(Clone, Copy)]
struct CodingAgentActionLayout {
    repo_prev: Bounds,
    repo_selector: Bounds,
    repo_next: Bounds,
    start_task: Bounds,
    review: Bounds,
}

#[derive(Clone, Copy)]
struct CodingAgentBodyLayout {
    left: Bounds,
    right: Bounds,
    timeline: Bounds,
    terminal: Bounds,
    composer: Bounds,
}

#[derive(Clone, Copy)]
struct CodingAgentComposerLayout {
    input: Bounds,
    send: Bounds,
    interrupt: Bounds,
    folder_selector: Bounds,
}

struct TimelineEntry {
    kind: &'static str,
    title: String,
    detail: Option<String>,
    accent: Hsla,
}

struct TaskThreadItem {
    speaker: &'static str,
    body: String,
    detail: String,
    accent: Hsla,
    running: bool,
}

struct CodingAgentThreadEntry {
    thread_id: String,
    title: String,
    subtitle: String,
    status: String,
    accent: Hsla,
    active: bool,
    running: bool,
}

struct ApprovalBarSummary {
    total: usize,
    title: String,
    detail: String,
    accent: Hsla,
}

struct ApprovalDetailCard {
    kind: &'static str,
    thread_label: String,
    headline: String,
    detail: String,
    queue_title: String,
    queue_detail: String,
    queue_aux: Option<String>,
    helper: String,
    accent: Hsla,
    rows: Vec<ApprovalDetailRow>,
}

struct ApprovalDetailRow {
    label: &'static str,
    value: String,
}

pub fn paint(
    content_bounds: Bounds,
    pane_state: &CodingAgentPaneState,
    autopilot_chat: &AutopilotChatState,
    inputs: &mut CodingAgentPaneInputs,
    paint: &mut PaintContext,
) {
    let selected_project_id = resolved_selected_project_id(pane_state, autopilot_chat);
    let selected_project = selected_project_id
        .as_deref()
        .and_then(|id| autopilot_chat.project_registry.get(id));
    let selected_workspace_root =
        resolved_selected_workspace_root(pane_state, selected_project);
    let selected_thread_id =
        resolved_selected_thread_id(pane_state, autopilot_chat, selected_project);
    let body = body_layout(content_bounds);
    let interrupt_enabled = interrupt_enabled(pane_state, autopilot_chat);

    paint_thread_rail(
        body.left,
        pane_state,
        autopilot_chat,
        selected_project,
        selected_workspace_root.as_deref(),
        paint,
    );
    paint_task_thread_contents(
        body.composer,
        pane_state,
        autopilot_chat,
        selected_workspace_root.as_deref(),
        selected_thread_id.as_deref(),
        paint,
    );
    paint_mission_control_section_panel(
        body.terminal,
        "TERMINAL",
        Hsla::from_hex(BADGE_WARNING_ACCENT),
        false,
        paint,
    );
    paint_terminal_contents(
        body.terminal,
        pane_state,
        autopilot_chat,
        inputs,
        selected_workspace_root.as_deref(),
        selected_thread_id.as_deref(),
        paint,
    );
    paint_composer_bar(
        body.composer,
        pane_state,
        inputs,
        interrupt_enabled,
        selected_project,
        selected_workspace_root.as_deref(),
        paint,
    );
}

pub fn repo_prev_button_bounds(content_bounds: Bounds) -> Bounds {
    let _ = content_bounds;
    Bounds::new(0.0, 0.0, 0.0, 0.0)
}

pub fn repo_selector_bounds(content_bounds: Bounds) -> Bounds {
    composer_layout(body_layout(content_bounds).composer).folder_selector
}

pub fn repo_next_button_bounds(content_bounds: Bounds) -> Bounds {
    let _ = content_bounds;
    Bounds::new(0.0, 0.0, 0.0, 0.0)
}

pub fn start_task_button_bounds(content_bounds: Bounds) -> Bounds {
    action_layout(content_bounds).start_task
}

pub fn review_button_bounds(content_bounds: Bounds) -> Bounds {
    action_layout(content_bounds).review
}

pub fn start_task_enabled(
    pane_state: &CodingAgentPaneState,
    autopilot_chat: &AutopilotChatState,
) -> bool {
    resolved_selected_workspace_root(
        pane_state,
        resolved_selected_project_id(pane_state, autopilot_chat)
            .as_deref()
            .and_then(|id| autopilot_chat.project_registry.get(id)),
    )
    .is_some()
}

pub fn review_enabled(
    pane_state: &CodingAgentPaneState,
    autopilot_chat: &AutopilotChatState,
) -> bool {
    let selected_project_id = resolved_selected_project_id(pane_state, autopilot_chat);
    let selected_project = selected_project_id
        .as_deref()
        .and_then(|id| autopilot_chat.project_registry.get(id));
    let selected_thread_id =
        resolved_selected_thread_id(pane_state, autopilot_chat, selected_project);
    let review_running = selected_thread_id
        .as_deref()
        .and_then(|thread_id| autopilot_chat.review_artifact_for_thread(thread_id))
        .is_some_and(|artifact| artifact.status.eq_ignore_ascii_case("running"));
    selected_thread_id.is_some() && !review_running
}

pub fn review_rail_enabled(
    pane_state: &CodingAgentPaneState,
    autopilot_chat: &AutopilotChatState,
) -> bool {
    let selected_project_id = resolved_selected_project_id(pane_state, autopilot_chat);
    let selected_project = selected_project_id
        .as_deref()
        .and_then(|id| autopilot_chat.project_registry.get(id));
    let selected_thread_id =
        resolved_selected_thread_id(pane_state, autopilot_chat, selected_project);
    let diff_artifact = selected_thread_id
        .as_deref()
        .and_then(|thread_id| autopilot_chat.diff_artifact_for_thread(thread_id));
    let review_artifact = selected_thread_id
        .as_deref()
        .and_then(|thread_id| autopilot_chat.review_artifact_for_thread(thread_id));
    selected_thread_id.is_some() || diff_artifact.is_some() || review_artifact.is_some()
}

pub fn composer_send_enabled(
    pane_state: &CodingAgentPaneState,
    autopilot_chat: &AutopilotChatState,
    inputs: &CodingAgentPaneInputs,
) -> bool {
    resolved_selected_workspace_root(
        pane_state,
        resolved_selected_project_id(pane_state, autopilot_chat)
            .as_deref()
            .and_then(|id| autopilot_chat.project_registry.get(id)),
    )
    .is_some()
        && pane_state.pending_thread_start_prompt.is_none()
        && !inputs.composer.get_value().trim().is_empty()
}

pub fn interrupt_enabled(
    pane_state: &CodingAgentPaneState,
    autopilot_chat: &AutopilotChatState,
) -> bool {
    let selected_project_id = resolved_selected_project_id(pane_state, autopilot_chat);
    let selected_project = selected_project_id
        .as_deref()
        .and_then(|id| autopilot_chat.project_registry.get(id));
    let selected_thread_id =
        resolved_selected_thread_id(pane_state, autopilot_chat, selected_project);
    selected_thread_id
        .as_deref()
        .zip(autopilot_chat.active_turn_metadata())
        .is_some_and(|(selected_thread_id, metadata)| metadata.thread_id == selected_thread_id)
}

pub fn terminal_input_enabled(
    pane_state: &CodingAgentPaneState,
    autopilot_chat: &AutopilotChatState,
) -> bool {
    let selected_project_id = resolved_selected_project_id(pane_state, autopilot_chat);
    let selected_project = selected_project_id
        .as_deref()
        .and_then(|id| autopilot_chat.project_registry.get(id));
    let has_workspace = resolved_selected_workspace_root(pane_state, selected_project).is_some();
    let selected_thread_id =
        resolved_selected_thread_id(pane_state, autopilot_chat, selected_project);
    terminal_input_enabled_for_selection(
        autopilot_chat,
        has_workspace,
        selected_thread_id.as_deref(),
    )
}

fn terminal_input_enabled_for_selection(
    autopilot_chat: &AutopilotChatState,
    has_workspace: bool,
    selected_thread_id: Option<&str>,
) -> bool {
    if !has_workspace {
        return false;
    }
    let Some(thread_id) = selected_thread_id else {
        return true;
    };
    !autopilot_chat
        .active_turn_metadata()
        .is_some_and(|metadata| metadata.thread_id == thread_id)
}

pub fn terminal_input_bounds(content_bounds: Bounds) -> Bounds {
    terminal_footer_layout(terminal_footer_bounds(body_layout(content_bounds).terminal)).0
}

pub fn terminal_shell_bounds(content_bounds: Bounds) -> Bounds {
    terminal_body_bounds(body_layout(content_bounds).terminal)
}

pub fn task_thread_scroll_viewport_bounds(content_bounds: Bounds) -> Bounds {
    task_thread_viewport_bounds(body_layout(content_bounds).composer)
}

pub fn terminal_send_button_bounds(content_bounds: Bounds) -> Bounds {
    terminal_footer_layout(terminal_footer_bounds(body_layout(content_bounds).terminal)).1
}

pub fn terminal_send_enabled(
    pane_state: &CodingAgentPaneState,
    autopilot_chat: &AutopilotChatState,
    inputs: &CodingAgentPaneInputs,
) -> bool {
    terminal_input_enabled(pane_state, autopilot_chat)
        && !inputs.terminal_input.get_value().trim().is_empty()
}

pub fn approval_actions_enabled(
    pane_state: &CodingAgentPaneState,
    autopilot_chat: &AutopilotChatState,
) -> bool {
    let selected_project_id = resolved_selected_project_id(pane_state, autopilot_chat);
    let selected_project = selected_project_id
        .as_deref()
        .and_then(|id| autopilot_chat.project_registry.get(id));
    let selected_thread_id =
        resolved_selected_thread_id(pane_state, autopilot_chat, selected_project);
    approval_bar_summary(
        autopilot_chat,
        selected_project,
        selected_thread_id.as_deref(),
    )
    .total
        > 0
}

pub fn composer_input_bounds(content_bounds: Bounds) -> Bounds {
    composer_layout(body_layout(content_bounds).composer).input
}

pub fn composer_send_button_bounds(content_bounds: Bounds) -> Bounds {
    composer_layout(body_layout(content_bounds).composer).send
}

pub fn composer_interrupt_button_bounds(content_bounds: Bounds) -> Bounds {
    composer_layout(body_layout(content_bounds).composer).interrupt
}

pub fn approval_inspect_button_bounds(content_bounds: Bounds) -> Bounds {
    approval_button_bounds(approval_bar_bounds(content_bounds)).0
}

pub fn approval_deny_button_bounds(content_bounds: Bounds) -> Bounds {
    approval_button_bounds(approval_bar_bounds(content_bounds)).1
}

pub fn approval_accept_button_bounds(content_bounds: Bounds) -> Bounds {
    approval_button_bounds(approval_bar_bounds(content_bounds)).2
}

pub fn right_rail_changed_files_tab_bounds(content_bounds: Bounds) -> Bounds {
    review_tab_bounds(right_panel_bounds(content_bounds)).0
}

pub fn right_rail_diff_tab_bounds(content_bounds: Bounds) -> Bounds {
    review_tab_bounds(right_panel_bounds(content_bounds)).1
}

pub fn changed_files_scroll_viewport_bounds(content_bounds: Bounds) -> Bounds {
    review_body_viewport_bounds(right_panel_bounds(content_bounds))
}

pub fn approval_detail_scroll_viewport_bounds(content_bounds: Bounds) -> Bounds {
    approval_detail_viewport_bounds(right_panel_bounds(content_bounds))
}

pub fn approval_detail_content_height(
    content_bounds: Bounds,
    pane_state: &CodingAgentPaneState,
    autopilot_chat: &AutopilotChatState,
) -> f32 {
    let selected_project_id = resolved_selected_project_id(pane_state, autopilot_chat);
    let selected_project = selected_project_id
        .as_deref()
        .and_then(|id| autopilot_chat.project_registry.get(id));
    let selected_thread_id =
        resolved_selected_thread_id(pane_state, autopilot_chat, selected_project);
    let viewport = approval_detail_scroll_viewport_bounds(content_bounds);
    approval_detail_card(
        autopilot_chat,
        selected_project,
        selected_thread_id.as_deref(),
    )
    .map(|detail| approval_detail_content_height_for_card(&detail, viewport))
    .unwrap_or(viewport.size.height)
}

pub fn changed_files_content_height(
    content_bounds: Bounds,
    pane_state: &CodingAgentPaneState,
    autopilot_chat: &AutopilotChatState,
) -> f32 {
    let selected_project_id = resolved_selected_project_id(pane_state, autopilot_chat);
    let selected_project = selected_project_id
        .as_deref()
        .and_then(|id| autopilot_chat.project_registry.get(id));
    let selected_thread_id =
        resolved_selected_thread_id(pane_state, autopilot_chat, selected_project);
    let artifact = selected_thread_id
        .as_deref()
        .and_then(|thread_id| autopilot_chat.diff_artifact_for_thread(thread_id));
    let viewport = changed_files_scroll_viewport_bounds(content_bounds);
    review_content_height_for_artifact(artifact, viewport)
}

pub fn diff_viewer_content_height(
    content_bounds: Bounds,
    pane_state: &CodingAgentPaneState,
    autopilot_chat: &AutopilotChatState,
) -> f32 {
    let selected_project_id = resolved_selected_project_id(pane_state, autopilot_chat);
    let selected_project = selected_project_id
        .as_deref()
        .and_then(|id| autopilot_chat.project_registry.get(id));
    let selected_thread_id =
        resolved_selected_thread_id(pane_state, autopilot_chat, selected_project);
    let artifact = selected_thread_id
        .as_deref()
        .and_then(|thread_id| autopilot_chat.diff_artifact_for_thread(thread_id));
    let viewport = changed_files_scroll_viewport_bounds(content_bounds);
    diff_viewer_content_height_for_artifact(
        artifact,
        resolved_selected_diff_file_path(pane_state, artifact),
        viewport,
    )
}

pub fn changed_file_index_at_point(
    content_bounds: Bounds,
    pane_state: &CodingAgentPaneState,
    autopilot_chat: &AutopilotChatState,
    point: Point,
) -> Option<usize> {
    if pane_state.right_rail_tab != CodingAgentRailTab::ChangedFiles {
        return None;
    }
    if pane_state.approval_drawer_open {
        return None;
    }
    let selected_project_id = resolved_selected_project_id(pane_state, autopilot_chat);
    let selected_project = selected_project_id
        .as_deref()
        .and_then(|id| autopilot_chat.project_registry.get(id));
    let selected_thread_id =
        resolved_selected_thread_id(pane_state, autopilot_chat, selected_project);
    let artifact = selected_thread_id
        .as_deref()
        .and_then(|thread_id| autopilot_chat.diff_artifact_for_thread(thread_id))?;
    let viewport = changed_files_scroll_viewport_bounds(content_bounds);
    if !viewport.contains(point) {
        return None;
    }
    let content_height = review_content_height_for_artifact(Some(artifact), viewport);
    let max_scroll = (content_height - viewport.size.height).max(0.0);
    let scroll_offset = pane_state.diff_scroll_offset.clamp(0.0, max_scroll);
    let mut y = viewport.origin.y - scroll_offset;
    for (index, _) in artifact.files.iter().enumerate() {
        let row_bounds = Bounds::new(
            viewport.origin.x,
            y,
            viewport.size.width - 6.0,
            REVIEW_FILE_ROW_HEIGHT,
        );
        if row_bounds.contains(point) {
            return Some(index);
        }
        y += REVIEW_FILE_ROW_HEIGHT + REVIEW_FILE_ROW_GAP;
    }
    None
}

pub fn timeline_scroll_viewport_bounds(content_bounds: Bounds) -> Bounds {
    let timeline = body_layout(content_bounds).timeline;
    Bounds::new(
        timeline.origin.x + TIMELINE_VIEWPORT_INSET_X,
        timeline.origin.y + SECTION_HEADER_HEIGHT + TIMELINE_VIEWPORT_INSET_Y,
        (timeline.size.width - TIMELINE_VIEWPORT_INSET_X * 2.0).max(0.0),
        (timeline.size.height - SECTION_HEADER_HEIGHT - TIMELINE_VIEWPORT_INSET_Y * 2.0).max(0.0),
    )
}

pub fn timeline_content_height(
    content_bounds: Bounds,
    pane_state: &CodingAgentPaneState,
    autopilot_chat: &AutopilotChatState,
) -> f32 {
    let selected_project_id = resolved_selected_project_id(pane_state, autopilot_chat);
    let selected_project = selected_project_id
        .as_deref()
        .and_then(|id| autopilot_chat.project_registry.get(id));
    let selected_thread_id =
        resolved_selected_thread_id(pane_state, autopilot_chat, selected_project);
    let selected_thread = selected_thread_id
        .as_deref()
        .and_then(|thread_id| autopilot_chat.thread_metadata.get(thread_id));
    let entries = timeline_entries(
        autopilot_chat,
        selected_project,
        selected_thread,
        selected_thread_id.as_deref(),
    );
    timeline_content_height_for_entries(entries.len(), timeline_scroll_viewport_bounds(content_bounds))
}

pub fn terminal_scroll_viewport_bounds(content_bounds: Bounds) -> Bounds {
    let terminal = body_layout(content_bounds).terminal;
    terminal_output_viewport_bounds(terminal)
}

pub fn terminal_content_height(
    content_bounds: Bounds,
    pane_state: &CodingAgentPaneState,
    autopilot_chat: &AutopilotChatState,
) -> f32 {
    let selected_project_id = resolved_selected_project_id(pane_state, autopilot_chat);
    let selected_project = selected_project_id
        .as_deref()
        .and_then(|id| autopilot_chat.project_registry.get(id));
    let session = resolved_selected_workspace_root(pane_state, selected_project)
        .map(|workspace_root| coding_agent_terminal_session_id_for_workspace(workspace_root.as_str()))
        .as_deref()
        .and_then(|session_id| autopilot_chat.terminal_session_for_thread(session_id));
    terminal_content_height_for_session(session, terminal_scroll_viewport_bounds(content_bounds))
}

pub fn task_thread_content_height(
    content_bounds: Bounds,
    pane_state: &CodingAgentPaneState,
    autopilot_chat: &AutopilotChatState,
) -> f32 {
    let selected_project_id = resolved_selected_project_id(pane_state, autopilot_chat);
    let selected_project = selected_project_id
        .as_deref()
        .and_then(|id| autopilot_chat.project_registry.get(id));
    let selected_workspace_root =
        resolved_selected_workspace_root(pane_state, selected_project);
    let selected_thread_id =
        resolved_selected_thread_id(pane_state, autopilot_chat, selected_project);
    let viewport = task_thread_scroll_viewport_bounds(content_bounds);
    let items = task_thread_items(
        pane_state,
        autopilot_chat,
        selected_workspace_root.as_deref(),
        selected_thread_id.as_deref(),
    );
    task_thread_content_height_for_items(items.as_slice(), viewport)
}

fn action_layout(content_bounds: Bounds) -> CodingAgentActionLayout {
    let actions_top = content_bounds.origin.y + 22.0;
    let total_width = (content_bounds.size.width - PADDING * 2.0).max(320.0);
    let repo_group_width = total_width;
    let repo_x = content_bounds.origin.x + PADDING;
    let selector_width = (repo_group_width - MINI_BUTTON_WIDTH * 2.0 - MINI_GAP * 2.0).max(180.0);

    CodingAgentActionLayout {
        repo_prev: Bounds::new(repo_x, actions_top, MINI_BUTTON_WIDTH, HEADER_ACTION_HEIGHT),
        repo_selector: Bounds::new(
            repo_x + MINI_BUTTON_WIDTH + MINI_GAP,
            actions_top,
            selector_width,
            HEADER_ACTION_HEIGHT,
        ),
        repo_next: Bounds::new(
            repo_x + MINI_BUTTON_WIDTH + MINI_GAP + selector_width + MINI_GAP,
            actions_top,
            MINI_BUTTON_WIDTH,
            HEADER_ACTION_HEIGHT,
        ),
        start_task: Bounds::new(0.0, 0.0, 0.0, 0.0),
        review: Bounds::new(0.0, 0.0, 0.0, 0.0),
    }
}

fn summary_bounds(content_bounds: Bounds) -> Bounds {
    Bounds::new(
        content_bounds.origin.x + PADDING,
        approval_bar_bounds(content_bounds).max_y() + 14.0,
        (content_bounds.size.width - PADDING * 2.0).max(220.0),
        SUMMARY_HEIGHT,
    )
}

fn body_layout(content_bounds: Bounds) -> CodingAgentBodyLayout {
    let body_top = content_bounds.origin.y + 18.0;
    let body_height = (content_bounds.max_y() - body_top - PADDING).max(220.0);
    let full_width = (content_bounds.size.width - PADDING * 2.0).max(280.0);
    let outer = Bounds::new(
        content_bounds.origin.x + PADDING,
        body_top,
        full_width,
        body_height,
    );
    let min_main_width = 220.0;
    let rail_width = THREAD_RAIL_WIDTH
        .min(THREAD_RAIL_MAX_WIDTH)
        .min((outer.size.width - GAP - min_main_width).max(THREAD_RAIL_MIN_WIDTH))
        .max((outer.size.width * 0.24).min(THREAD_RAIL_MIN_WIDTH));
    let left = Bounds::new(outer.origin.x, outer.origin.y, rail_width, outer.size.height);
    let right = Bounds::new(
        left.max_x() + GAP,
        outer.origin.y,
        (outer.max_x() - left.max_x() - GAP).max(220.0),
        outer.size.height,
    );
    let terminal_height = {
        let available = right.size.height.max(0.0);
        if available <= 0.0 {
            0.0
        } else {
            available.min(TERMINAL_PANEL_MAX_HEIGHT).max(160.0).min(available)
        }
    };
    let terminal = Bounds::new(
        right.origin.x,
        (right.max_y() - terminal_height).max(right.origin.y),
        right.size.width,
        terminal_height,
    );
    let composer = Bounds::new(
        right.origin.x,
        right.origin.y,
        right.size.width,
        (terminal.origin.y - right.origin.y - GAP).max(0.0),
    );
    let timeline = Bounds::new(0.0, 0.0, 0.0, 0.0);

    CodingAgentBodyLayout {
        left,
        right,
        timeline,
        terminal,
        composer,
    }
}

fn composer_layout(bounds: Bounds) -> CodingAgentComposerLayout {
    let folder_y = bounds.max_y() - COMPOSER_FOLDER_HEIGHT;
    let row_y = folder_y - MINI_GAP - COMPOSER_ROW_HEIGHT;
    let send = Bounds::new(
        bounds.max_x() - COMPOSER_SEND_WIDTH,
        row_y,
        COMPOSER_SEND_WIDTH,
        COMPOSER_ROW_HEIGHT,
    );
    let input = Bounds::new(
        bounds.origin.x,
        row_y,
        (send.origin.x - bounds.origin.x - MINI_GAP).max(120.0),
        COMPOSER_ROW_HEIGHT,
    );
    let interrupt = Bounds::new(0.0, 0.0, 0.0, 0.0);
    let folder_selector = Bounds::new(
        bounds.origin.x,
        folder_y,
        bounds.size.width.min(COMPOSER_FOLDER_WIDTH).max(180.0),
        COMPOSER_FOLDER_HEIGHT,
    );
    CodingAgentComposerLayout {
        input,
        send,
        interrupt,
        folder_selector,
    }
}

fn thread_rail_header_bounds(panel_bounds: Bounds) -> Bounds {
    Bounds::new(
        panel_bounds.origin.x,
        panel_bounds.origin.y,
        panel_bounds.size.width,
        SECTION_HEADER_HEIGHT,
    )
}

pub fn thread_rail_new_button_bounds(content_bounds: Bounds) -> Bounds {
    let rail = body_layout(content_bounds).left;
    let header = thread_rail_header_bounds(rail);
    Bounds::new(
        header.max_x() - THREAD_RAIL_NEW_BUTTON_SIZE - 10.0,
        header.origin.y + (header.size.height - THREAD_RAIL_NEW_BUTTON_SIZE) * 0.5,
        THREAD_RAIL_NEW_BUTTON_SIZE,
        THREAD_RAIL_NEW_BUTTON_SIZE,
    )
}

pub fn thread_rail_scroll_viewport_bounds(content_bounds: Bounds) -> Bounds {
    let rail = body_layout(content_bounds).left;
    thread_rail_viewport_bounds(rail)
}

fn thread_rail_viewport_bounds(rail: Bounds) -> Bounds {
    Bounds::new(
        rail.origin.x + THREAD_RAIL_VIEWPORT_INSET_X,
        rail.origin.y + SECTION_HEADER_HEIGHT + THREAD_RAIL_VIEWPORT_INSET_Y,
        (rail.size.width - THREAD_RAIL_VIEWPORT_INSET_X * 2.0).max(0.0),
        (rail.size.height - SECTION_HEADER_HEIGHT - THREAD_RAIL_VIEWPORT_INSET_Y * 2.0).max(0.0),
    )
}

fn thread_rail_row_bounds(viewport: Bounds, scroll_offset: f32, index: usize) -> Bounds {
    Bounds::new(
        viewport.origin.x,
        viewport.origin.y - scroll_offset + index as f32 * (THREAD_RAIL_ROW_HEIGHT + THREAD_RAIL_ROW_GAP),
        (viewport.size.width - 4.0).max(0.0),
        THREAD_RAIL_ROW_HEIGHT,
    )
}

fn thread_entry_title(
    autopilot_chat: &AutopilotChatState,
    thread_id: &str,
    metadata: Option<&AutopilotThreadMetadata>,
) -> String {
    metadata
        .and_then(|value| value.thread_name.as_deref())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| summarize_line(name, 28))
        .or_else(|| {
            autopilot_chat
                .suggested_thread_name(thread_id)
                .map(|name| summarize_line(name.as_str(), 28))
        })
        .unwrap_or_else(|| summarize_line(thread_id, 18))
}

fn thread_entry_preview(
    autopilot_chat: &AutopilotChatState,
    thread_id: &str,
    metadata: Option<&AutopilotThreadMetadata>,
) -> String {
    metadata
        .and_then(|value| value.preview.as_deref())
        .map(str::trim)
        .filter(|preview| !preview.is_empty())
        .map(|preview| summarize_line(preview, 38))
        .or_else(|| {
            autopilot_chat
                .cached_thread_messages(thread_id)
                .and_then(|messages| messages.last())
                .map(task_thread_message_body)
                .map(|body| summarize_line(body.as_str(), 38))
        })
        .unwrap_or_else(|| "No messages yet".to_string())
}

fn thread_entry_status(
    autopilot_chat: &AutopilotChatState,
    thread_id: &str,
    metadata: Option<&AutopilotThreadMetadata>,
) -> (String, bool, Hsla) {
    let approval_waiting = autopilot_chat
        .pending_command_approvals
        .iter()
        .any(|request| request.thread_id == thread_id)
        || autopilot_chat
            .pending_file_change_approvals
            .iter()
            .any(|request| request.thread_id == thread_id)
        || autopilot_chat
            .pending_tool_calls
            .iter()
            .any(|request| request.thread_id == thread_id)
        || autopilot_chat
            .pending_tool_user_input
            .iter()
            .any(|request| request.thread_id == thread_id);
    if approval_waiting {
        return (
            "approval".to_string(),
            false,
            Hsla::from_hex(BADGE_WARNING_ACCENT),
        );
    }

    let running = autopilot_chat
        .active_turn_metadata()
        .is_some_and(|turn| turn.thread_id == thread_id);
    let status = if running {
        "running".to_string()
    } else {
        metadata
            .and_then(|value| value.status.as_deref())
            .map(|status| status.replace('_', " "))
            .filter(|status| !status.trim().is_empty())
            .unwrap_or_else(|| "idle".to_string())
    };
    let normalized = status.trim().to_ascii_lowercase();
    let accent = if running
        || normalized == "queued"
        || normalized == "starting"
        || normalized == "resuming"
    {
        Hsla::from_hex(BADGE_RUNNING_ACCENT)
    } else if normalized.contains("error")
        || normalized.contains("fail")
        || normalized.contains("reject")
    {
        Hsla::from_hex(BADGE_ERROR_ACCENT)
    } else if normalized == "done"
        || normalized == "complete"
        || normalized == "completed"
        || normalized == "idle"
        || normalized == "ready"
    {
        Hsla::from_hex(BADGE_SUCCESS_ACCENT)
    } else {
        Hsla::from_hex(BADGE_MUTED_ACCENT)
    };
    (status, running, accent)
}

fn thread_ids_for_selection(
    autopilot_chat: &AutopilotChatState,
    selected_project: Option<&AutopilotProjectIdentity>,
    selected_workspace_root: Option<&str>,
) -> Vec<String> {
    let mut thread_ids = if let Some(project) = selected_project {
        project.thread_ids.clone()
    } else if let Some(workspace_root) = selected_workspace_root {
        autopilot_chat
            .threads
            .iter()
            .filter(|thread_id| {
                autopilot_chat
                    .thread_metadata
                    .get(thread_id.as_str())
                    .and_then(|metadata| metadata.workspace_root.as_deref().or(metadata.cwd.as_deref()))
                    == Some(workspace_root)
            })
            .cloned()
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    thread_ids.sort_by(|left, right| {
        autopilot_chat
            .thread_metadata
            .get(right)
            .and_then(|metadata| metadata.updated_at)
            .unwrap_or_default()
            .cmp(
                &autopilot_chat
                    .thread_metadata
                    .get(left)
                    .and_then(|metadata| metadata.updated_at)
                    .unwrap_or_default(),
            )
    });
    thread_ids
}

fn coding_agent_thread_entries(
    pane_state: &CodingAgentPaneState,
    autopilot_chat: &AutopilotChatState,
    selected_project: Option<&AutopilotProjectIdentity>,
    selected_workspace_root: Option<&str>,
) -> Vec<CodingAgentThreadEntry> {
    thread_ids_for_selection(autopilot_chat, selected_project, selected_workspace_root)
        .into_iter()
        .map(|thread_id| {
            let metadata = autopilot_chat.thread_metadata.get(thread_id.as_str());
            let (status, running, accent) =
                thread_entry_status(autopilot_chat, thread_id.as_str(), metadata);
            CodingAgentThreadEntry {
                title: thread_entry_title(autopilot_chat, thread_id.as_str(), metadata),
                subtitle: thread_entry_preview(autopilot_chat, thread_id.as_str(), metadata),
                accent,
                active: pane_state.active_thread_id.as_deref() == Some(thread_id.as_str()),
                running,
                status,
                thread_id,
            }
        })
        .collect()
}

pub fn thread_rail_content_height(
    content_bounds: Bounds,
    pane_state: &CodingAgentPaneState,
    autopilot_chat: &AutopilotChatState,
) -> f32 {
    let selected_project_id = resolved_selected_project_id(pane_state, autopilot_chat);
    let selected_project = selected_project_id
        .as_deref()
        .and_then(|id| autopilot_chat.project_registry.get(id));
    let selected_workspace_root =
        resolved_selected_workspace_root(pane_state, selected_project);
    let viewport = thread_rail_scroll_viewport_bounds(content_bounds);
    let count = coding_agent_thread_entries(
        pane_state,
        autopilot_chat,
        selected_project,
        selected_workspace_root.as_deref(),
    )
    .len();
    if count == 0 {
        viewport.size.height
    } else {
        count as f32 * THREAD_RAIL_ROW_HEIGHT
            + count.saturating_sub(1) as f32 * THREAD_RAIL_ROW_GAP
            + 8.0
    }
}

pub fn thread_rail_row_index_at_point(
    content_bounds: Bounds,
    pane_state: &CodingAgentPaneState,
    autopilot_chat: &AutopilotChatState,
    point: Point,
) -> Option<usize> {
    let selected_project_id = resolved_selected_project_id(pane_state, autopilot_chat);
    let selected_project = selected_project_id
        .as_deref()
        .and_then(|id| autopilot_chat.project_registry.get(id));
    let selected_workspace_root =
        resolved_selected_workspace_root(pane_state, selected_project);
    let entries = coding_agent_thread_entries(
        pane_state,
        autopilot_chat,
        selected_project,
        selected_workspace_root.as_deref(),
    );
    let viewport = thread_rail_scroll_viewport_bounds(content_bounds);
    if !viewport.contains(point) {
        return None;
    }
    let content_height = thread_rail_content_height(content_bounds, pane_state, autopilot_chat);
    let max_scroll = (content_height - viewport.size.height).max(0.0);
    let scroll_offset = pane_state.thread_list_scroll_offset.clamp(0.0, max_scroll);
    (0..entries.len()).find(|index| thread_rail_row_bounds(viewport, scroll_offset, *index).contains(point))
}

fn task_thread_viewport_bounds(panel_bounds: Bounds) -> Bounds {
    let layout = composer_layout(panel_bounds);
    let top = panel_bounds.origin.y + 6.0;
    let bottom = (layout.input.origin.y - 20.0).max(top);
    Bounds::new(
        panel_bounds.origin.x + 12.0,
        top,
        (panel_bounds.size.width - 24.0).max(0.0),
        (bottom - top).max(0.0),
    )
}

fn right_panel_bounds(content_bounds: Bounds) -> Bounds {
    body_layout(content_bounds).right
}

fn approval_bar_bounds(content_bounds: Bounds) -> Bounds {
    let actions = action_layout(content_bounds);
    Bounds::new(
        content_bounds.origin.x + PADDING,
        actions.repo_selector.max_y() + 12.0,
        (content_bounds.size.width - PADDING * 2.0).max(240.0),
        APPROVAL_BAR_HEIGHT,
    )
}

fn review_summary_bounds(panel_bounds: Bounds) -> Bounds {
    Bounds::new(
        panel_bounds.origin.x + REVIEW_VIEWPORT_INSET_X,
        panel_bounds.origin.y + SECTION_HEADER_HEIGHT + REVIEW_VIEWPORT_INSET_Y,
        (panel_bounds.size.width - REVIEW_VIEWPORT_INSET_X * 2.0).max(0.0),
        REVIEW_SUMMARY_HEIGHT.min(
            (panel_bounds.size.height - SECTION_HEADER_HEIGHT - REVIEW_VIEWPORT_INSET_Y * 2.0)
                .max(0.0),
        ),
    )
}

fn review_tab_bounds(panel_bounds: Bounds) -> (Bounds, Bounds) {
    let summary = review_summary_bounds(panel_bounds);
    let full = Bounds::new(
        panel_bounds.origin.x + REVIEW_VIEWPORT_INSET_X,
        summary.max_y() + 8.0,
        (panel_bounds.size.width - REVIEW_VIEWPORT_INSET_X * 2.0).max(0.0),
        REVIEW_TAB_HEIGHT,
    );
    let tab_width = ((full.size.width - MINI_GAP) * 0.5).max(60.0);
    (
        Bounds::new(full.origin.x, full.origin.y, tab_width, full.size.height),
        Bounds::new(
            full.origin.x + tab_width + MINI_GAP,
            full.origin.y,
            (full.size.width - tab_width - MINI_GAP).max(0.0),
            full.size.height,
        ),
    )
}

fn review_body_viewport_bounds(panel_bounds: Bounds) -> Bounds {
    let (_, diff_tab) = review_tab_bounds(panel_bounds);
    Bounds::new(
        panel_bounds.origin.x + REVIEW_VIEWPORT_INSET_X,
        diff_tab.max_y() + 8.0,
        (panel_bounds.size.width - REVIEW_VIEWPORT_INSET_X * 2.0).max(0.0),
        (panel_bounds.max_y() - diff_tab.max_y() - 16.0).max(0.0),
    )
}

fn approval_detail_viewport_bounds(panel_bounds: Bounds) -> Bounds {
    Bounds::new(
        panel_bounds.origin.x + REVIEW_VIEWPORT_INSET_X,
        panel_bounds.origin.y + SECTION_HEADER_HEIGHT + REVIEW_VIEWPORT_INSET_Y,
        (panel_bounds.size.width - REVIEW_VIEWPORT_INSET_X * 2.0).max(0.0),
        (panel_bounds.size.height - SECTION_HEADER_HEIGHT - REVIEW_VIEWPORT_INSET_Y * 2.0).max(0.0),
    )
}

fn paint_repo_selector(
    selector_bounds: Bounds,
    selected_project: Option<&AutopilotProjectIdentity>,
    selected_workspace_root: Option<&str>,
    paint: &mut PaintContext,
) {
    paint_standard_input_frame(selector_bounds, paint);
    let value = selected_workspace_root
        .or_else(|| selected_project.map(|project| project.workspace_root.as_str()))
        .map(|workspace_root| summarize_line(workspace_root, 44))
        .unwrap_or("Select local folder".to_string());
    let clip_bounds = Bounds::new(
        selector_bounds.origin.x + 10.0,
        selector_bounds.origin.y + 6.0,
        selector_bounds.size.width - 20.0,
        selector_bounds.size.height - 12.0,
    );
    paint.scene.push_clip(clip_bounds);
    paint.scene.draw_text(paint.text.layout_mono(
        &value,
        Point::new(selector_bounds.origin.x + 12.0, selector_bounds.origin.y + 9.0),
        9.5,
        if selected_workspace_root.is_some() || selected_project.is_some() {
            theme::text::PRIMARY
        } else {
            theme::text::MUTED
        },
    ));
    paint.scene.pop_clip();
}

fn paint_passive_button(bounds: Bounds, label: &str, accent: Hsla, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::APP.with_alpha(0.68))
            .with_border(theme::border::DEFAULT.with_alpha(0.18), 1.0)
            .with_corner_radius(8.0),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        label,
        Point::new(bounds.origin.x + 12.0, bounds.origin.y + 10.0),
        11.0,
        accent.with_alpha(0.72),
    ));
}

fn paint_timeline_contents(
    panel_bounds: Bounds,
    pane_state: &CodingAgentPaneState,
    autopilot_chat: &AutopilotChatState,
    selected_project: Option<&AutopilotProjectIdentity>,
    selected_thread: Option<&AutopilotThreadMetadata>,
    selected_thread_id: Option<&str>,
    paint: &mut PaintContext,
) {
    let viewport = Bounds::new(
        panel_bounds.origin.x + TIMELINE_VIEWPORT_INSET_X,
        panel_bounds.origin.y + SECTION_HEADER_HEIGHT + TIMELINE_VIEWPORT_INSET_Y,
        (panel_bounds.size.width - TIMELINE_VIEWPORT_INSET_X * 2.0).max(0.0),
        (panel_bounds.size.height - SECTION_HEADER_HEIGHT - TIMELINE_VIEWPORT_INSET_Y * 2.0).max(0.0),
    );
    let entries = timeline_entries(
        autopilot_chat,
        selected_project,
        selected_thread,
        selected_thread_id,
    );
    let content_height = timeline_content_height_for_entries(entries.len(), viewport);
    let max_scroll = (content_height - viewport.size.height).max(0.0);
    let scroll_offset = pane_state.thread_scroll_offset.clamp(0.0, max_scroll);

    paint.scene.push_clip(viewport);
    let mut y = viewport.origin.y + 2.0 - scroll_offset;
    for entry in entries {
        let row_bounds = Bounds::new(
            viewport.origin.x,
            y,
            viewport.size.width - 6.0,
            TIMELINE_ROW_HEIGHT,
        );
        if row_bounds.max_y() >= viewport.origin.y && row_bounds.origin.y <= viewport.max_y() {
            paint_timeline_row(row_bounds, &entry, paint);
        }
        y += TIMELINE_ROW_HEIGHT + TIMELINE_ROW_GAP;
    }
    paint.scene.pop_clip();

    if max_scroll > f32::EPSILON {
        paint_timeline_scrollbar(viewport, scroll_offset, content_height, paint);
    }
}

fn approval_button_bounds(bar_bounds: Bounds) -> (Bounds, Bounds, Bounds) {
    let approve = Bounds::new(
        bar_bounds.max_x() - APPROVAL_BUTTON_APPROVE_WIDTH - 12.0,
        bar_bounds.origin.y + (bar_bounds.size.height - APPROVAL_BUTTON_HEIGHT) * 0.5,
        APPROVAL_BUTTON_APPROVE_WIDTH,
        APPROVAL_BUTTON_HEIGHT,
    );
    let deny = Bounds::new(
        approve.origin.x - MINI_GAP - APPROVAL_BUTTON_DENY_WIDTH,
        approve.origin.y,
        APPROVAL_BUTTON_DENY_WIDTH,
        APPROVAL_BUTTON_HEIGHT,
    );
    let inspect = Bounds::new(
        deny.origin.x - MINI_GAP - APPROVAL_BUTTON_INSPECT_WIDTH,
        approve.origin.y,
        APPROVAL_BUTTON_INSPECT_WIDTH,
        APPROVAL_BUTTON_HEIGHT,
    );
    (inspect, deny, approve)
}

fn coding_agent_terminal_session_id_for_workspace(workspace_root: &str) -> String {
    format!("coding-agent-terminal::{workspace_root}")
}

fn paint_terminal_contents(
    panel_bounds: Bounds,
    pane_state: &CodingAgentPaneState,
    autopilot_chat: &AutopilotChatState,
    inputs: &mut CodingAgentPaneInputs,
    selected_workspace_root: Option<&str>,
    selected_thread_id: Option<&str>,
    paint: &mut PaintContext,
) {
    let session = selected_workspace_root
        .map(coding_agent_terminal_session_id_for_workspace)
        .as_deref()
        .and_then(|session_id| autopilot_chat.terminal_session_for_thread(session_id));
    let shell_bounds = terminal_body_bounds(panel_bounds);
    let viewport = terminal_output_viewport_bounds(panel_bounds);
    let footer_bounds = terminal_footer_bounds(panel_bounds);
    let terminal_input_enabled = terminal_input_enabled_for_selection(
        autopilot_chat,
        selected_workspace_root.is_some(),
        selected_thread_id,
    );

    let content_height = terminal_content_height_for_session(session, viewport);
    let max_scroll = (content_height - viewport.size.height).max(0.0);
    let scroll_offset = pane_state.terminal_scroll_offset.clamp(0.0, max_scroll);
    paint.scene.draw_quad(
        Quad::new(shell_bounds)
            .with_background(theme::bg::APP.with_alpha(0.76))
            .with_corner_radius(5.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            shell_bounds.origin.x + 10.0,
            shell_bounds.origin.y + 10.0,
            (shell_bounds.size.width - 20.0).max(0.0),
            (shell_bounds.size.height - 20.0).max(0.0),
        ))
        .with_background(theme::bg::SURFACE.with_alpha(0.96))
        .with_border(theme::border::DEFAULT.with_alpha(0.30), 1.0)
        .with_corner_radius(8.0),
    );
    paint.scene.push_clip(viewport);
    paint_terminal_output_lines(viewport, scroll_offset, session, selected_thread_id, paint);
    paint.scene.pop_clip();
    if max_scroll > f32::EPSILON {
        paint_timeline_scrollbar(viewport, scroll_offset, content_height, paint);
    }
    paint_terminal_footer(
        footer_bounds,
        inputs,
        terminal_input_enabled,
        selected_thread_id,
        session,
        paint,
    );
}

fn paint_task_thread_contents(
    panel_bounds: Bounds,
    pane_state: &CodingAgentPaneState,
    autopilot_chat: &AutopilotChatState,
    selected_workspace_root: Option<&str>,
    selected_thread_id: Option<&str>,
    paint: &mut PaintContext,
) {
    let viewport = task_thread_viewport_bounds(panel_bounds);
    let items = task_thread_items(
        pane_state,
        autopilot_chat,
        selected_workspace_root,
        selected_thread_id,
    );
    let content_height = task_thread_content_height_for_items(items.as_slice(), viewport);
    let max_scroll = (content_height - viewport.size.height).max(0.0);
    let scroll_offset = pane_state.thread_scroll_offset.clamp(0.0, max_scroll);

    paint.scene.push_clip(viewport);
    let mut y = viewport.origin.y + 2.0 - scroll_offset;
    for item in &items {
        let row_height = task_thread_row_height(item, viewport.size.width - 6.0);
        let row_bounds = Bounds::new(
            viewport.origin.x,
            y,
            viewport.size.width - 6.0,
            row_height,
        );
        if row_bounds.max_y() >= viewport.origin.y && row_bounds.origin.y <= viewport.max_y() {
            paint_task_thread_row(row_bounds, item, paint);
        }
        y += row_height + TASK_THREAD_ROW_GAP;
    }
    paint.scene.pop_clip();

    if max_scroll > f32::EPSILON {
        paint_timeline_scrollbar(viewport, scroll_offset, content_height, paint);
    }
}

fn paint_thread_rail(
    panel_bounds: Bounds,
    pane_state: &CodingAgentPaneState,
    autopilot_chat: &AutopilotChatState,
    selected_project: Option<&AutopilotProjectIdentity>,
    selected_workspace_root: Option<&str>,
    paint: &mut PaintContext,
) {
    paint_mission_control_section_panel(
        panel_bounds,
        "THREADS",
        Hsla::from_hex(BADGE_SUCCESS_ACCENT),
        false,
        paint,
    );

    let new_button = Bounds::new(
        panel_bounds.max_x() - THREAD_RAIL_NEW_BUTTON_SIZE - 10.0,
        panel_bounds.origin.y + (SECTION_HEADER_HEIGHT - THREAD_RAIL_NEW_BUTTON_SIZE) * 0.5,
        THREAD_RAIL_NEW_BUTTON_SIZE,
        THREAD_RAIL_NEW_BUTTON_SIZE,
    );
    let add_enabled = selected_workspace_root.is_some();
    paint.scene.draw_quad(
        Quad::new(new_button)
            .with_background(if add_enabled {
                theme::bg::SURFACE.with_alpha(0.92)
            } else {
                theme::bg::SURFACE.with_alpha(0.52)
            })
            .with_border(
                if add_enabled {
                    theme::status::SUCCESS.with_alpha(0.42)
                } else {
                    theme::border::DEFAULT.with_alpha(0.18)
                },
                1.0,
            )
            .with_corner_radius(5.0),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        "+",
        Point::new(new_button.origin.x + 7.0, new_button.origin.y + 4.0),
        13.0,
        if add_enabled {
            theme::text::PRIMARY
        } else {
            theme::text::MUTED
        },
    ));

    let viewport = thread_rail_viewport_bounds(panel_bounds);
    let entries = coding_agent_thread_entries(
        pane_state,
        autopilot_chat,
        selected_project,
        selected_workspace_root,
    );
    if entries.is_empty() {
        paint.scene.push_clip(viewport);
        paint.scene.draw_text(paint.text.layout(
            if selected_workspace_root.is_some() {
                "No threads yet. Press + to start one."
            } else {
                "Select a folder to manage threads."
            },
            Point::new(viewport.origin.x + 2.0, viewport.origin.y + 8.0),
            10.0,
            theme::text::MUTED,
        ));
        paint.scene.pop_clip();
        return;
    }

    let content_height = if entries.is_empty() {
        viewport.size.height
    } else {
        entries.len() as f32 * THREAD_RAIL_ROW_HEIGHT
            + entries.len().saturating_sub(1) as f32 * THREAD_RAIL_ROW_GAP
            + 8.0
    };
    let max_scroll = (content_height - viewport.size.height).max(0.0);
    let scroll_offset = pane_state.thread_list_scroll_offset.clamp(0.0, max_scroll);

    paint.scene.push_clip(viewport);
    for (index, entry) in entries.iter().enumerate() {
        let row = thread_rail_row_bounds(viewport, scroll_offset, index);
        if row.max_y() < viewport.origin.y || row.origin.y > viewport.max_y() {
            continue;
        }
        let accent = entry.accent;
        paint.scene.draw_quad(
            Quad::new(row)
                .with_background(if entry.active {
                    theme::bg::SURFACE.with_alpha(0.94)
                } else {
                    theme::bg::APP.with_alpha(0.82)
                })
                .with_border(
                    if entry.active {
                        accent.with_alpha(0.44)
                    } else {
                        theme::border::DEFAULT.with_alpha(0.18)
                    },
                    1.0,
                )
                .with_corner_radius(7.0),
        );
        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                row.origin.x + 8.0,
                row.origin.y + 10.0,
                6.0,
                6.0,
            ))
            .with_background(accent.with_alpha(if entry.running || entry.active { 0.92 } else { 0.42 }))
            .with_corner_radius(3.0),
        );
        let status_text = summarize_line(entry.status.as_str(), 10);
        let status_width = paint.text.measure(status_text.as_str(), 8.0);
        let status_x = row.max_x() - status_width - 8.0;
        let text_left = row.origin.x + 20.0;
        let text_right = (status_x - 10.0).max(text_left);
        let text_clip = Bounds::new(
            text_left,
            row.origin.y + 6.0,
            (text_right - text_left).max(0.0),
            row.size.height - 12.0,
        );
        paint.scene.push_clip(text_clip);
        let title = summarize_line(entry.title.as_str(), 22);
        paint.scene.draw_text(paint.text.layout_mono(
            title.as_str(),
            Point::new(text_left, row.origin.y + 8.0),
            10.0,
            theme::text::PRIMARY,
        ));
        let subtitle = summarize_line(entry.subtitle.as_str(), 30);
        paint.scene.draw_text(paint.text.layout(
            subtitle.as_str(),
            Point::new(text_left, row.origin.y + 23.0),
            8.5,
            theme::text::MUTED,
        ));
        paint.scene.pop_clip();
        paint.scene.draw_text(paint.text.layout_mono(
            status_text.as_str(),
            Point::new(status_x, row.origin.y + 8.0),
            8.0,
            if entry.running || entry.status.eq_ignore_ascii_case("approval") {
                accent.with_alpha(0.92)
            } else {
                theme::text::MUTED
            },
        ));
    }
    paint.scene.pop_clip();

    if max_scroll > f32::EPSILON {
        paint_timeline_scrollbar(viewport, scroll_offset, content_height, paint);
    }
}

fn paint_workspace_summary(
    bounds: Bounds,
    selected_project: Option<&AutopilotProjectIdentity>,
    selected_thread: Option<&crate::app_state::AutopilotThreadMetadata>,
    autopilot_chat: &AutopilotChatState,
    selected_thread_id: Option<&str>,
    paint: &mut PaintContext,
) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::ELEVATED.with_alpha(0.94))
            .with_border(theme::border::DEFAULT.with_alpha(0.18), 1.0)
            .with_corner_radius(CARD_RADIUS),
    );

    let unbound_state = selected_project.is_none();
    let inactive_state = selected_thread_id.is_none();
    let repo_label = selected_project
        .map(|project| project.project_name.as_str())
        .unwrap_or("No repo selected");
    let workspace_detail = selected_project
        .map(|project| project.workspace_root.as_str())
        .unwrap_or("Bind this pane to a project workspace to unlock timeline, terminal, and diff review.");
    let thread_label = selected_thread
        .and_then(|thread| thread.thread_name.as_deref())
        .or(selected_thread_id)
        .unwrap_or("No active thread");

    paint.scene.push_clip(Bounds::new(
        bounds.origin.x + 12.0,
        bounds.origin.y + 10.0,
        bounds.size.width - 24.0,
        44.0,
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        "WORKSPACE",
        Point::new(bounds.origin.x + 12.0, bounds.origin.y + 12.0),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        repo_label,
        Point::new(bounds.origin.x + 12.0, bounds.origin.y + 30.0),
        16.0,
        theme::text::PRIMARY,
    ));
    let secondary_copy = if unbound_state {
        "Use the repo controls above to connect Coding Agent to a workspace."
            .to_string()
    } else {
        format!(
            "{}  //  {}",
            summarize_line(workspace_detail, 56),
            if inactive_state {
                "no active thread yet".to_string()
            } else {
                format!("thread {thread_label}")
            }
        )
    };
    paint.scene.draw_text(paint.text.layout(
        &secondary_copy,
        Point::new(bounds.origin.x + 12.0, bounds.origin.y + 49.0),
        10.0,
        theme::text::SECONDARY,
    ));
    paint.scene.pop_clip();

    let badge_row_top = bounds.max_y() - BADGE_HEIGHT - 10.0;
    let badge_count = if unbound_state {
        2.0
    } else if inactive_state {
        3.0
    } else {
        4.0
    };
    let badge_width =
        ((bounds.size.width - 24.0 - BADGE_GAP * (badge_count - 1.0)) / badge_count).max(72.0);
    let branch_value = selected_project
        .and_then(|project| project.git_branch.as_ref())
        .map(|branch| match selected_project.and_then(|project| project.git_dirty) {
            Some(true) => format!("{branch}*"),
            _ => branch.to_string(),
        })
        .unwrap_or_else(|| "no-git".to_string());
    let approvals = approvals_for_selection(autopilot_chat, selected_project);
    let (status_value, status_accent) =
        session_status_for_selection(autopilot_chat, selected_thread, selected_thread_id);

    let mode_bounds = Bounds::new(
        bounds.origin.x + 12.0,
        badge_row_top,
        badge_width,
        BADGE_HEIGHT,
    );
    let status_bounds = Bounds::new(
        bounds.origin.x + 12.0 + (badge_width + BADGE_GAP),
        badge_row_top,
        badge_width,
        BADGE_HEIGHT,
    );
    if !unbound_state {
        paint_badge(
            Bounds::new(bounds.origin.x + 12.0, badge_row_top, badge_width, BADGE_HEIGHT),
            "BRANCH",
            &branch_value.to_ascii_uppercase(),
            Hsla::from_hex(BADGE_BRANCH_ACCENT),
            paint,
        );
        paint_badge(
            Bounds::new(
                bounds.origin.x + 12.0 + (badge_width + BADGE_GAP),
                badge_row_top,
                badge_width,
                BADGE_HEIGHT,
            ),
            "MODE",
            &mode_label(selected_project, autopilot_chat),
            Hsla::from_hex(BADGE_MODE_ACCENT),
            paint,
        );
        paint_badge(
            Bounds::new(
                bounds.origin.x + 12.0 + (badge_width + BADGE_GAP) * 2.0,
                badge_row_top,
                badge_width,
                BADGE_HEIGHT,
            ),
            "STATUS",
            &status_value,
            status_accent,
            paint,
        );
        if !inactive_state {
            paint_badge(
                Bounds::new(
                    bounds.origin.x + 12.0 + (badge_width + BADGE_GAP) * 3.0,
                    badge_row_top,
                    badge_width,
                    BADGE_HEIGHT,
                ),
                "APPROVALS",
                &approvals,
                if approvals == "CLEAR" {
                    Hsla::from_hex(BADGE_MUTED_ACCENT)
                } else {
                    Hsla::from_hex(BADGE_WARNING_ACCENT)
                },
                paint,
            );
        }
    } else {
        paint_badge(
            mode_bounds,
            "READY",
            "SELECT REPO",
            Hsla::from_hex(BADGE_MODE_ACCENT),
            paint,
        );
        paint_badge(
            status_bounds,
            "STATUS",
            "IDLE",
            Hsla::from_hex(BADGE_MUTED_ACCENT),
            paint,
        );
    }
}

fn paint_approval_bar(
    bounds: Bounds,
    pane_state: &CodingAgentPaneState,
    summary: &ApprovalBarSummary,
    paint: &mut PaintContext,
) {
    if summary.total == 0 {
        let compact = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + 10.0,
            bounds.size.width,
            36.0,
        );
        paint.scene.draw_quad(
            Quad::new(compact)
                .with_background(theme::bg::APP.with_alpha(0.44))
                .with_border(theme::border::DEFAULT.with_alpha(0.14), 1.0)
                .with_corner_radius(CARD_RADIUS),
        );
        paint.scene.draw_text(paint.text.layout_mono(
            "APPROVALS",
            Point::new(compact.origin.x + 12.0, compact.origin.y + 12.0),
            9.0,
            theme::text::MUTED,
        ));
        paint.scene.draw_text(paint.text.layout_mono(
            "No approvals pending",
            Point::new(compact.origin.x + 88.0, compact.origin.y + 10.0),
            12.0,
            theme::text::PRIMARY,
        ));
        paint.scene.draw_text(paint.text.layout_mono(
            "CLEAR",
            Point::new(compact.max_x() - 48.0, compact.origin.y + 11.0),
            10.0,
            Hsla::from_hex(BADGE_MUTED_ACCENT),
        ));
        return;
    }
    let border = if summary.total > 0 {
        summary.accent.with_alpha(0.34)
    } else {
        theme::border::DEFAULT.with_alpha(0.32)
    };
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::ELEVATED)
            .with_border(border, 1.0)
            .with_corner_radius(CARD_RADIUS),
    );
    let (inspect, deny, approve) = approval_button_bounds(bounds);
    let content_clip = Bounds::new(
        bounds.origin.x + 12.0,
        bounds.origin.y + 8.0,
        (inspect.origin.x - bounds.origin.x - 24.0).max(0.0),
        bounds.size.height - 16.0,
    );
    paint.scene.push_clip(content_clip);
    paint.scene.draw_text(paint.text.layout_mono(
        "APPROVALS",
        Point::new(bounds.origin.x + 12.0, bounds.origin.y + 10.0),
        9.0,
        if summary.total > 0 {
            summary.accent
        } else {
            theme::text::MUTED
        },
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        &summary.title,
        Point::new(bounds.origin.x + 12.0, bounds.origin.y + 25.0),
        13.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        &summary.detail,
        Point::new(bounds.origin.x + 12.0, bounds.origin.y + 42.0),
        10.0,
        theme::text::SECONDARY,
    ));
    paint.scene.pop_clip();

    if summary.total > 0 {
        paint_secondary_button(
            inspect,
            if pane_state.approval_drawer_open {
                "Hide"
            } else {
                "Inspect"
            },
            paint,
        );
        paint_secondary_button(deny, "Deny", paint);
        paint_primary_button(approve, "Approve", paint);
    } else {
        paint_disabled_button(inspect, "Inspect", paint);
        paint_disabled_button(deny, "Deny", paint);
        paint_disabled_button(approve, "Approve", paint);
    }
}

fn approval_request_card_height(detail: &ApprovalDetailCard) -> f32 {
    20.0 + detail.rows.len() as f32 * APPROVAL_DETAIL_ROW_HEIGHT + 16.0
}

fn approval_detail_content_height_for_card(detail: &ApprovalDetailCard, viewport: Bounds) -> f32 {
    let content_height = APPROVAL_DETAIL_HERO_HEIGHT
        + APPROVAL_DETAIL_CARD_GAP
        + approval_request_card_height(detail)
        + APPROVAL_DETAIL_CARD_GAP
        + APPROVAL_DETAIL_QUEUE_HEIGHT
        + APPROVAL_DETAIL_CARD_GAP
        + APPROVAL_DETAIL_NOTE_HEIGHT;
    content_height.max(viewport.size.height)
}

fn paint_approval_detail(
    bounds: Bounds,
    pane_state: &CodingAgentPaneState,
    detail: &ApprovalDetailCard,
    paint: &mut PaintContext,
) {
    let viewport = approval_detail_viewport_bounds(bounds);
    let content_height = approval_detail_content_height_for_card(detail, viewport);
    let max_scroll = (content_height - viewport.size.height).max(0.0);
    let scroll_offset = pane_state.approval_scroll_offset.clamp(0.0, max_scroll);
    let card_width = (viewport.size.width - 6.0).max(0.0);

    paint.scene.push_clip(viewport);
    let mut y = viewport.origin.y - scroll_offset;

    let hero = Bounds::new(viewport.origin.x, y, card_width, APPROVAL_DETAIL_HERO_HEIGHT);
    paint_standard_detail_card(hero, detail.accent, paint);
    paint_detail_card_accent(hero, detail.accent, paint);
    paint.scene.push_clip(Bounds::new(
        hero.origin.x + 12.0,
        hero.origin.y + 10.0,
        hero.size.width - 24.0,
        hero.size.height - 20.0,
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        detail.kind,
        Point::new(hero.origin.x + 12.0, hero.origin.y + 12.0),
        9.0,
        detail.accent,
    ));
    paint.scene.draw_text(paint.text.layout(
        &detail.headline,
        Point::new(hero.origin.x + 12.0, hero.origin.y + 30.0),
        12.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        &format!("THREAD  //  {}", detail.thread_label),
        Point::new(hero.origin.x + 12.0, hero.origin.y + 48.0),
        10.0,
        theme::text::SECONDARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        &detail.detail,
        Point::new(hero.origin.x + 12.0, hero.origin.y + 68.0),
        10.0,
        theme::text::PRIMARY,
    ));
    paint.scene.pop_clip();
    y = hero.max_y() + APPROVAL_DETAIL_CARD_GAP;

    let request = Bounds::new(
        viewport.origin.x,
        y,
        card_width,
        approval_request_card_height(detail),
    );
    paint_standard_detail_card(request, theme::border::DEFAULT.with_alpha(0.22), paint);
    paint.scene.draw_text(paint.text.layout_mono(
        "REQUEST CONTEXT",
        Point::new(request.origin.x + 12.0, request.origin.y + 10.0),
        9.0,
        theme::text::MUTED,
    ));
    let mut row_y = request.origin.y + 28.0;
    for (index, row) in detail.rows.iter().enumerate() {
        if index > 0 {
            paint.scene.draw_quad(
                Quad::new(Bounds::new(
                    request.origin.x + 12.0,
                    row_y - 6.0,
                    request.size.width - 24.0,
                    1.0,
                ))
                .with_background(theme::border::DEFAULT.with_alpha(0.18)),
            );
        }
        paint.scene.draw_text(paint.text.layout_mono(
            row.label,
            Point::new(request.origin.x + 12.0, row_y),
            9.0,
            theme::text::MUTED,
        ));
        paint.scene.draw_text(paint.text.layout(
            &row.value,
            Point::new(request.origin.x + 12.0, row_y + 13.0),
            10.0,
            theme::text::PRIMARY,
        ));
        row_y += APPROVAL_DETAIL_ROW_HEIGHT;
    }
    y = request.max_y() + APPROVAL_DETAIL_CARD_GAP;

    let queue = Bounds::new(viewport.origin.x, y, card_width, APPROVAL_DETAIL_QUEUE_HEIGHT);
    paint_standard_detail_card(queue, theme::border::DEFAULT.with_alpha(0.22), paint);
    paint.scene.draw_text(paint.text.layout_mono(
        "QUEUE",
        Point::new(queue.origin.x + 12.0, queue.origin.y + 10.0),
        9.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout(
        &detail.queue_title,
        Point::new(queue.origin.x + 12.0, queue.origin.y + 26.0),
        11.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        &detail.queue_detail,
        Point::new(queue.origin.x + 12.0, queue.origin.y + 42.0),
        9.0,
        theme::text::SECONDARY,
    ));
    if let Some(aux) = detail.queue_aux.as_ref() {
        paint.scene.draw_text(paint.text.layout(
            aux,
            Point::new(queue.origin.x + 12.0, queue.origin.y + 55.0),
            9.0,
            theme::text::MUTED,
        ));
    }
    y = queue.max_y() + APPROVAL_DETAIL_CARD_GAP;

    let note = Bounds::new(viewport.origin.x, y, card_width, APPROVAL_DETAIL_NOTE_HEIGHT);
    paint_standard_detail_card(note, theme::border::DEFAULT.with_alpha(0.18), paint);
    paint.scene.draw_text(paint.text.layout_mono(
        "NEXT",
        Point::new(note.origin.x + 12.0, note.origin.y + 10.0),
        9.0,
        detail.accent.with_alpha(0.9),
    ));
    paint.scene.draw_text(paint.text.layout(
        &summarize_line(detail.helper.as_str(), 96),
        Point::new(note.origin.x + 12.0, note.origin.y + 28.0),
        10.0,
        theme::text::SECONDARY,
    ));
    paint.scene.pop_clip();

    if max_scroll > f32::EPSILON {
        paint_timeline_scrollbar(viewport, scroll_offset, content_height, paint);
    }
}

fn paint_standard_detail_card(bounds: Bounds, border: Hsla, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::HOVER.with_alpha(0.56))
            .with_border(border, 1.0)
            .with_corner_radius(CARD_RADIUS),
    );
}

fn paint_detail_card_accent(bounds: Bounds, accent: Hsla, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(Bounds::new(bounds.origin.x, bounds.origin.y, 3.0, bounds.size.height))
            .with_background(accent.with_alpha(0.82))
            .with_corner_radius(2.0),
    );
}

fn paint_changed_files_rail(
    panel_bounds: Bounds,
    pane_state: &CodingAgentPaneState,
    artifact: Option<&AutopilotDiffArtifact>,
    review_artifact: Option<&AutopilotReviewArtifact>,
    selected_thread_id: Option<&str>,
    paint: &mut PaintContext,
) {
    if artifact.is_none() && review_artifact.is_none() && selected_thread_id.is_none() {
        paint_empty_state_panel(
            panel_bounds,
            "Review rail locked",
            "Changed files and inline diffs will appear here after the agent edits workspace files.",
            "Select a repo and start a coding task to activate review.",
            Hsla::from_hex(BADGE_MUTED_ACCENT),
            paint,
        );
        return;
    }
    let summary_bounds = review_summary_bounds(panel_bounds);
    paint_changed_files_summary(
        summary_bounds,
        artifact,
        review_artifact,
        pane_state,
        selected_thread_id,
        paint,
    );
    let (files_tab, diff_tab) = review_tab_bounds(panel_bounds);
    paint_review_tab_button(
        files_tab,
        "Changed files",
        pane_state.right_rail_tab == CodingAgentRailTab::ChangedFiles,
        paint,
    );
    paint_review_tab_button(
        diff_tab,
        "Diff",
        pane_state.right_rail_tab == CodingAgentRailTab::Diff,
        paint,
    );

    let viewport = review_body_viewport_bounds(panel_bounds);
    let Some(artifact) = artifact else {
        let message = match (selected_thread_id, review_artifact) {
            (None, _) => {
                "Select a repo with a coding thread to unlock changed files and diff review."
            }
            (_, Some(review_artifact)) if review_artifact.status.eq_ignore_ascii_case("running") => {
                "Review is running for this thread. File changes will appear here when the diff artifact is ready."
            }
            (_, Some(_)) => {
                "A review artifact exists for this thread, but no diff artifact is projected yet."
            }
            _ => "No diff artifact yet. File changes will appear here after the agent edits workspace files.",
        };
        paint.scene.draw_text(paint.text.layout(
            message,
            Point::new(viewport.origin.x, viewport.origin.y + 14.0),
            10.0,
            theme::text::MUTED,
        ));
        return;
    };

    let selected_path = resolved_selected_diff_file_path(pane_state, Some(artifact));
    match pane_state.right_rail_tab {
        CodingAgentRailTab::ChangedFiles => {
            let content_height = review_content_height_for_artifact(Some(artifact), viewport);
            let max_scroll = (content_height - viewport.size.height).max(0.0);
            let scroll_offset = pane_state.diff_scroll_offset.clamp(0.0, max_scroll);

            paint.scene.push_clip(viewport);
            let mut y = viewport.origin.y - scroll_offset;
            for file in &artifact.files {
                let row_bounds = Bounds::new(
                    viewport.origin.x,
                    y,
                    viewport.size.width - 6.0,
                    REVIEW_FILE_ROW_HEIGHT,
                );
                if row_bounds.max_y() >= viewport.origin.y && row_bounds.origin.y <= viewport.max_y()
                {
                    paint_changed_file_row(
                        row_bounds,
                        file,
                        selected_path == Some(file.path.as_str()),
                        paint,
                    );
                }
                y += REVIEW_FILE_ROW_HEIGHT + REVIEW_FILE_ROW_GAP;
            }
            paint.scene.pop_clip();

            if max_scroll > f32::EPSILON {
                paint_timeline_scrollbar(viewport, scroll_offset, content_height, paint);
            }
        }
        CodingAgentRailTab::Diff => {
            paint_diff_viewer(viewport, artifact, selected_path, pane_state, paint);
        }
    }
}

fn paint_review_tab_button(
    bounds: Bounds,
    label: &str,
    active: bool,
    paint: &mut PaintContext,
) {
    let border = if active {
        Hsla::from_hex(BADGE_MODE_ACCENT).with_alpha(0.58)
    } else {
        theme::border::DEFAULT.with_alpha(0.24)
    };
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(if active {
                theme::bg::HOVER.with_alpha(0.72)
            } else {
                theme::bg::APP.with_alpha(0.88)
            })
            .with_border(border, 1.0)
            .with_corner_radius(7.0),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        label,
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 8.0),
        10.0,
        if active {
            theme::text::PRIMARY
        } else {
            theme::text::SECONDARY
        },
    ));
}

fn paint_changed_files_summary(
    bounds: Bounds,
    artifact: Option<&AutopilotDiffArtifact>,
    review_artifact: Option<&AutopilotReviewArtifact>,
    pane_state: &CodingAgentPaneState,
    selected_thread_id: Option<&str>,
    paint: &mut PaintContext,
) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::HOVER.with_alpha(0.5))
            .with_border(theme::border::DEFAULT.with_alpha(0.28), 1.0)
            .with_corner_radius(8.0),
    );

    let (title, detail, aux) = if let Some(artifact) = artifact {
        let selected_path = resolved_selected_diff_file_path(pane_state, Some(artifact));
        let selected_file = selected_path
            .and_then(|path| artifact.files.iter().find(|file| file.path == path));
        let title = format!(
            "{} files  //  +{}  -{}",
            artifact.files.len(),
            artifact.added_line_count,
            artifact.removed_line_count
        );
        let detail = if let Some(file) = selected_file {
            format!(
                "Selected  //  {}  //  +{}  -{}",
                file.path, file.added_line_count, file.removed_line_count
            )
        } else {
            artifact
                .project_name
                .clone()
                .unwrap_or_else(|| "Workspace diff artifact ready".to_string())
        };
        let review_status = review_artifact.map(|artifact| {
            format!(
                "  //  review {}",
                humanize_compact_status(artifact.status.as_str())
            )
        });
        let aux = Some(format!(
            "thread  //  {}{}{}",
            selected_thread_id
                .map(|thread_id| summarize_line(thread_id, 18))
                .unwrap_or_else(|| "n/a".to_string()),
            artifact
                .git_branch
                .as_ref()
                .map(|branch| format!("  //  branch {branch}"))
                .unwrap_or_default(),
            review_status.unwrap_or_default(),
        ));
        (title, detail, aux)
    } else if let Some(review_artifact) = review_artifact {
        let title = format!(
            "Review {}",
            humanize_compact_status(review_artifact.status.as_str())
        );
        let detail = review_artifact
            .summary
            .clone()
            .unwrap_or_else(|| "Review artifact is present for this thread, but file diffs are not available yet.".to_string());
        let aux = Some(format!(
            "thread  //  {}  //  target {}",
            selected_thread_id
                .map(|thread_id| summarize_line(thread_id, 18))
                .unwrap_or_else(|| "n/a".to_string()),
            review_artifact.target
        ));
        (title, detail, aux)
    } else {
        (
            "No changed files".to_string(),
            "Run a coding task to populate the review rail.".to_string(),
            None,
        )
    };

    let clip = Bounds::new(
        bounds.origin.x + 10.0,
        bounds.origin.y + 8.0,
        bounds.size.width - 20.0,
        bounds.size.height - 16.0,
    );
    paint.scene.push_clip(clip);
    paint.scene.draw_text(paint.text.layout_mono(
        "REVIEW SUMMARY",
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 10.0),
        9.0,
        Hsla::from_hex(BADGE_MODE_ACCENT),
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        &title,
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 26.0),
        12.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        &detail,
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 43.0),
        10.0,
        theme::text::SECONDARY,
    ));
    if let Some(aux) = aux.as_ref() {
        paint.scene.draw_text(paint.text.layout(
            aux,
            Point::new(bounds.origin.x + 10.0, bounds.origin.y + 59.0),
            9.0,
            theme::text::MUTED,
        ));
    }
    paint.scene.pop_clip();
}

fn paint_changed_file_row(
    bounds: Bounds,
    file: &crate::app_state::AutopilotDiffFileArtifact,
    selected: bool,
    paint: &mut PaintContext,
) {
    let accent = if selected {
        Hsla::from_hex(BADGE_MODE_ACCENT)
    } else {
        theme::border::DEFAULT.with_alpha(0.28)
    };
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(if selected {
                theme::bg::HOVER.with_alpha(0.74)
            } else {
                theme::bg::APP.with_alpha(0.92)
            })
            .with_border(accent, 1.0)
            .with_corner_radius(8.0),
    );
    if selected {
        paint.scene.draw_quad(
            Quad::new(Bounds::new(bounds.origin.x, bounds.origin.y, 3.0, bounds.size.height))
                .with_background(Hsla::from_hex(BADGE_MODE_ACCENT).with_alpha(0.82))
                .with_corner_radius(2.0),
        );
    }
    let clip = Bounds::new(
        bounds.origin.x + 10.0,
        bounds.origin.y + 8.0,
        bounds.size.width - 20.0,
        bounds.size.height - 16.0,
    );
    paint.scene.push_clip(clip);
    paint.scene.draw_text(paint.text.layout_mono(
        &file.path,
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 9.0),
        10.0,
        theme::text::PRIMARY,
    ));
    let counts = format!("+{}  -{}", file.added_line_count, file.removed_line_count);
    paint.scene.draw_text(paint.text.layout_mono(
        &counts,
        Point::new(bounds.max_x() - 86.0, bounds.origin.y + 9.0),
        9.0,
        if file.removed_line_count > 0 {
            Hsla::from_hex(BADGE_WARNING_ACCENT)
        } else {
            Hsla::from_hex(BADGE_SUCCESS_ACCENT)
        },
    ));
    paint.scene.pop_clip();
}

fn paint_diff_viewer(
    viewport: Bounds,
    artifact: &AutopilotDiffArtifact,
    selected_path: Option<&str>,
    pane_state: &CodingAgentPaneState,
    paint: &mut PaintContext,
) {
    let Some(selected_path) = selected_path else {
        paint.scene.draw_text(paint.text.layout(
            "Select a changed file to inspect the patch.",
            Point::new(viewport.origin.x, viewport.origin.y + 14.0),
            10.0,
            theme::text::MUTED,
        ));
        return;
    };

    let file_patch_lines = diff_lines_for_file(artifact, selected_path);
    let content_height =
        diff_viewer_content_height_for_artifact(Some(artifact), Some(selected_path), viewport);
    let max_scroll = (content_height - viewport.size.height).max(0.0);
    let scroll_offset = pane_state.diff_scroll_offset.clamp(0.0, max_scroll);
    let frame = Bounds::new(
        viewport.origin.x,
        viewport.origin.y,
        viewport.size.width - 6.0,
        viewport.size.height,
    );
    paint.scene.draw_quad(
        Quad::new(frame)
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT.with_alpha(0.25), 1.0)
            .with_corner_radius(8.0),
    );

    let header = Bounds::new(frame.origin.x, frame.origin.y, frame.size.width, 34.0);
    paint.scene.draw_quad(
        Quad::new(header)
            .with_background(theme::bg::HOVER.with_alpha(0.56))
            .with_corner_radius(8.0),
    );
    paint.scene.push_clip(Bounds::new(
        header.origin.x + 10.0,
        header.origin.y + 8.0,
        header.size.width - 20.0,
        header.size.height - 12.0,
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        selected_path,
        Point::new(header.origin.x + 10.0, header.origin.y + 10.0),
        10.0,
        theme::text::PRIMARY,
    ));
    paint.scene.pop_clip();

    let body = Bounds::new(
        frame.origin.x + 8.0,
        header.max_y() + 6.0,
        frame.size.width - 16.0,
        (frame.max_y() - header.max_y() - 14.0).max(0.0),
    );
    paint.scene.push_clip(body);
    let mut y = body.origin.y + 4.0 - scroll_offset;
    for line in &file_patch_lines {
        if y + DIFF_LINE_HEIGHT >= body.origin.y && y <= body.max_y() {
            paint.scene.draw_text(paint.text.layout_mono(
                line,
                Point::new(body.origin.x, y),
                9.0,
                diff_line_color(line),
            ));
        }
        y += DIFF_LINE_HEIGHT;
    }
    paint.scene.pop_clip();

    if max_scroll > f32::EPSILON {
        paint_timeline_scrollbar(body, scroll_offset, content_height, paint);
    }
}

fn diff_line_color(line: &str) -> Hsla {
    if line.starts_with("@@") {
        Hsla::from_hex(BADGE_MODE_ACCENT)
    } else if line.starts_with('+') && !line.starts_with("+++") {
        Hsla::from_hex(BADGE_SUCCESS_ACCENT)
    } else if line.starts_with('-') && !line.starts_with("---") {
        Hsla::from_hex(BADGE_WARNING_ACCENT)
    } else if line.starts_with("diff --git ")
        || line.starts_with("--- ")
        || line.starts_with("+++ ")
    {
        theme::text::SECONDARY
    } else {
        theme::text::PRIMARY
    }
}

fn paint_timeline_row(bounds: Bounds, entry: &TimelineEntry, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::HOVER.with_alpha(0.55))
            .with_border(entry.accent.with_alpha(0.18), 1.0)
            .with_corner_radius(8.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(bounds.origin.x, bounds.origin.y, 3.0, bounds.size.height))
            .with_background(entry.accent.with_alpha(0.85))
            .with_corner_radius(2.0),
    );

    let clip = Bounds::new(
        bounds.origin.x + 12.0,
        bounds.origin.y + 6.0,
        bounds.size.width - 20.0,
        bounds.size.height - 12.0,
    );
    paint.scene.push_clip(clip);
    paint.scene.draw_text(paint.text.layout_mono(
        entry.kind,
        Point::new(bounds.origin.x + 12.0, bounds.origin.y + 7.0),
        9.0,
        entry.accent,
    ));
    paint.scene.draw_text(paint.text.layout(
        &entry.title,
        Point::new(bounds.origin.x + 12.0, bounds.origin.y + 21.0),
        12.0,
        theme::text::PRIMARY,
    ));
    if let Some(detail) = entry.detail.as_ref() {
        paint.scene.draw_text(paint.text.layout(
            detail,
            Point::new(bounds.origin.x + 12.0, bounds.origin.y + 34.0),
            9.5,
            theme::text::SECONDARY,
        ));
    }
    paint.scene.pop_clip();
}

fn paint_timeline_scrollbar(
    viewport: Bounds,
    scroll_offset: f32,
    content_height: f32,
    paint: &mut PaintContext,
) {
    if viewport.size.height <= f32::EPSILON || content_height <= f32::EPSILON {
        return;
    }

    let track = Bounds::new(
        viewport.max_x() - 4.0,
        viewport.origin.y,
        2.0,
        viewport.size.height,
    );
    paint.scene.draw_quad(
        Quad::new(track)
            .with_background(theme::border::DEFAULT.with_alpha(0.18))
            .with_corner_radius(1.0),
    );

    let thumb_max = viewport.size.height.max(0.0);
    let thumb_min = 18.0_f32.min(thumb_max);
    let thumb_height = ((viewport.size.height / content_height) * viewport.size.height)
        .clamp(thumb_min, thumb_max);
    let max_scroll = (content_height - viewport.size.height).max(1.0);
    let thumb_y = track.origin.y
        + ((scroll_offset / max_scroll) * (track.size.height - thumb_height).max(0.0));
    paint.scene.draw_quad(
        Quad::new(Bounds::new(track.origin.x, thumb_y, track.size.width, thumb_height))
            .with_background(theme::text::SECONDARY.with_alpha(0.55))
            .with_corner_radius(1.0),
    );
}

fn terminal_output_viewport_bounds(panel_bounds: Bounds) -> Bounds {
    let body = terminal_body_bounds(panel_bounds);
    let footer = terminal_footer_bounds(panel_bounds);
    Bounds::new(
        body.origin.x + 8.0,
        body.origin.y + 8.0,
        (body.size.width - 16.0).max(0.0),
        (footer.origin.y - body.origin.y - 16.0).max(0.0),
    )
}

fn terminal_body_bounds(panel_bounds: Bounds) -> Bounds {
    Bounds::new(
        panel_bounds.origin.x + 18.0,
        panel_bounds.origin.y + SECTION_HEADER_HEIGHT + 12.0,
        (panel_bounds.size.width - 36.0).max(0.0),
        (panel_bounds.size.height - SECTION_HEADER_HEIGHT - 24.0).max(0.0),
    )
}

fn terminal_footer_bounds(panel_bounds: Bounds) -> Bounds {
    let body = terminal_body_bounds(panel_bounds);
    Bounds::new(
        body.origin.x + 8.0,
        body.max_y() - TERMINAL_FOOTER_HEIGHT - 8.0,
        (body.size.width - 16.0).max(0.0),
        TERMINAL_FOOTER_HEIGHT,
    )
}

fn terminal_footer_layout(bounds: Bounds) -> (Bounds, Bounds) {
    let send = Bounds::new(
        bounds.max_x() - TERMINAL_SEND_WIDTH,
        bounds.origin.y,
        TERMINAL_SEND_WIDTH,
        bounds.size.height,
    );
    let input = Bounds::new(
        bounds.origin.x + TERMINAL_PROMPT_PREFIX_WIDTH,
        bounds.origin.y,
        (bounds.size.width - TERMINAL_SEND_WIDTH - MINI_GAP - TERMINAL_PROMPT_PREFIX_WIDTH)
            .max(0.0),
        bounds.size.height,
    );
    (input, send)
}

fn paint_terminal_output_lines(
    viewport: Bounds,
    scroll_offset: f32,
    session: Option<&AutopilotTerminalSession>,
    selected_thread_id: Option<&str>,
    paint: &mut PaintContext,
) {
    let Some(session) = session else {
        paint.scene.draw_text(paint.text.layout(
            if selected_thread_id.is_none() {
                "Select a local folder to open a shell for this workspace."
            } else {
                "Opening a shell for this workspace..."
            },
            Point::new(viewport.origin.x + 10.0, viewport.origin.y + 12.0),
            10.0,
            theme::text::MUTED,
        ));
        return;
    };

    if session.lines.is_empty() {
        let placeholder = match session.status {
            AutopilotTerminalSessionStatus::Pending => "Opening local shell...",
            AutopilotTerminalSessionStatus::Running => "Shell is ready. Type a command below.",
            _ => "Shell ended. Type a command below to reopen it.",
        };
        paint.scene.draw_text(paint.text.layout_mono(
            placeholder,
            Point::new(viewport.origin.x + 10.0, viewport.origin.y + 10.0),
            9.0,
            theme::text::MUTED,
        ));
        return;
    }

    let mut y = viewport.origin.y + 10.0 - scroll_offset;
    for line in &session.lines {
        let row_bounds = Bounds::new(
            viewport.origin.x + 8.0,
            y,
            viewport.size.width - 22.0,
            TERMINAL_LINE_HEIGHT,
        );
        if row_bounds.max_y() >= viewport.origin.y && row_bounds.origin.y <= viewport.max_y() {
            paint.scene.draw_text(paint.text.layout_mono(
                &summarize_line(line.text.as_str(), 180),
                Point::new(row_bounds.origin.x, row_bounds.origin.y),
                9.0,
                terminal_stream_color(&line.stream),
            ));
        }
        y += TERMINAL_LINE_HEIGHT;
    }
}

fn paint_terminal_footer(
    bounds: Bounds,
    inputs: &mut CodingAgentPaneInputs,
    terminal_input_enabled: bool,
    selected_thread_id: Option<&str>,
    _session: Option<&AutopilotTerminalSession>,
    paint: &mut PaintContext,
) {
    let (input_bounds, send_bounds) = terminal_footer_layout(bounds);
    paint.scene.draw_quad(
        Quad::new(Bounds::new(bounds.origin.x, bounds.origin.y - 8.0, bounds.size.width, 1.0))
            .with_background(theme::border::DEFAULT.with_alpha(0.18)),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        ">",
        Point::new(bounds.origin.x + 2.0, bounds.origin.y + 11.0),
        11.0,
        if terminal_input_enabled {
            theme::text::SECONDARY
        } else {
            theme::text::MUTED
        },
    ));
    if terminal_input_enabled {
        let can_submit = !inputs.terminal_input.get_value().trim().is_empty();
        inputs
            .terminal_input
            .set_max_width(input_bounds.size.width.max(120.0));
        inputs.terminal_input.paint(input_bounds, paint);
        if can_submit {
            paint_secondary_button(send_bounds, "Run", paint);
        } else {
            paint_disabled_button(send_bounds, "Run", paint);
        }
    } else {
        if inputs.terminal_input.is_focused() {
            inputs.terminal_input.blur();
        }
        let locked_message = if selected_thread_id.is_some() {
            "Terminal input is locked while the agent is running."
        } else {
            "Select a local folder to unlock terminal input."
        };
        paint.scene.push_clip(Bounds::new(
            input_bounds.origin.x + 2.0,
            input_bounds.origin.y + 8.0,
            input_bounds.size.width - 4.0,
            input_bounds.size.height - 16.0,
        ));
        paint.scene.draw_text(paint.text.layout_mono(
            locked_message,
            Point::new(input_bounds.origin.x + 2.0, input_bounds.origin.y + 11.0),
            10.0,
            theme::text::MUTED,
        ));
        paint.scene.pop_clip();
        paint_disabled_button(send_bounds, "Run", paint);
    }

}

fn paint_coding_agent_send_button(bounds: Bounds, enabled: bool, paint: &mut PaintContext) {
    let (background, border, icon_tint) = if enabled {
        (
            Hsla::from_hex(0x52E06D).with_alpha(0.18),
            Hsla::from_hex(0x52E06D).with_alpha(0.42),
            Hsla::from_hex(0xD8DFF0),
        )
    } else {
        (
            Hsla::from_hex(0x121924).with_alpha(0.22),
            Hsla::from_hex(0x263245).with_alpha(0.28),
            Hsla::from_hex(0x8A909E).with_alpha(0.82),
        )
    };
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(background)
            .with_border(border, 1.0)
            .with_corner_radius(10.0),
    );
    let icon_size = 15.0;
    let icon_bounds = Bounds::new(
        bounds.origin.x + (bounds.size.width - icon_size) * 0.5,
        bounds.origin.y + (bounds.size.height - icon_size) * 0.5,
        icon_size,
        icon_size,
    );
    paint.scene.draw_svg(
        SvgQuad::new(
            icon_bounds,
            std::sync::Arc::<[u8]>::from(CODING_AGENT_SEND_ICON_SVG_RAW.as_bytes()),
        )
        .with_tint(icon_tint),
    );
}

fn paint_composer_bar(
    bounds: Bounds,
    pane_state: &CodingAgentPaneState,
    inputs: &mut CodingAgentPaneInputs,
    _interrupt_enabled: bool,
    selected_project: Option<&AutopilotProjectIdentity>,
    selected_workspace_root: Option<&str>,
    paint: &mut PaintContext,
) {
    let layout = composer_layout(bounds);
    let pending_bootstrap = pane_state.pending_thread_start_prompt.is_some();
    let has_prompt = !inputs.composer.get_value().trim().is_empty();
    let send_button_active = !pending_bootstrap && has_prompt;

    inputs
        .composer
        .set_max_width(layout.input.size.width.max(140.0));
    inputs.composer.paint(layout.input, paint);
    if pending_bootstrap {
        paint_coding_agent_send_button(layout.send, false, paint);
    } else {
        paint_coding_agent_send_button(layout.send, send_button_active, paint);
    }

    paint_repo_selector(
        layout.folder_selector,
        selected_project,
        selected_workspace_root,
        paint,
    );
}

fn task_thread_items(
    pane_state: &CodingAgentPaneState,
    autopilot_chat: &AutopilotChatState,
    selected_workspace_root: Option<&str>,
    selected_thread_id: Option<&str>,
) -> Vec<TaskThreadItem> {
    let mut items = Vec::new();

    if let Some(thread_id) = selected_thread_id {
        if let Some(messages) = autopilot_chat.cached_thread_messages(thread_id) {
            for message in messages {
                items.push(task_thread_item_for_message(message));
            }
        }

        let turn_running = autopilot_chat
            .active_turn_metadata()
            .is_some_and(|metadata| metadata.thread_id == thread_id);
        if turn_running
            && !items.iter().any(|item| item.running)
        {
            items.push(TaskThreadItem {
                speaker: "AGENT",
                body: format!("{} Working on your task...", spinner_frame()),
                detail: "running".to_string(),
                accent: Hsla::from_hex(BADGE_RUNNING_ACCENT),
                running: true,
            });
        }
    } else if pane_state.pending_thread_start_workspace_root.is_some() {
        if let Some(prompt) = pane_state.pending_thread_start_prompt.as_deref()
            && !prompt.trim().is_empty()
        {
            items.push(TaskThreadItem {
                speaker: "YOU",
                body: prompt.trim().to_string(),
                detail: "queued".to_string(),
                accent: Hsla::from_hex(BADGE_BRANCH_ACCENT),
                running: false,
            });
        }
        items.push(TaskThreadItem {
            speaker: "AGENT",
            body: format!("{} Starting coding session...", spinner_frame()),
            detail: "queued".to_string(),
            accent: Hsla::from_hex(BADGE_WARNING_ACCENT),
            running: true,
        });
    }

    if items.is_empty() {
        let (body, detail) = if selected_workspace_root.is_none() {
            (
                "Select a local folder, then run a task to start working with the agent.",
                "idle",
            )
        } else {
            (
                "Your prompts, progress updates, and final results will appear here after you run a task.",
                "ready",
            )
        };
        items.push(TaskThreadItem {
            speaker: "SESSION",
            body: body.to_string(),
            detail: detail.to_string(),
            accent: Hsla::from_hex(BADGE_MUTED_ACCENT),
            running: false,
        });
    }

    items
}

fn task_thread_item_for_message(message: &crate::app_state::AutopilotMessage) -> TaskThreadItem {
    let speaker = match message.role {
        AutopilotRole::User => "YOU",
        AutopilotRole::Codex => "AGENT",
    };
    let accent = match message.role {
        AutopilotRole::User => Hsla::from_hex(BADGE_BRANCH_ACCENT),
        AutopilotRole::Codex => status_accent(message.status),
    };
    let running = matches!(
        message.status,
        AutopilotMessageStatus::Queued | AutopilotMessageStatus::Running
    );
    let body = task_thread_message_body(message);
    TaskThreadItem {
        speaker,
        body,
        detail: match message.status {
            AutopilotMessageStatus::Done => String::new(),
            status => message_status_label(status).to_string(),
        },
        accent,
        running,
    }
}

fn task_thread_message_body(message: &crate::app_state::AutopilotMessage) -> String {
    match message.role {
        AutopilotRole::User => message.content.trim().to_string(),
        AutopilotRole::Codex => {
            let content = message
                .structured
                .as_ref()
                .and_then(|structured| {
                    if !structured.answer.trim().is_empty() {
                        Some(structured.answer.trim().to_string())
                    } else if !structured.reasoning.trim().is_empty() {
                        Some(structured.reasoning.trim().to_string())
                    } else if let Some(block) = structured.progress_blocks.last() {
                        Some(format!("{} // {}", block.title.trim(), block.status.trim()))
                    } else {
                        None
                    }
                })
                .or_else(|| {
                    let trimmed = message.content.trim();
                    (!trimmed.is_empty()).then(|| trimmed.to_string())
                });
            match (message.status, content) {
                (_, Some(content)) => content,
                (AutopilotMessageStatus::Queued, None) => {
                    format!("{} Starting your task...", spinner_frame())
                }
                (AutopilotMessageStatus::Running, None) => {
                    format!("{} Working on your task...", spinner_frame())
                }
                (AutopilotMessageStatus::Done, None) => "Task completed.".to_string(),
                (AutopilotMessageStatus::Error, None) => "Task failed.".to_string(),
            }
        }
    }
}

fn task_thread_content_height_for_items(items: &[TaskThreadItem], viewport: Bounds) -> f32 {
    let mut total = 0.0;
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            total += TASK_THREAD_ROW_GAP;
        }
        total += task_thread_row_height(item, viewport.size.width - 6.0);
    }
    if !items.is_empty() {
        total += TASK_THREAD_BOTTOM_PADDING;
    }
    total.max(viewport.size.height)
}

fn task_thread_row_height(item: &TaskThreadItem, width: f32) -> f32 {
    let bubble_width = task_thread_bubble_width(width, item);
    let content_width = (bubble_width - TASK_THREAD_BUBBLE_PAD_X * 2.0).max(120.0);
    let lines = wrap_thread_lines(item.body.as_str(), task_thread_max_chars(content_width));
    let line_count = lines.len().max(1) as f32;
    let bubble_height = TASK_THREAD_BUBBLE_PAD_Y
        + line_count * TASK_THREAD_LINE_HEIGHT
        + TASK_THREAD_BUBBLE_PAD_Y;
    let status_height = if task_thread_shows_status(item) {
        TASK_THREAD_STATUS_LINE_HEIGHT + 4.0
    } else {
        0.0
    };
    bubble_height + status_height
}

fn paint_task_thread_row(bounds: Bounds, item: &TaskThreadItem, paint: &mut PaintContext) {
    let bubble_width = task_thread_bubble_width(bounds.size.width, item);
    let status_height = if task_thread_shows_status(item) {
        TASK_THREAD_STATUS_LINE_HEIGHT + 4.0
    } else {
        0.0
    };
    let bubble_bounds = if task_thread_is_user(item) {
        Bounds::new(
            bounds.max_x() - bubble_width - 4.0,
            bounds.origin.y + status_height,
            bubble_width,
            bounds.size.height - status_height,
        )
    } else if task_thread_is_session(item) {
        Bounds::new(
            bounds.origin.x,
            bounds.origin.y + status_height,
            bounds.size.width - 6.0,
            bounds.size.height - status_height,
        )
    } else {
        Bounds::new(
            bounds.origin.x + 4.0,
            bounds.origin.y + status_height,
            bubble_width,
            bounds.size.height - status_height,
        )
    };

    let (background, border, body_color) = if task_thread_is_user(item) {
        (
            theme::bg::APP.with_alpha(0.0),
            item.accent.with_alpha(0.22),
            theme::text::PRIMARY,
        )
    } else if task_thread_is_session(item) {
        (
            theme::bg::APP.with_alpha(0.44),
            theme::border::DEFAULT.with_alpha(0.12),
            theme::text::SECONDARY,
        )
    } else {
        (
            theme::bg::APP.with_alpha(0.28),
            theme::border::DEFAULT.with_alpha(0.08),
            theme::text::PRIMARY,
        )
    };

    if task_thread_shows_status(item) {
        let status_color = if item.running {
            item.accent.with_alpha(0.88)
        } else {
            item.accent.with_alpha(0.82)
        };
        let status_width = paint.text.measure(&item.detail, 8.5);
        let status_x = if task_thread_is_user(item) {
            bubble_bounds.max_x() - status_width - 2.0
        } else {
            bubble_bounds.origin.x + 2.0
        };
        paint.scene.draw_text(paint.text.layout_mono(
            &item.detail,
            Point::new(status_x, bounds.origin.y),
            8.5,
            status_color,
        ));
    }

    paint.scene.draw_quad(
        Quad::new(bubble_bounds)
            .with_background(background)
            .with_border(border, 1.0)
            .with_corner_radius(10.0),
    );

    let content_width = (bubble_bounds.size.width - TASK_THREAD_BUBBLE_PAD_X * 2.0).max(120.0);
    let lines = wrap_thread_lines(item.body.as_str(), task_thread_max_chars(content_width));
    let mut y = bubble_bounds.origin.y + TASK_THREAD_BUBBLE_PAD_Y;
    for line in lines {
        paint.scene.draw_text(paint.text.layout_mono(
            &line,
            Point::new(bubble_bounds.origin.x + TASK_THREAD_BUBBLE_PAD_X, y),
            11.0,
            body_color,
        ));
        y += TASK_THREAD_LINE_HEIGHT;
    }
}

fn task_thread_bubble_width(width: f32, item: &TaskThreadItem) -> f32 {
    if task_thread_is_session(item) {
        (width - 6.0).max(180.0)
    } else {
        (width * TASK_THREAD_BUBBLE_MAX_WIDTH_RATIO)
            .clamp(220.0, (width - 18.0).max(220.0))
    }
}

fn task_thread_is_user(item: &TaskThreadItem) -> bool {
    item.speaker == "YOU"
}

fn task_thread_is_session(item: &TaskThreadItem) -> bool {
    item.speaker == "SESSION"
}

fn task_thread_shows_status(item: &TaskThreadItem) -> bool {
    !task_thread_is_session(item) && !item.detail.trim().is_empty()
}

fn task_thread_max_chars(content_width: f32) -> usize {
    ((content_width / 6.2).floor() as usize).max(18)
}

fn wrap_thread_lines(raw: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for source_line in raw.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let chars = source_line.chars().collect::<Vec<_>>();
        if chars.is_empty() {
            continue;
        }
        for chunk in chars.chunks(max_chars.max(8)) {
            lines.push(chunk.iter().collect::<String>());
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn spinner_frame() -> &'static str {
    const FRAMES: [&str; 4] = ["|", "/", "-", "\\"];
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let index = ((now_ms / 180) % FRAMES.len() as u128) as usize;
    FRAMES[index]
}

fn paint_empty_state_panel(
    panel_bounds: Bounds,
    title: &str,
    detail: &str,
    helper: &str,
    accent: Hsla,
    paint: &mut PaintContext,
) {
    let card = Bounds::new(
        panel_bounds.origin.x + 12.0,
        panel_bounds.origin.y + SECTION_HEADER_HEIGHT + 14.0,
        (panel_bounds.size.width - 24.0).max(0.0),
        EMPTY_STATE_CARD_HEIGHT.min((panel_bounds.size.height - SECTION_HEADER_HEIGHT - 28.0).max(0.0)),
    );
    paint.scene.draw_quad(
        Quad::new(card)
            .with_background(theme::bg::APP.with_alpha(0.58))
            .with_border(theme::border::DEFAULT.with_alpha(0.18), 1.0)
            .with_corner_radius(8.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(card.origin.x, card.origin.y, 3.0, card.size.height))
            .with_background(accent.with_alpha(0.72))
            .with_corner_radius(2.0),
    );
    let clip = Bounds::new(
        card.origin.x + 12.0,
        card.origin.y + 10.0,
        card.size.width - 24.0,
        card.size.height - 20.0,
    );
    paint.scene.push_clip(clip);
    paint.scene.draw_text(paint.text.layout_mono(
        "READY",
        Point::new(card.origin.x + 12.0, card.origin.y + 12.0),
        9.0,
        accent.with_alpha(0.82),
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        title,
        Point::new(card.origin.x + 12.0, card.origin.y + 30.0),
        13.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        detail,
        Point::new(card.origin.x + 12.0, card.origin.y + 50.0),
        9.5,
        theme::text::SECONDARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        helper,
        Point::new(card.origin.x + 12.0, card.origin.y + 67.0),
        9.0,
        theme::text::MUTED,
    ));
    paint.scene.pop_clip();
}

fn timeline_entries(
    autopilot_chat: &AutopilotChatState,
    selected_project: Option<&AutopilotProjectIdentity>,
    selected_thread: Option<&AutopilotThreadMetadata>,
    selected_thread_id: Option<&str>,
) -> Vec<TimelineEntry> {
    let mut entries = Vec::new();
    let thread_title = selected_thread
        .and_then(|thread| thread.thread_name.as_deref())
        .or(selected_thread.and_then(|thread| thread.preview.as_deref()))
        .or(selected_thread_id)
        .unwrap_or("No bound coding session");
    let thread_status = selected_thread
        .and_then(|thread| thread.status.as_deref())
        .map(humanize_compact_status)
        .unwrap_or_else(|| "idle".to_string());
    entries.push(TimelineEntry {
        kind: "SESSION",
        title: thread_title.to_string(),
        detail: Some(format!("status {thread_status}")),
        accent: Hsla::from_hex(BADGE_MUTED_ACCENT),
    });

    if let Some(project) = selected_project {
        entries.push(TimelineEntry {
            kind: "WORKSPACE",
            title: project.project_name.clone(),
            detail: Some(project.workspace_root.clone()),
            accent: Hsla::from_hex(BADGE_BRANCH_ACCENT),
        });
    }

    if let Some(thread_id) = selected_thread_id {
        let command_approvals = autopilot_chat
            .pending_command_approvals
            .iter()
            .filter(|request| request.thread_id == thread_id)
            .count();
        let file_approvals = autopilot_chat
            .pending_file_change_approvals
            .iter()
            .filter(|request| request.thread_id == thread_id)
            .count();
        let total_approvals = command_approvals + file_approvals;
        if total_approvals > 0 {
            entries.push(TimelineEntry {
                kind: "APPROVAL",
                title: format!("{total_approvals} approvals waiting"),
                detail: Some(format!(
                    "{command_approvals} command // {file_approvals} file-change"
                )),
                accent: Hsla::from_hex(BADGE_WARNING_ACCENT),
            });
        }

        if let Some(session) = autopilot_chat.terminal_sessions.get(thread_id) {
            let detail = match session.exit_code {
                Some(exit_code) => format!("{} // exit {}", session.status.label(), exit_code),
                None => format!("{} // {}", session.status.label(), session.shell),
            };
            let accent = match session.status {
                AutopilotTerminalSessionStatus::Pending => Hsla::from_hex(BADGE_RUNNING_ACCENT),
                AutopilotTerminalSessionStatus::Running => Hsla::from_hex(BADGE_RUNNING_ACCENT),
                AutopilotTerminalSessionStatus::Exited => Hsla::from_hex(BADGE_SUCCESS_ACCENT),
                AutopilotTerminalSessionStatus::Failed => Hsla::from_hex(BADGE_ERROR_ACCENT),
                AutopilotTerminalSessionStatus::Closed => Hsla::from_hex(BADGE_MUTED_ACCENT),
            };
            entries.push(TimelineEntry {
                kind: "TERMINAL",
                title: format!("local session {}", session.status.label()),
                detail: Some(detail),
                accent,
            });
        }

        if let Some(messages) = autopilot_chat.cached_thread_messages(thread_id) {
            for message in messages.iter().rev() {
                push_timeline_entries_for_message(&mut entries, message);
            }
        }
    }

    if entries.is_empty() {
        entries.push(TimelineEntry {
            kind: "SESSION",
            title: "No coding session detected yet".to_string(),
            detail: Some(
                "Bind the pane to a repo with an active thread to unlock the timeline.".to_string(),
            ),
            accent: Hsla::from_hex(BADGE_MUTED_ACCENT),
        });
    }

    entries
}

fn push_timeline_entries_for_message(
    entries: &mut Vec<TimelineEntry>,
    message: &crate::app_state::AutopilotMessage,
) {
    match message.role {
        AutopilotRole::User => {
            entries.push(TimelineEntry {
                kind: "PROMPT",
                title: summarize_line(message.content.as_str(), 112),
                detail: Some(message_status_label(message.status).to_string()),
                accent: Hsla::from_hex(BADGE_BRANCH_ACCENT),
            });
        }
        AutopilotRole::Codex => {
            if let Some(structured) = message.structured.as_ref() {
                for event in structured.events.iter().rev() {
                    entries.push(TimelineEntry {
                        kind: event_kind_label(event),
                        title: summarize_line(event, 112),
                        detail: structured
                            .status
                            .as_deref()
                            .map(humanize_compact_status),
                        accent: event_accent(event),
                    });
                }
                for block in structured.progress_blocks.iter().rev() {
                    let detail = if block.rows.is_empty() {
                        block.status.clone()
                    } else {
                        format!("{} // {} rows", block.status, block.rows.len())
                    };
                    entries.push(TimelineEntry {
                        kind: "PROGRESS",
                        title: summarize_line(block.title.as_str(), 112),
                        detail: Some(detail),
                        accent: status_accent_from_text(block.status.as_str()),
                    });
                }
                let rendered = if !structured.answer.trim().is_empty() {
                    structured.answer.as_str()
                } else if !structured.reasoning.trim().is_empty() {
                    structured.reasoning.as_str()
                } else {
                    message.content.as_str()
                };
                if !rendered.trim().is_empty() {
                    entries.push(TimelineEntry {
                        kind: "AGENT",
                        title: summarize_line(rendered, 112),
                        detail: Some(message_status_label(message.status).to_string()),
                        accent: status_accent(message.status),
                    });
                }
            } else {
                entries.push(TimelineEntry {
                    kind: "AGENT",
                    title: summarize_line(message.content.as_str(), 112),
                    detail: Some(message_status_label(message.status).to_string()),
                    accent: status_accent(message.status),
                });
            }
        }
    }
}

fn terminal_meta_line(session: &AutopilotTerminalSession) -> String {
    let mut parts = vec![format!("status:{}", session.status.label())];
    if let Some(pid) = session.pid {
        parts.push(format!("pid:{pid}"));
    }
    if !session.shell.trim().is_empty() {
        parts.push(format!("shell:{}", summarize_line(session.shell.as_str(), 18)));
    }
    parts.push(format!("size:{}x{}", session.cols, session.rows));
    if !session.workspace_root.trim().is_empty() {
        parts.push(format!(
            "ws:{}",
            summarize_line(session.workspace_root.as_str(), 24)
        ));
    }
    if let Some(exit_code) = session.exit_code {
        parts.push(format!("exit:{exit_code}"));
    }
    parts.join("  •  ")
}

fn terminal_stream_color(stream: &wgpui::components::sections::TerminalStream) -> Hsla {
    match stream {
        wgpui::components::sections::TerminalStream::Stdout => theme::text::PRIMARY,
        wgpui::components::sections::TerminalStream::Stderr => theme::status::ERROR,
    }
}

fn terminal_status_accent(status: AutopilotTerminalSessionStatus) -> Hsla {
    match status {
        AutopilotTerminalSessionStatus::Pending => Hsla::from_hex(BADGE_RUNNING_ACCENT),
        AutopilotTerminalSessionStatus::Running => Hsla::from_hex(BADGE_RUNNING_ACCENT),
        AutopilotTerminalSessionStatus::Exited => Hsla::from_hex(BADGE_SUCCESS_ACCENT),
        AutopilotTerminalSessionStatus::Failed => Hsla::from_hex(BADGE_ERROR_ACCENT),
        AutopilotTerminalSessionStatus::Closed => Hsla::from_hex(BADGE_MUTED_ACCENT),
    }
}

fn terminal_content_height_for_session(
    session: Option<&AutopilotTerminalSession>,
    viewport: Bounds,
) -> f32 {
    let line_count = session.map(|value| value.lines.len()).unwrap_or(1).max(1);
    ((line_count as f32 * TERMINAL_LINE_HEIGHT) + 20.0).max(viewport.size.height)
}

fn review_content_height_for_artifact(
    artifact: Option<&AutopilotDiffArtifact>,
    viewport: Bounds,
) -> f32 {
    let row_count = artifact.map(|value| value.files.len()).unwrap_or(1).max(1);
    let rows_height = row_count as f32 * REVIEW_FILE_ROW_HEIGHT;
    let gaps_height = row_count.saturating_sub(1) as f32 * REVIEW_FILE_ROW_GAP;
    (rows_height + gaps_height + 6.0).max(viewport.size.height)
}

fn diff_viewer_content_height_for_artifact(
    artifact: Option<&AutopilotDiffArtifact>,
    selected_path: Option<&str>,
    viewport: Bounds,
) -> f32 {
    let Some(artifact) = artifact else {
        return viewport.size.height;
    };
    let Some(selected_path) = selected_path else {
        return viewport.size.height;
    };
    let line_count = diff_lines_for_file(artifact, selected_path).len().max(1);
    let frame_body_height = (viewport.size.height - 54.0).max(0.0);
    ((line_count as f32 * DIFF_LINE_HEIGHT) + 12.0).max(frame_body_height)
}

fn timeline_content_height_for_entries(entry_count: usize, viewport: Bounds) -> f32 {
    let rows_height = entry_count as f32 * TIMELINE_ROW_HEIGHT;
    let gaps_height = entry_count.saturating_sub(1) as f32 * TIMELINE_ROW_GAP;
    (rows_height + gaps_height + 4.0).max(viewport.size.height)
}

fn resolved_selected_diff_file_path<'a>(
    pane_state: &'a CodingAgentPaneState,
    artifact: Option<&'a AutopilotDiffArtifact>,
) -> Option<&'a str> {
    let artifact = artifact?;
    if let Some(path) = pane_state.selected_diff_file_path.as_deref()
        && artifact.files.iter().any(|file| file.path == path)
    {
        return Some(path);
    }
    artifact.files.first().map(|file| file.path.as_str())
}

fn diff_lines_for_file(artifact: &AutopilotDiffArtifact, selected_path: &str) -> Vec<String> {
    let mut sections: Vec<Vec<String>> = Vec::new();
    let mut current_lines: Vec<String> = Vec::new();
    let mut current_path: Option<String> = None;

    let flush_section =
        |sections: &mut Vec<Vec<String>>, current_path: &Option<String>, lines: &mut Vec<String>| {
            if current_path.as_deref() == Some(selected_path) && !lines.is_empty() {
                sections.push(std::mem::take(lines));
            } else {
                lines.clear();
            }
        };

    for line in artifact.raw_diff.lines() {
        if let Some(path) = parse_diff_header_path(line) {
            flush_section(&mut sections, &current_path, &mut current_lines);
            current_path = Some(path);
            current_lines.push(line.to_string());
            continue;
        }
        if line.starts_with("+++ b/") {
            current_path = Some(line.trim_start_matches("+++ b/").to_string());
        } else if line.starts_with("rename to ") {
            current_path = Some(line.trim_start_matches("rename to ").to_string());
        }
        if current_path.is_some() {
            current_lines.push(line.to_string());
        }
    }
    flush_section(&mut sections, &current_path, &mut current_lines);

    if sections.is_empty() {
        return artifact
            .raw_diff
            .lines()
            .take(40)
            .map(ToString::to_string)
            .collect();
    }

    let mut lines = Vec::new();
    for (index, section) in sections.into_iter().enumerate() {
        if index > 0 {
            lines.push(String::new());
        }
        lines.extend(section);
    }
    lines
}

fn parse_diff_header_path(line: &str) -> Option<String> {
    if !line.starts_with("diff --git ") {
        return None;
    }
    let mut parts = line.split_whitespace();
    let _ = parts.next();
    let _ = parts.next();
    let a_path = parts.next()?;
    let b_path = parts.next()?;
    Some(
        b_path
            .strip_prefix("b/")
            .or_else(|| a_path.strip_prefix("a/"))
            .unwrap_or(b_path)
            .to_string(),
    )
}

fn summarize_line(raw: &str, max_chars: usize) -> String {
    let line = raw
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("");
    if line.chars().count() <= max_chars {
        return line.to_string();
    }
    let mut summary = String::new();
    for ch in line.chars().take(max_chars.saturating_sub(1)) {
        summary.push(ch);
    }
    summary.push('…');
    summary
}

fn humanize_compact_status(raw: &str) -> String {
    raw.replace('_', " ")
        .replace("inProgress", "in progress")
        .replace("onRequest", "on request")
}

fn message_status_label(status: AutopilotMessageStatus) -> &'static str {
    match status {
        AutopilotMessageStatus::Queued => "queued",
        AutopilotMessageStatus::Running => "running",
        AutopilotMessageStatus::Done => "done",
        AutopilotMessageStatus::Error => "error",
    }
}

fn status_accent(status: AutopilotMessageStatus) -> Hsla {
    match status {
        AutopilotMessageStatus::Queued => Hsla::from_hex(BADGE_WARNING_ACCENT),
        AutopilotMessageStatus::Running => Hsla::from_hex(BADGE_RUNNING_ACCENT),
        AutopilotMessageStatus::Done => Hsla::from_hex(BADGE_SUCCESS_ACCENT),
        AutopilotMessageStatus::Error => Hsla::from_hex(BADGE_ERROR_ACCENT),
    }
}

fn status_accent_from_text(raw: &str) -> Hsla {
    let normalized = raw.to_ascii_lowercase();
    if normalized.contains("fail") || normalized.contains("error") {
        Hsla::from_hex(BADGE_ERROR_ACCENT)
    } else if normalized.contains("run") || normalized.contains("progress") {
        Hsla::from_hex(BADGE_RUNNING_ACCENT)
    } else if normalized.contains("done") || normalized.contains("complete") {
        Hsla::from_hex(BADGE_SUCCESS_ACCENT)
    } else if normalized.contains("wait") || normalized.contains("approval") {
        Hsla::from_hex(BADGE_WARNING_ACCENT)
    } else {
        Hsla::from_hex(BADGE_MUTED_ACCENT)
    }
}

fn event_kind_label(event: &str) -> &'static str {
    let normalized = event.to_ascii_lowercase();
    if normalized.contains("interrupt") || normalized.contains("cancel") {
        "INTERRUPT"
    } else if normalized.contains("fail") || normalized.contains("error") {
        "FAILURE"
    } else if normalized.contains("tool") || normalized.contains("command") {
        "ACTION"
    } else {
        "EVENT"
    }
}

fn event_accent(event: &str) -> Hsla {
    let normalized = event.to_ascii_lowercase();
    if normalized.contains("interrupt") || normalized.contains("cancel") {
        Hsla::from_hex(BADGE_WARNING_ACCENT)
    } else if normalized.contains("fail") || normalized.contains("error") {
        Hsla::from_hex(BADGE_ERROR_ACCENT)
    } else if normalized.contains("tool") || normalized.contains("command") {
        Hsla::from_hex(BADGE_MODE_ACCENT)
    } else {
        Hsla::from_hex(BADGE_MUTED_ACCENT)
    }
}

fn paint_header_mini_button(bounds: Bounds, label: &str, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::HOVER)
            .with_border(theme::border::DEFAULT, 1.0)
            .with_corner_radius(8.0),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        label,
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 9.0),
        13.0,
        theme::text::PRIMARY,
    ));
}

fn paint_badge(bounds: Bounds, label: &str, value: &str, accent: Hsla, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::APP.with_alpha(0.72))
            .with_border(accent.with_alpha(0.18), 1.0)
            .with_corner_radius(8.0),
    );
    let clip = Bounds::new(
        bounds.origin.x + 8.0,
        bounds.origin.y + 6.0,
        bounds.size.width - 16.0,
        bounds.size.height - 12.0,
    );
    paint.scene.push_clip(clip);
    paint.scene.draw_text(paint.text.layout_mono(
        label,
        Point::new(bounds.origin.x + 8.0, bounds.origin.y + 7.0),
        8.5,
        accent.with_alpha(0.82),
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        value,
        Point::new(bounds.origin.x + 8.0, bounds.origin.y + 18.0),
        10.0,
        theme::text::PRIMARY,
    ));
    paint.scene.pop_clip();
}

fn paint_section(bounds: Bounds, title: &str, subtitle: &str, quiet: bool, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(if quiet {
                theme::bg::APP.with_alpha(0.86)
            } else {
                theme::bg::APP
            })
            .with_corner_radius(CARD_RADIUS),
    );

    let header = Bounds::new(
        bounds.origin.x,
        bounds.origin.y,
        bounds.size.width,
        SECTION_HEADER_HEIGHT,
    );
    paint.scene.draw_quad(
        Quad::new(header)
            .with_background(if quiet {
                theme::bg::ELEVATED.with_alpha(0.84)
            } else {
                theme::bg::ELEVATED
            })
            .with_corner_radius(CARD_RADIUS),
    );

    paint.scene.draw_text(paint.text.layout_mono(
        title,
        Point::new(header.origin.x + 12.0, header.origin.y + 11.0),
        12.0,
        theme::text::PRIMARY,
    ));
    if !subtitle.is_empty() {
        paint.scene.draw_text(paint.text.layout(
            subtitle,
            Point::new(header.origin.x + 12.0, header.origin.y + 24.0),
            10.0,
            theme::text::MUTED,
        ));
    }
}

fn resolved_selected_project_id(
    pane_state: &CodingAgentPaneState,
    autopilot_chat: &AutopilotChatState,
) -> Option<String> {
    if let Some(project_id) = pane_state.selected_project_id.as_ref() {
        if autopilot_chat.project_registry.contains_key(project_id) {
            return Some(project_id.clone());
        }
    }
    if let Some(workspace_root) = pane_state.selected_workspace_root.as_deref()
        && let Some(project) = autopilot_chat
            .project_registry
            .values()
            .find(|project| project.workspace_root == workspace_root)
    {
        return Some(project.project_id.clone());
    }

    if let Some(thread_id) = pane_state.active_thread_id.as_deref() {
        if let Some(project) = autopilot_chat.project_for_thread(thread_id) {
            return Some(project.project_id.clone());
        }
    }
    None
}

fn resolved_selected_workspace_root(
    pane_state: &CodingAgentPaneState,
    selected_project: Option<&AutopilotProjectIdentity>,
) -> Option<String> {
    pane_state
        .selected_workspace_root
        .as_ref()
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .or_else(|| selected_project.map(|project| project.workspace_root.clone()))
}

fn resolved_selected_thread_id(
    pane_state: &CodingAgentPaneState,
    autopilot_chat: &AutopilotChatState,
    selected_project: Option<&AutopilotProjectIdentity>,
) -> Option<String> {
    let thread_id = pane_state.active_thread_id.as_ref()?;
    if let Some(project) = selected_project
        && project.thread_ids.iter().any(|candidate| candidate == thread_id)
    {
        return Some(thread_id.clone());
    }
    pane_state
        .selected_workspace_root
        .as_deref()
        .and_then(|workspace_root| {
            autopilot_chat
                .thread_metadata
                .get(thread_id)
                .and_then(|metadata| metadata.workspace_root.as_deref().or(metadata.cwd.as_deref()))
                .filter(|candidate| *candidate == workspace_root)
                .map(|_| thread_id.clone())
        })
}

fn sorted_project_ids(autopilot_chat: &AutopilotChatState) -> Vec<String> {
    let mut projects = autopilot_chat
        .project_registry
        .values()
        .collect::<Vec<&AutopilotProjectIdentity>>();
    projects.sort_by(|left, right| {
        left.project_name
            .to_ascii_lowercase()
            .cmp(&right.project_name.to_ascii_lowercase())
            .then_with(|| {
                left.workspace_root
                    .to_ascii_lowercase()
                    .cmp(&right.workspace_root.to_ascii_lowercase())
            })
    });
    projects
        .into_iter()
        .map(|project| project.project_id.clone())
        .collect()
}

fn mode_label(
    selected_project: Option<&AutopilotProjectIdentity>,
    autopilot_chat: &AutopilotChatState,
) -> String {
    let approval = selected_project
        .and_then(|project| project.defaults.approval_policy)
        .unwrap_or(autopilot_chat.approval_mode);
    let sandbox = selected_project
        .and_then(|project| project.defaults.sandbox_mode)
        .unwrap_or(autopilot_chat.sandbox_mode);
    format!(
        "{} + {}",
        approval_mode_label(approval).to_ascii_uppercase(),
        sandbox_mode_label(sandbox).to_ascii_uppercase()
    )
}

fn approvals_for_selection(
    autopilot_chat: &AutopilotChatState,
    selected_project: Option<&AutopilotProjectIdentity>,
) -> String {
    let Some(project) = selected_project else {
        return "CLEAR".to_string();
    };
    let command_count = autopilot_chat
        .pending_command_approvals
        .iter()
        .filter(|request| project.thread_ids.iter().any(|thread_id| thread_id == &request.thread_id))
        .count();
    let file_count = autopilot_chat
        .pending_file_change_approvals
        .iter()
        .filter(|request| project.thread_ids.iter().any(|thread_id| thread_id == &request.thread_id))
        .count();
    let total = command_count + file_count;
    if total == 0 {
        "CLEAR".to_string()
    } else {
        format!("{total} PENDING")
    }
}

fn approval_bar_summary(
    autopilot_chat: &AutopilotChatState,
    selected_project: Option<&AutopilotProjectIdentity>,
    selected_thread_id: Option<&str>,
) -> ApprovalBarSummary {
    let Some(project) = selected_project else {
        return ApprovalBarSummary {
            total: 0,
            title: "No approvals pending".to_string(),
            detail: "Bind this pane to a repo to watch for command and file approvals."
                .to_string(),
            accent: Hsla::from_hex(BADGE_MUTED_ACCENT),
        };
    };

    let thread_ids = &project.thread_ids;
    let command_count = autopilot_chat
        .pending_command_approvals
        .iter()
        .filter(|request| thread_ids.iter().any(|thread_id| thread_id == &request.thread_id))
        .count();
    let file_count = autopilot_chat
        .pending_file_change_approvals
        .iter()
        .filter(|request| thread_ids.iter().any(|thread_id| thread_id == &request.thread_id))
        .count();
    let total = command_count + file_count;

    if total == 0 {
        return ApprovalBarSummary {
            total,
            title: "No approvals pending".to_string(),
            detail: "Commands and file changes needing review will appear here.".to_string(),
            accent: Hsla::from_hex(BADGE_MUTED_ACCENT),
        };
    }

    let detail = if let Some(request) = autopilot_chat
        .pending_command_approvals
        .iter()
        .find(|request| thread_ids.iter().any(|thread_id| thread_id == &request.thread_id))
    {
        let thread_scope = if selected_thread_id == Some(request.thread_id.as_str()) {
            "current thread".to_string()
        } else {
            format!("thread {}", thread_label_for_id(autopilot_chat, request.thread_id.as_str()))
        };
        format!(
            "COMMAND  //  {thread_scope}  //  {}",
            summarize_line(
                request
                    .command
                    .as_deref()
                    .or(request.reason.as_deref())
                    .unwrap_or("Command execution needs approval."),
                88
            )
        )
    } else if let Some(request) = autopilot_chat
        .pending_file_change_approvals
        .iter()
        .find(|request| thread_ids.iter().any(|thread_id| thread_id == &request.thread_id))
    {
        let thread_scope = if selected_thread_id == Some(request.thread_id.as_str()) {
            "current thread".to_string()
        } else {
            format!("thread {}", thread_label_for_id(autopilot_chat, request.thread_id.as_str()))
        };
        format!(
            "FILE CHANGE  //  {thread_scope}  //  {}",
            summarize_line(
                request
                    .grant_root
                    .as_deref()
                    .or(request.reason.as_deref())
                    .unwrap_or("Workspace access needs approval."),
                88
            )
        )
    } else {
        "Approval details unavailable.".to_string()
    };

    ApprovalBarSummary {
        total,
        title: if total == 1 {
            "1 approval waiting".to_string()
        } else {
            format!("{total} approvals waiting")
        },
        detail,
        accent: Hsla::from_hex(BADGE_WARNING_ACCENT),
    }
}

fn approval_detail_card(
    autopilot_chat: &AutopilotChatState,
    selected_project: Option<&AutopilotProjectIdentity>,
    selected_thread_id: Option<&str>,
) -> Option<ApprovalDetailCard> {
    let project = selected_project?;
    let thread_ids = &project.thread_ids;
    let command_count = autopilot_chat
        .pending_command_approvals
        .iter()
        .filter(|request| thread_ids.iter().any(|thread_id| thread_id == &request.thread_id))
        .count();
    let file_count = autopilot_chat
        .pending_file_change_approvals
        .iter()
        .filter(|request| thread_ids.iter().any(|thread_id| thread_id == &request.thread_id))
        .count();
    let total = command_count + file_count;
    let queue_title = if total == 1 {
        "1 request is waiting in this repo.".to_string()
    } else {
        format!("{total} requests are waiting in this repo.")
    };
    let queue_detail = format!("{command_count} command  //  {file_count} file-change");
    let queue_aux = Some(format!(
        "repo  //  {}  //  {} threads bound",
        summarize_line(project.project_name.as_str(), 28),
        project.thread_ids.len()
    ));
    if let Some(request) = autopilot_chat
        .pending_command_approvals
        .iter()
        .find(|request| thread_ids.iter().any(|thread_id| thread_id == &request.thread_id))
    {
        let thread_label = if selected_thread_id == Some(request.thread_id.as_str()) {
            "Current thread".to_string()
        } else {
            thread_label_for_id(autopilot_chat, request.thread_id.as_str())
        };
        let thread_value = summarize_line(thread_label.as_str(), 108);
        return Some(ApprovalDetailCard {
            kind: "COMMAND APPROVAL",
            thread_label,
            headline: summarize_line(
                request
                    .command
                    .as_deref()
                    .unwrap_or("Command execution requires approval."),
                88,
            ),
            detail: request
                .reason
                .as_deref()
                .map(|value| summarize_line(value, 140))
                .unwrap_or_else(|| "Codex is requesting permission to execute a local command.".to_string()),
            queue_title,
            queue_detail,
            queue_aux,
            helper:
                "Review the command context below, then use Approve or Deny in the fixed bar above."
                    .to_string(),
            accent: Hsla::from_hex(BADGE_WARNING_ACCENT),
            rows: vec![
                ApprovalDetailRow {
                    label: "TYPE",
                    value: "command execution".to_string(),
                },
                ApprovalDetailRow {
                    label: "THREAD",
                    value: thread_value,
                },
                ApprovalDetailRow {
                    label: "COMMAND",
                    value: summarize_line(
                        request
                            .command
                            .as_deref()
                            .unwrap_or("Command unavailable."),
                        108,
                    ),
                },
                ApprovalDetailRow {
                    label: "CWD",
                    value: summarize_line(
                        request.cwd.as_deref().unwrap_or("Workspace root unavailable."),
                        108,
                    ),
                },
                ApprovalDetailRow {
                    label: "ITEM",
                    value: summarize_line(request.item_id.as_str(), 108),
                },
                ApprovalDetailRow {
                    label: "TURN",
                    value: summarize_line(request.turn_id.as_str(), 108),
                },
            ],
        });
    }
    if let Some(request) = autopilot_chat
        .pending_file_change_approvals
        .iter()
        .find(|request| thread_ids.iter().any(|thread_id| thread_id == &request.thread_id))
    {
        let thread_label = if selected_thread_id == Some(request.thread_id.as_str()) {
            "Current thread".to_string()
        } else {
            thread_label_for_id(autopilot_chat, request.thread_id.as_str())
        };
        let thread_value = summarize_line(thread_label.as_str(), 108);
        return Some(ApprovalDetailCard {
            kind: "FILE APPROVAL",
            thread_label,
            headline: summarize_line(
                request
                    .grant_root
                    .as_deref()
                    .unwrap_or("Workspace root access requires approval."),
                88,
            ),
            detail: request
                .reason
                .as_deref()
                .map(|value| summarize_line(value, 140))
                .unwrap_or_else(|| "Codex is requesting permission to change files in the local workspace.".to_string()),
            queue_title,
            queue_detail,
            queue_aux,
            helper:
                "Review the requested grant root below, then use Approve or Deny in the fixed bar above."
                    .to_string(),
            accent: Hsla::from_hex(BADGE_WARNING_ACCENT),
            rows: vec![
                ApprovalDetailRow {
                    label: "TYPE",
                    value: "file-change access".to_string(),
                },
                ApprovalDetailRow {
                    label: "GRANT ROOT",
                    value: summarize_line(
                        request
                            .grant_root
                            .as_deref()
                            .unwrap_or("Grant root unavailable."),
                        108,
                    ),
                },
                ApprovalDetailRow {
                    label: "THREAD",
                    value: thread_value,
                },
                ApprovalDetailRow {
                    label: "ITEM",
                    value: summarize_line(request.item_id.as_str(), 108),
                },
                ApprovalDetailRow {
                    label: "TURN",
                    value: summarize_line(request.turn_id.as_str(), 108),
                },
            ],
        });
    }
    None
}

fn thread_label_for_id(autopilot_chat: &AutopilotChatState, thread_id: &str) -> String {
    autopilot_chat
        .thread_metadata
        .get(thread_id)
        .and_then(|thread| {
            thread
                .thread_name
                .as_deref()
                .or(thread.preview.as_deref())
                .map(ToString::to_string)
        })
        .unwrap_or_else(|| summarize_line(thread_id, 18))
}

fn session_status_for_selection(
    autopilot_chat: &AutopilotChatState,
    selected_thread: Option<&crate::app_state::AutopilotThreadMetadata>,
    selected_thread_id: Option<&str>,
) -> (String, Hsla) {
    if let Some(thread_id) = selected_thread_id {
        if autopilot_chat.active_thread_id.as_deref() == Some(thread_id) {
            if !autopilot_chat.pending_command_approvals.is_empty()
                || !autopilot_chat.pending_file_change_approvals.is_empty()
            {
                return (
                    "AWAITING APPROVAL".to_string(),
                    Hsla::from_hex(BADGE_WARNING_ACCENT),
                );
            }
            if autopilot_chat.active_turn_id.is_some() {
                return (
                    "RUNNING".to_string(),
                    Hsla::from_hex(BADGE_RUNNING_ACCENT),
                );
            }
        }

        if let Some(session) = autopilot_chat.terminal_sessions.get(thread_id) {
            let terminal_status = session.status;
            let label = terminal_status.label().to_ascii_uppercase();
            let accent = match terminal_status {
                AutopilotTerminalSessionStatus::Pending => Hsla::from_hex(BADGE_RUNNING_ACCENT),
                AutopilotTerminalSessionStatus::Running => Hsla::from_hex(BADGE_RUNNING_ACCENT),
                AutopilotTerminalSessionStatus::Exited => Hsla::from_hex(BADGE_SUCCESS_ACCENT),
                AutopilotTerminalSessionStatus::Failed => Hsla::from_hex(BADGE_ERROR_ACCENT),
                AutopilotTerminalSessionStatus::Closed => Hsla::from_hex(BADGE_MUTED_ACCENT),
            };
            return (label, accent);
        }
    }

    if let Some(status) = selected_thread.and_then(|thread| thread.status.as_deref()) {
        return (
            status.replace('_', " ").to_ascii_uppercase(),
            Hsla::from_hex(BADGE_MUTED_ACCENT),
        );
    }

    ("IDLE".to_string(), Hsla::from_hex(BADGE_MUTED_ACCENT))
}

fn approval_mode_label(policy: AskForApproval) -> &'static str {
    match policy {
        AskForApproval::Never => "never",
        AskForApproval::OnFailure => "on-failure",
        AskForApproval::OnRequest => "on-request",
        AskForApproval::UnlessTrusted => "untrusted",
        AskForApproval::Reject { .. } => "reject",
    }
}

fn sandbox_mode_label(mode: SandboxMode) -> &'static str {
    match mode {
        SandboxMode::DangerFullAccess => "danger",
        SandboxMode::WorkspaceWrite => "workspace",
        SandboxMode::ReadOnly => "read-only",
    }
}

pub fn dispatch_input_event(state: &mut RenderState, event: &InputEvent) -> bool {
    let top_pane = state
        .panes
        .iter()
        .filter(|pane| pane.kind == PaneKind::CodingAgent)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| pane.bounds);
    let Some(bounds) = top_pane else {
        return false;
    };

    let content_bounds = crate::pane_system::pane_content_bounds(bounds);
    let selected_project_id = resolved_selected_project_id(&state.coding_agent, &state.autopilot_chat);
    let selected_project = selected_project_id
        .as_deref()
        .and_then(|id| state.autopilot_chat.project_registry.get(id));
    let selected_thread_id = resolved_selected_thread_id(
        &state.coding_agent,
        &state.autopilot_chat,
        selected_project,
    );
    let has_workspace =
        resolved_selected_workspace_root(&state.coding_agent, selected_project).is_some();
    let composer_bounds = composer_input_bounds(content_bounds);
    let terminal_bounds = terminal_input_bounds(content_bounds);
    state
        .coding_agent_inputs
        .composer
        .set_max_width(composer_bounds.size.width.max(140.0));
    state
        .coding_agent_inputs
        .terminal_input
        .set_max_width(terminal_bounds.size.width.max(120.0));

    let composer_handled = state
        .coding_agent_inputs
        .composer
        .event(event, composer_bounds, &mut state.event_context)
        .is_handled();
    if state.coding_agent_inputs.composer.is_focused() {
        state.coding_agent_inputs.terminal_input.blur();
    }

    let terminal_enabled = terminal_input_enabled_for_selection(
        &state.autopilot_chat,
        has_workspace,
        selected_thread_id.as_deref(),
    );
    let terminal_handled = if terminal_enabled {
        let handled = state
            .coding_agent_inputs
            .terminal_input
            .event(event, terminal_bounds, &mut state.event_context)
            .is_handled();
        if state.coding_agent_inputs.terminal_input.is_focused() {
            state.coding_agent_inputs.composer.blur();
        }
        handled
    } else {
        if state.coding_agent_inputs.terminal_input.is_focused() {
            state.coding_agent_inputs.terminal_input.blur();
        }
        false
    };

    composer_handled || terminal_handled
}
