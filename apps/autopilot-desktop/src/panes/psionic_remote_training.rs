use std::borrow::Cow;
use std::time::{SystemTime, UNIX_EPOCH};

use psionic_train::{
    RemoteTrainingArtifactSourceKind, RemoteTrainingDistributedSample,
    RemoteTrainingEventSeverity, RemoteTrainingGpuSample, RemoteTrainingSourceArtifact,
    RemoteTrainingVisualizationBundleV2,
};
use wgpui::viz::badge::{BadgeTone, tone_color as badge_tone_color};
use wgpui::viz::chart::{HistoryChartSeries, paint_history_chart_body};
use wgpui::viz::feed::{EventFeedRow, paint_event_feed_body};
use wgpui::viz::panel::{paint_shell as paint_panel_shell, paint_title as paint_panel_title};
use wgpui::viz::theme as viz_theme;
use wgpui::{Bounds, Hsla, PaintContext, Point, Quad, theme};

use crate::app_state::PaneLoadState;
use crate::desktop_control::{
    DesktopControlRemoteTrainingRunStatus, DesktopControlRemoteTrainingSelectedRunStatus,
    DesktopControlRemoteTrainingStatus,
};
use crate::pane_renderer::{
    paint_secondary_button, paint_selectable_row_background, paint_source_badge,
    paint_state_summary, split_text_for_display,
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

    paint_runs_panel(content_bounds, remote_training, paint);
    paint_run_detail_panel(
        layout.hero_panel,
        remote_training,
        selected,
        compare.as_ref(),
        accent,
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
    paint_provenance_panel(layout.provenance_panel, remote_training, selected, paint);
}

fn paint_runs_panel(
    content_bounds: Bounds,
    remote_training: &DesktopControlRemoteTrainingStatus,
    paint: &mut PaintContext,
) {
    let chunk_len = 32usize;
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
            bounds.size.width - 20.0,
            10.0,
        );
        paint.scene.draw_text(paint.text.layout_mono(
            headline.as_str(),
            Point::new(bounds.origin.x + 10.0, bounds.origin.y + 8.0),
            10.0,
            accent,
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
            bounds.size.width - 20.0,
            10.0,
        );
        paint.scene.draw_text(paint.text.layout(
            status_line.as_str(),
            Point::new(bounds.origin.x + 10.0, bounds.origin.y + 23.0),
            10.0,
            theme::text::MUTED,
        ));
        let primary = run
            .primary_score
            .as_ref()
            .map(primary_score_brief)
            .or_else(|| run.topology_summary.clone())
            .unwrap_or_else(|| truncate_to_chars(run.semantic_summary.as_str(), chunk_len));
        let freshness = if let Some(error) = run.contract_error.as_deref() {
            format!(
                "{} // contract {}",
                truncate_to_chars(primary.as_str(), chunk_len),
                truncate_to_chars(error, 20)
            )
        } else {
            format!(
                "{} // {}",
                truncate_to_chars(primary.as_str(), chunk_len),
                freshness_label(run)
            )
        };
        let freshness = truncate_line(paint, freshness.as_str(), bounds.size.width - 20.0, 10.0);
        paint.scene.draw_text(paint.text.layout(
            freshness.as_str(),
            Point::new(bounds.origin.x + 10.0, bounds.origin.y + 38.0),
            10.0,
            if run.contract_error.is_some() || run.stale {
                theme::status::ERROR
            } else {
                theme::text::PRIMARY
            },
        ));
    }
}

