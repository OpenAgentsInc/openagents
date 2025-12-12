//! Claude Panel - SDK configuration and status panel
//!
//! Exposes claude-agent-sdk functionality:
//! - Cost tracking (current session)
//! - Model selection
//! - Session management
//! - Account info
//! - Tools/MCP server status

use gpui::{
    div, prelude::*, px, App, Context, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Render, Styled, Window,
};
use std::collections::HashMap;
use theme_oa::{bg, border, status, text, FONT_FAMILY};
use claude_agent_sdk::{ModelInfo, AccountInfo};

/// Events emitted by ClaudePanel
#[derive(Clone, Debug)]
pub enum ClaudePanelEvent {
    /// Model was changed
    ModelChanged { model: String },
    /// Session fork requested
    SessionFork,
    /// Session resume requested
    SessionResume { session_id: String },
}

/// Cost tracking state
#[derive(Clone, Debug, Default)]
pub struct CostTracker {
    pub total_cost_usd: f64,
    pub model_usage: HashMap<String, f64>,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
}

/// Claude Panel component
pub struct ClaudePanel {
    /// Focus handle
    focus_handle: FocusHandle,
    /// Cost tracking data
    cost_tracker: CostTracker,
    /// Available models
    available_models: Vec<ModelInfo>,
    /// Current selected model
    selected_model_idx: Option<usize>,
    /// Model dropdown is open
    model_dropdown_open: bool,
    /// Current session ID
    session_id: Option<String>,
    /// Account information
    account_info: Option<AccountInfo>,
    /// Available tools
    tools: Vec<String>,
    /// MCP servers (name, status)
    mcp_servers: Vec<(String, String)>,
    /// Section expansion state
    cost_expanded: bool,
    model_expanded: bool,
    session_expanded: bool,
    account_expanded: bool,
    tools_expanded: bool,
}

impl ClaudePanel {
    /// Create a new Claude panel
    pub fn new(cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();

        Self {
            focus_handle,
            cost_tracker: CostTracker::default(),
            available_models: Vec::new(),
            selected_model_idx: None,
            model_dropdown_open: false,
            session_id: None,
            account_info: None,
            tools: Vec::new(),
            mcp_servers: Vec::new(),
            // Cost section always expanded by default
            cost_expanded: true,
            model_expanded: true,
            session_expanded: true,
            account_expanded: false,
            tools_expanded: false,
        }
    }

    /// Update the session ID
    pub fn set_session_id(&mut self, session_id: Option<String>, cx: &mut Context<Self>) {
        self.session_id = session_id;
        cx.notify();
    }

    /// Update available models
    pub fn set_available_models(&mut self, models: Vec<ModelInfo>, cx: &mut Context<Self>) {
        self.available_models = models;
        // Auto-select first model if none selected
        if self.selected_model_idx.is_none() && !self.available_models.is_empty() {
            self.selected_model_idx = Some(0);
        }
        cx.notify();
    }

    /// Update account information
    pub fn set_account_info(&mut self, account_info: Option<AccountInfo>, cx: &mut Context<Self>) {
        self.account_info = account_info;
        cx.notify();
    }

    /// Update tools and MCP servers
    pub fn set_tools_and_mcp(&mut self, tools: Vec<String>, mcp_servers: Vec<(String, String)>, cx: &mut Context<Self>) {
        self.tools = tools;
        self.mcp_servers = mcp_servers;
        cx.notify();
    }

    /// Get the currently selected model value (API identifier)
    pub fn selected_model(&self) -> Option<&str> {
        self.selected_model_idx
            .and_then(|idx| self.available_models.get(idx))
            .map(|model| model.value.as_str())
    }

    /// Update cost tracking from SDK result
    pub fn update_cost(&mut self, total: f64, model_usage: HashMap<String, f64>, input_tokens: u64, output_tokens: u64, cx: &mut Context<Self>) {
        self.cost_tracker.total_cost_usd = total;
        self.cost_tracker.model_usage = model_usage;
        self.cost_tracker.total_input_tokens = input_tokens;
        self.cost_tracker.total_output_tokens = output_tokens;
        cx.notify();
    }

