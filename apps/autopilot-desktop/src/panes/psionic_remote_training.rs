use std::borrow::Cow;
use std::time::{SystemTime, UNIX_EPOCH};

use psionic_train::{
    RemoteTrainingEventSeverity, RemoteTrainingGpuSample, RemoteTrainingVisualizationBundleV2,
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
    psionic_remote_training_layout, psionic_remote_training_refresh_button_bounds,
    psionic_remote_training_run_row_bounds,
};
const CARD_GAP: f32 = 8.0;

pub fn paint(
    content_bounds: Bounds,
    remote_training: &DesktopControlRemoteTrainingStatus,
    paint: &mut PaintContext,
) {
    let layout = psionic_remote_training_layout(content_bounds);
    let selected = remote_training.selected_run.as_ref();
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

    let cards = status_cards(remote_training, selected);
    let card_width = ((layout.status_row.size.width - CARD_GAP * 3.0) / 4.0).max(120.0);
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
        selected_summary_line(remote_training, selected).as_str(),
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
    paint_run_detail_panel(layout.hero_panel, remote_training, selected, accent, paint);
    paint_loss_panel(layout.loss_panel, selected, phase, paint);
    paint_math_panel(layout.math_panel, selected, phase, paint);
    paint_runtime_panel(layout.runtime_panel, selected, phase, paint);
    paint_hardware_panel(layout.hardware_panel, selected, phase, paint);
    paint_event_panel(layout.events_panel, selected, phase, paint);
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
    selected: Option<&DesktopControlRemoteTrainingSelectedRunStatus>,
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
    let Some(bundle) = selected.bundle.as_ref() else {
        paint_history_chart_body(
            bounds,
            accent,
            phase,
            Some(selected.run.run_id.as_str()),
            Some(selected.run.semantic_summary.as_str()),
            selected
                .run
                .series_unavailable_reason
                .as_deref()
                .unwrap_or("No retained loss bundle is cached for this run."),
            &[],
            paint,
        );
        return;
    };

    let train = bundle
        .loss_series
        .iter()
        .filter_map(|sample| sample.train_loss)
        .collect::<Vec<_>>();
    let ema = bundle
        .loss_series
        .iter()
        .filter_map(|sample| sample.ema_loss)
        .collect::<Vec<_>>();
    let validation = bundle
        .loss_series
        .iter()
        .filter_map(|sample| sample.validation_loss)
        .collect::<Vec<_>>();
    let header = format!(
        "train {}  //  ema {}  //  val {}",
        format_optional_f32(bundle.summary.latest_train_loss),
        format_optional_f32(bundle.summary.latest_ema_loss),
        format_optional_f32(bundle.summary.latest_validation_loss),
    );
    let footer = format!(
        "steps {}  //  global {}  //  {}",
        bundle.summary.total_steps_completed,
        bundle
            .summary
            .latest_global_step
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string()),
        bundle
            .series_unavailable_reason
            .as_deref()
            .unwrap_or("full loss series retained")
    );
    paint_history_chart_body(
        bounds,
        accent,
        phase,
        Some(header.as_str()),
        Some(footer.as_str()),
        bundle
            .series_unavailable_reason
            .as_deref()
            .unwrap_or("No loss samples were retained for this run."),
        &[
            HistoryChartSeries {
                label: "train",
                values: train.as_slice(),
                color: accent,
                fill_alpha: 0.11,
                line_alpha: 0.82,
            },
            HistoryChartSeries {
                label: "ema",
                values: ema.as_slice(),
                color: remote_gold(),
                fill_alpha: 0.0,
                line_alpha: 0.88,
            },
            HistoryChartSeries {
                label: "validation",
                values: validation.as_slice(),
                color: remote_mint(),
                fill_alpha: 0.0,
                line_alpha: 0.84,
            },
        ],
        paint,
    );
}

