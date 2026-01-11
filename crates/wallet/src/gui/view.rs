//! WGPUI wallet view.

use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

use crate::storage::config::WalletConfig as LocalWalletConfig;
use qrcode::QrCode;
use spark::{Balance, Payment, PaymentMethod, PaymentStatus, PaymentType};
use wgpui::components::{Button, Component, EventResult, TextInput};
use wgpui::{
    Bounds, EventContext, Hsla, InputEvent, MouseButton, Point, Quad, Scene, ScrollContainer, Size,
    TextSystem, theme,
};

use super::types::{WalletCommand, WalletTab, WalletUpdate};

const HEADER_HEIGHT: f32 = 86.0;
const TABS_HEIGHT: f32 = 44.0;
const PADDING: f32 = 24.0;
const GAP: f32 = 12.0;
const INPUT_HEIGHT: f32 = 36.0;
const BUTTON_HEIGHT: f32 = 36.0;
const TAB_WIDTH: f32 = 120.0;
const TAB_HEIGHT: f32 = 30.0;
const QR_QUIET_ZONE: usize = 4;
const HISTORY_ROW_HEIGHT: f32 = 52.0;
const HISTORY_HEADER_HEIGHT: f32 = 24.0;
const HISTORY_DETAILS_HEIGHT: f32 = 160.0;
const HISTORY_CHART_HEIGHT: f32 = 140.0;
const HISTORY_LOAD_THRESHOLD: f32 = 120.0;
const HISTORY_OVERSCAN: usize = 3;
const HISTORY_PAGE_SIZE: u32 = 25;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WalletUiEvent {
    SelectTab(WalletTab),
    SubmitSend,
    GenerateReceive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NoticeKind {
    Info,
    Success,
    Error,
}

#[derive(Debug, Clone)]
struct Notice {
    kind: NoticeKind,
    message: String,
}

impl Notice {
    fn color(&self) -> Hsla {
        match self.kind {
            NoticeKind::Info => theme::status::INFO,
            NoticeKind::Success => theme::status::SUCCESS,
            NoticeKind::Error => theme::status::ERROR,
        }
    }
}

#[derive(Debug, Clone)]
struct QrMatrix {
    size: usize,
    modules: Vec<bool>,
}

impl QrMatrix {
    fn from_payload(payload: &str) -> Option<Self> {
        let code = QrCode::new(payload.as_bytes()).ok()?;
        let size = code.width();
        let mut modules = Vec::with_capacity(size * size);
        for y in 0..size {
            for x in 0..size {
                let dark = code[(x, y)] == qrcode::types::Color::Dark;
                modules.push(dark);
            }
        }
        Some(Self { size, modules })
    }

    fn is_dark(&self, x: usize, y: usize) -> bool {
        self.modules
            .get(y.saturating_mul(self.size) + x)
            .copied()
            .unwrap_or(false)
    }
}

struct WalletLayout {
    header: Bounds,
    tabs: Bounds,
    content: Bounds,
    send_tab: Bounds,
    receive_tab: Bounds,
    history_tab: Bounds,
    send: SendLayout,
    receive: ReceiveLayout,
    history: HistoryLayout,
}

struct SendLayout {
    destination_input: Bounds,
    amount_input: Bounds,
    submit_button: Bounds,
    notice: Bounds,
}

struct ReceiveLayout {
    amount_input: Bounds,
    generate_button: Bounds,
    qr: Bounds,
    payload: Bounds,
    notice: Bounds,
}

struct HistoryLayout {
    header: Bounds,
    chart: Bounds,
    list: Bounds,
    details: Bounds,
}

#[derive(Debug, Clone)]
struct BalancePoint {
    timestamp: u64,
    balance: u128,
}

#[derive(Debug, Clone)]
struct PendingSend {
    destination: String,
    amount: u64,
}

pub struct WalletView {
    tab: WalletTab,
    hovered_tab: Option<WalletTab>,
    balance: Balance,
    balance_loaded: bool,
    notice: Option<Notice>,
    send_destination: TextInput,
    send_amount: TextInput,
    send_button: Button,
    receive_amount: TextInput,
    receive_button: Button,
    receive_payload: Option<String>,
    qr_matrix: Option<QrMatrix>,
    history_scroll: ScrollContainer,
    history_items: Vec<Payment>,
    history_chart: Vec<BalancePoint>,
    history_loading: bool,
    history_has_more: bool,
    history_page_size: u32,
    selected_history: Option<usize>,
    send_limit: Option<u64>,
    large_send_confirm: Option<u64>,
    pending_large_send: Option<PendingSend>,
    commands: Vec<WalletCommand>,
    ui_events: Rc<RefCell<Vec<WalletUiEvent>>>,
}

impl WalletView {
    pub fn new() -> Self {
        let ui_events = Rc::new(RefCell::new(Vec::new()));
        let send_events = ui_events.clone();
        let receive_events = ui_events.clone();

        let send_button = Button::new("Send Payment")
            .padding(18.0, 8.0)
            .on_click(move || {
                send_events.borrow_mut().push(WalletUiEvent::SubmitSend);
            });

        let receive_button = Button::new("Generate QR")
            .padding(18.0, 8.0)
            .on_click(move || {
                receive_events
                    .borrow_mut()
                    .push(WalletUiEvent::GenerateReceive);
            });

        let history_page_size = HISTORY_PAGE_SIZE;
        let (send_limit, large_send_confirm) = load_security_limits();

        let mut commands = Vec::new();
        commands.push(WalletCommand::RefreshBalance);
        commands.push(WalletCommand::RequestReceive { amount: None });
        commands.push(WalletCommand::LoadPayments {
            offset: 0,
            limit: history_page_size,
        });

        Self {
            tab: WalletTab::Send,
            hovered_tab: None,
            balance: Balance::default(),
            balance_loaded: false,
            notice: None,
            send_destination: TextInput::new()
                .with_id(1)
                .placeholder("Lightning invoice or Spark address")
                .font_size(theme::font_size::SM)
                .padding(12.0, 8.0),
            send_amount: TextInput::new()
                .with_id(2)
                .placeholder("Amount (sats)")
                .font_size(theme::font_size::SM)
                .padding(12.0, 8.0),
            send_button,
            receive_amount: TextInput::new()
                .with_id(3)
                .placeholder("Amount (sats, optional)")
                .font_size(theme::font_size::SM)
                .padding(12.0, 8.0),
            receive_button,
            receive_payload: None,
            qr_matrix: None,
            history_scroll: ScrollContainer::vertical(Bounds::ZERO),
            history_items: Vec::new(),
            history_chart: Vec::new(),
            history_loading: true,
            history_has_more: true,
            history_page_size,
            selected_history: None,
            send_limit,
            large_send_confirm,
            pending_large_send: None,
            commands,
            ui_events,
        }
    }

    pub fn apply_update(&mut self, update: WalletUpdate) {
        match update {
            WalletUpdate::Balance(balance) => {
                self.balance = balance;
                self.balance_loaded = true;
                self.update_history_chart();
            }
            WalletUpdate::ReceiveReady { payload, amount } => {
                self.receive_payload = Some(payload.clone());
                self.qr_matrix = QrMatrix::from_payload(&payload);
                self.notice = Some(Notice {
                    kind: NoticeKind::Success,
                    message: match amount {
                        Some(sats) => format!("Invoice ready for {} sats", sats),
                        None => "Receive address ready".to_string(),
                    },
                });
            }
            WalletUpdate::SendSuccess { payment_id } => {
                self.notice = Some(Notice {
                    kind: NoticeKind::Success,
                    message: format!("Payment sent ({})", payment_id),
                });
                self.send_destination.set_value("");
                self.send_amount.set_value("");
                self.pending_large_send = None;
            }
            WalletUpdate::PaymentsLoaded {
                payments,
                offset,
                has_more,
            } => {
                if offset == 0 {
                    self.history_items = payments;
                    self.selected_history = None;
                    self.history_scroll.scroll_to(Point::ZERO);
                } else {
                    self.history_items.extend(payments);
                }
                self.history_has_more = has_more;
                self.history_loading = false;
                self.update_history_chart();
            }
            WalletUpdate::Error { message } => {
                self.notice = Some(Notice {
                    kind: NoticeKind::Error,
                    message,
                });
                self.history_loading = false;
                self.pending_large_send = None;
            }
        }
    }

    pub fn drain_commands(&mut self) -> Vec<WalletCommand> {
        std::mem::take(&mut self.commands)
    }

    fn request_history_page(&mut self, offset: u32) {
        self.history_loading = true;
        self.commands.push(WalletCommand::LoadPayments {
            offset,
            limit: self.history_page_size,
        });
    }

    fn update_history_chart(&mut self) {
        let current_total = if self.balance_loaded {
            Some(self.balance.total_sats() as u128)
        } else {
            None
        };
        let points = compute_balance_series(&self.history_items, current_total);
        self.history_chart = downsample_points(&points, 60);
    }

    fn sync_history_scroll(&mut self, list_bounds: Bounds) {
        let content_height = self.history_items.len() as f32 * HISTORY_ROW_HEIGHT;
        self.history_scroll.set_viewport(list_bounds);
        self.history_scroll
            .set_content_size(Size::new(list_bounds.size.width, content_height));
    }

    fn maybe_request_more_history(&mut self, list_bounds: Bounds) {
        if self.history_loading {
            return;
        }
        if self.history_items.is_empty() {
            if self.history_has_more {
                self.request_history_page(0);
            }
            return;
        }
        if !self.history_has_more {
            return;
        }

        let visible_end =
            self.history_scroll.scroll_offset.y + list_bounds.size.height + HISTORY_LOAD_THRESHOLD;
        if visible_end >= self.history_scroll.content_size.height {
            let offset = self.history_items.len() as u32;
            self.request_history_page(offset);
        }
    }

    fn history_visible_range(&self, viewport_height: f32, scroll_offset: f32) -> Range<usize> {
        let item_count = self.history_items.len();
        if item_count == 0 {
            return 0..0;
        }

        let first_visible = (scroll_offset / HISTORY_ROW_HEIGHT).floor() as usize;
        let visible_count = (viewport_height / HISTORY_ROW_HEIGHT).ceil() as usize + 1;

        let start = first_visible.saturating_sub(HISTORY_OVERSCAN);
        let end = (first_visible + visible_count + HISTORY_OVERSCAN).min(item_count);

        start..end
    }

    fn layout(&self, bounds: Bounds) -> WalletLayout {
        let header = Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            HEADER_HEIGHT,
        );
        let tabs = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + HEADER_HEIGHT,
            bounds.size.width,
            TABS_HEIGHT,
        );
        let content = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + HEADER_HEIGHT + TABS_HEIGHT,
            bounds.size.width,
            bounds.size.height - HEADER_HEIGHT - TABS_HEIGHT,
        );

        let tab_y = tabs.origin.y + (TABS_HEIGHT - TAB_HEIGHT) / 2.0;
        let send_tab = Bounds::new(tabs.origin.x + PADDING, tab_y, TAB_WIDTH, TAB_HEIGHT);
        let receive_tab = Bounds::new(
            send_tab.origin.x + TAB_WIDTH + GAP,
            tab_y,
            TAB_WIDTH,
            TAB_HEIGHT,
        );
        let history_tab = Bounds::new(
            receive_tab.origin.x + TAB_WIDTH + GAP,
            tab_y,
            TAB_WIDTH,
            TAB_HEIGHT,
        );

        let form_width = (content.size.width - PADDING * 2.0).min(520.0);
        let form_x = content.origin.x + PADDING;
        let form_y = content.origin.y + PADDING;

        let destination_input = Bounds::new(form_x, form_y, form_width, INPUT_HEIGHT);
        let amount_input = Bounds::new(
            form_x,
            form_y + INPUT_HEIGHT + GAP,
            form_width,
            INPUT_HEIGHT,
        );
        let submit_button = Bounds::new(
            form_x,
            form_y + (INPUT_HEIGHT + GAP) * 2.0,
            form_width,
            BUTTON_HEIGHT,
        );
        let notice = Bounds::new(
            form_x,
            submit_button.origin.y + BUTTON_HEIGHT + GAP,
            content.size.width - PADDING * 2.0,
            INPUT_HEIGHT,
        );

        let receive_form_width = (content.size.width - PADDING * 2.0).min(320.0);
        let receive_form_x = content.origin.x + PADDING;
        let receive_form_y = content.origin.y + PADDING;
        let receive_amount_input = Bounds::new(
            receive_form_x,
            receive_form_y,
            receive_form_width,
            INPUT_HEIGHT,
        );
        let generate_button = Bounds::new(
            receive_form_x,
            receive_form_y + INPUT_HEIGHT + GAP,
            receive_form_width,
            BUTTON_HEIGHT,
        );

        let qr_top = generate_button.origin.y + BUTTON_HEIGHT + GAP * 2.0;
        let available_height = (content.origin.y + content.size.height - PADDING) - qr_top;
        let qr_size = available_height
            .min(content.size.width - PADDING * 2.0)
            .max(0.0);
        let qr = Bounds::new(
            content.origin.x + (content.size.width - qr_size) / 2.0,
            qr_top,
            qr_size,
            qr_size,
        );

        let payload = Bounds::new(
            content.origin.x + PADDING,
            qr.origin.y + qr.size.height + GAP,
            content.size.width - PADDING * 2.0,
            INPUT_HEIGHT,
        );
        let receive_notice = Bounds::new(
            content.origin.x + PADDING,
            payload.origin.y + INPUT_HEIGHT + GAP,
            content.size.width - PADDING * 2.0,
            INPUT_HEIGHT,
        );

        let history_header = Bounds::new(
            content.origin.x + PADDING,
            content.origin.y + PADDING,
            content.size.width - PADDING * 2.0,
            HISTORY_HEADER_HEIGHT,
        );

        let reserved_details = HISTORY_DETAILS_HEIGHT
            .min(content.size.height - PADDING * 2.0 - HISTORY_HEADER_HEIGHT - GAP)
            .max(0.0);
        let min_list_height = HISTORY_ROW_HEIGHT * 3.0;
        let available_for_chart =
            (content.size.height - PADDING * 2.0 - HISTORY_HEADER_HEIGHT - GAP - reserved_details)
                .max(0.0);
        let chart_height =
            HISTORY_CHART_HEIGHT.min((available_for_chart - min_list_height).max(0.0));
        let chart_gap = if chart_height > 0.0 { GAP } else { 0.0 };

        let history_chart = Bounds::new(
            content.origin.x + PADDING,
            history_header.origin.y + HISTORY_HEADER_HEIGHT + GAP,
            content.size.width - PADDING * 2.0,
            chart_height,
        );

        let history_list_y = history_chart.origin.y + chart_height + chart_gap;
        let history_list_height = (content.size.height
            - PADDING * 2.0
            - HISTORY_HEADER_HEIGHT
            - GAP
            - chart_height
            - chart_gap
            - reserved_details)
            .max(min_list_height);
        let history_list = Bounds::new(
            content.origin.x + PADDING,
            history_list_y,
            content.size.width - PADDING * 2.0,
            history_list_height,
        );
        let history_details_y = history_list.origin.y + history_list.size.height + GAP;
        let history_details_height =
            (content.origin.y + content.size.height - PADDING - history_details_y).max(0.0);
        let history_details = Bounds::new(
            content.origin.x + PADDING,
            history_details_y,
            content.size.width - PADDING * 2.0,
            history_details_height,
        );

        WalletLayout {
            header,
            tabs,
            content,
            send_tab,
            receive_tab,
            history_tab,
            send: SendLayout {
                destination_input,
                amount_input,
                submit_button,
                notice,
            },
            receive: ReceiveLayout {
                amount_input: receive_amount_input,
                generate_button,
                qr,
                payload,
                notice: receive_notice,
            },
            history: HistoryLayout {
                header: history_header,
                chart: history_chart,
                list: history_list,
                details: history_details,
            },
        }
    }

    fn balance_text(&self) -> String {
        if self.balance_loaded {
            format!("{} sats", format_sats(self.balance.total_sats()))
        } else {
            "Balance --".to_string()
        }
    }

    fn draw_text(
        &self,
        text: &str,
        x: f32,
        y: f32,
        font_size: f32,
        color: Hsla,
        text_system: &mut TextSystem,
        scene: &mut Scene,
    ) {
        if text.is_empty() {
            return;
        }
        let run = text_system.layout(text, Point::new(x, y + font_size), font_size, color);
        scene.draw_text(run);
    }

    fn draw_tab(
        &self,
        label: &str,
        tab: WalletTab,
        bounds: Bounds,
        text_system: &mut TextSystem,
        scene: &mut Scene,
    ) {
        let active = self.tab == tab;
        let hovered = self.hovered_tab == Some(tab);
        let background = if active {
            theme::bg::SELECTED
        } else if hovered {
            theme::bg::HOVER
        } else {
            theme::bg::SURFACE
        };
        let text_color = if active {
            theme::text::PRIMARY
        } else {
            theme::text::MUTED
        };

        scene.draw_quad(
            Quad::new(bounds)
                .with_background(background)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let font_size = theme::font_size::SM;
        let text_width = text_width(label, font_size);
        let text_x = bounds.origin.x + (bounds.size.width - text_width) / 2.0;
        let text_y = bounds.origin.y + (bounds.size.height - font_size) / 2.0;
        self.draw_text(
            label,
            text_x,
            text_y,
            font_size,
            text_color,
            text_system,
            scene,
        );
    }

    fn draw_notice(
        &self,
        notice: &Notice,
        bounds: Bounds,
        text_system: &mut TextSystem,
        scene: &mut Scene,
    ) {
        self.draw_text(
            &notice.message,
            bounds.origin.x,
            bounds.origin.y,
            theme::font_size::SM,
            notice.color(),
            text_system,
            scene,
        );
    }

    fn draw_qr(&self, bounds: Bounds, text_system: &mut TextSystem, scene: &mut Scene) {
        let Some(qr) = &self.qr_matrix else {
            self.draw_text(
                "Generating QR...",
                bounds.origin.x,
                bounds.origin.y,
                theme::font_size::SM,
                theme::text::MUTED,
                text_system,
                scene,
            );
            return;
        };

        let module_count = qr.size + QR_QUIET_ZONE * 2;
        let module_size = (bounds.size.width / module_count as f32)
            .min(bounds.size.height / module_count as f32)
            .floor();
        if module_size <= 0.0 {
            return;
        }

        let total_size = module_size * module_count as f32;
        let origin_x = bounds.origin.x + (bounds.size.width - total_size) / 2.0;
        let origin_y = bounds.origin.y + (bounds.size.height - total_size) / 2.0;

        let background = theme::bg::APP;
        let foreground = theme::text::PRIMARY;

        scene.draw_quad(
            Quad::new(Bounds::new(origin_x, origin_y, total_size, total_size))
                .with_background(background),
        );

        for y in 0..module_count {
            for x in 0..module_count {
                let is_dark = if x < QR_QUIET_ZONE
                    || y < QR_QUIET_ZONE
                    || x >= qr.size + QR_QUIET_ZONE
                    || y >= qr.size + QR_QUIET_ZONE
                {
                    false
                } else {
                    let code_x = x - QR_QUIET_ZONE;
                    let code_y = y - QR_QUIET_ZONE;
                    qr.is_dark(code_x, code_y)
                };

                if is_dark {
                    let px = origin_x + x as f32 * module_size;
                    let py = origin_y + y as f32 * module_size;
                    scene.draw_quad(
                        Quad::new(Bounds::new(px, py, module_size, module_size))
                            .with_background(foreground),
                    );
                }
            }
        }
    }

    fn history_row_summary(&self, payment: &Payment) -> String {
        format!(
            "{} | {} | {}",
            format_timestamp_display(payment.timestamp),
            format_payment_type(payment.payment_type),
            format_payment_status(payment.status)
        )
    }

    fn history_row_amount(&self, payment: &Payment) -> String {
        let prefix = match payment.payment_type {
            PaymentType::Send => "-",
            PaymentType::Receive => "+",
        };
        format!("{}{} sats", prefix, format_sats_u128(payment.amount))
    }

    fn history_row_amount_color(&self, payment: &Payment) -> Hsla {
        match payment.payment_type {
            PaymentType::Send => theme::text::SECONDARY,
            PaymentType::Receive => theme::accent::PRIMARY,
        }
    }

    fn draw_history(&mut self, layout: &HistoryLayout, cx: &mut wgpui::PaintContext) {
        self.sync_history_scroll(layout.list);

        cx.scene.draw_quad(
            Quad::new(layout.list)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::SUBTLE, 1.0),
        );
        cx.scene.draw_quad(
            Quad::new(layout.details)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::SUBTLE, 1.0),
        );

        self.draw_text(
            "TRANSACTIONS",
            layout.header.origin.x,
            layout.header.origin.y,
            theme::font_size::SM,
            theme::text::MUTED,
            cx.text,
            cx.scene,
        );

        self.draw_history_chart(layout.chart, cx);

        if self.history_items.is_empty() {
            let message = if self.history_loading {
                "Loading transactions..."
            } else {
                "No transactions yet."
            };
            self.draw_text(
                message,
                layout.list.origin.x + GAP,
                layout.list.origin.y + GAP,
                theme::font_size::SM,
                theme::text::MUTED,
                cx.text,
                cx.scene,
            );
        } else {
            self.draw_history_rows(layout.list, cx);

            if self.history_loading {
                let font_size = theme::font_size::XS;
                self.draw_text(
                    "Loading more...",
                    layout.list.origin.x + GAP,
                    layout.list.origin.y + layout.list.size.height - font_size - 6.0,
                    font_size,
                    theme::text::MUTED,
                    cx.text,
                    cx.scene,
                );
            }
        }

        self.draw_history_details(layout.details, cx);
    }

    fn draw_history_chart(&self, bounds: Bounds, cx: &mut wgpui::PaintContext) {
        if bounds.size.height <= 0.0 {
            return;
        }

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::APP)
                .with_border(theme::border::SUBTLE, 1.0),
        );

        let title_size = theme::font_size::XS;
        self.draw_text(
            "BALANCE TREND",
            bounds.origin.x + GAP,
            bounds.origin.y + GAP * 0.6,
            title_size,
            theme::text::MUTED,
            cx.text,
            cx.scene,
        );

        let chart_bounds = Bounds::new(
            bounds.origin.x + GAP,
            bounds.origin.y + GAP * 1.6 + title_size,
            bounds.size.width - GAP * 2.0,
            bounds.size.height - GAP * 2.4 - title_size,
        );
        if chart_bounds.size.width <= 0.0 || chart_bounds.size.height <= 0.0 {
            return;
        }

        if self.history_chart.is_empty() {
            self.draw_text(
                "No balance history yet.",
                chart_bounds.origin.x,
                chart_bounds.origin.y + chart_bounds.size.height / 2.0,
                theme::font_size::SM,
                theme::text::MUTED,
                cx.text,
                cx.scene,
            );
            return;
        }

        let (min_balance, max_balance) = min_max_balance(&self.history_chart);
        let span = (max_balance - min_balance).max(1);
        let span_f = span as f32;

        let points = &self.history_chart;
        let count = points.len().max(1) as f32;
        let bar_width = (chart_bounds.size.width / count).max(2.0);
        let bar_color = theme::accent::PRIMARY.with_alpha(0.25);
        let line_color = theme::accent::PRIMARY;

        let baseline = chart_bounds.origin.y + chart_bounds.size.height;

        for (index, point) in points.iter().enumerate() {
            let x = chart_bounds.origin.x + index as f32 * (chart_bounds.size.width / count);
            let ratio = (point.balance - min_balance) as f32 / span_f;
            let height = ratio * chart_bounds.size.height;
            let y = baseline - height;

            let bar_bounds = Bounds::new(x, y, bar_width, height.max(1.0));
            cx.scene
                .draw_quad(Quad::new(bar_bounds).with_background(bar_color));

            let dot_bounds = Bounds::new(x, y - 1.0, bar_width.max(2.0), 2.0);
            cx.scene
                .draw_quad(Quad::new(dot_bounds).with_background(line_color));
        }

        let min_label = format_sats_u128(min_balance);
        let max_label = format_sats_u128(max_balance);
        let label_size = theme::font_size::XS;
        self.draw_text(
            &format!("min {} sats", min_label),
            chart_bounds.origin.x,
            chart_bounds.origin.y + chart_bounds.size.height - label_size - 2.0,
            label_size,
            theme::text::MUTED,
            cx.text,
            cx.scene,
        );
        let max_width = text_width(&format!("max {} sats", max_label), label_size);
        self.draw_text(
            &format!("max {} sats", max_label),
            chart_bounds.origin.x + chart_bounds.size.width - max_width,
            chart_bounds.origin.y,
            label_size,
            theme::text::MUTED,
            cx.text,
            cx.scene,
        );
    }

    fn draw_history_rows(&self, list_bounds: Bounds, cx: &mut wgpui::PaintContext) {
        let scroll_offset = self.history_scroll.scroll_offset.y;
        let visible_range = self.history_visible_range(list_bounds.size.height, scroll_offset);

        cx.scene.push_clip(list_bounds);

        for index in visible_range {
            let Some(payment) = self.history_items.get(index) else {
                continue;
            };
            let row_y = list_bounds.origin.y + index as f32 * HISTORY_ROW_HEIGHT - scroll_offset;
            let row_bounds = Bounds::new(
                list_bounds.origin.x,
                row_y,
                list_bounds.size.width,
                HISTORY_ROW_HEIGHT,
            );

            if row_bounds.origin.y + row_bounds.size.height < list_bounds.origin.y
                || row_bounds.origin.y > list_bounds.origin.y + list_bounds.size.height
            {
                continue;
            }

            let selected = self.selected_history == Some(index);
            let background = if selected {
                theme::bg::SELECTED
            } else if index % 2 == 0 {
                theme::bg::APP
            } else {
                theme::bg::SURFACE
            };

            cx.scene
                .draw_quad(Quad::new(row_bounds).with_background(background));

            let divider = Bounds::new(
                row_bounds.origin.x,
                row_bounds.origin.y + row_bounds.size.height - 1.0,
                row_bounds.size.width,
                1.0,
            );
            cx.scene
                .draw_quad(Quad::new(divider).with_background(theme::border::SUBTLE));

            let font_size = theme::font_size::SM;
            let text_y = row_bounds.origin.y + (row_bounds.size.height - font_size) / 2.0;
            let summary = self.history_row_summary(payment);
            self.draw_text(
                &summary,
                row_bounds.origin.x + GAP,
                text_y,
                font_size,
                theme::text::PRIMARY,
                cx.text,
                cx.scene,
            );

            let amount = self.history_row_amount(payment);
            let amount_width = text_width(&amount, font_size);
            let amount_x = row_bounds.origin.x + row_bounds.size.width - GAP - amount_width;
            self.draw_text(
                &amount,
                amount_x,
                text_y,
                font_size,
                self.history_row_amount_color(payment),
                cx.text,
                cx.scene,
            );
        }

        cx.scene.pop_clip();
    }

    fn draw_history_details(&self, bounds: Bounds, cx: &mut wgpui::PaintContext) {
        if bounds.size.height <= 0.0 {
            return;
        }

        let padding = GAP;
        let mut y = bounds.origin.y + padding;
        let title_size = theme::font_size::SM;

        self.draw_text(
            "Transaction Details",
            bounds.origin.x + padding,
            y,
            title_size,
            theme::text::MUTED,
            cx.text,
            cx.scene,
        );
        y += title_size + 8.0;

        let Some(selected) = self.selected_history else {
            self.draw_text(
                "Select a transaction to see details.",
                bounds.origin.x + padding,
                y,
                theme::font_size::SM,
                theme::text::PRIMARY,
                cx.text,
                cx.scene,
            );
            return;
        };

        let Some(payment) = self.history_items.get(selected) else {
            return;
        };

        let detail_lines = [
            format!("ID: {}", truncate_middle(&payment.id, 12, 8)),
            format!("Type: {}", format_payment_type(payment.payment_type)),
            format!("Status: {}", format_payment_status(payment.status)),
            format!("Method: {}", format_payment_method(payment.method)),
            format!("Amount: {} sats", format_sats_u128(payment.amount)),
            format!("Fees: {} sats", format_sats_u128(payment.fees)),
            format!("Time: {}", format_timestamp_display(payment.timestamp)),
        ];

        for line in detail_lines {
            self.draw_text(
                &line,
                bounds.origin.x + padding,
                y,
                theme::font_size::SM,
                theme::text::PRIMARY,
                cx.text,
                cx.scene,
            );
            y += theme::font_size::SM + 6.0;
            if y > bounds.origin.y + bounds.size.height - padding {
                break;
            }
        }
    }

    fn handle_ui_events(&mut self) -> bool {
        let mut changed = false;
        let events: Vec<WalletUiEvent> = self.ui_events.borrow_mut().drain(..).collect();
        for event in events {
            changed = true;
            match event {
                WalletUiEvent::SelectTab(tab) => {
                    self.tab = tab;
                    self.notice = None;
                    if tab == WalletTab::History
                        && self.history_items.is_empty()
                        && !self.history_loading
                    {
                        self.request_history_page(0);
                    }
                }
                WalletUiEvent::SubmitSend => {
                    self.submit_send();
                }
                WalletUiEvent::GenerateReceive => {
                    self.generate_receive();
                }
            }
        }
        changed
    }

    fn submit_send(&mut self) {
        let destination = self.send_destination.get_value().trim().to_string();
        if destination.is_empty() {
            self.notice = Some(Notice {
                kind: NoticeKind::Error,
                message: "Enter a destination first.".to_string(),
            });
            return;
        }

        let amount = match parse_optional_amount(self.send_amount.get_value()) {
            Ok(amount) => amount,
            Err(message) => {
                self.notice = Some(Notice {
                    kind: NoticeKind::Error,
                    message,
                });
                return;
            }
        };

        if let Some(limit) = self.send_limit {
            match amount {
                Some(sats) if sats > limit => {
                    self.notice = Some(Notice {
                        kind: NoticeKind::Error,
                        message: format!(
                            "Amount exceeds configured limit ({} sats).",
                            format_sats(limit)
                        ),
                    });
                    return;
                }
                None => {
                    self.notice = Some(Notice {
                        kind: NoticeKind::Error,
                        message: "Amount required when a send limit is set.".to_string(),
                    });
                    return;
                }
                _ => {}
            }
        }

        if let Some(threshold) = self.large_send_confirm {
            if let Some(sats) = amount {
                if sats >= threshold {
                    if let Some(pending) = &self.pending_large_send {
                        if pending.amount == sats && pending.destination == destination {
                            self.pending_large_send = None;
                        } else {
                            self.pending_large_send = Some(PendingSend {
                                destination,
                                amount: sats,
                            });
                            self.notice = Some(Notice {
                                kind: NoticeKind::Info,
                                message: "Large payment detected. Click Send again to confirm."
                                    .to_string(),
                            });
                            return;
                        }
                    } else {
                        self.pending_large_send = Some(PendingSend {
                            destination,
                            amount: sats,
                        });
                        self.notice = Some(Notice {
                            kind: NoticeKind::Info,
                            message: "Large payment detected. Click Send again to confirm."
                                .to_string(),
                        });
                        return;
                    }
                } else {
                    self.pending_large_send = None;
                }
            } else {
                self.pending_large_send = None;
            }
        }

        self.notice = Some(Notice {
            kind: NoticeKind::Info,
            message: "Sending payment...".to_string(),
        });

        self.commands.push(WalletCommand::SendPayment {
            destination,
            amount,
        });
    }

    fn generate_receive(&mut self) {
        let amount = match parse_optional_amount(self.receive_amount.get_value()) {
            Ok(amount) => amount,
            Err(message) => {
                self.notice = Some(Notice {
                    kind: NoticeKind::Error,
                    message,
                });
                return;
            }
        };

        self.notice = Some(Notice {
            kind: NoticeKind::Info,
            message: "Preparing receive QR...".to_string(),
        });
        self.receive_payload = None;
        self.qr_matrix = None;
        self.commands.push(WalletCommand::RequestReceive { amount });
    }
}

