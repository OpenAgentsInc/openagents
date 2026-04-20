use std::borrow::Cow;
use std::time::{SystemTime, UNIX_EPOCH};

use psionic_train::{
    RemoteTrainingEventSeverity, TrainingExecutionPromotionOutcome, XtrainExplorerCheckpointState,
    XtrainExplorerEdgeKind, XtrainExplorerParticipantEdge, XtrainExplorerParticipantNode,
    XtrainExplorerParticipantState, XtrainExplorerSnapshot, XtrainExplorerWindowState,
};
use wgpui::viz::feed::{EventFeedRow, paint_event_feed_body};
use wgpui::viz::panel::{paint_shell as paint_panel_shell, paint_title as paint_panel_title};
use wgpui::viz::theme as viz_theme;
use wgpui::viz::topology::{TopologyNodeState, node_state_color};
use wgpui::{Bounds, Hsla, PaintContext, Point, Quad, theme};

use crate::app_state::{XtrainExplorerPaneState, XtrainExplorerViewMode};
use crate::pane_renderer::{
    paint_secondary_button, paint_selectable_row_background, paint_source_badge,
    paint_state_summary, paint_tertiary_button, split_text_for_display,
};
use crate::pane_system::{
    xtrain_explorer_layout, xtrain_explorer_participant_row_bounds,
    xtrain_explorer_refresh_button_bounds, xtrain_explorer_snapshot_row_bounds,
    xtrain_explorer_view_button_bounds,
};

const MAX_SNAPSHOT_ROWS: usize = 8;
const MAX_PARTICIPANT_ROWS: usize = 6;

pub fn paint(content_bounds: Bounds, pane: &XtrainExplorerPaneState, paint: &mut PaintContext) {
    let layout = xtrain_explorer_layout(content_bounds);
    let accent = viz_theme::track::EXPLORER;
    let phase = animation_phase();

    paint_source_badge(content_bounds, source_label(pane).as_str(), paint);
    paint_secondary_button(
        xtrain_explorer_refresh_button_bounds(content_bounds),
        "Refresh",
        paint,
    );
    for (index, view) in XtrainExplorerViewMode::ALL.iter().enumerate() {
        let bounds = xtrain_explorer_view_button_bounds(content_bounds, index);
        if pane.selected_view == *view {
            paint_secondary_button(bounds, view.label(), paint);
        } else {
            paint_tertiary_button(bounds, view.label(), paint);
        }
    }

    paint_summary_band(layout.summary_band, pane, accent, paint);

    paint_panel_shell(layout.snapshots_panel, viz_theme::track::XTRAIN, paint);
    paint_panel_title(
        layout.snapshots_panel,
        "EXPLORER INDEX",
        viz_theme::track::XTRAIN,
        paint,
    );
    paint_panel_shell(layout.graph_panel, accent, paint);
    paint_panel_title(layout.graph_panel, "PARTICIPANT GRAPH", accent, paint);
    paint_panel_shell(layout.window_panel, viz_theme::series::RUNTIME, paint);
    paint_panel_title(
        layout.window_panel,
        "ACTIVE WINDOW",
        viz_theme::series::RUNTIME,
        paint,
    );
    paint_panel_shell(layout.detail_panel, viz_theme::series::PROVENANCE, paint);
    paint_panel_title(
        layout.detail_panel,
        detail_panel_title(pane.selected_view),
        viz_theme::series::PROVENANCE,
        paint,
    );
    paint_panel_shell(layout.events_panel, viz_theme::series::EVENTS, paint);
    paint_panel_title(
        layout.events_panel,
        "EVENTS & EVIDENCE",
        viz_theme::series::EVENTS,
        paint,
    );

    paint_snapshots_panel(content_bounds, pane, paint);
    paint_graph_panel(layout.graph_panel, pane, phase, paint);
    paint_window_panel(layout.window_panel, pane, paint);
    paint_detail_panel(content_bounds, layout.detail_panel, pane, paint);
    paint_events_panel(layout.events_panel, pane, phase, paint);
}

