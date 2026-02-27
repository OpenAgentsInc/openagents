use wgpui::PaintContext;

use crate::app_state::{
    AgentProfileStatePaneState, AgentScheduleTickPaneState, TrajectoryAuditPaneState,
};
use crate::pane_renderer::{
    paint_action_button, paint_label_line, paint_multiline_phrase, paint_source_badge,
    paint_state_summary,
};
use crate::pane_system::{
    agent_profile_publish_profile_button_bounds, agent_profile_publish_state_button_bounds,
    agent_profile_update_goals_button_bounds, agent_schedule_apply_button_bounds,
    agent_schedule_inspect_button_bounds, agent_schedule_manual_tick_button_bounds,
    trajectory_filter_button_bounds, trajectory_open_session_button_bounds,
    trajectory_verify_button_bounds,
};

pub fn paint_agent_profile_state_pane(
    content_bounds: wgpui::Bounds,
    pane_state: &AgentProfileStatePaneState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "runtime", paint);

    let publish_profile = agent_profile_publish_profile_button_bounds(content_bounds);
    let publish_state = agent_profile_publish_state_button_bounds(content_bounds);
    let update_goals = agent_profile_update_goals_button_bounds(content_bounds);

    paint_action_button(publish_profile, "Publish Profile", paint);
    paint_action_button(publish_state, "Publish State", paint);
    paint_action_button(update_goals, "Update Goals", paint);

    let mut y = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        publish_profile.max_y() + 12.0,
        pane_state.load_state,
        &format!("State: {}", pane_state.load_state.label()),
        pane_state.last_action.as_deref(),
        pane_state.last_error.as_deref(),
    );

    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Profile",
        &pane_state.profile_name,
    );
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "About",
        &pane_state.profile_about,
    );
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Goals",
        &pane_state.goals_summary,
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "39200 profile",
        pane_state.profile_event_id.as_deref().unwrap_or("n/a"),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "39201 state",
        pane_state.state_event_id.as_deref().unwrap_or("n/a"),
    );
    let _ = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "39203 goals",
        pane_state.goals_event_id.as_deref().unwrap_or("n/a"),
    );
}

pub fn paint_agent_schedule_tick_pane(
    content_bounds: wgpui::Bounds,
    pane_state: &AgentScheduleTickPaneState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "runtime", paint);

    let apply_schedule = agent_schedule_apply_button_bounds(content_bounds);
    let manual_tick = agent_schedule_manual_tick_button_bounds(content_bounds);
    let inspect = agent_schedule_inspect_button_bounds(content_bounds);

    paint_action_button(apply_schedule, "Apply Schedule", paint);
    paint_action_button(manual_tick, "Manual Tick", paint);
    paint_action_button(inspect, "Inspect Result", paint);

    let mut y = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        apply_schedule.max_y() + 12.0,
        pane_state.load_state,
        &format!("State: {}", pane_state.load_state.label()),
        pane_state.last_action.as_deref(),
        pane_state.last_error.as_deref(),
    );

    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Heartbeat (s)",
        &pane_state.heartbeat_seconds.to_string(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Next tick reason",
        &pane_state.next_tick_reason,
    );
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Last tick outcome",
        &pane_state.last_tick_outcome,
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "39202 schedule",
        pane_state.schedule_event_id.as_deref().unwrap_or("n/a"),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "39210 tick request",
        pane_state.tick_request_event_id.as_deref().unwrap_or("n/a"),
    );
    let _ = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "39211 tick result",
        pane_state.tick_result_event_id.as_deref().unwrap_or("n/a"),
    );
}

pub fn paint_trajectory_audit_pane(
    content_bounds: wgpui::Bounds,
    pane_state: &TrajectoryAuditPaneState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "runtime", paint);

    let open_session = trajectory_open_session_button_bounds(content_bounds);
    let cycle_filter = trajectory_filter_button_bounds(content_bounds);
    let verify = trajectory_verify_button_bounds(content_bounds);

    paint_action_button(open_session, "Open Session", paint);
    paint_action_button(cycle_filter, "Cycle Filter", paint);
    paint_action_button(verify, "Verify Hash", paint);

    let mut y = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        open_session.max_y() + 12.0,
        pane_state.load_state,
        &format!("State: {}", pane_state.load_state.label()),
        pane_state.last_action.as_deref(),
        pane_state.last_error.as_deref(),
    );

    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Session",
        pane_state.active_session_id.as_deref().unwrap_or("n/a"),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Step filter",
        &pane_state.step_filter,
    );
    let _ = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Verified hash",
        pane_state.verified_hash.as_deref().unwrap_or("n/a"),
    );
}
