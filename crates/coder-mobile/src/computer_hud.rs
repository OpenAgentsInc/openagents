//! The world computer's screen, drawn in the Verse HUD.
//!
//! Tapping the physical monitor opens this panel over the world. It draws
//! the Computers surface (linked hosts with presence and route, adding a
//! host, a host's screen, ordering work, and Activity) and the terminal
//! screen from their Rust Native views, in the amber ladder, with the same
//! atlas and batch as the map and zone controls. Rust lays out, draws, and
//! hit-tests every control. Native code only forwards the views from the
//! reader worker, answers the few commands a tree cannot (open the camera
//! scanner or the keyboard for an input request), and exposes the laid-out
//! controls to accessibility.
//!
//! A tap never carries an intent. It names a node of the current view, and
//! the Computers controller resolves it against its own current revision
//! and checks authority again, exactly as a native tap does.
use coder_ui::theme::Intensity;
use rust_native::{Axis, Element, Node, TextRole, View};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use verse::ui::{Atlas, UiBatch, amber, field};

/// The panel's pages. Chats is the older read-only reader and pairing,
/// drawn by the native host.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Page {
    #[default]
    Computers,
    Terminal,
    Chats,
}

/// Which Rust Native surface a tap addresses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Surface {
    Computers,
    Terminal,
}

/// A value the Computers surface asks the native host to collect.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FeedInput {
    pub token: String,
    pub purpose: String,
    pub label: String,
    pub prompt: String,
    pub scan: bool,
    #[serde(default)]
    pub secret: bool,
    pub max_bytes: usize,
}

/// An invitation QR code: one string of `1` (dark) and `0` (light) per row.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FeedQr {
    pub size: usize,
    pub rows: Vec<String>,
}

/// What the reader worker last reported, forwarded by the native host.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Feed {
    #[serde(default)]
    pub computers: Option<Value>,
    #[serde(default)]
    pub computers_input: Option<FeedInput>,
    #[serde(default)]
    pub computers_qr: Option<FeedQr>,
    #[serde(default)]
    pub terminal: Option<Value>,
    /// The worker is running a request.
    #[serde(default)]
    pub busy: bool,
    /// A native or worker failure to show.
    #[serde(default)]
    pub error: Option<String>,
}

/// A checked feed: views validated and bounded.
#[derive(Clone, Debug, Default)]
struct Checked {
    computers: Option<View<Value>>,
    input: Option<FeedInput>,
    qr: Option<Vec<Vec<bool>>>,
    terminal: Option<View<Value>>,
    busy: bool,
    error: Option<String>,
}

fn view(value: Option<Value>) -> Result<Option<View<Value>>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    let view: View<Value> =
        serde_json::from_value(value).map_err(|_| "The computer screen is invalid.".to_owned())?;
    view.validate()
        .map(|valid| Some(valid.view().clone()))
        .map_err(|error| format!("The computer screen is invalid: {error}"))
}

impl Feed {
    fn check(self) -> Result<Checked, String> {
        let qr = match self.computers_qr {
            None => None,
            Some(qr) => {
                if qr.size == 0 || qr.size > 200 || qr.rows.len() != qr.size {
                    return Err("The invitation code is invalid.".into());
                }
                let mut modules = Vec::with_capacity(qr.size);
                for row in &qr.rows {
                    if row.len() != qr.size || row.bytes().any(|b| b != b'0' && b != b'1') {
                        return Err("The invitation code is invalid.".into());
                    }
                    modules.push(row.bytes().map(|b| b == b'1').collect());
                }
                Some(modules)
            }
        };
        if let Some(input) = &self.computers_input
            && (input.token.is_empty()
                || input.token.len() > 256
                || input.label.len() > 1024
                || input.prompt.len() > 4096)
        {
            return Err("The input request is invalid.".into());
        }
        Ok(Checked {
            computers: view(self.computers)?,
            input: self.computers_input,
            qr,
            terminal: view(self.terminal)?,
            busy: self.busy,
            error: self.error.map(|e| e.chars().take(400).collect()),
        })
    }
}

/// What the native host must do for the panel.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum Command {
    /// Forward a tap to the reader worker's surface.
    Activate {
        surface: Surface,
        instance: String,
        revision: u64,
        node: String,
    },
    /// Open the camera scanner for this input request.
    Scan {
        token: String,
    },
    /// Show the keyboard for this input request.
    Type {
        token: String,
    },
    /// Close this input request without a value.
    CancelInput {
        token: String,
    },
    /// Ask the reader worker for a fresh Computers view.
    Refresh,
    /// The terminal page fits this grid: send it with `terminal_resize`.
    TerminalResize {
        rows: u16,
        cols: u16,
    },
    /// Show the keyboard that types into the terminal.
    TerminalKeyboard,
    Copy {
        text: String,
    },
}

/// What a laid-out control does.
#[derive(Clone, Debug, PartialEq)]
enum Action {
    Close,
    Page(Page),
    Activate(Surface, String),
    Scan,
    Type,
    CancelInput,
    Keyboard,
    Select,
    Copy,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Text(Intensity),
    /// One run of a terminal grid row: one unwrapped monospaced line in
    /// the view's own colors (linear RGBA), with an optional fill.
    Run {
        fg: [f32; 4],
        bg: Option<[f32; 4]>,
    },
    Button {
        enabled: bool,
    },
    Qr,
}

/// One laid-out element in content coordinates (body) or screen
/// coordinates (chrome).
#[derive(Clone, Debug)]
struct Laid {
    key: String,
    label: String,
    kind: Kind,
    lines: Vec<String>,
    rect: [f32; 4],
    action: Option<Action>,
}

/// A control or text line the native host exposes to accessibility, in
/// logical points from the top-left of the surface.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct Item {
    pub key: String,
    pub label: String,
    /// `button`, `heading`, or `text`.
    pub role: &'static str,
    pub enabled: bool,
    pub frame: [f32; 4],
}

/// The panel as the native host sees it.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub(crate) struct Snapshot {
    pub visible: bool,
    pub page: Page,
    pub frame: [f32; 4],
    pub body: [f32; 4],
    pub items: Vec<Item>,
    pub input: Option<FeedInput>,
    pub busy: bool,
    pub scroll: f32,
    pub max_scroll: f32,
    pub captured_pointers: Vec<u64>,
}

struct Contact {
    id: u64,
    origin: [f32; 2],
    last: [f32; 2],
    valid: bool,
    scrolling: bool,
}

const PAD: f32 = 10.0;
const GAP: f32 = 6.0;
const DRIFT: f32 = 10.0;
const MAX_ITEMS: usize = 160;

/// The panel's state for one Verse mount.
pub(crate) struct ComputerHud {
    /// The platform draws the panel in the world. Hosts that keep their
    /// native panel leave it off.
    enabled: bool,
    glyphs: terminal_gfx::glyphs::Fallback,
    glyph_cursor: usize,
    page: Page,
    feed: Checked,
    feed_error: Option<String>,
    insets: [f32; 4],
    scroll: f32,
    selecting: bool,
    selection: Option<(f32, f32)>,
    pan: f32,
    screen: Option<String>,
    notice: Option<String>,
    terminal_instance: Option<String>,
    /// The grid last sent for the open terminal.
    terminal_size: Option<(u16, u16)>,
    /// How far the software keyboard covers the bottom, in points.
    keyboard: f32,
    contacts: Vec<Contact>,
    outbox: Vec<Command>,
    /// A tap was forwarded and no newer feed has arrived.
    pending: bool,
}

