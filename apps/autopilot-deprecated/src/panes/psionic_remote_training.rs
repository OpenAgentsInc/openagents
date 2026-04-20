use std::borrow::Cow;
use std::time::{SystemTime, UNIX_EPOCH};

use psionic_train::{
    RemoteTrainingArtifactSourceKind, RemoteTrainingDistributedSample, RemoteTrainingEventSeverity,
    RemoteTrainingGpuSample, RemoteTrainingSourceArtifact, RemoteTrainingVisualizationBundleV2,
};
use wgpui::components::hud::{DotShape, DotsGrid, Heatmap, RingGauge, Scanlines, SignalMeter};
use wgpui::viz::badge::{BadgeTone, tone_color as badge_tone_color};
use wgpui::viz::chart::{HistoryChartSeries, paint_history_chart_body};
use wgpui::viz::feed::{EventFeedRow, paint_event_feed_body};
use wgpui::viz::panel::{
    body_bounds as viz_panel_body_bounds, paint_shell as paint_panel_shell,
    paint_texture as paint_panel_texture, paint_title as paint_panel_title,
};
use wgpui::viz::theme as viz_theme;
use wgpui::{Bounds, Component, Hsla, PaintContext, Point, Quad, theme};

use crate::desktop_control::{
    DesktopControlRemoteTrainingRunStatus, DesktopControlRemoteTrainingSelectedRunStatus,
    DesktopControlRemoteTrainingStatus,
};
use crate::pane_renderer::{
    paint_secondary_button, paint_selectable_row_background, paint_source_badge,
    split_text_for_display,
};
use crate::pane_system::{
    psionic_remote_training_clear_anchor_button_bounds,
    psionic_remote_training_clear_compare_button_bounds,
    psionic_remote_training_compare_button_bounds, psionic_remote_training_layout,
    psionic_remote_training_refresh_button_bounds, psionic_remote_training_run_row_bounds,
    psionic_remote_training_topology_target_bounds,
};
const CARD_GAP: f32 = 8.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CompareNormalizationMode {
    PendingBaseline,
    DirectScore,
    PublicEquivalentScore,
    SummaryOnly,
    SideBySide,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CompareAnchorMode {
    None,
    GlobalStep,
    ElapsedMs,
}

struct TrainingRunCompare<'a> {
    baseline: &'a DesktopControlRemoteTrainingSelectedRunStatus,
    mode: CompareNormalizationMode,
    anchor_mode: CompareAnchorMode,
    delta_value: Option<f64>,
    delta_summary: Option<String>,
    caveat: String,
}

struct OwnedChartSeries {
    label: &'static str,
    values: Vec<f32>,
    color: Hsla,
    fill_alpha: f32,
    line_alpha: f32,
}

struct ChartPanelView {
    header: Option<String>,
    footer: Option<String>,
    empty_state: String,
    series: Vec<OwnedChartSeries>,
}

#[derive(Clone)]
pub(crate) struct RemoteTrainingTopologyFocusTarget {
    pub key: String,
    pub label: String,
    pub detail: String,
}

pub fn paint(
    content_bounds: Bounds,
    remote_training: &DesktopControlRemoteTrainingStatus,
    paint: &mut PaintContext,
) {
    let layout = psionic_remote_training_layout(content_bounds);
    let selected = remote_training.selected_run.as_ref();
    let compare = build_compare_state(remote_training, selected);
    let accent = selected
        .map(|selected| run_track_accent(&selected.run))
        .unwrap_or_else(remote_blue);
    let phase = animation_phase();

    paint_source_badge(content_bounds, remote_training.source.as_str(), paint);
    paint_secondary_button(
        psionic_remote_training_refresh_button_bounds(content_bounds),
        "Refresh",
        paint,
    );
    if selected.is_some() {
        let compare_label = match (
            remote_training.compare_baseline_run_id.as_deref(),
            selected.map(|selected| selected.run.run_id.as_str()),
        ) {
            (Some(baseline), Some(current)) if baseline == current => "Baseline Pinned",
            (Some(_), Some(_)) => "Re-Pin Baseline",
            _ => "Pin Baseline",
        };
        paint_secondary_button(
            psionic_remote_training_compare_button_bounds(content_bounds),
            compare_label,
            paint,
        );
    }
    if remote_training.compare_baseline_run_id.is_some() {
        paint_secondary_button(
            psionic_remote_training_clear_compare_button_bounds(content_bounds),
            "Clear Compare",
            paint,
        );
    }
    if remote_training.chart_anchor_ratio_milli.is_some() {
        paint_secondary_button(
            psionic_remote_training_clear_anchor_button_bounds(content_bounds),
            "Live Anchor",
            paint,
        );
    }

    let cards = status_cards(remote_training, selected, compare.as_ref());
    let card_count = cards.len().max(1) as f32;
    let card_width =
        ((layout.status_row.size.width - CARD_GAP * (card_count - 1.0)) / card_count).max(120.0);
    for (index, (label, value, color)) in cards.iter().enumerate() {
        let x = layout.status_row.origin.x + index as f32 * (card_width + CARD_GAP);
        let width = if index == cards.len().saturating_sub(1) {
            (layout.status_row.max_x() - x).max(0.0)
        } else {
            card_width
        };
        paint_status_card(
            Bounds::new(
                x,
                layout.status_row.origin.y,
                width,
                layout.status_row.size.height,
            ),
            label,
            value.as_str(),
            *color,
            paint,
        );
    }

    paint_summary_band(
        layout.summary_band,
        selected_summary_line(remote_training, selected, compare.as_ref()).as_str(),
        accent,
        paint,
    );

    paint_panel_shell(layout.runs_panel, remote_gold(), paint);
    paint_panel_title(layout.runs_panel, "RUN INDEX", remote_gold(), paint);
    paint_panel_shell(layout.hero_panel, accent, paint);
    paint_panel_title(layout.hero_panel, "RUN DETAIL", accent, paint);
    paint_panel_shell(layout.loss_panel, remote_coral(), paint);
    paint_panel_title(layout.loss_panel, "LOSS CURVES", remote_coral(), paint);
    paint_panel_shell(layout.math_panel, remote_mint(), paint);
    paint_panel_title(layout.math_panel, "OPTIMIZER MATH", remote_mint(), paint);
    paint_panel_shell(layout.runtime_panel, remote_blue(), paint);
    paint_panel_title(
        layout.runtime_panel,
        "PIPELINE TIMING",
        remote_blue(),
        paint,
    );
    paint_panel_shell(layout.hardware_panel, remote_gold(), paint);
    paint_panel_title(layout.hardware_panel, "GPU & FABRIC", remote_gold(), paint);
    paint_panel_shell(layout.events_panel, remote_coral(), paint);
    paint_panel_title(layout.events_panel, "EVENT FEED", remote_coral(), paint);
    paint_panel_shell(layout.provenance_panel, remote_mint(), paint);
    paint_panel_title(
        layout.provenance_panel,
        "HEARTBEAT & PROVENANCE",
        remote_mint(),
        paint,
    );
    paint_panel_texture(layout.runs_panel, remote_gold(), phase, paint);
    paint_panel_texture(layout.hero_panel, accent, phase, paint);
    paint_panel_texture(layout.provenance_panel, remote_mint(), phase, paint);

    paint_runs_panel(content_bounds, remote_training, paint);
    paint_run_detail_panel(
        layout.hero_panel,
        remote_training,
        selected,
        compare.as_ref(),
        accent,
        phase,
        paint,
    );
    paint_loss_panel(
        layout.loss_panel,
        remote_training,
        selected,
        compare.as_ref(),
        phase,
        paint,
    );
    paint_math_panel(
        layout.math_panel,
        remote_training,
        selected,
        compare.as_ref(),
        phase,
        paint,
    );
    paint_runtime_panel(
        layout.runtime_panel,
        remote_training,
        selected,
        compare.as_ref(),
        phase,
        paint,
    );
    paint_hardware_panel(
        content_bounds,
        layout.hardware_panel,
        remote_training,
        selected,
        compare.as_ref(),
        phase,
        paint,
    );
    paint_event_panel(
        layout.events_panel,
        remote_training,
        selected,
        compare.as_ref(),
        phase,
        paint,
    );
    paint_provenance_panel(
        layout.provenance_panel,
        remote_training,
        selected,
        phase,
        paint,
    );
}

fn paint_runs_panel(
    content_bounds: Bounds,
    remote_training: &DesktopControlRemoteTrainingStatus,
    paint: &mut PaintContext,
) {
    for (index, run) in remote_training.runs.iter().take(8).enumerate() {
        let bounds = psionic_remote_training_run_row_bounds(content_bounds, index);
        let selected = remote_training.selected_run_id.as_deref() == Some(run.run_id.as_str());
        paint_selectable_row_background(paint, bounds, selected);

        let accent = if run.contract_error.is_some() {
            theme::status::ERROR
        } else if run.stale {
            theme::status::ERROR
        } else if selected {
            run_track_accent(run)
        } else {
            theme::text::PRIMARY
        };
        let headline = truncate_line(
            paint,
            format!(
                "{} // {}",
                compact_label(run.track.track_family.as_str()),
                run.profile_id
            )
            .as_str(),
            bounds.size.width - 92.0,
            10.0,
        );
        paint.scene.draw_text(paint.text.layout_mono(
            headline.as_str(),
            Point::new(bounds.origin.x + 10.0, bounds.origin.y + 8.0),
            10.0,
            accent,
        ));
        let mid_line = run
            .primary_score
            .as_ref()
            .map(primary_score_brief)
            .unwrap_or_else(|| compact_label(run.result_classification.as_str()));
        let mid_line = truncate_line(paint, mid_line.as_str(), bounds.size.width - 92.0, 10.0);
        paint.scene.draw_text(paint.text.layout(
            mid_line.as_str(),
            Point::new(bounds.origin.x + 10.0, bounds.origin.y + 22.0),
            10.0,
            theme::text::PRIMARY,
        ));

        let status_line = truncate_line(
            paint,
            format!(
                "{} // {} // {}",
                compact_label(run.track.proof_posture.as_str()),
                compact_label(run.track.comparability_class.as_str()),
                compact_label(run.series_status.as_str())
            )
            .as_str(),
            bounds.size.width - 92.0,
            9.0,
        );
        paint.scene.draw_text(paint.text.layout_mono(
            status_line.as_str(),
            Point::new(bounds.origin.x + 10.0, bounds.origin.y + 36.0),
            9.0,
            theme::text::MUTED,
        ));

        paint_run_row_status_matrix(run, bounds, selected, paint);
    }
}

