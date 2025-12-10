//! Compute Market view - Main view for the Compute tab

use gpui::*;
use theme::bg;

use crate::types::{ActiveModel, TimeRange, EarningsDataPoint};
use super::go_online::render_go_online_panel;
use super::earnings_chart::{render_earnings_chart, mock_earnings_data};
use super::model_list::{render_model_list, render_network_stats, mock_models};

/// State for the Compute Market view
pub struct ComputeMarketState {
    pub is_online: bool,
    pub models_configured: u32,
    pub active_models: Vec<ActiveModel>,
    pub earnings_data: Vec<EarningsDataPoint>,
    pub selected_time_range: TimeRange,
    pub earnings_today_sats: u64,
    pub earnings_week_sats: u64,
    pub earnings_total_sats: u64,
    pub connected_relays: u32,
    pub pending_jobs: u32,
    pub completed_today: u64,
}

impl Default for ComputeMarketState {
    fn default() -> Self {
        Self {
            is_online: false,
            models_configured: 3,
            active_models: mock_models(),
            earnings_data: mock_earnings_data(),
            selected_time_range: TimeRange::Week,
            earnings_today_sats: 1_247,
            earnings_week_sats: 8_932,
            earnings_total_sats: 142_847,
            connected_relays: 3,
            pending_jobs: 2,
            completed_today: 127,
        }
    }
}

/// Render the Compute Market view
pub fn render_compute_market(state: &ComputeMarketState) -> impl IntoElement {
    div()
        .id("compute-market")
        .flex_1()
        .h_full()
        .flex()
        .flex_col()
        .gap(px(16.0))
        .p(px(16.0))
        .bg(bg::APP)
        .overflow_y_scroll()
        // Go Online panel
        .child(render_go_online_panel(state.is_online, state.models_configured))
        // Earnings chart
        .child(render_earnings_chart(
            &state.earnings_data,
            state.selected_time_range,
            state.earnings_week_sats,
        ))
        // Active models list
        .child(render_section_header("ACTIVE MODELS"))
        .child(render_model_list(&state.active_models))
        // Network stats
        .child(render_section_header("NETWORK STATUS"))
        .child(render_network_stats(
            state.connected_relays,
            state.pending_jobs,
            state.completed_today,
        ))
}

/// Render a section header
fn render_section_header(title: &str) -> impl IntoElement {
    div()
        .text_size(px(12.0))
        .font_family(theme::FONT_FAMILY)
        .text_color(theme::text::MUTED)
        .child(title.to_string())
}