impl ComputerHud {
    pub(crate) fn new(enabled: bool) -> Self {
        Self {
            enabled,
            glyphs: terminal_gfx::glyphs::Fallback::new(),
            glyph_cursor: 0,
            page: Page::Computers,
            feed: Checked::default(),
            feed_error: None,
            insets: [0.0; 4],
            scroll: 0.0,
            selecting: false,
            selection: None,
            pan: 0.0,
            screen: None,
            notice: None,
            terminal_instance: None,
            terminal_size: None,
            keyboard: 0.0,
            contacts: Vec::new(),
            outbox: Vec::new(),
            pending: false,
        }
    }

    /// Whether Rust draws and handles this page.
    pub(crate) fn drawn(&self) -> bool {
        self.enabled && self.page != Page::Chats
    }

    pub(crate) fn page(&self) -> Page {
        self.page
    }

    /// The panel opened at the monitor: start on Computers and ask for a
    /// fresh view.
    pub(crate) fn open(&mut self) {
        self.page = Page::Computers;
        self.scroll = 0.0;
        self.contacts.clear();
        if self.enabled {
            self.outbox.push(Command::Refresh);
        }
    }

    pub(crate) fn close(&mut self) {
        self.contacts.clear();
        self.selection = None;
        self.selecting = false;
        self.page = Page::Computers;
    }

    pub(crate) fn set_page(&mut self, page: Page) -> Result<(), String> {
        if !self.enabled {
            return Err("This app draws the computer natively".into());
        }
        if page == Page::Terminal && self.feed.terminal.is_none() {
            return Err("No terminal is open".into());
        }
        self.contacts.clear();
        self.selection = None;
        self.selecting = false;
        self.scroll = 0.0;
        if page == Page::Computers && self.page != Page::Computers {
            self.outbox.push(Command::Refresh);
        }
        self.page = page;
        Ok(())
    }

    pub(crate) fn set_insets(&mut self, insets: [f32; 4]) {
        if self.insets != insets {
            self.contacts.clear();
            self.insets = insets;
        }
    }

    /// The software keyboard covers `bottom` points of the surface. The
    /// terminal page, which the keyboard types into, keeps above it; other
    /// pages answer input natively and ignore it.
    pub(crate) fn set_keyboard(&mut self, bottom: f32) -> Result<(), String> {
        if !bottom.is_finite() || !(0.0..=4096.0).contains(&bottom) {
            return Err("The keyboard height is out of bounds".into());
        }
        if self.keyboard != bottom {
            self.contacts.clear();
            self.keyboard = bottom;
        }
        Ok(())
    }

    pub(crate) fn clear_contacts(&mut self) {
        self.contacts.clear();
    }

    pub(crate) fn take_commands(&mut self) -> Vec<Command> {
        std::mem::take(&mut self.outbox)
    }

    /// While the Terminal page shows, ask for the grid its body fits: the
    /// columns of the body's width and the rows left after the terminal's
    /// header and key row. Sent once per change.
    pub(crate) fn sync_terminal(&mut self, atlas: &Atlas, size: [f32; 2]) {
        if !self.drawn() || self.page != Page::Terminal || self.feed.terminal.is_none() {
            return;
        }
        let (_, items, body, height) = self.layout(atlas, size);
        let grid = items
            .iter()
            .filter(|laid| matches!(laid.kind, Kind::Run { .. }))
            .map(|laid| laid.rect[1])
            .fold(None::<(f32, f32)>, |range, top| match range {
                None => Some((top, top + atlas.line)),
                Some((low, high)) => Some((low.min(top), high.max(top + atlas.line))),
            });
        let grid_height = grid.map_or(0.0, |(low, high)| high - low);
        let chrome = (height - grid_height).max(0.0);
        let rows = ((body[3] - chrome) / atlas.line).floor().clamp(2.0, 80.0) as u16;
        let cols = (body[2] / atlas.advance).floor().clamp(10.0, 240.0) as u16;
        if self.terminal_size != Some((rows, cols)) {
            self.terminal_size = Some((rows, cols));
            self.outbox.push(Command::TerminalResize { rows, cols });
        }
    }

    /// Accept the reader worker's latest views. A new terminal view opens
    /// the Terminal page; a closed one returns to Computers.
    pub(crate) fn feed(&mut self, feed: Feed) -> Result<(), String> {
        self.selection = None;
        let checked = match feed.check() {
            Ok(checked) => checked,
            Err(error) => {
                self.feed_error = Some(error.clone());
                return Err(error);
            }
        };
        self.feed_error = None;
        self.pending = false;
        let terminal = checked.terminal.as_ref().map(|view| view.instance.clone());
        if terminal.is_some() && terminal != self.terminal_instance {
            if self.page != Page::Chats {
                self.page = Page::Terminal;
                self.scroll = 0.0;
            }
        } else if terminal.is_none() && self.page == Page::Terminal {
            self.page = Page::Computers;
            self.scroll = 0.0;
        }
        if terminal != self.terminal_instance {
            self.pan = 0.0;
            self.selecting = false;
            self.terminal_size = None;
        }
        self.terminal_instance = terminal;
        let screen = checked
            .computers
            .as_ref()
            .and_then(|view| heading(&view.root));
        // A new screen, or a new result notice, starts at the top, where the
        // notice is.
        let notice = checked
            .computers
            .as_ref()
            .and_then(|view| text_of(&view.root, "notice"));
        if (screen != self.screen || (notice.is_some() && notice != self.notice))
            && self.page == Page::Computers
        {
            self.scroll = 0.0;
        }
        self.screen = screen;
        self.notice = notice;
        self.feed = checked;
        Ok(())
    }

    fn surface(&self) -> Option<(Surface, &View<Value>)> {
        match self.page {
            Page::Computers => self
                .feed
                .computers
                .as_ref()
                .map(|v| (Surface::Computers, v)),
            Page::Terminal => self.feed.terminal.as_ref().map(|v| (Surface::Terminal, v)),
            Page::Chats => None,
        }
    }

    fn frame(&self, size: [f32; 2]) -> [f32; 4] {
        let [top, right, mut bottom, left] = self.insets;
        if self.page == Page::Terminal {
            bottom = bottom.max(self.keyboard);
        }
        let x = left + 8.0;
        let w = (size[0] - left - right - 16.0).max(1.0);
        // The upper band keeps the monitor and the world in view.
        let y = top + 8.0 + (size[1] * 0.08).min(72.0);
        let h = (size[1] - y - bottom - 8.0).max(1.0);
        [x, y, w, h]
    }