    /// Toggle cost section
    fn toggle_cost(&mut self, cx: &mut Context<Self>) {
        self.cost_expanded = !self.cost_expanded;
        cx.notify();
    }

    /// Toggle model section
    fn toggle_model(&mut self, cx: &mut Context<Self>) {
        self.model_expanded = !self.model_expanded;
        cx.notify();
    }

    /// Toggle session section
    fn toggle_session(&mut self, cx: &mut Context<Self>) {
        self.session_expanded = !self.session_expanded;
        cx.notify();
    }

    /// Toggle account section
    fn toggle_account(&mut self, cx: &mut Context<Self>) {
        self.account_expanded = !self.account_expanded;
        cx.notify();
    }

    /// Toggle tools section
    fn toggle_tools(&mut self, cx: &mut Context<Self>) {
        self.tools_expanded = !self.tools_expanded;
        cx.notify();
    }

    /// Render the header
    fn render_header(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        let close_hint = if cfg!(target_os = "macos") {
            "[Cmd+C to close]"
        } else {
            "[Ctrl+C to close]"
        };

        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px(px(12.0))
            .py(px(8.0))
            .border_b_1()
            .border_color(border::DEFAULT)
            .bg(bg::SURFACE)
            .child(
                div()
                    .font_family(FONT_FAMILY)
                    .text_sm()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(text::PRIMARY)
                    .child("CLAUDE")
            )
            .child(
                div()
                    .text_xs()
                    .text_color(text::MUTED)
                    .child(close_hint)
            )
    }

    /// Render the cost section
    fn render_cost_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_open = self.cost_expanded;

