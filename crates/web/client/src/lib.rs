//! OpenAgents Web - WGPUI Landing Page
//!
//! Landing page with GitHub login and repo selector.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use serde::Deserialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use wgpui::{
    Bounds, Point, Quad, Scene, TextSystem, InputEvent, MouseButton, Key, NamedKey, Modifiers,
    EventContext, PaintContext, EventResult, Button, ButtonVariant, TextInput, WebPlatform,
    run_animation_loop, setup_resize_observer, theme,
};
use wgpui::components::hud::{StatusBar, StatusItem, StatusItemAlignment};
use wgpui::components::organisms::{ThreadEntry, ThreadEntryType};
use wgpui::components::sections::{
    CodeDiff, CodeLine, CodeLineKind, CodePane, LastPrSummary, MetricsPane, TerminalLine,
    TerminalPane, TerminalStream, ThreadView, UsageSummary,
};
use wgpui::components::Text;
use wgpui::components::atoms::{BitcoinNetwork, PaymentMethod, PaymentStatus};
use wgpui::components::molecules::{
    BalanceCard, InvoiceDisplay, InvoiceInfo, InvoiceType, PaymentDirection, PaymentInfo,
    PaymentRow, WalletBalance,
};

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    web_sys::console::log_1(&"OpenAgents initialized".into());
}

#[derive(Clone, Default)]
struct UserInfo {
    github_username: Option<String>,
    nostr_npub: Option<String>,
}

