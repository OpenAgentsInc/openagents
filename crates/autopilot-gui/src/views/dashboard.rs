use std::cell::RefCell;
use std::rc::Rc;

use wgpui::components::{Button, ButtonVariant};
use wgpui::{Bounds, Component, EventContext, EventResult, InputEvent, PaintContext, Quad, Text, theme};

use crate::backend::BackendCommand;
use crate::state::{AppState, ApmTier};
use crate::views::fit_text;

/// APM tier colors matching the web dashboard
mod apm_colors {
    use wgpui::Hsla;
    use super::ApmTier;

    /// Convert RGB 0-255 to Hsla color
    fn rgb(r: u8, g: u8, b: u8) -> Hsla {
        Hsla::from_rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
    }

    pub fn baseline() -> Hsla { rgb(107, 114, 128) }  // gray
    pub fn active() -> Hsla { rgb(59, 130, 246) }     // blue
    pub fn productive() -> Hsla { rgb(16, 185, 129) } // green
    pub fn high_performance() -> Hsla { rgb(245, 158, 11) } // amber
    pub fn elite() -> Hsla { rgb(251, 191, 36) }      // gold

    pub fn for_tier(tier: ApmTier) -> Hsla {
        match tier {
            ApmTier::Baseline => baseline(),
            ApmTier::Active => active(),
            ApmTier::Productive => productive(),
            ApmTier::HighPerformance => high_performance(),
            ApmTier::Elite => elite(),
        }
    }
}

pub struct DashboardView {
    state: Rc<RefCell<AppState>>,
    start_button: Button,
    stop_button: Button,
}

impl DashboardView {
    pub fn new(
        state: Rc<RefCell<AppState>>,
        command_tx: std::sync::mpsc::Sender<BackendCommand>,
    ) -> Self {
        let start_tx = command_tx.clone();
        let stop_tx = command_tx;
        let start_button = Button::new("Start Full Auto")
            .variant(ButtonVariant::Primary)
            .on_click(move || {
                let _ = start_tx.send(BackendCommand::StartFullAuto);
            });
        let stop_button = Button::new("Stop Full Auto")
            .variant(ButtonVariant::Danger)
            .on_click(move || {
                let _ = stop_tx.send(BackendCommand::StopFullAuto);
            });

        Self {
            state,
            start_button,
            stop_button,
        }
    }
}

impl Component for DashboardView {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let state = self.state.borrow();
        let padding = theme::spacing::MD;
        let mut y = bounds.origin.y + padding;
        let line_height = theme::font_size::SM * 1.5;
        let available_width = (bounds.size.width - padding * 2.0).max(0.0);

        // APM Gauge Section
        let apm_value = state.current_apm.unwrap_or_else(|| {
            // Fall back to most recent session APM if no current
            state.sessions.first().and_then(|s| s.apm).unwrap_or(0.0)
        });
        let tier = ApmTier::from_apm(apm_value);
        let tier_color = apm_colors::for_tier(tier);

        // APM Header
        let mut apm_header = Text::new("APM (Actions/Min)")
            .font_size(theme::font_size::XS)
            .color(theme::text::MUTED);
        apm_header.paint(
            Bounds::new(bounds.origin.x + padding, y, available_width, line_height),
            cx,
        );
        y += line_height * 0.8;

        // APM Value Display (large, colored)
        let apm_text = format!("{:.1}", apm_value);
        let mut apm_value_display = Text::new(&apm_text)
            .font_size(theme::font_size::XL)
            .color(tier_color);
        apm_value_display.paint(
            Bounds::new(bounds.origin.x + padding, y, available_width * 0.4, line_height * 2.0),
            cx,
        );

        // Tier Badge (next to APM value)
        let mut tier_badge = Text::new(tier.name())
            .font_size(theme::font_size::SM)
            .color(tier_color);
        tier_badge.paint(
            Bounds::new(
                bounds.origin.x + padding + available_width * 0.35,
                y + line_height * 0.5,
                available_width * 0.4,
                line_height,
            ),
            cx,
        );
        y += line_height * 2.0;

        // APM Gauge Bar (horizontal)
        let gauge_height = 8.0;
        let gauge_width = available_width.min(200.0);
        let gauge_x = bounds.origin.x + padding;

