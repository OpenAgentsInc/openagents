/// Production Stats Dashboard - GPUI Implementation
///
/// Factorio-inspired production statistics panel.
/// Shows agent performance, bottlenecks, earnings, trends.
///
/// Features:
/// - Real-time metrics (earnings, throughput, efficiency)
/// - Sparklines showing trends
/// - Bottleneck detector
/// - Sortable agent performance table
/// - Activity feed

use gpui::*;

// ============================================================================
// Types
// ============================================================================

#[derive(Clone)]
pub struct ProductionMetrics {
    pub total_earnings: f64,      // Total sats earned
    pub earnings_today: f64,       // Sats earned today
    pub jobs_completed: u32,       // Total jobs
    pub jobs_today: u32,           // Jobs today
    pub jobs_per_hour: f64,        // Current throughput
    pub avg_cost_per_job: f64,     // Average cost
    pub success_rate: f64,         // % successful
    pub active_agents: u32,        // Currently busy
    pub total_agents: u32,         // Total available
}

#[derive(Clone)]
pub struct AgentPerformance {
    pub id: String,
    pub name: String,
    pub jobs_completed: u32,
    pub success_rate: f64,
    pub avg_time: f64,      // seconds
    pub total_cost: f64,    // sats
    pub status: String,     // "BUSY", "IDLE", "ERROR"
    pub bottleneck: bool,   // Is this agent a bottleneck?
}

#[derive(Clone)]
pub struct ActivityEvent {
    pub timestamp: String,
    pub agent: String,
    pub event_type: String, // "Started", "Completed", "Failed"
    pub message: String,
}

#[derive(Clone)]
pub struct ProductionStatsState {
    pub metrics: ProductionMetrics,
    pub agent_performance: Vec<AgentPerformance>,
    pub activity_feed: Vec<ActivityEvent>,
    pub earnings_history: Vec<f64>, // Last 24 hours
}

// ============================================================================
// View
// ============================================================================

pub struct ProductionStatsView {
    state: Entity<ProductionStatsState>,
}

