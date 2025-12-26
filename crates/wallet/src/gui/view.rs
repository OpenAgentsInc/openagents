//! WGPUI wallet view.

use std::cell::RefCell;
use std::rc::Rc;

use qrcode::QrCode;
use spark::Balance;
use wgpui::components::{Button, Component, EventResult, TextInput};
use wgpui::{
    Bounds, EventContext, InputEvent, MouseButton, Point, Quad, Scene, TextSystem, Hsla, theme,
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
    send: SendLayout,
    receive: ReceiveLayout,
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
                send_events
                    .borrow_mut()
                    .push(WalletUiEvent::SubmitSend);
            });

        let receive_button = Button::new("Generate QR")
            .padding(18.0, 8.0)
            .on_click(move || {
                receive_events
                    .borrow_mut()
                    .push(WalletUiEvent::GenerateReceive);
            });

        let mut commands = Vec::new();
        commands.push(WalletCommand::RefreshBalance);
        commands.push(WalletCommand::RequestReceive { amount: None });

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
            commands,
            ui_events,
        }
    }

    pub fn apply_update(&mut self, update: WalletUpdate) {
        match update {
            WalletUpdate::Balance(balance) => {
                self.balance = balance;
                self.balance_loaded = true;
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
            }
            WalletUpdate::Error { message } => {
                self.notice = Some(Notice {
                    kind: NoticeKind::Error,
                    message,
                });
            }
        }
    }

    pub fn drain_commands(&mut self) -> Vec<WalletCommand> {
        std::mem::take(&mut self.commands)
    }

    fn layout(&self, bounds: Bounds) -> WalletLayout {
        let header = Bounds::new(bounds.origin.x, bounds.origin.y, bounds.size.width, HEADER_HEIGHT);
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

        WalletLayout {
            header,
            tabs,
            content,
            send_tab,
            receive_tab,
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

        scene.draw_quad(Quad::new(bounds).with_background(background).with_border(theme::border::DEFAULT, 1.0));

        let font_size = theme::font_size::SM;
        let text_width = text_width(label, font_size);
        let text_x = bounds.origin.x + (bounds.size.width - text_width) / 2.0;
        let text_y = bounds.origin.y + (bounds.size.height - font_size) / 2.0;
        self.draw_text(label, text_x, text_y, font_size, text_color, text_system, scene);
    }

    fn draw_notice(&self, notice: &Notice, bounds: Bounds, text_system: &mut TextSystem, scene: &mut Scene) {
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

        scene.draw_quad(Quad::new(Bounds::new(origin_x, origin_y, total_size, total_size)).with_background(background));

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
                    scene.draw_quad(Quad::new(Bounds::new(px, py, module_size, module_size)).with_background(foreground));
                }
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

        self.notice = Some(Notice {
            kind: NoticeKind::Info,
            message: "Sending payment...".to_string(),
        });

        self.commands.push(WalletCommand::SendPayment { destination, amount });
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

        cx.scene.draw_quad(Quad::new(bounds).with_background(theme::bg::APP));
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
        let balance_label_x = layout.header.origin.x + layout.header.size.width - balance_label_width - PADDING;
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
        self.draw_tab("Receive", WalletTab::Receive, layout.receive_tab, cx.text, cx.scene);

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
                self.send_destination.paint(layout.send.destination_input, cx);

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
                self.receive_button.paint(layout.receive.generate_button, cx);
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
        }

        if let Some(notice) = &self.notice {
            let notice_bounds = match self.tab {
                WalletTab::Send => layout.send.notice,
                WalletTab::Receive => layout.receive.notice,
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
                } else {
                    None
                };
                if hovered != self.hovered_tab {
                    self.hovered_tab = hovered;
                    result = EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y } => {
                if *button == MouseButton::Left {
                    let point = Point::new(*x, *y);
                    if layout.send_tab.contains(point) {
                        self.ui_events.borrow_mut().push(WalletUiEvent::SelectTab(WalletTab::Send));
                        result = EventResult::Handled;
                    } else if layout.receive_tab.contains(point) {
                        self.ui_events
                            .borrow_mut()
                            .push(WalletUiEvent::SelectTab(WalletTab::Receive));
                        result = EventResult::Handled;
                    }
                }
            }
            _ => {}
        }

        match self.tab {
            WalletTab::Send => {
                result = result.or(self.send_destination.event(event, layout.send.destination_input, cx));
                result = result.or(self.send_amount.event(event, layout.send.amount_input, cx));
                result = result.or(self.send_button.event(event, layout.send.submit_button, cx));
            }
            WalletTab::Receive => {
                result = result.or(self.receive_amount.event(event, layout.receive.amount_input, cx));
                result = result.or(self.receive_button.event(event, layout.receive.generate_button, cx));
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use wgpui::InputEvent;

    fn click_at(view: &mut WalletView, bounds: Bounds, x: f32, y: f32) {
        let mut cx = EventContext::new();
        let down = InputEvent::MouseDown {
            button: MouseButton::Left,
            x,
            y,
        };
        let up = InputEvent::MouseUp {
            button: MouseButton::Left,
            x,
            y,
        };
        view.event(&down, bounds, &mut cx);
        view.event(&up, bounds, &mut cx);
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
        click_at(&mut view, bounds, layout.send.submit_button.origin.x + 5.0, layout.send.submit_button.origin.y + 5.0);

        let commands = view.drain_commands();
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            WalletCommand::SendPayment { destination, amount } => {
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
        click_at(&mut view, bounds, layout.receive_tab.origin.x + 5.0, layout.receive_tab.origin.y + 5.0);
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
}
