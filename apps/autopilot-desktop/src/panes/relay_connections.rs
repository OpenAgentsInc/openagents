use wgpui::{Component, InputEvent, PaintContext, Point, Quad, theme};

use crate::app_state::{PaneKind, RelayConnectionsPaneInputs, RelayConnectionsState, RenderState};
use crate::pane_renderer::{paint_action_button, paint_source_badge};
use crate::pane_system::{
    RelayConnectionsPaneAction, pane_content_bounds, relay_connections_add_button_bounds,
    relay_connections_remove_button_bounds, relay_connections_retry_button_bounds,
    relay_connections_row_bounds, relay_connections_url_input_bounds,
    relay_connections_visible_row_count,
};

pub fn paint(
    content_bounds: wgpui::Bounds,
    relay_connections: &RelayConnectionsState,
    relay_connections_inputs: &mut RelayConnectionsPaneInputs,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "runtime", paint);

    let input_bounds = relay_connections_url_input_bounds(content_bounds);
    let add_bounds = relay_connections_add_button_bounds(content_bounds);
    let remove_bounds = relay_connections_remove_button_bounds(content_bounds);
    let retry_bounds = relay_connections_retry_button_bounds(content_bounds);

    relay_connections_inputs
        .relay_url
        .set_max_width(input_bounds.size.width);
    relay_connections_inputs
        .relay_url
        .paint(input_bounds, paint);
    paint_action_button(add_bounds, "Add relay", paint);
    paint_action_button(remove_bounds, "Remove selected", paint);
    paint_action_button(retry_bounds, "Retry selected", paint);

    paint.scene.draw_text(paint.text.layout(
        "Relay URL",
        Point::new(input_bounds.origin.x, input_bounds.origin.y - 12.0),
        10.0,
        theme::text::MUTED,
    ));

    let state_color = match relay_connections.load_state {
        crate::app_state::PaneLoadState::Ready => theme::status::SUCCESS,
        crate::app_state::PaneLoadState::Loading => theme::accent::PRIMARY,
        crate::app_state::PaneLoadState::Error => theme::status::ERROR,
    };
    let mut y = input_bounds.max_y() + 12.0;
    paint.scene.draw_text(paint.text.layout(
        &format!("State: {}", relay_connections.load_state.label()),
        Point::new(content_bounds.origin.x + 12.0, y),
        11.0,
        state_color,
    ));
    y += 16.0;

    if let Some(action) = relay_connections.last_action.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            action,
            Point::new(content_bounds.origin.x + 12.0, y),
            10.0,
            theme::text::MUTED,
        ));
        y += 16.0;
    }
    if let Some(error) = relay_connections.last_error.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            error,
            Point::new(content_bounds.origin.x + 12.0, y),
            10.0,
            theme::status::ERROR,
        ));
        y += 16.0;
    }

    let visible_rows = relay_connections_visible_row_count(relay_connections.relays.len());
    if visible_rows == 0 {
        paint.scene.draw_text(paint.text.layout(
            "No relays configured.",
            Point::new(content_bounds.origin.x + 12.0, y),
            11.0,
            theme::text::MUTED,
        ));
        return;
    }

    for row_index in 0..visible_rows {
        let relay = &relay_connections.relays[row_index];
        let row_bounds = relay_connections_row_bounds(content_bounds, row_index);
        let selected = relay_connections.selected_url.as_deref() == Some(relay.url.as_str());
        paint.scene.draw_quad(
            Quad::new(row_bounds)
                .with_background(if selected {
                    theme::accent::PRIMARY.with_alpha(0.18)
                } else {
                    theme::bg::APP.with_alpha(0.78)
                })
                .with_border(
                    if selected {
                        theme::accent::PRIMARY
                    } else {
                        theme::border::DEFAULT
                    },
                    1.0,
                )
                .with_corner_radius(4.0),
        );

        let status_color = match relay.status {
            crate::app_state::RelayConnectionStatus::Connected => theme::status::SUCCESS,
            crate::app_state::RelayConnectionStatus::Connecting => theme::accent::PRIMARY,
            crate::app_state::RelayConnectionStatus::Disconnected => theme::text::MUTED,
            crate::app_state::RelayConnectionStatus::Error => theme::status::ERROR,
        };
        let latency = relay
            .latency_ms
            .map_or_else(|| "-".to_string(), |value| value.to_string());
        let last_seen = relay
            .last_seen_seconds_ago
            .map_or_else(|| "-".to_string(), |value| value.to_string());
        let last_error = relay.last_error.as_deref().unwrap_or("-");
        let summary = format!(
            "{} {} latency:{}ms seen:{}s err:{}",
            relay.url,
            relay.status.label(),
            latency,
            last_seen,
            last_error
        );
        paint.scene.draw_text(paint.text.layout_mono(
            &summary,
            Point::new(row_bounds.origin.x + 8.0, row_bounds.origin.y + 9.0),
            10.0,
            if selected {
                theme::text::PRIMARY
            } else {
                status_color
            },
        ));
    }

    if let Some(selected) = relay_connections.selected() {
        let selected_y =
            relay_connections_row_bounds(content_bounds, visible_rows.saturating_sub(1)).max_y()
                + 12.0;
        let details = format!(
            "Selected: {} [{}] last_error:{}",
            selected.url,
            selected.status.label(),
            selected.last_error.as_deref().unwrap_or("-")
        );
        paint.scene.draw_text(paint.text.layout_mono(
            &details,
            Point::new(content_bounds.origin.x + 12.0, selected_y),
            10.0,
            theme::text::MUTED,
        ));
    }
}

pub fn topmost_action_hit(
    state: &RenderState,
    point: Point,
) -> Option<(u64, RelayConnectionsPaneAction)> {
    for pane_idx in pane_indices_by_z_desc(state) {
        let pane = &state.panes[pane_idx];
        if pane.kind != PaneKind::RelayConnections {
            continue;
        }

        let content_bounds = pane_content_bounds(pane.bounds);
        if relay_connections_add_button_bounds(content_bounds).contains(point) {
            return Some((pane.id, RelayConnectionsPaneAction::AddRelay));
        }
        if relay_connections_remove_button_bounds(content_bounds).contains(point) {
            return Some((pane.id, RelayConnectionsPaneAction::RemoveSelected));
        }
        if relay_connections_retry_button_bounds(content_bounds).contains(point) {
            return Some((pane.id, RelayConnectionsPaneAction::RetrySelected));
        }

        let visible_rows = relay_connections_visible_row_count(state.relay_connections.relays.len());
        for row_index in 0..visible_rows {
            if relay_connections_row_bounds(content_bounds, row_index).contains(point) {
                return Some((pane.id, RelayConnectionsPaneAction::SelectRow(row_index)));
            }
        }
    }

    None
}

pub fn dispatch_input_event(state: &mut RenderState, event: &InputEvent) -> bool {
    let top_relay = state
        .panes
        .iter()
        .filter(|pane| pane.kind == PaneKind::RelayConnections)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| pane.bounds);
    let Some(bounds) = top_relay else {
        return false;
    };

    let input_bounds = relay_connections_url_input_bounds(pane_content_bounds(bounds));
    state
        .relay_connections_inputs
        .relay_url
        .event(event, input_bounds, &mut state.event_context)
        .is_handled()
}

fn pane_indices_by_z_desc(state: &RenderState) -> Vec<usize> {
    let mut ordered: Vec<usize> = (0..state.panes.len()).collect();
    ordered.sort_by(|lhs, rhs| state.panes[*rhs].z_index.cmp(&state.panes[*lhs].z_index));
    ordered
}
