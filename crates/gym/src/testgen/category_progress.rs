//! Category progress bars per test category

use gpui::prelude::*;
use gpui::*;
use std::sync::Arc;
use theme_oa::{bg, border, status, text, FONT_FAMILY};

/// Callback type for category selection
pub type OnCategorySelectCallback = Arc<dyn Fn(Option<TestCategory>, &mut Window, &mut App) + Send + Sync>;

/// Test category types (anti_cheat, existence, correctness, boundary, integration)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TestCategory {
    AntiCheat,
    Existence,
    Correctness,
    Boundary,
    Integration,
}

impl TestCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::AntiCheat => "Anti-Cheat",
            Self::Existence => "Existence",
            Self::Correctness => "Correctness",
            Self::Boundary => "Boundary",
            Self::Integration => "Integration",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::AntiCheat => "Prevent hardcoded solutions",
            Self::Existence => "Basic functionality exists",
            Self::Correctness => "Outputs are correct",
            Self::Boundary => "Edge cases handled",
            Self::Integration => "Components work together",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::AntiCheat => "AC",
            Self::Existence => "EX",
            Self::Correctness => "CO",
            Self::Boundary => "BO",
            Self::Integration => "IN",
        }
    }
}

/// Stats for a single category
#[derive(Debug, Clone)]
pub struct CategoryStats {
    pub category: TestCategory,
    pub generated: u32,
    pub target: u32,
    pub passed: u32,
}

impl CategoryStats {
    pub fn progress(&self) -> f32 {
        if self.target == 0 { 0.0 } else { self.generated as f32 / self.target as f32 }
    }

    pub fn pass_rate(&self) -> f32 {
        if self.generated == 0 { 0.0 } else { self.passed as f32 / self.generated as f32 }
    }
}

/// Category progress display component
pub struct CategoryProgress {
    stats: Vec<CategoryStats>,
    focus_handle: FocusHandle,
    selected_category: Option<TestCategory>,
    on_category_select: Option<OnCategorySelectCallback>,
}