fn paint_run_detail_panel(
    bounds: Bounds,
    remote_training: &DesktopControlRemoteTrainingStatus,
    selected: Option<&DesktopControlRemoteTrainingSelectedRunStatus>,
    compare: Option<&TrainingRunCompare<'_>>,
    accent: Hsla,
    phase: f32,
    paint: &mut PaintContext,
) {
    let body = viz_panel_body_bounds(bounds);
    let Some(selected) = selected else {
        paint.scene.draw_text(paint.text.layout(
            "Select a run to inspect its live training field.",
            Point::new(body.origin.x + 8.0, body.origin.y + 18.0),
            12.0,
            theme::text::MUTED,
        ));
        return;
    };
    let bundle = selected.bundle.as_ref();
    let phase_line = latest_phase_label(bundle, &selected.run);
    let track = &selected.run.track;
    let checkpoint = bundle
        .map(|bundle| {
            bundle
                .summary
                .latest_checkpoint_ref
                .clone()
                .unwrap_or_else(|| "checkpoint unavailable".to_string())
        })
        .unwrap_or_else(|| "checkpoint unavailable".to_string());
    let compare_summary = compare
        .and_then(|compare| compare.delta_summary.as_deref())
        .unwrap_or("compare idle");
    let proof_label = score_surface_compact_label(&selected.run);
    let freshness = freshness_label(&selected.run);
    let summary_title = selected
        .run
        .primary_score
        .as_ref()
        .map(primary_score_brief)
        .unwrap_or_else(|| compact_label(track.track_id.as_str()));
    let detail_text = bundle
        .and_then(|bundle| {
            bundle
                .score_surface
                .as_ref()
                .map(|surface| surface.semantic_summary.as_str())
        })
        .or_else(|| bundle.map(|bundle| bundle.summary.detail.as_str()))
        .or_else(|| {
            selected
                .run
                .score_surface
                .as_ref()
                .map(|surface| surface.semantic_summary.as_str())
        })
        .unwrap_or(selected.run.semantic_summary.as_str());
    let wrap = split_text_for_display(detail_text, 34);
    let badge_y = body.origin.y + 2.0;
    paint_badge_strip(
        body.origin.x + 4.0,
        badge_y,
        &[
            (
                compact_label(track.track_family.as_str()).to_uppercase(),
                track_badge_tone(track.track_family.as_str()),
            ),
            (
                compact_label(track.comparability_class.as_str()).to_uppercase(),
                BadgeTone::Neutral,
            ),
            (
                compact_label(track.proof_posture.as_str()).to_uppercase(),
                BadgeTone::Live,
            ),
            (
                compact_label(selected.run.series_status.as_str()).to_uppercase(),
                state_badge_tone(&selected.run),
            ),
            (
                compact_label(selected.run.contract_state.as_str()).to_uppercase(),
                contract_badge_tone(&selected.run),
            ),
        ],
        accent,
        paint,
    );

    let gauge_y = badge_y + 20.0;
    let gauge_size = 54.0;
    paint_metric_ring(
        Bounds::new(body.origin.x + 4.0, gauge_y, gauge_size, gauge_size),
        "PROOF",
        proof_label.as_str(),
        proof_level(track.proof_posture.as_str()),
        accent,
        paint,
    );
    paint_metric_ring(
        Bounds::new(body.origin.x + 70.0, gauge_y, gauge_size, gauge_size),
        "FRESH",
        freshness.as_str(),
        freshness_level(&selected.run),
        remote_blue(),
        paint,
    );
    paint_metric_ring(
        Bounds::new(body.origin.x + 136.0, gauge_y, gauge_size, gauge_size),
        "SERIES",
        compact_label(selected.run.series_status.as_str()).as_str(),
        series_coverage_level(bundle),
        remote_gold(),
        paint,
    );

    let matrix_bounds = Bounds::new(body.origin.x + 204.0, gauge_y + 2.0, 172.0, 60.0);
    paint_run_telemetry_matrix(matrix_bounds, selected, accent, phase, paint);

    let x = body.max_x() - 284.0;
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            x - 10.0,
            body.origin.y + 2.0,
            286.0,
            body.size.height - 4.0,
        ))
        .with_background(accent.with_alpha(0.08))
        .with_border(accent.with_alpha(0.22), 1.0)
        .with_corner_radius(8.0),
    );
    let summary_title = truncate_line(paint, summary_title.as_str(), 254.0, 10.0);
    let compare_summary = truncate_line(paint, compare_summary, 254.0, 9.0);
    paint.scene.draw_text(paint.text.layout_mono(
        summary_title.as_str(),
        Point::new(x, body.origin.y + 10.0),
        10.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        compare_summary.as_str(),
        Point::new(x, body.origin.y + 24.0),
        9.0,
        remote_gold(),
    ));
    let mut wrap_y = body.origin.y + 38.0;
    for line in wrap.iter().take(2) {
        paint.scene.draw_text(paint.text.layout(
            line.as_str(),
            Point::new(x, wrap_y),
            10.0,
            theme::text::PRIMARY,
        ));
        wrap_y += 14.0;
    }

    let footer_left = truncate_line(
        paint,
        format!(
            "{} // {} // {}",
            selected.run.lane_id,
            phase_line,
            chart_anchor_footer(remote_training, Some(selected), compare)
                .unwrap_or_else(|| "anchor live".to_string())
        )
        .as_str(),
        body.size.width - 24.0,
        9.0,
    );
    let footer_right = truncate_line(
        paint,
        format!(
            "{} // {}",
            track.score_law_ref.as_deref().unwrap_or("score law -"),
            truncate_to_chars(checkpoint.as_str(), 34)
        )
        .as_str(),
        body.size.width - 24.0,
        9.0,
    );
    paint.scene.draw_text(paint.text.layout_mono(
        footer_left.as_str(),
        Point::new(body.origin.x + 4.0, body.max_y() - 18.0),
        9.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        footer_right.as_str(),
        Point::new(body.origin.x + 4.0, body.max_y() - 6.0),
        9.0,
        theme::text::MUTED,
    ));
}

fn paint_loss_panel(
    bounds: Bounds,
    remote_training: &DesktopControlRemoteTrainingStatus,
    selected: Option<&DesktopControlRemoteTrainingSelectedRunStatus>,
    compare: Option<&TrainingRunCompare<'_>>,
    phase: f32,
    paint: &mut PaintContext,
) {
    let accent = remote_coral();
    let Some(selected) = selected else {
        paint_history_chart_body(
            bounds,
            accent,
            phase,
            None,
            None,
            "Select a run to inspect its retained loss telemetry.",
            &[],
            paint,
        );
        return;
    };
    let current = loss_chart_view(selected, "current");
    if let Some(compare) = compare {
        if compare_can_overlay(compare) {
            let mut overlay = loss_chart_view(selected, "current");
            let baseline = loss_chart_view(compare.baseline, "baseline");
            overlay.header = Some(comparison_header(
                Some(compare),
                current.header.as_deref().unwrap_or("current"),
                baseline.header.as_deref().unwrap_or("baseline"),
            ));
            overlay.footer = Some(
                chart_anchor_footer(remote_training, Some(selected), Some(compare))
                    .unwrap_or_else(|| current_vs_baseline_footer(compare)),
            );
            overlay
                .series
                .extend(baseline.series.into_iter().map(|series| OwnedChartSeries {
                    label: series.label,
                    values: series.values,
                    color: series.color.with_alpha(0.55),
                    fill_alpha: 0.0,
                    line_alpha: 0.58,
                }));
            paint_chart_panel_view(bounds, accent, phase, &overlay, paint);
            return;
        }
        if compare.mode != CompareNormalizationMode::PendingBaseline {
            let mut current = current;
            current.footer = Some(
                chart_anchor_footer(remote_training, Some(selected), Some(compare))
                    .unwrap_or_else(|| compare.caveat.clone()),
            );
            let mut baseline = loss_chart_view(compare.baseline, "baseline");
            baseline.footer = Some(current_vs_baseline_footer(compare));
            paint_side_by_side_chart_views(bounds, accent, phase, &current, &baseline, paint);
            return;
        }
    }
    let mut current = current;
    current.footer =
        chart_anchor_footer(remote_training, Some(selected), compare).or(current.footer.clone());
    paint_chart_panel_view(bounds, accent, phase, &current, paint);
}

fn paint_math_panel(
    bounds: Bounds,
    remote_training: &DesktopControlRemoteTrainingStatus,
    selected: Option<&DesktopControlRemoteTrainingSelectedRunStatus>,
    compare: Option<&TrainingRunCompare<'_>>,
    phase: f32,
    paint: &mut PaintContext,
) {
    let accent = remote_mint();
    let Some(selected) = selected else {
        paint_history_chart_body(
            bounds,
            accent,
            phase,
            None,
            None,
            "Select a run to inspect optimizer math telemetry.",
            &[],
            paint,
        );
        return;
    };
    let current = math_chart_view(selected, "current");
    if let Some(compare) = compare {
        if compare_can_overlay(compare) {
            let mut overlay = math_chart_view(selected, "current");
            let baseline = math_chart_view(compare.baseline, "baseline");
            overlay.header = Some(comparison_header(
                Some(compare),
                current.header.as_deref().unwrap_or("current"),
                baseline.header.as_deref().unwrap_or("baseline"),
            ));
            overlay.footer = Some(
                chart_anchor_footer(remote_training, Some(selected), Some(compare))
                    .unwrap_or_else(|| current_vs_baseline_footer(compare)),
            );
            overlay
                .series
                .extend(baseline.series.into_iter().map(|series| OwnedChartSeries {
                    label: series.label,
                    values: series.values,
                    color: series.color.with_alpha(0.55),
                    fill_alpha: 0.0,
                    line_alpha: 0.58,
                }));
            paint_chart_panel_view(bounds, accent, phase, &overlay, paint);
            return;
        }
        if compare.mode != CompareNormalizationMode::PendingBaseline {
            let mut current = current;
            current.footer = Some(
                chart_anchor_footer(remote_training, Some(selected), Some(compare))
                    .unwrap_or_else(|| compare.caveat.clone()),
            );
            let mut baseline = math_chart_view(compare.baseline, "baseline");
            baseline.footer = Some(current_vs_baseline_footer(compare));
            paint_side_by_side_chart_views(bounds, accent, phase, &current, &baseline, paint);
            return;
        }
    }
    let mut current = current;
    current.footer =
        chart_anchor_footer(remote_training, Some(selected), compare).or(current.footer.clone());
    paint_chart_panel_view(bounds, accent, phase, &current, paint);
}