    /// Lay out the chrome (screen coordinates), the body (content
    /// coordinates), and the body's visible region.
    fn layout(&self, atlas: &Atlas, size: [f32; 2]) -> (Vec<Laid>, Vec<Laid>, [f32; 4], f32) {
        let [x, y, w, h] = self.frame(size);
        let button_h = atlas.line + 14.0;
        let mut chrome = Vec::new();
        // Header: page tabs on the left, Close on the right.
        let mut tabs = vec![("hud-tab-computers", "COMPUTERS", Page::Computers)];
        if self.feed.terminal.is_some() {
            tabs.push(("hud-tab-terminal", "TERMINAL", Page::Terminal));
        }
        tabs.push(("hud-tab-chats", "CHATS", Page::Chats));
        let mut cx = x + PAD;
        for (key, label, page) in tabs {
            let bw = atlas.measure(label) + 16.0;
            let selected = self.page == page;
            chrome.push(Laid {
                key: key.into(),
                label: if selected {
                    format!("{label} (current)")
                } else {
                    label.into()
                },
                kind: Kind::Button { enabled: true },
                lines: vec![label.into()],
                rect: [cx, y + PAD, bw, button_h],
                action: Some(Action::Page(page)),
            });
            cx += bw + GAP;
        }
        if self.page == Page::Terminal && self.feed.terminal.is_some() {
            let bw = atlas.measure("KEYBOARD") + 16.0;
            chrome.push(Laid {
                key: "hud-terminal-keyboard".into(),
                label: "Keyboard".into(),
                kind: Kind::Button { enabled: true },
                lines: vec!["KEYBOARD".into()],
                rect: [cx, y + PAD, bw, button_h],
                action: Some(Action::Keyboard),
            });
            cx += bw + GAP;
        }
        let close_w = atlas.measure("CLOSE") + 16.0;
        chrome.push(Laid {
            key: "hud-close".into(),
            label: "Close computer".into(),
            kind: Kind::Button { enabled: true },
            lines: vec!["CLOSE".into()],
            rect: [(x + w - PAD - close_w).max(cx), y + PAD, close_w, button_h],
            action: Some(Action::Close),
        });
        let mut top = y + PAD + button_h + GAP;
        if self.page == Page::Terminal {
            for (index, (key, label, action)) in [
                (
                    "hud-terminal-select",
                    if self.selecting { "SCROLL" } else { "SELECT" },
                    Action::Select,
                ),
                ("hud-terminal-copy", "COPY", Action::Copy),
            ]
            .into_iter()
            .enumerate()
            {
                chrome.push(Laid {
                    key: key.into(),
                    label: label.into(),
                    kind: Kind::Button {
                        enabled: !matches!(action, Action::Copy) || self.selection.is_some(),
                    },
                    lines: vec![label.into()],
                    rect: [x + PAD + index as f32 * 84.0, top, 78.0, button_h],
                    action: Some(action),
                });
            }
            top += button_h + GAP;
        }

        let status = self
            .feed_error
            .clone()
            .or_else(|| self.feed.error.clone())
            .or_else(|| (self.pending || self.feed.busy).then(|| "Working…".to_owned()));
        if let Some(status) = status {
            let lines: Vec<String> = atlas
                .wrap(&status, w - 2.0 * PAD)
                .into_iter()
                .take(2)
                .collect();
            let lh = lines.len() as f32 * atlas.line;
            chrome.push(Laid {
                key: "hud-status".into(),
                label: status,
                kind: Kind::Text(Intensity::ThreeQuarters),
                lines,
                rect: [x + PAD, top, w - 2.0 * PAD, lh],
                action: None,
            });
            top += lh + GAP;
        }
        // Footer: the current input request, answered natively.
        let mut bottom = y + h - PAD;
        if let Some(input) = self
            .feed
            .input
            .as_ref()
            .filter(|_| self.page == Page::Computers)
        {
            let mut buttons = Vec::new();
            if input.scan {
                buttons.push(("hud-input-scan", "SCAN QR", "Scan QR code", Action::Scan));
            }
            buttons.push((
                "hud-input-type",
                if input.purpose == "invitation" {
                    "PASTE"
                } else {
                    "TYPE"
                },
                if input.purpose == "invitation" {
                    "Paste invitation"
                } else {
                    "Type"
                },
                Action::Type,
            ));
            buttons.push(("hud-input-cancel", "CANCEL", "Cancel", Action::CancelInput));
            let by = bottom - button_h;
            let mut bx = x + PAD;
            for (key, text, label, action) in buttons {
                let bw = atlas.measure(text) + 16.0;
                chrome.push(Laid {
                    key: key.into(),
                    label: label.into(),
                    kind: Kind::Button { enabled: true },
                    lines: vec![text.into()],
                    rect: [bx, by, bw, button_h],
                    action: Some(action),
                });
                bx += bw + GAP;
            }
            let prompt = format!("{}: {}", input.label, input.prompt);
            let lines: Vec<String> = atlas
                .wrap(&prompt, w - 2.0 * PAD)
                .into_iter()
                .take(3)
                .collect();
            let lh = lines.len() as f32 * atlas.line;
            chrome.push(Laid {
                key: "hud-input-prompt".into(),
                label: prompt,
                kind: Kind::Text(Intensity::Full),
                lines,
                rect: [x + PAD, by - GAP - lh, w - 2.0 * PAD, lh],
                action: None,
            });
            bottom = by - 2.0 * GAP - lh;
        }
        let body = [x + PAD, top, w - 2.0 * PAD, (bottom - top).max(0.0)];
        let mut content = Flow::new(atlas, body[2]);
        content.pan = self.pan.floor() as usize;
        if self.page == Page::Computers
            && let Some(qr) = &self.feed.qr
        {
            let side = body[2].min(220.0);
            content.items.push(Laid {
                key: "computers-qr".into(),
                label: "Invitation QR code".into(),
                kind: Kind::Qr,
                lines: Vec::new(),
                rect: [0.0, 0.0, side, side],
                action: None,
            });
            content.y = side + GAP;
            let _ = qr;
        }
        match self.surface() {
            Some((surface, view)) => content.block(&view.root, surface),
            None => content.text(
                "hud-empty",
                if self.page == Page::Terminal {
                    "No terminal is open."
                } else {
                    "Computers are loading…"
                },
                Intensity::Half,
            ),
        }
        let height = content.y;
        (chrome, content.items, body, height)
    }

    fn max_scroll(&self, atlas: &Atlas, size: [f32; 2]) -> f32 {
        let (_, _, body, height) = self.layout(atlas, size);
        (height - body[3]).max(0.0)
    }

    /// Where the panel and its visible controls are.
    pub(crate) fn snapshot(&self, atlas: &Atlas, size: [f32; 2], open: bool) -> Snapshot {
        if !open || !self.drawn() {
            return Snapshot {
                page: self.page,
                ..Snapshot::default()
            };
        }
        let (chrome, body_items, body, height) = self.layout(atlas, size);
        let max_scroll = (height - body[3]).max(0.0);
        let scroll = self.scroll.min(max_scroll);
        let mut items: Vec<Item> = chrome.iter().map(|laid| item(laid, laid.rect)).collect();
        for laid in &body_items {
            let [lx, ly, lw, lh] = laid.rect;
            let top = body[1] + ly - scroll;
            // Only what is fully in view: a control half under the header
            // would take taps meant for the header.
            if top >= body[1] - 0.5 && top + lh <= body[1] + body[3] + 0.5 {
                items.push(item(laid, [body[0] + lx, top, lw, lh]));
            }
            if items.len() >= MAX_ITEMS {
                break;
            }
        }
        Snapshot {
            visible: true,
            page: self.page,
            frame: self.frame(size),
            body,
            items,
            input: self.feed.input.clone(),
            busy: self.pending || self.feed.busy,
            scroll,
            max_scroll,
            captured_pointers: self.contacts.iter().map(|c| c.id).collect(),
        }
    }

