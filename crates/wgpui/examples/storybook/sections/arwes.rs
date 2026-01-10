use std::time::Duration;

use wgpui::components::hud::{
    CornerConfig, DotShape, DotsGrid, DotsOrigin, DrawDirection, Frame, FrameAnimation, FrameStyle,
    GridLinesBackground, LineDirection, MovingLinesBackground, PuffsBackground,
};
use wgpui::{
    AnimatorState, Bounds, Hsla, Illuminator, PaintContext, Point, TextDecipher,
    TextEffectTiming, TextSequence, theme,
};

use crate::constants::{
    BG_TILE_GAP, BG_TILE_H, BG_TILE_W, DOT_ORIGINS, DOT_SHAPES, FRAME_ANIMATIONS, FRAME_DIRECTIONS,
    FRAME_STYLES, FRAME_TILE_GAP, FRAME_TILE_H, FRAME_TILE_W, FRAME_VARIANT_H, FRAME_VARIANT_W,
    GLOW_PRESETS, ILLUMINATOR_TILE_GAP, ILLUMINATOR_TILE_H, ILLUMINATOR_TILE_W, LINE_DIRECTIONS,
    PANEL_PADDING, SECTION_GAP, TEXT_TILE_GAP, TEXT_TILE_H, TEXT_TILE_W,
};
use crate::helpers::{demo_frame, draw_panel, draw_tile, grid_metrics, inset_bounds, panel_height};
use crate::state::Storybook;