fn paint_runtime_panel(
    bounds: Bounds,
    remote_training: &DesktopControlRemoteTrainingStatus,
    selected: Option<&DesktopControlRemoteTrainingSelectedRunStatus>,
    compare: Option<&TrainingRunCompare<'_>>,
    phase: f32,
    paint: &mut PaintContext,
) {
    let accent = remote_blue();
    let Some(selected) = selected else {
        paint_history_chart_body(
            bounds,
            accent,
            phase,
            None,
            None,
            "Select a run to inspect pipeline timing and throughput.",
            &[],
            paint,
        );
        return;
    };
    let current = runtime_chart_view(selected, "current");
    if let Some(compare) = compare {
        if compare_can_overlay(compare) {
            let mut overlay = runtime_chart_view(selected, "current");
            let baseline = runtime_chart_view(compare.baseline, "baseline");
            overlay.header = Some(comparison_header(
                Some(compare),
                current.header.as_deref().unwrap_or("current"),
                baseline.header.as_deref().unwrap_or("baseline"),
            ));
            overlay.footer = Some(
                chart_anchor_footer(remote_training, Some(selected), Some(compare))
                    .unwrap_or_else(|| current_vs_baseline_footer(compare)),
            );
            overlay
                .series
                .extend(baseline.series.into_iter().map(|series| OwnedChartSeries {
                    label: series.label,
                    values: series.values,
                    color: series.color.with_alpha(0.55),
                    fill_alpha: 0.0,
                    line_alpha: 0.58,
                }));
            paint_chart_panel_view(bounds, accent, phase, &overlay, paint);
            return;
        }
        if compare.mode != CompareNormalizationMode::PendingBaseline {
            let mut current = current;
            current.footer = Some(
                chart_anchor_footer(remote_training, Some(selected), Some(compare))
                    .unwrap_or_else(|| compare.caveat.clone()),
            );
            let mut baseline = runtime_chart_view(compare.baseline, "baseline");
            baseline.footer = Some(current_vs_baseline_footer(compare));
            paint_side_by_side_chart_views(bounds, accent, phase, &current, &baseline, paint);
            return;
        }
    }
    let mut current = current;
    current.footer =
        chart_anchor_footer(remote_training, Some(selected), compare).or(current.footer.clone());
    paint_chart_panel_view(bounds, accent, phase, &current, paint);
}

fn paint_hardware_panel(
    content_bounds: Bounds,
    bounds: Bounds,
    remote_training: &DesktopControlRemoteTrainingStatus,
    selected: Option<&DesktopControlRemoteTrainingSelectedRunStatus>,
    compare: Option<&TrainingRunCompare<'_>>,
    phase: f32,
    paint: &mut PaintContext,
) {
    let accent = remote_gold();
    let Some(selected) = selected else {
        paint_history_chart_body(
            bounds,
            accent,
            phase,
            None,
            None,
            "Select a run to inspect device and distributed telemetry.",
            &[],
            paint,
        );
        return;
    };
    let current = hardware_chart_view(selected, "current");
    if let Some(compare) = compare {
        if compare_can_overlay(compare) {
            let mut overlay = hardware_chart_view(selected, "current");
            let baseline = hardware_chart_view(compare.baseline, "baseline");
            overlay.header = Some(comparison_header(
                Some(compare),
                current.header.as_deref().unwrap_or("current"),
                baseline.header.as_deref().unwrap_or("baseline"),
            ));
            overlay.footer = Some(
                chart_anchor_footer(remote_training, Some(selected), Some(compare))
                    .unwrap_or_else(|| current_vs_baseline_footer(compare)),
            );
            overlay
                .series
                .extend(baseline.series.into_iter().map(|series| OwnedChartSeries {
                    label: series.label,
                    values: series.values,
                    color: series.color.with_alpha(0.55),
                    fill_alpha: 0.0,
                    line_alpha: 0.58,
                }));
            paint_chart_panel_view(bounds, accent, phase, &overlay, paint);
            return;
        }
        if compare.mode != CompareNormalizationMode::PendingBaseline {
            let mut current = current;
            current.footer = Some(
                chart_anchor_footer(remote_training, Some(selected), Some(compare))
                    .unwrap_or_else(|| compare.caveat.clone()),
            );
            let mut baseline = hardware_chart_view(compare.baseline, "baseline");
            baseline.footer = Some(current_vs_baseline_footer(compare));
            paint_side_by_side_chart_views(bounds, accent, phase, &current, &baseline, paint);
            return;
        }
    }
    let mut current = current;
    current.footer =
        chart_anchor_footer(remote_training, Some(selected), compare).or(current.footer.clone());

    let body = viz_panel_body_bounds(bounds);
    let content_body = Bounds::new(
        body.origin.x,
        body.origin.y,
        body.size.width,
        (body.size.height - 28.0).max(80.0),
    );
    let chart_width = (content_body.size.width * 0.56).max(180.0);
    let chart_bounds = Bounds::new(
        content_body.origin.x,
        content_body.origin.y,
        chart_width,
        content_body.size.height,
    );
    paint_chart_panel_view(chart_bounds, accent, phase, &current, paint);

    let side_bounds = Bounds::new(
        chart_bounds.max_x() + 10.0,
        content_body.origin.y,
        (content_body.max_x() - chart_bounds.max_x() - 10.0).max(116.0),
        content_body.size.height,
    );
    paint_topology_matrix(
        Bounds::new(
            side_bounds.origin.x,
            side_bounds.origin.y + 2.0,
            side_bounds.size.width,
            side_bounds.size.height * 0.52,
        ),
        selected,
        remote_training.focused_topology_target.as_deref(),
        accent,
        phase,
        paint,
    );
    paint_hardware_signal_triplet(
        side_bounds.origin.x + 8.0,
        side_bounds.origin.y + side_bounds.size.height * 0.56,
        selected,
        accent,
        paint,
    );

    let focus_targets = topology_focus_targets(selected);
    for (index, target) in focus_targets.iter().enumerate() {
        let target_bounds = psionic_remote_training_topology_target_bounds(content_bounds, index);
        let selected_focus =
            remote_training.focused_topology_target.as_deref() == Some(target.key.as_str());
        paint.scene.draw_quad(
            Quad::new(target_bounds)
                .with_background(if selected_focus {
                    accent.with_alpha(0.18)
                } else {
                    theme::bg::APP.with_alpha(0.88)
                })
                .with_border(
                    if selected_focus {
                        accent
                    } else {
                        theme::border::DEFAULT
                    },
                    1.0,
                )
                .with_corner_radius(6.0),
        );
        let label = truncate_line(
            paint,
            target.label.as_str(),
            target_bounds.size.width - 8.0,
            9.0,
        );
        paint.scene.draw_text(paint.text.layout_mono(
            label.as_str(),
            Point::new(target_bounds.origin.x + 4.0, target_bounds.origin.y + 6.0),
            9.0,
            if selected_focus {
                accent
            } else {
                theme::text::PRIMARY
            },
        ));
    }

    let focus_detail = remote_training
        .focused_topology_target
        .as_deref()
        .and_then(|target_key| {
            focus_targets
                .iter()
                .find(|target| target.key == target_key)
                .map(|target| format!("focus {} // {}", target.label, target.detail))
        })
        .or_else(|| {
            selected
                .bundle
                .as_ref()
                .and_then(|bundle| latest_gpu_samples(bundle).first().copied())
                .map(|device| {
                    format!(
                        "{} // temp {} // power {}",
                        truncate_to_chars(device.device_id.as_str(), 12),
                        device
                            .temperature_celsius
                            .map(|value| format!("{value}C"))
                            .unwrap_or_else(|| "-".to_string()),
                        device
                            .power_watts
                            .map(|value| format!("{value}W"))
                            .unwrap_or_else(|| "-".to_string()),
                    )
                })
        })
        .unwrap_or_else(|| "focus unavailable".to_string());
    let focus_detail = truncate_line(paint, focus_detail.as_str(), bounds.size.width - 32.0, 9.0);
    paint.scene.draw_text(paint.text.layout_mono(
        focus_detail.as_str(),
        Point::new(bounds.origin.x + 16.0, bounds.max_y() - 40.0),
        9.0,
        theme::text::MUTED,
    ));
}

fn paint_event_panel(
    bounds: Bounds,
    remote_training: &DesktopControlRemoteTrainingStatus,
    selected: Option<&DesktopControlRemoteTrainingSelectedRunStatus>,
    compare: Option<&TrainingRunCompare<'_>>,
    phase: f32,
    paint: &mut PaintContext,
) {
    let Some(selected) = selected else {
        paint_event_feed_body(
            bounds,
            remote_coral(),
            phase,
            "Select a run to inspect its live event feed.",
            &[],
            paint,
        );
        return;
    };
    let rows = selected
        .bundle
        .as_ref()
        .map(|bundle| {
            build_event_rows(
                bundle,
                remote_training.focused_topology_target.as_deref(),
                remote_training.chart_anchor_ratio_milli,
            )
        })
        .unwrap_or_default();
    let empty = compare
        .map(|compare| {
            if remote_training.focused_topology_target.is_some() {
                format!(
                    "{} // {}",
                    compare.caveat, "event feed filtered by topology focus"
                )
            } else {
                compare.caveat.clone()
            }
        })
        .unwrap_or_else(|| {
            selected
                .run
                .series_unavailable_reason
                .clone()
                .unwrap_or_else(|| "No events were retained for this run.".to_string())
        });
    paint_event_feed_body(
        bounds,
        remote_coral(),
        phase,
        empty.as_str(),
        rows.as_slice(),
        paint,
    );
}

fn paint_provenance_panel(
    bounds: Bounds,
    remote_training: &DesktopControlRemoteTrainingStatus,
    selected: Option<&DesktopControlRemoteTrainingSelectedRunStatus>,
    _phase: f32,
    paint: &mut PaintContext,
) {
    let body = viz_panel_body_bounds(bounds);
    let Some(selected) = selected else {
        paint.scene.draw_text(paint.text.layout(
            "Select a run to inspect retained evidence and mirror health.",
            Point::new(body.origin.x + 8.0, body.origin.y + 18.0),
            11.0,
            theme::text::MUTED,
        ));
        return;
    };

    paint_signal_triplet(
        body.origin.x + 6.0,
        body.origin.y + 8.0,
        [
            ("FR", freshness_level(&selected.run), remote_blue()),
            (
                "CA",
                if selected.run.bundle_cached {
                    1.0
                } else {
                    0.12
                },
                remote_gold(),
            ),
            ("AU", provenance_authority_level(selected), remote_mint()),
        ],
        remote_mint(),
        paint,
    );

    let source_index = selected
        .source_index_path
        .as_deref()
        .unwrap_or("source index unavailable");
    let info_x = body.origin.x + 146.0;
    for (index, line) in [
        format!("sync {}", remote_training.sync_state),
        truncate_to_chars(selected.run.run_id.as_str(), 30),
        format!(
            "digest {}",
            truncate_to_chars(selected.run.bundle_digest.as_deref().unwrap_or("-"), 22)
        ),
        format!("index {}", truncate_to_chars(source_index, 26)),
    ]
    .into_iter()
    .enumerate()
    {
        let line = truncate_line(paint, line.as_str(), body.size.width - 154.0, 9.0);
        paint.scene.draw_text(paint.text.layout_mono(
            line.as_str(),
            Point::new(info_x, body.origin.y + 10.0 + index as f32 * 12.0),
            9.0,
            theme::text::PRIMARY,
        ));
    }

    if let Some(bundle) = selected.bundle.as_ref() {
        let tile_width = ((body.size.width - 14.0) / 2.0).max(96.0);
        for (index, artifact) in bundle.source_artifacts.iter().take(4).enumerate() {
            let row = index / 2;
            let col = index % 2;
            let row_bounds = Bounds::new(
                body.origin.x + col as f32 * (tile_width + 10.0),
                body.origin.y + 76.0 + row as f32 * 42.0,
                tile_width,
                36.0,
            );
            let expanded = remote_training.selected_provenance_artifact_role.as_deref()
                == Some(artifact.artifact_role.as_str());
            paint_provenance_artifact_tile(row_bounds, artifact, expanded, paint);
        }
    }

    if let Some(artifact) = focused_provenance_artifact(
        selected,
        remote_training.selected_provenance_artifact_role.as_deref(),
    ) {
        let detail_bounds = Bounds::new(body.origin.x, body.max_y() - 38.0, body.size.width, 32.0);
        paint.scene.draw_quad(
            Quad::new(detail_bounds)
                .with_background(remote_mint().with_alpha(0.10))
                .with_border(remote_mint().with_alpha(0.48), 1.0)
                .with_corner_radius(6.0),
        );
        let receipts = if artifact.source_receipt_ids.is_empty() {
            "-".to_string()
        } else {
            artifact.source_receipt_ids.join(", ")
        };
        for (index, line) in [
            format!(
                "{} // auth={} // digest={}",
                artifact.artifact_role,
                artifact.authoritative,
                truncate_to_chars(artifact.artifact_digest.as_deref().unwrap_or("-"), 20),
            ),
            format!(
                "{} // receipts {}",
                truncate_to_chars(artifact.artifact_uri.as_str(), 28),
                truncate_to_chars(receipts.as_str(), 18)
            ),
        ]
        .into_iter()
        .enumerate()
        {
            paint.scene.draw_text(paint.text.layout_mono(
                line.as_str(),
                Point::new(
                    detail_bounds.origin.x + 8.0,
                    detail_bounds.origin.y + 8.0 + index as f32 * 12.0,
                ),
                9.0,
                if index == 0 {
                    remote_mint()
                } else {
                    theme::text::PRIMARY
                },
            ));
        }
    }
}

