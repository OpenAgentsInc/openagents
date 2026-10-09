//! Desktop navigation and the sidebar, projected as semantic views.
//!
//! The layout reimplements Zeron's shell and sidebar design. A live window
//! lists the host's saved chats (`chat::Panel::sync_sidebar`); the example
//! chats in [`State::default`] are only for windows without a host
//! (captures and tests) and never create a task or a saved conversation.

use crate::model::{Intent, Model};
pub use openagents_chat_app::attention::Indicator;
use rust_native::style::{Color, Space, Style, TextAlign, TextWeight};
use rust_native::{Axis, Element, Glyph, Icon, Node, TextRole};
use serde::Serialize;
use std::collections::BTreeSet;

pub const MARK: &str = "openagents-mark";
pub const SIDEBAR_MIN: f32 = 224.0;
pub const SIDEBAR_MAX: f32 = 400.0;
pub const SIDEBAR_DEFAULT: f32 = 256.0;
/// The sidebar's **Filter sessions…** field shows once there are this many
/// chats (#10072); with fewer the whole list fits, and the palette
/// (Cmd/Ctrl+K) still searches them.
pub const SEARCH_MIN_CHATS: usize = 5;
const SAMPLE_LIMIT: usize = 40;
// The chrome's colors come from the theme seam
// (`openagents_chat_app::visual::current`), Coder Light or the dark look.
fn sidebar_color() -> Color {
    openagents_chat_app::visual::current().sidebar
}
fn selected_color() -> Color {
    openagents_chat_app::visual::current().selected
}
fn text_color() -> Color {
    openagents_chat_app::visual::current().text
}
fn muted_color() -> Color {
    openagents_chat_app::visual::current().muted
}
const CLEAR: Color = Color {
    red: 0,
    green: 0,
    blue: 0,
    alpha: 0,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Section {
    Pinned,
    OpenAgents,
    Website,
    Recent,
    Archived,
}

impl Section {
    fn key(self) -> &'static str {
        match self {
            Self::Pinned => "pinned",
            Self::OpenAgents => "openagents",
            Self::Website => "website",
            Self::Recent => "recent",
            Self::Archived => "archived",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Pinned => "Pinned",
            Self::OpenAgents => "OpenAgents",
            Self::Website => "Website",
            Self::Recent => "Recent",
            Self::Archived => "Archived",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    ToggleSidebar,
    ToggleSection {
        section: Section,
    },
    SelectChat {
        id: u64,
    },
    NewChat,
    Saved,
    Grid,
    /// The Map page (#10085): how OpenAgents routes requests.
    Map,
    Computers,
    Settings,
    /// Settings' update button: restart into a waiting build, or open a
    /// newer package's download.
    Update,
    /// The titlebar's history controls: the previous or next visited page.
    Back,
    Forward,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Chat(u64),
    Saved,
    Grid,
    Map,
    Computers,
    Settings,
}

#[derive(Clone, Debug)]
pub struct Chat {
    pub id: u64,
    pub title: String,
    pub detail: &'static str,
    /// What the chat's Coder work asks of the person, which the row names
    /// before its context line.
    pub indicator: Indicator,
    pub section: Section,
}

/// A newer release the window offers (on Linux; the Mac's menu bar offers
/// its own).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Update {
    pub version: String,
    /// `true` when a checked build is waiting and the button restarts into
    /// it; `false` when the button opens the package's download.
    pub ready: bool,
}

/// Presentation state for the shell. Computer state remains in `Model`.
#[derive(Clone, Debug)]
pub struct State {
    pub live: bool,
    pub page: Page,
    pub sidebar_width: f32,
    pub collapsed: bool,
    pub fullscreen: bool,
    pub profile_open: bool,
    pub closed_sections: BTreeSet<Section>,
    pub chats: Vec<Chat>,
    /// Every saved chat, before the sidebar's filter.
    pub total_chats: usize,
    pub search: String,
    pub projects: std::collections::BTreeMap<u64, String>,
    /// A newer release to offer on Settings.
    pub update: Option<Update>,
    /// Settings' page and preferences ([`crate::settings`]).
    pub settings: crate::settings::Settings,
    next_chat: u64,
    settings_return: Page,
    /// Pages visited in this window, oldest first, and the current one.
    history: Vec<Page>,
    at: usize,
}

/// The most visited pages the titlebar's Back and Forward remember.
const HISTORY_LIMIT: usize = 64;

impl Default for State {
    fn default() -> Self {
        let examples = [
            (
                "Welcome to OpenAgents",
                "OpenAgents · Today",
                Section::Pinned,
            ),
            (
                "A place for good ideas",
                "OpenAgents · Yesterday",
                Section::Pinned,
            ),
            (
                "Build the desktop shell",
                "Coder · Just now",
                Section::OpenAgents,
            ),
            (
                "Make the sidebar feel right",
                "Coder · 12 min ago",
                Section::OpenAgents,
            ),
            (
                "Review the latest changes",
                "Coder · 1 hour ago",
                Section::OpenAgents,
            ),
            ("Design a new home page", "Coder · Today", Section::Website),
            (
                "A quieter color palette",
                "OpenAgents · Today",
                Section::Website,
            ),
            (
                "Polish the little details",
                "Coder · Yesterday",
                Section::Website,
            ),
            (
                "What should I make next?",
                "OpenAgents · Today",
                Section::Recent,
            ),
            (
                "Take a look around the Grid",
                "Verse · Today",
                Section::Recent,
            ),
            ("Compare the latest results", "Gym · Today", Section::Recent),
            (
                "Plan a weekend project",
                "OpenAgents · Yesterday",
                Section::Recent,
            ),
            (
                "Connect my phone",
                "OpenAgents · Yesterday",
                Section::Recent,
            ),
            ("A small experiment", "Gym · Yesterday", Section::Recent),
            (
                "Notes for tomorrow",
                "OpenAgents · 2 days ago",
                Section::Recent,
            ),
            (
                "The first prototype",
                "Coder · Last week",
                Section::Archived,
            ),
            (
                "An idea to come back to",
                "OpenAgents · Last week",
                Section::Archived,
            ),
        ];
        Self {
            live: false,
            total_chats: 0,
            search: String::new(),
            projects: std::collections::BTreeMap::new(),
            update: None,
            settings: crate::settings::Settings::default(),
            page: Page::Chat(1),
            sidebar_width: SIDEBAR_DEFAULT,
            collapsed: false,
            fullscreen: false,
            profile_open: false,
            settings_return: Page::Chat(1),
            closed_sections: BTreeSet::from([Section::Archived]),
            chats: examples
                .into_iter()
                .enumerate()
                .map(|(index, (title, detail, section))| Chat {
                    id: index as u64 + 1,
                    title: title.into(),
                    detail,
                    indicator: Indicator::Idle,
                    section,
                })
                .collect(),
            next_chat: 18,
            history: vec![],
            at: 0,
        }
    }
}

impl State {
    /// A live shell starts with no fabricated conversations.
    pub fn empty() -> Self {
        Self {
            live: true,
            chats: vec![],
            page: Page::Chat(0),
            settings_return: Page::Chat(0),
            ..Self::default()
        }
    }

    /// Replace the sidebar with persisted host conversations.
    pub fn sync_chats(&mut self, chats: Vec<Chat>, selected: Option<u64>) {
        self.chats = chats;
        if let Some(id) = selected {
            self.page = Page::Chat(id);
        }
    }

    pub fn activate(&mut self, action: Action) {
        match action {
            Action::ToggleSidebar => self.collapsed = !self.collapsed,
            Action::ToggleSection { section } => {
                if !self.closed_sections.remove(&section) {
                    self.closed_sections.insert(section);
                }
            }
            Action::SelectChat { id } => {
                if self.chats.iter().any(|chat| chat.id == id) {
                    self.page = Page::Chat(id);
                }
            }
            Action::NewChat => {
                let id = self.next_chat;
                self.next_chat += 1;
                if self.chats.len() == SAMPLE_LIMIT {
                    self.chats.pop();
                }
                self.chats.insert(
                    0,
                    Chat {
                        id,
                        title: "New chat".into(),
                        detail: "OpenAgents · Just now",
                        indicator: Indicator::Idle,
                        section: Section::Recent,
                    },
                );
                self.closed_sections.remove(&Section::Recent);
                self.page = Page::Chat(id);
            }
            Action::Saved => self.page = Page::Saved,
            Action::Grid => self.page = Page::Grid,
            Action::Map => self.page = Page::Map,
            Action::Computers => self.page = Page::Computers,
            Action::Settings => {
                if self.page == Page::Settings {
                    self.page = self.settings_return;
                } else {
                    self.settings_return = self.page;
                    self.page = Page::Settings;
                }
            }
            // The shell runs these; the page stays.
            Action::Update | Action::Back | Action::Forward => {}
        }
    }

    /// Remembers the current page after navigation. Returning to a page
    /// by Back or Forward keeps the pages after it; visiting a new page
    /// drops them, as in a browser.
    pub fn record(&mut self) {
        if self.page == Page::Chat(0) || self.history.get(self.at) == Some(&self.page) {
            return;
        }
        if !self.history.is_empty() {
            self.history.truncate(self.at + 1);
        }
        self.history.push(self.page);
        if self.history.len() > HISTORY_LIMIT {
            self.history.remove(0);
        }
        self.at = self.history.len() - 1;
    }

    /// The page Back (`false`) or Forward (`true`) returns to, moving the
    /// history there. Conversations that have since left the list are skipped.
    pub fn step(&mut self, forward: bool) -> Option<Page> {
        let mut at = self.at;
        loop {
            at = if forward {
                at.checked_add(1).filter(|at| *at < self.history.len())?
            } else {
                at.checked_sub(1)?
            };
            let page = self.history[at];
            if !matches!(page, Page::Chat(id) if !self.chats.iter().any(|chat| chat.id == id)) {
                self.at = at;
                return Some(page);
            }
        }
    }

    fn can_step(&self, forward: bool) -> bool {
        let pages = if forward {
            self.history.get(self.at + 1..).unwrap_or_default()
        } else {
            &self.history[..self.at.min(self.history.len())]
        };
        pages.iter().any(
            |page| !matches!(page, Page::Chat(id) if !self.chats.iter().any(|chat| chat.id == *id)),
        )
    }

    pub fn resize(&mut self, width: f32) {
        if width.is_finite() {
            self.sidebar_width = width.clamp(SIDEBAR_MIN, SIDEBAR_MAX);
        }
    }

    /// Whether the pairing screens show: Phones and computers, on its own
    /// or as a page of Settings. Leaving them cancels a shown code.
    pub fn shows_computers(&self) -> bool {
        self.page == Page::Computers
            || (self.page == Page::Settings
                && self.settings.pane == crate::settings::Pane::Computers)
    }

    /// Whether the sidebar shows its filter field: enough chats to need
    /// it, or a filter already typed.
    pub fn shows_search(&self) -> bool {
        self.live && (self.total_chats >= SEARCH_MIN_CHATS || !self.search.is_empty())
    }

    pub fn selected(&self) -> Option<&Chat> {
        let Page::Chat(id) = self.page else {
            return None;
        };
        self.chats.iter().find(|chat| chat.id == id)
    }
}

fn node(key: &str, element: Element<Intent>) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style::default(),
        element,
    }
}