fn paint_math_panel(
    bounds: Bounds,
    selected: Option<&DesktopControlRemoteTrainingSelectedRunStatus>,
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
    let Some(bundle) = selected.bundle.as_ref() else {
        paint_history_chart_body(
            bounds,
            accent,
            phase,
            None,
            None,
            "No retained optimizer-math bundle is cached for this run.",
            &[],
            paint,
        );
        return;
    };
    let gradient = bundle
        .math_series
        .iter()
        .filter_map(|sample| sample.gradient_norm)
        .collect::<Vec<_>>();
    let update = bundle
        .math_series
        .iter()
        .filter_map(|sample| sample.update_norm)
        .collect::<Vec<_>>();
    let clip = bundle
        .math_series
        .iter()
        .filter_map(|sample| sample.clip_fraction)
        .collect::<Vec<_>>();
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
    let header = format!(
        "lr {}  //  grad {}  //  update {}",
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
    );
    let footer = format!(
        "param {}  //  loss_scale {}  //  non_finite {}  //  {}",
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
    );
    paint_history_chart_body(
        bounds,
        accent,
        phase,
        Some(header.as_str()),
        Some(footer.as_str()),
        "No retained optimizer-math samples were emitted for this run.",
        &[
            HistoryChartSeries {
                label: "grad",
                values: gradient.as_slice(),
                color: accent,
                fill_alpha: 0.10,
                line_alpha: 0.84,
            },
            HistoryChartSeries {
                label: "update",
                values: update.as_slice(),
                color: remote_blue(),
                fill_alpha: 0.0,
                line_alpha: 0.86,
            },
            HistoryChartSeries {
                label: "clip",
                values: clip.as_slice(),
                color: remote_gold(),
                fill_alpha: 0.0,
                line_alpha: 0.82,
            },
        ],
        paint,
    );
}

fn paint_runtime_panel(
    bounds: Bounds,
    selected: Option<&DesktopControlRemoteTrainingSelectedRunStatus>,
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
    let Some(bundle) = selected.bundle.as_ref() else {
        paint_history_chart_body(
            bounds,
            accent,
            phase,
            None,
            None,
            "No retained runtime bundle is cached for this run.",
            &[],
            paint,
        );
        return;
    };
    let forward = bundle
        .runtime_series
        .iter()
        .filter_map(|sample| sample.forward_ms.map(|value| value as f32))
        .collect::<Vec<_>>();
    let backward = bundle
        .runtime_series
        .iter()
        .filter_map(|sample| sample.backward_ms.map(|value| value as f32))
        .collect::<Vec<_>>();
    let optimizer = bundle
        .runtime_series
        .iter()
        .filter_map(|sample| sample.optimizer_ms.map(|value| value as f32))
        .collect::<Vec<_>>();
    let latest = bundle.runtime_series.last();
    let header = format!(
        "tok/s {}  //  samp/s {}",
        latest
            .and_then(|sample| sample.tokens_per_second)
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string()),
        latest
            .and_then(|sample| sample.samples_per_second_milli)
            .map(format_samples_per_second_milli)
            .unwrap_or_else(|| "-".to_string()),
    );
    let footer = format!(
        "wait {}  //  ckpt {}  //  eval {}",
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
    );
    paint_history_chart_body(
        bounds,
        accent,
        phase,
        Some(header.as_str()),
        Some(footer.as_str()),
        "No retained pipeline-timing samples were emitted for this run.",
        &[
            HistoryChartSeries {
                label: "forward",
                values: forward.as_slice(),
                color: accent,
                fill_alpha: 0.10,
                line_alpha: 0.86,
            },
            HistoryChartSeries {
                label: "backward",
                values: backward.as_slice(),
                color: remote_coral(),
                fill_alpha: 0.0,
                line_alpha: 0.86,
            },
            HistoryChartSeries {
                label: "optim",
                values: optimizer.as_slice(),
                color: remote_mint(),
                fill_alpha: 0.0,
                line_alpha: 0.82,
            },
        ],
        paint,
    );
}

