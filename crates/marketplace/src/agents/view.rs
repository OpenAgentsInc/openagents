//! Agent Store view - Main view for the Agents tab

use gpui::prelude::FluentBuilder;
use gpui::*;
use theme::bg;

use crate::types::{AgentListing, AgentCategory, AgentSortOption};
use ui::TextInput;
use super::agent_grid::{render_search_bar_with_input, render_trending_strip, render_agent_grid, mock_agents};
use super::agent_detail::render_agent_detail;

/// State for the Agent Store view
pub struct AgentStoreState {
    pub search_query: String,
    pub selected_category: AgentCategory,
    pub selected_sort: AgentSortOption,
    pub agents: Vec<AgentListing>,
    pub selected_agent_id: Option<String>,
    pub detail_panel_open: bool,
}

impl Default for AgentStoreState {
    fn default() -> Self {
        Self {
            search_query: String::new(),
            selected_category: AgentCategory::All,
            selected_sort: AgentSortOption::Trending,
            agents: mock_agents(),
            selected_agent_id: None,
            detail_panel_open: false,
        }
    }
}

/// Render the Agent Store view with a proper text input
pub fn render_agent_store_with_input(state: &AgentStoreState, search_input: Entity<TextInput>) -> impl IntoElement {
    let selected_agent = state.selected_agent_id.as_ref().and_then(|id| {
        state.agents.iter().find(|a| &a.id == id)
    });

    div()
        .flex_1()
        .h_full()
        .flex()
        .bg(bg::APP)
        // Main content area
        .child(
            div()
                .flex_1()
                .h_full()
                .flex()
                .flex_col()
                // Search bar with real text input
                .child(render_search_bar_with_input(
                    search_input,
                    state.selected_category,
                    state.selected_sort,
                ))
                // Trending strip
                .child(render_trending_strip(&state.agents))
                // Agent grid
                .child(render_agent_grid(
                    &state.agents,
                    state.selected_agent_id.as_deref(),
                )),
        )
        // Detail panel (conditional)
        .when_some(selected_agent, |el, agent| {
            el.child(render_agent_detail(agent))
        })
}