    /// Capture a contact that lands anywhere while the panel is drawn. The
    /// world behind it never sees the touch.
    pub(crate) fn down(&mut self, id: u64, at: [f32; 2]) {
        if self.contacts.len() >= 8 || self.contacts.iter().any(|c| c.id == id) {
            return;
        }
        let first = self.contacts.is_empty();
        self.contacts.push(Contact {
            id,
            origin: at,
            last: at,
            valid: first,
            scrolling: false,
        });
        if !first {
            for contact in &mut self.contacts {
                contact.valid = false;
            }
        }
    }

    pub(crate) fn captured(&self, id: u64) -> bool {
        self.contacts.iter().any(|c| c.id == id)
    }

    /// A drag scrolls the body; it never becomes a tap.
    pub(crate) fn moved(&mut self, atlas: &Atlas, size: [f32; 2], id: u64, at: [f32; 2]) {
        let max = self.max_scroll(atlas, size);
        let (_, _, body, _) = self.layout(atlas, size);
        let Some(contact) = self.contacts.iter_mut().find(|c| c.id == id) else {
            return;
        };
        if (at[0] - contact.origin[0]).hypot(at[1] - contact.origin[1]) > DRIFT {
            contact.valid = false;
            contact.scrolling = true;
        }
        if contact.scrolling {
            if self.page == Page::Terminal && self.selecting && inside(body, contact.origin) {
                self.selection = Some((
                    contact.origin[1] - body[1] + self.scroll,
                    at[1] - body[1] + self.scroll,
                ));
            } else if self.page == Page::Terminal
                && (at[0] - contact.origin[0]).abs() > (at[1] - contact.origin[1]).abs()
            {
                self.pan = (self.pan - (at[0] - contact.last[0]) / atlas.advance).clamp(0.0, 240.0);
            } else {
                self.scroll = (self.scroll - (at[1] - contact.last[1])).clamp(0.0, max);
            }
        }
        contact.last = at;
    }

    /// End a contact; a tap on a control acts. Returns `true` when the tap
    /// closes the computer.
    pub(crate) fn up(
        &mut self,
        atlas: &Atlas,
        size: [f32; 2],
        id: u64,
        at: [f32; 2],
        cancelled: bool,
    ) -> bool {
        self.moved(atlas, size, id, at);
        let Some(index) = self.contacts.iter().position(|c| c.id == id) else {
            return false;
        };
        let contact = self.contacts.remove(index);
        if cancelled || !contact.valid {
            return false;
        }
        let snapshot = self.snapshot(atlas, size, true);
        let Some(key) = snapshot
            .items
            .iter()
            .find(|item| item.role == "button" && inside(item.frame, at))
            .map(|item| item.key.clone())
        else {
            return false;
        };
        self.act(atlas, size, &key) == Some(true)
    }

    /// Scroll by `delta` points, for accessibility.
    pub(crate) fn scroll_by(&mut self, atlas: &Atlas, size: [f32; 2], delta: f32) {
        if delta.is_finite() {
            let max = self.max_scroll(atlas, size);
            self.scroll = (self.scroll + delta.clamp(-4096.0, 4096.0)).clamp(0.0, max);
        }
    }

    /// Run the enabled control `key` names on the current layout. Returns
    /// `None` when no such control is laid out, and `Some(true)` when it
    /// closes the computer.
    pub(crate) fn act(&mut self, atlas: &Atlas, size: [f32; 2], key: &str) -> Option<bool> {
        let (chrome, body, _, _) = self.layout(atlas, size);
        let action = chrome
            .into_iter()
            .chain(body)
            .find(|laid| laid.key == key)
            .and_then(|laid| match laid.kind {
                Kind::Button { enabled: true } => laid.action,
                _ => None,
            })?;
        let token = || self.feed.input.as_ref().map(|input| input.token.clone());
        match action {
            Action::Close => return Some(true),
            Action::Page(page) => {
                let _ = self.set_page(page);
            }
            Action::Activate(surface, node) => {
                let view = match surface {
                    Surface::Computers => self.feed.computers.as_ref(),
                    Surface::Terminal => self.feed.terminal.as_ref(),
                }?;
                self.outbox.push(Command::Activate {
                    surface,
                    instance: view.instance.clone(),
                    revision: view.revision,
                    node,
                });
                self.pending = true;
            }
            Action::Scan => self.outbox.push(Command::Scan { token: token()? }),
            Action::Type => self.outbox.push(Command::Type { token: token()? }),
            Action::CancelInput => {
                self.outbox.push(Command::CancelInput { token: token()? });
                self.pending = true;
            }
            Action::Keyboard => self.outbox.push(Command::TerminalKeyboard),
            Action::Select => {
                self.selecting = !self.selecting;
                self.selection = None;
            }
            Action::Copy => {
                if let Some((a, b)) = self.selection {
                    let (_, items, _, _) = self.layout(atlas, size);
                    let mut text = String::new();
                    let mut last_y = None;
                    for item in items.iter().filter(|item| {
                        matches!(item.kind, Kind::Run { .. })
                            && item.rect[1] + atlas.line > a.min(b)
                            && item.rect[1] <= a.max(b)
                    }) {
                        if last_y.is_some_and(|y| y != item.rect[1]) {
                            text.push('\n');
                        }
                        text.push_str(item.lines.first().map_or("", String::as_str));
                        last_y = Some(item.rect[1]);
                    }
                    self.outbox.push(Command::Copy { text });
                }
            }
        }
        Some(false)
    }

    /// Prepare at most one bounded viewport of missing host glyphs per frame.
    pub(crate) fn prepare_glyphs(&mut self, atlas: &mut Atlas, size: [f32; 2]) {
        if self.page != Page::Terminal || !self.drawn() {
            return;
        }
        let (_, items, _, _) = self.layout(atlas, size);
        let characters = items
            .iter()
            .flat_map(|item| item.lines.iter())
            .flat_map(|line| line.chars());
        let count = characters.clone().count();
        let budget = count.min(4096);
        for ch in characters
            .cycle()
            .skip(self.glyph_cursor % count.max(1))
            .take(budget)
        {
            if !self.glyphs.ensure(atlas, ch)
                && ch > '\u{7f}'
                && terminal_gfx::draw::box_lines(ch).is_none()
            {
                self.notice =
                    Some("Some characters may not display correctly on this device.".into());
            }
        }
        self.glyph_cursor = (self.glyph_cursor + budget) % count.max(1);
    }