fn paint_hardware_panel(
    bounds: Bounds,
    selected: Option<&DesktopControlRemoteTrainingSelectedRunStatus>,
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
    let Some(bundle) = selected.bundle.as_ref() else {
        paint_history_chart_body(
            bounds,
            accent,
            phase,
            None,
            None,
            "No retained device bundle is cached for this run.",
            &[],
            paint,
        );
        return;
    };
    let utilization =
        aggregated_gpu_percent(bundle, |sample| sample.utilization_bps as f32 / 100.0);
    let memory = aggregated_gpu_percent(bundle, |sample| {
        sample.memory_used_bytes as f32 / sample.memory_total_bytes as f32 * 100.0
    });
    let latest_devices = latest_gpu_samples(bundle);
    let distributed = bundle.distributed_series.last();
    let header = format!(
        "devices {}  //  latest {}",
        latest_devices.len(),
        latest_devices
            .first()
            .map(|sample| sample.device_label.as_str())
            .unwrap_or("gpu unavailable"),
    );
    let footer = if let Some(sample) = distributed {
        format!(
            "ranks {}  //  skew {}  //  collective {}  //  stalls {}",
            sample.participating_rank_count,
            sample
                .rank_skew_ms
                .map(format_duration_ms)
                .unwrap_or_else(|| "-".to_string()),
            sample
                .collective_ms
                .map(format_duration_ms)
                .unwrap_or_else(|| "-".to_string()),
            sample.stalled_rank_count
        )
    } else {
        "no distributed telemetry retained for this lane".to_string()
    };
    paint_history_chart_body(
        bounds,
        accent,
        phase,
        Some(header.as_str()),
        Some(footer.as_str()),
        "No retained GPU samples were emitted for this run.",
        &[
            HistoryChartSeries {
                label: "util%",
                values: utilization.as_slice(),
                color: accent,
                fill_alpha: 0.10,
                line_alpha: 0.86,
            },
            HistoryChartSeries {
                label: "mem%",
                values: memory.as_slice(),
                color: remote_blue(),
                fill_alpha: 0.0,
                line_alpha: 0.84,
            },
        ],
        paint,
    );

    let info_y = bounds.max_y() - 44.0;
    if let Some(device) = latest_devices.first() {
        let device_line = format!(
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
        );
        let device_line =
            truncate_line(paint, device_line.as_str(), bounds.size.width - 32.0, 10.0);
        paint.scene.draw_text(paint.text.layout_mono(
            device_line.as_str(),
            Point::new(bounds.origin.x + 16.0, info_y),
            10.0,
            theme::text::PRIMARY,
        ));
    }
}

fn paint_event_panel(
    bounds: Bounds,
    selected: Option<&DesktopControlRemoteTrainingSelectedRunStatus>,
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
        .map(build_event_rows)
        .unwrap_or_default();
    let empty = selected
        .run
        .series_unavailable_reason
        .as_deref()
        .unwrap_or("No events were retained for this run.");
    paint_event_feed_body(bounds, remote_coral(), phase, empty, rows.as_slice(), paint);
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
        for artifact in bundle.source_artifacts.iter().take(4) {
            let lines = split_text_for_display(
                format!("{} // {}", artifact.artifact_role, artifact.detail).as_str(),
                46,
            );
            let artifact_uri = truncate_line(
                paint,
                artifact.artifact_uri.as_str(),
                bounds.size.width - 32.0,
                9.0,
            );
            paint.scene.draw_text(paint.text.layout_mono(
                artifact_uri.as_str(),
                Point::new(bounds.origin.x + 16.0, y),
                9.0,
                remote_mint(),
            ));
            y += 12.0;
            for line in lines.iter().take(2) {
                paint.scene.draw_text(paint.text.layout(
                    line.as_str(),
                    Point::new(bounds.origin.x + 16.0, y),
                    10.0,
                    theme::text::PRIMARY,
                ));
                y += 12.0;
            }
            y += 4.0;
            if y > bounds.max_y() - 24.0 {
                break;
            }
        }
    }
}

fn status_cards(
    remote_training: &DesktopControlRemoteTrainingStatus,
    selected: Option<&DesktopControlRemoteTrainingSelectedRunStatus>,
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
    ]
}

fn selected_summary_line(
    remote_training: &DesktopControlRemoteTrainingStatus,
    selected: Option<&DesktopControlRemoteTrainingSelectedRunStatus>,
) -> String {
    if let Some(selected) = selected {
        let checkpoint = selected
            .bundle
            .as_ref()
            .and_then(|bundle| bundle.summary.latest_checkpoint_ref.as_deref())
            .unwrap_or("checkpoint unavailable");
        format!(
            "{} // {} // {} // {}",
            primary_score_brief_or_track(&selected.run),
            compact_label(selected.run.track.proof_posture.as_str()),
            freshness_label(&selected.run),
            truncate_to_chars(checkpoint, 42)
        )
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

fn build_event_rows(bundle: &RemoteTrainingVisualizationBundleV2) -> Vec<EventFeedRow<'_>> {
    bundle
        .event_series
        .iter()
        .rev()
        .take(6)
        .enumerate()
        .map(|(index, event)| EventFeedRow {
            label: Cow::Owned(format!("E{:02}", index + 1)),
            detail: Cow::Owned(format!("{} // {}", event.event_kind, event.detail)),
            color: event_color(event.severity),
        })
        .collect()
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