impl Component for WalletView {
    fn paint(&mut self, bounds: Bounds, cx: &mut wgpui::PaintContext) {
        let layout = self.layout(bounds);

        cx.scene
            .draw_quad(Quad::new(bounds).with_background(theme::bg::APP));
        cx.scene
            .draw_quad(Quad::new(layout.header).with_background(theme::bg::SURFACE));
        cx.scene.draw_quad(
            Quad::new(layout.tabs)
                .with_background(theme::bg::APP)
                .with_border(theme::border::SUBTLE, 1.0),
        );
        cx.scene
            .draw_quad(Quad::new(layout.content).with_background(theme::bg::APP));

        self.draw_text(
            "OPENAGENTS WALLET",
            layout.header.origin.x + PADDING,
            layout.header.origin.y + 18.0,
            theme::font_size::SM,
            theme::text::MUTED,
            cx.text,
            cx.scene,
        );

        let balance_label = "BALANCE";
        let balance_label_size = theme::font_size::XS;
        let balance_label_width = text_width(balance_label, balance_label_size);
        let balance_label_x =
            layout.header.origin.x + layout.header.size.width - balance_label_width - PADDING;
        let balance_label_y = layout.header.origin.y + 22.0;
        self.draw_text(
            balance_label,
            balance_label_x,
            balance_label_y,
            balance_label_size,
            theme::text::MUTED,
            cx.text,
            cx.scene,
        );

        let balance_text = self.balance_text();
        let balance_font = 26.0;
        let balance_width = text_width(&balance_text, balance_font);
        let balance_x = layout.header.origin.x + layout.header.size.width - balance_width - PADDING;
        let balance_y = layout.header.origin.y + 40.0;
        self.draw_text(
            &balance_text,
            balance_x,
            balance_y,
            balance_font,
            theme::accent::PRIMARY,
            cx.text,
            cx.scene,
        );

        self.draw_tab("Send", WalletTab::Send, layout.send_tab, cx.text, cx.scene);
        self.draw_tab(
            "Receive",
            WalletTab::Receive,
            layout.receive_tab,
            cx.text,
            cx.scene,
        );
        self.draw_tab(
            "History",
            WalletTab::History,
            layout.history_tab,
            cx.text,
            cx.scene,
        );

        match self.tab {
            WalletTab::Send => {
                self.draw_text(
                    "Destination",
                    layout.send.destination_input.origin.x,
                    layout.send.destination_input.origin.y - theme::font_size::XS - 6.0,
                    theme::font_size::XS,
                    theme::text::MUTED,
                    cx.text,
                    cx.scene,
                );
                self.send_destination
                    .paint(layout.send.destination_input, cx);

                self.draw_text(
                    "Amount (sats)",
                    layout.send.amount_input.origin.x,
                    layout.send.amount_input.origin.y - theme::font_size::XS - 6.0,
                    theme::font_size::XS,
                    theme::text::MUTED,
                    cx.text,
                    cx.scene,
                );
                self.send_amount.paint(layout.send.amount_input, cx);
                self.send_button.paint(layout.send.submit_button, cx);
            }
            WalletTab::Receive => {
                self.draw_text(
                    "Amount (optional)",
                    layout.receive.amount_input.origin.x,
                    layout.receive.amount_input.origin.y - theme::font_size::XS - 6.0,
                    theme::font_size::XS,
                    theme::text::MUTED,
                    cx.text,
                    cx.scene,
                );
                self.receive_amount.paint(layout.receive.amount_input, cx);
                self.receive_button
                    .paint(layout.receive.generate_button, cx);
                self.draw_qr(layout.receive.qr, cx.text, cx.scene);

                if let Some(payload) = &self.receive_payload {
                    self.draw_text(
                        payload,
                        layout.receive.payload.origin.x,
                        layout.receive.payload.origin.y,
                        theme::font_size::XS,
                        theme::text::SECONDARY,
                        cx.text,
                        cx.scene,
                    );
                }
            }
            WalletTab::History => {
                self.draw_history(&layout.history, cx);
            }
        }

        if let Some(notice) = &self.notice {
            let notice_bounds = match self.tab {
                WalletTab::Send => layout.send.notice,
                WalletTab::Receive => layout.receive.notice,
                WalletTab::History => layout.history.details,
            };
            self.draw_notice(notice, notice_bounds, cx.text, cx.scene);
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        let layout = self.layout(bounds);
        let mut result = EventResult::Ignored;

        match event {
            InputEvent::MouseMove { x, y } => {
                let point = Point::new(*x, *y);
                let hovered = if layout.send_tab.contains(point) {
                    Some(WalletTab::Send)
                } else if layout.receive_tab.contains(point) {
                    Some(WalletTab::Receive)
                } else if layout.history_tab.contains(point) {
                    Some(WalletTab::History)
                } else {
                    None
                };
                if hovered != self.hovered_tab {
                    self.hovered_tab = hovered;
                    result = EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y, .. } => {
                if *button == MouseButton::Left {
                    let point = Point::new(*x, *y);
                    if layout.send_tab.contains(point) {
                        self.ui_events
                            .borrow_mut()
                            .push(WalletUiEvent::SelectTab(WalletTab::Send));
                        result = EventResult::Handled;
                    } else if layout.receive_tab.contains(point) {
                        self.ui_events
                            .borrow_mut()
                            .push(WalletUiEvent::SelectTab(WalletTab::Receive));
                        result = EventResult::Handled;
                    } else if layout.history_tab.contains(point) {
                        self.ui_events
                            .borrow_mut()
                            .push(WalletUiEvent::SelectTab(WalletTab::History));
                        result = EventResult::Handled;
                    } else if self.tab == WalletTab::History && layout.history.list.contains(point)
                    {
                        self.sync_history_scroll(layout.history.list);
                        let relative_y = point.y - layout.history.list.origin.y
                            + self.history_scroll.scroll_offset.y;
                        let index = (relative_y / HISTORY_ROW_HEIGHT).floor() as usize;
                        if index < self.history_items.len() {
                            self.selected_history = Some(index);
                            result = EventResult::Handled;
                        }
                    }
                }
            }
            InputEvent::Scroll { dx, dy } => {
                if self.tab == WalletTab::History {
                    self.sync_history_scroll(layout.history.list);
                    self.history_scroll.scroll_by(Point::new(*dx, *dy));
                    self.maybe_request_more_history(layout.history.list);
                    result = EventResult::Handled;
                }
            }
            _ => {}
        }

        match self.tab {
            WalletTab::Send => {
                result = result.or(self.send_destination.event(
                    event,
                    layout.send.destination_input,
                    cx,
                ));
                result = result.or(self.send_amount.event(event, layout.send.amount_input, cx));
                result = result.or(self.send_button.event(event, layout.send.submit_button, cx));
            }
            WalletTab::Receive => {
                result =
                    result.or(self
                        .receive_amount
                        .event(event, layout.receive.amount_input, cx));
                result =
                    result.or(self
                        .receive_button
                        .event(event, layout.receive.generate_button, cx));
            }
            WalletTab::History => {}
        }

        if self.handle_ui_events() {
            result = EventResult::Handled;
        }

        result
    }
}

fn parse_optional_amount(raw: &str) -> Result<Option<u64>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let cleaned = trimmed.replace('_', "").replace(',', "");
    let amount: u64 = cleaned
        .parse()
        .map_err(|_| "Amount must be a number.".to_string())?;
    if amount == 0 {
        return Err("Amount must be greater than zero.".to_string());
    }
    Ok(Some(amount))
}

fn text_width(text: &str, font_size: f32) -> f32 {
    text.chars().count() as f32 * font_size * 0.6
}

fn format_sats(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (idx, ch) in digits.chars().rev().enumerate() {
        if idx != 0 && idx % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}

fn load_security_limits() -> (Option<u64>, Option<u64>) {
    if cfg!(test) {
        return (None, None);
    }
    match LocalWalletConfig::load() {
        Ok(config) => (
            config.security.max_send_sats,
            config.security.confirm_large_sats,
        ),
        Err(_) => (None, None),
    }
}

fn format_sats_u128(value: u128) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (idx, ch) in digits.chars().rev().enumerate() {
        if idx != 0 && idx % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}

fn format_payment_status(status: PaymentStatus) -> &'static str {
    match status {
        PaymentStatus::Completed => "Done",
        PaymentStatus::Pending => "Pending",
        PaymentStatus::Failed => "Failed",
    }
}

fn format_payment_type(payment_type: PaymentType) -> &'static str {
    match payment_type {
        PaymentType::Send => "Sent",
        PaymentType::Receive => "Received",
    }
}