impl Storybook {
    pub(crate) fn paint_arwes_frames(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mut y = bounds.origin.y;
        let width = bounds.size.width;
        let available = (width - PANEL_PADDING * 2.0).max(0.0);

        let permutations = FRAME_STYLES.len() * FRAME_ANIMATIONS.len() * FRAME_DIRECTIONS.len();
        let grid = grid_metrics(
            available,
            permutations,
            FRAME_TILE_W,
            FRAME_TILE_H,
            FRAME_TILE_GAP,
        );
        let permutation_height = panel_height(grid.height);
        let panel_bounds = Bounds::new(bounds.origin.x, y, width, permutation_height);
        draw_panel(
            "Frame permutations (style x animation x direction)",
            panel_bounds,
            cx,
            |inner, cx| {
                let grid = grid_metrics(
                    inner.size.width,
                    permutations,
                    FRAME_TILE_W,
                    FRAME_TILE_H,
                    FRAME_TILE_GAP,
                );
                let mut idx = 0;
                for style in FRAME_STYLES.iter().copied() {
                    for animation in FRAME_ANIMATIONS.iter().copied() {
                        for direction in FRAME_DIRECTIONS.iter().copied() {
                            let row = idx / grid.cols;
                            let col = idx % grid.cols;
                            let tile_bounds = Bounds::new(
                                inner.origin.x + col as f32 * (FRAME_TILE_W + FRAME_TILE_GAP),
                                inner.origin.y + row as f32 * (FRAME_TILE_H + FRAME_TILE_GAP),
                                FRAME_TILE_W,
                                FRAME_TILE_H,
                            );
                            let label = format!(
                                "{} {} {}",
                                frame_style_label(style),
                                frame_animation_label(animation),
                                draw_direction_label(direction)
                            );
                            draw_tile(tile_bounds, &label, cx, |inner, cx| {
                                let progress = match animation {
                                    FrameAnimation::Fade => 1.0,
                                    FrameAnimation::Flicker => 0.6,
                                    FrameAnimation::Draw | FrameAnimation::Assemble => 0.65,
                                };
                                let mut frame = Frame::new()
                                    .style(style)
                                    .animation_mode(animation)
                                    .draw_direction(direction)
                                    .animation_progress(progress);
                                if animation == FrameAnimation::Flicker {
                                    frame = frame.is_exiting(false);
                                }
                                frame.paint(inset_bounds(inner, 4.0), cx);
                            });
                            idx += 1;
                        }
                    }
                }
            },
        );
        y += permutation_height + SECTION_GAP;

        let flicker_count = FRAME_STYLES.len() * 2;
        let flicker_grid = grid_metrics(
            available,
            flicker_count,
            FRAME_VARIANT_W,
            FRAME_VARIANT_H,
            FRAME_TILE_GAP,
        );
        let flicker_height = panel_height(flicker_grid.height);
        let flicker_bounds = Bounds::new(bounds.origin.x, y, width, flicker_height);
        draw_panel(
            "Flicker state (enter vs exit)",
            flicker_bounds,
            cx,
            |inner, cx| {
                let grid = grid_metrics(
                    inner.size.width,
                    flicker_count,
                    FRAME_VARIANT_W,
                    FRAME_VARIANT_H,
                    FRAME_TILE_GAP,
                );
                let mut idx = 0;
                for style in FRAME_STYLES.iter().copied() {
                    for exiting in [false, true] {
                        let row = idx / grid.cols;
                        let col = idx % grid.cols;
                        let tile_bounds = Bounds::new(
                            inner.origin.x + col as f32 * (FRAME_VARIANT_W + FRAME_TILE_GAP),
                            inner.origin.y + row as f32 * (FRAME_VARIANT_H + FRAME_TILE_GAP),
                            FRAME_VARIANT_W,
                            FRAME_VARIANT_H,
                        );
                        let label = format!(
                            "{} {}",
                            frame_style_label(style),
                            if exiting { "Exit" } else { "Enter" }
                        );
                        draw_tile(tile_bounds, &label, cx, |inner, cx| {
                            let mut frame = Frame::new()
                                .style(style)
                                .animation_mode(FrameAnimation::Flicker)
                                .animation_progress(0.6)
                                .is_exiting(exiting);
                            frame.paint(inset_bounds(inner, 4.0), cx);
                        });
                        idx += 1;
                    }
                }
            },
        );
        y += flicker_height + SECTION_GAP;

        let glow_count = FRAME_STYLES.len() * 2;
        let glow_grid = grid_metrics(
            available,
            glow_count,
            FRAME_VARIANT_W,
            FRAME_VARIANT_H,
            FRAME_TILE_GAP,
        );
        let glow_height = panel_height(glow_grid.height);
        let glow_bounds = Bounds::new(bounds.origin.x, y, width, glow_height);
        draw_panel("Glow toggle (off/on)", glow_bounds, cx, |inner, cx| {
            let grid = grid_metrics(
                inner.size.width,
                glow_count,
                FRAME_VARIANT_W,
                FRAME_VARIANT_H,
                FRAME_TILE_GAP,
            );
            let mut idx = 0;
            for style in FRAME_STYLES.iter().copied() {
                for glow in [false, true] {
                    let row = idx / grid.cols;
                    let col = idx % grid.cols;
                    let tile_bounds = Bounds::new(
                        inner.origin.x + col as f32 * (FRAME_VARIANT_W + FRAME_TILE_GAP),
                        inner.origin.y + row as f32 * (FRAME_VARIANT_H + FRAME_TILE_GAP),
                        FRAME_VARIANT_W,
                        FRAME_VARIANT_H,
                    );
                    let label = format!(
                        "{} {}",
                        frame_style_label(style),
                        if glow { "Glow" } else { "NoGlow" }
                    );
                    draw_tile(tile_bounds, &label, cx, |inner, cx| {
                        let mut frame = Frame::new()
                            .style(style)
                            .animation_mode(FrameAnimation::Fade)
                            .animation_progress(1.0);
                        if glow {
                            frame = frame.glow_color(theme::accent::PRIMARY);
                        }
                        frame.paint(inset_bounds(inner, 4.0), cx);
                    });
                    idx += 1;
                }
            }
        });
        y += glow_height + SECTION_GAP;

        let glow_palette_count = FRAME_STYLES.len() * FRAME_ANIMATIONS.len() * GLOW_PRESETS.len();
        let glow_palette_grid = grid_metrics(
            available,
            glow_palette_count,
            FRAME_VARIANT_W,
            FRAME_VARIANT_H,
            FRAME_TILE_GAP,
        );
        let glow_palette_height = panel_height(glow_palette_grid.height);
        let glow_palette_bounds = Bounds::new(bounds.origin.x, y, width, glow_palette_height);
        draw_panel(
            "Glow palette x animation",
            glow_palette_bounds,
            cx,
            |inner, cx| {
                let grid = grid_metrics(
                    inner.size.width,
                    glow_palette_count,
                    FRAME_VARIANT_W,
                    FRAME_VARIANT_H,
                    FRAME_TILE_GAP,
                );
                let progress = self.light_frame_anim.current_value();
                let glow_pulse = self.glow_pulse_anim.current_value();
                let flicker_exit = progress > 0.5;
                let white = Hsla::new(0.0, 0.0, 1.0, 1.0);
                let dark_bg = Hsla::new(0.0, 0.0, 0.08, 0.85);
                let mut idx = 0;

                for style in FRAME_STYLES.iter().copied() {
                    for animation in FRAME_ANIMATIONS.iter().copied() {
                        for preset in GLOW_PRESETS.iter().copied() {
                            let row = idx / grid.cols;
                            let col = idx % grid.cols;
                            let tile_bounds = Bounds::new(
                                inner.origin.x + col as f32 * (FRAME_VARIANT_W + FRAME_TILE_GAP),
                                inner.origin.y + row as f32 * (FRAME_VARIANT_H + FRAME_TILE_GAP),
                                FRAME_VARIANT_W,
                                FRAME_VARIANT_H,
                            );
                            let label = format!(
                                "{} {} {}",
                                frame_style_short(style),
                                frame_animation_label(animation),
                                preset.short
                            );
                            draw_tile(tile_bounds, &label, cx, |inner, cx| {
                                let mut frame = demo_frame(style)
                                    .line_color(white)
                                    .bg_color(dark_bg)
                                    .stroke_width(2.0)
                                    .animation_mode(animation)
                                    .draw_direction(DrawDirection::CenterOut)
                                    .animation_progress(progress);
                                if animation == FrameAnimation::Flicker {
                                    frame = frame.is_exiting(flicker_exit);
                                }
                                let glow = preset.color.with_alpha(preset.color.a * glow_pulse);
                                frame = frame.glow_color(glow);
                                frame.paint(inset_bounds(inner, 4.0), cx);
                            });
                            idx += 1;
                        }
                    }
                }
            },
        );
        y += glow_palette_height + SECTION_GAP;

        let nefrex_count = 16;
        let nefrex_grid = grid_metrics(
            available,
            nefrex_count,
            FRAME_VARIANT_W,
            FRAME_VARIANT_H,
            FRAME_TILE_GAP,
        );
        let nefrex_height = panel_height(nefrex_grid.height);
        let nefrex_bounds = Bounds::new(bounds.origin.x, y, width, nefrex_height);
        draw_panel(
            "Nefrex corners (LT LB RT RB order)",
            nefrex_bounds,
            cx,
            |inner, cx| {
                let grid = grid_metrics(
                    inner.size.width,
                    nefrex_count,
                    FRAME_VARIANT_W,
                    FRAME_VARIANT_H,
                    FRAME_TILE_GAP,
                );
                for mask in 0..16 {
                    let row = mask / grid.cols;
                    let col = mask % grid.cols;
                    let tile_bounds = Bounds::new(
                        inner.origin.x + col as f32 * (FRAME_VARIANT_W + FRAME_TILE_GAP),
                        inner.origin.y + row as f32 * (FRAME_VARIANT_H + FRAME_TILE_GAP),
                        FRAME_VARIANT_W,
                        FRAME_VARIANT_H,
                    );
                    let config = CornerConfig {
                        left_top: mask & 1 != 0,
                        left_bottom: mask & 2 != 0,
                        right_top: mask & 4 != 0,
                        right_bottom: mask & 8 != 0,
                    };
                    let label = format!("{:04b}", mask);
                    draw_tile(tile_bounds, &label, cx, |inner, cx| {
                        let mut frame = Frame::new()
                            .style(FrameStyle::Nefrex)
                            .corner_config(config)
                            .animation_progress(1.0);
                        frame.paint(inset_bounds(inner, 4.0), cx);
                    });
                }
            },
        );
        y += nefrex_height + SECTION_GAP;

        let header_count = 2;
        let header_grid = grid_metrics(
            available,
            header_count,
            FRAME_VARIANT_W,
            FRAME_VARIANT_H,
            FRAME_TILE_GAP,
        );
        let header_height = panel_height(header_grid.height);
        let header_bounds = Bounds::new(bounds.origin.x, y, width, header_height);
        draw_panel("Header bottom toggle", header_bounds, cx, |inner, cx| {
            let grid = grid_metrics(
                inner.size.width,
                header_count,
                FRAME_VARIANT_W,
                FRAME_VARIANT_H,
                FRAME_TILE_GAP,
            );
            for (idx, bottom) in [false, true].iter().enumerate() {
                let row = idx / grid.cols;
                let col = idx % grid.cols;
                let tile_bounds = Bounds::new(
                    inner.origin.x + col as f32 * (FRAME_VARIANT_W + FRAME_TILE_GAP),
                    inner.origin.y + row as f32 * (FRAME_VARIANT_H + FRAME_TILE_GAP),
                    FRAME_VARIANT_W,
                    FRAME_VARIANT_H,
                );
                let label = if *bottom { "Bottom" } else { "Top" };
                draw_tile(tile_bounds, label, cx, |inner, cx| {
                    let mut frame = Frame::new()
                        .style(FrameStyle::Header)
                        .header_bottom(*bottom)
                        .animation_progress(1.0);
                    frame.paint(inset_bounds(inner, 4.0), cx);
                });
            }
        });
        y += header_height + SECTION_GAP;

        let circle_segments = [8u32, 16, 32, 64];
        let circle_count = circle_segments.len();
        let circle_grid = grid_metrics(
            available,
            circle_count,
            FRAME_VARIANT_W,
            FRAME_VARIANT_H,
            FRAME_TILE_GAP,
        );
        let circle_height = panel_height(circle_grid.height);
        let circle_bounds = Bounds::new(bounds.origin.x, y, width, circle_height);
        draw_panel("Circle segments", circle_bounds, cx, |inner, cx| {
            let grid = grid_metrics(
                inner.size.width,
                circle_count,
                FRAME_VARIANT_W,
                FRAME_VARIANT_H,
                FRAME_TILE_GAP,
            );
            for (idx, segments) in circle_segments.iter().enumerate() {
                let row = idx / grid.cols;
                let col = idx % grid.cols;
                let tile_bounds = Bounds::new(
                    inner.origin.x + col as f32 * (FRAME_VARIANT_W + FRAME_TILE_GAP),
                    inner.origin.y + row as f32 * (FRAME_VARIANT_H + FRAME_TILE_GAP),
                    FRAME_VARIANT_W,
                    FRAME_VARIANT_H,
                );
                let label = format!("{segments} seg");
                draw_tile(tile_bounds, &label, cx, |inner, cx| {
                    let mut frame = Frame::new()
                        .style(FrameStyle::Circle)
                        .circle_segments(*segments)
                        .animation_progress(1.0);
                    frame.paint(inset_bounds(inner, 4.0), cx);
                });
            }
        });
    }

