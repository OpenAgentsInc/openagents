//! OpenAgents Web - WGPUI Landing Page
//!
//! Landing page with GitHub login and repo selector.

use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use wgpui::{
    Bounds, Point, Quad, Scene, TextSystem, Platform, InputEvent, MouseButton, Key, NamedKey,
    Modifiers, EventContext, PaintContext, Component, EventResult, Button, ButtonVariant,
    TextInput, WebPlatform, run_animation_loop, setup_resize_observer, theme,
};
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
#[derive(Clone, Default)]
struct HudContext {
    username: String,
    repo: String,
    is_owner: bool,
    // Session info for WebSocket connection
    session_id: Option<String>,
    ws_url: Option<String>,
    status: String, // "idle", "starting", "running", "completed", "failed"
}

/// Task event types from WebSocket streaming
#[derive(Clone, Debug)]
enum TaskEvent {
    Status { status: String },
    Chunk { text: String },
    ToolStart { tool_name: String, tool_id: String },
    ToolDone { tool_id: String, output: String, is_error: bool },
    ToolProgress { tool_id: String, elapsed_secs: f32 },
    Usage { input_tokens: u64, output_tokens: u64, total_cost_usd: f64 },
    Done { summary: String },
    Error { error: String },
}

/// Tool call state for rendering
#[derive(Clone, Debug)]
struct ToolCallState {
    tool_name: String,
    tool_id: String,
    output: Option<String>,
    is_error: bool,
    elapsed_secs: f32,
    done: bool,
}

/// Thread state for streaming content
#[derive(Clone, Default)]
struct ThreadState {
    status: String,
    text_chunks: Vec<String>,
    tool_calls: Vec<ToolCallState>,
    usage: Option<(u64, u64, f64)>, // (input, output, cost)
    done: bool,
    error: Option<String>,
}

/// Session info for sidebar
#[derive(Clone)]
struct SessionInfo {
    id: String,
    timestamp: String,
    model: String,
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

    // App shell state
    left_dock_open: bool,
    right_dock_open: bool,
    full_auto_enabled: bool,
    full_auto_bounds: Bounds,
    selected_model: String,
    sessions: Vec<SessionInfo>,
    wallet: WalletUi,

    // Thread state for streaming events
    thread: ThreadState,
    // Start form
    prompt_input: TextInput,
    start_button: Button,
    start_button_bounds: Bounds,
    prompt_input_bounds: Bounds,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            mouse_pos: Point::ZERO,
            button_hovered: false,
            button_bounds: Bounds::ZERO,
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
            // App shell defaults
            left_dock_open: true,
            right_dock_open: true,
            full_auto_enabled: false,
            full_auto_bounds: Bounds::ZERO,
            selected_model: "sonnet".to_string(),
            sessions: vec![
                SessionInfo { id: "abc123".into(), timestamp: "Today 14:32".into(), model: "sonnet".into() },
                SessionInfo { id: "def456".into(), timestamp: "Yesterday 09:15".into(), model: "opus".into() },
                SessionInfo { id: "ghi789".into(), timestamp: "Dec 28 16:45".into(), model: "sonnet".into() },
            ],
            wallet: WalletUi::new(),
            // Thread state
            thread: ThreadState::default(),
            prompt_input: TextInput::new().placeholder("What would you like to do?"),
            start_button: Button::new("Start Autopilot").variant(ButtonVariant::Primary).padding(16.0, 8.0),
            start_button_bounds: Bounds::ZERO,
            prompt_input_bounds: Bounds::ZERO,
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
        session_id,
        ws_url,
        status,
    })
}

