//! Main Marketplace screen - Orchestrates all components

use gpui::*;
use std::sync::Arc;
use theme::{bg, border, text, FONT_FAMILY};

use crate::nostr_bridge::NostrBridge;
use crate::types::{DVMListing, MarketplaceTab, TrustTier, Transaction, Notification, NotificationKind};
use crate::resource_bar::{self, ResourceBarProps};
use crate::activity_feed::{self, mock_transactions, mock_notifications};
use crate::agents::{AgentStoreState, render_agent_store_with_input};
use crate::compute::{ComputeMarketState, render_compute_market};
use crate::services::{DVMList, ServicesMarketState, render_mcp_grid, mock_mcp_servers};
use nostr_chat::{ChatEvent, ChatState};
use ui::TextInput;

/// The main Marketplace screen component
pub struct MarketplaceScreen {
    focus_handle: FocusHandle,

    // Navigation state
    current_tab: MarketplaceTab,

    // Resource bar state
    wallet_balance_sats: u64,
    trust_tier: TrustTier,
    earnings_today_sats: u64,
    is_online: bool,
    connected_relays: u32,

    // Activity feed state
    activity_feed_collapsed: bool,
    transactions: Vec<Transaction>,
    notifications: Vec<Notification>,

    // Search input
    search_input: Entity<TextInput>,

    // Tab-specific state
    agent_store_state: AgentStoreState,
    compute_market_state: ComputeMarketState,
    services_market_state: ServicesMarketState,

    // DVM list entity with USE button click handling
    dvm_list: Entity<DVMList>,

    // Nostr integration (optional, for NIP-90 DVM support)
    nostr_bridge: Option<NostrBridge>,

    // Selected DVM for job submission dialog
    selected_dvm: Option<DVMListing>,
}

impl MarketplaceScreen {
    /// Create a new MarketplaceScreen (without Nostr integration)
    pub fn new(cx: &mut Context<Self>) -> Self {
        let search_input = cx.new(|cx| TextInput::new("Search agents...", cx));
        let dvm_list = cx.new(|cx| DVMList::new(cx));

        Self {
            focus_handle: cx.focus_handle(),
            current_tab: MarketplaceTab::Agents,
            wallet_balance_sats: 142_847,
            trust_tier: TrustTier::Gold,
            earnings_today_sats: 1_247,
            is_online: true,
            connected_relays: 3,
            activity_feed_collapsed: false,
            transactions: mock_transactions(),
            notifications: mock_notifications(),
            search_input,
            agent_store_state: AgentStoreState::default(),
            compute_market_state: ComputeMarketState::default(),
            services_market_state: ServicesMarketState::default(),
            dvm_list,
            nostr_bridge: None,
            selected_dvm: None,
        }
    }

    /// Create a new MarketplaceScreen with Nostr integration for NIP-90 DVM support
    pub fn with_nostr(cx: &mut Context<Self>, chat_state: Arc<ChatState>) -> Self {
        let search_input = cx.new(|cx| TextInput::new("Search agents...", cx));
        let dvm_list = cx.new(|cx| DVMList::new(cx));
        let bridge = NostrBridge::new(chat_state);

        Self {
            focus_handle: cx.focus_handle(),
            current_tab: MarketplaceTab::Agents,
            wallet_balance_sats: 142_847,
            trust_tier: TrustTier::Gold,
            earnings_today_sats: 1_247,
            is_online: true,
            connected_relays: 3,
            activity_feed_collapsed: false,
            transactions: mock_transactions(),
            notifications: mock_notifications(),
            search_input,
            agent_store_state: AgentStoreState::default(),
            compute_market_state: ComputeMarketState::default(),
            services_market_state: ServicesMarketState::default(),
            dvm_list,
            nostr_bridge: Some(bridge),
            selected_dvm: None,
        }
    }

    /// Handle USE button click on a DVM
    pub fn on_dvm_use(&mut self, dvm: &DVMListing, cx: &mut Context<Self>) {
        // Store the selected DVM for potential dialog
        self.selected_dvm = Some(dvm.clone());

        // Add a notification
        self.notifications.insert(0, Notification {
            id: format!("dvm-selected-{}", dvm.id),
            kind: NotificationKind::SystemAlert,
            title: format!("Selected: {}", dvm.name),
            message: format!("Kind {} - {} sats{}", dvm.kind, dvm.sats_per_unit, dvm.pricing_unit.label()),
            read: false,
            timestamp: "now".to_string(),
        });

        cx.notify();
    }

    /// Check if Nostr integration is available
    pub fn has_nostr(&self) -> bool {
        self.nostr_bridge.is_some()
    }

    /// Process pending Nostr events (call from timer/frame callback)
    pub fn process_nostr_events(&mut self, cx: &mut Context<Self>) {
        if let Some(bridge) = &mut self.nostr_bridge {
            for event in bridge.poll_events() {
                match event {
                    ChatEvent::JobSubmitted { job_id, kind } => {
                        // Add notification for job submission
                        self.notifications.insert(0, Notification {
                            id: format!("job-submitted-{}", &job_id[..8]),
                            kind: NotificationKind::JobCompleted,
                            title: "Job Submitted".to_string(),
                            message: format!("Kind {} job sent to DVM", kind),
                            read: false,
                            timestamp: "now".to_string(),
                        });
                    }
                    ChatEvent::JobResult { job_id, content } => {
                        // Add notification for job result
                        let preview: String = content.chars().take(50).collect();
                        self.notifications.insert(0, Notification {
                            id: format!("job-result-{}", &job_id[..8]),
                            kind: NotificationKind::JobCompleted,
                            title: "Job Completed".to_string(),
                            message: preview,
                            read: false,
                            timestamp: "now".to_string(),
                        });
                    }
                    ChatEvent::JobStatusUpdate { job_id, status } => {
                        // Update notification for job status
                        self.notifications.insert(0, Notification {
                            id: format!("job-status-{}", &job_id[..8]),
                            kind: NotificationKind::JobCompleted,
                            title: format!("Job {}", status),
                            message: format!("Job {} status: {}", &job_id[..8], status),
                            read: false,
                            timestamp: "now".to_string(),
                        });
                    }
                    ChatEvent::Connected { relay_count } => {
                        self.connected_relays = relay_count as u32;
                    }
                    _ => {}
                }
            }
            cx.notify();
        }
    }