    pub(crate) fn paint_arwes_backgrounds(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mut y = bounds.origin.y;
        let width = bounds.size.width;
        let available = (width - PANEL_PADDING * 2.0).max(0.0);

        let dots_count = DOT_SHAPES.len() * DOT_ORIGINS.len() * 2;
        let dots_grid = grid_metrics(available, dots_count, BG_TILE_W, BG_TILE_H, BG_TILE_GAP);
        let dots_height = panel_height(dots_grid.height);
        let dots_bounds = Bounds::new(bounds.origin.x, y, width, dots_height);
        draw_panel(
            "DotsGrid permutations (shape x origin x invert)",
            dots_bounds,
            cx,
            |inner, cx| {
                let grid = grid_metrics(
                    inner.size.width,
                    dots_count,
                    BG_TILE_W,
                    BG_TILE_H,
                    BG_TILE_GAP,
                );
                let mut idx = 0;
                for shape in DOT_SHAPES.iter().copied() {
                    for origin in DOT_ORIGINS.iter().copied() {
                        for inverted in [false, true] {
                            let row = idx / grid.cols;
                            let col = idx % grid.cols;
                            let tile_bounds = Bounds::new(
                                inner.origin.x + col as f32 * (BG_TILE_W + BG_TILE_GAP),
                                inner.origin.y + row as f32 * (BG_TILE_H + BG_TILE_GAP),
                                BG_TILE_W,
                                BG_TILE_H,
                            );
                            let label = format!(
                                "{} {} {}",
                                dot_shape_label(shape),
                                dots_origin_label(origin),
                                if inverted { "Inv" } else { "Norm" }
                            );
                            draw_tile(tile_bounds, &label, cx, |inner, cx| {
                                let mut grid = DotsGrid::new()
                                    .shape(shape)
                                    .origin(origin)
                                    .origin_inverted(inverted)
                                    .distance(20.0)
                                    .size(4.0)
                                    .opacity(1.0)
                                    .color(Hsla::new(180.0, 0.8, 0.5, 0.9));
                                grid.paint(inner, cx);
                            });
                            idx += 1;
                        }
                    }
                }
            },
        );
        y += dots_height + SECTION_GAP;

        let dots_states = [0.0f32, 0.35, 0.7, 1.0];
        let dots_state_count = dots_states.len();
        let dots_state_grid = grid_metrics(
            available,
            dots_state_count,
            BG_TILE_W,
            BG_TILE_H,
            BG_TILE_GAP,
        );
        let dots_state_height = panel_height(dots_state_grid.height);
        let dots_state_bounds = Bounds::new(bounds.origin.x, y, width, dots_state_height);
        draw_panel(
            "DotsGrid progress states",
            dots_state_bounds,
            cx,
            |inner, cx| {
                let grid = grid_metrics(
                    inner.size.width,
                    dots_state_count,
                    BG_TILE_W,
                    BG_TILE_H,
                    BG_TILE_GAP,
                );
                for (idx, progress) in dots_states.iter().enumerate() {
                    let row = idx / grid.cols;
                    let col = idx % grid.cols;
                    let tile_bounds = Bounds::new(
                        inner.origin.x + col as f32 * (BG_TILE_W + BG_TILE_GAP),
                        inner.origin.y + row as f32 * (BG_TILE_H + BG_TILE_GAP),
                        BG_TILE_W,
                        BG_TILE_H,
                    );
                    let label = format!("{}%", (progress * 100.0) as i32);
                    draw_tile(tile_bounds, &label, cx, |inner, cx| {
                        let mut grid = DotsGrid::new()
                            .shape(DotShape::Box)
                            .origin(DotsOrigin::Center)
                            .distance(18.0)
                            .size(5.0)
                            .opacity(1.0)
                            .animation_progress(*progress)
                            .color(Hsla::new(280.0, 0.9, 0.6, 0.95));
                        grid.paint(inner, cx);
                    });
                }
            },
        );
        y += dots_state_height + SECTION_GAP;

        let grid_lines_count = 8;
        let grid_lines_grid = grid_metrics(
            available,
            grid_lines_count,
            BG_TILE_W,
            BG_TILE_H,
            BG_TILE_GAP,
        );
        let grid_lines_height = panel_height(grid_lines_grid.height);
        let grid_lines_bounds = Bounds::new(bounds.origin.x, y, width, grid_lines_height);
        draw_panel(
            "GridLines permutations (orientation x dash)",
            grid_lines_bounds,
            cx,
            |inner, cx| {
                let grid = grid_metrics(
                    inner.size.width,
                    grid_lines_count,
                    BG_TILE_W,
                    BG_TILE_H,
                    BG_TILE_GAP,
                );
                let orientations = [
                    (true, true, "HV"),
                    (true, false, "H"),
                    (false, true, "V"),
                    (false, false, "None"),
                ];
                let dashes = [(Vec::new(), "Solid"), (vec![6.0, 4.0], "Dash")];
                let mut idx = 0;
                for (h, v, label) in orientations {
                    for (dash, dash_label) in dashes.iter() {
                        let row = idx / grid.cols;
                        let col = idx % grid.cols;
                        let tile_bounds = Bounds::new(
                            inner.origin.x + col as f32 * (BG_TILE_W + BG_TILE_GAP),
                            inner.origin.y + row as f32 * (BG_TILE_H + BG_TILE_GAP),
                            BG_TILE_W,
                            BG_TILE_H,
                        );
                        let label = format!("{label} {dash_label}");
                        draw_tile(tile_bounds, &label, cx, |inner, cx| {
                            let mut grid = GridLinesBackground::new()
                                .horizontal(h)
                                .vertical(v)
                                .spacing(24.0)
                                .line_width(2.0)
                                .color(Hsla::new(120.0, 0.7, 0.5, 0.8))
                                .horizontal_dash(dash.clone())
                                .vertical_dash(dash.clone());
                            grid.set_state(AnimatorState::Entered);
                            grid.paint(inner, cx);
                        });
                        idx += 1;
                    }
                }
            },
        );
        y += grid_lines_height + SECTION_GAP;

        let moving_count = LINE_DIRECTIONS.len() * 2;
        let moving_grid = grid_metrics(available, moving_count, BG_TILE_W, BG_TILE_H, BG_TILE_GAP);
        let moving_height = panel_height(moving_grid.height);
        let moving_bounds = Bounds::new(bounds.origin.x, y, width, moving_height);
        draw_panel(
            "MovingLines permutations (direction x spacing)",
            moving_bounds,
            cx,
            |inner, cx| {
                let grid = grid_metrics(
                    inner.size.width,
                    moving_count,
                    BG_TILE_W,
                    BG_TILE_H,
                    BG_TILE_GAP,
                );
                let spacings = [30.0, 70.0];
                let mut idx = 0;
                for direction in LINE_DIRECTIONS.iter().copied() {
                    for spacing in spacings.iter().copied() {
                        let row = idx / grid.cols;
                        let col = idx % grid.cols;
                        let tile_bounds = Bounds::new(
                            inner.origin.x + col as f32 * (BG_TILE_W + BG_TILE_GAP),
                            inner.origin.y + row as f32 * (BG_TILE_H + BG_TILE_GAP),
                            BG_TILE_W,
                            BG_TILE_H,
                        );
                        let label =
                            format!("{} {}", line_direction_label(direction), spacing as i32);
                        draw_tile(tile_bounds, &label, cx, |inner, cx| {
                            let mut lines = MovingLinesBackground::new()
                                .direction(direction)
                                .spacing(spacing)
                                .line_width(2.5)
                                .color(Hsla::new(45.0, 0.9, 0.6, 0.85))
                                .sets(5)
                                .cycle_duration(Duration::from_secs(4));
                            lines.update_with_delta(
                                AnimatorState::Entered,
                                Duration::from_millis(600),
                            );
                            lines.paint(inner, cx);
                        });
                        idx += 1;
                    }
                }
            },
        );
        y += moving_height + SECTION_GAP;

        let puff_presets = 6;
        let puff_grid = grid_metrics(available, puff_presets, BG_TILE_W, BG_TILE_H, BG_TILE_GAP);
        let puff_height = panel_height(puff_grid.height);
        let puff_bounds = Bounds::new(bounds.origin.x, y, width, puff_height);
        draw_panel("Puffs permutations", puff_bounds, cx, |inner, cx| {
            let grid = grid_metrics(
                inner.size.width,
                puff_presets,
                BG_TILE_W,
                BG_TILE_H,
                BG_TILE_GAP,
            );
            let presets: Vec<(&str, PuffsBackground)> = vec![
                (
                    "Cyan",
                    PuffsBackground::new()
                        .color(Hsla::new(180.0, 0.9, 0.5, 0.4))
                        .quantity(12)
                        .layers(8),
                ),
                (
                    "Dense Magenta",
                    PuffsBackground::new()
                        .color(Hsla::new(300.0, 0.85, 0.5, 0.35))
                        .quantity(20)
                        .layers(14)
                        .radius_offset((8.0, 70.0)),
                ),
                (
                    "Sparse Blue",
                    PuffsBackground::new()
                        .color(Hsla::new(220.0, 0.8, 0.6, 0.45))
                        .quantity(6)
                        .layers(6)
                        .radius_offset((4.0, 30.0)),
                ),
                (
                    "Warm Orange",
                    PuffsBackground::new()
                        .color(Hsla::new(32.0, 0.95, 0.55, 0.4))
                        .quantity(14)
                        .layers(12),
                ),
                (
                    "Wide Green",
                    PuffsBackground::new()
                        .color(Hsla::new(140.0, 0.8, 0.45, 0.4))
                        .quantity(10)
                        .padding(60.0)
                        .radius_offset((8.0, 55.0)),
                ),
                (
                    "Offset Purple",
                    PuffsBackground::new()
                        .color(Hsla::new(270.0, 0.85, 0.55, 0.4))
                        .quantity(12)
                        .y_offset((-30.0, -100.0))
                        .x_offset((15.0, 50.0)),
                ),
            ];

            for (idx, (label, mut puffs)) in presets.into_iter().enumerate() {
                let row = idx / grid.cols;
                let col = idx % grid.cols;
                let tile_bounds = Bounds::new(
                    inner.origin.x + col as f32 * (BG_TILE_W + BG_TILE_GAP),
                    inner.origin.y + row as f32 * (BG_TILE_H + BG_TILE_GAP),
                    BG_TILE_W,
                    BG_TILE_H,
                );
                draw_tile(tile_bounds, label, cx, |inner, cx| {
                    puffs.update_with_delta(AnimatorState::Entered, Duration::from_millis(500));
                    puffs.paint(inner, cx);
                });
            }
        });
    }