fn paint_summary_band(
    bounds: Bounds,
    pane: &XtrainExplorerPaneState,
    accent: Hsla,
    paint: &mut PaintContext,
) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::APP.with_alpha(0.84))
            .with_border(accent.with_alpha(0.24), 1.0)
            .with_corner_radius(8.0),
    );
    let summary = summary_line(pane);
    let y = bounds.origin.y + 8.0;
    let end_y = paint_state_summary(
        paint,
        bounds.origin.x + 12.0,
        y,
        pane.load_state,
        summary.as_str(),
        pane.last_action.as_deref(),
        pane.last_error.as_deref(),
    );
    if end_y > bounds.max_y() - 2.0 {
        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.max_x() - 2.0,
                bounds.origin.y + 6.0,
                1.0,
                bounds.size.height - 12.0,
            ))
            .with_background(accent.with_alpha(0.2)),
        );
    }
}

fn paint_snapshots_panel(
    content_bounds: Bounds,
    pane: &XtrainExplorerPaneState,
    paint: &mut PaintContext,
) {
    let Some(index) = pane.index.as_ref() else {
        paint_empty_panel_state(
            panel_body_bounds(xtrain_explorer_layout(content_bounds).snapshots_panel),
            pane,
            "No XTRAIN explorer index loaded.",
            paint,
        );
        return;
    };

    for (row_index, entry) in index.entries.iter().take(MAX_SNAPSHOT_ROWS).enumerate() {
        let row_bounds = xtrain_explorer_snapshot_row_bounds(content_bounds, row_index);
        let selected = pane.selected_snapshot_id.as_deref() == Some(entry.snapshot_id.as_str());
        paint_selectable_row_background(paint, row_bounds, selected);
        let title_color = if selected {
            viz_theme::track::EXPLORER
        } else {
            theme::text::PRIMARY
        };
        paint.scene.draw_text(paint.text.layout_mono(
            compact_identifier(entry.snapshot_id.as_str(), 28).as_str(),
            Point::new(row_bounds.origin.x + 10.0, row_bounds.origin.y + 7.0),
            10.0,
            title_color,
        ));
        paint.scene.draw_text(paint.text.layout(
            format!(
                "{} participants | {} held | {} settlements",
                entry.participant_count,
                entry.held_checkpoint_count,
                entry.published_settlement_count
            )
            .as_str(),
            Point::new(row_bounds.origin.x + 10.0, row_bounds.origin.y + 23.0),
            10.0,
            theme::text::PRIMARY,
        ));
        let summary = split_text_for_display(entry.semantic_summary.as_str(), 44)
            .into_iter()
            .next()
            .unwrap_or_default();
        paint.scene.draw_text(paint.text.layout(
            summary.as_str(),
            Point::new(row_bounds.origin.x + 10.0, row_bounds.origin.y + 36.0),
            9.0,
            theme::text::MUTED,
        ));
    }
}

fn paint_graph_panel(
    panel: Bounds,
    pane: &XtrainExplorerPaneState,
    phase: f32,
    paint: &mut PaintContext,
) {
    let Some(snapshot) = pane.snapshot.as_ref() else {
        paint_empty_panel_state(panel_body_bounds(panel), pane, "No explorer snapshot loaded.", paint);
        return;
    };
    let body = panel_body_bounds(panel);
    paint.scene.draw_text(paint.text.layout_mono(
        format!(
            "{} | epoch {} | window {}",
            compact_identifier(snapshot.network_id.as_str(), 24),
            compact_identifier(snapshot.current_epoch_id.as_str(), 18),
            compact_identifier(snapshot.active_window_id.as_str(), 16)
        )
        .as_str(),
        Point::new(body.origin.x, body.origin.y),
        10.0,
        theme::text::MUTED,
    ));

    let graph_bounds = Bounds::new(
        body.origin.x,
        body.origin.y + 22.0,
        body.size.width,
        (body.size.height - 38.0).max(72.0),
    );
    paint.scene.draw_quad(
        Quad::new(graph_bounds)
            .with_background(theme::bg::APP.with_alpha(0.42))
            .with_border(viz_theme::track::EXPLORER.with_alpha(0.12), 1.0)
            .with_corner_radius(8.0),
    );

    let positions = participant_positions(snapshot.participants.len(), graph_bounds);
    for edge in &snapshot.participant_edges {
        paint_graph_edge(snapshot, positions.as_slice(), edge, paint);
    }
    for (index, participant) in snapshot.participants.iter().enumerate() {
        if let Some(center) = positions.get(index).copied() {
            paint_graph_node(center, participant, pane, phase, paint);
        }
    }
}