/// Parse a WebSocket message into a TaskEvent
fn parse_task_event(data: &str) -> Option<TaskEvent> {
    let obj: serde_json::Value = serde_json::from_str(data).ok()?;
    let event_type = obj.get("type")?.as_str()?;

    match event_type {
        "status" => Some(TaskEvent::Status {
            status: obj.get("status")?.as_str()?.to_string(),
        }),
        "chunk" => Some(TaskEvent::Chunk {
            text: obj.get("text")?.as_str()?.to_string(),
        }),
        "tool_start" => Some(TaskEvent::ToolStart {
            tool_name: obj.get("tool_name")?.as_str()?.to_string(),
            tool_id: obj.get("tool_id")?.as_str()?.to_string(),
        }),
        "tool_done" => Some(TaskEvent::ToolDone {
            tool_id: obj.get("tool_id")?.as_str()?.to_string(),
            output: obj.get("output").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            is_error: obj.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false),
        }),
        "tool_progress" => Some(TaskEvent::ToolProgress {
            tool_id: obj.get("tool_id")?.as_str()?.to_string(),
            elapsed_secs: obj.get("elapsed_secs").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
        }),
        "usage" => Some(TaskEvent::Usage {
            input_tokens: obj.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
            output_tokens: obj.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
            total_cost_usd: obj.get("total_cost_usd").and_then(|v| v.as_f64()).unwrap_or(0.0),
        }),
        "done" => Some(TaskEvent::Done {
            summary: obj.get("summary").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        }),
        "error" => Some(TaskEvent::Error {
            error: obj.get("error")?.as_str()?.to_string(),
        }),
        _ => None,
    }
}

/// Apply a task event to the thread state
fn apply_task_event(thread: &mut ThreadState, event: TaskEvent) {
    match event {
        TaskEvent::Status { status } => {
            thread.status = status;
        }
        TaskEvent::Chunk { text } => {
            thread.text_chunks.push(text);
        }
        TaskEvent::ToolStart { tool_name, tool_id } => {
            thread.tool_calls.push(ToolCallState {
                tool_name,
                tool_id,
                output: None,
                is_error: false,
                elapsed_secs: 0.0,
                done: false,
            });
        }
        TaskEvent::ToolDone { tool_id, output, is_error } => {
            if let Some(tool) = thread.tool_calls.iter_mut().find(|t| t.tool_id == tool_id) {
                tool.output = Some(output);
                tool.is_error = is_error;
                tool.done = true;
            }
        }
        TaskEvent::ToolProgress { tool_id, elapsed_secs } => {
            if let Some(tool) = thread.tool_calls.iter_mut().find(|t| t.tool_id == tool_id) {
                tool.elapsed_secs = elapsed_secs;
            }
        }
        TaskEvent::Usage { input_tokens, output_tokens, total_cost_usd } => {
            thread.usage = Some((input_tokens, output_tokens, total_cost_usd));
        }
        TaskEvent::Done { summary: _ } => {
            thread.done = true;
            thread.status = "completed".to_string();
        }
        TaskEvent::Error { error } => {
            thread.error = Some(error);
            thread.status = "failed".to_string();
        }
    }
}

/// Connect to WebSocket for streaming events
fn connect_websocket(state: Rc<RefCell<AppState>>, ws_url: &str) {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };

    // Build full WebSocket URL
    let protocol = if window.location().protocol().unwrap_or_default() == "https:" {
        "wss:"
    } else {
        "ws:"
    };
    let host = window.location().host().unwrap_or_default();
    let full_url = format!("{}//{}{}", protocol, host, ws_url);

    let ws = match web_sys::WebSocket::new(&full_url) {
        Ok(ws) => ws,
        Err(e) => {
            web_sys::console::error_1(&format!("WebSocket creation failed: {:?}", e).into());
            return;
        }
    };

    // Handle incoming messages
    let state_clone = state.clone();
    let onmessage = Closure::<dyn FnMut(_)>::new(move |event: web_sys::MessageEvent| {
        if let Some(data) = event.data().as_string() {
            if let Some(task_event) = parse_task_event(&data) {
                let mut state = state_clone.borrow_mut();
                apply_task_event(&mut state.thread, task_event);
            }
        }
    });
    ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();

    // Handle open
    let state_clone = state.clone();
    let onopen = Closure::<dyn FnMut(_)>::new(move |_: web_sys::Event| {
        let mut state = state_clone.borrow_mut();
        state.thread.status = "connected".to_string();
        web_sys::console::log_1(&"WebSocket connected".into());
    });
    ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
    onopen.forget();

    // Handle errors
    let onerror = Closure::<dyn FnMut(_)>::new(move |e: web_sys::ErrorEvent| {
        web_sys::console::error_1(&format!("WebSocket error: {:?}", e.message()).into());
    });
    ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
    onerror.forget();

    // Handle close
    let state_clone = state.clone();
    let onclose = Closure::<dyn FnMut(_)>::new(move |_: web_sys::CloseEvent| {
        let mut state = state_clone.borrow_mut();
        if !state.thread.done && state.thread.error.is_none() {
            state.thread.status = "disconnected".to_string();
        }
        web_sys::console::log_1(&"WebSocket closed".into());
    });
    ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
    onclose.forget();
}

