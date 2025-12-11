//! Main Vibe screen - Full-stack AI-native IDE
//!
//! Orchestrates all Vibe components: projects, editor, database, deploy.

use gpui_oa::*;
use gpui_oa::prelude::*;
use theme_oa::{bg, border, status, text, FONT_FAMILY};

use crate::types::*;
use crate::projects::{render_project_grid, render_template_picker};
use crate::editor::{render_editor_workspace};
use crate::database::{render_database_dashboard};
use crate::deploy::{render_deploy_dashboard};

/// The main Vibe IDE screen component
pub struct VibeScreen {
    focus_handle: FocusHandle,

    // Navigation state
    current_tab: VibeTab,

    // Resource bar state
    credits_remaining: u32,
    credits_used_today: u32,
    current_project: Option<String>,

    // Tab-specific state
    projects_state: ProjectsTabState,
    editor_state: EditorTabState,
    database_state: DatabaseTabState,
    deploy_state: DeployTabState,
}

impl VibeScreen {
    /// Create a new VibeScreen with mock data
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            current_tab: VibeTab::Projects,
            credits_remaining: 87,
            credits_used_today: 13,
            current_project: None,

            projects_state: ProjectsTabState {
                projects: mock_projects(),
                templates: mock_templates(),
                search_query: String::new(),
                selected_category: TemplateCategory::All,
                show_templates: false,
                selected_project_id: None,
            },

            editor_state: EditorTabState {
                file_tree: mock_file_tree(),
                open_tabs: mock_editor_tabs(),
                active_file_path: Some("/workspace/src/App.tsx".to_string()),
                file_content: mock_file_content().to_string(),
                agent_mode: AgentMode::Agent,
                agent_tasks: mock_agent_tasks(),
                terminal_lines: mock_terminal_lines(),
                terminal_input: String::new(),
                show_preview: true,
                show_terminal: true,
                show_agent_panel: true,
            },

            database_state: DatabaseTabState {
                tables: mock_database_tables(),
                selected_table: Some("users".to_string()),
                table_rows: mock_table_rows("users"),
                sql_query: "SELECT * FROM users WHERE is_active = true LIMIT 10;".to_string(),
                query_results: vec![],
                show_schema: false,
            },