fn paint_window_panel(panel: Bounds, pane: &XtrainExplorerPaneState, paint: &mut PaintContext) {
    let Some(snapshot) = pane.snapshot.as_ref() else {
        paint_empty_panel_state(panel_body_bounds(panel), pane, "No active window loaded.", paint);
        return;
    };
    let body = panel_body_bounds(panel);
    let Some(window) = selected_window(snapshot) else {
        paint_empty_panel_state(body, pane, "Snapshot did not expose any windows.", paint);
        return;
    };
    let checkpoint = snapshot
        .checkpoints
        .iter()
        .find(|checkpoint| checkpoint.checkpoint_artifact_id == window.checkpoint_artifact_id);

    let lines = vec![
        (
            "Window".to_string(),
            compact_identifier(window.window_id.as_str(), 22),
            viz_theme::series::RUNTIME,
        ),
        (
            "Status".to_string(),
            compact_enum(format!("{:?}", window.status).as_str()),
            window_status_color(checkpoint),
        ),
        (
            "Dataset pages".to_string(),
            window.dataset_page_ids.len().to_string(),
            theme::text::PRIMARY,
        ),
        (
            "Miner sessions".to_string(),
            window.miner_session_ids.len().to_string(),
            theme::text::PRIMARY,
        ),
        (
            "Validator votes".to_string(),
            window.validator_vote_ids.len().to_string(),
            theme::text::PRIMARY,
        ),
        (
            "Settlement".to_string(),
            window
                .settlement_record_id
                .as_deref()
                .map(|value| compact_identifier(value, 18))
                .unwrap_or_else(|| "none".to_string()),
            provenance_color(window.settlement_record_id.is_some()),
        ),
    ];
    let mut y = body.origin.y;
    for (label, value, color) in lines {
        paint.scene.draw_text(paint.text.layout_mono(
            label.as_str(),
            Point::new(body.origin.x, y),
            10.0,
            theme::text::MUTED,
        ));
        paint.scene.draw_text(paint.text.layout(
            value.as_str(),
            Point::new(body.origin.x + 112.0, y),
            10.0,
            color,
        ));
        y += 18.0;
    }
    y += 6.0;
    for line in split_text_for_display(window.detail.as_str(), 44)
        .into_iter()
        .take(5)
    {
        paint.scene.draw_text(paint.text.layout(
            line.as_str(),
            Point::new(body.origin.x, y),
            10.0,
            theme::text::PRIMARY,
        ));
        y += 15.0;
    }
}

