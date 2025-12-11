//! Agent grid component - Grid layout of agent cards
//! Bloomberg-style: dense, text-first, no emojis

use gpui_oa::*;
use theme_oa::{bg, border, text, FONT_FAMILY};

use crate::types::{AgentListing, AgentCategory, AgentSortOption, TrustTier};
use ui_oa::TextInput;
use super::agent_card::render_agent_card;

/// Render the search bar with a real TextInput - Bloomberg style
pub fn render_search_bar_with_input(
    search_input: Entity<TextInput>,
    selected_category: AgentCategory,
    selected_sort: AgentSortOption,
) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .items_center()
        .gap(px(8.0))
        .px(px(12.0))
        .py(px(8.0))
        .bg(bg::SURFACE)
        .border_b_1()
        .border_color(border::DEFAULT)
        // Search input - Bloomberg command bar style
        .child(
            div()
                .text_size(px(11.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child("SEARCH:"),
        )
        .child(
            div()
                .flex_1()
                .flex()
                .items_center()
                .px(px(8.0))
                .py(px(4.0))
                .bg(bg::ELEVATED)
                .border_1()
                .border_color(border::DEFAULT)
                // No rounded corners
                .child(search_input),
        )
        // Category filter
        .child(render_dropdown("CAT", selected_category.label()))
        // Sort dropdown
        .child(render_dropdown("SORT", selected_sort.label()))
}

/// Render a dropdown filter - Bloomberg style
fn render_dropdown(label: &str, value: &str) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(4.0))
        .px(px(8.0))
        .py(px(4.0))
        .bg(bg::ELEVATED)
        .border_1()
        .border_color(border::DEFAULT)
        .cursor_pointer()
        .hover(|s| s.bg(bg::HOVER).border_color(border::SELECTED))
        .child(
            div()
                .text_size(px(10.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child(format!("{}:", label)),
        )
        .child(
            div()
                .text_size(px(10.0))
                .font_family(FONT_FAMILY)
                .text_color(text::PRIMARY)
                .child(value.to_string()),
        )
}

/// Render the trending agents horizontal strip - Bloomberg style
pub fn render_trending_strip(agents: &[AgentListing]) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .items_center()
        .gap(px(8.0))
        .px(px(12.0))
        .py(px(6.0))
        .bg(bg::PANEL)
        .border_b_1()
        .border_color(border::DEFAULT)
        // Label
        .child(
            div()
                .text_size(px(10.0))
                .font_family(FONT_FAMILY)
                .text_color(Hsla { h: 0.08, s: 0.9, l: 0.5, a: 1.0 })  // Orange for "hot"
                .child("TRENDING:"),
        )
        // Trending items - horizontal list
        .child(
            div()
                .id("trending-strip")
                .flex()
                .gap(px(6.0))
                .overflow_x_scroll()
                .children(agents.iter().take(6).enumerate().map(|(i, agent)| {
                    render_trending_item(i + 1, &agent.name)
                })),
        )
}

/// Render a trending agent item - Bloomberg style (numbered list)
fn render_trending_item(rank: usize, name: &str) -> impl IntoElement {
    div()
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap(px(4.0))
        .px(px(6.0))
        .py(px(2.0))
        .bg(bg::CARD)
        .border_1()
        .border_color(border::SUBTLE)
        .cursor_pointer()
        .hover(|s| s.bg(bg::HOVER).border_color(border::SELECTED))
        // Rank number
        .child(
            div()
                .text_size(px(9.0))
                .font_family(FONT_FAMILY)
                .text_color(text::DIM)
                .child(format!("{}.", rank)),
        )
        // Name
        .child(
            div()
                .text_size(px(10.0))
                .font_family(FONT_FAMILY)
                .text_color(text::PRIMARY)
                .child(name.to_string()),
        )
}

/// Render the agent grid - denser layout
pub fn render_agent_grid(
    agents: &[AgentListing],
    selected_agent_id: Option<&str>,
) -> impl IntoElement {
    div()
        .id("agent-grid")
        .flex_1()
        .w_full()
        .p(px(8.0))  // Tighter padding
        .overflow_y_scroll()
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap(px(8.0))  // Tighter gap
                .children(agents.iter().map(|agent| {
                    let is_selected = selected_agent_id.map_or(false, |id| id == agent.id);
                    div()
                        .w(px(200.0))  // Narrower cards
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