            deploy_state: DeployTabState {
                deployments: mock_deployments(),
                domains: mock_domains(),
                analytics: mock_analytics(),
                analytics_range: AnalyticsRange::Week,
                show_analytics: true,
            },
        }
    }

    /// Open a project for editing
    pub fn open_project(&mut self, project_id: &str, cx: &mut Context<Self>) {
        self.current_project = Some(project_id.to_string());
        self.current_tab = VibeTab::Editor;
        cx.notify();
    }

    /// Switch to a different tab
    pub fn set_tab(&mut self, tab: VibeTab, cx: &mut Context<Self>) {
        self.current_tab = tab;
        cx.notify();
    }

    /// Toggle template picker in projects tab
    pub fn toggle_templates(&mut self, cx: &mut Context<Self>) {
        self.projects_state.show_templates = !self.projects_state.show_templates;
        cx.notify();
    }

    /// Toggle preview panel in editor
    pub fn toggle_preview(&mut self, cx: &mut Context<Self>) {
        self.editor_state.show_preview = !self.editor_state.show_preview;
        cx.notify();
    }

    /// Toggle terminal panel in editor
    pub fn toggle_terminal(&mut self, cx: &mut Context<Self>) {
        self.editor_state.show_terminal = !self.editor_state.show_terminal;
        cx.notify();
    }

    /// Toggle agent panel in editor
    pub fn toggle_agent_panel(&mut self, cx: &mut Context<Self>) {
        self.editor_state.show_agent_panel = !self.editor_state.show_agent_panel;
        cx.notify();
    }

    /// Set agent mode
    pub fn set_agent_mode(&mut self, mode: AgentMode, cx: &mut Context<Self>) {
        self.editor_state.agent_mode = mode;
        cx.notify();
    }

    /// Select a database table
    pub fn select_table(&mut self, table_name: &str, cx: &mut Context<Self>) {
        self.database_state.selected_table = Some(table_name.to_string());
        self.database_state.table_rows = mock_table_rows(table_name);
        cx.notify();
    }

    /// Toggle schema view in database tab
    pub fn toggle_schema(&mut self, cx: &mut Context<Self>) {
        self.database_state.show_schema = !self.database_state.show_schema;
        cx.notify();
    }

    /// Trigger a deployment
    pub fn deploy(&mut self, cx: &mut Context<Self>) {
        // In real implementation, this would trigger the deploy pipeline
        cx.notify();
    }

    /// Set analytics time range
    pub fn set_analytics_range(&mut self, range: AnalyticsRange, cx: &mut Context<Self>) {
        self.deploy_state.analytics_range = range;
        cx.notify();
    }

    /// Render the resource bar (credits, project, status)
    fn render_resource_bar(&self) -> impl IntoElement {
        div()
            .id("vibe-resource-bar")
            .h(px(32.0))
            .w_full()
            .flex()
            .items_center()
            .justify_between()
            .px(px(16.0))
            .bg(bg::SURFACE)
            .border_b_1()
            .border_color(border::DEFAULT)
            // Left: project name
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::MUTED)
                            .child("VIBE"),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(text::MUTED)
                            .child("|"),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::PRIMARY)
                            .child(
                                self.current_project
                                    .clone()
                                    .unwrap_or_else(|| "No project open".to_string()),
                            ),
                    ),
            )
            // Right: credits and status
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(16.0))
                    // Credits
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::MUTED)
                                    .child("CREDITS"),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(status::SUCCESS)
                                    .child(format!("{}", self.credits_remaining)),
                            )
                            .child(
                                div()
                                    .text_size(px(9.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::MUTED)
                                    .child(format!("(-{} today)", self.credits_used_today)),
                            ),
                    )
                    // Build status indicator
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .w(px(6.0))
                                    .h(px(6.0))
                                    .rounded_full()
                                    .bg(status::SUCCESS),
                            )
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::MUTED)
                                    .child("READY"),
                            ),
                    ),
            )
    }

    /// Render a single tab button
    fn render_tab_button(&self, tab: VibeTab, cx: &mut Context<Self>) -> impl IntoElement {
        let is_active = tab == self.current_tab;
        let (bg_color, text_color, border_color) = if is_active {
            (bg::SELECTED, Hsla { h: 0.14, s: 1.0, l: 0.5, a: 1.0 }, border::SELECTED)
        } else {
            (Hsla::transparent_black(), text::MUTED, Hsla::transparent_black())
        };

        div()
            .id(SharedString::from(format!("vibe-tab-{}", tab.label())))
            .flex()
            .items_center()
            .gap(px(6.0))
            .px(px(12.0))
            .py(px(6.0))
            .bg(bg_color)
            .border_1()
            .border_color(border_color)
            .cursor_pointer()
            .hover(|s| s.bg(bg::HOVER).text_color(text::PRIMARY))
            .on_click(cx.listener(move |this, _event, _window, cx| {
                this.set_tab(tab, cx);
            }))
            .child(
                div()
                    .text_size(px(11.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text_color)
                    .child(tab.label()),
            )
            .child(
                div()
                    .text_size(px(9.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::MUTED)
                    .child(format!("[{}]", tab.shortcut())),
            )
    }

    /// Render the current tab content
    fn render_tab_content(&self, cx: &mut Context<Self>) -> AnyElement {
        match self.current_tab {
            VibeTab::Projects => {
                if self.projects_state.show_templates {
                    render_template_picker(&self.projects_state, cx).into_any_element()
                } else {
                    render_project_grid(&self.projects_state, cx).into_any_element()
                }
            }
            VibeTab::Editor => {
                render_editor_workspace(&self.editor_state, cx).into_any_element()
            }
            VibeTab::Database => {
                render_database_dashboard(&self.database_state, cx).into_any_element()
            }
            VibeTab::Deploy => {
                render_deploy_dashboard(&self.deploy_state, cx).into_any_element()
            }
        }
    }
}

impl Focusable for VibeScreen {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for VibeScreen {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("vibe-screen")
            .flex()
            .flex_col()
            .h_full()
            .w_full()
            .bg(bg::APP)
            // Resource bar (top HUD)
            .child(self.render_resource_bar())
            // Tab bar
            .child(
                div()
                    .id("vibe-tab-bar")
                    .h(px(44.0))
                    .w_full()
                    .flex()
                    .items_center()
                    .px(px(16.0))
                    .gap(px(4.0))
                    .bg(bg::SURFACE)
                    .border_b_1()
                    .border_color(border::DEFAULT)
                    .child(self.render_tab_button(VibeTab::Projects, cx))
                    .child(self.render_tab_button(VibeTab::Editor, cx))
                    .child(self.render_tab_button(VibeTab::Database, cx))
                    .child(self.render_tab_button(VibeTab::Deploy, cx)),
            )
            // Main content area
            .child(
                div()
                    .id("vibe-content")
                    .flex()
                    .flex_1()
                    .overflow_hidden()
                    .child(self.render_tab_content(cx)),
            )
    }
}