fn paint_detail_panel(
    content_bounds: Bounds,
    panel: Bounds,
    pane: &XtrainExplorerPaneState,
    paint: &mut PaintContext,
) {
    let Some(snapshot) = pane.snapshot.as_ref() else {
        paint_empty_panel_state(panel_body_bounds(panel), pane, "No explorer detail loaded.", paint);
        return;
    };
    let body = panel_body_bounds(panel);
    let participant_rows_top = participant_rows_top(body);
    let detail_bounds = Bounds::new(
        body.origin.x,
        body.origin.y,
        body.size.width,
        (participant_rows_top - body.origin.y - 14.0).max(48.0),
    );
    paint_detail_view(detail_bounds, pane.selected_view, snapshot, pane, paint);

    paint.scene.draw_text(paint.text.layout_mono(
        "participants",
        Point::new(body.origin.x, participant_rows_top - 18.0),
        10.0,
        theme::text::MUTED,
    ));
    for (row_index, participant) in snapshot
        .participants
        .iter()
        .take(MAX_PARTICIPANT_ROWS)
        .enumerate()
    {
        let row_bounds = xtrain_explorer_participant_row_bounds(content_bounds, row_index);
        let selected = pane.selected_participant_id.as_deref() == Some(participant.participant_id.as_str());
        paint_selectable_row_background(paint, row_bounds, selected);
        paint.scene.draw_text(paint.text.layout_mono(
            compact_participant_label(participant).as_str(),
            Point::new(row_bounds.origin.x + 8.0, row_bounds.origin.y + 8.0),
            9.0,
            if selected {
                viz_theme::track::EXPLORER
            } else {
                theme::text::PRIMARY
            },
        ));
        paint.scene.draw_text(paint.text.layout(
            format!(
                "{} | {}",
                compact_enum(format!("{:?}", participant.participant_state).as_str()),
                compact_enum(format!("{:?}", participant.availability_status).as_str())
            )
            .as_str(),
            Point::new(row_bounds.origin.x + 150.0, row_bounds.origin.y + 8.0),
            9.0,
            participant_state_color(participant.participant_state),
        ));
    }
}

fn paint_events_panel(
    panel: Bounds,
    pane: &XtrainExplorerPaneState,
    phase: f32,
    paint: &mut PaintContext,
) {
    let Some(snapshot) = pane.snapshot.as_ref() else {
        paint_empty_panel_state(panel_body_bounds(panel), pane, "No explorer events loaded.", paint);
        return;
    };
    let body = panel_body_bounds(panel);
    let authoritative_artifacts = snapshot
        .source_artifacts
        .iter()
        .filter(|artifact| artifact.authoritative)
        .count();
    let settlement_summary = selected_window(snapshot)
        .and_then(|window| window.settlement_record_id.as_deref())
        .map(|value| compact_identifier(value, 18))
        .unwrap_or_else(|| "none".to_string());
    let sibling_links = snapshot.run_surface_links.len();
    for (index, line) in [
        format!("contracts: {authoritative_artifacts} authoritative"),
        format!("settlement: {settlement_summary}"),
        format!("sibling lanes: {sibling_links}"),
    ]
    .into_iter()
    .enumerate()
    {
        paint.scene.draw_text(paint.text.layout_mono(
            line.as_str(),
            Point::new(body.origin.x, body.origin.y + index as f32 * 14.0),
            9.0,
            theme::text::MUTED,
        ));
    }

    let feed_bounds = Bounds::new(
        body.origin.x,
        body.origin.y + 46.0,
        body.size.width,
        (body.size.height - 46.0).max(40.0),
    );
    let events = snapshot
        .events
        .iter()
        .map(|event| EventFeedRow {
            label: Cow::Owned(compact_identifier(event.event_kind.as_str(), 20)),
            detail: Cow::Borrowed(event.detail.as_str()),
            color: event_severity_color(event.severity),
        })
        .collect::<Vec<_>>();
    paint_event_feed_body(
        feed_bounds,
        viz_theme::series::EVENTS,
        phase,
        "No explorer events retained for this snapshot.",
        events.as_slice(),
        paint,
    );
}

fn paint_detail_view(
    bounds: Bounds,
    selected_view: XtrainExplorerViewMode,
    snapshot: &XtrainExplorerSnapshot,
    pane: &XtrainExplorerPaneState,
    paint: &mut PaintContext,
) {
    let lines = match selected_view {
        XtrainExplorerViewMode::Overview => overview_lines(snapshot, pane),
        XtrainExplorerViewMode::Participants => participant_lines(snapshot, pane),
        XtrainExplorerViewMode::Windows => window_lines(snapshot),
        XtrainExplorerViewMode::Checkpoints => checkpoint_lines(snapshot),
        XtrainExplorerViewMode::Evidence => evidence_lines(snapshot),
    };
    let mut y = bounds.origin.y;
    for (line, color) in lines {
        if y > bounds.max_y() - 14.0 {
            break;
        }
        paint.scene.draw_text(paint.text.layout(
            line.as_str(),
            Point::new(bounds.origin.x, y),
            10.0,
            color,
        ));
        y += 15.0;
    }
}