impl ProductionStatsView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let state = cx.new(|_cx| ProductionStatsState {
            metrics: ProductionMetrics {
                total_earnings: 1234.0,
                earnings_today: 47.2,
                jobs_completed: 482,
                jobs_today: 23,
                jobs_per_hour: 47.3,
                avg_cost_per_job: 0.12,
                success_rate: 98.4,
                active_agents: 3,
                total_agents: 10,
            },
            agent_performance: vec![],
            activity_feed: vec![],
            earnings_history: vec![],
        });
        Self { state }
    }

    /// Render big metric card
    fn render_metric_card(&self, label: &str, value: String, trend: Option<String>) -> Div {
        div()
            .flex_1()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .px(px(16.0))
            .py(px(12.0))
            .bg(rgba(0x1a1a1a, 0.6))
            .border_1()
            .border_color(rgba(0xffffff, 0.1))
            .rounded(px(8.0))
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(rgb(0x888888))
                    .text_transform(TextTransform::Uppercase)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text(label)
            )
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(32.0))
                            .text_color(rgb(0xffffff))
                            .font_weight(FontWeight::BOLD)
                            .font_family(".AppleSystemUIFontMonospaced")
                            .text(&value)
                    )
                    .when_some(trend, |div, trend_text| {
                        div.child(
                            div()
                                .text_size(px(14.0))
                                .text_color(rgb(0x00ff00))
                                .font_family(".AppleSystemUIFontMonospaced")
                                .text(trend_text)
                        )
                    })
            )
    }

    /// Render agent performance row
    fn render_agent_row(&self, agent: &AgentPerformance) -> Div {
        let status_color = match agent.status.as_str() {
            "BUSY" => rgb(0x00ff00),
            "IDLE" => rgb(0x4a9eff),
            "ERROR" => rgb(0xff0000),
            _ => rgb(0x666666),
        };

        div()
            .flex()
            .items_center()
            .gap(px(12.0))
            .px(px(12.0))
            .py(px(10.0))
            .border_b_1()
            .border_color(rgba(0xffffff, 0.05))
            .hover(|style| style.bg(rgba(0x2a2a2a, 0.3)))
            .when(agent.bottleneck, |div| {
                div.border_l_4().border_color(rgb(0xff0000))
            })
            // Name
            .child(
                div()
                    .w(px(150.0))
                    .text_size(px(12.0))
                    .text_color(rgb(0xffffff))
                    .font_weight(FontWeight::MEDIUM)
                    .text(&agent.name)
            )
            // Status
            .child(
                div()
                    .w(px(60.0))
                    .text_size(px(10.0))
                    .text_color(status_color)
                    .font_weight(FontWeight::BOLD)
                    .font_family(".AppleSystemUIFontMonospaced")
                    .text(&agent.status)
            )
            // Jobs
            .child(
                div()
                    .w(px(60.0))
                    .text_size(px(12.0))
                    .text_color(rgb(0xffffff))
                    .font_family(".AppleSystemUIFontMonospaced")
                    .text(format!("{}", agent.jobs_completed))
            )
            // Success Rate
            .child(
                div()
                    .w(px(70.0))
                    .text_size(px(12.0))
                    .text_color(rgb(0x00ff00))
                    .font_family(".AppleSystemUIFontMonospaced")
                    .text(format!("{:.1}%", agent.success_rate))
            )
            // Avg Time
            .child(
                div()
                    .w(px(80.0))
                    .text_size(px(12.0))
                    .text_color(rgb(0xffffff))
                    .font_family(".AppleSystemUIFontMonospaced")
                    .text(format!("{:.1}s", agent.avg_time))
            )
            // Cost
            .child(
                div()
                    .flex_1()
                    .text_size(px(12.0))
                    .text_color(rgb(0xffffff))
                    .font_family(".AppleSystemUIFontMonospaced")
                    .text(format!("${:.2}", agent.total_cost))
            )
            // Bottleneck indicator
            .when(agent.bottleneck, |div| {
                div.child(
                    div()
                        .px(px(8.0))
                        .py(px(4.0))
                        .text_size(px(9.0))
                        .text_color(rgb(0xff0000))
                        .bg(rgba(0xff0000, 0.2))
                        .border_1()
                        .border_color(rgb(0xff0000))
                        .rounded(px(4.0))
                        .font_weight(FontWeight::BOLD)
                        .text("BOTTLENECK")
                )
            })
    }

    /// Render activity feed item
    fn render_activity_item(&self, event: &ActivityEvent) -> Div {
        let event_color = match event.event_type.as_str() {
            "Started" => rgb(0x4a9eff),
            "Completed" => rgb(0x00ff00),
            "Failed" => rgb(0xff0000),
            _ => rgb(0x888888),
        };

        div()
            .flex()
            .gap(px(12.0))
            .px(px(12.0))
            .py(px(8.0))
            .border_b_1()
            .border_color(rgba(0xffffff, 0.05))
            .child(
                div()
                    .text_size(px(10.0))
                    .text_color(rgb(0x666666))
                    .font_family(".AppleSystemUIFontMonospaced")
                    .text(&event.timestamp)
            )
            .child(
                div()
                    .text_size(px(10.0))
                    .text_color(event_color)
                    .font_weight(FontWeight::BOLD)
                    .font_family(".AppleSystemUIFontMonospaced")
                    .text(&event.event_type)
            )
            .child(
                div()
                    .text_size(px(10.0))
                    .text_color(rgb(0xffffff))
                    .text(&event.message)
            )
    }
}

