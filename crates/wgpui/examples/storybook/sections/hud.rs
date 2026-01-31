use wgpui::components::atoms::{Mode, Model, Status};
use wgpui::components::hud::{
    CornerConfig, DrawDirection, Frame, FrameAnimation, NotificationLevel, ResizablePane, Reticle,
    Scanlines, SignalMeter, StatusBar, StatusBarPosition, StatusItem, Tooltip, TooltipPosition,
};
use wgpui::{Bounds, Component, Hsla, PaintContext, Point, Quad, theme};

use crate::constants::{
    BG_TILE_GAP, BG_TILE_H, BG_TILE_W, LIGHT_DEMO_FRAMES_INNER_H, LIGHT_DEMO_HERO_INNER_H,
    PANEL_PADDING, TOOLCALL_DEMO_INNER_H,
};
use crate::helpers::{
    draw_bitcoin_symbol, draw_panel, draw_tile, grid_metrics, inset_bounds, panel_height,
    panel_stack,
};
use crate::state::Storybook;

impl Storybook {
    pub(crate) fn paint_hud_widgets(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let available = (bounds.size.width - PANEL_PADDING * 2.0).max(0.0);
        let pulse = self.glow_pulse_anim.current_value();

        let scan_presets = [
            ("Tight", 8.0, 18.0, 0.8, 190.0, 0.0),
            ("Wide", 20.0, 24.0, 0.6, 190.0, 0.2),
            ("Soft", 14.0, 30.0, 0.5, 210.0, 0.4),
            ("Amber", 12.0, 22.0, 0.7, 35.0, 0.1),
            ("Emerald", 10.0, 28.0, 0.75, 120.0, 0.3),
            ("Deep", 16.0, 34.0, 0.55, 200.0, 0.55),
        ];

        let scan_grid = grid_metrics(
            available,
            scan_presets.len(),
            BG_TILE_W,
            BG_TILE_H,
            BG_TILE_GAP,
        );
        let scan_height = panel_height(scan_grid.height);
        let meter_presets = [
            ("Low 4", 4, 0.2, 190.0),
            ("Med 5", 5, 0.45, 190.0),
            ("High 6", 6, 0.75, 190.0),
            ("Full 8", 8, 1.0, 150.0),
            ("Amber", 6, 0.6, 35.0),
            ("Green", 5, 0.8, 120.0),
        ];

        let meter_grid = grid_metrics(
            available,
            meter_presets.len(),
            BG_TILE_W,
            BG_TILE_H,
            BG_TILE_GAP,
        );
        let meter_height = panel_height(meter_grid.height);

        let reticle_presets = [
            ("Compact", 18.0, 4.0, 4.0, 8.0, 190.0),
            ("Wide", 32.0, 6.0, 6.0, 12.0, 190.0),
            ("Long", 40.0, 8.0, 4.0, 14.0, 200.0),
            ("Amber", 28.0, 5.0, 8.0, 10.0, 35.0),
            ("Green", 26.0, 6.0, 6.0, 12.0, 120.0),
            ("Offset", 24.0, 10.0, 10.0, 8.0, 160.0),
        ];

        let reticle_grid = grid_metrics(
            available,
            reticle_presets.len(),
            BG_TILE_W,
            BG_TILE_H,
            BG_TILE_GAP,
        );
        let reticle_height = panel_height(reticle_grid.height);

        let resizable_presets: [(&str, bool, bool, f32); 6] = [
            ("Default", true, false, 8.0),
            ("Visible", true, true, 8.0),
            ("Large", true, true, 12.0),
            ("Small", true, true, 4.0),
            ("Disabled", false, false, 8.0),
            ("Styled", true, true, 10.0),
        ];

        let resizable_grid = grid_metrics(
            available,
            resizable_presets.len(),
            BG_TILE_W,
            BG_TILE_H,
            BG_TILE_GAP,
        );
        let resizable_height = panel_height(resizable_grid.height);

        let panels = panel_stack(
            bounds,
            &[scan_height, meter_height, reticle_height, resizable_height],
        );
        let scan_bounds = panels[0];
        draw_panel("Scanline sweeps", scan_bounds, cx, |inner, cx| {
            let grid = grid_metrics(
                inner.size.width,
                scan_presets.len(),
                BG_TILE_W,
                BG_TILE_H,
                BG_TILE_GAP,
            );
            for (idx, (label, spacing, scan_width, opacity, hue, offset)) in
                scan_presets.iter().enumerate()
            {
                let row = idx / grid.cols;
                let col = idx % grid.cols;
                let tile_bounds = Bounds::new(
                    inner.origin.x + col as f32 * (BG_TILE_W + BG_TILE_GAP),
                    inner.origin.y + row as f32 * (BG_TILE_H + BG_TILE_GAP),
                    BG_TILE_W,
                    BG_TILE_H,
                );
                let progress = (pulse + *offset).fract();
                draw_tile(tile_bounds, label, cx, |inner, cx| {
                    let mut scanlines = Scanlines::new()
                        .spacing(*spacing)
                        .scan_width(*scan_width)
                        .scan_progress(progress)
                        .opacity(*opacity)
                        .line_color(Hsla::new(*hue, 0.35, 0.6, 0.25))
                        .scan_color(Hsla::new(*hue, 0.8, 0.7, 0.35));
                    scanlines.paint(inner, cx);
                });
            }
        });

        let meter_bounds = panels[1];
        draw_panel("Signal meters", meter_bounds, cx, |inner, cx| {
            let grid = grid_metrics(
                inner.size.width,
                meter_presets.len(),
                BG_TILE_W,
                BG_TILE_H,
                BG_TILE_GAP,
            );
            for (idx, (label, bars, level, hue)) in meter_presets.iter().enumerate() {
                let row = idx / grid.cols;
                let col = idx % grid.cols;
                let tile_bounds = Bounds::new(
                    inner.origin.x + col as f32 * (BG_TILE_W + BG_TILE_GAP),
                    inner.origin.y + row as f32 * (BG_TILE_H + BG_TILE_GAP),
                    BG_TILE_W,
                    BG_TILE_H,
                );
                draw_tile(tile_bounds, label, cx, |inner, cx| {
                    let active = Hsla::new(*hue, 0.8, 0.6, 0.9);
                    let inactive = Hsla::new(*hue, 0.25, 0.3, 0.35);
                    let mut meter = SignalMeter::new()
                        .bars(*bars)
                        .level(*level)
                        .gap(3.0)
                        .active_color(active)
                        .inactive_color(inactive);
                    meter.paint(inset_bounds(inner, 8.0), cx);
                });
            }
        });

        let reticle_bounds = panels[2];
        draw_panel("Reticle variants", reticle_bounds, cx, |inner, cx| {
            let grid = grid_metrics(
                inner.size.width,
                reticle_presets.len(),
                BG_TILE_W,
                BG_TILE_H,
                BG_TILE_GAP,
            );
            for (idx, (label, line_length, gap, center, tick, hue)) in
                reticle_presets.iter().enumerate()
            {
                let row = idx / grid.cols;
                let col = idx % grid.cols;
                let tile_bounds = Bounds::new(
                    inner.origin.x + col as f32 * (BG_TILE_W + BG_TILE_GAP),
                    inner.origin.y + row as f32 * (BG_TILE_H + BG_TILE_GAP),
                    BG_TILE_W,
                    BG_TILE_H,
                );
                draw_tile(tile_bounds, label, cx, |inner, cx| {
                    let mut reticle = Reticle::new()
                        .line_length(*line_length)
                        .gap(*gap)
                        .center_size(*center)
                        .tick_length(*tick)
                        .color(Hsla::new(*hue, 0.6, 0.6, 0.85));
                    reticle.paint(inset_bounds(inner, 6.0), cx);
                });
            }
        });

        // Resizable pane demos
        let resizable_bounds = panels[3];
        draw_panel("Resizable panes", resizable_bounds, cx, |inner, cx| {
            let grid = grid_metrics(
                inner.size.width,
                resizable_presets.len(),
                BG_TILE_W,
                BG_TILE_H,
                BG_TILE_GAP,
            );
            for (idx, (label, resizable, show_handles, handle_size)) in
                resizable_presets.iter().enumerate()
            {
                let row = idx / grid.cols;
                let col = idx % grid.cols;
                let tile_bounds = Bounds::new(
                    inner.origin.x + col as f32 * (BG_TILE_W + BG_TILE_GAP),
                    inner.origin.y + row as f32 * (BG_TILE_H + BG_TILE_GAP),
                    BG_TILE_W,
                    BG_TILE_H,
                );
                draw_tile(tile_bounds, label, cx, |inner, cx| {
                    let (handle_color, handle_hover_color, bg, border) = if *label == "Styled" {
                        (
                            Hsla::new(180.0, 0.6, 0.4, 0.4),
                            Hsla::new(180.0, 0.8, 0.6, 0.8),
                            Hsla::new(180.0, 0.2, 0.1, 0.6),
                            Hsla::new(180.0, 0.6, 0.5, 0.8),
                        )
                    } else {
                        (
                            Hsla::new(0.0, 0.0, 0.5, 0.3),
                            Hsla::new(180.0, 0.6, 0.5, 0.6),
                            Hsla::new(0.0, 0.0, 0.15, 0.5),
                            Hsla::new(0.0, 0.0, 0.4, 0.6),
                        )
                    };
                    let mut pane = ResizablePane::new()
                        .resizable(*resizable)
                        .show_handles(*show_handles)
                        .handle_size(*handle_size)
                        .handle_color(handle_color)
                        .handle_hover_color(handle_hover_color)
                        .background(bg)
                        .border_color(border)
                        .border_width(1.0);
                    pane.paint(inset_bounds(inner, 4.0), cx);
                });
            }
        });
    }