fn overview_lines(
    snapshot: &XtrainExplorerSnapshot,
    pane: &XtrainExplorerPaneState,
) -> Vec<(String, Hsla)> {
    let selected_participant = selected_participant(snapshot, pane);
    let mut lines = vec![
        (snapshot.detail.clone(), theme::text::PRIMARY),
        (
            format!(
                "participants={} edges={} windows={} checkpoints={}",
                snapshot.participants.len(),
                snapshot.participant_edges.len(),
                snapshot.windows.len(),
                snapshot.checkpoints.len()
            ),
            theme::text::MUTED,
        ),
        (
            format!("linked bounded run lanes={}", snapshot.run_surface_links.len()),
            viz_theme::track::XTRAIN,
        ),
    ];
    if let Some(participant) = selected_participant {
        lines.push((
            format!(
                "selected {} -> roles={} execution_classes={}",
                compact_participant_label(participant),
                participant.role_classes.len(),
                participant.execution_classes.len()
            ),
            participant_state_color(participant.participant_state),
        ));
    }
    lines
}

fn participant_lines(
    snapshot: &XtrainExplorerSnapshot,
    pane: &XtrainExplorerPaneState,
) -> Vec<(String, Hsla)> {
    let Some(participant) = selected_participant(snapshot, pane) else {
        return vec![("No participant selected.".to_string(), theme::text::MUTED)];
    };
    vec![
        (
            format!("participant {}", compact_participant_label(participant)),
            participant_state_color(participant.participant_state),
        ),
        (
            format!("node {}", compact_identifier(participant.node_id.as_str(), 26)),
            theme::text::PRIMARY,
        ),
        (
            format!(
                "availability {} | payout {}",
                compact_enum(format!("{:?}", participant.availability_status).as_str()),
                participant
                    .payout_microunits
                    .map(|value| format!("{value} microunits"))
                    .unwrap_or_else(|| "none".to_string())
            ),
            theme::text::PRIMARY,
        ),
        (
            format!("roles {}", join_compact_debug(participant.role_classes.len(), "roles")),
            theme::text::MUTED,
        ),
        (
            participant.detail.clone(),
            theme::text::PRIMARY,
        ),
    ]
}

fn window_lines(snapshot: &XtrainExplorerSnapshot) -> Vec<(String, Hsla)> {
    let Some(window) = selected_window(snapshot) else {
        return vec![("Snapshot has no active window.".to_string(), theme::text::MUTED)];
    };
    vec![
        (
            format!("window {}", compact_identifier(window.window_id.as_str(), 20)),
            viz_theme::series::RUNTIME,
        ),
        (
            format!(
                "pages={} miner_sessions={} validator_votes={}",
                window.dataset_page_ids.len(),
                window.miner_session_ids.len(),
                window.validator_vote_ids.len()
            ),
            theme::text::PRIMARY,
        ),
        (
            format!(
                "checkpoint={} settlement={}",
                compact_identifier(window.checkpoint_artifact_id.as_str(), 22),
                window
                    .settlement_record_id
                    .as_deref()
                    .map(|value| compact_identifier(value, 20))
                    .unwrap_or_else(|| "none".to_string())
            ),
            theme::text::PRIMARY,
        ),
        (window.detail.clone(), theme::text::PRIMARY),
    ]
}