fn stack(key: &str, axis: Axis, gap: Space, children: Vec<Node<Intent>>) -> Node<Intent> {
    let mut node = node(key, Element::Stack { axis, children });
    node.style.gap = Some(gap);
    if key.starts_with("sidebar-group-") || key == "sidebar-body" {
        node.style.gap_points = Some(2);
    }
    node
}

fn text(key: &str, value: impl Into<String>, role: TextRole) -> Node<Intent> {
    let mut node = node(
        key,
        Element::Text {
            value: value.into(),
            role,
        },
    );
    node.style.text_size = Some(if role == TextRole::Heading { 11 } else { 12 });
    node.style.line_height = Some(if role == TextRole::Heading { 14 } else { 16 });
    node
}

fn action(
    key: &str,
    label: impl Into<String>,
    action: Action,
    glyph: Option<Glyph>,
    selected: bool,
) -> Node<Intent> {
    let label = label.into();
    let multiline = label.contains('\n');
    let mut node = node(
        key,
        Element::Button {
            shortcut: None,
            label,
            enabled: true,
            icon: glyph.map(|glyph| Icon {
                glyph,
                circular: false,
                pill: false,
            }),
            intent: Intent::Navigate { action },
        },
    );
    node.style = Style {
        background: Some(if selected {
            selected_color()
        } else {
            sidebar_color()
        }),
        foreground: Some(if selected {
            text_color()
        } else {
            muted_color()
        }),
        align: Some(TextAlign::Start),
        weight: Some(TextWeight::Normal),
        radius: Some(8),
        text_size: Some(if multiline { 13 } else { 12 }),
        line_height: Some(if multiline { 17 } else { 16 }),
        button_detail: multiline.then_some(rust_native::style::ButtonDetail {
            text_size: 11,
            line_height: 16,
            color: muted_color(),
            leading: true,
        }),
        button_padding: Some([8, 6]),
        min_height: Some(if multiline { 45 } else { 28 }),
        ..Style::default()
    };
    node
}