fn paint_run_row_status_matrix(
    run: &DesktopControlRemoteTrainingRunStatus,
    bounds: Bounds,
    selected: bool,
    paint: &mut PaintContext,
) {
    let cell_size = 7.0;
    let gap = 4.0;
    let origin_x = bounds.max_x() - 44.0;
    let origin_y = bounds.origin.y + 9.0;
    let track = run_track_accent(run);
    let cells = [
        (selected, track),
        (!run.stale, remote_blue()),
        (run.bundle_cached, remote_gold()),
        (run.primary_score.is_some(), remote_mint()),
        (run.series_status == "available", remote_coral()),
        (run.contract_error.is_none(), theme::status::SUCCESS),
        (
            run.track.comparability_class != "not_comparable",
            track.with_alpha(0.86),
        ),
        (
            run.track.proof_posture != "summary_only",
            remote_mint().with_alpha(0.92),
        ),
    ];
    for (index, (active, color)) in cells.into_iter().enumerate() {
        let row = index / 4;
        let col = index % 4;
        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                origin_x + col as f32 * (cell_size + gap),
                origin_y + row as f32 * (cell_size + gap),
                cell_size,
                cell_size,
            ))
            .with_background(color.with_alpha(if active { 0.9 } else { 0.14 }))
            .with_corner_radius(1.5),
        );
    }
}

fn paint_metric_ring(
    bounds: Bounds,
    label: &str,
    value: &str,
    level: f32,
    color: Hsla,
    paint: &mut PaintContext,
) {
    let mut ring = RingGauge::new()
        .level(level.clamp(0.0, 1.0))
        .segments(28)
        .dot_size(3.0)
        .active_color(color.with_alpha(0.92))
        .inactive_color(viz_theme::surface::CHART_BG.with_alpha(0.78))
        .head_color(theme::text::PRIMARY.with_alpha(0.98));
    ring.paint(bounds, paint);
    paint.scene.draw_text(paint.text.layout_mono(
        label,
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 8.0),
        8.0,
        color.with_alpha(0.9),
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        truncate_to_chars(value, 10).as_str(),
        Point::new(bounds.origin.x + 6.0, bounds.max_y() - 12.0),
        8.0,
        theme::text::PRIMARY,
    ));
}

fn paint_run_telemetry_matrix(
    bounds: Bounds,
    selected: &DesktopControlRemoteTrainingSelectedRunStatus,
    accent: Hsla,
    phase: f32,
    paint: &mut PaintContext,
) {
    let labels = ["LS", "MA", "RT", "HW"];
    let label_width = 18.0;
    let matrix_bounds = Bounds::new(
        bounds.origin.x + label_width,
        bounds.origin.y,
        (bounds.size.width - label_width).max(48.0),
        bounds.size.height,
    );

    let mut dots = DotsGrid::new()
        .shape(DotShape::Cross)
        .distance(18.0)
        .size(0.7)
        .color(accent.with_alpha(0.12))
        .animation_progress(1.0);
    dots.paint(matrix_bounds, paint);

    let mut heatmap = Heatmap::new()
        .data(4, 12, build_run_telemetry_matrix(selected))
        .range(0.0, 1.0)
        .gap(2.0)
        .low_color(viz_theme::surface::CHART_BG.with_alpha(0.94))
        .mid_color(Some(accent.with_alpha(0.62)))
        .high_color(theme::text::PRIMARY.with_alpha(0.98));
    heatmap.paint(matrix_bounds, paint);

    let mut scanlines = Scanlines::new()
        .spacing(10.0)
        .line_color(accent.with_alpha(0.05))
        .scan_color(accent.with_alpha(0.14))
        .scan_width(12.0)
        .scan_progress(phase)
        .opacity(0.82);
    scanlines.paint(matrix_bounds, paint);

    for (index, label) in labels.into_iter().enumerate() {
        paint.scene.draw_text(paint.text.layout_mono(
            label,
            Point::new(bounds.origin.x, bounds.origin.y + 8.0 + index as f32 * 14.0),
            8.0,
            accent.with_alpha(0.9),
        ));
    }
}

fn build_run_telemetry_matrix(
    selected: &DesktopControlRemoteTrainingSelectedRunStatus,
) -> Vec<f32> {
    let Some(bundle) = selected.bundle.as_ref() else {
        return vec![0.08; 48];
    };
    let loss = bundle
        .loss_series
        .iter()
        .filter_map(|sample| sample.train_loss.or(sample.ema_loss))
        .collect::<Vec<_>>();
    let math = bundle
        .math_series
        .iter()
        .filter_map(|sample| sample.gradient_norm.or(sample.update_norm))
        .collect::<Vec<_>>();
    let runtime = bundle
        .runtime_series
        .iter()
        .filter_map(|sample| {
            sample
                .tokens_per_second
                .map(|value| value as f32)
                .or_else(|| {
                    sample
                        .samples_per_second_milli
                        .map(|value| value as f32 / 1000.0)
                })
        })
        .collect::<Vec<_>>();
    let hardware = aggregated_gpu_percent(bundle, |sample| sample.utilization_bps as f32 / 100.0);

    let mut data = Vec::with_capacity(48);
    data.extend(sampled_normalized_series(loss.as_slice(), 12, true));
    data.extend(sampled_normalized_series(math.as_slice(), 12, false));
    data.extend(sampled_normalized_series(runtime.as_slice(), 12, false));
    data.extend(sampled_normalized_series(hardware.as_slice(), 12, false));
    data
}

fn sampled_normalized_series(values: &[f32], sample_count: usize, invert: bool) -> Vec<f32> {
    if values.is_empty() {
        return vec![0.08; sample_count];
    }
    let sampled = wgpui::viz::sampling::sample_history_series(values, sample_count);
    let min = sampled.iter().copied().fold(f32::INFINITY, f32::min);
    let max = sampled.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let span = (max - min).abs();
    if span < 1e-4 {
        return vec![0.64; sample_count];
    }
    sampled
        .into_iter()
        .map(|value| {
            let normalized = ((value - min) / span).clamp(0.0, 1.0);
            if invert { 1.0 - normalized } else { normalized }
        })
        .collect()
}

fn paint_topology_matrix(
    bounds: Bounds,
    selected: &DesktopControlRemoteTrainingSelectedRunStatus,
    focused_target: Option<&str>,
    accent: Hsla,
    phase: f32,
    paint: &mut PaintContext,
) {
    paint.scene.draw_text(paint.text.layout_mono(
        "DEVICE MATRIX",
        Point::new(bounds.origin.x, bounds.origin.y),
        8.0,
        accent.with_alpha(0.92),
    ));
    let Some(bundle) = selected.bundle.as_ref() else {
        return;
    };
    let latest = latest_gpu_samples(bundle);
    if latest.is_empty() {
        return;
    }

    let label_width = 54.0;
    let grid_bounds = Bounds::new(
        bounds.origin.x + label_width,
        bounds.origin.y + 12.0,
        (bounds.size.width - label_width).max(44.0),
        (bounds.size.height - 16.0).max(26.0),
    );
    let data = latest
        .iter()
        .flat_map(|sample| {
            [
                (sample.utilization_bps as f32 / 10_000.0).clamp(0.0, 1.0),
                (sample.memory_used_bytes as f32 / sample.memory_total_bytes as f32)
                    .clamp(0.0, 1.0),
                sample
                    .temperature_celsius
                    .map(|value| (value as f32 / 100.0).clamp(0.0, 1.0))
                    .unwrap_or(0.12),
                sample
                    .power_watts
                    .map(|value| (value as f32 / 700.0).clamp(0.0, 1.0))
                    .unwrap_or(0.12),
            ]
        })
        .collect::<Vec<_>>();

    let mut heatmap = Heatmap::new()
        .data(latest.len().max(1), 4, data)
        .range(0.0, 1.0)
        .gap(2.0)
        .low_color(viz_theme::surface::CHART_BG.with_alpha(0.94))
        .mid_color(Some(accent.with_alpha(0.72)))
        .high_color(theme::text::PRIMARY.with_alpha(0.98));
    heatmap.paint(grid_bounds, paint);

    let mut scanlines = Scanlines::new()
        .spacing(10.0)
        .line_color(accent.with_alpha(0.04))
        .scan_color(accent.with_alpha(0.12))
        .scan_width(12.0)
        .scan_progress(phase)
        .opacity(0.72);
    scanlines.paint(grid_bounds, paint);

    for (row, sample) in latest.iter().enumerate() {
        let label = truncate_to_chars(sample.device_label.as_str(), 9);
        let color = if focused_target == Some(format!("device:{}", sample.device_id).as_str()) {
            accent
        } else {
            theme::text::MUTED
        };
        paint.scene.draw_text(paint.text.layout_mono(
            label.as_str(),
            Point::new(bounds.origin.x, bounds.origin.y + 20.0 + row as f32 * 14.0),
            8.0,
            color,
        ));
    }
    for (index, label) in ["UT", "MM", "TP", "PW"].into_iter().enumerate() {
        paint.scene.draw_text(paint.text.layout_mono(
            label,
            Point::new(
                grid_bounds.origin.x + 8.0 + index as f32 * 22.0,
                bounds.origin.y,
            ),
            8.0,
            accent.with_alpha(0.86),
        ));
    }
}