    /// Draw the panel in logical points, scaled to the surface.
    pub(crate) fn draw(
        &self,
        atlas: &Atlas,
        size: [f32; 2],
        scale: f32,
        anchor: Option<[f32; 2]>,
    ) -> UiBatch {
        let mut ui = UiBatch::default();
        if !self.drawn() {
            return ui;
        }
        let [x, y, w, h] = self.frame(size);
        let full = amber(Intensity::Full, 1.0);
        let half = amber(Intensity::Half, 1.0);
        // A leader from the monitor keeps the panel attached to the world.
        if let Some([ax, ay]) = anchor
            && ay < y
        {
            let lx = ax.clamp(x + 12.0, x + w - 12.0);
            ui.rect(atlas, ax - 3.0, ay - 3.0, 6.0, 6.0, full);
            ui.rect(atlas, lx - 1.0, ay, 2.0, (y - ay).max(0.0), half);
            ui.rect(
                atlas,
                ax.min(lx),
                ay - 1.0,
                (ax - lx).abs() + 1.0,
                2.0,
                half,
            );
        }
        ui.rect(atlas, x, y, w, h, field(0.94));
        ui.frame(atlas, x, y, w, h, 1.0, full);
        let (chrome, body_items, body, height) = self.layout(atlas, size);
        let scroll = self.scroll.min((height - body[3]).max(0.0));
        for laid in &chrome {
            draw_laid(
                &mut ui,
                atlas,
                laid,
                [laid.rect[0], laid.rect[1]],
                None,
                self,
            );
        }
        ui.rect(
            atlas,
            body[0],
            body[1] - 3.0,
            body[2],
            1.0,
            amber(Intensity::Quarter, 1.0),
        );
        let clip = [body[1], body[1] + body[3]];
        for laid in &body_items {
            let origin = [body[0] + laid.rect[0], body[1] + laid.rect[1] - scroll];
            if origin[1] + laid.rect[3] < clip[0] || origin[1] > clip[1] {
                continue;
            }
            draw_laid(&mut ui, atlas, laid, origin, Some(clip), self);
        }
        if height > body[3] {
            // A scroll bar says there is more.
            let track = body[3];
            let thumb = (track * body[3] / height).max(16.0);
            let at = (track - thumb) * (scroll / (height - body[3]).max(1.0));
            ui.rect(
                atlas,
                x + w - 4.0,
                body[1] + at,
                2.0,
                thumb,
                amber(Intensity::Half, 0.8),
            );
        }
        for v in &mut ui.vertices {
            v.pos[0] *= scale;
            v.pos[1] *= scale;
        }
        ui
    }
}

fn draw_laid(
    ui: &mut UiBatch,
    atlas: &Atlas,
    laid: &Laid,
    origin: [f32; 2],
    clip: Option<[f32; 2]>,
    hud: &ComputerHud,
) {
    let visible = |top: f32, height: f32| {
        clip.is_none_or(|[low, high]| top >= low - 0.5 && top + height <= high + 0.5)
    };
    match laid.kind {
        Kind::Text(step) => {
            for (i, line) in laid.lines.iter().enumerate() {
                let ly = origin[1] + i as f32 * atlas.line;
                if visible(ly, atlas.line) {
                    ui.text(atlas, origin[0], ly, line, amber(step, 1.0));
                }
            }
        }
        Kind::Run { fg, bg } => {
            let [_, _, w, h] = laid.rect;
            if !visible(origin[1], h) {
                return;
            }
            if let Some(bg) = bg {
                ui.rect(atlas, origin[0], origin[1], w, h, bg);
            }
            if hud
                .selection
                .is_some_and(|(a, b)| laid.rect[1] + h > a.min(b) && laid.rect[1] <= a.max(b))
            {
                ui.rect(
                    atlas,
                    origin[0],
                    origin[1],
                    w,
                    h,
                    amber(Intensity::Half, 0.35),
                );
            }
            let text = laid.lines.first().map_or("", String::as_str);
            let cells = terminal_gfx::phone::cells(text, usize::MAX);
            terminal_gfx::phone::draw(ui, atlas, origin, &cells, fg);
        }
        Kind::Button { enabled } => {
            let [_, _, bw, bh] = laid.rect;
            if !visible(origin[1], bh) {
                return;
            }
            let selected = matches!(&laid.action, Some(Action::Page(page)) if *page == hud.page);
            let color = if enabled {
                amber(Intensity::Full, 1.0)
            } else {
                amber(Intensity::Quarter, 1.0)
            };
            if selected {
                ui.rect(
                    atlas,
                    origin[0],
                    origin[1],
                    bw,
                    bh,
                    amber(Intensity::Quarter, 0.6),
                );
            }
            ui.frame(atlas, origin[0], origin[1], bw, bh, 1.0, color);
            for (i, line) in laid.lines.iter().enumerate() {
                ui.text(
                    atlas,
                    origin[0] + 8.0,
                    origin[1] + 7.0 + i as f32 * atlas.line,
                    line,
                    color,
                );
            }
        }
        Kind::Qr => {
            let [_, _, side, _] = laid.rect;
            let Some(modules) = &hud.feed.qr else {
                return;
            };
            if !visible(origin[1], side) {
                return;
            }
            // Dark modules on a light field, as scanners expect.
            ui.rect(
                atlas,
                origin[0],
                origin[1],
                side,
                side,
                amber(Intensity::Full, 1.0),
            );
            let cell = side / modules.len().max(1) as f32;
            for (row, cells) in modules.iter().enumerate() {
                for (col, dark) in cells.iter().enumerate() {
                    if *dark {
                        ui.rect(
                            atlas,
                            origin[0] + col as f32 * cell,
                            origin[1] + row as f32 * cell,
                            cell.ceil(),
                            cell.ceil(),
                            field(1.0),
                        );
                    }
                }
            }
        }
    }
}

fn item(laid: &Laid, frame: [f32; 4]) -> Item {
    let (role, enabled) = match laid.kind {
        Kind::Button { enabled } => ("button", enabled),
        Kind::Text(Intensity::Full) => ("heading", true),
        Kind::Text(_) | Kind::Run { .. } | Kind::Qr => ("text", true),
    };
    Item {
        key: laid.key.clone(),
        label: laid.label.chars().take(480).collect(),
        role,
        enabled,
        frame,
    }
}

/// Draws a box-drawing character the atlas lacks as lines through its cell.
/// Returns `false` for any other character.
/// A Rust Native color as the batch's linear RGBA.
fn linear(color: rust_native::style::Color) -> [f32; 4] {
    let rgb = (u32::from(color.red) << 16) | (u32::from(color.green) << 8) | u32::from(color.blue);
    let [r, g, b] = verse::palette::linear(rgb);
    [r, g, b, f32::from(color.alpha) / 255.0]
}

fn inside([x, y, w, h]: [f32; 4], p: [f32; 2]) -> bool {
    p[0] >= x && p[0] <= x + w && p[1] >= y && p[1] <= y + h
}

/// The value of the text node `key`, if the tree has one.
fn text_of(node: &Node<Value>, key: &str) -> Option<String> {
    match &node.element {
        Element::Text { value, .. } if node.key == key => Some(value.clone()),
        Element::Stack { children, .. } | Element::List { children, .. } => {
            children.iter().find_map(|child| text_of(child, key))
        }
        _ => None,
    }
}