fn checkpoint_lines(snapshot: &XtrainExplorerSnapshot) -> Vec<(String, Hsla)> {
    let Some(checkpoint) = snapshot.checkpoints.first() else {
        return vec![("Snapshot has no retained checkpoints.".to_string(), theme::text::MUTED)];
    };
    vec![
        (
            format!(
                "checkpoint {}",
                compact_identifier(checkpoint.checkpoint_artifact_id.as_str(), 24)
            ),
            checkpoint_color(checkpoint.outcome),
        ),
        (
            format!(
                "outcome {} | validator_votes={} | disagreements={}",
                compact_enum(format!("{:?}", checkpoint.outcome).as_str()),
                checkpoint.validator_vote_ids.len(),
                checkpoint.disagreement_receipt_ids.len()
            ),
            checkpoint_color(checkpoint.outcome),
        ),
        (
            format!(
                "settlement {}",
                checkpoint
                    .settlement_record_id
                    .as_deref()
                    .map(|value| compact_identifier(value, 20))
                    .unwrap_or_else(|| "none".to_string())
            ),
            provenance_color(checkpoint.settlement_record_id.is_some()),
        ),
        (checkpoint.detail.clone(), theme::text::PRIMARY),
    ]
}

fn evidence_lines(snapshot: &XtrainExplorerSnapshot) -> Vec<(String, Hsla)> {
    let mut lines = Vec::new();
    for artifact in snapshot.source_artifacts.iter().take(4) {
        lines.push((
            format!(
                "{} -> {}",
                compact_identifier(artifact.artifact_role.as_str(), 26),
                compact_identifier(artifact.artifact_uri.as_str(), 28)
            ),
            if artifact.authoritative {
                viz_theme::series::PROVENANCE
            } else {
                theme::text::MUTED
            },
        ));
    }
    if let Some(link) = snapshot.run_surface_links.first() {
        lines.push((
            format!(
                "linked run surface {}",
                compact_identifier(link.bundle_artifact_uri.as_str(), 32)
            ),
            viz_theme::track::XTRAIN,
        ));
        lines.push((link.detail.clone(), theme::text::PRIMARY));
    }
    if lines.is_empty() {
        lines.push(("Snapshot has no retained evidence links.".to_string(), theme::text::MUTED));
    }
    lines
}

fn paint_empty_panel_state(
    bounds: Bounds,
    pane: &XtrainExplorerPaneState,
    empty_state: &str,
    paint: &mut PaintContext,
) {
    paint_state_summary(
        paint,
        bounds.origin.x,
        bounds.origin.y,
        pane.load_state,
        empty_state,
        pane.last_action.as_deref(),
        pane.last_error.as_deref(),
    );
}

fn panel_body_bounds(panel: Bounds) -> Bounds {
    Bounds::new(
        panel.origin.x + 12.0,
        panel.origin.y + 28.0,
        (panel.size.width - 24.0).max(0.0),
        (panel.size.height - 36.0).max(0.0),
    )
}

fn participant_rows_top(body: Bounds) -> f32 {
    let participant_list_height = (body.size.height * 0.38)
        .clamp(30.0 * 3.0, body.size.height - 64.0)
        .min(body.size.height);
    body.origin.y + (body.size.height - participant_list_height)
}

fn selected_window(snapshot: &XtrainExplorerSnapshot) -> Option<&XtrainExplorerWindowState> {
    snapshot
        .windows
        .iter()
        .find(|window| window.window_id == snapshot.active_window_id)
        .or_else(|| snapshot.windows.first())
}

fn selected_participant<'a>(
    snapshot: &'a XtrainExplorerSnapshot,
    pane: &'a XtrainExplorerPaneState,
) -> Option<&'a XtrainExplorerParticipantNode> {
    pane.selected_participant_id
        .as_deref()
        .and_then(|participant_id| {
            snapshot
                .participants
                .iter()
                .find(|participant| participant.participant_id == participant_id)
        })
        .or_else(|| snapshot.participants.first())
}

fn detail_panel_title(view: XtrainExplorerViewMode) -> &'static str {
    match view {
        XtrainExplorerViewMode::Overview => "DETAIL // OVERVIEW",
        XtrainExplorerViewMode::Participants => "DETAIL // PARTICIPANTS",
        XtrainExplorerViewMode::Windows => "DETAIL // WINDOWS",
        XtrainExplorerViewMode::Checkpoints => "DETAIL // CHECKPOINTS",
        XtrainExplorerViewMode::Evidence => "DETAIL // EVIDENCE",
    }
}