fn icon_button(key: &str, label: &str, action: Action, glyph: Glyph) -> Node<Intent> {
    let mut node = node(
        key,
        Element::Button {
            shortcut: None,
            label: label.into(),
            enabled: true,
            icon: Some(Icon {
                glyph,
                circular: true,
                pill: false,
            }),
            intent: Intent::Navigate { action },
        },
    );
    node.style.background = Some(CLEAR);
    node.style.foreground = Some(muted_color());
    node
}

/// A 24-point titlebar control with Zeron's 11% hover wash; a disabled
/// history control keeps its place with its arrow at 35% of the muted ink.
fn cluster_button(
    key: &str,
    label: &str,
    action: Action,
    glyph: Glyph,
    enabled: bool,
) -> Node<Intent> {
    let mut node = icon_button(key, label, action, glyph);
    if let Element::Button { enabled: on, .. } = &mut node.element {
        *on = enabled;
    }
    node.style.min_height = Some(24);
    node.style.radius = Some(6);
    node.style.glyph_size = Some(16);
    node.style.hover_background = Some(selected_color());
    if !enabled {
        node.style.glyph_color = Some(Color {
            alpha: 89,
            ..muted_color()
        });
    }
    node
}

/// Fixed horizontal space in the titlebar.
fn gap(key: &str, width: u16) -> Node<Intent> {
    let mut node = stack(key, Axis::Horizontal, Space::None, vec![]);
    node.style.padding_points = Some([0, 0, 0, width.min(128)]);
    node.style.intrinsic_width = Some(true);
    node
}

fn sidebar(state: &State, model: &Model) -> Node<Intent> {
    let header_rows = if state.shows_search() {
        vec![node(
            "chat-search",
            Element::Composer {
                token: "chat-search".into(),
                placeholder: "Filter sessions…".into(),
                max_bytes: 128,
                enabled: true,
                busy: false,
                stop: None,
                choices: vec![],
                draft: Some(state.search.clone()),
                focus: false,
            },
        )]
    } else if state.live {
        vec![]
    } else {
        vec![text("shell-brand", "OpenAgents", TextRole::Body)]
    };
    let header = stack("sidebar-header", Axis::Vertical, Space::None, header_rows);
    let mut groups = vec![];
    for section in [
        Section::Pinned,
        Section::OpenAgents,
        Section::Website,
        Section::Recent,
        Section::Archived,
    ] {
        if state.live && matches!(section, Section::OpenAgents | Section::Website) {
            continue;
        }
        let closed = state.closed_sections.contains(&section);
        let count = state
            .chats
            .iter()
            .filter(|chat| chat.section == section)
            .count();
        let label = format!(
            "{}  {}  {count}",
            if closed { "+" } else { "−" },
            section.label()
        );
        if count == 0 && matches!(section, Section::Pinned | Section::Archived) {
            continue;
        }
        let mut rows = if state.live && section == Section::Recent {
            vec![]
        } else {
            vec![action(
                &format!("sidebar-section-{}", section.key()),
                label,
                Action::ToggleSection { section },
                matches!(section, Section::OpenAgents | Section::Website).then_some(Glyph::Folder),
                false,
            )]
        };
        if !closed {
            rows.extend(
                state
                    .chats
                    .iter()
                    .filter(|chat| chat.section == section)
                    .map(|chat| {
                        // One newest-first list (#10100): a Coder chat's
                        // project is its row's context line, not a header
                        // that would sort its old chats above a new one.
                        let mut detail = state.projects.get(&chat.id).map_or_else(
                            || chat.detail.to_owned(),
                            |project| format!("Coder · {project}"),
                        );
                        if let Some(label) = chat.indicator.label() {
                            detail = format!("{label} · {detail}");
                        }
                        action(
                            &format!("sidebar-chat-{}", chat.id),
                            format!("{}\n{detail}", chat.title),
                            Action::SelectChat { id: chat.id },
                            None,
                            state.page == Page::Chat(chat.id),
                        )
                    }),
            );
        }
        groups.push(stack(
            &format!("sidebar-group-{}", section.key()),
            Axis::Vertical,
            Space::Xs,
            rows,
        ));
    }
    let body = stack("sidebar-body", Axis::Vertical, Space::Md, groups);
    let mut profile = action(
        "sidebar-profile",
        "Local",
        Action::NewChat,
        None,
        state.profile_open,
    );
    if let Element::Button { intent, .. } = &mut profile.element {
        *intent = Intent::Chat {
            action: crate::chat_action::Action::Profile,
        };
    }
    profile.style.align = Some(TextAlign::Center);
    profile.style.intrinsic_width = Some(true);
    profile.style.text_size = Some(13);
    profile.style.line_height = Some(17);
    profile.style.weight = Some(TextWeight::Medium);
    profile.style.button_padding = Some([8, 0]);
    profile.style.foreground = Some(if state.profile_open {
        text_color()
    } else {
        Color {
            alpha: 204,
            ..text_color()
        }
    });
    profile.style.hover_foreground = Some(text_color());
    profile.style.hover_background = Some(if state.profile_open {
        selected_color()
    } else {
        Color {
            alpha: 22,
            ..selected_color()
        }
    });
    profile.style.button_avatar = Some(rust_native::style::ButtonAvatar {
        initial: 'L',
        size: 16,
        text_size: 10,
        weight: TextWeight::Semibold,
        background: text_color(),
        foreground: sidebar_color(),
    });
    let mut settings = icon_button(
        "sidebar-settings",
        "Settings",
        Action::Settings,
        Glyph::Settings,
    );
    settings.style.radius = Some(8);
    settings.style.glyph_size = Some(15);
    settings.style.hover_background = Some(selected_color());
    settings.style.hover_foreground = Some(text_color());
    if state.page == Page::Settings {
        settings.style.background = Some(selected_color());
        settings.style.foreground = Some(text_color());
    }
    // The Verse page (#10071): the Grid lives there and nowhere else.
    let mut verse = icon_button("sidebar-verse", "Verse", Action::Grid, Glyph::Cloud);
    verse.style.radius = Some(8);
    verse.style.glyph_size = Some(15);
    verse.style.hover_background = Some(selected_color());
    verse.style.hover_foreground = Some(text_color());
    if state.page == Page::Grid {
        verse.style.background = Some(selected_color());
        verse.style.foreground = Some(text_color());
    }
    // The Map page (#10085), beside Verse.
    let mut map = icon_button("sidebar-map", "Map", Action::Map, Glyph::Map);
    map.style.radius = Some(8);
    map.style.glyph_size = Some(15);
    map.style.hover_background = Some(selected_color());
    map.style.hover_foreground = Some(text_color());
    if state.page == Page::Map {
        map.style.background = Some(selected_color());
        map.style.foreground = Some(text_color());
    }
    let mut spacer = stack(
        "sidebar-footer-spacer",
        Axis::Horizontal,
        Space::None,
        vec![],
    );
    spacer.style.fill_height = Some(false);
    let mut footer = stack(
        "sidebar-footer",
        Axis::Horizontal,
        Space::None,
        vec![profile, spacer, map, verse, settings],
    );
    footer.style.gap_points = Some(4);
    let mut bottom = vec![];
    if let Some(line) = model
        .host
        .as_ref()
        .and_then(|host| openagents_chat_app::watchers::line(&host.watchers))
    {
        // The background watchers running on this computer (#10172): one
        // quiet line, shown from the host's first answer at every start.
        let mut line = text("sidebar-watchers", line, TextRole::Status);
        line.style.text_size = Some(11);
        line.style.foreground = Some(muted_color());
        line.style.padding_points = Some([0, 8, 0, 8]);
        bottom.push(line);
    }
    if state.live
        && let Some(engines) = engines(model, state.sidebar_width)
    {
        bottom.push(engines);
    }
    bottom.push(footer);
    let mut bottom = stack("sidebar-bottom", Axis::Vertical, Space::None, bottom);
    bottom.style.gap_points = Some(8);
    let mut pane = stack(
        "shell-sidebar",
        Axis::Vertical,
        Space::None,
        if state.collapsed {
            ["sidebar-header", "sidebar-body", "sidebar-bottom"]
                .into_iter()
                .map(|key| stack(key, Axis::Vertical, Space::None, vec![]))
                .collect()
        } else {
            vec![header, body, bottom]
        },
    );
    pane.style.background = Some(sidebar_color());
    pane
}

