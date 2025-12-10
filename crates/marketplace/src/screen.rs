//! Main Marketplace screen - Orchestrates all components

use gpui::*;
use theme::{bg, border, text, FONT_FAMILY};

use crate::types::{MarketplaceTab, TrustTier, Transaction, Notification};
use crate::resource_bar::{self, ResourceBarProps};
use crate::activity_feed::{self, mock_transactions, mock_notifications};
use crate::agents::{AgentStoreState, render_agent_store_with_input};
use crate::compute::{ComputeMarketState, render_compute_market};
use crate::services::{ServicesMarketState, render_services_market};
use crate::text_input::TextInput;

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
}

impl MarketplaceScreen {
    /// Create a new MarketplaceScreen
    pub fn new(cx: &mut Context<Self>) -> Self {
        let search_input = cx.new(|cx| TextInput::new("Search agents...", cx));

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

    /// Render a single tab button
    fn render_tab_button(&self, tab: MarketplaceTab, cx: &mut Context<Self>) -> impl IntoElement {
        let is_active = tab == self.current_tab;
        let (bg_color, text_color, border_color) = if is_active {
            (bg::SELECTED, text::BRIGHT, border::SELECTED)
        } else {
            (Hsla::transparent_black(), text::MUTED, Hsla::transparent_black())
        };

        div()
            .id(SharedString::from(format!("tab-{}", tab.label())))
            .flex()
            .items_center()
            .gap(px(6.0))
            .px(px(16.0))
            .py(px(10.0))
            .bg(bg_color)
            .border_1()
            .border_color(border_color)
            .rounded(px(6.0))
            .cursor_pointer()
            .hover(|s| s.bg(bg::HOVER).text_color(text::PRIMARY))
            .on_click(cx.listener(move |this, _event, _window, cx| {
                this.set_tab(tab, cx);
            }))
            .child(
                div()
                    .text_size(px(13.0))
                    .child(tab.icon().to_string()),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text_color)
                    .child(tab.label().to_string()),
            )
    }

    /// Render the current tab content
    fn render_tab_content(&self) -> AnyElement {
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
                div()
                    .flex_1()
                    .child(render_services_market(&self.services_market_state))
                    .into_any_element()
            }
        }
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
                    .child(self.render_tab_content())
                    // Activity feed (right panel)
                    .child(self.render_activity_feed()),
            )
    }
}