fn paint_hardware_signal_triplet(
    origin_x: f32,
    origin_y: f32,
    selected: &DesktopControlRemoteTrainingSelectedRunStatus,
    accent: Hsla,
    paint: &mut PaintContext,
) {
    let Some(bundle) = selected.bundle.as_ref() else {
        return;
    };
    let latest = latest_gpu_samples(bundle);
    let util = average_f32(
        latest
            .iter()
            .map(|sample| sample.utilization_bps as f32 / 10_000.0)
            .collect::<Vec<_>>()
            .as_slice(),
    );
    let memory = average_f32(
        latest
            .iter()
            .map(|sample| sample.memory_used_bytes as f32 / sample.memory_total_bytes as f32)
            .collect::<Vec<_>>()
            .as_slice(),
    );
    let fabric = bundle
        .distributed_series
        .last()
        .and_then(|sample| sample.rank_skew_ms)
        .map(|value| (1.0 - value as f32 / 250.0).clamp(0.0, 1.0))
        .unwrap_or_else(|| if latest.len() > 1 { 0.52 } else { 0.18 });

    paint_signal_triplet(
        origin_x,
        origin_y,
        [
            ("UT", util, accent),
            ("MM", memory, remote_blue()),
            ("FX", fabric, remote_mint()),
        ],
        accent,
        paint,
    );
}

fn paint_signal_triplet(
    origin_x: f32,
    origin_y: f32,
    specs: [(&str, f32, Hsla); 3],
    accent: Hsla,
    paint: &mut PaintContext,
) {
    for (index, (label, level, color)) in specs.into_iter().enumerate() {
        let x = origin_x + index as f32 * 36.0;
        let mut meter = SignalMeter::new()
            .bars(6)
            .gap(2.0)
            .level(level.clamp(0.0, 1.0))
            .min_bar_height(0.16)
            .active_color(color.with_alpha(0.94))
            .inactive_color(accent.with_alpha(0.08));
        meter.paint(Bounds::new(x, origin_y, 18.0, 44.0), paint);
        paint.scene.draw_text(paint.text.layout_mono(
            label,
            Point::new(x - 1.0, origin_y + 52.0),
            8.0,
            color.with_alpha(0.88),
        ));
    }
}

fn paint_provenance_artifact_tile(
    bounds: Bounds,
    artifact: &RemoteTrainingSourceArtifact,
    expanded: bool,
    paint: &mut PaintContext,
) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(if expanded {
                remote_mint().with_alpha(0.16)
            } else {
                theme::bg::APP.with_alpha(0.72)
            })
            .with_border(
                if expanded {
                    remote_mint()
                } else {
                    theme::border::DEFAULT
                },
                1.0,
            )
            .with_corner_radius(6.0),
    );
    if artifact.authoritative {
        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.max_x() - 10.0,
                bounds.origin.y + 6.0,
                4.0,
                4.0,
            ))
            .with_background(remote_mint()),
        );
    }
    let role = truncate_line(
        paint,
        artifact.artifact_role.as_str(),
        bounds.size.width - 14.0,
        9.0,
    );
    let detail = truncate_line(
        paint,
        format!(
            "{} // r{}",
            artifact_source_kind_label(artifact.source_kind),
            artifact.source_receipt_ids.len()
        )
        .as_str(),
        bounds.size.width - 14.0,
        8.0,
    );
    paint.scene.draw_text(paint.text.layout_mono(
        role.as_str(),
        Point::new(bounds.origin.x + 6.0, bounds.origin.y + 7.0),
        9.0,
        if expanded {
            remote_mint()
        } else {
            theme::text::PRIMARY
        },
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        detail.as_str(),
        Point::new(bounds.origin.x + 6.0, bounds.origin.y + 20.0),
        8.0,
        theme::text::MUTED,
    ));
}

fn freshness_level(run: &DesktopControlRemoteTrainingRunStatus) -> f32 {
    match run.heartbeat_age_ms {
        Some(age_ms) if run.stale => (1.0 - age_ms as f32 / 120_000.0).clamp(0.08, 0.24),
        Some(age_ms) => (1.0 - age_ms as f32 / 30_000.0).clamp(0.1, 1.0),
        None => 0.08,
    }
}

fn proof_level(proof_posture: &str) -> f32 {
    match proof_posture {
        "summary_only" => 0.22,
        "runtime_measured" => 0.58,
        "score_closeout_measured" => 0.82,
        "bounded_train_to_infer" => 0.9,
        "refused" => 0.12,
        _ => 0.34,
    }
}

fn series_coverage_level(bundle: Option<&RemoteTrainingVisualizationBundleV2>) -> f32 {
    let Some(bundle) = bundle else {
        return 0.12;
    };
    let mut present = 0.0_f32;
    let total = 6.0_f32;
    present += if bundle.loss_series.is_empty() {
        0.0
    } else {
        1.0
    };
    present += if bundle.math_series.is_empty() {
        0.0
    } else {
        1.0
    };
    present += if bundle.runtime_series.is_empty() {
        0.0
    } else {
        1.0
    };
    present += if bundle.gpu_series.is_empty() {
        0.0
    } else {
        1.0
    };
    present += if bundle.distributed_series.is_empty() {
        0.0
    } else {
        1.0
    };
    present += if bundle.event_series.is_empty() {
        0.0
    } else {
        1.0
    };
    if total == 0.0 {
        0.0
    } else {
        (present / total).clamp(0.08_f32, 1.0_f32)
    }
}

fn provenance_authority_level(selected: &DesktopControlRemoteTrainingSelectedRunStatus) -> f32 {
    let Some(bundle) = selected.bundle.as_ref() else {
        return 0.14;
    };
    if bundle.source_artifacts.is_empty() {
        return 0.14;
    }
    let authoritative = bundle
        .source_artifacts
        .iter()
        .filter(|artifact| artifact.authoritative)
        .count();
    (authoritative as f32 / bundle.source_artifacts.len() as f32).clamp(0.08, 1.0)
}

fn score_surface_compact_label(run: &DesktopControlRemoteTrainingRunStatus) -> String {
    run.score_surface
        .as_ref()
        .map(|surface| compact_label(surface.score_closeout_posture.as_str()))
        .unwrap_or_else(|| "score pending".to_string())
}

fn average_f32(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().copied().sum::<f32>() / values.len() as f32
}

fn status_cards(
    remote_training: &DesktopControlRemoteTrainingStatus,
    selected: Option<&DesktopControlRemoteTrainingSelectedRunStatus>,
    compare: Option<&TrainingRunCompare<'_>>,
) -> Vec<(&'static str, String, Hsla)> {
    let sync_color = match remote_training.sync_state.as_str() {
        "ready" => theme::status::SUCCESS,
        "stale" => theme::status::WARNING,
        "error" => theme::status::ERROR,
        _ => remote_blue(),
    };
    let partial_count = remote_training
        .runs
        .iter()
        .filter(|run| run.series_status == "partial")
        .count();
    let coverage = format!(
        "{} full / {} partial / {} unavailable",
        remote_training.full_series_run_count,
        partial_count,
        remote_training.summary_only_run_count
    );
    let selected_value = selected
        .map(|selected| primary_score_brief_or_track(&selected.run))
        .unwrap_or_else(|| "no selection".to_string());
    let freshness = selected
        .map(|selected| {
            format!(
                "{} // {}",
                compact_label(selected.run.track.comparability_class.as_str()),
                compact_label(selected.run.track.proof_posture.as_str())
            )
        })
        .unwrap_or_else(|| "no heartbeat".to_string());
    let compare_value = compare
        .map(|compare| {
            compare
                .delta_summary
                .clone()
                .unwrap_or_else(|| compare.caveat.clone())
        })
        .unwrap_or_else(|| "compare idle".to_string());
    vec![
        (
            "SYNC",
            format!(
                "{} @ {}ms",
                remote_training.sync_state, remote_training.refresh_interval_ms
            ),
            sync_color,
        ),
        (
            "RUNS",
            format!(
                "{} total / {} active",
                remote_training.run_count, remote_training.active_run_count
            ),
            remote_blue(),
        ),
        ("COVERAGE", coverage, remote_gold()),
        (
            "SELECTED",
            format!("{selected_value} // {freshness}"),
            remote_mint(),
        ),
        ("COMPARE", compare_value, remote_coral()),
    ]
}

fn selected_summary_line(
    remote_training: &DesktopControlRemoteTrainingStatus,
    selected: Option<&DesktopControlRemoteTrainingSelectedRunStatus>,
    compare: Option<&TrainingRunCompare<'_>>,
) -> String {
    if let Some(selected) = selected {
        let checkpoint = selected
            .bundle
            .as_ref()
            .and_then(|bundle| bundle.summary.latest_checkpoint_ref.as_deref())
            .unwrap_or("checkpoint unavailable");
        let base = format!(
            "{} // {} // {} // {}",
            primary_score_brief_or_track(&selected.run),
            compact_label(selected.run.track.proof_posture.as_str()),
            freshness_label(&selected.run),
            truncate_to_chars(checkpoint, 42)
        );
        if let Some(compare) = compare {
            format!(
                "{} // {} // {}",
                base,
                baseline_compare_label(compare),
                compare
                    .delta_summary
                    .as_deref()
                    .unwrap_or(compare.caveat.as_str())
            )
        } else if let Some(anchor) = compare_anchor_label(remote_training, Some(selected), compare)
        {
            format!("{base} // {anchor}")
        } else {
            base
        }
    } else if remote_training.available {
        format!(
            "{} runs visible // pick a row to inspect the live mirror",
            remote_training.run_count
        )
    } else {
        "Remote training mirror is not configured yet.".to_string()
    }
}

fn latest_phase_label(
    bundle: Option<&RemoteTrainingVisualizationBundleV2>,
    run: &DesktopControlRemoteTrainingRunStatus,
) -> String {
    bundle
        .and_then(|bundle| bundle.heartbeat_series.last())
        .map(|sample| match sample.subphase.as_deref() {
            Some(subphase) => format!("{}.{subphase}", sample.phase),
            None => sample.phase.clone(),
        })
        .or_else(|| {
            bundle.and_then(|bundle| {
                bundle
                    .timeline
                    .last()
                    .map(|entry| match entry.subphase.as_deref() {
                        Some(subphase) => format!("{}.{subphase}", entry.phase),
                        None => entry.phase.clone(),
                    })
            })
        })
        .unwrap_or_else(|| format!("{} // {}", run.result_classification, run.series_status))
}

fn freshness_label(run: &DesktopControlRemoteTrainingRunStatus) -> String {
    match (run.stale, run.heartbeat_age_ms) {
        (true, Some(age_ms)) => format!("stale {age_ms}ms"),
        (false, Some(age_ms)) => format!("{age_ms}ms old"),
        (_, None) => "heartbeat unavailable".to_string(),
    }
}

