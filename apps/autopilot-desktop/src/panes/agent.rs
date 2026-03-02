use wgpui::PaintContext;

use crate::app_state::{
    AgentProfileStatePaneState, AgentScheduleTickPaneState, TrajectoryAuditPaneState,
};
use crate::pane_renderer::{
    paint_action_button, paint_label_line, paint_multiline_phrase, paint_source_badge,
    paint_state_summary,
};
use crate::pane_system::{
    agent_profile_abort_goal_button_bounds, agent_profile_create_goal_button_bounds,
    agent_profile_publish_profile_button_bounds, agent_profile_publish_state_button_bounds,
    agent_profile_receipt_button_bounds, agent_profile_start_goal_button_bounds,
    agent_profile_update_goals_button_bounds, agent_schedule_apply_button_bounds,
    agent_schedule_inspect_button_bounds, agent_schedule_manual_tick_button_bounds,
    agent_schedule_toggle_os_scheduler_button_bounds, trajectory_filter_button_bounds,
    trajectory_open_session_button_bounds, trajectory_verify_button_bounds,
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
    let create_goal = agent_profile_create_goal_button_bounds(content_bounds);
    let start_goal = agent_profile_start_goal_button_bounds(content_bounds);
    let abort_goal = agent_profile_abort_goal_button_bounds(content_bounds);
    let inspect_receipt = agent_profile_receipt_button_bounds(content_bounds);

    paint_action_button(publish_profile, "Publish Profile", paint);
    paint_action_button(publish_state, "Publish State", paint);
    paint_action_button(update_goals, "Update Goals", paint);
    paint_action_button(create_goal, "Create Goal", paint);
    paint_action_button(start_goal, "Start Goal", paint);
    paint_action_button(abort_goal, "Abort Goal", paint);
    paint_action_button(inspect_receipt, "Inspect Receipt", paint);

    let mut y = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        create_goal.max_y() + 12.0,
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
        "Selected goal",
        pane_state.selected_goal_id.as_deref().unwrap_or("n/a"),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Goal status",
        &pane_state.selected_goal_status,
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Goal attempts",
        &pane_state.selected_goal_attempts.to_string(),
    );
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Selected skills",
        &pane_state.selected_goal_selected_skills,
    );
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Last receipt",
        &pane_state.selected_goal_receipt_summary,
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
    let toggle_os_scheduler = agent_schedule_toggle_os_scheduler_button_bounds(content_bounds);

    paint_action_button(apply_schedule, "Apply Schedule", paint);
    paint_action_button(manual_tick, "Manual Tick", paint);
    paint_action_button(inspect, "Inspect Result", paint);
    paint_action_button(
        toggle_os_scheduler,
        if pane_state.os_scheduler_enabled {
            "Disable OS Scheduler"
        } else {
            "Enable OS Scheduler"
        },
        paint,
    );

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
        "Selected goal",
        pane_state.selected_goal_id.as_deref().unwrap_or("n/a"),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Scheduler mode",
        &pane_state.scheduler_mode,
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Next goal run (epoch)",
        &pane_state
            .next_goal_run_epoch_seconds
            .map(|value| value.to_string())
            .unwrap_or_else(|| "n/a".to_string()),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Last goal run (epoch)",
        &pane_state
            .last_goal_run_epoch_seconds
            .map(|value| value.to_string())
            .unwrap_or_else(|| "n/a".to_string()),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Missed-run policy",
        &pane_state.missed_run_policy,
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Pending catch-up runs",
        &pane_state.pending_catchup_runs.to_string(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Last recovery (epoch)",
        &pane_state
            .last_recovery_epoch_seconds
            .map(|value| value.to_string())
            .unwrap_or_else(|| "n/a".to_string()),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Cron expression",
        &pane_state.cron_expression,
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Cron timezone",
        &pane_state.cron_timezone,
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Cron next preview (epoch)",
        &pane_state
            .cron_next_run_preview_epoch_seconds
            .map(|value| value.to_string())
            .unwrap_or_else(|| "n/a".to_string()),
    );
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Cron parse status",
        pane_state.cron_parse_error.as_deref().unwrap_or("ok"),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "OS scheduler opt-in",
        if pane_state.os_scheduler_enabled {
            "enabled"
        } else {
            "disabled"
        },
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "OS scheduler adapter",
        &pane_state.os_scheduler_adapter,
    );
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "OS scheduler descriptor",
        pane_state
            .os_scheduler_descriptor_path
            .as_deref()
            .unwrap_or("n/a"),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "OS scheduler last reconcile",
        &pane_state
            .os_scheduler_last_reconciled_epoch_seconds
            .map(|value| value.to_string())
            .unwrap_or_else(|| "n/a".to_string()),
    );
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "OS scheduler status",
        pane_state
            .os_scheduler_last_reconcile_result
            .as_deref()
            .unwrap_or("n/a"),
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