/// The key of the first heading in a tree: it names the screen, so a new
/// screen starts at the top.
fn heading(node: &Node<Value>) -> Option<String> {
    match &node.element {
        Element::Text {
            role: TextRole::Heading,
            ..
        } => Some(node.key.clone()),
        Element::Stack { children, .. } | Element::List { children, .. } => {
            children.iter().find_map(heading)
        }
        _ => None,
    }
}

/// A vertical layout cursor over the body's width.
struct Flow<'a> {
    atlas: &'a Atlas,
    width: f32,
    pan: usize,
    y: f32,
    items: Vec<Laid>,
}

impl<'a> Flow<'a> {
    fn new(atlas: &'a Atlas, width: f32) -> Self {
        Self {
            atlas,
            width: width.max(atlas.advance * 8.0),
            pan: 0,
            y: 0.0,
            items: Vec::new(),
        }
    }

    fn text(&mut self, key: &str, value: &str, step: Intensity) {
        let mut lines = Vec::new();
        for paragraph in value.split('\n') {
            lines.extend(self.atlas.wrap(paragraph, self.width));
        }
        let height = lines.len() as f32 * self.atlas.line;
        self.items.push(Laid {
            key: key.into(),
            label: value.into(),
            kind: Kind::Text(step),
            lines,
            rect: [0.0, self.y, self.width, height],
            action: None,
        });
        self.y += height + 2.0;
    }

    fn button(
        &mut self,
        node: &Node<Value>,
        label: &str,
        enabled: bool,
        surface: Surface,
        x: f32,
    ) -> f32 {
        let inner = (self.width - x - 16.0).max(self.atlas.advance * 4.0);
        let lines: Vec<String> = self.atlas.wrap(label, inner).into_iter().take(3).collect();
        let text_w = lines
            .iter()
            .map(|line| self.atlas.measure(line))
            .fold(0.0, f32::max);
        let w = (text_w + 16.0).min(self.width - x);
        let h = lines.len() as f32 * self.atlas.line + 14.0;
        self.items.push(Laid {
            key: node.key.clone(),
            label: label.into(),
            kind: Kind::Button { enabled },
            lines,
            rect: [x, self.y, w, h],
            action: Some(Action::Activate(surface, node.key.clone())),
        });
        h
    }

    /// A terminal grid: each row on one line, its runs side by side in
    /// monospaced cells, never wrapped; what passes the body's edge is cut.
    fn grid(&mut self, rows: &[Node<Value>]) {
        let columns = (self.width / self.atlas.advance).floor().max(1.0) as usize;
        for row in rows {
            let runs: Vec<&Node<Value>> = match &row.element {
                Element::Stack { children, .. } => children.iter().collect(),
                _ => vec![row],
            };
            let mut col = 0usize;
            let mut skip = self.pan;
            for run in runs {
                let Element::Text { value, .. } = &run.element else {
                    continue;
                };
                if col >= columns {
                    break;
                }
                let all = terminal_gfx::phone::cells(value, usize::MAX);
                let start = all
                    .iter()
                    .position(|cell| {
                        if skip == 0 {
                            return true;
                        }
                        skip = skip.saturating_sub(usize::from(cell.width));
                        false
                    })
                    .unwrap_or(all.len());
                let value: String = all[start..]
                    .iter()
                    .flat_map(|cell| std::iter::once(cell.ch).chain(cell.combining.iter().copied()))
                    .collect();
                let row = terminal_gfx::phone::cells(&value, columns - col);
                let cells = terminal_gfx::phone::columns(&row);
                let text: String = row
                    .iter()
                    .flat_map(|cell| std::iter::once(cell.ch).chain(cell.combining.iter().copied()))
                    .collect();
                let fg = run
                    .style
                    .foreground
                    .map_or(amber(Intensity::ThreeQuarters, 1.0), linear);
                self.items.push(Laid {
                    key: run.key.clone(),
                    label: text.trim().to_owned(),
                    kind: Kind::Run {
                        fg,
                        bg: run.style.background.map(linear),
                    },
                    lines: vec![text],
                    rect: [
                        col as f32 * self.atlas.advance,
                        self.y,
                        cells as f32 * self.atlas.advance,
                        self.atlas.line,
                    ],
                    action: None,
                });
                col += cells;
            }
            self.y += self.atlas.line;
        }
        self.y += GAP;
    }

    fn block(&mut self, node: &Node<Value>, surface: Surface) {
        if self.items.len() >= 1024 {
            return;
        }
        if surface == Surface::Terminal
            && node.key == "terminal-grid"
            && let Element::Stack { children, .. } = &node.element
        {
            self.grid(children);
            return;
        }
        match &node.element {
            Element::Text { value, role } => {
                let step = match role {
                    TextRole::Heading => Intensity::Full,
                    TextRole::Status => Intensity::Half,
                    _ => Intensity::ThreeQuarters,
                };
                if *role == TextRole::Heading && self.y > 0.0 {
                    self.y += 4.0;
                }
                self.text(&node.key, value, step);
            }
            Element::Button { label, enabled, .. } => {
                let h = self.button(node, label, *enabled, surface, 0.0);
                self.y += h + GAP;
            }
            Element::Stack {
                axis: Axis::Horizontal,
                children,
            } => self.row(children, surface),
            Element::Stack { children, .. }
            | Element::List { children, .. }
            | Element::Transcript { children, .. }
            | Element::Message { children, .. } => {
                if node.style.padding_top.is_some() && self.y > 0.0 {
                    self.y += 4.0;
                }
                for child in children {
                    self.block(child, surface);
                }
            }
            // Coder's screens do not send the other conversation elements;
            // draw their text if one arrives.
            Element::Markdown { blocks } => {
                self.text(
                    &node.key,
                    &rust_native::markdown::plain(blocks),
                    Intensity::ThreeQuarters,
                );
            }
            Element::Tool { name, detail, .. } => {
                self.text(&node.key, &format!("{name} {detail}"), Intensity::Half);
            }
            Element::Working { label } => self.text(&node.key, label, Intensity::Half),
            Element::RichText { .. }
            | Element::Field { .. }
            | Element::Choice { .. }
            | Element::Dialog { .. } => {
                self.text(&node.key, "Unsupported v3 component", Intensity::Half);
            }
            Element::Surface { .. } | Element::Composer { .. } => {}
        }
    }

