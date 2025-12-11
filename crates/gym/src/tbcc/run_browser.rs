//! TBCC Run Browser Tab - Browse execution history

use gpui::prelude::*;
use gpui::*;
use std::sync::{Arc, RwLock};
use theme_oa::{bg, border, status, text, FONT_FAMILY};

use super::types::{TBRunSummary, TBRunOutcome, format_duration};
use crate::services::RunStore;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataSource {
    All,
    Local,
    HuggingFace,
}

impl DataSource {
    fn label(&self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Local => "Local",
            Self::HuggingFace => "HuggingFace",
        }
    }
}

pub struct RunBrowserView {
    runs: Vec<TBRunSummary>,
    selected_run_id: Option<String>,
    data_source: DataSource,
    loading: bool,
    run_store: Option<Arc<RwLock<RunStore>>>,
    focus_handle: FocusHandle,
}

impl RunBrowserView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            runs: vec![],
            selected_run_id: None,
            data_source: DataSource::All,
            loading: false,
            run_store: None,
            focus_handle: cx.focus_handle(),
        }
    }

    /// Set the run store and refresh data
    pub fn set_run_store(&mut self, store: Arc<RwLock<RunStore>>, cx: &mut Context<Self>) {
        self.run_store = Some(store);
        self.refresh(cx);
    }

    /// Refresh runs from the run store
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if let Some(ref store) = self.run_store {
            if let Ok(guard) = store.read() {
                self.runs = guard.get_all_runs();
            }
        }
        cx.notify();
    }

    #[allow(dead_code)]
    fn select_run(&mut self, run_id: String, cx: &mut Context<Self>) {
        self.selected_run_id = Some(run_id);
        cx.notify();
    }

    fn set_data_source(&mut self, source: DataSource, cx: &mut Context<Self>) {
        self.data_source = source;
        cx.notify();
    }

    fn outcome_color(&self, outcome: Option<TBRunOutcome>) -> (Hsla, Hsla) {
        match outcome {
            Some(TBRunOutcome::Success) => (status::SUCCESS, status::SUCCESS_BG),
            Some(TBRunOutcome::Failure) => (status::ERROR, status::ERROR_BG),
            Some(TBRunOutcome::Error) => (status::ERROR, status::ERROR_BG),
            Some(TBRunOutcome::Timeout) => (status::WARNING, status::WARNING_BG),
            Some(TBRunOutcome::Aborted) => (text::MUTED, bg::ELEVATED),
            None => (status::INFO, status::INFO_BG), // Running
        }
    }

    fn format_date(&self, iso: &str) -> String {
        // Simple date formatting - just extract readable parts
        if iso.len() >= 16 {
            let date = &iso[5..10]; // MM-DD
            let time = &iso[11..16]; // HH:MM
            format!("{} {}", date, time)
        } else {
            iso[..10.min(iso.len())].to_string()
        }
    }

    fn render_source_filters(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let sources = [DataSource::All, DataSource::Local, DataSource::HuggingFace];

        div()
            .flex()
            .bg(bg::ELEVATED)
            .rounded(px(6.0))
            .p(px(2.0))
            .border_1()
            .border_color(border::DEFAULT)
            .children(sources.iter().map(|&source| {
                let is_active = self.data_source == source;

                div()
                    .flex_1()
                    .px(px(12.0))
                    .py(px(6.0))
                    .rounded(px(4.0))
                    .text_size(px(11.0))
                    .font_family(FONT_FAMILY)
                    .font_weight(FontWeight::MEDIUM)
                    .cursor_pointer()
                    .text_center()
                    .when(is_active, |el| {
                        el.bg(bg::CARD)
                            .text_color(text::BRIGHT)
                            .shadow_sm()
                    })
                    .when(!is_active, |el| {
                        el.text_color(text::MUTED)
                            .hover(|el| el.text_color(text::PRIMARY))
                    })
                    .on_mouse_down(MouseButton::Left, cx.listener(move |view, _event, _window, cx| {
                        view.set_data_source(source, cx);
                    }))
                    .child(source.label())
            }))
    }

    fn render_run_row(&self, run: &TBRunSummary) -> impl IntoElement {
        let is_selected = self.selected_run_id.as_ref() == Some(&run.id);
        let (text_color, bg_color) = self.outcome_color(run.outcome);

        let outcome_label = match run.outcome {
            Some(TBRunOutcome::Success) => "success",
            Some(TBRunOutcome::Failure) => "failed",
            Some(TBRunOutcome::Error) => "error",
            Some(TBRunOutcome::Timeout) => "timeout",
            Some(TBRunOutcome::Aborted) => "aborted",
            None => "running",
        };

        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .p(px(12.0))
            .cursor_pointer()
            .border_b_1()
            .border_color(border::SUBTLE)
            .when(is_selected, |el| {
                el.bg(bg::SELECTED)
            })
            .when(!is_selected, |el| {
                el.bg(bg::ROW)
                    .hover(|el| el.bg(bg::HOVER))
            })
            // Header row
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(8.0))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(13.0))
                            .font_family(FONT_FAMILY)
                            .text_color(if is_selected { text::BRIGHT } else { text::PRIMARY })
                            .font_weight(FontWeight::MEDIUM)
                            .overflow_hidden()
                            .child(run.task_name.clone())
                    )
                    .child(
                        div()
                            .px(px(6.0))
                            .py(px(2.0))
                            .rounded(px(4.0))
                            .text_size(px(9.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text_color)
                            .bg(bg_color)
                            .font_weight(FontWeight::MEDIUM)
                            .child(outcome_label.to_uppercase())
                    )
            )
            // Stats row
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_size(px(11.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::MUTED)
                    .child(self.format_date(&run.started_at))
                    .child(
                        div()
                            .flex()
                            .gap(px(12.0))
                            .child(format!("{} steps", run.steps_count))
                            .child(format_duration(run.duration_ms))
                    )
            )
            // Source indicator
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .text_size(px(10.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::DISABLED)
                    .child(
                        div()
                            .w(px(6.0))
                            .h(px(6.0))
                            .rounded_full()
                            .bg(status::INFO.opacity(0.5))
                    )
                    .child("Local Run")
            )
    }

    fn render_run_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let list_content = if self.loading {
            div()
                .p(px(32.0))
                .text_center()
                .text_size(px(13.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child("Loading runs...")
                .into_any_element()
        } else if self.runs.is_empty() {
            div()
                .p(px(32.0))
                .text_center()
                .text_size(px(13.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child("No runs found")
                .into_any_element()
        } else {
            div()
                .children(self.runs.iter().map(|run| self.render_run_row(run)))
                .into_any_element()
        };

        div()
            .w(px(380.0))
            .h_full()
            .flex()
            .flex_col()
            .bg(bg::SURFACE)
            .border_r_1()
            .border_color(border::DEFAULT)
            // Header
            .child(
                div()
                    .p(px(16.0))
                    .border_b_1()
                    .border_color(border::DEFAULT)
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    // Title row
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::PRIMARY)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Run History")
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::MUTED)
                                    .cursor_pointer()
                                    .hover(|el| el.text_color(text::PRIMARY))
                                    .child("↻ Refresh")
                            )
                    )
                    // Source filters
                    .child(self.render_source_filters(cx))
            )
            // Run list
            .child(
                div()
                    .id("run-list-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .child(list_content)
            )
    }

    fn render_run_detail(&self) -> impl IntoElement {
        let selected_run = self.selected_run_id.as_ref()
            .and_then(|id| self.runs.iter().find(|r| &r.id == id));

        match selected_run {
            Some(run) => {
                let (outcome_text, _outcome_bg) = self.outcome_color(run.outcome);
                let outcome_label = match run.outcome {
                    Some(TBRunOutcome::Success) => "Success",
                    Some(TBRunOutcome::Failure) => "Failed",
                    Some(TBRunOutcome::Error) => "Error",
                    Some(TBRunOutcome::Timeout) => "Timeout",
                    Some(TBRunOutcome::Aborted) => "Aborted",
                    None => "Running",
                };

                div()
                    .id("run-detail-scroll")
                    .flex_1()
                    .h_full()
                    .overflow_y_scroll()
                    .bg(bg::APP)
                    // Header
                    .child(
                        div()
                            .px(px(24.0))
                            .py(px(16.0))
                            .border_b_1()
                            .border_color(border::DEFAULT)
                            .bg(bg::SURFACE)
                            // Title row
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .mb(px(8.0))
                                    .child(
                                        div()
                                            .text_size(px(18.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(text::BRIGHT)
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(run.task_name.clone())
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(8.0))
                                            .child(
                                                div()
                                                    .text_size(px(11.0))
                                                    .font_family(FONT_FAMILY)
                                                    .text_color(text::DISABLED)
                                                    .child(run.id.clone())
                                            )
                                            .child(
                                                div()
                                                    .px(px(8.0))
                                                    .py(px(4.0))
                                                    .rounded(px(4.0))
                                                    .text_size(px(11.0))
                                                    .font_family(FONT_FAMILY)
                                                    .text_color(status::INFO)
                                                    .bg(status::INFO_BG)
                                                    .border_1()
                                                    .border_color(status::INFO.opacity(0.3))
                                                    .child("Local")
                                            )
                                    )
                            )
                            // Stats row
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(24.0))
                                    .text_size(px(13.0))
                                    .font_family(FONT_FAMILY)
                                    // Status
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(6.0))
                                            .child(
                                                div()
                                                    .text_color(text::MUTED)
                                                    .child("Status:")
                                            )
                                            .child(
                                                div()
                                                    .text_color(outcome_text)
                                                    .child(outcome_label)
                                            )
                                    )
                                    // Steps
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(6.0))
                                            .child(
                                                div()
                                                    .text_color(text::MUTED)
                                                    .child("Steps:")
                                            )
                                            .child(
                                                div()
                                                    .text_color(text::PRIMARY)
                                                    .child(format!("{}", run.steps_count))
                                            )
                                    )
                                    // Duration
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(6.0))
                                            .child(
                                                div()
                                                    .text_color(text::MUTED)
                                                    .child("Duration:")
                                            )
                                            .child(
                                                div()
                                                    .text_color(text::PRIMARY)
                                                    .child(format_duration(run.duration_ms))
                                            )
                                    )
                                    // Tokens
                                    .when_some(run.tokens_used, |el, tokens| {
                                        el.child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(6.0))
                                                .child(
                                                    div()
                                                        .text_color(text::MUTED)
                                                        .child("Tokens:")
                                                )
                                                .child(
                                                    div()
                                                        .text_color(text::PRIMARY)
                                                        .child(format!("{}", tokens))
                                                )
                                        )
                                    })
                            )
                    )
                    // Steps section
                    .child(
                        div()
                            .p(px(24.0))
                            // Section header
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::MUTED)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .mb(px(16.0))
                                    .child("EXECUTION STEPS")
                            )
                            // Placeholder steps
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(12.0))
                                    .children((0..run.steps_count.min(5)).map(|i| {
                                        let is_success = i < run.steps_count - 1 || run.outcome == Some(TBRunOutcome::Success);
                                        let border_color = if is_success {
                                            status::SUCCESS.opacity(0.3)
                                        } else {
                                            status::ERROR.opacity(0.3)
                                        };

                                        div()
                                            .bg(bg::CARD)
                                            .border_1()
                                            .border_color(border_color)
                                            .rounded(px(8.0))
                                            .overflow_hidden()
                                            // Step header
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .justify_between()
                                                    .px(px(16.0))
                                                    .py(px(10.0))
                                                    .bg(bg::SURFACE)
                                                    .border_b_1()
                                                    .border_color(border::SUBTLE)
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .items_center()
                                                            .gap(px(12.0))
                                                            .child(
                                                                div()
                                                                    .text_size(px(11.0))
                                                                    .font_family(FONT_FAMILY)
                                                                    .text_color(text::DISABLED)
                                                                    .child(format!("#{}", i + 1))
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_size(px(13.0))
                                                                    .font_family(FONT_FAMILY)
                                                                    .text_color(text::PRIMARY)
                                                                    .child(format!("Step {}", i + 1))
                                                            )
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(px(11.0))
                                                            .font_family(FONT_FAMILY)
                                                            .text_color(text::MUTED)
                                                            .child("~2s")
                                                    )
                                            )
                                            // Step content placeholder
                                            .child(
                                                div()
                                                    .p(px(16.0))
                                                    .child(
                                                        div()
                                                            .text_size(px(12.0))
                                                            .font_family(FONT_FAMILY)
                                                            .text_color(text::MUTED)
                                                            .child("Step details would appear here...")
                                                    )
                                            )
                                    }))
                            )
                            // Show more indicator
                            .when(run.steps_count > 5, |el| {
                                el.child(
                                    div()
                                        .mt(px(12.0))
                                        .text_center()
                                        .text_size(px(12.0))
                                        .font_family(FONT_FAMILY)
                                        .text_color(text::MUTED)
                                        .child(format!("+ {} more steps", run.steps_count - 5))
                                )
                            })
                    )
                    .into_any_element()
            }
            None => {
                // Empty state
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(bg::APP)
                    .child(
                        div()
                            .text_center()
                            .child(
                                div()
                                    .text_size(px(32.0))
                                    .text_color(text::DISABLED)
                                    .mb(px(12.0))
                                    .child("←")
                            )
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::MUTED)
                                    .child("Select a run to view details")
                            )
                    )
                    .into_any_element()
            }
        }
    }
}

impl Focusable for RunBrowserView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for RunBrowserView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .h_full()
            .w_full()
            .bg(bg::APP)
            // Left panel: Run list
            .child(self.render_run_list(cx))
            // Right panel: Run detail
            .child(self.render_run_detail())
    }
}