fn source_label(pane: &XtrainExplorerPaneState) -> String {
    pane.index_path
        .as_ref()
        .and_then(|path| path.file_name().and_then(|value| value.to_str()))
        .unwrap_or("xtrain_explorer")
        .to_string()
}

fn summary_line(pane: &XtrainExplorerPaneState) -> String {
    match pane.snapshot.as_ref() {
        Some(snapshot) => format!(
            "{} participants | {} events | active window {} | selected {}",
            snapshot.participants.len(),
            snapshot.events.len(),
            compact_identifier(snapshot.active_window_id.as_str(), 18),
            pane.selected_participant_id
                .as_deref()
                .map(|value| compact_identifier(value, 18))
                .unwrap_or_else(|| "none".to_string())
        ),
        None => "Waiting for XTRAIN explorer snapshot".to_string(),
    }
}

fn participant_positions(count: usize, bounds: Bounds) -> Vec<Point> {
    let center = Point::new(bounds.origin.x + bounds.size.width * 0.5, bounds.origin.y + bounds.size.height * 0.48);
    let dx = bounds.size.width * 0.29;
    let dy = bounds.size.height * 0.28;
    let pattern = [
        Point::new(center.x, center.y - dy),
        Point::new(center.x + dx, center.y),
        Point::new(center.x, center.y + dy),
        Point::new(center.x - dx, center.y),
        Point::new(center.x - dx * 0.72, center.y - dy * 0.78),
        Point::new(center.x + dx * 0.72, center.y - dy * 0.78),
    ];
    pattern.into_iter().take(count).collect()
}

fn paint_graph_edge(
    snapshot: &XtrainExplorerSnapshot,
    positions: &[Point],
    edge: &XtrainExplorerParticipantEdge,
    paint: &mut PaintContext,
) {
    let source_index = snapshot
        .participants
        .iter()
        .position(|participant| participant.participant_id == edge.source_participant_id);
    let target_index = snapshot
        .participants
        .iter()
        .position(|participant| participant.participant_id == edge.target_participant_id);
    let (Some(source_index), Some(target_index)) = (source_index, target_index) else {
        return;
    };
    let (Some(source), Some(target)) = (positions.get(source_index), positions.get(target_index)) else {
        return;
    };
    let color = edge_color(edge.edge_kind);
    paint_orthogonal_connector(*source, *target, color, paint);
}

fn paint_orthogonal_connector(source: Point, target: Point, color: Hsla, paint: &mut PaintContext) {
    let thickness = 2.0;
    let mid_x = (source.x + target.x) * 0.5;
    let horizontal_a = Bounds::new(
        source.x.min(mid_x),
        source.y - thickness * 0.5,
        (source.x - mid_x).abs().max(thickness),
        thickness,
    );
    let vertical = Bounds::new(
        mid_x - thickness * 0.5,
        source.y.min(target.y),
        thickness,
        (source.y - target.y).abs().max(thickness),
    );
    let horizontal_b = Bounds::new(
        mid_x.min(target.x),
        target.y - thickness * 0.5,
        (mid_x - target.x).abs().max(thickness),
        thickness,
    );
    for segment in [horizontal_a, vertical, horizontal_b] {
        paint
            .scene
            .draw_quad(Quad::new(segment).with_background(color.with_alpha(0.7)));
    }
}