fn aggregated_gpu_percent(
    bundle: &RemoteTrainingVisualizationBundleV2,
    projector: impl Fn(&RemoteTrainingGpuSample) -> f32,
) -> Vec<f32> {
    let mut by_timestamp = std::collections::BTreeMap::<u64, (f32, usize)>::new();
    for sample in &bundle.gpu_series {
        let entry = by_timestamp
            .entry(sample.observed_at_ms)
            .or_insert((0.0, 0));
        entry.0 += projector(sample);
        entry.1 += 1;
    }
    by_timestamp
        .into_values()
        .map(|(total, count)| total / count.max(1) as f32)
        .collect()
}

fn latest_gpu_samples(
    bundle: &RemoteTrainingVisualizationBundleV2,
) -> Vec<&RemoteTrainingGpuSample> {
    let latest = bundle
        .gpu_series
        .iter()
        .map(|sample| sample.observed_at_ms)
        .max();
    latest.map_or_else(Vec::new, |latest| {
        bundle
            .gpu_series
            .iter()
            .filter(|sample| sample.observed_at_ms == latest)
            .collect()
    })
}

pub(crate) fn topology_focus_targets(
    selected: &DesktopControlRemoteTrainingSelectedRunStatus,
) -> Vec<RemoteTrainingTopologyFocusTarget> {
    let Some(bundle) = selected.bundle.as_ref() else {
        return Vec::new();
    };
    let mut targets = latest_gpu_samples(bundle)
        .into_iter()
        .take(3)
        .map(|sample| RemoteTrainingTopologyFocusTarget {
            key: format!("device:{}", sample.device_id),
            label: sample.device_label.clone(),
            detail: format!(
                "util {} // mem {} // temp {}",
                format_percent_bps(sample.utilization_bps),
                format_bytes_compact(sample.memory_used_bytes),
                sample
                    .temperature_celsius
                    .map(|value| format!("{value}C"))
                    .unwrap_or_else(|| "-".to_string())
            ),
        })
        .collect::<Vec<_>>();
    if let Some(distributed) = bundle.distributed_series.last() {
        targets.push(RemoteTrainingTopologyFocusTarget {
            key: "fabric".to_string(),
            label: format!("fabric {}", distributed.participating_rank_count),
            detail: distributed_focus_detail(distributed),
        });
    }
    targets.truncate(4);
    targets
}

pub(crate) fn provenance_artifact_roles(
    selected: &DesktopControlRemoteTrainingSelectedRunStatus,
) -> Vec<String> {
    selected
        .bundle
        .as_ref()
        .map(|bundle| {
            bundle
                .source_artifacts
                .iter()
                .take(4)
                .map(|artifact| artifact.artifact_role.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn build_compare_state<'a>(
    remote_training: &'a DesktopControlRemoteTrainingStatus,
    selected: Option<&'a DesktopControlRemoteTrainingSelectedRunStatus>,
) -> Option<TrainingRunCompare<'a>> {
    let selected = selected?;
    let baseline = remote_training.compare_baseline.as_ref()?;
    if baseline.run.run_id == selected.run.run_id {
        return Some(TrainingRunCompare {
            baseline,
            mode: CompareNormalizationMode::PendingBaseline,
            anchor_mode: CompareAnchorMode::None,
            delta_value: None,
            delta_summary: None,
            caveat: "baseline pinned; select another run to compare".to_string(),
        });
    }

    let score_metric_matches = selected
        .run
        .primary_score
        .as_ref()
        .zip(baseline.run.primary_score.as_ref())
        .map(|(current, baseline)| {
            current.score_metric_id == baseline.score_metric_id
                && current.score_direction == baseline.score_direction
        })
        .unwrap_or(false);
    let same_track = selected.run.track.track_id == baseline.run.track.track_id;
    let public_equivalent = score_metric_matches
        && selected.run.track.public_equivalence_class
            == baseline.run.track.public_equivalence_class
        && !matches!(
            selected.run.track.public_equivalence_class.as_str(),
            "not_applicable" | "not_public_equivalent"
        );

    let mode = if selected.run.primary_score.is_none() || baseline.run.primary_score.is_none() {
        CompareNormalizationMode::SummaryOnly
    } else if score_metric_matches && same_track {
        CompareNormalizationMode::DirectScore
    } else if public_equivalent {
        CompareNormalizationMode::PublicEquivalentScore
    } else {
        CompareNormalizationMode::SideBySide
    };

    let anchor_mode = if matches!(
        mode,
        CompareNormalizationMode::DirectScore | CompareNormalizationMode::PublicEquivalentScore
    ) {
        compare_anchor_mode(selected.bundle.as_ref(), baseline.bundle.as_ref())
    } else {
        CompareAnchorMode::None
    };

    let (delta_value, delta_summary) = selected
        .run
        .primary_score
        .as_ref()
        .zip(baseline.run.primary_score.as_ref())
        .filter(|(current, baseline)| {
            current.score_metric_id == baseline.score_metric_id
                && current.score_direction == baseline.score_direction
        })
        .map(|(current, baseline)| {
            let delta = current.score_value - baseline.score_value;
            let better = match current.score_direction.as_str() {
                "lower_is_better" => delta < 0.0,
                "higher_is_better" => delta > 0.0,
                _ => false,
            };
            let delta_label = if delta > 0.0 {
                format!("+{delta:.4}")
            } else {
                format!("{delta:.4}")
            };
            (
                Some(delta),
                Some(format!(
                    "{} {} vs baseline // {}",
                    delta_label,
                    current.score_unit,
                    if better { "better" } else { "worse_or_flat" }
                )),
            )
        })
        .unwrap_or((None, None));

    let caveat = match mode {
        CompareNormalizationMode::PendingBaseline => {
            "baseline pinned; select another run to compare".to_string()
        }
        CompareNormalizationMode::DirectScore => "same-track direct delta".to_string(),
        CompareNormalizationMode::PublicEquivalentScore => {
            "public-equivalent compare across distinct tracks".to_string()
        }
        CompareNormalizationMode::SummaryOnly => {
            "summary-only compare; primary score or series is unavailable".to_string()
        }
        CompareNormalizationMode::SideBySide => {
            "incomparable tracks; showing side-by-side without winner claims".to_string()
        }
    };

    Some(TrainingRunCompare {
        baseline,
        mode,
        anchor_mode,
        delta_value,
        delta_summary,
        caveat,
    })
}

fn compare_anchor_mode(
    current: Option<&RemoteTrainingVisualizationBundleV2>,
    baseline: Option<&RemoteTrainingVisualizationBundleV2>,
) -> CompareAnchorMode {
    let Some(current) = current else {
        return CompareAnchorMode::None;
    };
    let Some(baseline) = baseline else {
        return CompareAnchorMode::None;
    };
    if current
        .loss_series
        .iter()
        .any(|sample| sample.global_step.is_some())
        && baseline
            .loss_series
            .iter()
            .any(|sample| sample.global_step.is_some())
    {
        CompareAnchorMode::GlobalStep
    } else if !current.loss_series.is_empty() && !baseline.loss_series.is_empty() {
        CompareAnchorMode::ElapsedMs
    } else {
        CompareAnchorMode::None
    }
}

fn compare_anchor_label(
    remote_training: &DesktopControlRemoteTrainingStatus,
    selected: Option<&DesktopControlRemoteTrainingSelectedRunStatus>,
    compare: Option<&TrainingRunCompare<'_>>,
) -> Option<String> {
    let selected = selected?;
    let ratio = remote_training.chart_anchor_ratio_milli?;
    let bundle = selected.bundle.as_ref()?;
    match compare
        .map(|compare| compare.anchor_mode)
        .unwrap_or(CompareAnchorMode::None)
    {
        CompareAnchorMode::GlobalStep => anchor_global_step_label(bundle, ratio),
        CompareAnchorMode::ElapsedMs => anchor_elapsed_ms_label(bundle, ratio),
        CompareAnchorMode::None => anchor_elapsed_ms_label(bundle, ratio)
            .or_else(|| anchor_global_step_label(bundle, ratio)),
    }
}

fn anchor_global_step_label(
    bundle: &RemoteTrainingVisualizationBundleV2,
    ratio: u16,
) -> Option<String> {
    bundle_anchor_loss_sample(bundle, ratio)
        .and_then(|sample| sample.global_step)
        .map(|step| format!("anchor step {step}"))
}

fn anchor_elapsed_ms_label(
    bundle: &RemoteTrainingVisualizationBundleV2,
    ratio: u16,
) -> Option<String> {
    bundle_anchor_loss_sample(bundle, ratio).map(|sample| format!("anchor {}ms", sample.elapsed_ms))
}

fn bundle_anchor_loss_sample(
    bundle: &RemoteTrainingVisualizationBundleV2,
    ratio: u16,
) -> Option<&psionic_train::RemoteTrainingLossSample> {
    let index = anchor_index(bundle.loss_series.len(), ratio)?;
    bundle.loss_series.get(index)
}

fn anchor_index(len: usize, ratio: u16) -> Option<usize> {
    if len == 0 {
        return None;
    }
    Some(
        (((len - 1) as f32) * (ratio as f32 / 1000.0))
            .round()
            .clamp(0.0, (len - 1) as f32) as usize,
    )
}

fn build_event_rows(
    bundle: &RemoteTrainingVisualizationBundleV2,
    focused_topology_target: Option<&str>,
    chart_anchor_ratio_milli: Option<u16>,
) -> Vec<EventFeedRow<'static>> {
    let anchor_index =
        chart_anchor_ratio_milli.and_then(|ratio| anchor_index(bundle.event_series.len(), ratio));
    let filtered = bundle
        .event_series
        .iter()
        .enumerate()
        .filter(|(_, event)| {
            focused_topology_target.is_none_or(|target| event_matches_focus(event, target))
        })
        .collect::<Vec<_>>();
    let filtered = if filtered.is_empty() {
        bundle.event_series.iter().enumerate().collect::<Vec<_>>()
    } else {
        filtered
    };
    let mut ordered = filtered;
    if let Some(anchor_index) = anchor_index {
        ordered.sort_by_key(|(index, _)| usize::abs_diff(*index, anchor_index));
    } else {
        ordered.reverse();
    }
    ordered
        .into_iter()
        .take(6)
        .enumerate()
        .map(|(index, (_, event))| EventFeedRow {
            label: Cow::Owned(format!("E{:02}", index + 1)),
            detail: Cow::Owned(format!("{} // {}", event.event_kind, event.detail)),
            color: event_color(event.severity),
        })
        .collect()
}

fn event_matches_focus(event: &psionic_train::RemoteTrainingEventSample, target: &str) -> bool {
    let needle = target
        .strip_prefix("device:")
        .or_else(|| target.strip_prefix("fabric:"))
        .unwrap_or(target);
    event.event_kind.contains(needle) || event.detail.contains(needle)
}

fn distributed_focus_detail(distributed: &RemoteTrainingDistributedSample) -> String {
    format!(
        "skew {} // collective {} // stalls {}",
        distributed
            .rank_skew_ms
            .map(format_duration_ms)
            .unwrap_or_else(|| "-".to_string()),
        distributed
            .collective_ms
            .map(format_duration_ms)
            .unwrap_or_else(|| "-".to_string()),
        distributed.stalled_rank_count
    )
}

fn artifact_source_kind_label(source_kind: RemoteTrainingArtifactSourceKind) -> &'static str {
    match source_kind {
        RemoteTrainingArtifactSourceKind::RuntimeOwned => "runtime_owned",
        RemoteTrainingArtifactSourceKind::FinalizerOwned => "finalizer_owned",
        RemoteTrainingArtifactSourceKind::ProviderGenerated => "provider_generated",
        RemoteTrainingArtifactSourceKind::LogDerivedFallback => "log_derived_fallback",
        RemoteTrainingArtifactSourceKind::LocalMirror => "local_mirror",
    }
}