/// Start autopilot session via POST /api/hud/start
async fn start_autopilot(state: Rc<RefCell<AppState>>, repo: String, prompt: String) {
    // Set starting status
    {
        let mut s = state.borrow_mut();
        s.thread.status = "starting".to_string();
        s.thread.text_chunks.clear();
        s.thread.tool_calls.clear();
        s.thread.usage = None;
        s.thread.done = false;
        s.thread.error = None;
    }

    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };

    // Build request body
    let body = serde_json::json!({
        "repo": repo,
        "prompt": prompt,
    });

    let opts = web_sys::RequestInit::new();
    opts.set_method("POST");
    opts.set_body(&JsValue::from_str(&body.to_string()));

    let headers = match web_sys::Headers::new() {
        Ok(h) => h,
        Err(_) => return,
    };
    let _ = headers.set("Content-Type", "application/json");
    opts.set_headers(&headers);

    let resp = match JsFuture::from(window.fetch_with_str_and_init("/api/hud/start", &opts)).await {
        Ok(r) => r,
        Err(e) => {
            let mut s = state.borrow_mut();
            s.thread.status = "failed".to_string();
            s.thread.error = Some(format!("Failed to start: {:?}", e));
            return;
        }
    };

    let resp: web_sys::Response = match resp.dyn_into() {
        Ok(r) => r,
        Err(_) => return,
    };

    if !resp.ok() {
        let mut s = state.borrow_mut();
        s.thread.status = "failed".to_string();
        s.thread.error = Some(format!("Start failed ({})", resp.status()));
        return;
    }

    let json = match resp.json() {
        Ok(p) => match JsFuture::from(p).await {
            Ok(j) => j,
            Err(_) => return,
        },
        Err(_) => return,
    };

    // Parse response
    let obj = js_sys::Object::from(json);
    let ws_url = js_sys::Reflect::get(&obj, &"ws_url".into())
        .ok()
        .and_then(|v| v.as_string());

    if let Some(url) = ws_url {
        // Update HUD context with new session
        {
            let mut s = state.borrow_mut();
            if let Some(ctx) = s.hud_context.as_mut() {
                ctx.ws_url = Some(url.clone());
                ctx.status = "running".to_string();
            }
            s.thread.status = "running".to_string();
        }

        // Connect WebSocket
        connect_websocket(state, &url);
    }
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
        if state.view != AppView::RepoView || !state.right_dock_open {
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

    // Fetch current user - if logged in, show repo selector, then app shell after selection
    {
        let state_clone = state.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let user_info = fetch_current_user().await;

            {
                let mut state = state_clone.borrow_mut();
                state.loading = false;

                // If logged in, show repo selector first
                if let Some(info) = user_info.clone() {
                    state.user = info;
                    state.view = AppView::RepoSelector;
                    state.repos_loading = true;
                }
            }

            // Fetch repos if logged in
            if user_info.is_some() {
                queue_wallet_actions(state_clone.clone(), vec![WalletAction::Refresh]);

                let repos = fetch_repos().await;
                let mut state = state_clone.borrow_mut();
                state.repos = repos;
                state.repos_loading = false;
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

            // Check Full Auto toggle click (only in RepoView)
            if state.view == AppView::RepoView && state.full_auto_bounds.contains(click_pos) {
                state.full_auto_enabled = !state.full_auto_enabled;
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

                    state.selected_repo = Some(full_name);
                    state.hud_context = Some(HudContext {
                        username: owner,
                        repo: repo_name,
                        is_owner: true,
                        session_id: None,
                        ws_url: None,
                        status: "idle".to_string(),
                    });
                    state.view = AppView::RepoView;
                    return;
                }
            }

            // Check start button click (in RepoView)
            if state.view == AppView::RepoView && state.start_button_bounds.contains(click_pos) {
                // Get repo and prompt
                if let Some(ctx) = state.hud_context.as_ref() {
                    let repo = format!("{}/{}", ctx.username, ctx.repo);
                    let prompt = state.prompt_input.get_value().to_string();

                    if !prompt.is_empty() {
                        // Clear prompt and start
                        state.prompt_input.set_value("");

                        // Drop the borrow before spawning async
                        drop(state);

                        // Spawn the start task
                        let state_for_start = state_clone.clone();
                        wasm_bindgen_futures::spawn_local(async move {
                            start_autopilot(state_for_start, repo, prompt).await;
                        });
                        return;
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

    // Mouse down events for wallet and prompt input
    {
        let state_clone = state.clone();
        let canvas = platform.borrow().canvas().clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::MouseEvent| {
            let x = event.offset_x() as f32;
            let y = event.offset_y() as f32;
            let button = mouse_button_from_event(&event);
            let input_event = InputEvent::MouseDown { button, x, y };

            // Handle prompt input
            {
                let mut state = state_clone.borrow_mut();
                if state.view == AppView::RepoView {
                    let mut event_ctx = EventContext::new();
                    let bounds = state.prompt_input_bounds;
                    state.prompt_input.event(&input_event, bounds, &mut event_ctx);
                }
            }

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

    // Scroll events for repo list
    {
        let state_clone = state.clone();
        let canvas = platform.borrow().canvas().clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::WheelEvent| {
            let mut state = state_clone.borrow_mut();
            if state.view == AppView::RepoSelector {
                state.scroll_offset += event.delta_y() as f32 * 0.5;
                state.scroll_offset = state.scroll_offset.max(0.0);
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
            let cursor = if state.button_hovered || state.hovered_repo_idx.is_some() {
                "pointer"
            } else {
                "default"
            };
            let _ = canvas2.style().set_property("cursor", cursor);
        });
        canvas.add_event_listener_with_callback("mousemove", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    // Keyboard events for dock toggles and prompt input
    {
        let state_clone = state.clone();
        let window = web_sys::window().unwrap();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::KeyboardEvent| {
            let mut handled = EventResult::Ignored;

            if let Some(key) = key_from_event(&event) {
                let modifiers = modifiers_from_event(&event);
                let input_event = InputEvent::KeyDown { key, modifiers };

                // Handle prompt input
                {
                    let mut state = state_clone.borrow_mut();
                    if state.view == AppView::RepoView && state.prompt_input.is_focused() {
                        let mut event_ctx = EventContext::new();
                        let bounds = state.prompt_input_bounds;
                        let result = state.prompt_input.event(&input_event, bounds, &mut event_ctx);
                        if matches!(result, EventResult::Handled) {
                            handled = EventResult::Handled;
                        }
                    }
                }

                if matches!(handled, EventResult::Ignored) {
                    handled = dispatch_wallet_event(
                        &state_clone,
                        input_event,
                    );
                }
            }

            let meta = event.meta_key() || event.ctrl_key();
            let key = event.key();

            let (wallet_focused, prompt_focused) = {
                let state = state_clone.borrow();
                let wallet = state.view == AppView::RepoView && state.right_dock_open && state.wallet.has_focus();
                let prompt = state.view == AppView::RepoView && state.prompt_input.is_focused();
                (wallet, prompt)
            };

            if meta && !wallet_focused && !prompt_focused && state_clone.borrow().view == AppView::RepoView {
                let mut state = state_clone.borrow_mut();
                match key.as_str() {
                    "[" => {
                        state.left_dock_open = !state.left_dock_open;
                        event.prevent_default();
                    }
                    "]" => {
                        state.right_dock_open = !state.right_dock_open;
                        event.prevent_default();
                    }
                    "\\" => {
                        let both_open = state.left_dock_open && state.right_dock_open;
                        state.left_dock_open = !both_open;
                        state.right_dock_open = !both_open;
                        event.prevent_default();
                    }
                    "a" => {
                        state.full_auto_enabled = !state.full_auto_enabled;
                        event.prevent_default();
                    }
                    _ => {}
                }
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
                build_landing_page(&mut scene, platform.text_system(), &mut state, width, height);
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
) {
    // Background
    scene.draw_quad(Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(theme::bg::APP));

    let center_x = width / 2.0;
    let center_y = height / 2.0;

    // Title
    let title = "OpenAgents";
    let title_size = 48.0;
    let title_width = title.len() as f32 * title_size * 0.6;
    let title_run = text_system.layout(
        title,
        Point::new(center_x - title_width / 2.0, center_y - 60.0),
        title_size,
        theme::text::PRIMARY,
    );
    scene.draw_text(title_run);

    // Button
    let (button_text, button_bg_base): (&str, _) = if state.loading {
        ("Loading...", theme::text::MUTED)
    } else {
        ("Login with GitHub", theme::accent::PRIMARY)
    };

    let button_font_size = 16.0;
    let button_text_width = button_text.len() as f32 * button_font_size * 0.6;
    let button_padding_x = 32.0;
    let button_padding_y = 16.0;
    let button_width = button_text_width + button_padding_x * 2.0;
    let button_height = button_font_size + button_padding_y * 2.0;
    let button_x = center_x - button_width / 2.0;
    let button_y = center_y + 20.0;

    if !state.loading {
        state.button_bounds = Bounds::new(button_x, button_y, button_width, button_height);
    } else {
        state.button_bounds = Bounds::ZERO;
    }

    let button_bg = if state.button_hovered && !state.loading {
        button_bg_base
    } else {
        button_bg_base.with_alpha(0.8)
    };

    scene.draw_quad(
        Quad::new(Bounds::new(button_x, button_y, button_width, button_height))
            .with_background(button_bg)
            .with_corner_radius(4.0),
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
    // Background
    scene.draw_quad(Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(theme::bg::APP));

    // Layout constants
    let status_h = 28.0;
    let left_w = if state.left_dock_open { 280.0 } else { 0.0 };
    let right_w = if state.right_dock_open { 300.0 } else { 0.0 };
    let center_x = left_w;
    let center_w = width - left_w - right_w;
    let content_h = height - status_h;

    // Draw sidebars
    if state.left_dock_open {
        draw_left_sidebar(scene, text_system, state, 0.0, 0.0, left_w, content_h);
    }

    // Draw center placeholder
    draw_center_pane(scene, text_system, state, center_x, 0.0, center_w, content_h, scale_factor);

    if state.right_dock_open {
        draw_right_sidebar(
            scene,
            text_system,
            state,
            width - right_w,
            0.0,
            right_w,
            content_h,
            scale_factor,
        );
    }

    // Draw status bar
    draw_status_bar(scene, text_system, state, 0.0, content_h, width, status_h);
}

fn draw_left_sidebar(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) {
    // Sidebar background
    scene.draw_quad(
        Quad::new(Bounds::new(x, y, w, h))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let padding = 12.0;
    let mut cy = y + padding;

    // Model selector
    let model_label = format!("Model: {}", state.selected_model);
    let model_run = text_system.layout(
        &model_label,
        Point::new(x + padding, cy),
        14.0,
        theme::text::PRIMARY,
    );
    scene.draw_text(model_run);
    cy += 32.0;

    // Divider
    scene.draw_quad(
        Quad::new(Bounds::new(x + padding, cy, w - padding * 2.0, 1.0))
            .with_background(theme::border::DEFAULT),
    );
    cy += 16.0;

    // Sessions header
    let sessions_run = text_system.layout(
        "Sessions",
        Point::new(x + padding, cy),
        12.0,
        theme::text::MUTED,
    );
    scene.draw_text(sessions_run);
    cy += 24.0;

    // Session list
    for session in &state.sessions {
        // Session row background
        scene.draw_quad(
            Quad::new(Bounds::new(x + padding, cy, w - padding * 2.0, 28.0))
                .with_background(theme::bg::ELEVATED),
        );

        // Session timestamp
        let ts_run = text_system.layout(
            &session.timestamp,
            Point::new(x + padding + 8.0, cy + 6.0),
            11.0,
            theme::text::PRIMARY,
        );
        scene.draw_text(ts_run);

        // Session model badge
        let model_badge = text_system.layout(
            &session.model,
            Point::new(x + w - padding - 50.0, cy + 6.0),
            10.0,
            theme::text::MUTED,
        );
        scene.draw_text(model_badge);

        cy += 32.0;
    }

    // Hotkey legend at bottom
    let legend_y = y + h - 80.0;

    let legend_title = text_system.layout(
        "Hotkeys",
        Point::new(x + padding, legend_y),
        10.0,
        theme::text::MUTED,
    );
    scene.draw_text(legend_title);

    let hotkeys = [
        ("cmd-[", "left dock"),
        ("cmd-]", "right dock"),
        ("cmd-\\", "both docks"),
        ("cmd-a", "full auto"),
    ];

    for (i, (key, desc)) in hotkeys.iter().enumerate() {
        let hy = legend_y + 14.0 + (i as f32 * 12.0);
        let key_run = text_system.layout(key, Point::new(x + padding, hy), 9.0, theme::accent::PRIMARY);
        scene.draw_text(key_run);
        let desc_run = text_system.layout(desc, Point::new(x + padding + 50.0, hy), 9.0, theme::text::MUTED);
        scene.draw_text(desc_run);
    }
}

fn draw_right_sidebar(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    scale_factor: f32,
) {
    // Sidebar background
    scene.draw_quad(
        Quad::new(Bounds::new(x, y, w, h))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let padding = 12.0;
    let mut cy = y + padding;

    // Full Auto toggle
    let (label, color) = if state.full_auto_enabled {
        ("● FULL AUTO ON", theme::status::SUCCESS)
    } else {
        ("○ FULL AUTO OFF", theme::text::MUTED)
    };

    let toggle_bg = if state.full_auto_enabled {
        theme::status::SUCCESS.with_alpha(0.15)
    } else {
        theme::bg::ELEVATED
    };

    state.full_auto_bounds = Bounds::new(x + padding, cy, w - padding * 2.0, 32.0);

    scene.draw_quad(
        Quad::new(state.full_auto_bounds)
            .with_background(toggle_bg)
            .with_border(color, 1.0),
    );

    let toggle_run = text_system.layout(
        label,
        Point::new(x + padding + 12.0, cy + 8.0),
        14.0,
        color,
    );
    scene.draw_text(toggle_run);
    cy += 48.0;

    // Divider
    scene.draw_quad(
        Quad::new(Bounds::new(x + padding, cy, w - padding * 2.0, 1.0))
            .with_background(theme::border::DEFAULT),
    );
    cy += 16.0;

    let wallet_height = (y + h - padding) - cy;
    if wallet_height > 0.0 {
        let wallet_bounds = Bounds::new(x + padding, cy, w - padding * 2.0, wallet_height);
        let mut cx = PaintContext::new(scene, text_system, scale_factor);
        state.wallet.paint(wallet_bounds, &mut cx);
    }
}

fn draw_center_pane(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    scale_factor: f32,
) {
    // Center pane background (slightly different from sidebars)
    scene.draw_quad(
        Quad::new(Bounds::new(x, y, w, h))
            .with_background(theme::bg::APP),
    );

    let padding = 24.0;
    let mut cy = y + padding;
    let content_x = x + padding;
    let content_w = w - padding * 2.0;

    // Get repo info for display
    let (owner, repo) = state.hud_context.as_ref()
        .map(|ctx| (ctx.username.as_str(), ctx.repo.as_str()))
        .unwrap_or(("", ""));

    // Repo name as title
    let title = format!("{}/{}", owner, repo);
    let title_run = text_system.layout(
        &title,
        Point::new(content_x, cy),
        20.0,
        theme::text::PRIMARY,
    );
    scene.draw_text(title_run);
    cy += 32.0;

    // Status indicator
    let (status_text, status_color) = match state.thread.status.as_str() {
        "idle" => ("Ready to start", theme::text::MUTED),
        "starting" => ("Starting...", theme::status::WARNING),
        "connected" | "running" => ("Running", theme::status::SUCCESS),
        "completed" => ("Completed", theme::status::SUCCESS),
        "failed" => ("Failed", theme::status::ERROR),
        "disconnected" => ("Disconnected", theme::status::WARNING),
        _ => ("Ready", theme::text::MUTED),
    };
    let status_run = text_system.layout(status_text, Point::new(content_x, cy), 11.0, status_color);
    scene.draw_text(status_run);
    cy += 20.0;

    // Show usage if available
    if let Some((input, output, cost)) = state.thread.usage {
        let usage_text = format!("{} in / {} out, ${:.4}", input, output, cost);
        let usage_run = text_system.layout(&usage_text, Point::new(content_x + 120.0, cy - 20.0), 10.0, theme::text::MUTED);
        scene.draw_text(usage_run);
    }

    // Divider
    scene.draw_quad(
        Quad::new(Bounds::new(content_x, cy, content_w, 1.0))
            .with_background(theme::border::DEFAULT),
    );
    cy += 16.0;

    // Check if we should show start form or thread content
    let is_idle = state.thread.status == "idle" || state.thread.status.is_empty();
    let has_content = !state.thread.text_chunks.is_empty() || !state.thread.tool_calls.is_empty();

    if is_idle && !has_content {
        // Show start form
        draw_start_form(scene, text_system, state, content_x, cy, content_w, h - cy - padding, scale_factor);
    } else {
        // Show thread content
        draw_thread_content(scene, text_system, state, content_x, cy, content_w, h - cy - padding);
    }
}

fn draw_start_form(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    x: f32,
    y: f32,
    w: f32,
    _h: f32,
    scale_factor: f32,
) {
    let mut cy = y;

    // Label
    let label = text_system.layout(
        "What would you like Autopilot to do?",
        Point::new(x, cy),
        14.0,
        theme::text::PRIMARY,
    );
    scene.draw_text(label);
    cy += 28.0;

    // Prompt input
    let input_h = 80.0;
    let input_bounds = Bounds::new(x, cy, w, input_h);
    state.prompt_input_bounds = input_bounds;

    // Draw input background
    scene.draw_quad(
        Quad::new(input_bounds)
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    // Draw the actual text input
    let mut cx = PaintContext::new(scene, text_system, scale_factor);
    state.prompt_input.paint(input_bounds, &mut cx);
    cy += input_h + 16.0;

    // Start button
    let button_w = 160.0;
    let button_h = 36.0;
    let button_bounds = Bounds::new(x, cy, button_w, button_h);
    state.start_button_bounds = button_bounds;

    let mut cx = PaintContext::new(scene, text_system, scale_factor);
    state.start_button.paint(button_bounds, &mut cx);
}

fn draw_thread_content(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &AppState,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) {
    let mut cy = y;
    let line_height = 14.0;
    let max_y = y + h - 20.0;

    // Combine all text chunks
    let full_text: String = state.thread.text_chunks.join("");

    // Wrap text and display
    let chars_per_line = (w / 7.0).floor() as usize;
    let lines: Vec<&str> = full_text.lines().collect();

    for line in lines {
        if cy > max_y {
            // Show "more..." indicator
            let more = text_system.layout("...", Point::new(x, cy), 11.0, theme::text::MUTED);
            scene.draw_text(more);
            break;
        }

        // Wrap long lines
        if line.len() > chars_per_line && chars_per_line > 0 {
            for chunk in line.as_bytes().chunks(chars_per_line) {
                if cy > max_y { break; }
                if let Ok(chunk_str) = std::str::from_utf8(chunk) {
                    let text_run = text_system.layout(chunk_str, Point::new(x, cy), 11.0, theme::text::PRIMARY);
                    scene.draw_text(text_run);
                    cy += line_height;
                }
            }
        } else if !line.is_empty() {
            let text_run = text_system.layout(line, Point::new(x, cy), 11.0, theme::text::PRIMARY);
            scene.draw_text(text_run);
            cy += line_height;
        } else {
            cy += line_height * 0.5; // Empty line
        }
    }

    // Draw tool calls
    if !state.thread.tool_calls.is_empty() && cy < max_y {
        cy += 12.0;
        let tools_label = text_system.layout("Tool Calls:", Point::new(x, cy), 10.0, theme::text::MUTED);
        scene.draw_text(tools_label);
        cy += 16.0;

        for tool in &state.thread.tool_calls {
            if cy > max_y { break; }

            let status_icon = if tool.done {
                if tool.is_error { "x" } else { "+" }
            } else {
                "o"
            };
            let tool_color = if tool.is_error {
                theme::status::ERROR
            } else if tool.done {
                theme::status::SUCCESS
            } else {
                theme::status::WARNING
            };

            let tool_text = format!("{} {} ({:.1}s)", status_icon, tool.tool_name, tool.elapsed_secs);
            let tool_run = text_system.layout(&tool_text, Point::new(x + 8.0, cy), 10.0, tool_color);
            scene.draw_text(tool_run);
            cy += 14.0;
        }
    }

    // Show error if any
    if let Some(ref error) = state.thread.error {
        let error_y = (y + h - 40.0).max(cy + 8.0);
        scene.draw_quad(
            Quad::new(Bounds::new(x, error_y, w, 32.0))
                .with_background(theme::status::ERROR.with_alpha(0.15))
                .with_border(theme::status::ERROR, 1.0),
        );
        let error_run = text_system.layout(error, Point::new(x + 8.0, error_y + 8.0), 10.0, theme::status::ERROR);
        scene.draw_text(error_run);
    }
}

fn draw_status_bar(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &AppState,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) {
    // Status bar background
    scene.draw_quad(
        Quad::new(Bounds::new(x, y, w, h))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let padding = 12.0;

    // Left: dock toggle hints
    let hints = "cmd-[ / cmd-] toggle docks";
    let hints_run = text_system.layout(
        hints,
        Point::new(x + padding, y + 8.0),
        10.0,
        theme::text::MUTED,
    );
    scene.draw_text(hints_run);

    // Right: repo path
    if let Some(ctx) = &state.hud_context {
        let repo_text = format!("{}/{}", ctx.username, ctx.repo);
        let text_w = repo_text.len() as f32 * 10.0 * 0.6;
        let repo_run = text_system.layout(
            &repo_text,
            Point::new(w - padding - text_w, y + 8.0),
            10.0,
            theme::text::MUTED,
        );
        scene.draw_text(repo_run);
    }
}