    pub(crate) fn paint_light_demo(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let progress = self.light_frame_anim.current_value();
        let glow_pulse = self.glow_pulse_anim.current_value();

        let frames_height = panel_height(LIGHT_DEMO_FRAMES_INNER_H);
        let hero_height = panel_height(LIGHT_DEMO_HERO_INNER_H);
        let panels = panel_stack(bounds, &[frames_height, hero_height]);
        let frames_bounds = panels[0];
        draw_panel("Light demo frames", frames_bounds, cx, |inner, cx| {
            let frame_w = ((inner.size.width - 16.0).max(0.0) / 2.0).max(0.0);
            let frame_h = 60.0;
            let left_x = inner.origin.x;
            let right_x = inner.origin.x + frame_w + 8.0;
            let mut row_y = inner.origin.y;

            let white = Hsla::new(0.0, 0.0, 1.0, 1.0);
            let dark_bg = Hsla::new(0.0, 0.0, 0.08, 0.8);
            let muted = Hsla::new(0.0, 0.0, 0.7, 1.0);
            let white_glow = Hsla::new(0.0, 0.0, 1.0, 0.6 * glow_pulse);
            let cyan_glow = Hsla::new(180.0, 1.0, 0.7, 0.5 * glow_pulse);
            let purple_glow = Hsla::new(280.0, 1.0, 0.7, 0.5 * glow_pulse);
            let green_glow = Hsla::new(120.0, 1.0, 0.6, 0.5 * glow_pulse);

            Frame::corners()
                .line_color(white)
                .bg_color(dark_bg)
                .glow_color(white_glow)
                .stroke_width(2.0)
                .corner_length(18.0)
                .animation_mode(FrameAnimation::Fade)
                .animation_progress(progress)
                .paint(Bounds::new(left_x, row_y, frame_w, frame_h), cx);
            let lbl = cx.text.layout(
                "Fade",
                Point::new(left_x + 10.0, row_y + frame_h / 2.0),
                11.0,
                white.with_alpha(progress),
            );
            cx.scene.draw_text(lbl);

            Frame::lines()
                .line_color(white)
                .bg_color(dark_bg)
                .glow_color(cyan_glow)
                .stroke_width(2.0)
                .animation_mode(FrameAnimation::Draw)
                .draw_direction(DrawDirection::CenterOut)
                .animation_progress(progress)
                .paint(Bounds::new(right_x, row_y, frame_w, frame_h), cx);
            let lbl = cx.text.layout(
                "Draw (CenterOut)",
                Point::new(right_x + 10.0, row_y + frame_h / 2.0),
                11.0,
                white,
            );
            cx.scene.draw_text(lbl);

            row_y += frame_h + 10.0;

            Frame::octagon()
                .line_color(white)
                .bg_color(dark_bg)
                .glow_color(purple_glow)
                .stroke_width(2.0)
                .corner_length(14.0)
                .animation_mode(FrameAnimation::Flicker)
                .animation_progress(progress)
                .paint(Bounds::new(left_x, row_y, frame_w, frame_h), cx);
            let lbl = cx.text.layout(
                "Flicker",
                Point::new(left_x + 10.0, row_y + frame_h / 2.0),
                11.0,
                white,
            );
            cx.scene.draw_text(lbl);

            Frame::nefrex()
                .line_color(white)
                .bg_color(dark_bg)
                .glow_color(green_glow)
                .stroke_width(1.5)
                .square_size(12.0)
                .small_line_length(12.0)
                .large_line_length(40.0)
                .corner_config(CornerConfig::all())
                .animation_mode(FrameAnimation::Assemble)
                .animation_progress(progress)
                .paint(Bounds::new(right_x, row_y, frame_w, frame_h), cx);
            let lbl = cx.text.layout(
                "Assemble",
                Point::new(right_x + 10.0, row_y + frame_h / 2.0),
                11.0,
                white,
            );
            cx.scene.draw_text(lbl);

            row_y += frame_h + 10.0;

            Frame::underline()
                .line_color(white)
                .bg_color(dark_bg)
                .glow_color(white_glow)
                .stroke_width(2.0)
                .square_size(12.0)
                .animation_mode(FrameAnimation::Draw)
                .draw_direction(DrawDirection::LeftToRight)
                .animation_progress(progress)
                .paint(Bounds::new(left_x, row_y, frame_w, frame_h), cx);
            let lbl = cx.text.layout(
                "Underline (Draw)",
                Point::new(left_x + 10.0, row_y + frame_h / 2.0),
                11.0,
                white,
            );
            cx.scene.draw_text(lbl);

            Frame::kranox()
                .line_color(white)
                .bg_color(dark_bg)
                .glow_color(cyan_glow)
                .stroke_width(2.0)
                .square_size(10.0)
                .small_line_length(10.0)
                .large_line_length(35.0)
                .animation_mode(FrameAnimation::Draw)
                .draw_direction(DrawDirection::EdgesIn)
                .animation_progress(progress)
                .paint(Bounds::new(right_x, row_y, frame_w, frame_h), cx);
            let lbl = cx.text.layout(
                "Kranox (EdgesIn)",
                Point::new(right_x + 10.0, row_y + frame_h / 2.0),
                11.0,
                white,
            );
            cx.scene.draw_text(lbl);

            row_y += frame_h + 14.0;

            let wallet_bounds = Bounds::new(left_x, row_y, inner.size.width, 80.0);
            Frame::corners()
                .line_color(white)
                .bg_color(dark_bg)
                .glow_color(cyan_glow)
                .stroke_width(2.0)
                .corner_length(24.0)
                .animation_mode(FrameAnimation::Draw)
                .draw_direction(DrawDirection::CenterOut)
                .animation_progress(progress)
                .paint(wallet_bounds, cx);

            let balance_label = cx.text.layout(
                "Balance",
                Point::new(wallet_bounds.origin.x + 20.0, wallet_bounds.origin.y + 14.0),
                11.0,
                muted,
            );
            cx.scene.draw_text(balance_label);

            let font_size = 28.0;
            let symbol_x = wallet_bounds.origin.x + 20.0;
            let symbol_y = wallet_bounds.origin.y + 28.0;
            draw_bitcoin_symbol(cx.scene, cx.text, symbol_x, symbol_y, font_size, white);

            let sats_amount = cx.text.layout(
                "42069",
                Point::new(symbol_x + font_size * 0.55, symbol_y),
                font_size,
                white,
            );
            cx.scene.draw_text(sats_amount);

            let usd_value = cx.text.layout(
                "~ $42.07",
                Point::new(wallet_bounds.origin.x + 20.0, wallet_bounds.origin.y + 60.0),
                13.0,
                muted,
            );
            cx.scene.draw_text(usd_value);
        });
        let hero_bounds = panels[1];
        draw_panel("Light demo hero frame", hero_bounds, cx, |inner, cx| {
            let pane_w = inner.size.width.min(520.0);
            let pane_h = inner.size.height.min(220.0);
            let pane_x = inner.origin.x + (inner.size.width - pane_w) / 2.0;
            let pane_y = inner.origin.y + (inner.size.height - pane_h) / 2.0;
            let text_alpha = ((progress - 0.2) / 0.8).clamp(0.0, 1.0);

            let white = Hsla::new(0.0, 0.0, 1.0, text_alpha);
            let muted = Hsla::new(0.0, 0.0, 0.7, text_alpha);
            let accent = Hsla::new(0.5, 1.0, 0.6, text_alpha);
            let dark_bg = Hsla::new(0.0, 0.0, 0.06, 0.9);
            let cyan_glow = Hsla::new(0.5, 1.0, 0.6, 0.7 * glow_pulse);

            let mut frame = Frame::nefrex()
                .line_color(white)
                .bg_color(dark_bg)
                .glow_color(cyan_glow)
                .stroke_width(2.0)
                .corner_config(CornerConfig::all())
                .square_size(14.0)
                .small_line_length(14.0)
                .large_line_length(50.0)
                .animation_mode(FrameAnimation::Assemble)
                .draw_direction(DrawDirection::CenterOut)
                .animation_progress(progress);
            frame.paint(Bounds::new(pane_x, pane_y, pane_w, pane_h), cx);

            if text_alpha > 0.01 {
                let title = cx.text.layout(
                    "OpenAgents",
                    Point::new(pane_x + 30.0, pane_y + 40.0),
                    32.0,
                    white,
                );
                cx.scene.draw_text(title);

                let subtitle = cx.text.layout(
                    "Decentralized AI Infrastructure",
                    Point::new(pane_x + 30.0, pane_y + 80.0),
                    16.0,
                    muted,
                );
                cx.scene.draw_text(subtitle);

                let body_lines = [
                    "Build autonomous agents",
                    "Deploy on decentralized compute",
                    "Earn Bitcoin for contributions",
                ];
                for (idx, line) in body_lines.iter().enumerate() {
                    let line_y = pane_y + 130.0 + idx as f32 * 28.0;
                    let bullet =
                        cx.text
                            .layout(">", Point::new(pane_x + 30.0, line_y), 14.0, accent);
                    cx.scene.draw_text(bullet);
                    let text =
                        cx.text
                            .layout(line, Point::new(pane_x + 44.0, line_y + 2.0), 13.0, muted);
                    cx.scene.draw_text(text);
                }
            }
        });
    }