    /// Switch to a different tab
    pub fn set_tab(&mut self, tab: MarketplaceTab, cx: &mut Context<Self>) {
        self.current_tab = tab;
        cx.notify();
    }

    /// Toggle the activity feed
    pub fn toggle_activity_feed(&mut self, cx: &mut Context<Self>) {
        self.activity_feed_collapsed = !self.activity_feed_collapsed;
        cx.notify();
    }

    /// Toggle online status
    pub fn toggle_online(&mut self, cx: &mut Context<Self>) {
        self.is_online = !self.is_online;
        self.compute_market_state.is_online = self.is_online;
        cx.notify();
    }

    /// Render the resource bar
    fn render_resource_bar(&self) -> impl IntoElement {
        resource_bar::render(ResourceBarProps {
            wallet_balance_sats: self.wallet_balance_sats,
            trust_tier: self.trust_tier,
            earnings_today_sats: self.earnings_today_sats,
            is_online: self.is_online,
            connected_relays: self.connected_relays,
        })
    }

    /// Render a single tab button - Bloomberg style (no icons, sharp edges)
    fn render_tab_button(&self, tab: MarketplaceTab, cx: &mut Context<Self>) -> impl IntoElement {
        let is_active = tab == self.current_tab;
        let (bg_color, text_color, border_color) = if is_active {
            (bg::SELECTED, Hsla { h: 0.14, s: 1.0, l: 0.5, a: 1.0 }, border::SELECTED)  // Yellow when active
        } else {
            (Hsla::transparent_black(), text::MUTED, Hsla::transparent_black())
        };

        div()
            .id(SharedString::from(format!("tab-{}", tab.label())))
            .flex()
            .items_center()
            .px(px(12.0))
            .py(px(6.0))
            .bg(bg_color)
            .border_1()
            .border_color(border_color)
            // No rounded corners - Bloomberg style
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
                    .child(tab.label().to_uppercase()),
            )
    }

    /// Render the current tab content
    fn render_tab_content(&self, cx: &mut Context<Self>) -> AnyElement {
        match self.current_tab {
            MarketplaceTab::Agents => {
                div()
                    .flex_1()
                    .child(render_agent_store_with_input(&self.agent_store_state, self.search_input.clone()))
                    .into_any_element()
            }
            MarketplaceTab::Compute => {
                div()
                    .flex_1()
                    .child(render_compute_market(&self.compute_market_state))
                    .into_any_element()
            }
            MarketplaceTab::Services => {
                // Render services market with interactive DVM list
                self.render_services_tab(cx)
            }
        }
    }

    /// Render the services tab with interactive DVM list
    fn render_services_tab(&self, _cx: &mut Context<Self>) -> AnyElement {
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
            .child(self.render_search_bar())
            // Category filters
            .child(self.render_category_filters())
            // Interactive DVM list (with USE button callbacks)
            .child(self.dvm_list.clone())
            // MCP Servers
            .child(render_mcp_grid(&mock_mcp_servers()))
            .into_any_element()
    }

    /// Render search bar for services
    fn render_search_bar(&self) -> impl IntoElement {
        let query = &self.services_market_state.search_query;
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
    fn render_category_filters(&self) -> impl IntoElement {
        use crate::types::ServiceCategory;
        let selected = self.services_market_state.selected_category;
        div()
            .flex()
            .flex_wrap()
            .gap(px(6.0))
            .children(ServiceCategory::all().iter().map(|&cat| {
                self.render_category_chip(cat, cat == selected)
            }))
    }

    /// Render a category chip
    fn render_category_chip(&self, category: crate::types::ServiceCategory, is_selected: bool) -> impl IntoElement {
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

    /// Render the activity feed
    fn render_activity_feed(&self) -> impl IntoElement {
        activity_feed::render_activity_feed(
            &self.transactions,
            &self.notifications,
            self.activity_feed_collapsed,
        )
    }
}

impl Focusable for MarketplaceScreen {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for MarketplaceScreen {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
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
                    .h(px(48.0))
                    .w_full()
                    .flex()
                    .items_center()
                    .px(px(20.0))
                    .gap(px(4.0))
                    .bg(bg::SURFACE)
                    .border_b_1()
                    .border_color(border::DEFAULT)
                    .child(self.render_tab_button(MarketplaceTab::Agents, cx))
                    .child(self.render_tab_button(MarketplaceTab::Compute, cx))
                    .child(self.render_tab_button(MarketplaceTab::Services, cx)),
            )
            // Main content area with optional activity feed
            .child(
                div()
                    .flex()
                    .flex_1()
                    .overflow_hidden()
                    // Tab content
                    .child(self.render_tab_content(cx))
                    // Activity feed (right panel)
                    .child(self.render_activity_feed()),
            )
    }
}