/// The engines this computer's Coder runs, one condensed row each, above
/// the sidebar's footer (#10072, from the #10018 report): the name, the
/// model, and a small meter for the tightest usage window. Hovering a row
/// shows every window with its reset ([`engine_tooltip`]); a click opens
/// Settings' Coder page, which shows the full report. Read-only: nothing
/// here changes an engine.
fn engines(model: &Model, sidebar_width: f32) -> Option<Node<Intent>> {
    if model.engine.is_none() && model.engine_note.is_none() {
        return None;
    }
    let mut rows = vec![];
    if let Some(report) = &model.engine {
        if report.routes.is_empty() {
            for (index, account) in report.accounts.iter().take(4).enumerate() {
                rows.push(engine_row(
                    sidebar_width,
                    index,
                    &account.name,
                    sign_in(account.signed_in),
                    None,
                ));
            }
        } else {
            for (index, route) in report.routes.iter().take(4).enumerate() {
                rows.push(engine_row(
                    sidebar_width,
                    index,
                    &route.name,
                    &route.model,
                    tightest(&route.usage).map(|percent| (route.provider.as_str(), percent)),
                ));
            }
            // An engine a person's own runs can use that no route names,
            // such as Grok Build (#10091): signed in or not, with no meter.
            let shown = report.routes.len().min(4);
            for (offset, account) in openagents_chat_app::engine::extra_accounts(report)
                .into_iter()
                .take(4)
                .enumerate()
            {
                rows.push(engine_row(
                    sidebar_width,
                    shown + offset,
                    &account.name,
                    sign_in(account.signed_in),
                    None,
                ));
            }
        }
    }
    if let Some(note) = &model.engine_note {
        let mut line = text("sidebar-engine-note", note.clone(), TextRole::Status);
        line.style.text_size = Some(11);
        line.style.foreground = Some(muted_color());
        line.style.padding_points = Some([0, 8, 0, 8]);
        rows.push(line);
    }
    let mut section = stack("sidebar-engines", Axis::Vertical, Space::None, rows);
    section.style.gap_points = Some(2);
    Some(section)
}

fn sign_in(signed_in: bool) -> &'static str {
    if signed_in {
        "Signed in"
    } else {
        "Not signed in"
    }
}

/// The used share of a route's tightest window, from 0 to 100, when its
/// usage is read.
fn tightest(usage: &crate::control::RouteUsage) -> Option<u8> {
    match usage {
        crate::control::RouteUsage::Windows {
            windows,
            limit_reached,
            used_percent,
        } => Some(if *limit_reached {
            100
        } else {
            windows
                .iter()
                .map(|window| window.used_percent)
                .max()
                .unwrap_or(*used_percent)
                .min(100)
        }),
        _ => None,
    }
}