fn format_payment_method(method: PaymentMethod) -> &'static str {
    match method {
        PaymentMethod::Lightning => "Lightning",
        PaymentMethod::Spark => "Spark",
        PaymentMethod::Token => "Token",
        PaymentMethod::Deposit => "Deposit",
        PaymentMethod::Withdraw => "Withdraw",
        PaymentMethod::Unknown => "Unknown",
    }
}

fn format_timestamp_display(timestamp: u64) -> String {
    chrono::DateTime::from_timestamp(timestamp as i64, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "Unknown".to_string())
}

fn truncate_middle(value: &str, head: usize, tail: usize) -> String {
    if value.len() <= head + tail + 3 {
        return value.to_string();
    }
    let head_part = &value[..head];
    let tail_part = &value[value.len() - tail..];
    format!("{}...{}", head_part, tail_part)
}

fn compute_balance_series(payments: &[Payment], current_total: Option<u128>) -> Vec<BalancePoint> {
    let Some(current_total) = current_total else {
        return Vec::new();
    };

    if payments.is_empty() {
        return vec![BalancePoint {
            timestamp: 0,
            balance: current_total,
        }];
    }

    let mut sorted = payments.to_vec();
    sorted.sort_by_key(|payment| std::cmp::Reverse(payment.timestamp));

    let mut balance = current_total as i128;
    let mut points = Vec::with_capacity(sorted.len());

    for payment in &sorted {
        points.push(BalancePoint {
            timestamp: payment.timestamp,
            balance: balance.max(0) as u128,
        });

        let amount = payment.amount as i128;
        let fees = payment.fees as i128;
        match payment.payment_type {
            PaymentType::Send => {
                balance = balance.saturating_add(amount + fees);
            }
            PaymentType::Receive => {
                balance = balance.saturating_sub(amount);
            }
        }
    }

    points.reverse();
    points
}

