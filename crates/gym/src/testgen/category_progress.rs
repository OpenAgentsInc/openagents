//! Category progress bars per test category

use gpui::prelude::*;
use gpui::*;
use theme::{bg, border, status, text, FONT_FAMILY};

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
}

impl CategoryProgress {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            stats: Vec::new(),
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn set_stats(&mut self, stats: Vec<CategoryStats>) {
        self.stats = stats;
    }

    fn render_category_row(&self, stats: &CategoryStats) -> impl IntoElement {
        let progress = stats.progress();
        let progress_width = (progress.min(1.0) * 140.0).max(4.0);
        let is_complete = stats.generated >= stats.target;

        let bar_color = if is_complete {
            status::SUCCESS
        } else if stats.generated > 0 {
            status::INFO
        } else {
            text::DISABLED
        };

        let category_label = stats.category.label().to_string();
        let icon = stats.category.icon().to_string();

        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .px(px(16.0))
            .py(px(10.0))
            .border_b_1()
            .border_color(border::SUBTLE)
            .hover(|el| el.bg(bg::HOVER))
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
                                            .child(icon)
                                    )
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::PRIMARY)
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(category_label)
                            )
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(if is_complete { status::SUCCESS } else { text::MUTED })
                            .child(format!("{}/{}", stats.generated, stats.target))
                    )
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
                            .rounded(px(2.0))
                    )
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
                            .child(format!("{} passed", stats.passed))
                    )
                    .when(stats.generated > 0, |el| {
                        el.child(
                            div()
                                .text_size(px(10.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::DISABLED)
                                .child(format!("({:.0}%)", stats.pass_rate() * 100.0))
                        )
                    })
            )
    }

    fn render_summary(&self) -> impl IntoElement {
        let total_generated: u32 = self.stats.iter().map(|s| s.generated).sum();
        let total_target: u32 = self.stats.iter().map(|s| s.target).sum();
        let total_passed: u32 = self.stats.iter().map(|s| s.passed).sum();

        div()
            .flex()
            .items_center()
            .justify_between()
            .px(px(16.0))
            .py(px(12.0))
            .bg(bg::ELEVATED)
            .border_b_1()
            .border_color(border::DEFAULT)
            .child(
                div()
                    .text_size(px(11.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::MUTED)
                    .child("Total Coverage")
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
                            .child(format!("{}/{} tests", total_generated, total_target))
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .font_family(FONT_FAMILY)
                            .text_color(status::SUCCESS)
                            .child(format!("{} passing", total_passed))
                    )
            )
    }
}

impl Focusable for CategoryProgress {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for CategoryProgress {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("category-progress-scroll")
            .h_full()
            .w_full()
            .overflow_y_scroll()
            .bg(bg::SURFACE)
            // Summary
            .child(self.render_summary())
            // Category rows
            .children(self.stats.iter().map(|stats| {
                self.render_category_row(stats)
            }))
    }
}