fn engine_row(
    sidebar_width: f32,
    index: usize,
    name: &str,
    detail: &str,
    meter: Option<(&str, u8)>,
) -> Node<Intent> {
    // One line: the model is shortened to the row's room, and the hover
    // text has it whole. Rows draw in Paper Mono, whose every character
    // advances 0.606 em (about 7.3 points at 12 points).
    const ADVANCE: f32 = 12.0 * 0.606;
    let room = sidebar_width.clamp(SIDEBAR_MIN, SIDEBAR_MAX)
        - 16.0
        - 16.0
        - if meter.is_some() { 76.0 } else { 0.0 };
    let budget = ((room / ADVANCE) as usize).saturating_sub(name.chars().count() + 2);
    let detail = if detail.chars().count() > budget {
        let mut short: String = detail.chars().take(budget.saturating_sub(1)).collect();
        short.push('…');
        short
    } else {
        detail.to_owned()
    };
    let mut row = node(
        &format!("sidebar-engine-{index}"),
        Element::Button {
            shortcut: None,
            label: format!("{name}  {detail}"),
            enabled: true,
            icon: None,
            intent: Intent::Settings {
                action: crate::settings::Action::Pane {
                    pane: crate::settings::Pane::Coder,
                },
            },
        },
    );
    row.style = Style {
        background: Some(sidebar_color()),
        foreground: Some(muted_color()),
        hover_background: Some(selected_color()),
        hover_foreground: Some(text_color()),
        align: Some(TextAlign::Start),
        weight: Some(TextWeight::Normal),
        radius: Some(8),
        text_size: Some(12),
        line_height: Some(16),
        button_padding: Some([8, 6]),
        min_height: Some(28),
        ..Style::default()
    };
    let mut children = vec![row];
    if let Some((provider, percent)) = meter {
        let provider: String = provider
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .take(24)
            .collect();
        children.push(node(
            &format!("sidebar-engine-{index}-meter"),
            Element::Surface {
                resource: format!("engine-meter:{provider}:{percent}"),
                label: format!("{name} usage {percent} percent"),
            },
        ));
        let mut share = text(
            &format!("sidebar-engine-{index}-percent"),
            format!("{percent}%"),
            TextRole::Status,
        );
        share.style.text_size = Some(11);
        share.style.foreground = Some(muted_color());
        share.style.intrinsic_width = Some(true);
        share.style.padding_points = Some([5, 8, 0, 0]);
        children.push(share);
    }
    let mut line = stack(
        &format!("sidebar-engine-row-{index}"),
        Axis::Horizontal,
        Space::None,
        children,
    );
    line.style.gap_points = Some(6);
    line
}

/// The hover text for an engine row: every usage window with its reset.
pub fn engine_tooltip(model: &Model, key: &str) -> Option<String> {
    let index: usize = key.strip_prefix("sidebar-engine-")?.parse().ok()?;
    let report = model.engine.as_ref()?;
    if report.routes.is_empty() {
        let account = report.accounts.get(index)?;
        return Some(format!("{} · {}", account.name, sign_in(account.signed_in)));
    }
    let shown = report.routes.len().min(4);
    if index >= shown {
        let account = *openagents_chat_app::engine::extra_accounts(report).get(index - shown)?;
        return Some(format!("{} · {}", account.name, sign_in(account.signed_in)));
    }
    let route = report.routes.get(index)?;
    Some(format!(
        "{} · {} · {}. {}",
        route.name,
        route.model,
        if route.signed_in {
            "Signed in"
        } else {
            "Not signed in"
        },
        openagents_chat_app::engine::usage_sentence(&route.usage)
    ))
}

fn placeholder(state: &State) -> Node<Intent> {
    let new = state
        .selected()
        .is_some_and(|chat| chat.title == "New chat");
    let heading = if new {
        "A fresh start."
    } else {
        "What would you like to do?"
    };
    let line = if new {
        "Your next idea has a place to begin."
    } else {
        "Your chats, projects, and computers in one place."
    };
    let mut new_chat = action(
        "welcome-new-chat",
        "New chat",
        Action::NewChat,
        Some(Glyph::Compose),
        false,
    );
    new_chat.style.align = None;
    let mut grid = action(
        "welcome-grid",
        "Watch the Grid",
        Action::Grid,
        Some(Glyph::Cloud),
        false,
    );
    grid.style.align = None;
    let mut buttons = stack(
        "shell-welcome-actions",
        Axis::Wrap,
        Space::Sm,
        vec![new_chat, grid],
    );
    buttons.style.align = Some(TextAlign::Center);
    let mut body = stack(
        "shell-welcome",
        Axis::Vertical,
        Space::Md,
        vec![
            node(
                "shell-mark",
                Element::Surface {
                    resource: MARK.into(),
                    label: "OpenAgents".into(),
                },
            ),
            text("shell-welcome-title", heading, TextRole::Heading),
            text("shell-welcome-line", line, TextRole::Status),
            buttons,
        ],
    );
    body.style.align = Some(TextAlign::Center);
    body
}