impl CategoryProgress {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            stats: Vec::new(),
            focus_handle: cx.focus_handle(),
            selected_category: None,
            on_category_select: None,
        }
    }

    pub fn set_stats(&mut self, stats: Vec<CategoryStats>) {
        self.stats = stats;
    }

    pub fn set_on_category_select(&mut self, callback: OnCategorySelectCallback) {
        self.on_category_select = Some(callback);
    }

    pub fn clear_filter(&mut self) {
        self.selected_category = None;
    }

    fn render_clickable_summary(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let total_generated: u32 = self.stats.iter().map(|s| s.generated).sum();
        let total_target: u32 = self.stats.iter().map(|s| s.target).sum();
        let total_passed: u32 = self.stats.iter().map(|s| s.passed).sum();
        let is_selected = self.selected_category.is_none();

        div()
            .id("category-summary")
            .flex()
            .items_center()
            .justify_between()
            .px(px(16.0))
            .py(px(12.0))
            .bg(if is_selected {
                bg::SELECTED
            } else {
                bg::ELEVATED
            })
            .border_b_1()
            .border_color(border::DEFAULT)
            .cursor_pointer()
            .hover(|el| el.bg(if is_selected { bg::SELECTED } else { bg::HOVER }))
            .on_click(cx.listener(|this, _evt, window, cx| {
                this.selected_category = None;
                if let Some(callback) = &this.on_category_select {
                    callback(None, window, cx);
                }
                cx.notify();
            }))
            .child(
                div()
                    .text_size(px(11.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::MUTED)
                    .child("All Categories"),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::PRIMARY)
                            .font_weight(FontWeight::MEDIUM)
                            .child(format!("{}/{} tests", total_generated, total_target)),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .font_family(FONT_FAMILY)
                            .text_color(status::SUCCESS)
                            .child(format!("{} passing", total_passed)),
                    ),
            )
    }

    fn render_clickable_category_row(
        &self,
        stats: &CategoryStats,
        idx: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let progress = stats.progress();
        let progress_width = (progress.min(1.0) * 140.0).max(4.0);
        let is_complete = stats.generated >= stats.target;
        let category = stats.category;
        let is_selected = self.selected_category == Some(category);

        let bar_color = if is_complete {
            status::SUCCESS
        } else if stats.generated > 0 {
            status::INFO
        } else {
            text::DISABLED
        };

        let category_label = stats.category.label().to_string();
        let icon = stats.category.icon().to_string();
        let generated = stats.generated;
        let target = stats.target;
        let passed = stats.passed;
        let pass_rate = stats.pass_rate();

        div()
            .id(ElementId::Name(format!("category-row-{}", idx).into()))
            .flex()
            .flex_col()
            .gap(px(6.0))
            .px(px(16.0))
            .py(px(10.0))
            .border_b_1()
            .border_color(border::SUBTLE)
            .bg(if is_selected { bg::SELECTED } else { bg::SURFACE })
            .cursor_pointer()
            .hover(|el| el.bg(if is_selected { bg::SELECTED } else { bg::HOVER }))
            .when(is_selected, |el| {
                el.border_l_2().border_color(border::SELECTED)
            })
            .on_click(cx.listener(move |this, _evt, window, cx| {
                // Toggle: if already selected, clear; otherwise select
                if this.selected_category == Some(category) {
                    this.selected_category = None;
                    if let Some(callback) = &this.on_category_select {
                        callback(None, window, cx);
                    }
                } else {
                    this.selected_category = Some(category);
                    if let Some(callback) = &this.on_category_select {
                        callback(Some(category), window, cx);
                    }
                }
                cx.notify();
            }))
            // Header row
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .w(px(24.0))
                                    .h(px(24.0))
                                    .rounded(px(4.0))
                                    .bg(bg::ELEVATED)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        div()
                                            .text_size(px(9.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(text::MUTED)
                                            .font_weight(FontWeight::BOLD)
                                            .child(icon),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(if is_selected {
                                        text::BRIGHT
                                    } else {
                                        text::PRIMARY
                                    })
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(category_label),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(if is_complete {
                                status::SUCCESS
                            } else {
                                text::MUTED
                            })
                            .child(format!("{}/{}", generated, target)),
                    ),
            )
            // Progress bar
            .child(
                div()
                    .w_full()
                    .h(px(4.0))
                    .bg(bg::ELEVATED)
                    .rounded(px(2.0))
                    .overflow_hidden()
                    .child(
                        div()
                            .w(px(progress_width))
                            .h_full()
                            .bg(bar_color)
                            .rounded(px(2.0)),
                    ),
            )
            // Pass rate
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_size(px(10.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::DISABLED)
                            .child(format!("{} passed", passed)),
                    )
                    .when(generated > 0, |el| {
                        el.child(
                            div()
                                .text_size(px(10.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::DISABLED)
                                .child(format!("({:.0}%)", pass_rate * 100.0)),
                        )
                    }),
            )
    }
}

impl Focusable for CategoryProgress {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for CategoryProgress {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let stats_clone: Vec<_> = self.stats.clone();

        // Pre-render all category rows using a for loop (closures can't capture cx)
        let mut rows = Vec::with_capacity(stats_clone.len());
        for (idx, stats) in stats_clone.iter().enumerate() {
            rows.push(self.render_clickable_category_row(stats, idx, cx));
        }

        // Pre-render summary (must come after rows due to borrow rules)
        let summary = self.render_clickable_summary(cx);

        div()
            .id("category-progress-scroll")
            .h_full()
            .w_full()
            .overflow_y_scroll()
            .bg(bg::SURFACE)
            // Summary - clicking clears filter
            .child(summary)
            // Category rows
            .children(rows)
    }
}