#[derive(Clone)]
struct RepoInfo {
    name: String,
    full_name: String,
    description: Option<String>,
    private: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum AppView {
    Landing,
    RepoSelector,
    RepoView,
}

/// Context from /repo/:owner/:repo route
#[derive(Clone, Default, Deserialize)]
#[serde(default)]
struct HudContext {
    username: String,
    repo: String,
    #[serde(default)]
    is_owner: bool,
    #[serde(default = "default_true")]
    is_public: bool,
    #[serde(default)]
    embed_mode: bool,
    #[serde(default)]
    agent_id: Option<String>,
    #[serde(default)]
    stream_url: Option<String>,
    // Session info for WebSocket connection
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    ws_url: Option<String>,
    #[serde(default = "default_status")]
    status: String, // "idle", "starting", "running", "completed", "failed"
}

fn default_true() -> bool {
    true
}

fn default_status() -> String {
    "idle".to_string()
}

#[derive(Clone, Deserialize)]
struct LiveIssue {
    label: String,
    url: String,
    #[serde(default)]
    title: Option<String>,
}

#[derive(Clone, Deserialize)]
struct LiveHudResponse {
    enabled: bool,
    #[serde(default)]
    hud_context: Option<HudContext>,
    #[serde(default)]
    issue: Option<LiveIssue>,
}

#[derive(Clone)]
struct LandingLive {
    hud_context: HudContext,
    issue: Option<LiveIssue>,
}

/// HUD event types from streaming or replay.
#[derive(Clone, Debug)]
enum HudEvent {
    SessionStart { session_id: Option<String> },
    SessionEnd { success: Option<bool> },
    TickStart { tick_id: Option<String>, cause: Option<String> },
    TickEnd { tick_id: Option<String>, success: Option<bool> },
    ToolStart { tool_name: String, tool_id: Option<String> },
    ToolDone { tool_id: Option<String>, output: Option<String>, success: Option<bool> },
    Chunk { text: String },
    FileDiff { path: String, lines: Vec<String>, additions: Option<u64>, deletions: Option<u64> },
    ContainerOutput { stream: TerminalStream, data: String },
    Usage { input_tokens: Option<u64>, output_tokens: Option<u64>, cost_usd: Option<f64> },
    Error { error: String },
}

#[derive(Clone, Debug, Default)]
struct HudSettingsData {
    public: bool,
    embed_allowed: bool,
    redaction_policy: String,
}

#[derive(Clone, Default)]
struct HudLayout {
    thread_bounds: Bounds,
    code_bounds: Bounds,
    terminal_bounds: Bounds,
    metrics_bounds: Bounds,
    status_bounds: Bounds,
    settings_public_bounds: Bounds,
    settings_embed_bounds: Bounds,
}

struct HudUi {
    thread: ThreadView,
    code: CodePane,
    terminal: TerminalPane,
    metrics: MetricsPane,
    status_bar: StatusBar,
    assistant_entry: Option<usize>,
    assistant_text: String,
    tool_entries: HashMap<String, usize>,
    status_text: String,
    settings: HudSettingsData,
}

impl HudUi {
    fn new() -> Self {
        let mut status_bar = StatusBar::new();
        status_bar.set_items(vec![
            StatusItem::text("status", "idle").left(),
            StatusItem::text("mode", "HUD").center(),
        ]);
        Self {
            thread: ThreadView::new().auto_scroll(true),
            code: CodePane::new().auto_scroll(true),
            terminal: TerminalPane::new().auto_scroll(true),
            metrics: MetricsPane::new(),
            status_bar,
            assistant_entry: None,
            assistant_text: String::new(),
            tool_entries: HashMap::new(),
            status_text: "idle".to_string(),
            settings: HudSettingsData {
                public: true,
                embed_allowed: true,
                redaction_policy: "standard".to_string(),
            },
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum WalletStatus {
    Loading,
    Ready,
    Partial,
    Error,
}

#[derive(Clone, Copy, PartialEq)]
enum WalletView {
    Overview,
    Send,
    Receive,
}

#[derive(Clone)]
struct WalletBalanceData {
    spark_sats: u64,
    lightning_sats: u64,
    onchain_sats: u64,
    total_sats: u64,
}

#[derive(Clone, Default)]
struct WalletAddressesData {
    spark: Option<String>,
    onchain: Option<String>,
}

#[derive(Clone)]
struct WalletPaymentData {
    id: String,
    amount_sats: u64,
    fee_sats: u64,
    direction: String,
    method: String,
    status: String,
    timestamp: String,
    description: Option<String>,
}

#[derive(Clone)]
struct WalletInvoiceData {
    method: String,
    payment_request: String,
    amount_sats: Option<u64>,
    description: Option<String>,
}

enum WalletAction {
    Refresh,
    SendPayment,
    ReceiveSpark,
    ReceiveLightning,
    ReceiveOnchain,
}

#[derive(Clone, Default)]
struct WalletLayout {
    bounds: Bounds,
    refresh_button: Bounds,
    tab_overview: Bounds,
    tab_send: Bounds,
    tab_receive: Bounds,
    send_address: Bounds,
    send_amount: Bounds,
    send_button: Bounds,
    receive_amount: Bounds,
    receive_spark_button: Bounds,
    receive_lightning_button: Bounds,
    receive_onchain_button: Bounds,
    payment_rows: Vec<Bounds>,
}

struct WalletUi {
    status: WalletStatus,
    view: WalletView,
    network: Option<String>,
    balance: Option<WalletBalanceData>,
    addresses: WalletAddressesData,
    payments: Vec<WalletPaymentData>,
    payment_rows: Vec<PaymentRow>,
    last_invoice: Option<WalletInvoiceData>,
    error: Option<String>,
    send_notice: Option<String>,
    receive_notice: Option<String>,
    send_address_input: TextInput,
    send_amount_input: TextInput,
    receive_amount_input: TextInput,
    send_button: Button,
    receive_spark_button: Button,
    receive_lightning_button: Button,
    receive_onchain_button: Button,
    refresh_button: Button,
    event_ctx: EventContext,
    layout: WalletLayout,
    actions: Rc<RefCell<Vec<WalletAction>>>,
}

impl WalletUi {
    fn new() -> Self {
        let actions = Rc::new(RefCell::new(Vec::new()));

        let send_actions = actions.clone();
        let receive_spark_actions = actions.clone();
        let receive_lightning_actions = actions.clone();
        let receive_onchain_actions = actions.clone();
        let refresh_actions = actions.clone();

        let send_button = Button::new("Send")
            .variant(ButtonVariant::Primary)
            .padding(14.0, 6.0)
            .on_click(move || {
                send_actions.borrow_mut().push(WalletAction::SendPayment);
            });

        let receive_spark_button = Button::new("Spark Address")
            .variant(ButtonVariant::Secondary)
            .padding(10.0, 6.0)
            .on_click(move || {
                receive_spark_actions
                    .borrow_mut()
                    .push(WalletAction::ReceiveSpark);
            });

        let receive_lightning_button = Button::new("Lightning Invoice")
            .variant(ButtonVariant::Secondary)
            .padding(10.0, 6.0)
            .on_click(move || {
                receive_lightning_actions
                    .borrow_mut()
                    .push(WalletAction::ReceiveLightning);
            });

        let receive_onchain_button = Button::new("On-chain Address")
            .variant(ButtonVariant::Secondary)
            .padding(10.0, 6.0)
            .on_click(move || {
                receive_onchain_actions
                    .borrow_mut()
                    .push(WalletAction::ReceiveOnchain);
            });

        let refresh_button = Button::new("Refresh")
            .variant(ButtonVariant::Secondary)
            .padding(10.0, 4.0)
            .on_click(move || {
                refresh_actions.borrow_mut().push(WalletAction::Refresh);
            });

        Self {
            status: WalletStatus::Loading,
            view: WalletView::Overview,
            network: None,
            balance: None,
            addresses: WalletAddressesData::default(),
            payments: Vec::new(),
            payment_rows: Vec::new(),
            last_invoice: None,
            error: None,
            send_notice: None,
            receive_notice: None,
            send_address_input: TextInput::new().placeholder("Invoice or address"),
            send_amount_input: TextInput::new().placeholder("Amount (sats)"),
            receive_amount_input: TextInput::new().placeholder("Amount (sats, optional)"),
            send_button,
            receive_spark_button,
            receive_lightning_button,
            receive_onchain_button,
            refresh_button,
            event_ctx: EventContext::new(),
            layout: WalletLayout::default(),
            actions,
        }
    }

    fn apply_summary(&mut self, summary: WalletSummaryData) {
        self.status = summary.status;
        self.network = summary.network;
        self.balance = summary.balance;
        self.addresses = summary.addresses;
        self.payments = summary.payments;
        self.error = summary.error;
        self.rebuild_rows();
    }

    fn set_invoice(&mut self, invoice: WalletInvoiceData) {
        self.last_invoice = Some(invoice);
        self.receive_notice = Some("Receive request ready".to_string());
    }

    fn rebuild_rows(&mut self) {
        self.payment_rows = self
            .payments
            .iter()
            .map(|payment| payment_row_from_data(payment))
            .collect();
    }

    fn clear_notices(&mut self) {
        self.send_notice = None;
        self.receive_notice = None;
    }

    fn take_actions(&self) -> Vec<WalletAction> {
        let mut actions = self.actions.borrow_mut();
        std::mem::take(&mut *actions)
    }

    fn set_view(&mut self, view: WalletView) {
        self.view = view;
        self.send_address_input.blur();
        self.send_amount_input.blur();
        self.receive_amount_input.blur();
        self.clear_notices();
    }

    fn handle_event(&mut self, event: &InputEvent) -> EventResult {
        if let InputEvent::MouseDown { button, x, y } = event {
            if *button == MouseButton::Left {
                let point = Point::new(*x, *y);
                if self.layout.tab_overview.contains(point) {
                    self.set_view(WalletView::Overview);
                } else if self.layout.tab_send.contains(point) {
                    self.set_view(WalletView::Send);
                } else if self.layout.tab_receive.contains(point) {
                    self.set_view(WalletView::Receive);
                }
            }
        }

        let mut handled = EventResult::Ignored;
        handled = merge_event_result(
            handled,
            self.refresh_button.event(event, self.layout.refresh_button, &mut self.event_ctx),
        );

        match self.view {
            WalletView::Overview => {
                for (row, bounds) in self.payment_rows.iter_mut().zip(self.layout.payment_rows.iter())
                {
                    handled = merge_event_result(
                        handled,
                        row.event(event, *bounds, &mut self.event_ctx),
                    );
                }
            }
            WalletView::Send => {
                handled = merge_event_result(
                    handled,
                    self.send_address_input.event(
                        event,
                        self.layout.send_address,
                        &mut self.event_ctx,
                    ),
                );
                handled = merge_event_result(
                    handled,
                    self.send_amount_input.event(
                        event,
                        self.layout.send_amount,
                        &mut self.event_ctx,
                    ),
                );
                handled = merge_event_result(
                    handled,
                    self.send_button.event(event, self.layout.send_button, &mut self.event_ctx),
                );
            }
            WalletView::Receive => {
                handled = merge_event_result(
                    handled,
                    self.receive_amount_input.event(
                        event,
                        self.layout.receive_amount,
                        &mut self.event_ctx,
                    ),
                );
                handled = merge_event_result(
                    handled,
                    self.receive_spark_button.event(
                        event,
                        self.layout.receive_spark_button,
                        &mut self.event_ctx,
                    ),
                );
                handled = merge_event_result(
                    handled,
                    self.receive_lightning_button.event(
                        event,
                        self.layout.receive_lightning_button,
                        &mut self.event_ctx,
                    ),
                );
                handled = merge_event_result(
                    handled,
                    self.receive_onchain_button.event(
                        event,
                        self.layout.receive_onchain_button,
                        &mut self.event_ctx,
                    ),
                );
            }
        }

        self.sync_focus();
        handled
    }

    fn sync_focus(&mut self) {
        if self.send_address_input.is_focused() {
            self.send_amount_input.blur();
            self.receive_amount_input.blur();
            return;
        }

        if self.send_amount_input.is_focused() {
            self.send_address_input.blur();
            self.receive_amount_input.blur();
            return;
        }

        if self.receive_amount_input.is_focused() {
            self.send_address_input.blur();
            self.send_amount_input.blur();
        }
    }

    fn has_focus(&self) -> bool {
        self.send_address_input.is_focused()
            || self.send_amount_input.is_focused()
            || self.receive_amount_input.is_focused()
    }

    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        self.layout = WalletLayout::default();
        self.layout.bounds = bounds;

        let padding = 10.0;
        let gap = 8.0;
        let section_gap = 12.0;
        let content_x = bounds.origin.x + padding;
        let content_w = bounds.size.width - padding * 2.0;
        let mut y = bounds.origin.y + padding;

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::ELEVATED)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let header = cx.text.layout(
            "Wallet",
            Point::new(content_x, y),
            12.0,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(header);

        let refresh_w = 72.0;
        let refresh_h = 22.0;
        let refresh_bounds = Bounds::new(
            bounds.origin.x + bounds.size.width - padding - refresh_w,
            y - 3.0,
            refresh_w,
            refresh_h,
        );
        self.refresh_button.paint(refresh_bounds, cx);
        self.layout.refresh_button = refresh_bounds;

        if let Some(network) = self.network.as_deref() {
            let net_label = cx.text.layout(
                network,
                Point::new(content_x + 56.0, y),
                10.0,
                theme::text::MUTED,
            );
            cx.scene.draw_text(net_label);
        }

        y += 28.0;

        if let Some(balance) = &self.balance {
            let mut balance_card = BalanceCard::new(WalletBalance::new(
                balance.spark_sats,
                balance.lightning_sats,
                balance.onchain_sats,
            ))
            .network(network_from_label(self.network.as_deref()))
            .show_breakdown(true);

            let card_bounds = Bounds::new(content_x, y, content_w, 180.0);
            balance_card.paint(card_bounds, cx);
            y = card_bounds.origin.y + card_bounds.size.height + section_gap;
        } else {
            let balance_label = if matches!(self.status, WalletStatus::Loading) {
                "Loading wallet..."
            } else {
                "Balance unavailable"
            };
            let balance_text = cx.text.layout(
                balance_label,
                Point::new(content_x, y),
                10.0,
                theme::text::MUTED,
            );
            cx.scene.draw_text(balance_text);
            y += 20.0 + section_gap;
        }

        let tab_height = 24.0;
        let tab_width = (content_w - gap * 2.0) / 3.0;
        let tab_y = y;

        self.layout.tab_overview = Bounds::new(content_x, tab_y, tab_width, tab_height);
        self.layout.tab_send = Bounds::new(content_x + tab_width + gap, tab_y, tab_width, tab_height);
        self.layout.tab_receive = Bounds::new(
            content_x + (tab_width + gap) * 2.0,
            tab_y,
            tab_width,
            tab_height,
        );

        draw_wallet_tab(
            cx,
            self.layout.tab_overview,
            "Overview",
            self.view == WalletView::Overview,
        );
        draw_wallet_tab(
            cx,
            self.layout.tab_send,
            "Send",
            self.view == WalletView::Send,
        );
        draw_wallet_tab(
            cx,
            self.layout.tab_receive,
            "Receive",
            self.view == WalletView::Receive,
        );

        y += tab_height + section_gap;

        match self.view {
            WalletView::Overview => {
                let addr_label = cx.text.layout(
                    "Addresses",
                    Point::new(content_x, y),
                    10.0,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(addr_label);
                y += 14.0;

                if let Some(address) = self.addresses.spark.as_deref() {
                    let info = InvoiceInfo::new(InvoiceType::SparkAddress, address)
                        .status(PaymentStatus::Pending);
                    let mut display = InvoiceDisplay::new(info).show_qr(false).compact(true);
                    let bounds = Bounds::new(content_x, y, content_w, 120.0);
                    display.paint(bounds, cx);
                    y = bounds.origin.y + bounds.size.height + gap;
                } else {
                    let msg = cx.text.layout(
                        "Spark address unavailable",
                        Point::new(content_x, y),
                        10.0,
                        theme::text::MUTED,
                    );
                    cx.scene.draw_text(msg);
                    y += 16.0 + gap;
                }

                if let Some(address) = self.addresses.onchain.as_deref() {
                    let info = InvoiceInfo::new(InvoiceType::OnChainAddress, address)
                        .status(PaymentStatus::Pending);
                    let mut display = InvoiceDisplay::new(info).show_qr(false).compact(true);
                    let bounds = Bounds::new(content_x, y, content_w, 120.0);
                    display.paint(bounds, cx);
                    y = bounds.origin.y + bounds.size.height + section_gap;
                } else {
                    let msg = cx.text.layout(
                        "On-chain address unavailable",
                        Point::new(content_x, y),
                        10.0,
                        theme::text::MUTED,
                    );
                    cx.scene.draw_text(msg);
                    y += 16.0 + section_gap;
                }

                let history_label = cx.text.layout(
                    "Recent Payments",
                    Point::new(content_x, y),
                    10.0,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(history_label);
                y += 16.0;

                self.layout.payment_rows.clear();
                if self.payment_rows.is_empty() {
                    let empty = cx.text.layout(
                        "No payments yet",
                        Point::new(content_x, y),
                        10.0,
                        theme::text::MUTED,
                    );
                    cx.scene.draw_text(empty);
                } else {
                    let row_h = 56.0;
                    let available_h = bounds.origin.y + bounds.size.height - y - gap;
                    let max_rows = if available_h >= row_h {
                        (available_h / (row_h + gap)).floor().max(1.0) as usize
                    } else {
                        0
                    };
                    for (idx, row) in self.payment_rows.iter_mut().take(max_rows).enumerate() {
                        let row_bounds = Bounds::new(
                            content_x,
                            y + idx as f32 * (row_h + gap),
                            content_w,
                            row_h,
                        );
                        row.paint(row_bounds, cx);
                        self.layout.payment_rows.push(row_bounds);
                    }
                }
            }
            WalletView::Send => {
                let send_label = cx.text.layout(
                    "Send",
                    Point::new(content_x, y),
                    10.0,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(send_label);
                y += 14.0;

                let address_bounds = Bounds::new(content_x, y, content_w, 32.0);
                self.send_address_input.paint(address_bounds, cx);
                self.layout.send_address = address_bounds;
                y += 32.0 + gap;

                let amount_bounds = Bounds::new(content_x, y, content_w, 32.0);
                self.send_amount_input.paint(amount_bounds, cx);
                self.layout.send_amount = amount_bounds;
                y += 32.0 + gap;

                let button_bounds = Bounds::new(content_x, y, 120.0, 30.0);
                self.send_button.paint(button_bounds, cx);
                self.layout.send_button = button_bounds;
                y += 30.0 + gap;

                if let Some(notice) = self.send_notice.as_deref() {
                    let color = if notice.starts_with("Error:") {
                        theme::status::ERROR
                    } else {
                        theme::status::SUCCESS
                    };
                    let note = cx.text.layout(
                        notice,
                        Point::new(content_x, y),
                        10.0,
                        color,
                    );
                    cx.scene.draw_text(note);
                }
            }
            WalletView::Receive => {
                let receive_label = cx.text.layout(
                    "Receive",
                    Point::new(content_x, y),
                    10.0,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(receive_label);
                y += 14.0;

                let amount_bounds = Bounds::new(content_x, y, content_w, 32.0);
                self.receive_amount_input.paint(amount_bounds, cx);
                self.layout.receive_amount = amount_bounds;
                y += 32.0 + gap;

                let btn_w = (content_w - gap * 2.0) / 3.0;
                let btn_h = 28.0;

                let spark_bounds = Bounds::new(content_x, y, btn_w, btn_h);
                self.receive_spark_button.paint(spark_bounds, cx);
                self.layout.receive_spark_button = spark_bounds;

                let lightning_bounds = Bounds::new(content_x + btn_w + gap, y, btn_w, btn_h);
                self.receive_lightning_button.paint(lightning_bounds, cx);
                self.layout.receive_lightning_button = lightning_bounds;

                let onchain_bounds =
                    Bounds::new(content_x + (btn_w + gap) * 2.0, y, btn_w, btn_h);
                self.receive_onchain_button.paint(onchain_bounds, cx);
                self.layout.receive_onchain_button = onchain_bounds;

                y += btn_h + section_gap;

                if let Some(invoice) = &self.last_invoice {
                    let invoice_type = match invoice.method.as_str() {
                        "lightning" => InvoiceType::Bolt11,
                        "onchain" | "bitcoin" => InvoiceType::OnChainAddress,
                        _ => InvoiceType::SparkAddress,
                    };
                    let mut info = InvoiceInfo::new(invoice_type, &invoice.payment_request)
                        .status(PaymentStatus::Pending);
                    if let Some(amount) = invoice.amount_sats {
                        info = info.amount(amount);
                    }
                    if let Some(desc) = invoice.description.as_deref() {
                        info = info.description(desc);
                    }
                    let show_qr = invoice.method == "lightning";
                    let mut display = InvoiceDisplay::new(info).show_qr(show_qr);
                    let bounds = Bounds::new(content_x, y, content_w, if show_qr { 280.0 } else { 160.0 });
                    display.paint(bounds, cx);
                } else if let Some(notice) = self.receive_notice.as_deref() {
                    let note = cx.text.layout(
                        notice,
                        Point::new(content_x, y),
                        10.0,
                        theme::status::SUCCESS,
                    );
                    cx.scene.draw_text(note);
                }
            }
        }

        if let Some(err) = self.error.as_deref() {
            let error_text = cx.text.layout(
                err,
                Point::new(content_x, bounds.origin.y + bounds.size.height - 18.0),
                9.0,
                theme::status::ERROR,
            );
            cx.scene.draw_text(error_text);
        }
    }
}

#[derive(Clone)]
struct WalletSummaryData {
    status: WalletStatus,
    network: Option<String>,
    balance: Option<WalletBalanceData>,
    addresses: WalletAddressesData,
    payments: Vec<WalletPaymentData>,
    error: Option<String>,
}

#[derive(Clone)]
struct WalletSendData {
    payment_id: String,
    status: String,
    method: String,
    amount_sats: u64,
    fee_sats: u64,
}

fn merge_event_result(lhs: EventResult, rhs: EventResult) -> EventResult {
    if matches!(lhs, EventResult::Handled) || matches!(rhs, EventResult::Handled) {
        EventResult::Handled
    } else {
        EventResult::Ignored
    }
}

fn draw_wallet_tab(cx: &mut PaintContext, bounds: Bounds, label: &str, active: bool) {
    let bg = if active {
        theme::accent::PRIMARY.with_alpha(0.25)
    } else {
        theme::bg::SURFACE
    };
    let border = if active {
        theme::accent::PRIMARY
    } else {
        theme::border::DEFAULT
    };
    cx.scene.draw_quad(
        Quad::new(bounds)
            .with_background(bg)
            .with_border(border, 1.0),
    );

    let label_run = cx.text.layout(
        label,
        Point::new(bounds.origin.x + 6.0, bounds.origin.y + 6.0),
        10.0,
        if active { theme::accent::PRIMARY } else { theme::text::MUTED },
    );
    cx.scene.draw_text(label_run);
}

fn payment_row_from_data(payment: &WalletPaymentData) -> PaymentRow {
    let direction = match payment.direction.as_str() {
        "receive" => PaymentDirection::Receive,
        _ => PaymentDirection::Send,
    };
    let method = match payment.method.as_str() {
        "spark" => PaymentMethod::Spark,
        "onchain" | "bitcoin" => PaymentMethod::OnChain,
        "token" => PaymentMethod::Token,
        "deposit" => PaymentMethod::Deposit,
        "withdraw" => PaymentMethod::Withdraw,
        _ => PaymentMethod::Lightning,
    };
    let status = match payment.status.as_str() {
        "completed" => PaymentStatus::Completed,
        "failed" => PaymentStatus::Failed,
        _ => PaymentStatus::Pending,
    };

    let mut info = PaymentInfo::new(payment.id.clone(), payment.amount_sats, direction)
        .fee(payment.fee_sats)
        .method(method)
        .status(status)
        .timestamp(payment.timestamp.clone());

    if let Some(desc) = payment.description.as_deref() {
        info = info.description(desc);
    }

    PaymentRow::new(info)
}

fn network_from_label(label: Option<&str>) -> BitcoinNetwork {
    match label.unwrap_or("testnet") {
        "mainnet" => BitcoinNetwork::Mainnet,
        "signet" => BitcoinNetwork::Signet,
        "regtest" => BitcoinNetwork::Regtest,
        _ => BitcoinNetwork::Testnet,
    }
}
struct AppState {
    mouse_pos: Point,
    button_hovered: bool,
    button_bounds: Bounds,
    landing_issue_bounds: Bounds,
    landing_issue_url: Option<String>,
    landing_live: Option<LandingLive>,
    user: UserInfo,
    loading: bool,
    view: AppView,
    repos: Vec<RepoInfo>,
    repos_loading: bool,
    hovered_repo_idx: Option<usize>,
    repo_bounds: Vec<Bounds>,
    selected_repo: Option<String>,
    scroll_offset: f32,
    // For RepoView
    hud_context: Option<HudContext>,
    hud_ui: HudUi,
    hud_layout: HudLayout,
    hud_stream: Option<HudStreamHandle>,
    hud_settings_loaded: bool,
    hud_metrics_polling: bool,
    hud_metrics_timer: Option<i32>,
    wallet: WalletUi,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            mouse_pos: Point::ZERO,
            button_hovered: false,
            button_bounds: Bounds::ZERO,
            landing_issue_bounds: Bounds::ZERO,
            landing_issue_url: None,
            landing_live: None,
            user: UserInfo::default(),
            loading: true,
            view: AppView::Landing,
            repos: Vec::new(),
            repos_loading: false,
            hovered_repo_idx: None,
            repo_bounds: Vec::new(),
            selected_repo: None,
            scroll_offset: 0.0,
            hud_context: None,
            hud_ui: HudUi::new(),
            hud_layout: HudLayout::default(),
            hud_stream: None,
            hud_settings_loaded: false,
            hud_metrics_polling: false,
            hud_metrics_timer: None,
            wallet: WalletUi::new(),
        }
    }
}

/// Check if window.HUD_CONTEXT exists (we're on /repo/:owner/:repo)
fn get_hud_context() -> Option<HudContext> {
    let window = web_sys::window()?;
    let context = js_sys::Reflect::get(&window, &"HUD_CONTEXT".into()).ok()?;

    if context.is_undefined() || context.is_null() {
        return None;
    }

    let username = js_sys::Reflect::get(&context, &"username".into())
        .ok()
        .and_then(|v| v.as_string())?;
    let repo = js_sys::Reflect::get(&context, &"repo".into())
        .ok()
        .and_then(|v| v.as_string())?;
    let is_owner = js_sys::Reflect::get(&context, &"is_owner".into())
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let is_public = js_sys::Reflect::get(&context, &"is_public".into())
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let embed_mode = js_sys::Reflect::get(&context, &"embed_mode".into())
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let agent_id = js_sys::Reflect::get(&context, &"agent_id".into())
        .ok()
        .and_then(|v| js_optional_string(&v));
    let stream_url = js_sys::Reflect::get(&context, &"stream_url".into())
        .ok()
        .and_then(|v| js_optional_string(&v));
    let session_id = js_sys::Reflect::get(&context, &"session_id".into())
        .ok()
        .and_then(|v| js_optional_string(&v));
    let ws_url = js_sys::Reflect::get(&context, &"ws_url".into())
        .ok()
        .and_then(|v| js_optional_string(&v));
    let status = js_sys::Reflect::get(&context, &"status".into())
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_else(|| "idle".to_string());

    Some(HudContext {
        username,
        repo,
        is_owner,
        is_public,
        embed_mode,
        agent_id,
        stream_url,
        session_id,
        ws_url,
        status,
    })
}

fn parse_hud_event(data: &str) -> Option<HudEvent> {
    if let Ok(obj) = serde_json::from_str::<serde_json::Value>(data) {
        let event_type = obj
            .get("event_type")
            .or_else(|| obj.get("type"))
            .and_then(|v| v.as_str())
            .unwrap_or("chunk");

        return match event_type {
            "session_start" => Some(HudEvent::SessionStart {
                session_id: obj.get("session_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
            }),
            "session_end" => Some(HudEvent::SessionEnd {
                success: obj.get("success").and_then(|v| v.as_bool()),
            }),
            "tick_start" => Some(HudEvent::TickStart {
                tick_id: obj.get("tick_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
                cause: obj.get("cause").and_then(|v| v.as_str()).map(|s| s.to_string()),
            }),
            "tick_end" => Some(HudEvent::TickEnd {
                tick_id: obj.get("tick_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
                success: obj.get("success").and_then(|v| v.as_bool()),
            }),
            "tool_start" => Some(HudEvent::ToolStart {
                tool_name: obj
                    .get("tool_name")
                    .or_else(|| obj.get("tool"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("tool")
                    .to_string(),
                tool_id: obj.get("tool_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
            }),
            "tool_done" => Some(HudEvent::ToolDone {
                tool_id: obj.get("tool_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
                output: obj
                    .get("result")
                    .or_else(|| obj.get("output"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                success: obj.get("success").and_then(|v| v.as_bool()),
            }),
            "chunk" => Some(HudEvent::Chunk {
                text: obj
                    .get("text")
                    .or_else(|| obj.pointer("/delta/text"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            }),
            "file_diff" => Some(HudEvent::FileDiff {
                path: obj
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
                lines: collect_diff_lines(&obj),
                additions: obj.get("additions").and_then(|v| v.as_u64()),
                deletions: obj.get("deletions").and_then(|v| v.as_u64()),
            }),
            "container_output" => Some(HudEvent::ContainerOutput {
                stream: match obj.get("stream").and_then(|v| v.as_str()) {
                    Some("stderr") => TerminalStream::Stderr,
                    _ => TerminalStream::Stdout,
                },
                data: obj
                    .get("data")
                    .or_else(|| obj.get("text"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            }),
            "usage" => Some(HudEvent::Usage {
                input_tokens: obj.get("input_tokens").and_then(|v| v.as_u64()),
                output_tokens: obj.get("output_tokens").and_then(|v| v.as_u64()),
                cost_usd: obj.get("cost_usd").and_then(|v| v.as_f64()),
            }),
            "error" => Some(HudEvent::Error {
                error: obj
                    .get("message")
                    .or_else(|| obj.get("error"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("error")
                    .to_string(),
            }),
            _ => None,
        };
    }

    Some(HudEvent::Chunk {
        text: data.to_string(),
    })
}

fn collect_diff_lines(obj: &serde_json::Value) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(hunks) = obj.get("hunks").and_then(|v| v.as_array()) {
        for hunk in hunks {
            if let Some(hunk_lines) = hunk.get("lines").and_then(|v| v.as_array()) {
                for line in hunk_lines {
                    if let Some(text) = line.as_str() {
                        lines.push(text.to_string());
                    }
                }
            } else if let Some(text) = hunk.as_str() {
                lines.extend(text.lines().map(|l| l.to_string()));
            }
        }
    } else if let Some(diff) = obj.get("diff").and_then(|v| v.as_str()) {
        lines.extend(diff.lines().map(|l| l.to_string()));
    }
    lines
}

fn apply_hud_event(hud: &mut HudUi, event: HudEvent) {
    match event {
        HudEvent::SessionStart { session_id } => {
            let label = session_id
                .map(|id| format!("session {}", id))
                .unwrap_or_else(|| "session started".to_string());
            hud.status_text = label;
        }
        HudEvent::SessionEnd { success } => {
            hud.status_text = if success == Some(true) {
                "session complete".to_string()
            } else {
                "session ended".to_string()
            };
        }
        HudEvent::TickStart { tick_id, cause } => {
            let mut text = "tick start".to_string();
            if let Some(id) = tick_id {
                text = format!("{} {}", text, id);
            }
            if let Some(cause) = cause {
                text = format!("{} ({})", text, cause);
            }
            hud.thread
                .push_entry(ThreadEntry::new(ThreadEntryType::System, Text::new(text)));
        }
        HudEvent::TickEnd { tick_id, success } => {
            let mut text = "tick end".to_string();
            if let Some(id) = tick_id {
                text = format!("{} {}", text, id);
            }
            if let Some(success) = success {
                text = format!("{} {}", text, if success { "ok" } else { "fail" });
            }
            hud.thread
                .push_entry(ThreadEntry::new(ThreadEntryType::System, Text::new(text)));
        }
        HudEvent::ToolStart { tool_name, tool_id } => {
            let text = format!("tool start {}", tool_name);
            hud.thread
                .push_entry(ThreadEntry::new(ThreadEntryType::Tool, Text::new(text)));
            if let Some(id) = tool_id {
                let idx = hud.thread.entry_count().saturating_sub(1);
                hud.tool_entries.insert(id, idx);
            }
            hud.assistant_entry = None;
        }
        HudEvent::ToolDone { tool_id, output, success } => {
            let summary = output.unwrap_or_default();
            let suffix = if let Some(success) = success {
                if success { "ok" } else { "fail" }
            } else {
                "done"
            };
            if let Some(id) = tool_id {
                if let Some(idx) = hud.tool_entries.get(&id).copied() {
                    if let Some(entry) = hud.thread.entry_mut(idx) {
                        let content = format!("tool {} {}", suffix, summary);
                        entry.set_content(Text::new(content));
                        return;
                    }
                }
            }
            let content = format!("tool {} {}", suffix, summary);
            hud.thread
                .push_entry(ThreadEntry::new(ThreadEntryType::Tool, Text::new(content)));
        }
        HudEvent::Chunk { text } => {
            if text.is_empty() {
                return;
            }
            hud.assistant_text.push_str(&text);
            if let Some(idx) = hud.assistant_entry {
                if let Some(entry) = hud.thread.entry_mut(idx) {
                    entry.set_content(Text::new(hud.assistant_text.clone()));
                    return;
                }
            }
            hud.thread.push_entry(ThreadEntry::new(
                ThreadEntryType::Assistant,
                Text::new(hud.assistant_text.clone()),
            ));
            hud.assistant_entry = Some(hud.thread.entry_count().saturating_sub(1));
        }
        HudEvent::FileDiff { path, lines, additions, deletions } => {
            let mut diff_lines = Vec::new();
            for line in lines {
                let (kind, text) = if let Some(rest) = line.strip_prefix('+') {
                    (CodeLineKind::Add, rest)
                } else if let Some(rest) = line.strip_prefix('-') {
                    (CodeLineKind::Remove, rest)
                } else {
                    (CodeLineKind::Context, line.as_str())
                };
                diff_lines.push(CodeLine::new(kind, text));
            }
            let diff = CodeDiff::new(path)
                .additions(additions.unwrap_or(0) as usize)
                .deletions(deletions.unwrap_or(0) as usize)
                .lines(diff_lines);
            hud.code.push_diff(diff);
        }
        HudEvent::ContainerOutput { stream, data } => {
            for line in data.lines() {
                if !line.is_empty() {
                    hud.terminal
                        .push_line(TerminalLine::new(stream.clone(), line.to_string()));
                }
            }
        }
        HudEvent::Usage { input_tokens, output_tokens, cost_usd } => {
            let usage = UsageSummary {
                input_tokens: input_tokens.unwrap_or(0),
                output_tokens: output_tokens.unwrap_or(0),
                cost_usd: cost_usd.unwrap_or(0.0),
            };
            hud.metrics.set_usage(Some(usage));
        }
        HudEvent::Error { error } => {
            hud.status_text = "error".to_string();
            hud.thread
                .push_entry(ThreadEntry::new(ThreadEntryType::Error, Text::new(error)));
        }
    }
}

enum HudStreamHandle {
    EventSource(web_sys::EventSource),
    WebSocket(web_sys::WebSocket),
}

impl HudStreamHandle {
    fn close(self) {
        match self {
            HudStreamHandle::EventSource(source) => {
                source.close();
            }
            HudStreamHandle::WebSocket(ws) => {
                let _ = ws.close();
            }
        }
    }
}

fn connect_event_source(state: Rc<RefCell<AppState>>, stream_url: &str) -> Option<HudStreamHandle> {
    let source = web_sys::EventSource::new(stream_url).ok()?;
    let state_clone = state.clone();
    let onmessage = Closure::<dyn FnMut(_)>::new(move |event: web_sys::MessageEvent| {
        if let Some(data) = event.data().as_string() {
            if let Some(hud_event) = parse_hud_event(&data) {
                let mut state = state_clone.borrow_mut();
                apply_hud_event(&mut state.hud_ui, hud_event);
            }
        }
    });
    source.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();

    let state_clone = state.clone();
    let onerror = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::Event| {
        let mut state = state_clone.borrow_mut();
        state.hud_ui.status_text = "stream error".to_string();
    });
    source.set_onerror(Some(onerror.as_ref().unchecked_ref()));
    onerror.forget();

    let state_clone = state.clone();
    let onopen = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::Event| {
        let mut state = state_clone.borrow_mut();
        state.hud_ui.status_text = "streaming".to_string();
    });
    source.set_onopen(Some(onopen.as_ref().unchecked_ref()));
    onopen.forget();

    Some(HudStreamHandle::EventSource(source))
}

fn connect_websocket(state: Rc<RefCell<AppState>>, ws_url: &str) -> Option<HudStreamHandle> {
    let window = web_sys::window()?;
    let protocol = if window.location().protocol().unwrap_or_default() == "https:" {
        "wss:"
    } else {
        "ws:"
    };
    let host = window.location().host().unwrap_or_default();
    let full_url = format!("{}//{}{}", protocol, host, ws_url);

    let ws = web_sys::WebSocket::new(&full_url).ok()?;
    let state_clone = state.clone();
    let onmessage = Closure::<dyn FnMut(_)>::new(move |event: web_sys::MessageEvent| {
        if let Some(data) = event.data().as_string() {
            if let Some(hud_event) = parse_hud_event(&data) {
                let mut state = state_clone.borrow_mut();
                apply_hud_event(&mut state.hud_ui, hud_event);
            }
        }
    });
    ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();

    let state_clone = state.clone();
    let onopen = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::Event| {
        let mut state = state_clone.borrow_mut();
        state.hud_ui.status_text = "streaming".to_string();
    });
    ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
    onopen.forget();

    let state_clone = state.clone();
    let onclose = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::CloseEvent| {
        let mut state = state_clone.borrow_mut();
        state.hud_ui.status_text = "disconnected".to_string();
    });
    ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
    onclose.forget();

    Some(HudStreamHandle::WebSocket(ws))
}

/// Fetch current user from /api/auth/me
async fn fetch_current_user() -> Option<UserInfo> {
    let window = web_sys::window()?;
    let resp = JsFuture::from(window.fetch_with_str("/api/auth/me")).await.ok()?;
    let resp: web_sys::Response = resp.dyn_into().ok()?;

    if !resp.ok() {
        return None;
    }

    let json = JsFuture::from(resp.json().ok()?).await.ok()?;
    let obj = js_sys::Object::from(json);

    let username = js_sys::Reflect::get(&obj, &"github_username".into()).ok()?;
    if username.is_undefined() || username.is_null() {
        return None;
    }

    let nostr_npub = js_sys::Reflect::get(&obj, &"nostr_npub".into())
        .ok()
        .and_then(|v| v.as_string());

    username
        .as_string()
        .map(|github_username| UserInfo {
            github_username: Some(github_username),
            nostr_npub,
        })
}

async fn fetch_live_hud() -> Option<LandingLive> {
    let window = web_sys::window()?;
    let resp = JsFuture::from(window.fetch_with_str("/api/hud/live")).await.ok()?;
    let resp: web_sys::Response = resp.dyn_into().ok()?;
    if !resp.ok() {
        return None;
    }

    let json = JsFuture::from(resp.json().ok()?).await.ok()?;
    let payload: LiveHudResponse = serde_json::from_str(
        &js_sys::JSON::stringify(&json)
            .ok()?
            .as_string()?,
    )
    .ok()?;
    if !payload.enabled {
        return None;
    }
    let context = payload.hud_context?;
    Some(LandingLive {
        hud_context: context,
        issue: payload.issue,
    })
}

/// Fetch repos from /api/repos
async fn fetch_repos() -> Vec<RepoInfo> {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return Vec::new(),
    };

    let resp = match JsFuture::from(window.fetch_with_str("/api/repos")).await {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let resp: web_sys::Response = match resp.dyn_into() {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    if !resp.ok() {
        return Vec::new();
    }

    let json = match resp.json() {
        Ok(p) => match JsFuture::from(p).await {
            Ok(j) => j,
            Err(_) => return Vec::new(),
        },
        Err(_) => return Vec::new(),
    };

    let arr = match js_sys::Array::try_from(json) {
        Ok(a) => a,
        Err(_) => return Vec::new(),
    };

    let mut repos = Vec::new();
    for i in 0..arr.length() {
        let obj = arr.get(i);
        let name = js_sys::Reflect::get(&obj, &"name".into())
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_default();
        let full_name = js_sys::Reflect::get(&obj, &"full_name".into())
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_default();
        let description = js_sys::Reflect::get(&obj, &"description".into())
            .ok()
            .and_then(|v| if v.is_null() { None } else { v.as_string() });
        let private = js_sys::Reflect::get(&obj, &"private".into())
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        repos.push(RepoInfo {
            name,
            full_name,
            description,
            private,
        });
    }

    repos
}

fn init_hud_runtime(state: Rc<RefCell<AppState>>) {
    let (context, replay_speed) = {
        let state = state.borrow();
        (state.hud_context.clone(), replay_speed_from_query())
    };

    let Some(context) = context else {
        return;
    };

    if let Some(speed) = replay_speed {
        if let Some(agent_id) = context.agent_id.clone() {
            start_replay(state, agent_id, speed);
        } else {
            state.borrow_mut().hud_ui.status_text = "replay unavailable".to_string();
        }
        return;
    }

    if state.borrow().hud_stream.is_none() {
        let stream_url = context
            .stream_url
            .clone()
            .or_else(|| {
                context
                    .agent_id
                    .as_ref()
                    .map(|id| format!("/agents/{}/hud/stream?watch=1", id))
            });
        let handle = if let Some(url) = stream_url.as_deref() {
            connect_event_source(state.clone(), url)
        } else if let Some(ws_url) = context.ws_url.as_deref() {
            connect_websocket(state.clone(), ws_url)
        } else {
            None
        };
        state.borrow_mut().hud_stream = handle;
    }

    if let Some(agent_id) = context.agent_id.clone() {
        start_metrics_poll(state.clone(), agent_id.clone());
    }

    if context.is_owner && !state.borrow().hud_settings_loaded {
        let state_clone = state.clone();
        wasm_bindgen_futures::spawn_local(async move {
            if let Some(repo) = state_clone
                .borrow()
                .hud_context
                .as_ref()
                .map(|ctx| format!("{}/{}", ctx.username, ctx.repo))
            {
                if let Some(settings) = fetch_hud_settings(&repo).await {
                    let mut state = state_clone.borrow_mut();
                    state.hud_ui.settings = settings;
                    state.hud_settings_loaded = true;
                }
            }
        });
    }
}

async fn ensure_hud_session(state: Rc<RefCell<AppState>>, repo: String) {
    let existing = fetch_hud_session(&repo).await;
    let session = match existing {
        Some(session) if session.session_id.is_some() => Some(session),
        Some(session) if session.can_start == Some(true) => {
            start_hud_session(&repo, DEFAULT_AUTOPILOT_PROMPT).await.ok()
        }
        Some(session) => Some(session),
        None => start_hud_session(&repo, DEFAULT_AUTOPILOT_PROMPT).await.ok(),
    };

    let Some(session) = session else {
        let mut guard = state.borrow_mut();
        guard.hud_ui.status_text = "start failed".to_string();
        return;
    };

    let matches_repo = state
        .borrow()
        .hud_context
        .as_ref()
        .map(|ctx| format!("{}/{}", ctx.username, ctx.repo) == repo)
        .unwrap_or(false);
    if !matches_repo {
        return;
    }

    {
        let mut guard = state.borrow_mut();
        if let Some(ctx) = guard.hud_context.as_mut() {
            ctx.session_id = session.session_id.clone();
            ctx.ws_url = session.ws_url.clone();
            ctx.status = session.status.clone();
        }
        guard.hud_ui.status_text = session.status.clone();
        guard.hud_settings_loaded = false;
        if let Some(handle) = guard.hud_stream.take() {
            handle.close();
        }
    }

    init_hud_runtime(state);
}

fn replay_speed_from_query() -> Option<f64> {
    let replay = query_param("replay");
    if let Some(value) = replay {
        if value.is_empty() {
            return Some(20.0);
        }
        return value.parse::<f64>().ok().or(Some(20.0));
    }
    query_param("speed").and_then(|v| v.parse::<f64>().ok())
}

fn query_param(name: &str) -> Option<String> {
    let window = web_sys::window()?;
    let search = window.location().search().ok()?;
    let params = web_sys::UrlSearchParams::new_with_str(&search).ok()?;
    params.get(name)
}

#[derive(Default)]
struct MetricsPayload {
    apm: Option<f32>,
    queue_depth: Option<u64>,
    oldest_issue: Option<String>,
    last_pr: LastPrSummary,
}

fn start_metrics_poll(state: Rc<RefCell<AppState>>, agent_id: String) {
    {
        let mut guard = state.borrow_mut();
        if guard.hud_metrics_polling {
            return;
        }
        guard.hud_metrics_polling = true;
    }

    let window = match web_sys::window() {
        Some(window) => window,
        None => return,
    };

    let state_clone = state.clone();
    let closure = Closure::<dyn FnMut()>::new(move || {
        let state_inner = state_clone.clone();
        let agent_id = agent_id.clone();
        wasm_bindgen_futures::spawn_local(async move {
            if let Some(payload) = fetch_metrics(&agent_id).await {
                let mut state = state_inner.borrow_mut();
                state.hud_ui.metrics.set_apm(payload.apm);
                state
                    .hud_ui
                    .metrics
                    .set_queue(payload.queue_depth, payload.oldest_issue);
                state.hud_ui.metrics.set_last_pr(payload.last_pr);
            }
        });
    });

    let interval_id = window.set_interval_with_callback_and_timeout_and_arguments_0(
        closure.as_ref().unchecked_ref(),
        4000,
    );
    if let Ok(id) = interval_id {
        let mut guard = state.borrow_mut();
        guard.hud_metrics_timer = Some(id);
    }
    closure.forget();
}

fn stop_metrics_poll(state: &mut AppState) {
    if let Some(window) = web_sys::window() {
        if let Some(id) = state.hud_metrics_timer.take() {
            window.clear_interval_with_handle(id);
        }
    }
    state.hud_metrics_polling = false;
}

async fn fetch_metrics(agent_id: &str) -> Option<MetricsPayload> {
    let apm = fetch_metric_json(&format!("/agents/{}/metrics/apm", agent_id)).await;
    let queue = fetch_metric_json(&format!("/agents/{}/metrics/queue", agent_id)).await;
    let last_pr = fetch_metric_json(&format!("/agents/{}/metrics/last_pr", agent_id)).await;

    let apm_value = apm
        .as_ref()
        .and_then(|value| value.get("value"))
        .and_then(|value| value.as_f64())
        .map(|value| value as f32);

    let queue_depth = queue
        .as_ref()
        .and_then(|value| value.get("depth"))
        .and_then(|value| value.as_u64());
    let oldest_issue = queue
        .as_ref()
        .and_then(|value| value.get("oldest_issue"))
        .and_then(|value| value.as_str())
        .map(|s| s.to_string());

    let last_pr_summary = LastPrSummary {
        url: last_pr
            .as_ref()
            .and_then(|value| value.get("url"))
            .and_then(|value| value.as_str())
            .map(|s| s.to_string()),
        title: last_pr
            .as_ref()
            .and_then(|value| value.get("title"))
            .and_then(|value| value.as_str())
            .map(|s| s.to_string()),
        merged: last_pr
            .as_ref()
            .and_then(|value| value.get("merged"))
            .and_then(|value| value.as_bool()),
    };

    Some(MetricsPayload {
        apm: apm_value,
        queue_depth,
        oldest_issue,
        last_pr: last_pr_summary,
    })
}

async fn fetch_metric_json(url: &str) -> Option<serde_json::Value> {
    let window = web_sys::window()?;
    let resp = JsFuture::from(window.fetch_with_str(url)).await.ok()?;
    let resp: web_sys::Response = resp.dyn_into().ok()?;
    if !resp.ok() {
        return None;
    }
    let json = JsFuture::from(resp.json().ok()?).await.ok()?;
    serde_json::from_str(&js_sys::JSON::stringify(&json).ok()?.as_string()?).ok()
}

async fn fetch_hud_settings(repo: &str) -> Option<HudSettingsData> {
    let window = web_sys::window()?;
    let repo_param = js_sys::encode_uri_component(repo);
    let url = format!("/api/hud/settings?repo={}", repo_param);
    let resp = JsFuture::from(window.fetch_with_str(&url)).await.ok()?;
    let resp: web_sys::Response = resp.dyn_into().ok()?;
    if !resp.ok() {
        return None;
    }
    let json = JsFuture::from(resp.json().ok()?).await.ok()?;
    let value: serde_json::Value = serde_json::from_str(
        &js_sys::JSON::stringify(&json)
            .ok()?
            .as_string()?,
    )
    .ok()?;

    Some(HudSettingsData {
        public: value.get("is_public").and_then(|v| v.as_bool()).unwrap_or(true),
        embed_allowed: value
            .get("embed_allowed")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        redaction_policy: "standard".to_string(),
    })
}

async fn update_hud_settings(repo: &str, settings: HudSettingsData) -> Result<(), String> {
    let window = web_sys::window().ok_or("No window available")?;
    let url = "/api/hud/settings";
    let body = serde_json::json!({
        "repo": repo,
        "is_public": settings.public,
        "embed_allowed": settings.embed_allowed,
    });

    let opts = web_sys::RequestInit::new();
    opts.set_method("POST");
    opts.set_body(&JsValue::from_str(&body.to_string()));

    let headers = web_sys::Headers::new().map_err(|_| "Failed to create headers".to_string())?;
    headers
        .set("Content-Type", "application/json")
        .map_err(|_| "Failed to set headers".to_string())?;
    opts.set_headers(&headers);

    let resp = JsFuture::from(window.fetch_with_str_and_init(&url, &opts))
        .await
        .map_err(|_| "Request failed".to_string())?;
    let resp: web_sys::Response = resp.dyn_into().map_err(|_| "Response invalid".to_string())?;
    if !resp.ok() {
        return Err(format!("Settings update failed ({})", resp.status()));
    }

    Ok(())
}

const DEFAULT_AUTOPILOT_PROMPT: &str =
    "Work the highest priority open issues and report progress in the HUD.";

#[derive(Clone, Deserialize)]
struct HudSessionResponse {
    status: String,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    ws_url: Option<String>,
    #[serde(default)]
    can_start: Option<bool>,
}

async fn fetch_hud_session(repo: &str) -> Option<HudSessionResponse> {
    let window = web_sys::window()?;
    let repo_param = js_sys::encode_uri_component(repo);
    let url = format!("/api/hud/session?repo={}", repo_param);
    let resp = JsFuture::from(window.fetch_with_str(&url)).await.ok()?;
    let resp: web_sys::Response = resp.dyn_into().ok()?;
    if !resp.ok() {
        return None;
    }
    let json = JsFuture::from(resp.json().ok()?).await.ok()?;
    serde_json::from_str(
        &js_sys::JSON::stringify(&json)
            .ok()?
            .as_string()?,
    )
    .ok()
}

async fn start_hud_session(repo: &str, prompt: &str) -> Result<HudSessionResponse, String> {
    let window = web_sys::window().ok_or("No window available")?;
    let body = serde_json::json!({
        "repo": repo,
        "prompt": prompt,
    });

    let opts = web_sys::RequestInit::new();
    opts.set_method("POST");
    opts.set_body(&JsValue::from_str(&body.to_string()));

    let headers = web_sys::Headers::new().map_err(|_| "Failed to create headers".to_string())?;
    headers
        .set("Content-Type", "application/json")
        .map_err(|_| "Failed to set headers".to_string())?;
    opts.set_headers(&headers);

    let resp = JsFuture::from(window.fetch_with_str_and_init("/api/hud/start", &opts))
        .await
        .map_err(|_| "Request failed".to_string())?;
    let resp: web_sys::Response = resp.dyn_into().map_err(|_| "Response invalid".to_string())?;
    if !resp.ok() {
        return Err(format!("HUD start failed ({})", resp.status()));
    }

    let json = JsFuture::from(resp.json().map_err(|_| "Invalid response".to_string())?)
        .await
        .map_err(|_| "Invalid response".to_string())?;
    serde_json::from_str(
        &js_sys::JSON::stringify(&json)
            .map_err(|_| "Invalid response".to_string())?
            .as_string()
            .ok_or_else(|| "Invalid response".to_string())?,
    )
    .map_err(|_| "Invalid response".to_string())
}

#[derive(Deserialize)]
struct TraceLine {
    timestamp: u64,
    data: String,
}

fn start_replay(state: Rc<RefCell<AppState>>, agent_id: String, speed: f64) {
    let state_clone = state.clone();
    wasm_bindgen_futures::spawn_local(async move {
        let events = fetch_trajectory_events(&agent_id).await;
        schedule_replay(state_clone, events, speed);
    });
}

async fn fetch_trajectory_events(agent_id: &str) -> Vec<(u64, HudEvent)> {
    let window = match web_sys::window() {
        Some(window) => window,
        None => return Vec::new(),
    };
    let url = format!("/agents/{}/logs/trajectory", agent_id);
    let resp = match JsFuture::from(window.fetch_with_str(&url)).await {
        Ok(resp) => resp,
        Err(_) => return Vec::new(),
    };
    let resp: web_sys::Response = match resp.dyn_into() {
        Ok(resp) => resp,
        Err(_) => return Vec::new(),
    };
    if !resp.ok() {
        return Vec::new();
    }
    let text_promise = match resp.text() {
        Ok(promise) => promise,
        Err(_) => return Vec::new(),
    };
    let text = JsFuture::from(text_promise)
        .await
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_default();

    let mut events = Vec::new();
    for line in text.lines() {
        if let Ok(trace) = serde_json::from_str::<TraceLine>(line) {
            if let Some(event) = parse_hud_event(&trace.data) {
                events.push((trace.timestamp, event));
            }
        }
    }
    events
}

fn schedule_replay(state: Rc<RefCell<AppState>>, events: Vec<(u64, HudEvent)>, speed: f64) {
    let window = match web_sys::window() {
        Some(window) => window,
        None => return,
    };
    if events.is_empty() {
        state.borrow_mut().hud_ui.status_text = "replay empty".to_string();
        return;
    }
    state.borrow_mut().hud_ui.status_text = format!("replay {}x", speed);
    let start = events[0].0;
    for (timestamp, event) in events {
        let delay = ((timestamp.saturating_sub(start)) as f64 / speed).round() as i32;
        let state_clone = state.clone();
        let event = event.clone();
        let cb = Closure::once(move || {
            let mut state = state_clone.borrow_mut();
            apply_hud_event(&mut state.hud_ui, event);
        });
        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
            cb.as_ref().unchecked_ref(),
            delay,
        );
        cb.forget();
    }
}

fn wallet_status_from_label(label: &str) -> WalletStatus {
    match label {
        "ready" => WalletStatus::Ready,
        "partial" => WalletStatus::Partial,
        "error" => WalletStatus::Error,
        _ => WalletStatus::Loading,
    }
}

fn js_value_u64(value: &JsValue) -> Option<u64> {
    if let Some(num) = value.as_f64() {
        if num.is_finite() && num >= 0.0 {
            return Some(num as u64);
        }
    }

    value
        .as_string()
        .and_then(|text| text.trim().parse::<u64>().ok())
}

fn js_optional_string(value: &JsValue) -> Option<String> {
    if value.is_null() || value.is_undefined() {
        None
    } else {
        value.as_string()
    }
}

fn parse_wallet_balance(value: &JsValue) -> Option<WalletBalanceData> {
    if value.is_null() || value.is_undefined() {
        return None;
    }

    let obj = js_sys::Object::from(value.clone());
    Some(WalletBalanceData {
        spark_sats: js_value_u64(&js_sys::Reflect::get(&obj, &"spark_sats".into()).ok()?)?,
        lightning_sats: js_value_u64(&js_sys::Reflect::get(&obj, &"lightning_sats".into()).ok()?)?,
        onchain_sats: js_value_u64(&js_sys::Reflect::get(&obj, &"onchain_sats".into()).ok()?)?,
        total_sats: js_value_u64(&js_sys::Reflect::get(&obj, &"total_sats".into()).ok()?)?,
    })
}

fn parse_wallet_addresses(value: &JsValue) -> WalletAddressesData {
    if value.is_null() || value.is_undefined() {
        return WalletAddressesData::default();
    }

    let obj = js_sys::Object::from(value.clone());
    WalletAddressesData {
        spark: js_optional_string(&js_sys::Reflect::get(&obj, &"spark".into()).unwrap_or(JsValue::NULL)),
        onchain: js_optional_string(&js_sys::Reflect::get(&obj, &"onchain".into()).unwrap_or(JsValue::NULL)),
    }
}

fn parse_wallet_payment(value: &JsValue) -> Option<WalletPaymentData> {
    if value.is_null() || value.is_undefined() {
        return None;
    }

    let obj = js_sys::Object::from(value.clone());
    let id = js_sys::Reflect::get(&obj, &"id".into())
        .ok()
        .and_then(|v| v.as_string())?;

    let amount_sats = js_value_u64(&js_sys::Reflect::get(&obj, &"amount_sats".into()).ok()?)
        .unwrap_or(0);
    let fee_sats = js_value_u64(&js_sys::Reflect::get(&obj, &"fee_sats".into()).ok()?)
        .unwrap_or(0);
    let direction = js_sys::Reflect::get(&obj, &"direction".into())
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_else(|| "send".to_string());
    let method = js_sys::Reflect::get(&obj, &"method".into())
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_else(|| "unknown".to_string());
    let status = js_sys::Reflect::get(&obj, &"status".into())
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_else(|| "pending".to_string());
    let timestamp = js_sys::Reflect::get(&obj, &"timestamp".into())
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_default();
    let description = js_sys::Reflect::get(&obj, &"description".into())
        .ok()
        .and_then(|v| js_optional_string(&v));

    Some(WalletPaymentData {
        id,
        amount_sats,
        fee_sats,
        direction,
        method,
        status,
        timestamp,
        description,
    })
}

fn parse_wallet_summary(value: JsValue) -> Option<WalletSummaryData> {
    let obj = js_sys::Object::from(value);
    let status = js_sys::Reflect::get(&obj, &"status".into())
        .ok()
        .and_then(|v| v.as_string())
        .map(|status| wallet_status_from_label(&status))
        .unwrap_or(WalletStatus::Loading);
    let network = js_sys::Reflect::get(&obj, &"network".into())
        .ok()
        .and_then(|v| v.as_string());
    let balance = js_sys::Reflect::get(&obj, &"balance".into())
        .ok()
        .and_then(|v| parse_wallet_balance(&v));
    let addresses = js_sys::Reflect::get(&obj, &"addresses".into())
        .ok()
        .map(|v| parse_wallet_addresses(&v))
        .unwrap_or_default();
    let payments = js_sys::Reflect::get(&obj, &"payments".into())
        .ok()
        .map(|v| {
            js_sys::Array::try_from(v)
                .ok()
                .map(|arr| {
                    let mut items = Vec::new();
                    for idx in 0..arr.length() {
                        if let Some(payment) = parse_wallet_payment(&arr.get(idx)) {
                            items.push(payment);
                        }
                    }
                    items
                })
                .unwrap_or_default()
        })
        .unwrap_or_default();
    let error = js_sys::Reflect::get(&obj, &"error".into())
        .ok()
        .and_then(|v| js_optional_string(&v));

    Some(WalletSummaryData {
        status,
        network,
        balance,
        addresses,
        payments,
        error,
    })
}

fn parse_wallet_send(value: JsValue) -> Option<WalletSendData> {
    let obj = js_sys::Object::from(value);
    Some(WalletSendData {
        payment_id: js_sys::Reflect::get(&obj, &"payment_id".into())
            .ok()
            .and_then(|v| v.as_string())?,
        status: js_sys::Reflect::get(&obj, &"status".into())
            .ok()
            .and_then(|v| v.as_string())?,
        method: js_sys::Reflect::get(&obj, &"method".into())
            .ok()
            .and_then(|v| v.as_string())?,
        amount_sats: js_value_u64(&js_sys::Reflect::get(&obj, &"amount_sats".into()).ok()?)?,
        fee_sats: js_value_u64(&js_sys::Reflect::get(&obj, &"fee_sats".into()).ok()?)?,
    })
}

fn parse_wallet_invoice(value: JsValue) -> Option<WalletInvoiceData> {
    let obj = js_sys::Object::from(value);
    Some(WalletInvoiceData {
        method: js_sys::Reflect::get(&obj, &"method".into())
            .ok()
            .and_then(|v| v.as_string())?,
        payment_request: js_sys::Reflect::get(&obj, &"payment_request".into())
            .ok()
            .and_then(|v| v.as_string())?,
        amount_sats: js_sys::Reflect::get(&obj, &"amount_sats".into())
            .ok()
            .and_then(|v| js_value_u64(&v)),
        description: js_sys::Reflect::get(&obj, &"description".into())
            .ok()
            .and_then(|v| js_optional_string(&v)),
    })
}

async fn fetch_wallet_summary() -> Result<WalletSummaryData, String> {
    let window = web_sys::window().ok_or("No window available")?;
    let resp = JsFuture::from(window.fetch_with_str("/api/wallet/summary"))
        .await
        .map_err(|_| "Failed to fetch wallet summary".to_string())?;
    let resp: web_sys::Response = resp
        .dyn_into()
        .map_err(|_| "Wallet summary response invalid".to_string())?;

    if !resp.ok() {
        return Err(format!("Wallet summary failed ({})", resp.status()));
    }

    let json = JsFuture::from(resp.json().map_err(|_| "Wallet summary JSON invalid".to_string())?)
        .await
        .map_err(|_| "Wallet summary JSON invalid".to_string())?;

    parse_wallet_summary(json).ok_or_else(|| "Wallet summary malformed".to_string())
}

async fn post_json(url: &str, payload: js_sys::Object) -> Result<JsValue, String> {
    let window = web_sys::window().ok_or("No window available")?;
    let body = js_sys::JSON::stringify(&payload)
        .map_err(|_| "Failed to serialize request body".to_string())?;

    let opts = web_sys::RequestInit::new();
    opts.set_method("POST");
    opts.set_body(body.as_ref());

    let headers = web_sys::Headers::new().map_err(|_| "Failed to create headers".to_string())?;
    headers
        .set("Content-Type", "application/json")
        .map_err(|_| "Failed to set headers".to_string())?;
    opts.set_headers(headers.as_ref());

    let resp = JsFuture::from(window.fetch_with_str_and_init(url, &opts))
        .await
        .map_err(|_| "Request failed".to_string())?;
    let resp: web_sys::Response = resp.dyn_into().map_err(|_| "Response invalid".to_string())?;

    if !resp.ok() {
        return Err(format!("Request failed ({})", resp.status()));
    }

    JsFuture::from(resp.json().map_err(|_| "Response JSON invalid".to_string())?)
        .await
        .map_err(|_| "Response JSON invalid".to_string())
}

async fn send_wallet_payment(
    payment_request: &str,
    amount_sats: Option<u64>,
) -> Result<WalletSendData, String> {
    let payload = js_sys::Object::new();
    js_sys::Reflect::set(
        &payload,
        &"payment_request".into(),
        &JsValue::from_str(payment_request),
    )
    .map_err(|_| "Failed to build payment payload".to_string())?;

    if let Some(amount) = amount_sats {
        js_sys::Reflect::set(
            &payload,
            &"amount_sats".into(),
            &JsValue::from_f64(amount as f64),
        )
        .map_err(|_| "Failed to build payment payload".to_string())?;
    }

    let json = post_json("/api/wallet/send", payload).await?;
    parse_wallet_send(json).ok_or_else(|| "Payment response malformed".to_string())
}

async fn request_wallet_receive(
    method: &str,
    amount_sats: Option<u64>,
) -> Result<WalletInvoiceData, String> {
    let payload = js_sys::Object::new();
    js_sys::Reflect::set(&payload, &"method".into(), &JsValue::from_str(method))
        .map_err(|_| "Failed to build receive payload".to_string())?;

    if let Some(amount) = amount_sats {
        js_sys::Reflect::set(
            &payload,
            &"amount_sats".into(),
            &JsValue::from_f64(amount as f64),
        )
        .map_err(|_| "Failed to build receive payload".to_string())?;
    }

    let json = post_json("/api/wallet/receive", payload).await?;
    parse_wallet_invoice(json).ok_or_else(|| "Receive response malformed".to_string())
}

fn modifiers_from_event(event: &web_sys::KeyboardEvent) -> Modifiers {
    Modifiers {
        shift: event.shift_key(),
        ctrl: event.ctrl_key(),
        alt: event.alt_key(),
        meta: event.meta_key(),
    }
}

fn key_from_event(event: &web_sys::KeyboardEvent) -> Option<Key> {
    let key = event.key();
    match key.as_str() {
        "Enter" => Some(Key::Named(NamedKey::Enter)),
        "Escape" => Some(Key::Named(NamedKey::Escape)),
        "Backspace" => Some(Key::Named(NamedKey::Backspace)),
        "Delete" => Some(Key::Named(NamedKey::Delete)),
        "Tab" => Some(Key::Named(NamedKey::Tab)),
        "Home" => Some(Key::Named(NamedKey::Home)),
        "End" => Some(Key::Named(NamedKey::End)),
        "PageUp" => Some(Key::Named(NamedKey::PageUp)),
        "PageDown" => Some(Key::Named(NamedKey::PageDown)),
        "ArrowUp" => Some(Key::Named(NamedKey::ArrowUp)),
        "ArrowDown" => Some(Key::Named(NamedKey::ArrowDown)),
        "ArrowLeft" => Some(Key::Named(NamedKey::ArrowLeft)),
        "ArrowRight" => Some(Key::Named(NamedKey::ArrowRight)),
        _ => {
            if key.chars().count() == 1 {
                Some(Key::Character(key))
            } else {
                None
            }
        }
    }
}

fn mouse_button_from_event(event: &web_sys::MouseEvent) -> MouseButton {
    match event.button() {
        1 => MouseButton::Middle,
        2 => MouseButton::Right,
        _ => MouseButton::Left,
    }
}

fn parse_amount_input(value: &str) -> Result<Option<u64>, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    trimmed
        .parse::<u64>()
        .map(Some)
        .map_err(|_| "Amount must be a whole number of sats".to_string())
}

fn dispatch_wallet_event(state: &Rc<RefCell<AppState>>, event: InputEvent) -> EventResult {
    let (result, actions) = {
        let mut state = state.borrow_mut();
        if state.view != AppView::RepoView {
            return EventResult::Ignored;
        }

        let result = state.wallet.handle_event(&event);
        let actions = state.wallet.take_actions();
        (result, actions)
    };

    if !actions.is_empty() {
        queue_wallet_actions(state.clone(), actions);
    }

    result
}

fn queue_wallet_actions(state: Rc<RefCell<AppState>>, actions: Vec<WalletAction>) {
    if actions.is_empty() {
        return;
    }

    for action in actions {
        let state_clone = state.clone();
        wasm_bindgen_futures::spawn_local(async move {
            handle_wallet_action(state_clone, action).await;
        });
    }
}

async fn refresh_wallet_summary(state: Rc<RefCell<AppState>>) {
    {
        let mut state = state.borrow_mut();
        state.wallet.status = WalletStatus::Loading;
        state.wallet.error = None;
    }

    match fetch_wallet_summary().await {
        Ok(summary) => {
            let mut state = state.borrow_mut();
            state.wallet.apply_summary(summary);
        }
        Err(err) => {
            let mut state = state.borrow_mut();
            state.wallet.status = WalletStatus::Error;
            state.wallet.error = Some(err);
        }
    }
}

async fn handle_receive_action(
    state: Rc<RefCell<AppState>>,
    method: &str,
    requires_amount: bool,
) {
    let amount_input = {
        let state = state.borrow();
        state.wallet.receive_amount_input.get_value().to_string()
    };

    let amount_sats = match parse_amount_input(&amount_input) {
        Ok(amount) => amount,
        Err(err) => {
            let mut state = state.borrow_mut();
            state.wallet.receive_notice = Some(format!("Error: {}", err));
            return;
        }
    };

    if requires_amount && amount_sats.is_none() {
        let mut state = state.borrow_mut();
        state.wallet.receive_notice = Some("Error: amount is required".to_string());
        return;
    }

    {
        let mut state = state.borrow_mut();
        state.wallet.receive_notice = Some("Requesting invoice...".to_string());
        state.wallet.last_invoice = None;
    }

    match request_wallet_receive(method, amount_sats).await {
        Ok(invoice) => {
            let mut state = state.borrow_mut();
            state.wallet.set_invoice(invoice);
            state.wallet.receive_amount_input.set_value("");
        }
        Err(err) => {
            let mut state = state.borrow_mut();
            state.wallet.receive_notice = Some(format!("Error: {}", err));
            return;
        }
    }

    refresh_wallet_summary(state).await;
}

async fn handle_wallet_action(state: Rc<RefCell<AppState>>, action: WalletAction) {
    match action {
        WalletAction::Refresh => {
            refresh_wallet_summary(state).await;
        }
        WalletAction::SendPayment => {
            let (payment_request, amount_input) = {
                let state = state.borrow();
                (
                    state.wallet.send_address_input.get_value().trim().to_string(),
                    state.wallet.send_amount_input.get_value().to_string(),
                )
            };

            if payment_request.is_empty() {
                let mut state = state.borrow_mut();
                state.wallet.send_notice = Some("Error: payment request required".to_string());
                return;
            }

            let amount_sats = match parse_amount_input(&amount_input) {
                Ok(amount) => amount,
                Err(err) => {
                    let mut state = state.borrow_mut();
                    state.wallet.send_notice = Some(format!("Error: {}", err));
                    return;
                }
            };

            {
                let mut state = state.borrow_mut();
                state.wallet.send_notice = Some("Sending payment...".to_string());
            }

            match send_wallet_payment(&payment_request, amount_sats).await {
                Ok(sent) => {
                    let notice = if sent.status == "failed" {
                        "Error: payment failed".to_string()
                    } else {
                        let status_label = match sent.status.as_str() {
                            "completed" => "Sent",
                            "pending" => "Pending",
                            _ => "Status",
                        };
                        format!(
                            "{} {} sats (fee {}, {})",
                            status_label, sent.amount_sats, sent.fee_sats, sent.method
                        )
                    };

                    {
                        let mut state = state.borrow_mut();
                        state.wallet.send_notice = Some(notice);
                        if sent.status != "failed" {
                            state.wallet.send_address_input.set_value("");
                            state.wallet.send_amount_input.set_value("");
                        }
                    }

                    if sent.status != "failed" {
                        refresh_wallet_summary(state).await;
                    }
                }
                Err(err) => {
                    let mut state = state.borrow_mut();
                    state.wallet.send_notice = Some(format!("Error: {}", err));
                }
            }
        }
        WalletAction::ReceiveSpark => {
            handle_receive_action(state, "spark", false).await;
        }
        WalletAction::ReceiveLightning => {
            handle_receive_action(state, "lightning", true).await;
        }
        WalletAction::ReceiveOnchain => {
            handle_receive_action(state, "onchain", false).await;
        }
    }
}

#[wasm_bindgen]
pub async fn start_demo(canvas_id: &str) -> Result<(), JsValue> {
    let platform = WebPlatform::init(canvas_id)
        .await
        .map_err(|e| JsValue::from_str(&e))?;

    let platform = Rc::new(RefCell::new(platform));
    let state = Rc::new(RefCell::new(AppState::default()));

    // Force initial resize
    platform.borrow_mut().handle_resize();

    if let Some(context) = get_hud_context() {
        let mut state_guard = state.borrow_mut();
        state_guard.loading = false;
        state_guard.view = AppView::RepoView;
        state_guard.hud_ui.status_text = context.status.clone();
        state_guard.hud_context = Some(context);
        drop(state_guard);
        init_hud_runtime(state.clone());
    } else {
        // Fetch live HUD and current user. Landing shows live fishbowl if logged out.
        let state_clone = state.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let live_hud = fetch_live_hud().await;
            let user_info = fetch_current_user().await;

            {
                let mut state = state_clone.borrow_mut();
                state.loading = false;

                // If logged in, show repo selector first
                if let Some(info) = user_info.clone() {
                    state.user = info;
                    state.view = AppView::RepoSelector;
                    state.repos_loading = true;
                } else if let Some(live) = live_hud.clone() {
                    state.hud_ui.status_text = live.hud_context.status.clone();
                    state.hud_context = Some(live.hud_context.clone());
                    state.landing_live = Some(live);
                    state.view = AppView::Landing;
                }
            }

            if user_info.is_some() {
                queue_wallet_actions(state_clone.clone(), vec![WalletAction::Refresh]);

                let repos = fetch_repos().await;
                let mut state = state_clone.borrow_mut();
                state.repos = repos;
                state.repos_loading = false;
            } else if state_clone.borrow().hud_context.is_some() {
                init_hud_runtime(state_clone.clone());
            }
        });
    }

    // Set up resize observer
    {
        let platform_clone = platform.clone();
        let canvas = platform.borrow().canvas().clone();
        setup_resize_observer(&canvas, move || {
            platform_clone.borrow_mut().handle_resize();
        });
    }

    // Mouse move events
    {
        let state_clone = state.clone();
        let canvas = platform.borrow().canvas().clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::MouseEvent| {
            let x = event.offset_x() as f32;
            let y = event.offset_y() as f32;

            {
                let mut state = state_clone.borrow_mut();
                state.mouse_pos = Point::new(x, y);
                state.button_hovered = state.button_bounds.contains(state.mouse_pos);

                // Check repo hover
                state.hovered_repo_idx = None;
                for (i, bounds) in state.repo_bounds.iter().enumerate() {
                    if bounds.contains(state.mouse_pos) {
                        state.hovered_repo_idx = Some(i);
                        break;
                    }
                }
            }

            dispatch_wallet_event(
                &state_clone,
                InputEvent::MouseMove { x, y },
            );
        });
        canvas.add_event_listener_with_callback("mousemove", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    // Click events
    {
        let state_clone = state.clone();
        let canvas = platform.borrow().canvas().clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::MouseEvent| {
            let mut state = state_clone.borrow_mut();
            let click_pos = Point::new(event.offset_x() as f32, event.offset_y() as f32);

            // Landing issue banner click
            if state.view == AppView::Landing
                && state.landing_issue_bounds.contains(click_pos)
                && state.landing_issue_url.is_some()
            {
                if let Some(window) = web_sys::window() {
                    if let Some(url) = state.landing_issue_url.clone() {
                        let _ = window.open_with_url_and_target(&url, "_blank");
                    }
                }
                return;
            }

            // Check repo click - select repo and switch to app shell (no navigation)
            if let Some(idx) = state.hovered_repo_idx {
                if idx < state.repos.len() {
                    let repo = &state.repos[idx];
                    let full_name = repo.full_name.clone();

                    // Parse owner/repo from full_name
                    let parts: Vec<&str> = full_name.split('/').collect();
                    let (owner, repo_name) = if parts.len() == 2 {
                        (parts[0].to_string(), parts[1].to_string())
                    } else {
                        (full_name.clone(), "".to_string())
                    };

                    let share_owner = owner.clone();
                    let share_repo = repo_name.clone();
                    state.selected_repo = Some(full_name);
                    state.hud_context = Some(HudContext {
                        username: owner,
                        repo: repo_name,
                        is_owner: true,
                        is_public: true,
                        embed_mode: false,
                        agent_id: None,
                        stream_url: None,
                        session_id: None,
                        ws_url: None,
                        status: "starting".to_string(),
                    });
                    state.hud_ui.status_text = "starting".to_string();
                    state.view = AppView::RepoView;
                    state.hud_settings_loaded = false;
                    state.landing_live = None;
                    state.landing_issue_bounds = Bounds::ZERO;
                    state.landing_issue_url = None;
                    if let Some(handle) = state.hud_stream.take() {
                        handle.close();
                    }
                    stop_metrics_poll(&mut state);
                    drop(state);
                    init_hud_runtime(state_clone.clone());
                    let repo_full = repo.full_name.clone();
                    let state_for_session = state_clone.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        ensure_hud_session(state_for_session, repo_full).await;
                    });
                    if let Some(window) = web_sys::window() {
                        if let Ok(history) = window.history() {
                            let path = format!("/hud/@{}/{}", share_owner, share_repo);
                            let _ = history.replace_state_with_url(&JsValue::NULL, "", Some(&path));
                        }
                    }
                    return;
                }
            }

            // HUD settings toggles
            if state.view == AppView::RepoView {
                let can_edit = state
                    .hud_context
                    .as_ref()
                    .map(|ctx| ctx.is_owner)
                    .unwrap_or(false);
                if can_edit {
                    let mut changed = false;
                    if state.hud_layout.settings_public_bounds.contains(click_pos) {
                        state.hud_ui.settings.public = !state.hud_ui.settings.public;
                        changed = true;
                    }
                    if state.hud_layout.settings_embed_bounds.contains(click_pos) {
                        state.hud_ui.settings.embed_allowed = !state.hud_ui.settings.embed_allowed;
                        changed = true;
                    }
                    if changed {
                        if let Some(repo) = state
                            .hud_context
                            .as_ref()
                            .map(|ctx| format!("{}/{}", ctx.username, ctx.repo))
                        {
                            let settings = state.hud_ui.settings.clone();
                            drop(state);
                            wasm_bindgen_futures::spawn_local(async move {
                                let _ = update_hud_settings(&repo, settings).await;
                            });
                            return;
                        }
                    }
                }
            }

            // Check button click
            if state.button_bounds.contains(click_pos) {
                if let Some(window) = web_sys::window() {
                    match state.view {
                        AppView::RepoView | AppView::RepoSelector => {
                            // Logout button
                            let opts = web_sys::RequestInit::new();
                            opts.set_method("POST");
                            let _ = window.fetch_with_str_and_init("/api/auth/logout", &opts);
                            let _ = window.location().reload();
                        }
                        AppView::Landing => {
                            // Login button
                            let _ = window.location().set_href("/api/auth/github/start");
                        }
                    }
                }
            }
        });
        canvas.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    // Mouse down events
    {
        let state_clone = state.clone();
        let canvas = platform.borrow().canvas().clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::MouseEvent| {
            let x = event.offset_x() as f32;
            let y = event.offset_y() as f32;
            let button = mouse_button_from_event(&event);
            let input_event = InputEvent::MouseDown { button, x, y };
            dispatch_wallet_event(&state_clone, input_event);
        });
        canvas.add_event_listener_with_callback("mousedown", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    // Mouse up events for wallet interactions
    {
        let state_clone = state.clone();
        let canvas = platform.borrow().canvas().clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::MouseEvent| {
            let x = event.offset_x() as f32;
            let y = event.offset_y() as f32;
            let button = mouse_button_from_event(&event);
            dispatch_wallet_event(&state_clone, InputEvent::MouseUp { button, x, y });
        });
        canvas.add_event_listener_with_callback("mouseup", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    // Scroll events
    {
        let state_clone = state.clone();
        let canvas = platform.borrow().canvas().clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::WheelEvent| {
            let mut state = state_clone.borrow_mut();
            if state.view == AppView::RepoSelector {
                state.scroll_offset += event.delta_y() as f32 * 0.5;
                state.scroll_offset = state.scroll_offset.max(0.0);
                return;
            }

            if state.view == AppView::RepoView {
                let point = Point::new(event.offset_x() as f32, event.offset_y() as f32);
                let scroll = InputEvent::Scroll {
                    dx: 0.0,
                    dy: event.delta_y() as f32,
                };
                let mut event_ctx = EventContext::new();
                if state.hud_layout.thread_bounds.contains(point) {
                    state
                        .hud_ui
                        .thread
                        .event(&scroll, state.hud_layout.thread_bounds, &mut event_ctx);
                } else if state.hud_layout.code_bounds.contains(point) {
                    state
                        .hud_ui
                        .code
                        .event(&scroll, state.hud_layout.code_bounds, &mut event_ctx);
                } else if state.hud_layout.terminal_bounds.contains(point) {
                    state
                        .hud_ui
                        .terminal
                        .event(&scroll, state.hud_layout.terminal_bounds, &mut event_ctx);
                } else if state.hud_layout.metrics_bounds.contains(point) {
                    state
                        .hud_ui
                        .metrics
                        .event(&scroll, state.hud_layout.metrics_bounds, &mut event_ctx);
                }
            }
        });
        canvas.add_event_listener_with_callback("wheel", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    // Cursor style
    {
        let state_clone = state.clone();
        let canvas = platform.borrow().canvas().clone();
        let canvas2 = canvas.clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::MouseEvent| {
            let state = state_clone.borrow();
            let hud_hover = state.view == AppView::RepoView
                && (state.hud_layout.settings_public_bounds.contains(state.mouse_pos)
                    || state.hud_layout.settings_embed_bounds.contains(state.mouse_pos));
            let landing_hover =
                state.view == AppView::Landing && state.landing_issue_bounds.contains(state.mouse_pos);
            let cursor = if state.button_hovered
                || state.hovered_repo_idx.is_some()
                || hud_hover
                || landing_hover
            {
                "pointer"
            } else {
                "default"
            };
            let _ = canvas2.style().set_property("cursor", cursor);
        });
        canvas.add_event_listener_with_callback("mousemove", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    // Keyboard events
    {
        let state_clone = state.clone();
        let window = web_sys::window().unwrap();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::KeyboardEvent| {
            let mut handled = EventResult::Ignored;

            if let Some(key) = key_from_event(&event) {
                let modifiers = modifiers_from_event(&event);
                let input_event = InputEvent::KeyDown { key, modifiers };
                handled = dispatch_wallet_event(&state_clone, input_event);
            }

            if matches!(handled, EventResult::Handled) {
                event.prevent_default();
            }
        });
        window.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    // Animation loop
    run_animation_loop(move || {
        let mut platform = platform.borrow_mut();
        let mut state = state.borrow_mut();

        let size = platform.logical_size();
        let width = size.width;
        let height = size.height;

        let mut scene = Scene::new();

        let scale_factor = platform.scale_factor();

        match state.view {
            AppView::Landing => {
                build_landing_page(
                    &mut scene,
                    platform.text_system(),
                    &mut state,
                    width,
                    height,
                    scale_factor,
                );
            }
            AppView::RepoSelector => {
                build_repo_selector(&mut scene, platform.text_system(), &mut state, width, height);
            }
            AppView::RepoView => {
                build_repo_view(
                    &mut scene,
                    platform.text_system(),
                    &mut state,
                    width,
                    height,
                    scale_factor,
                );
            }
        }

        if let Err(e) = platform.render_scene(&scene) {
            web_sys::console::error_1(&format!("Render error: {}", e).into());
        }
    });

    Ok(())
}

fn build_landing_page(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    width: f32,
    height: f32,
    scale_factor: f32,
) {
    let has_live = state.landing_live.is_some() && state.hud_context.is_some();
    if has_live {
        draw_hud_view(scene, text_system, state, width, height, scale_factor);
        scene.draw_quad(
            Quad::new(Bounds::new(0.0, 0.0, width, height))
                .with_background(theme::bg::APP.with_alpha(0.12)),
        );
    } else {
        scene.draw_quad(
            Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(theme::bg::APP),
        );
    }

    state.landing_issue_bounds = Bounds::ZERO;
    state.landing_issue_url = None;

    let pad = 12.0;
    if let Some(live) = state.landing_live.as_ref() {
        let banner_h = 22.0;
        let live_label = "LIVE";
        let label_size = 10.0;
        let label_w = live_label.len() as f32 * label_size * 0.6 + 10.0;
        let label_bounds = Bounds::new(pad, pad, label_w, banner_h);

        scene.draw_quad(
            Quad::new(label_bounds)
                .with_background(theme::status::ERROR)
                .with_border(theme::border::DEFAULT, 1.0)
                .with_corner_radius(0.0),
        );

        let label_run = text_system.layout(
            live_label,
            Point::new(label_bounds.origin.x + 5.0, label_bounds.origin.y + 5.0),
            label_size,
            theme::bg::APP,
        );
        scene.draw_text(label_run);

        let issue_text = live
            .issue
            .as_ref()
            .map(|issue| format!("Autopilot is working on {}", issue.label))
            .unwrap_or_else(|| "Autopilot is working live".to_string());
        let issue_size = 12.0;
        let issue_x = label_bounds.origin.x + label_bounds.size.width + 8.0;
        let issue_y = label_bounds.origin.y + 4.0;
        let issue_run = text_system.layout(
            &issue_text,
            Point::new(issue_x, issue_y),
            issue_size,
            theme::text::PRIMARY,
        );
        scene.draw_text(issue_run);

        if let Some(issue) = live.issue.as_ref() {
            let issue_w = issue_text.len() as f32 * issue_size * 0.6;
            state.landing_issue_bounds = Bounds::new(issue_x, issue_y, issue_w, banner_h);
            state.landing_issue_url = Some(issue.url.clone());
        }

        let repo_label = format!("@{}/{}", live.hud_context.username, live.hud_context.repo);
        let repo_run = text_system.layout(
            &repo_label,
            Point::new(pad, label_bounds.origin.y + label_bounds.size.height + 6.0),
            11.0,
            theme::text::MUTED,
        );
        scene.draw_text(repo_run);

        if let Some(issue) = live.issue.as_ref().and_then(|issue| issue.title.as_ref()) {
            let title_run = text_system.layout(
                issue,
                Point::new(pad, label_bounds.origin.y + label_bounds.size.height + 20.0),
                11.0,
                theme::text::MUTED,
            );
            scene.draw_text(title_run);
        }
    } else {
        let placeholder = text_system.layout(
            "No live session is broadcasting right now.",
            Point::new(pad, pad),
            12.0,
            theme::text::MUTED,
        );
        scene.draw_text(placeholder);
    }

    let panel_h = if height < 560.0 { 108.0 } else { 128.0 };
    let panel_w = (width - pad * 2.0).max(240.0);
    let panel_x = pad;
    let panel_y = height - panel_h - pad;
    let panel_bounds = Bounds::new(panel_x, panel_y, panel_w, panel_h);

    scene.draw_quad(
        Quad::new(panel_bounds)
            .with_background(theme::bg::SURFACE.with_alpha(0.94))
            .with_border(theme::border::DEFAULT, 1.0)
            .with_corner_radius(0.0),
    );

    let title = "Autopilot for code";
    let title_run = text_system.layout(
        title,
        Point::new(panel_x + 12.0, panel_y + 10.0),
        if width < 600.0 { 18.0 } else { 22.0 },
        theme::text::PRIMARY,
    );
    scene.draw_text(title_run);

    let subtitle = "Watch it work. Connect GitHub to get your own HUD in under 30 seconds.";
    let subtitle_run = text_system.layout(
        subtitle,
        Point::new(panel_x + 12.0, panel_y + 40.0),
        12.0,
        theme::text::MUTED,
    );
    scene.draw_text(subtitle_run);

    let (button_text, button_bg_base): (&str, _) = if state.loading {
        ("Connecting...", theme::text::MUTED)
    } else {
        ("Connect GitHub → Get Your Own Autopilot", theme::accent::PRIMARY)
    };

    let button_font_size = 13.0;
    let button_text_width = button_text.len() as f32 * button_font_size * 0.6;
    let button_padding_x = 18.0;
    let button_padding_y = 10.0;
    let button_width = button_text_width + button_padding_x * 2.0;
    let button_height = button_font_size + button_padding_y * 2.0;
    let button_x = panel_x + 12.0;
    let button_y = panel_y + panel_h - button_height - 12.0;

    if !state.loading {
        state.button_bounds = Bounds::new(button_x, button_y, button_width, button_height);
    } else {
        state.button_bounds = Bounds::ZERO;
    }

    let button_bg = if state.button_hovered && !state.loading {
        button_bg_base
    } else {
        button_bg_base.with_alpha(0.85)
    };

    scene.draw_quad(
        Quad::new(Bounds::new(button_x, button_y, button_width, button_height))
            .with_background(button_bg)
            .with_border(theme::border::DEFAULT, 1.0)
            .with_corner_radius(0.0),
    );

    let button_text_run = text_system.layout(
        button_text,
        Point::new(button_x + button_padding_x, button_y + button_padding_y),
        button_font_size,
        theme::bg::APP,
    );
    scene.draw_text(button_text_run);
}

fn build_repo_selector(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    width: f32,
    height: f32,
) {
    // Background
    scene.draw_quad(Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(theme::bg::APP));

    let padding = 24.0;
    let mut y = padding;

    // Header
    let header = format!(
        "Welcome, {}",
        state.user.github_username.as_deref().unwrap_or("User")
    );
    let header_run = text_system.layout(
        &header,
        Point::new(padding, y),
        24.0,
        theme::text::PRIMARY,
    );
    scene.draw_text(header_run);

    // Logout button (small, top right)
    let logout_text = "Logout";
    let logout_size = 12.0;
    let logout_width = logout_text.len() as f32 * logout_size * 0.6 + 16.0;
    let logout_x = width - padding - logout_width;
    state.button_bounds = Bounds::new(logout_x, y - 4.0, logout_width, 24.0);

    let logout_bg = if state.button_hovered {
        theme::status::ERROR
    } else {
        theme::status::ERROR.with_alpha(0.7)
    };

    scene.draw_quad(
        Quad::new(state.button_bounds)
            .with_background(logout_bg)
            .with_corner_radius(4.0),
    );

    let logout_run = text_system.layout(
        logout_text,
        Point::new(logout_x + 8.0, y),
        logout_size,
        theme::text::PRIMARY,
    );
    scene.draw_text(logout_run);

    y += 28.0;

    if let Some(npub) = state.user.nostr_npub.as_deref() {
        let npub_text = format!("npub: {}", npub);
        let npub_run = text_system.layout(
            &npub_text,
            Point::new(padding, y),
            11.0,
            theme::text::MUTED,
        );
        scene.draw_text(npub_run);
        y += 18.0;
    }

    y += 16.0;

    // Subtitle
    let subtitle = "Select a repository:";
    let subtitle_run = text_system.layout(
        subtitle,
        Point::new(padding, y),
        14.0,
        theme::text::MUTED,
    );
    scene.draw_text(subtitle_run);

    y += 32.0;

    // Repo list
    state.repo_bounds.clear();

    if state.repos_loading {
        let loading_run = text_system.layout(
            "Loading repositories...",
            Point::new(padding, y),
            14.0,
            theme::text::MUTED,
        );
        scene.draw_text(loading_run);
    } else if state.repos.is_empty() {
        let empty_run = text_system.layout(
            "No repositories found",
            Point::new(padding, y),
            14.0,
            theme::text::MUTED,
        );
        scene.draw_text(empty_run);
    } else {
        let row_height = 56.0;
        for (i, repo) in state.repos.iter().enumerate() {
            let row_y = y + (i as f32 * row_height) - state.scroll_offset;

            // Skip if outside visible area
            if row_y + row_height < y || row_y > height {
                state.repo_bounds.push(Bounds::ZERO);
                continue;
            }

            let row_bounds = Bounds::new(padding, row_y, width - padding * 2.0, row_height - 4.0);
            state.repo_bounds.push(row_bounds);

            // Row background
            let is_hovered = state.hovered_repo_idx == Some(i);
            let is_selected = state.selected_repo.as_ref() == Some(&repo.full_name);

            let row_bg = if is_selected {
                theme::accent::PRIMARY.with_alpha(0.2)
            } else if is_hovered {
                theme::bg::HOVER
            } else {
                theme::bg::SURFACE
            };

            scene.draw_quad(
                Quad::new(row_bounds)
                    .with_background(row_bg)
                    .with_border(theme::border::DEFAULT, 1.0),
            );

            // Repo name
            let name_run = text_system.layout(
                &repo.full_name,
                Point::new(padding + 12.0, row_y + 10.0),
                14.0,
                theme::text::PRIMARY,
            );
            scene.draw_text(name_run);

            // Private badge
            if repo.private {
                let badge_text = "Private";
                let badge_x = padding + 12.0 + repo.full_name.len() as f32 * 14.0 * 0.6 + 8.0;
                let badge_bounds = Bounds::new(badge_x, row_y + 10.0, 50.0, 16.0);
                scene.draw_quad(
                    Quad::new(badge_bounds)
                        .with_background(theme::status::WARNING.with_alpha(0.2))
                        .with_border(theme::status::WARNING, 1.0),
                );
                let badge_run = text_system.layout(
                    badge_text,
                    Point::new(badge_x + 6.0, row_y + 11.0),
                    10.0,
                    theme::status::WARNING,
                );
                scene.draw_text(badge_run);
            }

            // Description
            if let Some(desc) = &repo.description {
                let desc_truncated = if desc.len() > 80 {
                    format!("{}...", &desc[..77])
                } else {
                    desc.clone()
                };
                let desc_run = text_system.layout(
                    &desc_truncated,
                    Point::new(padding + 12.0, row_y + 32.0),
                    11.0,
                    theme::text::MUTED,
                );
                scene.draw_text(desc_run);
            }
        }

        // Scroll indicator
        let total_height = state.repos.len() as f32 * row_height;
        let visible_height = height - y;
        if total_height > visible_height {
            let scroll_track_height = visible_height - 20.0;
            let scroll_thumb_height = (visible_height / total_height) * scroll_track_height;
            let scroll_thumb_y = y + 10.0 + (state.scroll_offset / total_height) * scroll_track_height;

            // Track
            scene.draw_quad(
                Quad::new(Bounds::new(width - 8.0, y, 4.0, scroll_track_height))
                    .with_background(theme::bg::SURFACE),
            );

            // Thumb
            scene.draw_quad(
                Quad::new(Bounds::new(width - 8.0, scroll_thumb_y, 4.0, scroll_thumb_height))
                    .with_background(theme::text::MUTED),
            );

            // Clamp scroll
            let max_scroll = total_height - visible_height;
            state.scroll_offset = state.scroll_offset.min(max_scroll).max(0.0);
        }
    }
}

fn build_repo_view(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    width: f32,
    height: f32,
    scale_factor: f32,
) {
    draw_hud_view(scene, text_system, state, width, height, scale_factor);
}

fn draw_hud_view(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    width: f32,
    height: f32,
    scale_factor: f32,
) {
    scene.draw_quad(
        Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(theme::bg::APP),
    );

    let status_h = 28.0;
    let padding = 6.0;
    let gutter = 6.0;
    let content_x = padding;
    let content_y = padding;
    let content_w = width - padding * 2.0;
    let content_h = height - status_h - padding * 2.0;

    let mut layout = HudLayout::default();

    let mut thread_bounds;
    let mut code_bounds;
    let mut terminal_bounds;
    let mut metrics_bounds;

    if width < 900.0 {
        let pane_h = ((content_h - gutter * 3.0) / 4.0).max(120.0);
        thread_bounds = Bounds::new(content_x, content_y, content_w, pane_h);
        code_bounds = Bounds::new(content_x, content_y + pane_h + gutter, content_w, pane_h);
        terminal_bounds =
            Bounds::new(content_x, content_y + (pane_h + gutter) * 2.0, content_w, pane_h);
        metrics_bounds =
            Bounds::new(content_x, content_y + (pane_h + gutter) * 3.0, content_w, pane_h);
    } else {
        let left_w = (content_w * 0.34).max(280.0).min(420.0);
        let right_w = (content_w * 0.28).max(240.0).min(360.0);
        let center_w = (content_w - left_w - right_w - gutter * 2.0).max(220.0);
        let left_x = content_x;
        let center_x = left_x + left_w + gutter;
        let right_x = center_x + center_w + gutter;
        thread_bounds = Bounds::new(left_x, content_y, left_w, content_h);
        code_bounds = Bounds::new(center_x, content_y, center_w, content_h);
        let terminal_h = (content_h * 0.6).max(180.0);
        terminal_bounds = Bounds::new(right_x, content_y, right_w, terminal_h);
        metrics_bounds = Bounds::new(
            right_x,
            content_y + terminal_h + gutter,
            right_w,
            content_h - terminal_h - gutter,
        );
    }

    layout.settings_public_bounds = Bounds::ZERO;
    layout.settings_embed_bounds = Bounds::ZERO;
    if let Some(ctx) = state.hud_context.as_ref() {
        if ctx.is_owner {
            let settings_height = 58.0;
            let settings_bounds = Bounds::new(
                thread_bounds.origin.x,
                thread_bounds.origin.y,
                thread_bounds.size.width,
                settings_height,
            );
            let (public_bounds, embed_bounds) = draw_hud_settings(
                scene,
                text_system,
                &state.hud_ui.settings,
                settings_bounds,
            );
            layout.settings_public_bounds = public_bounds;
            layout.settings_embed_bounds = embed_bounds;
            thread_bounds.origin.y += settings_height + gutter;
            thread_bounds.size.height -= settings_height + gutter;
        }
    }

    layout.thread_bounds = thread_bounds;
    layout.code_bounds = code_bounds;
    layout.terminal_bounds = terminal_bounds;
    layout.metrics_bounds = metrics_bounds;
    layout.status_bounds = Bounds::new(0.0, height - status_h, width, status_h);
    state.hud_layout = layout;

    let mut cx = PaintContext::new(scene, text_system, scale_factor);
    state.hud_ui.thread.paint(thread_bounds, &mut cx);
    state.hud_ui.code.paint(code_bounds, &mut cx);
    state.hud_ui.terminal.paint(terminal_bounds, &mut cx);
    state.hud_ui.metrics.paint(metrics_bounds, &mut cx);

    if state.hud_ui.thread.entry_count() == 0 {
        let placeholder = cx.text.layout(
            "No events yet",
            Point::new(thread_bounds.origin.x + 10.0, thread_bounds.origin.y + 10.0),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(placeholder);
    }

    if let Some(ctx) = state.hud_context.as_ref() {
        let repo = format!("{}/{}", ctx.username, ctx.repo);
        let scope = if ctx.is_public { "public" } else { "private" };
        state.hud_ui.status_bar.set_items(vec![
            StatusItem::text("status", state.hud_ui.status_text.clone())
                .align(StatusItemAlignment::Left),
            StatusItem::text("scope", scope).center(),
            StatusItem::text("repo", repo).right(),
        ]);
    }
    state
        .hud_ui
        .status_bar
        .paint(state.hud_layout.status_bounds, &mut cx);
}

fn draw_hud_settings(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    settings: &HudSettingsData,
    bounds: Bounds,
) -> (Bounds, Bounds) {
    scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let label = text_system.layout(
        "HUD Settings",
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 6.0),
        theme::font_size::XS,
        theme::text::MUTED,
    );
    scene.draw_text(label);

    let toggle_w = bounds.size.width - 20.0;
    let toggle_h = 18.0;
    let public_bounds = Bounds::new(
        bounds.origin.x + 10.0,
        bounds.origin.y + 22.0,
        toggle_w,
        toggle_h,
    );
    let embed_bounds = Bounds::new(
        bounds.origin.x + 10.0,
        bounds.origin.y + 42.0,
        toggle_w,
        toggle_h,
    );

    let public_label = if settings.public {
        ("Public ON", theme::status::SUCCESS)
    } else {
        ("Public OFF", theme::text::MUTED)
    };
    scene.draw_quad(
        Quad::new(public_bounds)
            .with_background(theme::bg::ELEVATED)
            .with_border(public_label.1, 1.0),
    );
    let public_run = text_system.layout(
        public_label.0,
        Point::new(public_bounds.origin.x + 8.0, public_bounds.origin.y + 3.0),
        theme::font_size::XS,
        public_label.1,
    );
    scene.draw_text(public_run);

    let embed_label = if settings.embed_allowed {
        ("Embed ON", theme::status::SUCCESS)
    } else {
        ("Embed OFF", theme::text::MUTED)
    };
    scene.draw_quad(
        Quad::new(embed_bounds)
            .with_background(theme::bg::ELEVATED)
            .with_border(embed_label.1, 1.0),
    );
    let embed_run = text_system.layout(
        embed_label.0,
        Point::new(embed_bounds.origin.x + 8.0, embed_bounds.origin.y + 3.0),
        theme::font_size::XS,
        embed_label.1,
    );
    scene.draw_text(embed_run);

    (public_bounds, embed_bounds)
}