/// The shell wraps the existing computer screens without changing their intents.
pub fn root(state: &State, model: &Model, now: u64) -> Node<Intent> {
    let prompt = model.nearby().is_some();
    let title: String = if prompt {
        "Connect a phone".into()
    } else {
        match state.page {
            Page::Chat(_) => state
                .selected()
                .map_or("New chat", |chat| chat.title.as_str())
                .into(),
            Page::Saved => "Saved sessions".into(),
            Page::Grid => "Verse".into(),
            Page::Map => "Map".into(),
            Page::Computers => "Phones and computers".into(),
            Page::Settings => "Settings".into(),
        }
    };
    // Zeron's unified titlebar (`render_titlebar_cluster`,
    // `render_session_title_bar`): after the window controls, a 24-point
    // sidebar toggle, then Back and Forward on a 2-point rhythm, and the
    // new-session plus while a conversation is open, each group 8 points
    // apart. The title starts past the sidebar's 16-point gutter.
    let chat_page = matches!(state.page, Page::Chat(_)) && !prompt;
    let session = chat_page && state.selected().is_some();
    let mut toggle = cluster_button(
        "shell-toggle-sidebar",
        if state.collapsed {
            "Show sidebar"
        } else {
            "Hide sidebar"
        },
        Action::ToggleSidebar,
        Glyph::Menu,
        true,
    );
    toggle.style.glyph_size = Some(16);
    let cluster_start: u16 = if cfg!(target_os = "macos") {
        if state.fullscreen { 12 } else { 88 }
    } else {
        10
    };
    let mut cluster = vec![
        toggle,
        gap("shell-titlebar-group-gap", 8),
        cluster_button(
            "shell-back",
            "Back",
            Action::Back,
            Glyph::Back,
            state.can_step(false),
        ),
        gap("shell-titlebar-history-gap", 2),
        cluster_button(
            "shell-forward",
            "Forward",
            Action::Forward,
            Glyph::Forward,
            state.can_step(true),
        ),
    ];
    let mut cluster_end = f32::from(cluster_start) + 24.0 * 3.0 + 8.0 + 2.0;
    // Zeron hides the plus on its blank new-session canvas and outside
    // chats. OpenAgents keeps it, so New chat stays one visible,
    // accessible control on every page.
    {
        cluster.push(gap("shell-titlebar-new-gap", 8));
        cluster.push(cluster_button(
            "shell-new-chat",
            "New chat",
            Action::NewChat,
            Glyph::Plus,
            true,
        ));
        cluster_end += 32.0;
    }
    let sidebar_width = if state.collapsed {
        0.0
    } else {
        state.sidebar_width.clamp(SIDEBAR_MIN, SIDEBAR_MAX)
    };
    // The title follows the controls by the 12-point identity gap, and
    // otherwise begins 16 points into the conversation beside the sidebar.
    let title_start = (sidebar_width + 16.0).max(cluster_end + 12.0);
    let mut lead = title_start - cluster_end;
    let mut part = 0;
    while lead > 0.0 {
        let width = lead.min(128.0);
        cluster.push(gap(
            &format!("shell-titlebar-title-gap-{part}"),
            width as u16,
        ));
        lead -= width;
        part += 1;
    }
    let mut heading = text("shell-page-title", title, TextRole::Body);
    heading.style.text_size = Some(12);
    heading.style.line_height = Some(18);
    heading.style.weight = Some(TextWeight::Medium);
    heading.style.foreground = Some(Color {
        alpha: 217,
        ..text_color()
    });
    let mut identity = vec![heading];
    if session
        && let Some(project) = state
            .selected()
            .and_then(|chat| state.projects.get(&chat.id))
    {
        let mut target = text("shell-page-target", project.clone(), TextRole::Body);
        target.style.text_size = Some(12);
        target.style.line_height = Some(18);
        target.style.weight = Some(TextWeight::Normal);
        target.style.foreground = Some(Color {
            alpha: 128,
            ..muted_color()
        });
        identity.push(target);
    }
    let mut identity = stack(
        "shell-titlebar-identity",
        Axis::Horizontal,
        Space::None,
        identity,
    );
    identity.style.gap_points = Some(6);
    identity.style.padding_points = Some([0, 8, 0, 0]);
    cluster.push(identity);
    let mut header = stack("shell-titlebar", Axis::Horizontal, Space::None, cluster);
    header.style.gap_points = Some(0);
    header.style.min_height = Some(38);
    header.style.padding_points = Some([4, 6, 0, cluster_start]);
    header.style.background = Some(sidebar_color());
    if state.live
        && session
        && let Element::Stack { children, .. } = &mut header.element
    {
        // OpenAgents keeps its chat actions in one trailing 28-point header
        // control, drawn as Zeron's header icon buttons.
        let mut menu = icon_button("chat-menu", "Chat actions", Action::NewChat, Glyph::More);
        if let Element::Button { intent, .. } = &mut menu.element {
            *intent = Intent::Chat {
                action: crate::chat_action::Action::Menu,
            };
        }
        menu.style.min_height = Some(28);
        menu.style.radius = Some(6);
        menu.style.glyph_size = Some(16);
        menu.style.hover_background = Some(selected_color());
        children.push(menu);
    }
    // The engines sit in the sidebar (#10072); the conversation starts at
    // the top of the reading pane.
    let content_header = stack(
        "shell-content-header",
        Axis::Vertical,
        Space::None,
        Vec::new(),
    );
    let body = if prompt {
        crate::screens::root(model, now)
    } else {
        match state.page {
            Page::Chat(_) | Page::Saved => placeholder(state),
            Page::Grid => {
                let mut body = stack(
                    "shell-grid",
                    Axis::Vertical,
                    Space::Sm,
                    vec![
                        text("shell-grid-title", "The Grid", TextRole::Heading),
                        text(
                            "shell-grid-line",
                            if cfg!(windows) {
                                "Playable Verse is not yet available on Windows."
                            } else {
                                "A window into the shared world."
                            },
                            TextRole::Status,
                        ),
                    ],
                );
                body.style.align = Some(TextAlign::Center);
                body
            }
            // The shell puts the Map page's views here ([`crate::route_map`]).
            Page::Map => stack("shell-map", Axis::Vertical, Space::None, vec![]),
            Page::Computers => crate::screens::root(model, now),
            Page::Settings => crate::settings::view(
                &state.settings,
                state.live,
                state.update.as_ref(),
                model,
                now,
            ),
        }
    };
    // The Verse page is the world, edge to edge, with nothing under it
    // (#10116).
    let footer = stack(
        "shell-content-footer",
        Axis::Horizontal,
        Space::Sm,
        if state.page == Page::Grid && !prompt {
            vec![]
        } else {
            vec![text("shell-content-note", "OpenAgents", TextRole::Status)]
        },
    );
    let mut content = stack(
        "shell-content",
        Axis::Vertical,
        Space::None,
        vec![content_header, body, footer],
    );
    content.style.background = Some(if matches!(state.page, Page::Grid) {
        Color {
            alpha: 0,
            ..openagents_chat_app::visual::current().canvas
        }
    } else {
        openagents_chat_app::visual::current().canvas
    });
    content.style.radius = Some(10);
    stack(
        "desktop-window",
        Axis::Vertical,
        Space::None,
        vec![
            header,
            stack(
                "desktop-shell",
                Axis::Horizontal,
                Space::None,
                vec![sidebar(state, model), content],
            ),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_native::{Element, Node};

    #[test]
    fn new_chats_are_bounded_and_reopen_their_section() {
        let mut state = State::default();
        state.closed_sections.insert(Section::Recent);
        for _ in 0..80 {
            state.activate(Action::NewChat);
        }
        assert_eq!(state.chats.len(), SAMPLE_LIMIT);
        assert_eq!(state.selected().expect("selected chat").title, "New chat");
        assert!(!state.closed_sections.contains(&Section::Recent));
        let page = state.page;
        state.activate(Action::SelectChat { id: u64::MAX });
        assert_eq!(state.page, page);
    }

    #[test]
    fn collapse_preserves_width_and_chat_selection() {
        let mut state = State::default();
        state.resize(350.0);
        state.activate(Action::SelectChat { id: 4 });
        state.activate(Action::ToggleSidebar);
        state.activate(Action::ToggleSidebar);
        assert_eq!(state.sidebar_width, 350.0);
        assert_eq!(state.page, Page::Chat(4));
        state.resize(f32::NAN);
        assert_eq!(state.sidebar_width, 350.0);
    }

    #[test]
    fn every_shell_route_validates_and_uses_plain_words() {
        use crate::model::{Agent, Screen};
        let model = Model::new(std::time::Instant::now(), Screen::Home, Agent::Enabled);
        let mut state = State::default();
        for action in [
            Action::Grid,
            Action::Computers,
            Action::Settings,
            Action::NewChat,
            Action::ToggleSidebar,
        ] {
            state.activate(action);
            let root = root(&state, &model, 0);
            for value in crate::screens::words(&root) {
                assert!(crate::words::banned_in(&value).is_empty(), "{value}");
            }
            rust_native::View::new("shell-test", 1, root)
                .validate()
                .expect("valid view");
        }
    }

    #[test]
    fn settings_offers_a_waiting_update() {
        use crate::model::{Agent, Screen};
        let model = Model::new(std::time::Instant::now(), Screen::Home, Agent::Enabled);
        let mut state = State::default();
        state.activate(Action::Settings);
        let plain = root(&state, &model, 0);
        assert!(contains(&plain, "settings-version"));
        assert!(!contains(&plain, "settings-update"));
        for (ready, label) in [
            (true, "Restart to update to 1.1.0"),
            (false, "Download 1.1.0"),
        ] {
            state.update = Some(Update {
                version: "1.1.0".into(),
                ready,
            });
            let view = root(&state, &model, 0);
            let Some(Node {
                element:
                    Element::Button {
                        label: shown,
                        intent,
                        ..
                    },
                ..
            }) = find(&view, "settings-update")
            else {
                panic!("no update button")
            };
            assert_eq!(shown, label);
            assert_eq!(
                *intent,
                Intent::Navigate {
                    action: Action::Update
                }
            );
            for value in crate::screens::words(&view) {
                assert!(crate::words::banned_in(&value).is_empty(), "{value}");
            }
            rust_native::View::new("shell-test", 1, view)
                .validate()
                .expect("valid view");
            state.activate(Action::Update);
            assert_eq!(state.page, Page::Settings);
        }
    }

    #[test]
    fn the_filter_shows_once_there_are_enough_chats() {
        use crate::model::{Agent, Screen};
        let model = Model::new(std::time::Instant::now(), Screen::Home, Agent::Enabled);
        let mut state = State::empty();
        for total in 0..SEARCH_MIN_CHATS {
            state.total_chats = total;
            assert!(
                !contains(&root(&state, &model, 0), "chat-search"),
                "{total}"
            );
        }
        state.total_chats = SEARCH_MIN_CHATS;
        assert!(contains(&root(&state, &model, 0), "chat-search"));
        // A filter already typed stays visible however few chats match.
        state.total_chats = 2;
        state.search = "pla".into();
        assert!(contains(&root(&state, &model, 0), "chat-search"));
    }

    /// The background watchers running here show above the engines from
    /// the host's first answer (#10172); with none, no line.
    #[test]
    fn the_sidebar_counts_the_background_watchers() {
        use crate::control::HostControl;
        use crate::model::{Agent, Refreshed, Screen};
        let mut model = Model::new(std::time::Instant::now(), Screen::Home, Agent::Enabled);
        let state = State::empty();
        assert!(!contains(&root(&state, &model, 0), "sidebar-watchers"));
        let mut host = crate::fake::FakeHost::new("Studio Mac", 0);
        let mut answer = |watchers: Vec<String>| Refreshed {
            status: host.status().unwrap(),
            devices: vec![],
            projects: vec![],
            autostart: host.autostart().unwrap(),
            nearby: None,
            watchers,
            background: None,
        };
        model.host = Some(answer(vec![]));
        assert!(!contains(&root(&state, &model, 0), "sidebar-watchers"));
        model.host = Some(answer(vec!["disk cleanup".into(), "logs".into()]));
        let view = root(&state, &model, 0);
        let Some(Node {
            element: Element::Text { value, .. },
            ..
        }) = find(&view, "sidebar-watchers")
        else {
            panic!("no watchers line")
        };
        assert_eq!(value, "2 background watchers · disk cleanup, logs");
        let bottom = find(&view, "sidebar-bottom").expect("the sidebar's bottom");
        let Element::Stack { children, .. } = &bottom.element else {
            panic!("a stack")
        };
        assert_eq!(children[0].key, "sidebar-watchers");
        for value in crate::screens::words(&view) {
            assert!(crate::words::banned_in(&value).is_empty(), "{value}");
        }
    }

    #[test]
    fn the_map_and_the_verse_open_from_the_footer_beside_settings() {
        use crate::model::{Agent, Screen};
        let model = Model::new(std::time::Instant::now(), Screen::Home, Agent::Enabled);
        let mut state = State::empty();
        let view = root(&state, &model, 0);
        let footer = find(&view, "sidebar-footer").expect("the footer");
        let Element::Stack { children, .. } = &footer.element else {
            panic!("a stack")
        };
        let keys: Vec<_> = children.iter().map(|child| child.key.as_str()).collect();
        assert_eq!(
            keys,
            [
                "sidebar-profile",
                "sidebar-footer-spacer",
                "sidebar-map",
                "sidebar-verse",
                "sidebar-settings"
            ]
        );
        // The Map page (#10085) sits beside Verse.
        let Element::Button { label, intent, .. } = &children[2].element else {
            panic!("a button")
        };
        assert_eq!(label, "Map");
        assert_eq!(
            *intent,
            Intent::Navigate {
                action: Action::Map
            }
        );
        let Element::Button { label, intent, .. } = &children[3].element else {
            panic!("a button")
        };
        assert_eq!(label, "Verse");
        assert_eq!(
            *intent,
            Intent::Navigate {
                action: Action::Grid
            }
        );
        state.activate(Action::Grid);
        let view = root(&state, &model, 0);
        let verse = find(&view, "sidebar-verse").unwrap();
        assert_eq!(verse.style.background, Some(selected_color()));
        let Some(Node {
            element: Element::Text { value, .. },
            ..
        }) = find(&view, "shell-page-title")
        else {
            panic!("a title")
        };
        assert_eq!(value, "Verse");
        state.activate(Action::Map);
        let view = root(&state, &model, 0);
        let map = find(&view, "sidebar-map").unwrap();
        assert_eq!(map.style.background, Some(selected_color()));
        let Some(Node {
            element: Element::Text { value, .. },
            ..
        }) = find(&view, "shell-page-title")
        else {
            panic!("a title")
        };
        assert_eq!(value, "Map");
    }

    fn contains(node: &Node<Intent>, key: &str) -> bool {
        if node.key == key {
            return true;
        }
        match &node.element {
            Element::Stack { children, .. } => children.iter().any(|child| contains(child, key)),
            _ => false,
        }
    }

    fn has_button(node: &Node<Intent>) -> bool {
        match &node.element {
            Element::Button { .. } => true,
            Element::Stack { children, .. } => children.iter().any(has_button),
            _ => false,
        }
    }

    fn find<'a>(node: &'a Node<Intent>, key: &str) -> Option<&'a Node<Intent>> {
        if node.key == key {
            return Some(node);
        }
        match &node.element {
            Element::Stack { children, .. } => children.iter().find_map(|child| find(child, key)),
            _ => None,
        }
    }

    #[test]
    fn the_sidebar_shows_each_engine_in_one_row_and_the_transcript_none() {
        use crate::control::{EngineReport, EngineRoute, RouteUsage, UsageWindow};
        use crate::model::{Agent, Screen};
        let mut model = Model::new(std::time::Instant::now(), Screen::Home, Agent::Enabled);
        let plain = root(&State::default(), &model, 0);
        assert!(contains(&plain, "shell-content-header"));
        assert!(!contains(&plain, "engine-strip"));
        model.engine = Some(EngineReport {
            enabled: true,
            adapter: "microcoder-repository".into(),
            model: "gpt-6-luna".into(),
            routes: vec![EngineRoute {
                provider: "codex".into(),
                name: "Codex".into(),
                model: "gpt-6-luna".into(),
                signed_in: true,
                usage: RouteUsage::Windows {
                    windows: vec![UsageWindow {
                        name: "primary".into(),
                        label: "Primary".into(),
                        used_percent: 72,
                        resets_at: Some(1_791_050_824),
                        resets: Some("2026-10-03 18:07 UTC".into()),
                    }],
                    limit_reached: false,
                    used_percent: 72,
                },
            }],
            accounts: vec![],
            usage_probe: Some(90),
            refresh_due: false,
        });
        model.engine_note = Some("Coder's engine report was unreadable.".into());
        let root = root(&State::empty(), &model, 0);
        // Nothing above the conversation (#10072).
        let header = find(&root, "shell-content-header").expect("the content header");
        assert!(matches!(&header.element, Element::Stack { children, .. } if children.is_empty()));
        assert!(!contains(&root, "engine-strip"));
        // One condensed row per engine above the sidebar's footer.
        let bottom = find(&root, "sidebar-bottom").expect("the sidebar's bottom");
        let Element::Stack { children, .. } = &bottom.element else {
            panic!("a stack")
        };
        assert_eq!(children[0].key, "sidebar-engines");
        assert_eq!(children[1].key, "sidebar-footer");
        let Some(Node {
            element: Element::Button { label, intent, .. },
            ..
        }) = find(&root, "sidebar-engine-0")
        else {
            panic!("no engine row")
        };
        assert_eq!(label, "Codex  gpt-6-luna");
        assert_eq!(
            *intent,
            Intent::Settings {
                action: crate::settings::Action::Pane {
                    pane: crate::settings::Pane::Coder
                }
            }
        );
        assert!(contains(&root, "sidebar-engine-0-meter"));
        let words = crate::screens::words(&root);
        assert!(words.iter().any(|word| word.contains("72%")));
        assert!(words.iter().any(|word| word.contains("unreadable")));
        assert!(!words.iter().any(|word| word.contains("UTC")));
        for value in &words {
            assert!(crate::words::banned_in(value).is_empty(), "{value}");
        }
        // Resets on hover.
        let tip = engine_tooltip(&model, "sidebar-engine-0").expect("a tooltip");
        assert!(tip.contains("Signed in"), "{tip}");
        assert!(
            tip.contains("Primary 72% until 2026-10-03 18:07 UTC"),
            "{tip}"
        );
        assert_eq!(engine_tooltip(&model, "sidebar-engine-9"), None);
        rust_native::View::new("shell-engine", 1, root)
            .validate()
            .expect("valid view");
        // The full report, resets included, on Settings' Coder page.
        let mut state = State::empty();
        state.page = Page::Settings;
        state.settings.pane = crate::settings::Pane::Coder;
        let settings = super::root(&state, &model, 0);
        let strip = find(&settings, "engine-strip").expect("the full report");
        assert!(!has_button(strip));
        assert!(
            crate::screens::words(&settings)
                .iter()
                .any(|word| word.contains("UTC"))
        );
    }

    /// Grok Build, installed here and allowed by default, shows beside the
    /// host's routes as signed in with no meter, rather than hidden: it
    /// reports no usage (#10091).
    #[test]
    fn grok_build_shows_as_an_engine_row_with_no_meter() {
        use crate::control::{EngineAccount, EngineReport, EngineRoute, RouteUsage};
        use crate::model::{Agent, Screen};
        let mut model = Model::new(std::time::Instant::now(), Screen::Home, Agent::Enabled);
        let account = |provider: &str, name: &str, signed_in: bool| EngineAccount {
            provider: provider.into(),
            name: name.into(),
            signed_in,
        };
        model.engine = Some(EngineReport {
            enabled: true,
            adapter: "microcoder-repository".into(),
            model: "gpt-6-luna".into(),
            routes: vec![EngineRoute {
                provider: "codex".into(),
                name: "Codex".into(),
                model: "gpt-6-luna".into(),
                signed_in: true,
                usage: RouteUsage::Off,
            }],
            accounts: vec![
                account("codex", "Codex", true),
                account("claude", "Claude Code", false),
                account("grok", "Grok Build", true),
            ],
            usage_probe: None,
            refresh_due: false,
        });
        let root = root(&State::empty(), &model, 0);
        let Some(Node {
            element: Element::Button { label, .. },
            ..
        }) = find(&root, "sidebar-engine-1")
        else {
            panic!("no Grok Build row")
        };
        assert_eq!(label, "Grok Build  Signed in");
        assert!(!contains(&root, "sidebar-engine-1-meter"));
        // Claude Code, not signed in and named by no route, is left out.
        assert!(!contains(&root, "sidebar-engine-2"));
        assert_eq!(
            engine_tooltip(&model, "sidebar-engine-1").as_deref(),
            Some("Grok Build · Signed in")
        );
        let mut state = State::empty();
        state.page = Page::Settings;
        state.settings.pane = crate::settings::Pane::Coder;
        let settings = super::root(&state, &model, 0);
        assert!(contains(&settings, "engine-account-grok"));
        assert!(!contains(&settings, "engine-account-claude"));
    }
}