        div()
            .px(px(12.0))
            .py(px(8.0))
            .border_b_1()
            .border_color(border::DEFAULT)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .cursor_pointer()
                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                        this.toggle_cost(cx);
                    }))
                    .child(
                        div()
                            .text_xs()
                            .text_color(text::MUTED)
                            .font_weight(gpui::FontWeight::BOLD)
                            .child("COST")
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(text::MUTED)
                            .child(if is_open { "[-]" } else { "[+]" })
                    )
            )
            .when(is_open, |el| {
                let total = self.cost_tracker.total_cost_usd;
                let input_tokens = self.cost_tracker.total_input_tokens;
                let output_tokens = self.cost_tracker.total_output_tokens;
                let has_usage = total > 0.0 || input_tokens > 0 || output_tokens > 0;

                el.mt(px(8.0))
                    .child(
                        div()
                            .text_sm()
                            .text_color(if has_usage { status::SUCCESS } else { text::MUTED })
                            .child(format!("${:.2}", total))
                    )
                    .when(!self.cost_tracker.model_usage.is_empty(), |el| {
                        el.mt(px(8.0))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(text::MUTED)
                                    .child("By Model")
                            )
                            .children(self.cost_tracker.model_usage.iter().map(|(model, cost)| {
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .justify_between()
                                    .py(px(2.0))
                                    .text_xs()
                                    .child(
                                        div()
                                            .text_color(text::SECONDARY)
                                            .flex_1()
                                            .overflow_hidden()
                                            .child(model.clone())
                                    )
                                    .child(
                                        div()
                                            .text_color(text::MUTED)
                                            .child(format!("${:.2}", cost))
                                    )
                            }))
                    })
                    .when(input_tokens > 0 || output_tokens > 0, |el| {
                        let total_tokens = input_tokens + output_tokens;
                        el.mt(px(8.0))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(text::MUTED)
                                    .child(format!(
                                        "{:.1}K tokens ({:.1}K in / {:.1}K out)",
                                        total_tokens as f64 / 1000.0,
                                        input_tokens as f64 / 1000.0,
                                        output_tokens as f64 / 1000.0
                                    ))
                            )
                    })
            })
    }

    /// Render the model section
    fn render_model_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_open = self.model_expanded;
        let dropdown_open = self.model_dropdown_open;
        let selected_model_display = self.selected_model().map(|s| s.split("-").next().unwrap_or("Claude").to_string()).unwrap_or_else(|| "No model".to_string());
        let has_models = !self.available_models.is_empty();

        div()
            .px(px(12.0))
            .py(px(8.0))
            .border_b_1()
            .border_color(border::DEFAULT)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .cursor_pointer()
                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                        this.toggle_model(cx);
                    }))
                    .child(
                        div()
                            .text_xs()
                            .text_color(text::MUTED)
                            .font_weight(gpui::FontWeight::BOLD)
                            .child("MODEL")
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(text::MUTED)
                            .child(if is_open { "[-]" } else { "[+]" })
                    )
            )
            .when(is_open, |el| {
                if has_models {
                    el.mt(px(8.0))
                        .relative()
                        .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                            this.model_dropdown_open = false;
                            cx.notify();
                        }))
                        .child(
                            // Model dropdown button
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .px(px(8.0))
                                .py(px(6.0))
                                .bg(bg::CARD)
                                .border_1()
                                .border_color(if dropdown_open { border::FOCUS } else { border::DEFAULT })
                                .text_sm()
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                                    this.model_dropdown_open = !this.model_dropdown_open;
                                    cx.notify();
                                }))
                                .child(
                                    div()
                                        .flex_1()
                                        .text_color(text::PRIMARY)
                                        .text_xs()
                                        .child(selected_model_display.clone())
                                )
                                .child(
                                    div()
                                        .text_color(text::MUTED)
                                        .child("v")
                                )
                        )
                        .when(dropdown_open, |el| {
                            el.child(
                                gpui::deferred(
                                    div()
                                        .absolute()
                                        .top(px(36.0))
                                        .left(px(0.0))
                                        .w_full()
                                        .bg(bg::SURFACE)
                                        .border_1()
                                        .border_color(border::DEFAULT)
                                        .occlude()
                                        .children(self.available_models.iter().enumerate().map(|(idx, model)| {
                                            let is_selected = Some(idx) == self.selected_model_idx;
                                            div()
                                                .w_full()
                                                .px(px(8.0))
                                                .py(px(6.0))
                                                .bg(if is_selected { bg::ELEVATED } else { bg::SURFACE })
                                                .border_b_1()
                                                .border_color(border::SUBTLE)
                                                .text_xs()
                                                .text_color(text::PRIMARY)
                                                .cursor_pointer()
                                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                                    this.selected_model_idx = Some(idx);
                                                    this.model_dropdown_open = false;
                                                    if let Some(model_val) = this.selected_model() {
                                                        cx.emit(ClaudePanelEvent::ModelChanged {
                                                            model: model_val.to_string(),
                                                        });
                                                    }
                                                    cx.notify();
                                                }))
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .gap(px(2.0))
                                                        .child(
                                                            div()
                                                                .font_weight(if is_selected { gpui::FontWeight::BOLD } else { gpui::FontWeight::NORMAL })
                                                                .child(model.display_name.clone())
                                                        )
                                                        .when(!model.description.is_empty(), |el| {
                                                            el.child(
                                                                div()
                                                                    .text_color(text::MUTED)
                                                                    .text_xs()
                                                                    .child(model.description.clone())
                                                            )
                                                        })
                                                        .when(is_selected, |el| {
                                                            el.child(
                                                                div()
                                                                    .text_color(status::SUCCESS)
                                                                    .child("✓")
                                                            )
                                                        })
                                                )
                                        }))
                                )
                                .with_priority(1)
                            )
                        })
                } else {
                    el.mt(px(8.0))
                        .child(
                            div()
                                .px(px(8.0))
                                .py(px(6.0))
                                .bg(bg::CARD)
                                .border_1()
                                .border_color(border::DEFAULT)
                                .text_sm()
                                .text_color(text::SECONDARY)
                                .child("Loading models...")
                        )
                }
            })
    }

    /// Render the session section
    fn render_session_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_open = self.session_expanded;
        let has_session = self.session_id.is_some();

        div()
            .px(px(12.0))
            .py(px(8.0))
            .border_b_1()
            .border_color(border::DEFAULT)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .cursor_pointer()
                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                        this.toggle_session(cx);
                    }))
                    .child(
                        div()
                            .text_xs()
                            .text_color(text::MUTED)
                            .font_weight(gpui::FontWeight::BOLD)
                            .child("SESSION")
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(text::MUTED)
                            .child(if is_open { "[-]" } else { "[+]" })
                    )
            )
            .when(is_open, |el| {
                if has_session {
                    el.mt(px(8.0))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(6.0))
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .gap(px(4.0))
                                        .items_center()
                                        .child(
                                            div()
                                                .flex_1()
                                                .text_xs()
                                                .text_color(text::SECONDARY)
                                                .font_family(FONT_FAMILY)
                                                .child(self.session_id.as_ref().unwrap().clone())
                                        )
                                        .child({
                                            let session_id = self.session_id.clone();
                                            div()
                                                .px(px(6.0))
                                                .py(px(2.0))
                                                .bg(bg::CARD)
                                                .border_1()
                                                .border_color(border::DEFAULT)
                                                .text_xs()
                                                .text_color(text::MUTED)
                                                .cursor_pointer()
                                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(move |_this, _, _, _cx| {
                                                    if let Some(id) = session_id.clone() {
                                                        log::info!("Copied session ID to clipboard: {}", id);
                                                    }
                                                }))
                                                .child("[Copy]")
                                        })
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .gap(px(4.0))
                                        .child(
                                            div()
                                                .px(px(8.0))
                                                .py(px(4.0))
                                                .bg(bg::CARD)
                                                .border_1()
                                                .border_color(border::DEFAULT)
                                                .text_xs()
                                                .text_color(text::PRIMARY)
                                                .cursor_pointer()
                                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_this, _, _, cx| {
                                                    cx.emit(ClaudePanelEvent::SessionFork);
                                                }))
                                                .child("[Fork]")
                                        )
                                        .child(
                                            div()
                                                .px(px(8.0))
                                                .py(px(4.0))
                                                .bg(bg::CARD)
                                                .border_1()
                                                .border_color(border::DEFAULT)
                                                .text_xs()
                                                .text_color(text::PRIMARY)
                                                .cursor_pointer()
                                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_this, _, _, _cx| {
                                                    // TODO: Show session history modal
                                                    log::info!("Show session history (coming in future)");
                                                }))
                                                .child("[History...]")
                                        )
                                )
                        )
                } else {
                    el.mt(px(8.0))
                        .child(
                            div()
                                .px(px(8.0))
                                .py(px(6.0))
                                .bg(bg::CARD)
                                .border_1()
                                .border_color(border::DEFAULT)
                                .text_sm()
                                .text_color(text::SECONDARY)
                                .child("No active session")
                        )
                }
            })
    }

    /// Render the account section (placeholder for Phase 4)
    fn render_account_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_open = self.account_expanded;

        div()
            .px(px(12.0))
            .py(px(8.0))
            .border_b_1()
            .border_color(border::DEFAULT)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .cursor_pointer()
                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                        this.toggle_account(cx);
                    }))
                    .child(
                        div()
                            .text_xs()
                            .text_color(text::MUTED)
                            .font_weight(gpui::FontWeight::BOLD)
                            .child("ACCOUNT")
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(text::MUTED)
                            .child(if is_open { "[-]" } else { "[+]" })
                    )
            )
            .when(is_open, |el| {
                if let Some(account) = &self.account_info {
                    el.mt(px(8.0))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(6.0))
                                .px(px(8.0))
                                .py(px(6.0))
                                .bg(bg::CARD)
                                .border_1()
                                .border_color(border::DEFAULT)
                                .when(account.email.is_some(), |el| {
                                    el.child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(2.0))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(text::MUTED)
                                                    .child("Email")
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(text::PRIMARY)
                                                    .font_family(FONT_FAMILY)
                                                    .child(account.email.as_ref().unwrap().clone())
                                            )
                                    )
                                })
                                .when(account.organization.is_some(), |el| {
                                    el.child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(2.0))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(text::MUTED)
                                                    .child("Organization")
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(text::PRIMARY)
                                                    .child(account.organization.as_ref().unwrap().clone())
                                            )
                                    )
                                })
                                .when(account.subscription_type.is_some(), |el| {
                                    el.child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(2.0))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(text::MUTED)
                                                    .child("Subscription")
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(text::PRIMARY)
                                                    .child(account.subscription_type.as_ref().unwrap().clone())
                                            )
                                    )
                                })
                                .when(account.token_source.is_some(), |el| {
                                    el.child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(2.0))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(text::MUTED)
                                                    .child("Token Source")
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(text::PRIMARY)
                                                    .child(account.token_source.as_ref().unwrap().clone())
                                            )
                                    )
                                })
                        )
                } else {
                    el.mt(px(8.0))
                        .child(
                            div()
                                .px(px(8.0))
                                .py(px(6.0))
                                .bg(bg::CARD)
                                .border_1()
                                .border_color(border::DEFAULT)
                                .text_sm()
                                .text_color(text::SECONDARY)
                                .child("No account info")
                        )
                }
            })
    }

    /// Render the tools section (placeholder for Phase 5)
    fn render_tools_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_open = self.tools_expanded;

        div()
            .px(px(12.0))
            .py(px(8.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .cursor_pointer()
                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                        this.toggle_tools(cx);
                    }))
                    .child(
                        div()
                            .text_xs()
                            .text_color(text::MUTED)
                            .font_weight(gpui::FontWeight::BOLD)
                            .child("TOOLS")
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(text::MUTED)
                            .child(if is_open { "[-]" } else { "[+]" })
                    )
            )
            .when(is_open, |el| {
                if self.tools.is_empty() && self.mcp_servers.is_empty() {
                    el.mt(px(8.0))
                        .child(
                            div()
                                .px(px(8.0))
                                .py(px(6.0))
                                .bg(bg::CARD)
                                .border_1()
                                .border_color(border::DEFAULT)
                                .text_sm()
                                .text_color(text::SECONDARY)
                                .child("No tools or MCP servers")
                        )
                } else {
                    el.mt(px(8.0))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(8.0))
                                .px(px(8.0))
                                .py(px(6.0))
                                // Tools section
                                .when(!self.tools.is_empty(), |el| {
                                    el.child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(4.0))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(text::MUTED)
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .child("TOOLS")
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .gap(px(2.0))
                                                    .children(self.tools.iter().map(|tool| {
                                                        div()
                                                            .text_xs()
                                                            .text_color(text::PRIMARY)
                                                            .child(format!("• {}", tool))
                                                    }))
                                            )
                                    )
                                })
                                // MCP servers section
                                .when(!self.mcp_servers.is_empty(), |el| {
                                    el.child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(4.0))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(text::MUTED)
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .child("MCP SERVERS")
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .gap(px(2.0))
                                                    .children(self.mcp_servers.iter().map(|(name, status)| {
                                                        div()
                                                            .flex()
                                                            .flex_row()
                                                            .gap(px(4.0))
                                                            .items_center()
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(text::PRIMARY)
                                                                    .child(format!("• {}", name))
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(if status == "ready" {
                                                                        status::SUCCESS
                                                                    } else if status == "error" {
                                                                        status::ERROR
                                                                    } else {
                                                                        status::WARNING
                                                                    })
                                                                    .child(format!("[{}]", status))
                                                            )
                                                    }))
                                            )
                                    )
                                })
                        )
                }
            })
    }
}

impl EventEmitter<ClaudePanelEvent> for ClaudePanel {}

impl Focusable for ClaudePanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ClaudePanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .h_full()
            .bg(bg::APP)
            .font_family(FONT_FAMILY)
            .track_focus(&self.focus_handle)
            .child(self.render_header(cx))
            .child(self.render_cost_section(cx))
            .child(self.render_model_section(cx))
            .child(self.render_session_section(cx))
            .child(self.render_account_section(cx))
            .child(self.render_tools_section(cx))
    }
}