    pub(crate) fn paint_arwes_text_effects(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mut y = bounds.origin.y;
        let width = bounds.size.width;
        let available = (width - PANEL_PADDING * 2.0).max(0.0);

        let sequence_presets = 8;
        let sequence_grid = grid_metrics(
            available,
            sequence_presets,
            TEXT_TILE_W,
            TEXT_TILE_H,
            TEXT_TILE_GAP,
        );
        let sequence_height = panel_height(sequence_grid.height);
        let sequence_bounds = Bounds::new(bounds.origin.x, y, width, sequence_height);
        draw_panel(
            "TextSequence permutations",
            sequence_bounds,
            cx,
            |inner, cx| {
                let grid = grid_metrics(
                    inner.size.width,
                    sequence_presets,
                    TEXT_TILE_W,
                    TEXT_TILE_H,
                    TEXT_TILE_GAP,
                );
                let mut items: Vec<(String, TextSequence)> = Vec::new();
                items.push((
                    "Normal cursor".to_string(),
                    TextSequence::new("Sequence reveal"),
                ));
                items.push((
                    "Cursor off".to_string(),
                    TextSequence::new("Sequence reveal").show_cursor(false),
                ));
                items.push((
                    "Bold".to_string(),
                    TextSequence::new("Sequence reveal").bold(),
                ));
                items.push((
                    "Italic".to_string(),
                    TextSequence::new("Sequence reveal").italic(),
                ));
                items.push((
                    "Bold Italic".to_string(),
                    TextSequence::new("Sequence reveal").bold_italic(),
                ));
                items.push((
                    "Cursor _".to_string(),
                    TextSequence::new("Sequence reveal").cursor_char('_'),
                ));
                let mut entering = TextSequence::new("Sequence reveal").timing(
                    TextEffectTiming::new(Duration::from_millis(900), Duration::from_millis(50)),
                );
                entering.update_with_delta(AnimatorState::Entering, Duration::from_millis(350));
                items.push(("Entering".to_string(), entering));
                let mut exiting = TextSequence::new("Sequence reveal");
                exiting.update_with_delta(AnimatorState::Exiting, Duration::from_millis(350));
                items.push(("Exiting".to_string(), exiting));

                for (idx, (label, mut seq)) in items.into_iter().enumerate() {
                    let row = idx / grid.cols;
                    let col = idx % grid.cols;
                    let tile_bounds = Bounds::new(
                        inner.origin.x + col as f32 * (TEXT_TILE_W + TEXT_TILE_GAP),
                        inner.origin.y + row as f32 * (TEXT_TILE_H + TEXT_TILE_GAP),
                        TEXT_TILE_W,
                        TEXT_TILE_H,
                    );
                    draw_tile(tile_bounds, &label, cx, |inner, cx| {
                        let text_bounds = Bounds::new(
                            inner.origin.x,
                            inner.origin.y + 8.0,
                            inner.size.width,
                            inner.size.height - 8.0,
                        );
                        seq.paint(text_bounds, cx);
                    });
                }
            },
        );
        y += sequence_height + SECTION_GAP;

        let decipher_presets = 6;
        let decipher_grid = grid_metrics(
            available,
            decipher_presets,
            TEXT_TILE_W,
            TEXT_TILE_H,
            TEXT_TILE_GAP,
        );
        let decipher_height = panel_height(decipher_grid.height);
        let decipher_bounds = Bounds::new(bounds.origin.x, y, width, decipher_height);
        draw_panel(
            "TextDecipher permutations",
            decipher_bounds,
            cx,
            |inner, cx| {
                let grid = grid_metrics(
                    inner.size.width,
                    decipher_presets,
                    TEXT_TILE_W,
                    TEXT_TILE_H,
                    TEXT_TILE_GAP,
                );
                let mut items: Vec<(String, TextDecipher)> = Vec::new();
                let mut default = TextDecipher::new("Decrypting payload");
                default.update_with_delta(AnimatorState::Entering, Duration::from_millis(350));
                items.push(("Default".to_string(), default));

                let mut digits = TextDecipher::new("Decrypting payload")
                    .characters("0123456789")
                    .scramble_interval(Duration::from_millis(40));
                digits.update_with_delta(AnimatorState::Entering, Duration::from_millis(350));
                items.push(("Digits".to_string(), digits));

                let mut binary = TextDecipher::new("Decrypting payload")
                    .characters("01")
                    .scramble_interval(Duration::from_millis(25));
                binary.update_with_delta(AnimatorState::Entering, Duration::from_millis(350));
                items.push(("Binary".to_string(), binary));

                let mut slow = TextDecipher::new("Decrypting payload")
                    .scramble_interval(Duration::from_millis(120));
                slow.update_with_delta(AnimatorState::Entering, Duration::from_millis(350));
                items.push(("Slow".to_string(), slow));

                let mut bold = TextDecipher::new("Decrypting payload").bold();
                bold.update_with_delta(AnimatorState::Entering, Duration::from_millis(350));
                items.push(("Bold".to_string(), bold));

                let mut exit = TextDecipher::new("Decrypting payload");
                exit.update_with_delta(AnimatorState::Exiting, Duration::from_millis(350));
                items.push(("Exiting".to_string(), exit));

                for (idx, (label, mut decipher)) in items.into_iter().enumerate() {
                    let row = idx / grid.cols;
                    let col = idx % grid.cols;
                    let tile_bounds = Bounds::new(
                        inner.origin.x + col as f32 * (TEXT_TILE_W + TEXT_TILE_GAP),
                        inner.origin.y + row as f32 * (TEXT_TILE_H + TEXT_TILE_GAP),
                        TEXT_TILE_W,
                        TEXT_TILE_H,
                    );
                    draw_tile(tile_bounds, &label, cx, |inner, cx| {
                        let text_bounds = Bounds::new(
                            inner.origin.x,
                            inner.origin.y + 8.0,
                            inner.size.width,
                            inner.size.height - 8.0,
                        );
                        decipher.paint(text_bounds, cx);
                    });
                }
            },
        );
    }

