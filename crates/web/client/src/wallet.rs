use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

// JavaScript bridge functions for browser-based Breez SDK
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = breezWalletInit)]
    fn js_wallet_init() -> js_sys::Promise;

    #[wasm_bindgen(js_name = breezWalletConnect)]
    fn js_wallet_connect(entropy_hex: &str, network: &str) -> js_sys::Promise;

    #[wasm_bindgen(js_name = breezWalletGetBalance)]
    fn js_wallet_get_balance() -> js_sys::Promise;

    #[wasm_bindgen(js_name = breezWalletGetSparkAddress)]
    fn js_wallet_get_spark_address() -> js_sys::Promise;

    #[wasm_bindgen(js_name = breezWalletGetBitcoinAddress)]
    fn js_wallet_get_bitcoin_address() -> js_sys::Promise;

    #[wasm_bindgen(js_name = breezWalletListPayments)]
    fn js_wallet_list_payments(limit: u32, offset: u32) -> js_sys::Promise;

    #[wasm_bindgen(js_name = breezWalletSendPayment)]
    fn js_wallet_send_payment(payment_request: &str, amount_sats: Option<u64>) -> js_sys::Promise;

    #[wasm_bindgen(js_name = breezWalletCreateInvoice)]
    fn js_wallet_create_invoice(amount_sats: u64, description: &str) -> js_sys::Promise;

    #[wasm_bindgen(js_name = breezWalletIsConnected)]
    fn js_wallet_is_connected() -> bool;
}
use wgpui::{
    Bounds, Component, EventContext, EventResult, InputEvent, MouseButton, PaintContext, Point,
    Quad, theme,
};
use wgpui::components::atoms::{BitcoinNetwork, PaymentMethod, PaymentStatus};
use wgpui::components::molecules::{
    BalanceCard, InvoiceDisplay, InvoiceInfo, InvoiceType, PaymentDirection, PaymentInfo,
    PaymentRow, WalletBalance,
};
use wgpui::{Button, ButtonVariant, TextInput};

