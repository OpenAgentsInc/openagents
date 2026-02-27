use wgpui::{PaintContext, Point, theme};

use crate::app_state::AgentNetworkSimulationPaneState;
use crate::pane_renderer::{
    paint_action_button, paint_label_line, paint_multiline_phrase, paint_source_badge,
    paint_state_summary,
};
use crate::pane_system::{
    agent_network_simulation_reset_button_bounds, agent_network_simulation_run_button_bounds,
};

pub fn paint_agent_network_simulation_pane(
    content_bounds: wgpui::Bounds,
    pane_state: &AgentNetworkSimulationPaneState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "sim+nip28", paint);

    let run_bounds = agent_network_simulation_run_button_bounds(content_bounds);
    let reset_bounds = agent_network_simulation_reset_button_bounds(content_bounds);
    paint_action_button(run_bounds, "Run Simulation Round", paint);
    paint_action_button(reset_bounds, "Reset Simulation", paint);

    let mut y = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        run_bounds.max_y() + 12.0,
        pane_state.load_state,
        &format!("State: {}", pane_state.load_state.label()),
        pane_state.last_action.as_deref(),
        pane_state.last_error.as_deref(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "NIP-28 channel",
        pane_state
            .channel_event_id
            .as_deref()
            .unwrap_or("not-created"),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Channel name",
        &pane_state.channel_name,
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Rounds run",
        &pane_state.rounds_run.to_string(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Transferred sats",
        &pane_state.total_transferred_sats.to_string(),
    );

    let skills = if pane_state.learned_skills.is_empty() {
        "none".to_string()
    } else {
        pane_state.learned_skills.join(", ")
    };
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Learned skills",
        &skills,
    );

    if pane_state.events.is_empty() {
        paint.scene.draw_text(paint.text.layout(
            "No simulation events yet.",
            Point::new(content_bounds.origin.x + 12.0, y),
            11.0,
            theme::text::MUTED,
        ));
        return;
    }

    paint.scene.draw_text(paint.text.layout(
        "Latest protocol timeline",
        Point::new(content_bounds.origin.x + 12.0, y),
        11.0,
        theme::text::MUTED,
    ));
    let mut row_y = y + 16.0;
    for event in pane_state.events.iter().rev().take(8) {
        let color = match event.protocol.as_str() {
            "NIP-28" => theme::accent::PRIMARY,
            "NIP-SKL" => theme::status::SUCCESS,
            "NIP-AC" => theme::status::ERROR,
            "NIP-SA" => theme::text::PRIMARY,
            _ => theme::text::PRIMARY,
        };
        let summary = format!(
            "#{} [{}] {} {}",
            event.seq, event.protocol, event.event_ref, event.summary
        );
        paint.scene.draw_text(paint.text.layout_mono(
            &summary,
            Point::new(content_bounds.origin.x + 12.0, row_y),
            10.0,
            color,
        ));
        row_y += 14.0;
    }
}