fn format_percent_bps(value: u32) -> String {
    format!("{:.1}%", value as f32 / 100.0)
}

fn paint_chart_panel_view(
    bounds: Bounds,
    accent: Hsla,
    phase: f32,
    view: &ChartPanelView,
    paint: &mut PaintContext,
) {
    let borrowed = view
        .series
        .iter()
        .map(|series| HistoryChartSeries {
            label: series.label,
            values: series.values.as_slice(),
            color: series.color,
            fill_alpha: series.fill_alpha,
            line_alpha: series.line_alpha,
        })
        .collect::<Vec<_>>();
    paint_history_chart_body(
        bounds,
        accent,
        phase,
        view.header.as_deref(),
        view.footer.as_deref(),
        view.empty_state.as_str(),
        borrowed.as_slice(),
        paint,
    );
}

fn split_chart_panel(bounds: Bounds) -> (Bounds, Bounds) {
    let half_height = ((bounds.size.height - 8.0) / 2.0).max(92.0);
    (
        Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            half_height,
        ),
        Bounds::new(
            bounds.origin.x,
            bounds.origin.y + half_height + 8.0,
            bounds.size.width,
            half_height,
        ),
    )
}

fn paint_side_by_side_chart_views(
    bounds: Bounds,
    accent: Hsla,
    phase: f32,
    current: &ChartPanelView,
    baseline: &ChartPanelView,
    paint: &mut PaintContext,
) {
    let (top, bottom) = split_chart_panel(bounds);
    paint_chart_panel_view(top, accent, phase, current, paint);
    paint_chart_panel_view(bottom, accent, phase, baseline, paint);
}

fn focused_provenance_artifact<'a>(
    selected: &'a DesktopControlRemoteTrainingSelectedRunStatus,
    focused_role: Option<&str>,
) -> Option<&'a RemoteTrainingSourceArtifact> {
    let bundle = selected.bundle.as_ref()?;
    let focused_role = focused_role?;
    bundle
        .source_artifacts
        .iter()
        .find(|artifact| artifact.artifact_role == focused_role)
}

fn baseline_compare_label(compare: &TrainingRunCompare<'_>) -> &'static str {
    match compare.mode {
        CompareNormalizationMode::PendingBaseline => "baseline pending",
        CompareNormalizationMode::DirectScore => "direct compare",
        CompareNormalizationMode::PublicEquivalentScore => "public-equivalent compare",
        CompareNormalizationMode::SummaryOnly => "summary-only compare",
        CompareNormalizationMode::SideBySide => "side-by-side compare",
    }
}

fn compare_can_overlay(compare: &TrainingRunCompare<'_>) -> bool {
    matches!(
        compare.mode,
        CompareNormalizationMode::DirectScore | CompareNormalizationMode::PublicEquivalentScore
    ) && compare.anchor_mode != CompareAnchorMode::None
        && compare.baseline.bundle.is_some()
}

fn chart_anchor_footer(
    remote_training: &DesktopControlRemoteTrainingStatus,
    selected: Option<&DesktopControlRemoteTrainingSelectedRunStatus>,
    compare: Option<&TrainingRunCompare<'_>>,
) -> Option<String> {
    compare_anchor_label(remote_training, selected, compare).map(|label| {
        compare
            .and_then(|compare| compare.delta_summary.clone())
            .map(|delta| format!("{label} // {delta}"))
            .unwrap_or(label)
    })
}

fn comparison_header(
    compare: Option<&TrainingRunCompare<'_>>,
    current_summary: &str,
    baseline_summary: &str,
) -> String {
    compare
        .map(|compare| match compare.mode {
            CompareNormalizationMode::DirectScore
            | CompareNormalizationMode::PublicEquivalentScore => {
                format!("{current_summary} // baseline {baseline_summary}")
            }
            _ => format!("current {current_summary} // baseline {baseline_summary}"),
        })
        .unwrap_or_else(|| current_summary.to_string())
}

fn current_vs_baseline_footer(compare: &TrainingRunCompare<'_>) -> String {
    compare
        .delta_summary
        .clone()
        .unwrap_or_else(|| compare.caveat.clone())
}

fn loss_chart_view(
    selected: &DesktopControlRemoteTrainingSelectedRunStatus,
    title_prefix: &str,
) -> ChartPanelView {
    let Some(bundle) = selected.bundle.as_ref() else {
        return ChartPanelView {
            header: Some(format!("{title_prefix} {}", selected.run.run_id)),
            footer: Some(selected.run.semantic_summary.clone()),
            empty_state: selected
                .run
                .series_unavailable_reason
                .clone()
                .unwrap_or_else(|| "No retained loss bundle is cached for this run.".to_string()),
            series: Vec::new(),
        };
    };
    ChartPanelView {
        header: Some(format!(
            "{title_prefix} train {} // ema {} // val {}",
            format_optional_f32(bundle.summary.latest_train_loss),
            format_optional_f32(bundle.summary.latest_ema_loss),
            format_optional_f32(bundle.summary.latest_validation_loss),
        )),
        footer: Some(format!(
            "steps {} // global {} // {}",
            bundle.summary.total_steps_completed,
            bundle
                .summary
                .latest_global_step
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            bundle
                .series_unavailable_reason
                .clone()
                .unwrap_or_else(|| "full loss series retained".to_string())
        )),
        empty_state: "No loss samples were retained for this run.".to_string(),
        series: vec![
            OwnedChartSeries {
                label: "train",
                values: bundle
                    .loss_series
                    .iter()
                    .filter_map(|sample| sample.train_loss)
                    .collect(),
                color: remote_coral(),
                fill_alpha: 0.11,
                line_alpha: 0.82,
            },
            OwnedChartSeries {
                label: "ema",
                values: bundle
                    .loss_series
                    .iter()
                    .filter_map(|sample| sample.ema_loss)
                    .collect(),
                color: remote_gold(),
                fill_alpha: 0.0,
                line_alpha: 0.88,
            },
            OwnedChartSeries {
                label: "validation",
                values: bundle
                    .loss_series
                    .iter()
                    .filter_map(|sample| sample.validation_loss)
                    .collect(),
                color: remote_mint(),
                fill_alpha: 0.0,
                line_alpha: 0.84,
            },
        ],
    }
}

fn math_chart_view(
    selected: &DesktopControlRemoteTrainingSelectedRunStatus,
    title_prefix: &str,
) -> ChartPanelView {
    let Some(bundle) = selected.bundle.as_ref() else {
        return ChartPanelView {
            header: Some(format!("{title_prefix} {}", selected.run.run_id)),
            footer: Some(selected.run.semantic_summary.clone()),
            empty_state: "No retained optimizer-math bundle is cached for this run.".to_string(),
            series: Vec::new(),
        };
    };
    let latest = bundle.math_series.last();
    let diagnostics = latest
        .map(|sample| {
            sample
                .model_specific_diagnostics
                .iter()
                .map(|(key, value)| format!("{key}={value:.2}"))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "no model-specific diagnostics".to_string());
    ChartPanelView {
        header: Some(format!(
            "{title_prefix} lr {} // grad {} // update {}",
            latest
                .and_then(|sample| sample.learning_rate)
                .map(|value| format!("{value:.6}"))
                .unwrap_or_else(|| "-".to_string()),
            latest
                .and_then(|sample| sample.gradient_norm)
                .map(|value| format!("{value:.3}"))
                .unwrap_or_else(|| "-".to_string()),
            latest
                .and_then(|sample| sample.update_norm)
                .map(|value| format!("{value:.3}"))
                .unwrap_or_else(|| "-".to_string()),
        )),
        footer: Some(format!(
            "param {} // loss_scale {} // non_finite {} // {}",
            latest
                .and_then(|sample| sample.parameter_norm)
                .map(|value| format!("{value:.2}"))
                .unwrap_or_else(|| "-".to_string()),
            latest
                .and_then(|sample| sample.loss_scale)
                .map(|value| format!("{value:.0}"))
                .unwrap_or_else(|| "-".to_string()),
            latest
                .map(|sample| sample.non_finite_count.to_string())
                .unwrap_or_else(|| "-".to_string()),
            diagnostics
        )),
        empty_state: "No retained optimizer-math samples were emitted for this run.".to_string(),
        series: vec![
            OwnedChartSeries {
                label: "grad",
                values: bundle
                    .math_series
                    .iter()
                    .filter_map(|sample| sample.gradient_norm)
                    .collect(),
                color: remote_mint(),
                fill_alpha: 0.10,
                line_alpha: 0.84,
            },
            OwnedChartSeries {
                label: "update",
                values: bundle
                    .math_series
                    .iter()
                    .filter_map(|sample| sample.update_norm)
                    .collect(),
                color: remote_blue(),
                fill_alpha: 0.0,
                line_alpha: 0.86,
            },
            OwnedChartSeries {
                label: "clip",
                values: bundle
                    .math_series
                    .iter()
                    .filter_map(|sample| sample.clip_fraction)
                    .collect(),
                color: remote_gold(),
                fill_alpha: 0.0,
                line_alpha: 0.82,
            },
        ],
    }
}

fn runtime_chart_view(
    selected: &DesktopControlRemoteTrainingSelectedRunStatus,
    title_prefix: &str,
) -> ChartPanelView {
    let Some(bundle) = selected.bundle.as_ref() else {
        return ChartPanelView {
            header: Some(format!("{title_prefix} {}", selected.run.run_id)),
            footer: Some(selected.run.semantic_summary.clone()),
            empty_state: "No retained runtime bundle is cached for this run.".to_string(),
            series: Vec::new(),
        };
    };
    let latest = bundle.runtime_series.last();
    ChartPanelView {
        header: Some(format!(
            "{title_prefix} tok/s {} // samp/s {}",
            latest
                .and_then(|sample| sample.tokens_per_second)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            latest
                .and_then(|sample| sample.samples_per_second_milli)
                .map(format_samples_per_second_milli)
                .unwrap_or_else(|| "-".to_string()),
        )),
        footer: Some(format!(
            "wait {} // ckpt {} // eval {}",
            latest
                .and_then(|sample| sample.data_wait_ms)
                .map(format_duration_ms)
                .unwrap_or_else(|| "-".to_string()),
            latest
                .and_then(|sample| sample.checkpoint_ms)
                .map(format_duration_ms)
                .unwrap_or_else(|| "-".to_string()),
            latest
                .and_then(|sample| sample.evaluation_ms)
                .map(format_duration_ms)
                .unwrap_or_else(|| "-".to_string()),
        )),
        empty_state: "No retained pipeline-timing samples were emitted for this run.".to_string(),
        series: vec![
            OwnedChartSeries {
                label: "forward",
                values: bundle
                    .runtime_series
                    .iter()
                    .filter_map(|sample| sample.forward_ms.map(|value| value as f32))
                    .collect(),
                color: remote_blue(),
                fill_alpha: 0.10,
                line_alpha: 0.86,
            },
            OwnedChartSeries {
                label: "backward",
                values: bundle
                    .runtime_series
                    .iter()
                    .filter_map(|sample| sample.backward_ms.map(|value| value as f32))
                    .collect(),
                color: remote_coral(),
                fill_alpha: 0.0,
                line_alpha: 0.86,
            },
            OwnedChartSeries {
                label: "optim",
                values: bundle
                    .runtime_series
                    .iter()
                    .filter_map(|sample| sample.optimizer_ms.map(|value| value as f32))
                    .collect(),
                color: remote_mint(),
                fill_alpha: 0.0,
                line_alpha: 0.82,
            },
        ],
    }
}

fn hardware_chart_view(
    selected: &DesktopControlRemoteTrainingSelectedRunStatus,
    title_prefix: &str,
) -> ChartPanelView {
    let Some(bundle) = selected.bundle.as_ref() else {
        return ChartPanelView {
            header: Some(format!("{title_prefix} {}", selected.run.run_id)),
            footer: Some(selected.run.semantic_summary.clone()),
            empty_state: "No retained device bundle is cached for this run.".to_string(),
            series: Vec::new(),
        };
    };
    let latest_devices = latest_gpu_samples(bundle);
    let distributed = bundle.distributed_series.last();
    ChartPanelView {
        header: Some(format!(
            "{title_prefix} devices {} // latest {}",
            latest_devices.len(),
            latest_devices
                .first()
                .map(|sample| sample.device_label.as_str())
                .unwrap_or("gpu unavailable"),
        )),
        footer: Some(
            distributed
                .map(distributed_focus_detail)
                .unwrap_or_else(|| "no distributed telemetry retained for this lane".to_string()),
        ),
        empty_state: "No retained GPU samples were emitted for this run.".to_string(),
        series: vec![
            OwnedChartSeries {
                label: "util%",
                values: aggregated_gpu_percent(bundle, |sample| {
                    sample.utilization_bps as f32 / 100.0
                }),
                color: remote_gold(),
                fill_alpha: 0.10,
                line_alpha: 0.86,
            },
            OwnedChartSeries {
                label: "mem%",
                values: aggregated_gpu_percent(bundle, |sample| {
                    sample.memory_used_bytes as f32 / sample.memory_total_bytes as f32 * 100.0
                }),
                color: remote_blue(),
                fill_alpha: 0.0,
                line_alpha: 0.84,
            },
        ],
    }
}

fn event_color(severity: RemoteTrainingEventSeverity) -> Hsla {
    match severity {
        RemoteTrainingEventSeverity::Info => remote_blue(),
        RemoteTrainingEventSeverity::Warning => theme::status::WARNING,
        RemoteTrainingEventSeverity::Error => theme::status::ERROR,
    }
}

fn animation_phase() -> f32 {
    current_epoch_ms() as f32 / 1000.0 % 1.0
}

fn current_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn paint_status_card(
    bounds: Bounds,
    label: &str,
    value: &str,
    accent: Hsla,
    paint: &mut PaintContext,
) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(accent.with_alpha(0.08))
            .with_border(accent.with_alpha(0.72), 1.0)
            .with_corner_radius(8.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            4.0,
            bounds.size.height,
        ))
        .with_background(accent.with_alpha(0.88)),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        label,
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 8.0),
        9.0,
        theme::text::MUTED,
    ));
    let wrapped = split_text_for_display(value, ((bounds.size.width - 20.0) / 6.2) as usize);
    paint.scene.draw_text(paint.text.layout(
        wrapped.first().map(String::as_str).unwrap_or("-"),
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 24.0),
        11.0,
        accent,
    ));
}

