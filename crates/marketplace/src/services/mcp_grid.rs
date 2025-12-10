//! MCP grid component - Model Context Protocol server cards

use gpui::*;
use theme::{bg, border, text, accent, FONT_FAMILY};

use crate::types::MCPServerListing;

/// Render the MCP servers section
pub fn render_mcp_grid(servers: &[MCPServerListing]) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(12.0))
        // Header
        .child(render_section_header())
        // Cards grid
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap(px(12.0))
                .children(servers.iter().map(|server| {
                    render_mcp_card(server)
                })),
        )
}

/// Render the section header
fn render_section_header() -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(
            div()
                .text_size(px(14.0))
                .child("🔧"),
        )
        .child(
            div()
                .text_size(px(12.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child("MODEL CONTEXT PROTOCOL SERVERS"),
        )
}

/// Render an MCP server card
fn render_mcp_card(server: &MCPServerListing) -> impl IntoElement {
    div()
        .w(px(200.0))
        .flex()
        .flex_col()
        .gap(px(8.0))
        .p(px(12.0))
        .bg(bg::CARD)
        .border_1()
        .border_color(border::DEFAULT)
        .rounded(px(8.0))
        .cursor_pointer()
        .hover(|s| s.bg(bg::HOVER).border_color(border::SELECTED))
        // Name
        .child(
            div()
                .text_size(px(14.0))
                .font_family(FONT_FAMILY)
                .text_color(text::PRIMARY)
                .child(server.name.clone()),
        )
        // Tool count
        .child(
            div()
                .text_size(px(12.0))
                .font_family(FONT_FAMILY)
                .text_color(text::SECONDARY)
                .child(format!("{} tools", server.tool_count)),
        )
        // Pricing
        .child(
            div()
                .text_size(px(12.0))
                .font_family(FONT_FAMILY)
                .text_color(if server.sats_per_unit == 0 {
                    theme::status::SUCCESS
                } else {
                    accent::PRIMARY
                })
                .child(if server.sats_per_unit == 0 {
                    "Free".to_string()
                } else {
                    format!("{} sats{}", server.sats_per_unit, server.pricing_unit.label())
                }),
        )
        // Install button
        .child(
            div()
                .w_full()
                .py(px(6.0))
                .bg(accent::PRIMARY_MUTED)
                .rounded(px(4.0))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|s| s.bg(accent::PRIMARY))
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_family(FONT_FAMILY)
                        .text_color(accent::PRIMARY)
                        .child("INSTALL"),
                ),
        )
}

/// Generate mock MCP servers for UI development
pub fn mock_mcp_servers() -> Vec<MCPServerListing> {
    vec![
        MCPServerListing::mock("GitHub Tools", 12, 5),
        MCPServerListing::mock("Filesystem", 8, 0),
        MCPServerListing::mock("Database", 15, 10),
        MCPServerListing::mock("Web Scraper", 6, 3),
        MCPServerListing::mock("Slack Integration", 10, 5),
        MCPServerListing::mock("Calendar", 7, 0),
    ]
}
