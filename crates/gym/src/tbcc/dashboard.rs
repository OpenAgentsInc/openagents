//! TBCC Dashboard Tab - KPIs, recent runs, quick actions

use gpui::prelude::*;
use gpui::*;
use theme::{bg, border, status, text, FONT_FAMILY};

use super::types::{DashboardStats, TBRunSummary, TBRunStatus, TBRunOutcome};

pub struct DashboardView {
    stats: Option<DashboardStats>,
    recent_runs: Vec<TBRunSummary>,
    loading: bool,
    focus_handle: FocusHandle,
}

impl DashboardView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        // Sample data for development
        let stats = Some(DashboardStats {
            success_rate: 67.5,
            avg_steps: 12.3,
            avg_duration: 45.2,
            total_runs: 24,
        });

        let recent_runs = vec![
            TBRunSummary {
                id: "run-001".to_string(),
                task_id: "regex-log".to_string(),
                task_name: "Regex Log Parser".to_string(),
                status: TBRunStatus::Completed,
                outcome: Some(TBRunOutcome::Success),
                duration_ms: Some(42500),
                steps: 15,
            },
            TBRunSummary {
                id: "run-002".to_string(),
                task_id: "file-ops".to_string(),
                task_name: "File Operations".to_string(),
                status: TBRunStatus::Completed,
                outcome: Some(TBRunOutcome::Failure),
                duration_ms: Some(38200),
                steps: 10,
            },
            TBRunSummary {
                id: "run-003".to_string(),
                task_id: "api-client".to_string(),
                task_name: "API Client".to_string(),
                status: TBRunStatus::Running,
                outcome: None,
                duration_ms: None,
                steps: 7,
            },
        ];

        Self {
            stats,
            recent_runs,
            loading: false,
            focus_handle: cx.focus_handle(),
        }
    }

    fn render_kpi_card(&self, title: &str, value: String, subtitle: Option<String>) -> impl IntoElement {
        let title = title.to_string();

        div()
            .flex_1()
            .p(px(16.0))
            .bg(bg::CARD)
            .border_1()
            .border_color(border::DEFAULT)
            .rounded(px(8.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::MUTED)
                            .child(title)
                    )
                    .child(
                        div()
                            .text_size(px(28.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::BRIGHT)
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(value)
                    )
                    .when_some(subtitle, |el, subtitle| {
                        el.child(
                            div()
                                .text_size(px(10.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child(subtitle)
                        )
                    })
            )
    }

    fn render_run_row(&self, run: &TBRunSummary) -> impl IntoElement {
        let (status_color, status_text) = match run.status {
            TBRunStatus::Queued => (text::MUTED, "Queued"),
            TBRunStatus::Running => (status::RUNNING, "Running"),
            TBRunStatus::Completed => (text::PRIMARY, "Completed"),
            TBRunStatus::Error => (status::ERROR, "Error"),
        };

        let (outcome_color, outcome_icon) = match run.outcome {
            Some(TBRunOutcome::Success) => (status::SUCCESS, "✓"),
            Some(TBRunOutcome::Failure) => (status::ERROR, "✗"),
            Some(TBRunOutcome::Timeout) => (status::WARNING, "⏱"),
            Some(TBRunOutcome::Error) => (status::ERROR, "!"),
            Some(TBRunOutcome::Aborted) => (text::MUTED, "−"),
            None => (text::MUTED, "◦"),
        };

        let duration_text = run.duration_ms.map(|ms| format!("{:.1}s", ms as f64 / 1000.0)).unwrap_or_else(|| "--".to_string());

        div()
            .flex()
            .items_center()
            .gap(px(12.0))
            .px(px(12.0))
            .py(px(10.0))
            .bg(bg::ROW)
            .border_b_1()
            .border_color(border::SUBTLE)
            .hover(|el| el.bg(bg::HOVER))
            .cursor_pointer()
            .child(
                div()
                    .w(px(24.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(16.0))
                    .text_color(outcome_color)
                    .child(outcome_icon)
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::PRIMARY)
                            .child(run.task_name.clone())
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::MUTED)
                            .child(format!("{} steps", run.steps))
                            .child("•")
                            .child(duration_text)
                    )
            )
            .child(
                div()
                    .px(px(8.0))
                    .py(px(4.0))
                    .text_size(px(10.0))
                    .font_family(FONT_FAMILY)
                    .text_color(status_color)
                    .child(status_text)
            )
    }

    fn render_quick_action(&self, label: &str, description: &str) -> impl IntoElement {
        let label = label.to_string();
        let description = description.to_string();

        div()
            .px(px(16.0))
            .py(px(12.0))
            .bg(bg::HOVER)
            .border_1()
            .border_color(border::DEFAULT)
            .rounded(px(6.0))
            .cursor_pointer()
            .hover(|el| el.bg(bg::CARD).border_color(border::SELECTED))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::PRIMARY)
                            .font_weight(FontWeight::MEDIUM)
                            .child(label)
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::MUTED)
                            .child(description)
                    )
            )
    }
}

impl Focusable for DashboardView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for DashboardView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .h_full()
            .w_full()
            .bg(bg::APP)
            .p(px(20.0))
            .gap(px(20.0))
            // KPI Grid
            .child(
                div()
                    .flex()
                    .gap(px(16.0))
                    .when_some(self.stats.as_ref(), |el, stats| {
                        el
                            .child(self.render_kpi_card(
                                "Success Rate",
                                format!("{:.1}%", stats.success_rate),
                                Some(format!("{} total runs", stats.total_runs))
                            ))
                            .child(self.render_kpi_card(
                                "Avg Steps",
                                format!("{:.1}", stats.avg_steps),
                                Some("per successful run".to_string())
                            ))
                            .child(self.render_kpi_card(
                                "Avg Duration",
                                format!("{:.1}s", stats.avg_duration),
                                Some("per run".to_string())
                            ))
                            .child(self.render_kpi_card(
                                "Total Runs",
                                format!("{}", stats.total_runs),
                                Some("all time".to_string())
                            ))
                    })
            )
            // Recent Runs Section
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::PRIMARY)
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Recent Runs")
                    )
                    .child(
                        div()
                            .bg(bg::SURFACE)
                            .border_1()
                            .border_color(border::DEFAULT)
                            .rounded(px(8.0))
                            .overflow_hidden()
                            .children(self.recent_runs.iter().map(|run| self.render_run_row(run)))
                    )
            )
            // Quick Actions
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::PRIMARY)
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Quick Actions")
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(12.0))
                            .child(self.render_quick_action(
                                "Run Benchmark",
                                "Execute full TB2 benchmark suite"
                            ))
                            .child(self.render_quick_action(
                                "Run Single Task",
                                "Test one task from the suite"
                            ))
                            .child(self.render_quick_action(
                                "View All Tasks",
                                "Browse available benchmark tasks"
                            ))
                    )
            )
    }
}