fn paint_summary_band(bounds: Bounds, text: &str, accent: Hsla, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(accent.with_alpha(0.08))
            .with_border(accent.with_alpha(0.72), 1.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            10.0,
            bounds.size.height,
        ))
        .with_background(accent.with_alpha(0.88)),
    );
    let chunk_len = ((bounds.size.width - 28.0) / 6.2).floor().max(18.0) as usize;
    let summary = split_text_for_display(text, chunk_len)
        .into_iter()
        .next()
        .unwrap_or_else(|| "REMOTE TRAINING".to_string());
    paint.scene.draw_text(paint.text.layout_mono(
        summary.as_str(),
        Point::new(bounds.origin.x + 16.0, bounds.origin.y + 9.0),
        11.0,
        theme::text::PRIMARY,
    ));
}

fn truncate_line(paint: &mut PaintContext, value: &str, max_width: f32, font_size: f32) -> String {
    if value.is_empty() || max_width <= 0.0 {
        return String::new();
    }
    let measure = |candidate: &str, paint: &mut PaintContext| -> f32 {
        paint
            .text
            .layout(candidate, Point::ZERO, font_size, theme::text::PRIMARY)
            .bounds()
            .size
            .width
    };
    if measure(value, paint) <= max_width {
        return value.to_string();
    }
    if measure("...", paint) > max_width {
        return String::new();
    }
    let chars: Vec<char> = value.chars().collect();
    let mut low = 0usize;
    let mut high = chars.len();
    while low < high {
        let mid = (low + high + 1) / 2;
        let candidate = format!("{}...", chars[..mid].iter().collect::<String>());
        if measure(candidate.as_str(), paint) <= max_width {
            low = mid;
        } else {
            high = mid - 1;
        }
    }
    if low == 0 {
        String::from("...")
    } else {
        format!("{}...", chars[..low].iter().collect::<String>())
    }
}

fn truncate_to_chars(value: &str, max_chars: usize) -> String {
    let chars = value.chars().collect::<Vec<_>>();
    if chars.len() <= max_chars {
        return value.to_string();
    }
    if max_chars <= 3 {
        return "...".to_string();
    }
    format!("{}...", chars[..max_chars - 3].iter().collect::<String>())
}

fn format_optional_f32(value: Option<f32>) -> String {
    value
        .map(|value| format!("{value:.3}"))
        .unwrap_or_else(|| "-".to_string())
}

fn format_duration_ms(value: u64) -> String {
    format!("{value}ms")
}

fn format_samples_per_second_milli(value: u32) -> String {
    format!("{:.2}", value as f32 / 1000.0)
}

fn format_bytes_compact(value: u64) -> String {
    if value >= 1_000_000_000 {
        format!("{:.1}GB", value as f32 / 1_000_000_000.0)
    } else if value >= 1_000_000 {
        format!("{:.1}MB", value as f32 / 1_000_000.0)
    } else if value >= 1_000 {
        format!("{:.1}KB", value as f32 / 1_000.0)
    } else {
        format!("{value}B")
    }
}

fn compact_label(value: &str) -> String {
    value.replace('_', " ")
}

fn primary_score_brief_or_track(run: &DesktopControlRemoteTrainingRunStatus) -> String {
    run.primary_score
        .as_ref()
        .map(primary_score_brief)
        .unwrap_or_else(|| compact_label(run.track.track_family.as_str()))
}

fn primary_score_brief(
    score: &crate::desktop_control::DesktopControlRemoteTrainingPrimaryScoreStatus,
) -> String {
    format!(
        "{} {:.4} {}",
        compact_label(score.score_metric_id.as_str()),
        score.score_value,
        score.score_unit
    )
}

fn primary_score_detail(
    score: &crate::desktop_control::DesktopControlRemoteTrainingPrimaryScoreStatus,
) -> String {
    format!(
        "{} {:.6} {} // {}",
        compact_label(score.score_metric_id.as_str()),
        score.score_value,
        score.score_unit,
        compact_label(score.score_direction.as_str())
    )
}

fn run_track_accent(run: &DesktopControlRemoteTrainingRunStatus) -> Hsla {
    match run.track.track_family.as_str() {
        "parameter_golf" => viz_theme::track::PGOLF,
        "homegolf" => viz_theme::track::HOMEGOLF,
        "xtrain" => viz_theme::track::XTRAIN,
        _ => viz_theme::series::RUNTIME,
    }
}

fn track_badge_tone(track_family: &str) -> BadgeTone {
    match track_family {
        "parameter_golf" => BadgeTone::TrackPgolf,
        "homegolf" => BadgeTone::TrackHomegolf,
        "xtrain" => BadgeTone::TrackXtrain,
        _ => BadgeTone::Neutral,
    }
}

fn state_badge_tone(run: &DesktopControlRemoteTrainingRunStatus) -> BadgeTone {
    if run.contract_error.is_some() {
        BadgeTone::Error
    } else if run.stale {
        BadgeTone::Warning
    } else if run.series_status == "available" {
        BadgeTone::Live
    } else {
        BadgeTone::Neutral
    }
}

fn contract_badge_tone(run: &DesktopControlRemoteTrainingRunStatus) -> BadgeTone {
    if run.contract_error.is_some() {
        BadgeTone::Error
    } else {
        BadgeTone::Neutral
    }
}

fn paint_badge_strip(
    x: f32,
    y: f32,
    badges: &[(String, BadgeTone)],
    accent: Hsla,
    paint: &mut PaintContext,
) {
    let mut cursor_x = x;
    for (label, tone) in badges {
        let badge = truncate_to_chars(label.as_str(), 28);
        let width = paint
            .text
            .layout_mono(badge.as_str(), Point::ZERO, 8.0, theme::text::PRIMARY)
            .bounds()
            .size
            .width
            + 18.0;
        paint.scene.draw_quad(
            Quad::new(Bounds::new(cursor_x, y, width, 16.0))
                .with_background(badge_tone_color(*tone).with_alpha(0.14))
                .with_border(accent.with_alpha(0.18), 1.0)
                .with_corner_radius(8.0),
        );
        paint.scene.draw_text(paint.text.layout_mono(
            badge.as_str(),
            Point::new(cursor_x + 8.0, y + 4.0),
            8.0,
            badge_tone_color(*tone).with_alpha(0.92),
        ));
        cursor_x += width + 6.0;
    }
}

fn remote_blue() -> Hsla {
    viz_theme::series::RUNTIME
}

fn remote_gold() -> Hsla {
    viz_theme::series::HARDWARE
}

fn remote_mint() -> Hsla {
    viz_theme::series::PROVENANCE
}

fn remote_coral() -> Hsla {
    viz_theme::series::LOSS
}