    /// Buttons flow left to right and wrap; anything else takes its own
    /// line below them.
    fn row(&mut self, children: &[Node<Value>], surface: Surface) {
        let mut x = 0.0;
        let mut line_h: f32 = 0.0;
        for child in children {
            match &child.element {
                Element::Button { label, enabled, .. } => {
                    let inner = self.atlas.measure(label) + 16.0;
                    if x > 0.0 && x + inner > self.width {
                        self.y += line_h + GAP;
                        x = 0.0;
                        line_h = 0.0;
                    }
                    let h = self.button(child, label, *enabled, surface, x);
                    let w = self.items.last().map_or(0.0, |laid| laid.rect[2]);
                    x += w + GAP;
                    line_h = line_h.max(h);
                }
                _ => {
                    if line_h > 0.0 {
                        self.y += line_h + GAP;
                    }
                    x = 0.0;
                    line_h = 0.0;
                    self.block(child, surface);
                }
            }
        }
        if line_h > 0.0 {
            self.y += line_h + GAP;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn atlas() -> Atlas {
        Atlas::new(12.0)
    }

    fn feed(buttons: usize) -> Feed {
        let mut children = vec![json!({
            "key": "computers-title", "style": {},
            "element": {"kind": "text", "props": {"value": "Computers", "role": "heading"}}
        })];
        for i in 0..buttons {
            children.push(json!({
                "key": format!("host-{i}"), "style": {},
                "element": {"kind": "stack", "props": {"axis": "horizontal", "children": [
                    {"key": format!("host-{i}-open"), "style": {}, "element": {"kind": "button",
                        "props": {"label": "Open", "enabled": true, "intent": {"kind": "refresh"}}}},
                    {"key": format!("host-{i}-order"), "style": {}, "element": {"kind": "button",
                        "props": {"label": "Order", "enabled": false, "intent": {"kind": "refresh"}}}},
                    {"key": format!("host-{i}-order-reason"), "style": {}, "element": {"kind": "text",
                        "props": {"value": "Unavailable: offline", "role": "status"}}}
                ]}}
            }));
        }
        Feed {
            computers: Some(json!({
                "schema": "rust-native.view.v2", "instance": "computers-1", "revision": 7,
                "root": {"key": "root", "style": {}, "element": {"kind": "stack",
                    "props": {"axis": "vertical", "children": children}}}
            })),
            ..Feed::default()
        }
    }

    fn tap(hud: &mut ComputerHud, size: [f32; 2], key: &str, id: u64) -> bool {
        let snapshot = hud.snapshot(&atlas(), size, true);
        let item = snapshot
            .items
            .iter()
            .find(|item| item.key == key)
            .unwrap_or_else(|| panic!("{key} is not in view"));
        let at = [
            item.frame[0] + item.frame[2] / 2.0,
            item.frame[1] + item.frame[3] / 2.0,
        ];
        hud.down(id, at);
        hud.up(&atlas(), size, id, at, false)
    }

    #[test]
    fn a_tap_names_the_node_of_the_current_view_and_disabled_controls_do_nothing() {
        let size = [393.0, 852.0];
        let mut hud = ComputerHud::new(true);
        hud.open();
        assert_eq!(hud.take_commands(), vec![Command::Refresh]);
        hud.feed(feed(2)).unwrap();
        assert!(!tap(&mut hud, size, "host-1-open", 1));
        assert_eq!(
            hud.take_commands(),
            vec![Command::Activate {
                surface: Surface::Computers,
                instance: "computers-1".into(),
                revision: 7,
                node: "host-1-open".into(),
            }]
        );
        let snapshot = hud.snapshot(&atlas(), size, true);
        assert!(snapshot.busy);
        assert!(snapshot.items.iter().any(|item| item.key == "hud-status"));
        tap(&mut hud, size, "host-0-order", 2);
        assert!(hud.take_commands().is_empty());
        // A reason follows its disabled control on its own line.
        let reason = snapshot
            .items
            .iter()
            .find(|item| item.key == "host-0-order-reason")
            .unwrap();
        let order = snapshot
            .items
            .iter()
            .find(|item| item.key == "host-0-order")
            .unwrap();
        assert!(reason.frame[1] >= order.frame[1] + order.frame[3]);
        assert!(tap(&mut hud, size, "hud-close", 3));
    }

    #[test]
    fn drags_scroll_long_screens_and_never_tap() {
        let size = [393.0, 700.0];
        let mut hud = ComputerHud::new(true);
        hud.feed(feed(40)).unwrap();
        let first = hud.snapshot(&atlas(), size, true);
        assert!(first.max_scroll > 0.0);
        assert!(!first.items.iter().any(|item| item.key == "host-39-open"));
        let body = first.body;
        let start = [body[0] + 40.0, body[1] + body[3] - 10.0];
        hud.down(9, start);
        for step in 1..=200 {
            hud.moved(&atlas(), size, 9, [start[0], start[1] - step as f32 * 20.0]);
        }
        assert!(!hud.up(&atlas(), size, 9, [start[0], start[1] - 4000.0], false));
        assert!(hud.take_commands().is_empty());
        let end = hud.snapshot(&atlas(), size, true);
        assert_eq!(end.scroll, end.max_scroll);
        assert!(end.items.iter().any(|item| item.key == "host-39-open"));
        // A new screen starts at the top.
        let mut other = feed(40);
        other.computers.as_mut().unwrap()["root"]["element"]["props"]["children"][0]["key"] =
            json!("order-title");
        hud.feed(other).unwrap();
        assert_eq!(hud.snapshot(&atlas(), size, true).scroll, 0.0);
    }

    #[test]
    fn a_second_finger_or_a_cancel_voids_the_tap() {
        let size = [393.0, 852.0];
        let mut hud = ComputerHud::new(true);
        hud.feed(feed(1)).unwrap();
        let snapshot = hud.snapshot(&atlas(), size, true);
        let open = snapshot
            .items
            .iter()
            .find(|i| i.key == "host-0-open")
            .unwrap();
        let at = [open.frame[0] + 4.0, open.frame[1] + 4.0];
        hud.down(1, at);
        hud.down(2, [10.0, 10.0]);
        hud.up(&atlas(), size, 2, [10.0, 10.0], false);
        hud.up(&atlas(), size, 1, at, false);
        hud.down(3, at);
        hud.up(&atlas(), size, 3, at, true);
        assert!(hud.take_commands().is_empty());
    }

    #[test]
    fn input_requests_ask_the_native_host_and_the_terminal_page_follows_its_view() {
        let size = [393.0, 852.0];
        let mut hud = ComputerHud::new(true);
        let mut with_input = feed(1);
        with_input.computers_input = Some(FeedInput {
            token: "t-1".into(),
            purpose: "invitation".into(),
            label: "Computer invitation".into(),
            prompt: "Scan or paste the complete coder-host: invitation.".into(),
            scan: true,
            secret: false,
            max_bytes: 16384,
        });
        with_input.computers_qr = Some(FeedQr {
            size: 2,
            rows: vec!["10".into(), "01".into()],
        });
        hud.feed(with_input).unwrap();
        let snapshot = hud.snapshot(&atlas(), size, true);
        assert!(snapshot.items.iter().any(|item| item.key == "computers-qr"));
        tap(&mut hud, size, "hud-input-scan", 1);
        tap(&mut hud, size, "hud-input-type", 2);
        tap(&mut hud, size, "hud-input-cancel", 3);
        assert_eq!(
            hud.take_commands(),
            vec![
                Command::Scan {
                    token: "t-1".into()
                },
                Command::Type {
                    token: "t-1".into()
                },
                Command::CancelInput {
                    token: "t-1".into()
                },
            ]
        );
        let mut terminal = feed(1);
        terminal.terminal = Some(json!({
            "schema": "rust-native.view.v2", "instance": "terminal-1", "revision": 1,
            "root": {"key": "terminal-close", "style": {}, "element": {"kind": "button",
                "props": {"label": "Back", "enabled": true, "intent": {"kind": "close"}}}}
        }));
        hud.feed(terminal).unwrap();
        assert_eq!(hud.page(), Page::Terminal);
        tap(&mut hud, size, "terminal-close", 4);
        assert!(matches!(
            hud.take_commands().as_slice(),
            [Command::Activate { surface: Surface::Terminal, node, .. }] if node == "terminal-close"
        ));
        hud.feed(feed(1)).unwrap();
        assert_eq!(hud.page(), Page::Computers);
        tap(&mut hud, size, "hud-tab-chats", 5);
        assert_eq!(hud.page(), Page::Chats);
        assert!(!hud.drawn());
        assert!(!hud.snapshot(&atlas(), size, true).visible);
    }

    #[test]
    fn invalid_feeds_are_refused_and_named() {
        let mut hud = ComputerHud::new(true);
        let mut bad = feed(1);
        bad.computers_qr = Some(FeedQr {
            size: 2,
            rows: vec!["1".into(), "01".into()],
        });
        assert!(hud.feed(bad).is_err());
        let mut bad = feed(1);
        bad.computers.as_mut().unwrap()["revision"] = json!(0);
        assert!(hud.feed(bad).is_err());
        let snapshot = hud.snapshot(&atlas(), [393.0, 852.0], true);
        assert!(
            snapshot
                .items
                .iter()
                .any(|item| item.key == "hud-status" && item.label.contains("invalid"))
        );
        let native = ComputerHud::new(false);
        assert!(!native.drawn());
    }

    fn terminal_feed(output: &[u8]) -> Feed {
        let mut model = coder_computers::terminal::Model::new("ab".repeat(32), "Studio", 6, 30);
        model.phase = coder_computers::terminal::Phase::Attached;
        model.vt.feed(output);
        let view = coder_computers::terminal::view(&model, "terminal-1", 1);
        Feed {
            terminal: Some(serde_json::to_value(&view).unwrap()),
            ..feed(1)
        }
    }

    #[test]
    fn host_wide_text_and_touch_selection_preserve_columns_and_copy_without_input() {
        let size = [393.0, 852.0];
        let atlas = atlas();
        let mut hud = ComputerHud::new(true);
        hud.open();
        hud.take_commands();
        hud.feed(terminal_feed(
            "界e\u{301}\x1b[31mx\x1b[0m\r\nsecond".as_bytes(),
        ))
        .unwrap();
        let shown = hud.snapshot(&atlas, size, true);
        let first = shown
            .items
            .iter()
            .find(|item| item.key == "terminal-row-0-0")
            .unwrap();
        let color = shown
            .items
            .iter()
            .find(|item| item.key == "terminal-row-0-1")
            .unwrap();
        assert_eq!(first.label, "界e\u{301}");
        assert_eq!(color.frame[0] - first.frame[0], 3.0 * atlas.advance);
        hud.act(&atlas, size, "hud-terminal-select");
        let a = [first.frame[0] + 1.0, first.frame[1] + 1.0];
        let b = [a[0], a[1] + 2.0 * atlas.line];
        hud.down(42, a);
        hud.moved(&atlas, size, 42, b);
        hud.up(&atlas, size, 42, b, false);
        hud.act(&atlas, size, "hud-terminal-copy");
        assert!(
            matches!(hud.take_commands().as_slice(), [Command::Copy { text }] if text.starts_with("界e\u{301}x"))
        );
        // A new host projection retires the selection before it can copy changed output.
        hud.feed(terminal_feed(b"changed")).unwrap();
        assert!(hud.selection.is_none());
    }

    #[test]
    fn the_terminal_page_draws_rows_in_cells_and_asks_for_its_grid() {
        let size = [393.0, 852.0];
        let atlas = atlas();
        let mut hud = ComputerHud::new(true);
        hud.open();
        hud.take_commands();
        hud.feed(terminal_feed(
            b"$ echo hi\r\nhi\r\n\x1b[31merr\x1b[0m ok\r\n\x1b(0lqk\x1b(B\r\n$ ",
        ))
        .unwrap();
        assert_eq!(hud.page(), Page::Terminal);
        let snapshot = hud.snapshot(&atlas, size, true);
        let label = |key: &str| {
            snapshot
                .items
                .iter()
                .find(|item| item.key == key)
                .map(|item| (item.label.clone(), item.frame))
        };
        // Rows are one line each, a cell per character, never wrapped.
        let (text, first) = label("terminal-row-0").unwrap();
        assert_eq!(text, "$ echo hi");
        let (text, second) = label("terminal-row-1").unwrap();
        assert_eq!(text, "hi");
        assert_eq!(second[1] - first[1], atlas.line);
        assert_eq!(first[2], 9.0 * atlas.advance);
        // A colored run sits in the columns after the one before it.
        let (text, red) = label("terminal-row-2-0").unwrap();
        let (ok, rest) = label("terminal-row-2-1").unwrap();
        assert_eq!((text.as_str(), ok.as_str()), ("err", "ok"));
        assert_eq!(rest[0], red[0] + 3.0 * atlas.advance);
        // Line drawing is drawn, not replaced with `?`.
        assert_eq!(label("terminal-row-3").unwrap().0, "┌─┐");
        let batch = hud.draw(&atlas, size, 1.0, None);
        assert!(!batch.vertices.is_empty());

        // The page asks once for the grid its body fits.
        hud.sync_terminal(&atlas, size);
        let commands = hud.take_commands();
        let [Command::TerminalResize { rows, cols }] = commands.as_slice() else {
            panic!("expected one resize, got {commands:?}");
        };
        let body = snapshot.body;
        assert_eq!(
            usize::from(*cols),
            (body[2] / atlas.advance).floor() as usize
        );
        assert!(*rows >= 10 && f32::from(*rows) * atlas.line < body[3]);
        hud.sync_terminal(&atlas, size);
        assert!(hud.take_commands().is_empty());
        // A wider surface asks again.
        hud.sync_terminal(&atlas, [852.0, 393.0]);
        assert!(matches!(
            hud.take_commands().as_slice(),
            [Command::TerminalResize { cols: wide, .. }] if wide > cols
        ));

        // The keyboard shrinks the terminal page, so its rows stay above it.
        hud.sync_terminal(&atlas, size);
        hud.take_commands();
        hud.set_keyboard(300.0).unwrap();
        hud.sync_terminal(&atlas, size);
        assert!(matches!(
            hud.take_commands().as_slice(),
            [Command::TerminalResize { rows: fewer, .. }] if fewer < rows
        ));
        assert!(
            hud.snapshot(&atlas, size, true).frame[1] + hud.snapshot(&atlas, size, true).frame[3]
                <= size[1] - 300.0
        );
        assert!(hud.set_keyboard(f32::NAN).is_err());

        // KEYBOARD asks the native host for the terminal keyboard.
        assert!(!tap(&mut hud, size, "hud-terminal-keyboard", 9));
        assert_eq!(hud.take_commands(), vec![Command::TerminalKeyboard]);
        // Keys on the accessory row are view controls.
        assert!(!tap(&mut hud, size, "terminal-key-up", 10));
        assert!(matches!(
            hud.take_commands().as_slice(),
            [Command::Activate { surface: Surface::Terminal, node, .. }] if node == "terminal-key-up"
        ));
    }
}
