use std::borrow::Cow;

use wgpui::viz::badge::{BadgeTone, tone_color as badge_color};
use wgpui::viz::chart::{HistoryChartSeries, paint_history_chart_body};
use wgpui::viz::feed::{EventFeedRow, paint_event_feed_body};
use wgpui::viz::panel;
use wgpui::viz::provenance::{ProvenanceTone, tone_color as provenance_color};
use wgpui::viz::theme as viz_theme;
use wgpui::viz::topology::{TopologyNodeState, node_state_color};
use wgpui::{Bounds, Hsla, PaintContext, Point, Quad, Scene, TextSystem, theme};

pub const DEFAULT_VIZ_PRIMITIVES_WIDTH: f64 = 1220.0;
pub const DEFAULT_VIZ_PRIMITIVES_HEIGHT: f64 = 860.0;

pub fn build_viz_primitives_demo(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    width: f32,
    height: f32,
    time: f32,
) {
    let mut cx = PaintContext::new(scene, text_system, 1.0);
    let root = Bounds::new(0.0, 0.0, width, height);
    cx.scene
        .draw_quad(Quad::new(root).with_background(theme::bg::APP));

    let left = Bounds::new(24.0, 24.0, width * 0.32 - 30.0, 236.0);
    let right = Bounds::new(
        left.max_x() + 18.0,
        24.0,
        width - left.max_x() - 42.0,
        406.0,
    );
    let bottom = Bounds::new(
        24.0,
        right.max_y() + 18.0,
        width - 48.0,
        height - right.max_y() - 42.0,
    );
    let phase = (time * 0.18).fract();

    panel::paint_shell(left, viz_theme::track::PGOLF, &mut cx);
    panel::paint_title(
        left,
        "TRAINING VIZ TOKENS",
        viz_theme::track::PGOLF,
        &mut cx,
    );
    panel::paint_texture(left, viz_theme::track::PGOLF, phase, &mut cx);
    paint_token_badges(left, &mut cx);

    panel::paint_shell(right, viz_theme::series::LOSS, &mut cx);
    panel::paint_title(right, "SCALAR CHART", viz_theme::series::LOSS, &mut cx);
    paint_history_chart_body(
        right,
        viz_theme::series::LOSS,
        phase,
        Some("pgolf.run.11l // loss, ema, selectivity"),
        Some("sampled at shared plot-column density"),
        "No scalar history available.",
        &[
            HistoryChartSeries {
                label: "loss",
                values: &[2.8, 2.4, 2.1, 1.8, 1.56, 1.33, 1.21, 1.12, 1.04, 0.98],
                color: viz_theme::series::LOSS,
                fill_alpha: 0.18,
                line_alpha: 0.74,
            },
            HistoryChartSeries {
                label: "ema",
                values: &[2.7, 2.48, 2.26, 1.98, 1.72, 1.49, 1.32, 1.2, 1.11, 1.04],
                color: viz_theme::series::PROVENANCE,
                fill_alpha: 0.12,
                line_alpha: 0.86,
            },
            HistoryChartSeries {
                label: "selectivity",
                values: &[0.22, 0.24, 0.31, 0.36, 0.4, 0.47, 0.51, 0.56, 0.61, 0.66],
                color: viz_theme::series::HARDWARE,
                fill_alpha: 0.0,
                line_alpha: 0.92,
            },
        ],
        &mut cx,
    );

    panel::paint_shell(bottom, viz_theme::series::EVENTS, &mut cx);
    panel::paint_title(bottom, "EVENT RAIL", viz_theme::series::EVENTS, &mut cx);
    paint_event_feed_body(
        bottom,
        viz_theme::series::EVENTS,
        phase,
        "No events recorded.",
        &[
            EventFeedRow {
                label: Cow::Borrowed("score_closeout"),
                detail: Cow::Borrowed(
                    "Detached closeout receipt retained and linked into retained evidence.",
                ),
                color: provenance_color(ProvenanceTone::Evidence),
            },
            EventFeedRow {
                label: Cow::Borrowed("promotion_gate"),
                detail: Cow::Borrowed(
                    "Topology verdict remained warning while cluster drift stayed above threshold.",
                ),
                color: node_state_color(TopologyNodeState::Warning),
            },
            EventFeedRow {
                label: Cow::Borrowed("cache_refresh"),
                detail: Cow::Borrowed(
                    "Viewer fell back to cached bundle while live heartbeat aged past the freshness target.",
                ),
                color: provenance_color(ProvenanceTone::Cached),
            },
            EventFeedRow {
                label: Cow::Borrowed("lane_ready"),
                detail: Cow::Borrowed(
                    "Bounded XTRAIN handoff produced a comparable train_to_infer proof surface.",
                ),
                color: badge_color(BadgeTone::TrackXtrain),
            },
        ],
        &mut cx,
    );
}

fn paint_token_badges(bounds: Bounds, cx: &mut PaintContext) {
    let intro = [
        "Shared tokens live in wgpui::viz::theme.",
        "Charts bind to series tokens.",
        "Badges bind to state and track tokens.",
        "Panels bind to surface tokens.",
    ];
    let mut y = bounds.origin.y + 40.0;
    for line in intro {
        cx.scene.draw_text(cx.text.layout(
            line,
            Point::new(bounds.origin.x + 16.0, y),
            11.0,
            theme::text::PRIMARY,
        ));
        y += 18.0;
    }

    let badges = [
        ("PGOLF", badge_color(BadgeTone::TrackPgolf)),
        ("HOMEGOLF", badge_color(BadgeTone::TrackHomegolf)),
        ("XTRAIN", badge_color(BadgeTone::TrackXtrain)),
        ("LIVE", badge_color(BadgeTone::Live)),
        ("WARNING", badge_color(BadgeTone::Warning)),
        ("ERROR", badge_color(BadgeTone::Error)),
    ];

    let mut x = bounds.origin.x + 16.0;
    let badge_y = bounds.origin.y + 130.0;
    for (label, color) in badges {
        draw_badge(Bounds::new(x, badge_y, 94.0, 26.0), label, color, cx);
        x += 102.0;
        if x + 94.0 > bounds.max_x() - 12.0 {
            x = bounds.origin.x + 16.0;
        }
    }
}

fn draw_badge(bounds: Bounds, label: &str, color: Hsla, cx: &mut PaintContext) {
    cx.scene.draw_quad(
        Quad::new(bounds)
            .with_background(color.with_alpha(0.12))
            .with_border(color.with_alpha(0.42), 1.0)
            .with_corner_radius(6.0),
    );
    cx.scene.draw_text(cx.text.layout_mono(
        label,
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 7.0),
        10.0,
        color.with_alpha(0.94),
    ));
}