fn paint_graph_node(
    center: Point,
    participant: &XtrainExplorerParticipantNode,
    pane: &XtrainExplorerPaneState,
    phase: f32,
    paint: &mut PaintContext,
) {
    let selected = pane.selected_participant_id.as_deref() == Some(participant.participant_id.as_str());
    let state = participant_node_state(participant.participant_state);
    let color = node_state_color(state);
    let pulse = if selected { 0.08 + phase * 0.08 } else { 0.0 };
    let node_bounds = Bounds::new(center.x - 12.0, center.y - 12.0, 24.0, 24.0);
    paint.scene.draw_quad(
        Quad::new(node_bounds)
            .with_background(color.with_alpha(0.22 + pulse))
            .with_border(color.with_alpha(0.92), if selected { 2.0 } else { 1.0 })
            .with_corner_radius(12.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(center.x - 4.0, center.y - 4.0, 8.0, 8.0))
            .with_background(color.with_alpha(0.95))
            .with_corner_radius(4.0),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        compact_participant_label(participant).as_str(),
        Point::new(center.x - 38.0, center.y + 17.0),
        9.0,
        if selected {
            viz_theme::track::EXPLORER
        } else {
            theme::text::PRIMARY
        },
    ));
    paint.scene.draw_text(paint.text.layout(
        compact_enum(format!("{:?}", participant.participant_state).as_str()).as_str(),
        Point::new(center.x - 24.0, center.y + 30.0),
        8.0,
        color,
    ));
}

fn participant_node_state(state: XtrainExplorerParticipantState) -> TopologyNodeState {
    match state {
        XtrainExplorerParticipantState::Active => TopologyNodeState::Active,
        XtrainExplorerParticipantState::Held => TopologyNodeState::Warning,
        XtrainExplorerParticipantState::Refused => TopologyNodeState::Error,
    }
}

fn participant_state_color(state: XtrainExplorerParticipantState) -> Hsla {
    node_state_color(participant_node_state(state))
}

fn edge_color(kind: XtrainExplorerEdgeKind) -> Hsla {
    match kind {
        XtrainExplorerEdgeKind::ValidatorScore => viz_theme::series::RUNTIME,
        XtrainExplorerEdgeKind::CheckpointSync => viz_theme::series::PROVENANCE,
        XtrainExplorerEdgeKind::Refusal => viz_theme::state::ERROR,
    }
}

fn event_severity_color(severity: RemoteTrainingEventSeverity) -> Hsla {
    match severity {
        RemoteTrainingEventSeverity::Info => viz_theme::series::EVENTS,
        RemoteTrainingEventSeverity::Warning => viz_theme::state::WARNING,
        RemoteTrainingEventSeverity::Error => viz_theme::state::ERROR,
    }
}

fn checkpoint_color(outcome: TrainingExecutionPromotionOutcome) -> Hsla {
    match outcome {
        TrainingExecutionPromotionOutcome::PromotedRevision => viz_theme::state::LIVE,
        TrainingExecutionPromotionOutcome::HeldNoPromotion => viz_theme::state::WARNING,
        TrainingExecutionPromotionOutcome::RefusedPromotion => viz_theme::state::ERROR,
    }
}

fn window_status_color(checkpoint: Option<&XtrainExplorerCheckpointState>) -> Hsla {
    checkpoint
        .map(|checkpoint| checkpoint_color(checkpoint.outcome))
        .unwrap_or(theme::text::MUTED)
}

fn provenance_color(authoritative: bool) -> Hsla {
    if authoritative {
        viz_theme::series::PROVENANCE
    } else {
        theme::text::MUTED
    }
}

fn compact_participant_label(participant: &XtrainExplorerParticipantNode) -> String {
    participant
        .participant_id
        .split('.')
        .next()
        .unwrap_or(participant.participant_id.as_str())
        .replace('_', " ")
}

fn compact_identifier(value: &str, limit: usize) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= limit {
        return trimmed.to_string();
    }
    format!(
        "{}…",
        trimmed.chars().take(limit.saturating_sub(1)).collect::<String>()
    )
}

fn compact_enum(value: &str) -> String {
    value
        .replace("HeldNoPromotion", "held-no-promotion")
        .replace("ValidatorScore", "validator-score")
        .replace("CheckpointSync", "checkpoint-sync")
        .replace("PromotionHeld", "promotion-held")
        .replace("Refused", "refused")
        .replace("Active", "active")
        .replace("Held", "held")
}

fn join_compact_debug(count: usize, label: &str) -> String {
    format!("{count} {label}")
}

fn animation_phase() -> f32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| ((duration.as_millis() % 4_000) as f32) / 4_000.0)
        .unwrap_or(0.0)
}