        // Background bar (gray)
        cx.scene.draw_quad(
            Quad::new(Bounds::new(gauge_x, y, gauge_width, gauge_height))
                .with_background(apm_colors::baseline()),
        );

        // Calculate fill percentage (max 60 APM for gauge)
        let fill_pct = (apm_value / 60.0).min(1.0);
        let fill_width = gauge_width * fill_pct as f32;

        // Colored fill bar
        if fill_width > 0.0 {
            cx.scene.draw_quad(
                Quad::new(Bounds::new(gauge_x, y, fill_width, gauge_height))
                    .with_background(tier_color),
            );
        }

        // Tier threshold markers
        let tier_thresholds = [(5.0, "5"), (15.0, "15"), (30.0, "30"), (50.0, "50")];
        for (threshold, _label) in tier_thresholds.iter() {
            let marker_x = gauge_x + (gauge_width * (*threshold / 60.0) as f32);
            cx.scene.draw_quad(
                Quad::new(Bounds::new(marker_x - 0.5, y - 2.0, 1.0, gauge_height + 4.0))
                    .with_background(theme::text::MUTED),
            );
        }
        y += gauge_height + theme::spacing::SM;

        // Separator
        y += theme::spacing::XS;

        // Summary Stats Section
        let full_auto_line = state
            .full_auto_metrics
            .as_ref()
            .map(|m| format!("Full auto: {}", m.worker_status))
            .unwrap_or_else(|| "Full auto: offline".to_string());
        let memory_line = state
            .full_auto_metrics
            .as_ref()
            .map(|m| {
                format!(
                    "Memory: {:.1} / {:.1} GB",
                    m.memory_available_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
                    m.memory_total_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
                )
            })
            .unwrap_or_else(|| "Memory: -".to_string());

        let error_rate_line = state
            .session_error_rate()
            .map(|rate| format!("Error rate: {:.1}%", rate * 100.0))
            .unwrap_or_else(|| "Error rate: -".to_string());

        let session_cost_line = state
            .session_cost_usd()
            .map(|cost| format!("Session est: ${:.2}", cost))
            .unwrap_or_else(|| "Session est: -".to_string());

        let summary_lines = [
            format!("Sessions: {}", state.summary.total_sessions),
            format!("Issues: {}", state.summary.total_issues_completed),
            format!("Cost: ${:.2}", state.summary.total_cost_usd),
            error_rate_line,
            session_cost_line,
            format!("Rate: {:.0}%", state.summary.completion_rate * 100.0),
            full_auto_line,
            memory_line,
        ];

        for line in summary_lines {
            let line = fit_text(cx, &line, theme::font_size::SM, available_width);
            let mut text = Text::new(line)
                .font_size(theme::font_size::SM)
                .color(theme::text::PRIMARY);
            text.paint(
                Bounds::new(
                    bounds.origin.x + padding,
                    y,
                    bounds.size.width - padding * 2.0,
                    line_height,
                ),
                cx,
            );
            y += line_height;
        }

        y += theme::spacing::SM;
        let header_line = format!("Recent sessions: {}", state.sessions.len());
        let header_line = fit_text(cx, &header_line, theme::font_size::XS, available_width);
        let mut header = Text::new(header_line)
            .font_size(theme::font_size::XS)
            .color(theme::text::MUTED);
        header.paint(
            Bounds::new(
                bounds.origin.x + padding,
                y,
                bounds.size.width - padding * 2.0,
                line_height,
            ),
            cx,
        );
        y += line_height;

        for session in state.sessions.iter().take(6) {
            let id = session.id.chars().take(8).collect::<String>();
            let line = format!(
                "{}  {}  ${:.3}",
                id,
                format!("{:?}", session.final_status),
                session.cost_usd
            );
            let line = fit_text(cx, &line, theme::font_size::XS, available_width);
            let mut text = Text::new(line)
                .font_size(theme::font_size::XS)
                .color(theme::text::MUTED);
            text.paint(
                Bounds::new(
                    bounds.origin.x + padding,
                    y,
                    bounds.size.width - padding * 2.0,
                    line_height,
                ),
                cx,
            );
            y += line_height;
        }