    pub(crate) fn paint_arwes_illuminator(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mut y = bounds.origin.y;
        let width = bounds.size.width;
        let available = (width - PANEL_PADDING * 2.0).max(0.0);

        let preset_count = 8;
        let preset_grid = grid_metrics(
            available,
            preset_count,
            ILLUMINATOR_TILE_W,
            ILLUMINATOR_TILE_H,
            ILLUMINATOR_TILE_GAP,
        );
        let preset_height = panel_height(preset_grid.height);
        let preset_bounds = Bounds::new(bounds.origin.x, y, width, preset_height);
        draw_panel("Illuminator presets", preset_bounds, cx, |inner, cx| {
            let grid = grid_metrics(
                inner.size.width,
                preset_count,
                ILLUMINATOR_TILE_W,
                ILLUMINATOR_TILE_H,
                ILLUMINATOR_TILE_GAP,
            );
            let presets: Vec<(&str, Illuminator)> = vec![
                ("Default", Illuminator::new()),
                (
                    "Small",
                    Illuminator::new()
                        .radius(40.0)
                        .intensity(0.7)
                        .color(Hsla::new(180.0, 0.6, 0.7, 0.25)),
                ),
                (
                    "Large",
                    Illuminator::new()
                        .radius(90.0)
                        .intensity(1.0)
                        .color(Hsla::new(200.0, 0.7, 0.7, 0.2)),
                ),
                (
                    "Warm",
                    Illuminator::new()
                        .radius(70.0)
                        .intensity(0.9)
                        .color(Hsla::new(25.0, 0.8, 0.6, 0.22)),
                ),
                (
                    "High Rings",
                    Illuminator::new()
                        .radius(70.0)
                        .rings(16)
                        .segments(64)
                        .intensity(0.8),
                ),
                (
                    "Low Rings",
                    Illuminator::new()
                        .radius(70.0)
                        .rings(6)
                        .segments(24)
                        .intensity(0.9),
                ),
                (
                    "Green",
                    Illuminator::new()
                        .radius(60.0)
                        .intensity(0.8)
                        .color(Hsla::new(120.0, 0.7, 0.6, 0.22)),
                ),
                (
                    "Blue",
                    Illuminator::new()
                        .radius(60.0)
                        .intensity(0.8)
                        .color(Hsla::new(210.0, 0.7, 0.6, 0.22)),
                ),
            ];

            for (idx, (label, mut illuminator)) in presets.into_iter().enumerate() {
                let row = idx / grid.cols;
                let col = idx % grid.cols;
                let tile_bounds = Bounds::new(
                    inner.origin.x + col as f32 * (ILLUMINATOR_TILE_W + ILLUMINATOR_TILE_GAP),
                    inner.origin.y + row as f32 * (ILLUMINATOR_TILE_H + ILLUMINATOR_TILE_GAP),
                    ILLUMINATOR_TILE_W,
                    ILLUMINATOR_TILE_H,
                );
                draw_tile(tile_bounds, label, cx, |inner, cx| {
                    let center = Point::new(
                        inner.origin.x + inner.size.width / 2.0,
                        inner.origin.y + inner.size.height / 2.0,
                    );
                    illuminator.snap_to_position(center.x, center.y);
                    illuminator.update_with_delta(AnimatorState::Entered, Duration::from_millis(1));
                    illuminator.paint(inner, cx);
                });
            }
        });
        y += preset_height + SECTION_GAP;

        let state_count = 4;
        let state_grid = grid_metrics(
            available,
            state_count,
            ILLUMINATOR_TILE_W,
            ILLUMINATOR_TILE_H,
            ILLUMINATOR_TILE_GAP,
        );
        let state_height = panel_height(state_grid.height);
        let state_bounds = Bounds::new(bounds.origin.x, y, width, state_height);
        draw_panel("Illuminator states", state_bounds, cx, |inner, cx| {
            let grid = grid_metrics(
                inner.size.width,
                state_count,
                ILLUMINATOR_TILE_W,
                ILLUMINATOR_TILE_H,
                ILLUMINATOR_TILE_GAP,
            );
            let states = [
                (AnimatorState::Entering, "Entering"),
                (AnimatorState::Entered, "Entered"),
                (AnimatorState::Exiting, "Exiting"),
                (AnimatorState::Exited, "Exited"),
            ];
            for (idx, (state, label)) in states.iter().enumerate() {
                let row = idx / grid.cols;
                let col = idx % grid.cols;
                let tile_bounds = Bounds::new(
                    inner.origin.x + col as f32 * (ILLUMINATOR_TILE_W + ILLUMINATOR_TILE_GAP),
                    inner.origin.y + row as f32 * (ILLUMINATOR_TILE_H + ILLUMINATOR_TILE_GAP),
                    ILLUMINATOR_TILE_W,
                    ILLUMINATOR_TILE_H,
                );
                draw_tile(tile_bounds, label, cx, |inner, cx| {
                    let center = Point::new(
                        inner.origin.x + inner.size.width / 2.0,
                        inner.origin.y + inner.size.height / 2.0,
                    );
                    let mut illuminator = Illuminator::new().radius(70.0).intensity(0.8);
                    illuminator.snap_to_position(center.x, center.y);
                    illuminator.update_with_delta(*state, Duration::from_millis(350));
                    illuminator.paint(inner, cx);
                });
            }
        });
    }
}