fn downsample_points(points: &[BalancePoint], max_points: usize) -> Vec<BalancePoint> {
    if points.len() <= max_points {
        return points.to_vec();
    }

    let step = (points.len() as f32 / max_points as f32).ceil() as usize;
    let mut sampled = points.iter().step_by(step).cloned().collect::<Vec<_>>();
    if let Some(last) = points.last() {
        if sampled.last().map(|p| p.timestamp) != Some(last.timestamp) {
            sampled.push(last.clone());
        }
    }
    sampled
}

fn min_max_balance(points: &[BalancePoint]) -> (u128, u128) {
    let mut min = u128::MAX;
    let mut max = 0u128;
    for point in points {
        min = min.min(point.balance);
        max = max.max(point.balance);
    }
    if min == u128::MAX { (0, 0) } else { (min, max) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spark::{Payment, PaymentMethod, PaymentStatus, PaymentType};
    use wgpui::{InputEvent, Modifiers};

    fn click_at(view: &mut WalletView, bounds: Bounds, x: f32, y: f32) {
        let mut cx = EventContext::new();
        let down = InputEvent::MouseDown {
            button: MouseButton::Left,
            x,
            y,
            modifiers: Modifiers::default(),
        };
        let up = InputEvent::MouseUp {
            button: MouseButton::Left,
            x,
            y,
        };
        view.event(&down, bounds, &mut cx);
        view.event(&up, bounds, &mut cx);
    }

    fn sample_payment(id: &str, payment_type: PaymentType, timestamp: u64) -> Payment {
        Payment {
            id: id.to_string(),
            payment_type,
            status: PaymentStatus::Completed,
            amount: 12_345,
            fees: 12,
            timestamp,
            method: PaymentMethod::Lightning,
            details: None,
        }
    }

    #[test]
    fn test_balance_text_updates_from_backend() {
        let mut view = WalletView::new();
        let balance = Balance {
            spark_sats: 1200,
            lightning_sats: 300,
            onchain_sats: 0,
        };
        view.apply_update(WalletUpdate::Balance(balance));
        assert_eq!(view.balance_text(), "1,500 sats");
    }

    #[test]
    fn test_send_form_emits_command() {
        let mut view = WalletView::new();
        view.drain_commands();
        let bounds = Bounds::new(0.0, 0.0, 900.0, 700.0);
        view.send_destination.set_value("lnbc1send");
        view.send_amount.set_value("2500");
        let layout = view.layout(bounds);
        click_at(
            &mut view,
            bounds,
            layout.send.submit_button.origin.x + 5.0,
            layout.send.submit_button.origin.y + 5.0,
        );

        let commands = view.drain_commands();
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            WalletCommand::SendPayment {
                destination,
                amount,
            } => {
                assert_eq!(destination, "lnbc1send");
                assert_eq!(amount, &Some(2500));
            }
            _ => panic!("expected send command"),
        }
    }

    #[test]
    fn test_receive_flow_emits_command_and_qr() {
        let mut view = WalletView::new();
        view.drain_commands();
        let bounds = Bounds::new(0.0, 0.0, 900.0, 700.0);
        let layout = view.layout(bounds);
        click_at(
            &mut view,
            bounds,
            layout.receive_tab.origin.x + 5.0,
            layout.receive_tab.origin.y + 5.0,
        );
        view.receive_amount.set_value("1200");

        let layout = view.layout(bounds);
        click_at(
            &mut view,
            bounds,
            layout.receive.generate_button.origin.x + 5.0,
            layout.receive.generate_button.origin.y + 5.0,
        );

        let commands = view.drain_commands();
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            WalletCommand::RequestReceive { amount } => {
                assert_eq!(amount, &Some(1200));
            }
            _ => panic!("expected receive command"),
        }

        view.apply_update(WalletUpdate::ReceiveReady {
            payload: "spark1receive".to_string(),
            amount: Some(1200),
        });
        assert_eq!(view.receive_payload.as_deref(), Some("spark1receive"));
        assert!(view.qr_matrix.is_some());
    }

    #[test]
    fn test_history_scroll_requests_more() {
        let mut view = WalletView::new();
        view.drain_commands();
        let bounds = Bounds::new(0.0, 0.0, 900.0, 720.0);
        let layout = view.layout(bounds);
        click_at(
            &mut view,
            bounds,
            layout.history_tab.origin.x + 5.0,
            layout.history_tab.origin.y + 5.0,
        );

        let payments: Vec<Payment> = (0..HISTORY_PAGE_SIZE as usize)
            .map(|i| sample_payment(&format!("pay-{}", i), PaymentType::Receive, 1_700_000_000))
            .collect();
        view.apply_update(WalletUpdate::PaymentsLoaded {
            payments,
            offset: 0,
            has_more: true,
        });

        let mut cx = EventContext::new();
        let scroll = InputEvent::Scroll {
            dx: 0.0,
            dy: 10_000.0,
        };
        view.event(&scroll, bounds, &mut cx);

        let commands = view.drain_commands();
        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            WalletCommand::LoadPayments {
                offset,
                limit
            } if *offset == HISTORY_PAGE_SIZE && *limit == HISTORY_PAGE_SIZE
        )));
    }

    #[test]
    fn test_history_click_selects_payment() {
        let mut view = WalletView::new();
        view.drain_commands();
        let bounds = Bounds::new(0.0, 0.0, 900.0, 720.0);
        let layout = view.layout(bounds);
        click_at(
            &mut view,
            bounds,
            layout.history_tab.origin.x + 5.0,
            layout.history_tab.origin.y + 5.0,
        );

        let payments = vec![
            sample_payment("pay-1", PaymentType::Send, 1_700_000_000),
            sample_payment("pay-2", PaymentType::Receive, 1_700_000_100),
        ];
        view.apply_update(WalletUpdate::PaymentsLoaded {
            payments,
            offset: 0,
            has_more: false,
        });

        let layout = view.layout(bounds);
        click_at(
            &mut view,
            bounds,
            layout.history.list.origin.x + 5.0,
            layout.history.list.origin.y + 5.0,
        );
        assert_eq!(view.selected_history, Some(0));
    }

    #[test]
    fn test_history_chart_builds_from_balance_and_payments() {
        let mut view = WalletView::new();
        view.drain_commands();
        view.apply_update(WalletUpdate::Balance(Balance {
            spark_sats: 800,
            lightning_sats: 200,
            onchain_sats: 0,
        }));

        let payments = vec![
            sample_payment("send-1", PaymentType::Send, 30),
            sample_payment("recv-1", PaymentType::Receive, 10),
        ];
        view.apply_update(WalletUpdate::PaymentsLoaded {
            payments,
            offset: 0,
            has_more: false,
        });

        assert_eq!(view.history_chart.len(), 2);
        assert_eq!(view.history_chart[0].timestamp, 10);
        assert_eq!(view.history_chart[0].balance, 13_357);
        assert_eq!(view.history_chart[1].timestamp, 30);
        assert_eq!(view.history_chart[1].balance, 1_000);
    }

    #[test]
    fn test_downsample_points_keeps_last_point() {
        let points = (0..120)
            .map(|i| BalancePoint {
                timestamp: i,
                balance: i as u128,
            })
            .collect::<Vec<_>>();
        let sampled = downsample_points(&points, 50);
        assert!(sampled.len() <= 50 + 1);
        assert_eq!(sampled.last().map(|p| p.timestamp), Some(119));
    }

    #[test]
    fn test_send_limit_blocks_gui_payment() {
        let mut view = WalletView::new();
        view.drain_commands();
        view.send_limit = Some(1_000);
        view.send_destination.set_value("lnbc1limit");
        view.send_amount.set_value("2000");
        view.submit_send();

        assert!(matches!(
            view.notice,
            Some(Notice {
                kind: NoticeKind::Error,
                ..
            })
        ));
        assert!(view.drain_commands().is_empty());
    }

    #[test]
    fn test_large_send_requires_second_confirm() {
        let mut view = WalletView::new();
        view.drain_commands();
        view.large_send_confirm = Some(1_000);
        view.send_destination.set_value("lnbc1large");
        view.send_amount.set_value("2000");

        view.submit_send();
        assert!(view.pending_large_send.is_some());
        assert!(view.drain_commands().is_empty());

        view.submit_send();
        let commands = view.drain_commands();
        assert_eq!(commands.len(), 1);
    }
}