fn paint_run_detail_panel(
    bounds: Bounds,
    remote_training: &DesktopControlRemoteTrainingStatus,
    selected: Option<&DesktopControlRemoteTrainingSelectedRunStatus>,
    compare: Option<&TrainingRunCompare<'_>>,
    accent: Hsla,
    paint: &mut PaintContext,
) {
    let load_state = load_state_for_status(remote_training);
    let mut y = paint_state_summary(
        paint,
        bounds.origin.x + 16.0,
        bounds.origin.y + 34.0,
        load_state,
        selected
            .map(|selected| selected.run.run_id.as_str())
            .unwrap_or("No remote training run selected"),
        remote_training.last_action.as_deref(),
        remote_training.last_error.as_deref(),
    );

    let Some(selected) = selected else {
        return;
    };
    let bundle = selected.bundle.as_ref();
    let phase_line = latest_phase_label(bundle, &selected.run);
    let subsystems = latest_subsystems_label(bundle);
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
    paint_badge_strip(
        bounds.origin.x + 16.0,
        y,
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
    y += 22.0;

    if let Some(compare) = compare {
        for line in [
            format!("compare: {}", baseline_compare_label(compare)),
            format!("baseline: {}", compare.baseline.run.run_id),
            format!(
                "delta: {}",
                compare
                    .delta_summary
                    .as_deref()
                    .unwrap_or(compare.caveat.as_str())
            ),
        ] {
            let line = truncate_line(paint, line.as_str(), bounds.size.width - 32.0, 10.0);
            paint.scene.draw_text(paint.text.layout_mono(
                line.as_str(),
                Point::new(bounds.origin.x + 16.0, y),
                10.0,
                remote_gold(),
            ));
            y += 14.0;
        }
    }

    let score_line = selected
        .run
        .primary_score
        .as_ref()
        .map(primary_score_detail)
        .unwrap_or_else(|| "score: unavailable".to_string());
    let detail_lines = [
        format!("track: {}", track.track_id),
        format!("score: {score_line}"),
        format!(
            "closeout: {} // gate {}",
            selected
                .run
                .score_surface
                .as_ref()
                .map(|surface| compact_label(surface.score_closeout_posture.as_str()))
                .unwrap_or_else(|| "score unavailable".to_string()),
            selected
                .run
                .score_surface
                .as_ref()
                .map(|surface| compact_label(surface.promotion_gate_posture.as_str()))
                .unwrap_or_else(|| "n/a".to_string())
        ),
        format!("lane: {}", selected.run.lane_id),
        format!("repo: {}", selected.run.repo_revision),
        format!(
            "score law: {}",
            track.score_law_ref.as_deref().unwrap_or("-")
        ),
        format!(
            "caps: artifact {} // wallclock {}",
            track
                .artifact_cap_bytes
                .map(format_bytes_compact)
                .unwrap_or_else(|| "-".to_string()),
            track
                .wallclock_cap_seconds
                .map(|value| format!("{value}s"))
                .unwrap_or_else(|| "-".to_string())
        ),
        format!("phase: {phase_line}"),
        format!("subsystems: {subsystems}"),
        format!("checkpoint: {checkpoint}"),
        chart_anchor_footer(remote_training, Some(selected), compare)
            .map(|anchor| format!("anchor: {anchor}"))
            .unwrap_or_else(|| "anchor: live".to_string()),
        format!(
            "topology: {}",
            selected
                .run
                .topology_summary
                .as_deref()
                .unwrap_or("topology unavailable")
        ),
        format!(
            "contract: {}",
            selected
                .run
                .contract_error
                .as_deref()
                .unwrap_or(selected.run.contract_state.as_str())
        ),
        format!("heartbeat: {}", freshness_label(&selected.run)),
    ];
    for line in detail_lines {
        let line = truncate_line(paint, line.as_str(), bounds.size.width - 32.0, 10.0);
        paint.scene.draw_text(paint.text.layout_mono(
            line.as_str(),
            Point::new(bounds.origin.x + 16.0, y),
            10.0,
            theme::text::PRIMARY,
        ));
        y += 14.0;
    }

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
    let wrap = split_text_for_display(detail_text, 80);
    let mut wrap_y = bounds.origin.y + 34.0;
    let x = bounds.max_x() - 360.0;
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            x - 10.0,
            bounds.origin.y + 28.0,
            350.0,
            bounds.size.height - 36.0,
        ))
        .with_background(accent.with_alpha(0.08))
        .with_border(accent.with_alpha(0.22), 1.0)
        .with_corner_radius(8.0),
    );
    for line in wrap.iter().take(4) {
        paint.scene.draw_text(paint.text.layout(
            line.as_str(),
            Point::new(x, wrap_y),
            10.0,
            theme::text::PRIMARY,
        ));
        wrap_y += 14.0;
    }
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
            overlay.series.extend(
                baseline
                    .series
                    .into_iter()
                    .map(|series| OwnedChartSeries {
                        label: series.label,
                        values: series.values,
                        color: series.color.with_alpha(0.55),
                        fill_alpha: 0.0,
                        line_alpha: 0.58,
                    }),
            );
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
    current.footer = chart_anchor_footer(remote_training, Some(selected), compare)
        .or(current.footer.clone());
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
            overlay.series.extend(
                baseline
                    .series
                    .into_iter()
                    .map(|series| OwnedChartSeries {
                        label: series.label,
                        values: series.values,
                        color: series.color.with_alpha(0.55),
                        fill_alpha: 0.0,
                        line_alpha: 0.58,
                    }),
            );
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
    current.footer = chart_anchor_footer(remote_training, Some(selected), compare)
        .or(current.footer.clone());
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
            overlay.series.extend(
                baseline
                    .series
                    .into_iter()
                    .map(|series| OwnedChartSeries {
                        label: series.label,
                        values: series.values,
                        color: series.color.with_alpha(0.55),
                        fill_alpha: 0.0,
                        line_alpha: 0.58,
                    }),
            );
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
    current.footer = chart_anchor_footer(remote_training, Some(selected), compare)
        .or(current.footer.clone());
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
            overlay.series.extend(
                baseline
                    .series
                    .into_iter()
                    .map(|series| OwnedChartSeries {
                        label: series.label,
                        values: series.values,
                        color: series.color.with_alpha(0.55),
                        fill_alpha: 0.0,
                        line_alpha: 0.58,
                    }),
            );
            paint_chart_panel_view(bounds, accent, phase, &overlay, paint);
        } else if compare.mode != CompareNormalizationMode::PendingBaseline {
            let mut current = current;
            current.footer = Some(
                chart_anchor_footer(remote_training, Some(selected), Some(compare))
                    .unwrap_or_else(|| compare.caveat.clone()),
            );
            let mut baseline = hardware_chart_view(compare.baseline, "baseline");
            baseline.footer = Some(current_vs_baseline_footer(compare));
            paint_side_by_side_chart_views(bounds, accent, phase, &current, &baseline, paint);
        } else {
            let mut current = current;
            current.footer = chart_anchor_footer(remote_training, Some(selected), Some(compare))
                .or(current.footer.clone());
            paint_chart_panel_view(bounds, accent, phase, &current, paint);
        }
    } else {
        let mut current = current;
        current.footer = chart_anchor_footer(remote_training, Some(selected), compare)
            .or(current.footer.clone());
        paint_chart_panel_view(bounds, accent, phase, &current, paint);
    }

    let focus_targets = topology_focus_targets(selected);
    for (index, target) in focus_targets.iter().enumerate() {
        let target_bounds = psionic_remote_training_topology_target_bounds(content_bounds, index);
        let selected_focus =
            remote_training.focused_topology_target.as_deref() == Some(target.key.as_str());
        paint.scene.draw_quad(
            Quad::new(target_bounds)
                .with_background(
                    if selected_focus {
                        accent.with_alpha(0.18)
                    } else {
                        theme::bg::APP.with_alpha(0.88)
                    },
                )
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
        let label = truncate_line(paint, target.label.as_str(), target_bounds.size.width - 8.0, 9.0);
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
    let focus_detail = truncate_line(paint, focus_detail.as_str(), bounds.size.width - 32.0, 10.0);
    paint.scene.draw_text(paint.text.layout_mono(
        focus_detail.as_str(),
        Point::new(bounds.origin.x + 16.0, bounds.max_y() - 44.0),
        10.0,
        theme::text::PRIMARY,
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
                format!("{} // {}", compare.caveat, "event feed filtered by topology focus")
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
    paint: &mut PaintContext,
) {
    let load_state = load_state_for_status(remote_training);
    let mut y = paint_state_summary(
        paint,
        bounds.origin.x + 16.0,
        bounds.origin.y + 34.0,
        load_state,
        "Mirror health",
        remote_training.last_action.as_deref(),
        remote_training.last_error.as_deref(),
    );
    let Some(selected) = selected else {
        return;
    };
    let source_index = selected
        .source_index_path
        .as_deref()
        .unwrap_or("source index unavailable");
    let lines = [
        format!("sync: {}", remote_training.sync_state),
        format!("selected: {}", selected.run.run_id),
        format!("track: {}", selected.run.track.track_id),
        format!(
            "contract: {}",
            selected
                .run
                .contract_error
                .as_deref()
                .unwrap_or(selected.run.contract_state.as_str())
        ),
        format!(
            "digest: {}",
            selected.run.bundle_digest.as_deref().unwrap_or("-")
        ),
        format!(
            "score law: {}",
            selected.run.track.score_law_ref.as_deref().unwrap_or("-")
        ),
        format!("index: {}", source_index),
    ];
    for line in lines {
        let line = truncate_line(paint, line.as_str(), bounds.size.width - 32.0, 10.0);
        paint.scene.draw_text(paint.text.layout_mono(
            line.as_str(),
            Point::new(bounds.origin.x + 16.0, y),
            10.0,
            theme::text::PRIMARY,
        ));
        y += 14.0;
    }

    if let Some(bundle) = selected.bundle.as_ref() {
        for (index, artifact) in bundle.source_artifacts.iter().take(4).enumerate() {
            let row_bounds = Bounds::new(
                bounds.origin.x + 16.0,
                bounds.origin.y + 156.0 + index as f32 * 38.0,
                (bounds.size.width - 32.0).max(0.0),
                34.0,
            );
            let expanded = remote_training
                .selected_provenance_artifact_role
                .as_deref()
                == Some(artifact.artifact_role.as_str());
            paint.scene.draw_quad(
                Quad::new(row_bounds)
                    .with_background(
                        if expanded {
                            remote_mint().with_alpha(0.16)
                        } else {
                            theme::bg::APP.with_alpha(0.72)
                        },
                    )
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
            let role = truncate_line(
                paint,
                format!(
                    "{} // {}",
                    artifact.artifact_role,
                    artifact_source_kind_label(artifact.source_kind)
                )
                .as_str(),
                row_bounds.size.width - 12.0,
                9.0,
            );
            let detail = truncate_line(
                paint,
                artifact.detail.as_str(),
                row_bounds.size.width - 12.0,
                9.0,
            );
            paint.scene.draw_text(paint.text.layout_mono(
                role.as_str(),
                Point::new(row_bounds.origin.x + 6.0, row_bounds.origin.y + 6.0),
                9.0,
                if expanded {
                    remote_mint()
                } else {
                    theme::text::PRIMARY
                },
            ));
            paint.scene.draw_text(paint.text.layout(
                detail.as_str(),
                Point::new(row_bounds.origin.x + 6.0, row_bounds.origin.y + 18.0),
                9.0,
                theme::text::MUTED,
            ));
        }
    }

    if let Some(artifact) = focused_provenance_artifact(
        selected,
        remote_training.selected_provenance_artifact_role.as_deref(),
    ) {
        let detail_bounds = Bounds::new(
            bounds.origin.x + 16.0,
            bounds.max_y() - 66.0,
            (bounds.size.width - 32.0).max(0.0),
            50.0,
        );
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
                "{} // authoritative={} // digest={}",
                artifact.artifact_role,
                artifact.authoritative,
                artifact.artifact_digest.as_deref().unwrap_or("-"),
            ),
            truncate_to_chars(artifact.artifact_uri.as_str(), 64),
            format!("receipts {receipts} // {}", artifact.detail),
        ]
        .into_iter()
        .enumerate()
        {
            paint.scene.draw_text(paint.text.layout_mono(
                line.as_str(),
                Point::new(
                    detail_bounds.origin.x + 8.0,
                    detail_bounds.origin.y + 12.0 + index as f32 * 12.0,
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

fn load_state_for_status(remote_training: &DesktopControlRemoteTrainingStatus) -> PaneLoadState {
    match remote_training.sync_state.as_str() {
        "ready" | "stale" => PaneLoadState::Ready,
        "error" => PaneLoadState::Error,
        _ => PaneLoadState::Loading,
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

fn latest_subsystems_label(bundle: Option<&RemoteTrainingVisualizationBundleV2>) -> String {
    bundle
        .and_then(|bundle| bundle.heartbeat_series.last())
        .map(|sample| sample.active_subsystems.join(", "))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "no live heartbeat retained".to_string())
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
        && selected.run.track.public_equivalence_class == baseline.run.track.public_equivalence_class
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
    match compare.map(|compare| compare.anchor_mode).unwrap_or(CompareAnchorMode::None) {
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

fn anchor_elapsed_ms_label(bundle: &RemoteTrainingVisualizationBundleV2, ratio: u16) -> Option<String> {
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
    let anchor_index = chart_anchor_ratio_milli.and_then(|ratio| anchor_index(bundle.event_series.len(), ratio));
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
        Bounds::new(bounds.origin.x, bounds.origin.y, bounds.size.width, half_height),
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
            CompareNormalizationMode::DirectScore | CompareNormalizationMode::PublicEquivalentScore => {
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
