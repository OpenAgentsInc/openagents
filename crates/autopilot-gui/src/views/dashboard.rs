use std::cell::RefCell;
use std::rc::Rc;

use wgpui::components::{Button, ButtonVariant};
use wgpui::{Bounds, Component, EventContext, EventResult, InputEvent, PaintContext, Text, theme};

use crate::backend::BackendCommand;
use crate::state::AppState;
use crate::views::fit_text;

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

        let summary_lines = [
            format!("Total sessions: {}", state.summary.total_sessions),
            format!("Issues completed: {}", state.summary.total_issues_completed),
            format!("Total cost: ${:.3}", state.summary.total_cost_usd),
            format!("Completion rate: {:.1}%", state.summary.completion_rate * 100.0),
            format!("Avg duration: {:.1}s", state.summary.avg_duration_seconds),
            format!("Avg tokens: {:.0}", state.summary.avg_tokens_per_session),
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
