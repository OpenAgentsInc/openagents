//! Services Market view - Main view for the Services tab

use gpui::*;
use theme::{bg, border, text, FONT_FAMILY};

use crate::types::{DVMListing, MCPServerListing, ServiceCategory};
use super::dvm_list::{render_dvm_list, mock_dvms};
use super::mcp_grid::{render_mcp_grid, mock_mcp_servers};

/// State for the Services Market view
pub struct ServicesMarketState {
    pub search_query: String,
    pub selected_category: ServiceCategory,
    pub dvms: Vec<DVMListing>,
    pub mcp_servers: Vec<MCPServerListing>,
}

impl Default for ServicesMarketState {
    fn default() -> Self {
        Self {
            search_query: String::new(),
            selected_category: ServiceCategory::All,
            dvms: mock_dvms(),
            mcp_servers: mock_mcp_servers(),
        }
    }
}

/// Render the Services Market view
pub fn render_services_market(state: &ServicesMarketState) -> impl IntoElement {
    div()
        .id("services-market")
        .flex_1()
        .h_full()
        .flex()
        .flex_col()
        .gap(px(16.0))
        .p(px(16.0))
        .bg(bg::APP)
        .overflow_y_scroll()
        // Search bar
        .child(render_search_bar(&state.search_query))
        // Category filters
        .child(render_category_filters(state.selected_category))
        // Trending DVMs
        .child(render_dvm_list(&state.dvms))
        // MCP Servers
        .child(render_mcp_grid(&state.mcp_servers))
}

/// Render the search bar
fn render_search_bar(query: &str) -> impl IntoElement {
    div()
        .w_full()
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
        .child(
            div()
                .flex_1()
                .text_size(px(14.0))
                .font_family(FONT_FAMILY)
                .text_color(if query.is_empty() {
                    text::PLACEHOLDER
                } else {
                    text::PRIMARY
                })
                .child(if query.is_empty() {
                    "Search DVMs and MCP servers...".to_string()
                } else {
                    query.to_string()
                }),
        )
}

/// Render category filter chips
fn render_category_filters(selected: ServiceCategory) -> impl IntoElement {
    div()
        .flex()
        .flex_wrap()
        .gap(px(6.0))
        .children(ServiceCategory::all().iter().map(|&cat| {
            render_category_chip(cat, cat == selected)
        }))
}

/// Render a category chip
fn render_category_chip(category: ServiceCategory, is_selected: bool) -> impl IntoElement {
    let (bg_color, text_color, border_color) = if is_selected {
        (bg::SELECTED, text::BRIGHT, border::SELECTED)
    } else {
        (Hsla::transparent_black(), text::MUTED, border::DEFAULT)
    };

    div()
        .px(px(12.0))
        .py(px(6.0))
        .bg(bg_color)
        .border_1()
        .border_color(border_color)
        .rounded(px(16.0))
        .cursor_pointer()
        .hover(|s| s.bg(bg::HOVER).text_color(text::PRIMARY))
        .child(
            div()
                .text_size(px(12.0))
                .font_family(FONT_FAMILY)
                .text_color(text_color)
                .child(category.label().to_string()),
        )
}