impl Render for ProductionStatsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .p(px(20.0))
            .bg(rgba(0x000000, 0.95))
            .h_full()
            .overflow_y_auto()
            // Title
            .child(
                div()
                    .text_size(px(24.0))
                    .text_color(rgb(0xffffff))
                    .font_weight(FontWeight::BOLD)
                    .text("Production Statistics")
            )
            // Top metrics
            .child(
                div()
                    .flex()
                    .gap(px(16.0))
                    .child(self.render_metric_card(
                        "Earnings Today",
                        format!("{:.1} sats", state.metrics.earnings_today),
                        Some("↑ +12%".to_string()),
                    ))
                    .child(self.render_metric_card(
                        "Jobs/Hour",
                        format!("{:.1}", state.metrics.jobs_per_hour),
                        Some("↑ +23%".to_string()),
                    ))
                    .child(self.render_metric_card(
                        "Success Rate",
                        format!("{:.1}%", state.metrics.success_rate),
                        None,
                    ))
                    .child(self.render_metric_card(
                        "Active Agents",
                        format!("{}/{}", state.metrics.active_agents, state.metrics.total_agents),
                        None,
                    ))
            )
            // Agent Performance Table
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(16.0))
                            .text_color(rgb(0xffffff))
                            .font_weight(FontWeight::BOLD)
                            .text("Agent Performance")
                    )
                    .child(
                        div()
                            .bg(rgba(0x1a1a1a, 0.6))
                            .border_1()
                            .border_color(rgba(0xffffff, 0.1))
                            .rounded(px(8.0))
                            .overflow_hidden()
                            // Table header
                            .child(
                                div()
                                    .flex()
                                    .gap(px(12.0))
                                    .px(px(12.0))
                                    .py(px(10.0))
                                    .bg(rgba(0x000000, 0.4))
                                    .border_b_1()
                                    .border_color(rgba(0xffffff, 0.1))
                                    .child(
                                        div()
                                            .w(px(150.0))
                                            .text_size(px(10.0))
                                            .text_color(rgb(0x888888))
                                            .text_transform(TextTransform::Uppercase)
                                            .font_weight(FontWeight::BOLD)
                                            .text("AGENT")
                                    )
                                    .child(
                                        div()
                                            .w(px(60.0))
                                            .text_size(px(10.0))
                                            .text_color(rgb(0x888888))
                                            .text_transform(TextTransform::Uppercase)
                                            .font_weight(FontWeight::BOLD)
                                            .text("STATUS")
                                    )
                                    .child(
                                        div()
                                            .w(px(60.0))
                                            .text_size(px(10.0))
                                            .text_color(rgb(0x888888))
                                            .text_transform(TextTransform::Uppercase)
                                            .font_weight(FontWeight::BOLD)
                                            .text("JOBS")
                                    )
                                    .child(
                                        div()
                                            .w(px(70.0))
                                            .text_size(px(10.0))
                                            .text_color(rgb(0x888888))
                                            .text_transform(TextTransform::Uppercase)
                                            .font_weight(FontWeight::BOLD)
                                            .text("SUCCESS")
                                    )
                                    .child(
                                        div()
                                            .w(px(80.0))
                                            .text_size(px(10.0))
                                            .text_color(rgb(0x888888))
                                            .text_transform(TextTransform::Uppercase)
                                            .font_weight(FontWeight::BOLD)
                                            .text("AVG TIME")
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .text_size(px(10.0))
                                            .text_color(rgb(0x888888))
                                            .text_transform(TextTransform::Uppercase)
                                            .font_weight(FontWeight::BOLD)
                                            .text("COST")
                                    )
                            )
                            // Table rows
                            .children(
                                state
                                    .agent_performance
                                    .iter()
                                    .map(|agent| self.render_agent_row(agent))
                                    .collect::<Vec<_>>()
                            )
                    )
            )
            // Activity Feed
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(16.0))
                            .text_color(rgb(0xffffff))
                            .font_weight(FontWeight::BOLD)
                            .text("Activity Feed")
                    )
                    .child(
                        div()
                            .bg(rgba(0x1a1a1a, 0.6))
                            .border_1()
                            .border_color(rgba(0xffffff, 0.1))
                            .rounded(px(8.0))
                            .overflow_hidden()
                            .max_h(px(300.0))
                            .overflow_y_auto()
                            .children(
                                state
                                    .activity_feed
                                    .iter()
                                    .map(|event| self.render_activity_item(event))
                                    .collect::<Vec<_>>()
                            )
                    )
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
            cx.open_window(WindowOptions::default(), |_, cx| cx.new(ProductionStatsView::new))
                .ok();
        });
    }
}