fn frame_style_short(style: FrameStyle) -> &'static str {
    match style {
        FrameStyle::Corners => "Crn",
        FrameStyle::Lines => "Lin",
        FrameStyle::Octagon => "Oct",
        FrameStyle::Underline => "Und",
        FrameStyle::Nefrex => "Nef",
        FrameStyle::Kranox => "Krn",
        FrameStyle::Nero => "Nro",
        FrameStyle::Header => "Hdr",
        FrameStyle::Circle => "Cir",
    }
}

fn frame_style_label(style: FrameStyle) -> &'static str {
    match style {
        FrameStyle::Corners => "Corners",
        FrameStyle::Lines => "Lines",
        FrameStyle::Octagon => "Octagon",
        FrameStyle::Underline => "Underline",
        FrameStyle::Nefrex => "Nefrex",
        FrameStyle::Kranox => "Kranox",
        FrameStyle::Nero => "Nero",
        FrameStyle::Header => "Header",
        FrameStyle::Circle => "Circle",
    }
}

fn frame_animation_label(animation: FrameAnimation) -> &'static str {
    match animation {
        FrameAnimation::Fade => "Fade",
        FrameAnimation::Draw => "Draw",
        FrameAnimation::Flicker => "Flicker",
        FrameAnimation::Assemble => "Asm",
    }
}

fn draw_direction_label(direction: DrawDirection) -> &'static str {
    match direction {
        DrawDirection::LeftToRight => "L->R",
        DrawDirection::RightToLeft => "R->L",
        DrawDirection::TopToBottom => "T->B",
        DrawDirection::BottomToTop => "B->T",
        DrawDirection::CenterOut => "Center",
        DrawDirection::EdgesIn => "Edges",
    }
}

fn dot_shape_label(shape: DotShape) -> &'static str {
    match shape {
        DotShape::Box => "Box",
        DotShape::Circle => "Circle",
        DotShape::Cross => "Cross",
    }
}

fn dots_origin_label(origin: DotsOrigin) -> &'static str {
    match origin {
        DotsOrigin::Left => "L",
        DotsOrigin::Right => "R",
        DotsOrigin::Top => "T",
        DotsOrigin::Bottom => "B",
        DotsOrigin::Center => "C",
        DotsOrigin::Point(_, _) => "P",
    }
}

fn line_direction_label(direction: LineDirection) -> &'static str {
    match direction {
        LineDirection::Right => "Right",
        LineDirection::Left => "Left",
        LineDirection::Down => "Down",
        LineDirection::Up => "Up",
    }
}
