/// Resource Bar Component - GPUI Implementation
///
/// StarCraft-inspired resource counter showing sats, tokens, jobs/hr.
/// Always visible at top of screen with color-coded warnings.
///
/// Features:
/// - Real-time updates
/// - Color-coded thresholds (green → orange → red)
/// - Trend indicators (↑ +12%)
/// - Click to view details

use gpui::*;

// ============================================================================
// Types
// ============================================================================

#[derive(Clone, Debug)]
pub struct ResourceMetric {
    pub icon: String,
    pub label: String,
    pub value: f64,
    pub max: Option<f64>,
    pub trend: Option<f64>, // Percentage change
    pub unit: String,
}

impl ResourceMetric {
    /// Get color based on usage percentage
    fn color(&self) -> Hsla {
        if let Some(max) = self.max {
            let ratio = self.value / max;
            if ratio < 0.7 {
                rgb(0x00ff00) // Green - healthy
            } else if ratio < 0.9 {
                rgb(0xffa500) // Orange - warning
            } else {
                rgb(0xff0000) // Red - critical
            }
        } else {
            rgb(0x00ff00) // No limit = green
        }
    }

    /// Format value with appropriate precision
    fn format_value(&self) -> String {
        if self.value >= 1000.0 {
            format!("{:.1}k", self.value / 1000.0)
        } else if self.value >= 1_000_000.0 {
            format!("{:.1}M", self.value / 1_000_000.0)
        } else {
            format!("{:.0}", self.value)
        }
    }

    /// Format trend as percentage
    fn format_trend(&self) -> Option<String> {
        self.trend.map(|t| {
            if t >= 0.0 {
                format!("↑ +{:.0}%", t)
            } else {
                format!("↓ {:.0}%", t)
            }
        })
    }
}

#[derive(Clone)]
pub struct ResourceBarState {
    pub metrics: Vec<ResourceMetric>,
}

impl Default for ResourceBarState {
    fn default() -> Self {
        Self {
            metrics: vec![
                ResourceMetric {
                    icon: "💰".to_string(),
                    label: "Sats".to_string(),
                    value: 1234.0,
                    max: None,
                    trend: Some(12.0),
                    unit: "sats".to_string(),
                },
                ResourceMetric {
                    icon: "🔥".to_string(),
                    label: "Tokens".to_string(),
                    value: 8500.0,
                    max: Some(10000.0),
                    trend: Some(-5.0),
                    unit: "tokens".to_string(),
                },
                ResourceMetric {
                    icon: "⚡".to_string(),
                    label: "Jobs/hr".to_string(),
                    value: 47.0,
                    max: None,
                    trend: Some(23.0),
                    unit: "/hr".to_string(),
                },
            ],
        }
    }
}

// ============================================================================
// View
// ============================================================================

pub struct ResourceBarView {
    state: Entity<ResourceBarState>,
}

impl ResourceBarView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let state = cx.new(|_cx| ResourceBarState::default());
        Self { state }
    }

    /// Update a specific metric
    pub fn update_metric(&mut self, label: &str, value: f64, trend: Option<f64>, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            if let Some(metric) = state.metrics.iter_mut().find(|m| m.label == label) {
                metric.value = value;
                metric.trend = trend;
            }
            cx.notify();
        });
    }

    /// Render a single resource metric
    fn render_metric(&self, metric: &ResourceMetric, cx: &mut Context<Self>) -> Div {
        let color = metric.color();
        let value_text = metric.format_value();
        let trend_text = metric.format_trend();

        let label = metric.label.clone();

        div()
            .flex()
            .items_center()
            .gap(px(12.0))
            .px(px(16.0))
            .py(px(8.0))
            .bg(rgba(0x1a1a1a, 0.6))
            .border_1()
            .border_color(rgba(0xffffff, 0.1))
            .rounded(px(6.0))
            .cursor_pointer()
            .hover(|style| style.bg(rgba(0x2a2a2a, 0.6)))
            .on_click(cx.listener(move |_this, _event, _window, _cx| {
                // In real app, open detail modal
                println!("Clicked {}", label);
            }))
            // Icon
            .child(
                div()
                    .text_size(px(20.0))
                    .text(&metric.icon)
            )
            // Label + Value
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(rgb(0x888888))
                            .text_transform(TextTransform::Uppercase)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text(&metric.label)
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_size(px(18.0))
                                    .text_color(color)
                                    .font_weight(FontWeight::BOLD)
                                    .font_family(".AppleSystemUIFontMonospaced")
                                    .text(&value_text)
                            )
                            .when_some(trend_text, |div, trend| {
                                div.child(
                                    div()
                                        .text_size(px(11.0))
                                        .text_color(if metric.trend.unwrap_or(0.0) >= 0.0 {
                                            rgb(0x00ff00)
                                        } else {
                                            rgb(0xff0000)
                                        })
                                        .font_family(".AppleSystemUIFontMonospaced")
                                        .text(trend)
                                )
                            })
                    )
            )
            // Progress bar (if has max)
            .when_some(metric.max, |div, max| {
                let ratio = metric.value / max;
                div.child(
                    div()
                        .flex_1()
                        .h(px(4.0))
                        .bg(rgba(0x333333, 0.6))
                        .rounded_full()
                        .overflow_hidden()
                        .child(
                            div()
                                .w(relative(ratio as f32))
                                .h_full()
                                .bg(color)
                        )
                )
            })
    }
}

impl Render for ResourceBarView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        div()
            .fixed()
            .top_0()
            .right_0()
            .flex()
            .gap(px(12.0))
            .px(px(20.0))
            .py(px(12.0))
            .bg(rgba(0x000000, 0.8))
            .border_b_1()
            .border_color(rgba(0xffffff, 0.1))
            .shadow_xl()
            .children(
                state
                    .metrics
                    .iter()
                    .map(|metric| self.render_metric(metric, cx))
                    .collect::<Vec<_>>()
            )
    }
}

// ============================================================================
// Usage Example
// ============================================================================

#[cfg(test)]
mod example {
    use super::*;

    fn example_usage() {
        Application::new().run(|cx: &mut App| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                let mut view = cx.new(ResourceBarView::new);

                // Simulate real-time updates
                view.update(cx, |view, cx| {
                    // Update sats balance
                    view.update_metric("Sats", 1500.0, Some(15.0), cx);

                    // Update token usage (approaching limit)
                    view.update_metric("Tokens", 9200.0, Some(-3.0), cx);

                    // Update jobs/hr (increasing)
                    view.update_metric("Jobs/hr", 52.0, Some(28.0), cx);
                });

                view
            })
            .ok();
        });
    }
}
