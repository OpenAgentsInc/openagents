//! Agent grid component - Grid layout of agent cards

use gpui::*;
use theme::{bg, border, text, FONT_FAMILY};

use crate::types::{AgentListing, AgentCategory, AgentSortOption, TrustTier};
use crate::text_input::TextInput;
use super::agent_card::render_agent_card;

/// Render the search bar with a real TextInput
pub fn render_search_bar_with_input(
    search_input: Entity<TextInput>,
    selected_category: AgentCategory,
    selected_sort: AgentSortOption,
) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .items_center()
        .gap(px(12.0))
        .p(px(16.0))
        // Search input - real text input!
        .child(
            div()
                .flex_1()
                .flex()
                .items_center()
                .gap(px(8.0))
                .px(px(12.0))
                .py(px(10.0))
                .bg(bg::ELEVATED)
                .border_1()
                .border_color(border::DEFAULT)
                .rounded(px(6.0))
                .child(
                    div()
                        .text_size(px(14.0))
                        .child("🔍"),
                )
                .child(search_input),
        )
        // Category filter
        .child(render_dropdown("category", "Category", selected_category.label()))
        // Sort dropdown
        .child(render_dropdown("sort", "Sort", selected_sort.label()))
}

/// Render a dropdown filter
fn render_dropdown(id: &str, label: &str, value: &str) -> impl IntoElement {
    div()
        .id(SharedString::from(format!("dropdown-{}", id)))
        .flex()
        .items_center()
        .gap(px(8.0))
        .px(px(12.0))
        .py(px(10.0))
        .bg(bg::ELEVATED)
        .border_1()
        .border_color(border::DEFAULT)
        .rounded(px(6.0))
        .cursor_pointer()
        .hover(|s| s.bg(bg::HOVER).border_color(border::SELECTED))
        .child(
            div()
                .text_size(px(11.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child(label.to_string()),
        )
        .child(
            div()
                .text_size(px(12.0))
                .font_family(FONT_FAMILY)
                .text_color(text::PRIMARY)
                .child(value.to_string()),
        )
        .child(
            div()
                .text_size(px(10.0))
                .text_color(text::MUTED)
                .child("▼"),
        )
}

/// Render the trending agents horizontal strip
pub fn render_trending_strip(agents: &[AgentListing]) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .px(px(16.0))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .text_size(px(14.0))
                        .child("🔥"),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child("HOT THIS WEEK"),
                ),
        )
        .child(
            div()
                .id("trending-strip")
                .flex()
                .gap(px(8.0))
                .overflow_x_scroll()
                .children(agents.iter().take(5).map(|agent| {
                    render_trending_chip(&agent.name)
                })),
        )
}

/// Render a trending agent chip
fn render_trending_chip(name: &str) -> impl IntoElement {
    div()
        .flex_shrink_0()
        .px(px(12.0))
        .py(px(6.0))
        .bg(bg::CARD)
        .border_1()
        .border_color(border::DEFAULT)
        .rounded(px(16.0))
        .cursor_pointer()
        .hover(|s| s.bg(bg::HOVER).border_color(border::SELECTED))
        .child(
            div()
                .text_size(px(12.0))
                .font_family(FONT_FAMILY)
                .text_color(text::PRIMARY)
                .child(name.to_string()),
        )
}

/// Render the agent grid
pub fn render_agent_grid(
    agents: &[AgentListing],
    selected_agent_id: Option<&str>,
) -> impl IntoElement {
    div()
        .id("agent-grid")
        .flex_1()
        .w_full()
        .p(px(16.0))
        .overflow_y_scroll()
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap(px(16.0))
                .children(agents.iter().map(|agent| {
                    let is_selected = selected_agent_id.map_or(false, |id| id == agent.id);
                    div()
                        .w(px(280.0))
                        .child(render_agent_card(agent, is_selected))
                })),
        )
}

/// Generate mock agents for UI development
pub fn mock_agents() -> Vec<AgentListing> {
    vec![
        AgentListing::mock("1", "MechaCoder v3", AgentCategory::Coding, TrustTier::Diamond),
        AgentListing::mock("2", "DeepSearch", AgentCategory::Research, TrustTier::Gold),
        AgentListing::mock("3", "ArtGen Pro", AgentCategory::Creative, TrustTier::Gold),
        AgentListing::mock("4", "DocWriter", AgentCategory::Communication, TrustTier::Silver),
        AgentListing::mock("5", "DataBot", AgentCategory::Data, TrustTier::Gold),
        AgentListing::mock("6", "AutoTask", AgentCategory::Automation, TrustTier::Bronze),
        AgentListing::mock("7", "FinanceGPT", AgentCategory::Finance, TrustTier::Silver),
        AgentListing::mock("8", "CodeReview AI", AgentCategory::Coding, TrustTier::Gold),
    ]
}