use crate::state::{AppState, AppView};
use crate::utils::js_optional_string;

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum WalletStatus {
    Loading,
    Ready,
    Partial,
    Error,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum WalletView {
    Overview,
    Send,
    Receive,
}

#[derive(Clone)]
pub(crate) struct WalletBalanceData {
    pub(crate) spark_sats: u64,
    pub(crate) lightning_sats: u64,
    pub(crate) onchain_sats: u64,
}

#[derive(Clone, Default)]
pub(crate) struct WalletAddressesData {
    pub(crate) spark: Option<String>,
    pub(crate) onchain: Option<String>,
}

#[derive(Clone)]
pub(crate) struct WalletPaymentData {
    pub(crate) id: String,
    pub(crate) amount_sats: u64,
    pub(crate) fee_sats: u64,
    pub(crate) direction: String,
    pub(crate) method: String,
    pub(crate) status: String,
    pub(crate) timestamp: String,
    pub(crate) description: Option<String>,
}

#[derive(Clone)]
pub(crate) struct WalletInvoiceData {
    pub(crate) method: String,
    pub(crate) payment_request: String,
    pub(crate) amount_sats: Option<u64>,
    pub(crate) description: Option<String>,
}

pub(crate) enum WalletAction {
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

pub(crate) struct WalletUi {
    pub(crate) status: WalletStatus,
    pub(crate) view: WalletView,
    pub(crate) network: Option<String>,
    pub(crate) balance: Option<WalletBalanceData>,
    pub(crate) addresses: WalletAddressesData,
    pub(crate) payments: Vec<WalletPaymentData>,
    pub(crate) payment_rows: Vec<PaymentRow>,
    pub(crate) last_invoice: Option<WalletInvoiceData>,
    pub(crate) error: Option<String>,
    pub(crate) send_notice: Option<String>,
    pub(crate) receive_notice: Option<String>,
    pub(crate) send_address_input: TextInput,
    pub(crate) send_amount_input: TextInput,
    pub(crate) receive_amount_input: TextInput,
    pub(crate) send_button: Button,
    pub(crate) receive_spark_button: Button,
    pub(crate) receive_lightning_button: Button,
    pub(crate) receive_onchain_button: Button,
    pub(crate) refresh_button: Button,
    pub(crate) event_ctx: EventContext,
    layout: WalletLayout,
    actions: Rc<RefCell<Vec<WalletAction>>>,
}

impl WalletUi {
    pub(crate) fn new() -> Self {
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

    pub(crate) fn set_invoice(&mut self, invoice: WalletInvoiceData) {
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

    pub(crate) fn take_actions(&self) -> Vec<WalletAction> {
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

    pub(crate) fn handle_event(&mut self, event: &InputEvent) -> EventResult {
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
                for (row, bounds) in self
                    .payment_rows
                    .iter_mut()
                    .zip(self.layout.payment_rows.iter())
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

    pub(crate) fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
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

fn parse_wallet_balance(value: &JsValue) -> Option<WalletBalanceData> {
    if value.is_null() || value.is_undefined() {
        return None;
    }

    let obj = js_sys::Object::from(value.clone());
    Some(WalletBalanceData {
        spark_sats: js_value_u64(&js_sys::Reflect::get(&obj, &"spark_sats".into()).ok()?)?,
        lightning_sats: js_value_u64(&js_sys::Reflect::get(&obj, &"lightning_sats".into()).ok()?)?,
        onchain_sats: js_value_u64(&js_sys::Reflect::get(&obj, &"onchain_sats".into()).ok()?)?,
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
        .unwrap_or_else(|| "lightning".to_string());
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
        .and_then(|v| js_optional_string(&v));
    let balance = js_sys::Reflect::get(&obj, &"balance".into())
        .ok()
        .and_then(|v| parse_wallet_balance(&v));
    let addresses = js_sys::Reflect::get(&obj, &"addresses".into())
        .ok()
        .map(|v| parse_wallet_addresses(&v))
        .unwrap_or_default();
    let error = js_sys::Reflect::get(&obj, &"error".into())
        .ok()
        .and_then(|v| js_optional_string(&v));

    let mut payments = Vec::new();
    let payments_value =
        js_sys::Reflect::get(&obj, &"payments".into()).unwrap_or(JsValue::NULL);
    let arr = js_sys::Array::from(&payments_value);
    for idx in 0..arr.length() {
        if let Some(payment) = parse_wallet_payment(&arr.get(idx)) {
            payments.push(payment);
        }
    }

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
    let status = js_sys::Reflect::get(&obj, &"status".into())
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_else(|| "pending".to_string());
    let method = js_sys::Reflect::get(&obj, &"method".into())
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_else(|| "lightning".to_string());
    let amount_sats =
        js_value_u64(&js_sys::Reflect::get(&obj, &"amount_sats".into()).ok()?).unwrap_or(0);
    let fee_sats = js_value_u64(&js_sys::Reflect::get(&obj, &"fee_sats".into()).ok()?).unwrap_or(0);

    Some(WalletSendData {
        status,
        method,
        amount_sats,
        fee_sats,
    })
}

fn parse_wallet_invoice(value: JsValue) -> Option<WalletInvoiceData> {
    let obj = js_sys::Object::from(value);
    let method = js_sys::Reflect::get(&obj, &"method".into())
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_else(|| "spark".to_string());
    let payment_request = js_sys::Reflect::get(&obj, &"payment_request".into())
        .ok()
        .and_then(|v| v.as_string())?;
    let amount_sats =
        js_sys::Reflect::get(&obj, &"amount_sats".into())
            .ok()
            .and_then(|v| js_value_u64(&v));
    let description = js_sys::Reflect::get(&obj, &"description".into())
        .ok()
        .and_then(|v| js_optional_string(&v));

    Some(WalletInvoiceData {
        method,
        payment_request,
        amount_sats,
        description,
    })
}

/// Fetch wallet seed from worker, initialize browser SDK, and get wallet info
async fn fetch_wallet_summary() -> Result<WalletSummaryData, String> {
    // First, check if SDK is already connected
    if !js_wallet_is_connected() {
        // Initialize the Breez SDK WASM
        let init_result = JsFuture::from(js_wallet_init())
            .await
            .map_err(|e| format!("SDK init failed: {:?}", e))?;

        let init_obj = js_sys::Object::from(init_result);
        if let Ok(err) = js_sys::Reflect::get(&init_obj, &"error".into()) {
            if !err.is_undefined() && !err.is_null() {
                return Err(format!("SDK init error: {}", err.as_string().unwrap_or_default()));
            }
        }

        // Fetch seed from wallet-worker
        let window = web_sys::window().ok_or("No window available")?;
        let resp = JsFuture::from(window.fetch_with_str("/api/wallet/seed"))
            .await
            .map_err(|_| "Failed to fetch wallet seed".to_string())?;
        let resp: web_sys::Response = resp.dyn_into().map_err(|_| "Invalid response".to_string())?;

        if !resp.ok() {
            // Fallback to old summary endpoint if seed not available
            return fetch_wallet_summary_fallback().await;
        }

        let json = JsFuture::from(resp.json().map_err(|_| "Invalid response".to_string())?)
            .await
            .map_err(|_| "Invalid response".to_string())?;

        let seed_obj = js_sys::Object::from(json);
        let entropy_hex = js_sys::Reflect::get(&seed_obj, &"entropy_hex".into())
            .ok()
            .and_then(|v| v.as_string())
            .ok_or("Missing entropy_hex")?;
        let network = js_sys::Reflect::get(&seed_obj, &"network".into())
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_else(|| "testnet".to_string());

        // Connect wallet with seed
        let connect_result = JsFuture::from(js_wallet_connect(&entropy_hex, &network))
            .await
            .map_err(|e| format!("Wallet connect failed: {:?}", e))?;

        let connect_obj = js_sys::Object::from(connect_result);
        if let Ok(err) = js_sys::Reflect::get(&connect_obj, &"error".into()) {
            if !err.is_undefined() && !err.is_null() {
                return Err(format!("Connect error: {}", err.as_string().unwrap_or_default()));
            }
        }
    }

    // Now get wallet info from browser SDK
    let mut errors = Vec::new();

    // Get balance
    let balance = match JsFuture::from(js_wallet_get_balance()).await {
        Ok(result) => {
            let obj = js_sys::Object::from(result);
            if let Ok(balance_val) = js_sys::Reflect::get(&obj, &"balance".into()) {
                parse_wallet_balance(&balance_val)
            } else if let Ok(err) = js_sys::Reflect::get(&obj, &"error".into()) {
                errors.push(format!("balance: {}", err.as_string().unwrap_or_default()));
                None
            } else {
                None
            }
        }
        Err(e) => {
            errors.push(format!("balance: {:?}", e));
            None
        }
    };

    // Get spark address
    let spark_address = match JsFuture::from(js_wallet_get_spark_address()).await {
        Ok(result) => {
            let obj = js_sys::Object::from(result);
            js_sys::Reflect::get(&obj, &"address".into())
                .ok()
                .and_then(|v| v.as_string())
        }
        Err(e) => {
            errors.push(format!("spark address: {:?}", e));
            None
        }
    };

    // Get bitcoin address
    let onchain_address = match JsFuture::from(js_wallet_get_bitcoin_address()).await {
        Ok(result) => {
            let obj = js_sys::Object::from(result);
            js_sys::Reflect::get(&obj, &"address".into())
                .ok()
                .and_then(|v| v.as_string())
        }
        Err(e) => {
            errors.push(format!("onchain address: {:?}", e));
            None
        }
    };

    // Get payments
    let payments = match JsFuture::from(js_wallet_list_payments(10, 0)).await {
        Ok(result) => {
            let obj = js_sys::Object::from(result);
            if let Ok(payments_val) = js_sys::Reflect::get(&obj, &"payments".into()) {
                let arr = js_sys::Array::from(&payments_val);
                let mut payments_vec = Vec::new();
                for idx in 0..arr.length() {
                    if let Some(payment) = parse_wallet_payment(&arr.get(idx)) {
                        payments_vec.push(payment);
                    }
                }
                payments_vec
            } else {
                Vec::new()
            }
        }
        Err(e) => {
            errors.push(format!("payments: {:?}", e));
            Vec::new()
        }
    };

    let status = if balance.is_some() {
        if errors.is_empty() {
            WalletStatus::Ready
        } else {
            WalletStatus::Partial
        }
    } else {
        WalletStatus::Error
    };

    Ok(WalletSummaryData {
        status,
        network: Some("mainnet".to_string()), // TODO: get from SDK
        balance,
        addresses: WalletAddressesData {
            spark: spark_address,
            onchain: onchain_address,
        },
        payments,
        error: if errors.is_empty() {
            None
        } else {
            Some(errors.join("; "))
        },
    })
}

/// Fallback to old wallet-worker summary endpoint
async fn fetch_wallet_summary_fallback() -> Result<WalletSummaryData, String> {
    let window = web_sys::window().ok_or("No window available")?;
    let resp = JsFuture::from(window.fetch_with_str("/api/wallet/summary"))
        .await
        .map_err(|_| "Failed to fetch wallet summary".to_string())?;
    let resp: web_sys::Response = resp.dyn_into().map_err(|_| "Invalid response".to_string())?;
    if !resp.ok() {
        return Err(format!("Wallet summary failed ({})", resp.status()));
    }
    let json = JsFuture::from(resp.json().map_err(|_| "Invalid response".to_string())?)
        .await
        .map_err(|_| "Invalid response".to_string())?;

    parse_wallet_summary(json).ok_or_else(|| "Wallet summary malformed".to_string())
}

async fn send_wallet_payment(
    payment_request: &str,
    amount_sats: Option<u64>,
) -> Result<WalletSendData, String> {
    let payload = js_sys::Object::new();
    js_sys::Reflect::set(&payload, &"payment_request".into(), &JsValue::from_str(payment_request))
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

    JsFuture::from(resp.json().map_err(|_| "Invalid response".to_string())?)
        .await
        .map_err(|_| "Invalid response".to_string())
}

pub(crate) fn dispatch_wallet_event(state: &Rc<RefCell<AppState>>, event: InputEvent) -> EventResult {
    // Use try_borrow_mut to avoid panic if animation loop holds borrow
    let Ok(mut guard) = state.try_borrow_mut() else {
        return EventResult::Ignored;
    };
    let is_owner = guard
        .hud_context
        .as_ref()
        .map(|ctx| ctx.is_owner)
        .unwrap_or(false);
    if guard.view != AppView::RepoView || !is_owner {
        return EventResult::Ignored;
    }

    let result = guard.wallet.handle_event(&event);
    let actions = guard.wallet.take_actions();
    drop(guard);

    if !actions.is_empty() {
        queue_wallet_actions(state.clone(), actions);
    }

    result
}

pub(crate) fn queue_wallet_actions(state: Rc<RefCell<AppState>>, actions: Vec<WalletAction>) {
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
    if let Ok(mut guard) = state.try_borrow_mut() {
        guard.wallet.status = WalletStatus::Loading;
        guard.wallet.error = None;
    }

    match fetch_wallet_summary().await {
        Ok(summary) => {
            if let Ok(mut guard) = state.try_borrow_mut() {
                guard.wallet.apply_summary(summary);
            }
        }
        Err(err) => {
            if let Ok(mut guard) = state.try_borrow_mut() {
                guard.wallet.status = WalletStatus::Error;
                guard.wallet.error = Some(err);
            }
        }
    }
}

async fn handle_receive_action(
    state: Rc<RefCell<AppState>>,
    method: &str,
    requires_amount: bool,
) {
    let amount_input = state
        .try_borrow()
        .ok()
        .map(|s| s.wallet.receive_amount_input.get_value().to_string())
        .unwrap_or_default();

    let amount_sats = match parse_amount_input(&amount_input) {
        Ok(amount) => amount,
        Err(err) => {
            if let Ok(mut guard) = state.try_borrow_mut() {
                guard.wallet.receive_notice = Some(format!("Error: {}", err));
            }
            return;
        }
    };

    if requires_amount && amount_sats.is_none() {
        if let Ok(mut guard) = state.try_borrow_mut() {
            guard.wallet.receive_notice = Some("Error: amount is required".to_string());
        }
        return;
    }

    if let Ok(mut guard) = state.try_borrow_mut() {
        guard.wallet.receive_notice = Some("Requesting invoice...".to_string());
        guard.wallet.last_invoice = None;
    }

    match request_wallet_receive(method, amount_sats).await {
        Ok(invoice) => {
            if let Ok(mut guard) = state.try_borrow_mut() {
                guard.wallet.set_invoice(invoice);
                guard.wallet.receive_amount_input.set_value("");
            }
        }
        Err(err) => {
            if let Ok(mut guard) = state.try_borrow_mut() {
                guard.wallet.receive_notice = Some(format!("Error: {}", err));
            }
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
            let (payment_request, amount_input) = state
                .try_borrow()
                .ok()
                .map(|s| {
                    (
                        s.wallet.send_address_input.get_value().trim().to_string(),
                        s.wallet.send_amount_input.get_value().to_string(),
                    )
                })
                .unwrap_or_default();

            if payment_request.is_empty() {
                if let Ok(mut guard) = state.try_borrow_mut() {
                    guard.wallet.send_notice = Some("Error: payment request required".to_string());
                }
                return;
            }

            let amount_sats = match parse_amount_input(&amount_input) {
                Ok(amount) => amount,
                Err(err) => {
                    if let Ok(mut guard) = state.try_borrow_mut() {
                        guard.wallet.send_notice = Some(format!("Error: {}", err));
                    }
                    return;
                }
            };

            if let Ok(mut guard) = state.try_borrow_mut() {
                guard.wallet.send_notice = Some("Sending payment...".to_string());
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

                    if let Ok(mut guard) = state.try_borrow_mut() {
                        guard.wallet.send_notice = Some(notice);
                        if sent.status != "failed" {
                            guard.wallet.send_address_input.set_value("");
                            guard.wallet.send_amount_input.set_value("");
                        }
                    }

                    if sent.status != "failed" {
                        refresh_wallet_summary(state).await;
                    }
                }
                Err(err) => {
                    if let Ok(mut guard) = state.try_borrow_mut() {
                        guard.wallet.send_notice = Some(format!("Error: {}", err));
                    }
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