        let full_auto_running = state
            .full_auto_metrics
            .as_ref()
            .map(|m| m.worker_status != "stopped")
            .unwrap_or(false);
        let button = if full_auto_running {
            &mut self.stop_button
        } else {
            &mut self.start_button
        };
        let (button_w, button_h) = button.size_hint();
        let button_w = button_w.unwrap_or(140.0).min(bounds.size.width - padding * 2.0);
        let button_h = button_h.unwrap_or(32.0);
        let button_bounds = Bounds::new(
            bounds.origin.x + bounds.size.width - padding - button_w,
            bounds.origin.y + bounds.size.height - padding - button_h,
            button_w,
            button_h,
        );
        button.paint(button_bounds, cx);
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        let state = self.state.borrow();
        let padding = theme::spacing::MD;
        let full_auto_running = state
            .full_auto_metrics
            .as_ref()
            .map(|m| m.worker_status != "stopped")
            .unwrap_or(false);

        let button = if full_auto_running {
            &mut self.stop_button
        } else {
            &mut self.start_button
        };
        let (button_w, button_h) = button.size_hint();
        let button_w = button_w.unwrap_or(140.0);
        let button_h = button_h.unwrap_or(32.0);
        let button_bounds = Bounds::new(
            bounds.origin.x + bounds.size.width - padding - button_w,
            bounds.origin.y + bounds.size.height - padding - button_h,
            button_w,
            button_h,
        );
        button.event(event, button_bounds, cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use autopilot::daemon::supervisor::DaemonMetrics;
    use std::sync::mpsc;
    use wgpui::{EventContext, MouseButton};

    fn button_bounds(bounds: Bounds, button: &Button) -> Bounds {
        let padding = theme::spacing::MD;
        let (button_w, button_h) = button.size_hint();
        let button_w = button_w.unwrap_or(140.0);
        let button_h = button_h.unwrap_or(32.0);
        Bounds::new(
            bounds.origin.x + bounds.size.width - padding - button_w,
            bounds.origin.y + bounds.size.height - padding - button_h,
            button_w,
            button_h,
        )
    }

    #[test]
    fn test_dashboard_view_start_full_auto() {
        let state = Rc::new(RefCell::new(AppState::new()));
        let (tx, rx) = mpsc::channel();
        let mut view = DashboardView::new(state, tx);
        let bounds = Bounds::new(0.0, 0.0, 520.0, 320.0);

        let button = &view.start_button;
        let button_bounds = button_bounds(bounds, button);
        let mut cx = EventContext::new();
        let down = InputEvent::MouseDown {
            button: MouseButton::Left,
            x: button_bounds.origin.x + 2.0,
            y: button_bounds.origin.y + 2.0,
        };
        let up = InputEvent::MouseUp {
            button: MouseButton::Left,
            x: button_bounds.origin.x + 2.0,
            y: button_bounds.origin.y + 2.0,
        };

        view.event(&down, bounds, &mut cx);
        view.event(&up, bounds, &mut cx);

        let cmd = rx.try_recv().expect("command");
        assert!(matches!(cmd, BackendCommand::StartFullAuto));
    }

    #[test]
    fn test_dashboard_view_stop_full_auto() {
        let state = Rc::new(RefCell::new(AppState::new()));
        state.borrow_mut().full_auto_metrics = Some(DaemonMetrics {
            worker_status: "running".to_string(),
            ..DaemonMetrics::default()
        });

        let (tx, rx) = mpsc::channel();
        let mut view = DashboardView::new(state, tx);
        let bounds = Bounds::new(0.0, 0.0, 520.0, 320.0);

        let button = &view.stop_button;
        let button_bounds = button_bounds(bounds, button);
        let mut cx = EventContext::new();
        let down = InputEvent::MouseDown {
            button: MouseButton::Left,
            x: button_bounds.origin.x + 2.0,
            y: button_bounds.origin.y + 2.0,
        };
        let up = InputEvent::MouseUp {
            button: MouseButton::Left,
            x: button_bounds.origin.x + 2.0,
            y: button_bounds.origin.y + 2.0,
        };

        view.event(&down, bounds, &mut cx);
        view.event(&up, bounds, &mut cx);

        let cmd = rx.try_recv().expect("command");
        assert!(matches!(cmd, BackendCommand::StopFullAuto));
    }
}