    pub(crate) fn paint_toolcall_demo(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let demo_height = panel_height(TOOLCALL_DEMO_INNER_H);
        let panel_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            demo_height,
        );
        draw_panel("Toolcall UI demo", panel_bounds, cx, |inner, cx| {
            self.toolcall_demo.paint(inner, cx);
        });
    }

    pub(crate) fn paint_system_ui(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Tooltip demos
        let tooltip_height = panel_height(180.0);
        let status_height = panel_height(120.0);
        let notif_height = panel_height(260.0);
        let menu_height = panel_height(200.0);
        let palette_height = panel_height(240.0);
        let panels = panel_stack(
            bounds,
            &[
                tooltip_height,
                status_height,
                notif_height,
                menu_height,
                palette_height,
            ],
        );
        let tooltip_bounds = panels[0];
        draw_panel("Tooltip positions", tooltip_bounds, cx, |inner, cx| {
            let positions = [
                ("Top", TooltipPosition::Top, 0),
                ("Bottom", TooltipPosition::Bottom, 1),
                ("Left", TooltipPosition::Left, 2),
                ("Right", TooltipPosition::Right, 3),
                ("Auto", TooltipPosition::Auto, 4),
            ];
            let tile_w = 140.0;
            let tile_h = 60.0;
            let gap = 16.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor() as usize;

            for (idx, (label, position, _)) in positions.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_bounds = Bounds::new(
                    inner.origin.x + col as f32 * (tile_w + gap),
                    inner.origin.y + row as f32 * (tile_h + gap),
                    tile_w,
                    tile_h,
                );

                // Draw target button
                let btn_bounds = Bounds::new(
                    tile_bounds.origin.x + tile_w / 2.0 - 40.0,
                    tile_bounds.origin.y + tile_h / 2.0 - 12.0,
                    80.0,
                    24.0,
                );
                cx.scene.draw_quad(
                    Quad::new(btn_bounds)
                        .with_background(theme::bg::MUTED)
                        .with_border(theme::border::DEFAULT, 1.0),
                );
                let btn_text = cx.text.layout(
                    *label,
                    Point::new(btn_bounds.origin.x + 8.0, btn_bounds.origin.y + 6.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(btn_text);

                // Draw tooltip (always visible for demo)
                let mut tooltip =
                    Tooltip::new(format!("Tooltip positioned {}", label.to_lowercase()))
                        .position(*position)
                        .target(btn_bounds);
                tooltip.show();
                tooltip.paint(tile_bounds, cx);
            }
        });

        // StatusBar demos
        let status_bounds = panels[1];
        draw_panel("StatusBar variants", status_bounds, cx, |inner, cx| {
            // Top status bar
            let top_bar_bounds =
                Bounds::new(inner.origin.x, inner.origin.y, inner.size.width, 32.0);
            let mut top_bar = StatusBar::new()
                .position(StatusBarPosition::Top)
                .height(28.0)
                .items(vec![
                    StatusItem::mode("mode", Mode::Plan).left(),
                    StatusItem::text("file", "src/main.rs").center(),
                    StatusItem::model("model", Model::CodexOpus).right(),
                    StatusItem::status("status", Status::Online).right(),
                ]);
            top_bar.paint(top_bar_bounds, cx);

            // Bottom status bar
            let bot_bar_bounds = Bounds::new(
                inner.origin.x,
                inner.origin.y + 50.0,
                inner.size.width,
                32.0,
            );
            let mut bot_bar = StatusBar::new()
                .position(StatusBarPosition::Top)
                .height(28.0)
                .items(vec![
                    StatusItem::mode("mode", Mode::Act).left(),
                    StatusItem::text("branch", "main").left(),
                    StatusItem::text("line", "Ln 42, Col 8").center(),
                    StatusItem::status("status", Status::Busy).right(),
                    StatusItem::model("model", Model::CodexSonnet).right(),
                ]);
            bot_bar.paint(bot_bar_bounds, cx);
        });

        // Notifications demos
        let notif_bounds = panels[2];
        draw_panel("Notification levels", notif_bounds, cx, |inner, cx| {
            let levels = [
                ("Info", NotificationLevel::Info, "System update available"),
                (
                    "Success",
                    NotificationLevel::Success,
                    "Build completed successfully",
                ),
                (
                    "Warning",
                    NotificationLevel::Warning,
                    "Deprecated API usage detected",
                ),
                (
                    "Error",
                    NotificationLevel::Error,
                    "Connection to server failed",
                ),
            ];

            let notif_w = 320.0;
            let notif_h = 50.0;
            let gap = 12.0;

            for (idx, (title, level, message)) in levels.iter().enumerate() {
                let notif_bounds = Bounds::new(
                    inner.origin.x,
                    inner.origin.y + idx as f32 * (notif_h + gap),
                    notif_w,
                    notif_h,
                );

                // Draw notification preview manually
                cx.scene.draw_quad(
                    Quad::new(notif_bounds)
                        .with_background(theme::bg::SURFACE)
                        .with_border(level.color(), 2.0),
                );

                let icon_run = cx.text.layout(
                    level.icon(),
                    Point::new(notif_bounds.origin.x + 10.0, notif_bounds.origin.y + 10.0),
                    theme::font_size::LG,
                    level.color(),
                );
                cx.scene.draw_text(icon_run);

                let title_run = cx.text.layout(
                    *title,
                    Point::new(notif_bounds.origin.x + 40.0, notif_bounds.origin.y + 10.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(title_run);

                let msg_run = cx.text.layout(
                    *message,
                    Point::new(notif_bounds.origin.x + 40.0, notif_bounds.origin.y + 28.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(msg_run);
            }
        });

        // ContextMenu demo
        let menu_bounds = panels[3];
        draw_panel("ContextMenu preview", menu_bounds, cx, |inner, cx| {
            // Draw a static preview of a context menu
            let menu_w = 180.0;
            let menu_h = 160.0;
            let menu_bounds =
                Bounds::new(inner.origin.x + 20.0, inner.origin.y + 10.0, menu_w, menu_h);

            cx.scene.draw_quad(
                Quad::new(menu_bounds)
                    .with_background(theme::bg::ELEVATED)
                    .with_border(theme::border::DEFAULT, 1.0),
            );

            let items = [
                ("Cut", Some("Cmd+X"), false, false),
                ("Copy", Some("Cmd+C"), false, false),
                ("Paste", Some("Cmd+V"), true, false),
                ("---", None, false, true),
                ("Select All", Some("Cmd+A"), false, false),
            ];

            let item_h = 28.0;
            let sep_h = 9.0;
            let mut item_y = menu_bounds.origin.y + 4.0;

            for (idx, (label, shortcut, disabled, is_sep)) in items.iter().enumerate() {
                if *is_sep {
                    cx.scene.draw_quad(
                        Quad::new(Bounds::new(
                            menu_bounds.origin.x + 4.0,
                            item_y + sep_h / 2.0,
                            menu_w - 8.0,
                            1.0,
                        ))
                        .with_background(theme::border::DEFAULT),
                    );
                    item_y += sep_h;
                    continue;
                }

                let is_hovered = idx == 1; // Highlight "Copy"
                if is_hovered {
                    cx.scene.draw_quad(
                        Quad::new(Bounds::new(
                            menu_bounds.origin.x + 4.0,
                            item_y,
                            menu_w - 8.0,
                            item_h,
                        ))
                        .with_background(theme::bg::MUTED),
                    );
                }

                let text_color = if *disabled {
                    theme::text::MUTED
                } else {
                    theme::text::PRIMARY
                };
                let label_run = cx.text.layout(
                    *label,
                    Point::new(menu_bounds.origin.x + 12.0, item_y + 8.0),
                    theme::font_size::SM,
                    text_color,
                );
                cx.scene.draw_text(label_run);

                if let Some(sc) = shortcut {
                    let sc_run = cx.text.layout(
                        *sc,
                        Point::new(menu_bounds.origin.x + menu_w - 60.0, item_y + 8.0),
                        theme::font_size::XS,
                        theme::text::MUTED,
                    );
                    cx.scene.draw_text(sc_run);
                }

                item_y += item_h;
            }

            // Description
            let desc = cx.text.layout(
                "Right-click context menu with shortcuts",
                Point::new(inner.origin.x + menu_w + 40.0, inner.origin.y + 80.0),
                theme::font_size::SM,
                theme::text::MUTED,
            );
            cx.scene.draw_text(desc);
        });

        // CommandPalette demo
        let palette_bounds = panels[4];
        draw_panel("CommandPalette preview", palette_bounds, cx, |inner, cx| {
            let palette_w = 400.0;
            let palette_h = 200.0;
            let palette_x = inner.origin.x + (inner.size.width - palette_w) / 2.0;
            let palette_y = inner.origin.y + 10.0;

            // Palette container
            cx.scene.draw_quad(
                Quad::new(Bounds::new(palette_x, palette_y, palette_w, palette_h))
                    .with_background(theme::bg::ELEVATED)
                    .with_border(theme::accent::PRIMARY, 1.0),
            );

            // Search input
            let input_h = 36.0;
            cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    palette_x + 8.0,
                    palette_y + 8.0,
                    palette_w - 16.0,
                    input_h,
                ))
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
            );
            let search_text = cx.text.layout(
                "file",
                Point::new(palette_x + 16.0, palette_y + 18.0),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(search_text);

            // Command list
            let commands = [
                ("file.new", "New File", "Cmd+N"),
                ("file.open", "Open File", "Cmd+O"),
                ("file.save", "Save", "Cmd+S"),
                ("file.close", "Close Tab", "Cmd+W"),
            ];

            let item_h = 36.0;
            let list_y = palette_y + input_h + 16.0;

            for (idx, (id, label, shortcut)) in commands.iter().enumerate() {
                let item_y = list_y + idx as f32 * item_h;
                let is_selected = idx == 0;

                if is_selected {
                    cx.scene.draw_quad(
                        Quad::new(Bounds::new(
                            palette_x + 4.0,
                            item_y,
                            palette_w - 8.0,
                            item_h,
                        ))
                        .with_background(theme::bg::MUTED),
                    );
                }

                let label_run = cx.text.layout(
                    *label,
                    Point::new(palette_x + 16.0, item_y + 10.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(label_run);

                let id_run = cx.text.layout(
                    *id,
                    Point::new(palette_x + 16.0, item_y + 24.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(id_run);

                let shortcut_run = cx.text.layout(
                    *shortcut,
                    Point::new(palette_x + palette_w - 70.0, item_y + 12.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(shortcut_run);
            }
        });
    }
}
