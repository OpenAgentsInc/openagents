//! Desktop navigation and sample sidebar content, projected as semantic views.
//!
//! The layout reimplements Zeron's shell and sidebar design. Sample chats
//! exist only for this window and never create a task or a saved conversation.

use crate::model::{Intent, Model};
use rust_native::style::{Color, Space, Style, TextAlign, TextWeight};
use rust_native::{Axis, Element, Glyph, Icon, Node, TextRole};
use serde::Serialize;
use std::collections::BTreeSet;

pub const MARK: &str = "openagents-mark";
pub const SIDEBAR_MIN: f32 = 224.0;
pub const SIDEBAR_MAX: f32 = 400.0;
pub const SIDEBAR_DEFAULT: f32 = 256.0;
const SAMPLE_LIMIT: usize = 40;
const SIDEBAR: Color = openagents_chat_app::visual::SIDEBAR;
const SELECTED: Color = openagents_chat_app::visual::SELECTED;
const TEXT: Color = openagents_chat_app::visual::TEXT;
const MUTED: Color = openagents_chat_app::visual::MUTED;
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
    ToggleSection { section: Section },
    SelectChat { id: u64 },
    NewChat,
    Saved,
    Grid,
    Computers,
    Settings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Chat(u64),
    Saved,
    Grid,
    Computers,
    Settings,
}

#[derive(Clone, Debug)]
pub struct Chat {
    pub id: u64,
    pub title: String,
    pub detail: &'static str,
    pub section: Section,
}

/// Presentation state for the shell. Computer state remains in `Model`.
#[derive(Clone, Debug)]
pub struct State {
    pub live: bool,
    pub page: Page,
    pub sidebar_width: f32,
    pub collapsed: bool,
    pub closed_sections: BTreeSet<Section>,
    pub chats: Vec<Chat>,
    pub search: String,
    pub projects: std::collections::BTreeMap<u64, String>,
    next_chat: u64,
}

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
            search: String::new(),
            projects: std::collections::BTreeMap::new(),
            page: Page::Chat(1),
            sidebar_width: SIDEBAR_DEFAULT,
            collapsed: false,
            closed_sections: BTreeSet::from([Section::Archived]),
            chats: examples
                .into_iter()
                .enumerate()
                .map(|(index, (title, detail, section))| Chat {
                    id: index as u64 + 1,
                    title: title.into(),
                    detail,
                    section,
                })
                .collect(),
            next_chat: 18,
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
                        section: Section::Recent,
                    },
                );
                self.closed_sections.remove(&Section::Recent);
                self.page = Page::Chat(id);
            }
            Action::Saved => self.page = Page::Saved,
            Action::Grid => self.page = Page::Grid,
            Action::Computers => self.page = Page::Computers,
            Action::Settings => self.page = Page::Settings,
        }
    }

    pub fn resize(&mut self, width: f32) {
        if width.is_finite() {
            self.sidebar_width = width.clamp(SIDEBAR_MIN, SIDEBAR_MAX);
        }
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
    if key.starts_with("sidebar-group-")
        || key.starts_with("project-group-")
        || key == "sidebar-body"
    {
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
        background: Some(if selected { SELECTED } else { SIDEBAR }),
        foreground: Some(if selected { TEXT } else { MUTED }),
        align: Some(TextAlign::Start),
        weight: Some(TextWeight::Normal),
        radius: Some(8),
        text_size: Some(12),
        line_height: Some(16),
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
    node.style.foreground = Some(MUTED);
    node
}

fn sidebar(state: &State) -> Node<Intent> {
    let header_rows = if state.live {
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
        if state.live && section == Section::Recent {
            // Bound headings as well as chat rows within the semantic node budget.
            let mut projects = std::collections::BTreeMap::<&str, Vec<&Chat>>::new();
            for chat in &state.chats {
                if let Some(project) = state.projects.get(&chat.id) {
                    projects.entry(project).or_default().push(chat);
                }
            }
            let mut overflow = vec![text("project-more", "More projects", TextRole::Heading)];
            for (index, (project, chats)) in projects.into_iter().enumerate() {
                let rows = chats.into_iter().map(|chat| {
                    action(
                        &format!("sidebar-chat-{}", chat.id),
                        if index < 64 {
                            format!("{}\n{}", chat.title, chat.detail)
                        } else {
                            format!("{} · {project}\n{}", chat.title, chat.detail)
                        },
                        Action::SelectChat { id: chat.id },
                        None,
                        state.page == Page::Chat(chat.id),
                    )
                });
                if index < 64 {
                    let mut group = vec![text(
                        &format!("project-{}", groups.len()),
                        project,
                        TextRole::Heading,
                    )];
                    group.extend(rows);
                    groups.push(stack(
                        &format!("project-group-{}", groups.len()),
                        Axis::Vertical,
                        Space::Xs,
                        group,
                    ));
                } else {
                    overflow.extend(rows);
                }
            }
            if overflow.len() > 1 {
                groups.push(stack(
                    "project-more-group",
                    Axis::Vertical,
                    Space::Xs,
                    overflow,
                ));
            }
        }
        let closed = state.closed_sections.contains(&section);
        let count = state
            .chats
            .iter()
            .filter(|chat| chat.section == section && !state.projects.contains_key(&chat.id))
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
                    .filter(|chat| {
                        chat.section == section && !state.projects.contains_key(&chat.id)
                    })
                    .map(|chat| {
                        action(
                            &format!("sidebar-chat-{}", chat.id),
                            format!("{}\n{}", chat.title, chat.detail),
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
    let mut command = icon_button(
        "sidebar-commands",
        "Commands · Cmd/Ctrl+K",
        Action::NewChat,
        Glyph::Terminal,
    );
    if let Element::Button { intent, .. } = &mut command.element {
        *intent = Intent::Chat {
            action: crate::chat_action::Action::Palette,
        };
    }
    let footer = stack(
        "sidebar-footer",
        Axis::Horizontal,
        Space::Sm,
        vec![
            icon_button(
                "sidebar-computers",
                "Phones and computers",
                Action::Computers,
                Glyph::Computer,
            ),
            icon_button("sidebar-grid", "The Grid", Action::Grid, Glyph::Cloud),
            icon_button(
                "sidebar-saved",
                "Saved sessions",
                Action::Saved,
                Glyph::History,
            ),
            command,
            icon_button(
                "sidebar-settings",
                "Settings",
                Action::Settings,
                Glyph::Menu,
            ),
        ],
    );
    let mut pane = stack(
        "shell-sidebar",
        Axis::Vertical,
        Space::None,
        if state.collapsed {
            ["sidebar-header", "sidebar-body", "sidebar-footer"]
                .into_iter()
                .map(|key| stack(key, Axis::Vertical, Space::None, vec![]))
                .collect()
        } else {
            vec![header, body, footer]
        },
    );
    pane.style.background = Some(SIDEBAR);
    pane
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
            Page::Grid => "The Grid".into(),
            Page::Computers => "Phones and computers".into(),
            Page::Settings => "Settings".into(),
        }
    };
    let mut heading = text("shell-page-title", title, TextRole::Body);
    heading.style.text_size = Some(12);
    heading.style.line_height = Some(18);
    let mut header = stack(
        "shell-content-header",
        Axis::Horizontal,
        Space::Sm,
        vec![
            icon_button(
                "shell-toggle-sidebar",
                if state.collapsed {
                    "Show sidebar"
                } else {
                    "Hide sidebar"
                },
                Action::ToggleSidebar,
                Glyph::Menu,
            ),
            heading,
            icon_button(
                "shell-new-chat",
                "New chat",
                Action::NewChat,
                Glyph::Compose,
            ),
        ],
    );
    header.style.min_height = Some(28);
    if state.live
        && state.selected().is_some()
        && !prompt
        && matches!(state.page, Page::Chat(_))
        && let Element::Stack { children, .. } = &mut header.element
    {
        let mut menu = icon_button("chat-menu", "Chat actions", Action::NewChat, Glyph::More);
        if let Element::Button { intent, .. } = &mut menu.element {
            *intent = Intent::Chat {
                action: crate::chat_action::Action::Menu,
            };
        }
        children.push(menu);
    }
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
                            "A window into the shared world.",
                            TextRole::Status,
                        ),
                    ],
                );
                body.style.align = Some(TextAlign::Center);
                body
            }
            Page::Computers => crate::screens::root(model, now),
            Page::Settings => stack(
                "shell-settings",
                Axis::Vertical,
                Space::Md,
                vec![
                    text(
                        "shell-settings-title",
                        "OpenAgents desktop",
                        TextRole::Heading,
                    ),
                    text(
                        "shell-settings-line",
                        if state.live {
                            "Chats are encrypted on this computer. Your connected phones and computers use your existing setup."
                        } else {
                            "Chats in this preview are examples. Your connected phones and computers use your existing setup."
                        },
                        TextRole::Body,
                    ),
                    action(
                        "settings-computers",
                        "Manage phones and computers",
                        Action::Computers,
                        Some(Glyph::Computer),
                        false,
                    ),
                ],
            ),
        }
    };
    let footer = stack(
        "shell-content-footer",
        Axis::Horizontal,
        Space::Sm,
        vec![text(
            "shell-content-note",
            if matches!(state.page, Page::Chat(_)) {
                "Sample conversation"
            } else {
                "OpenAgents"
            },
            TextRole::Status,
        )],
    );
    let mut content = stack(
        "shell-content",
        Axis::Vertical,
        Space::None,
        vec![header, body, footer],
    );
    content.style.background = Some(if matches!(state.page, Page::Grid) {
        Color {
            alpha: 0,
            ..openagents_chat_app::visual::CANVAS
        }
    } else {
        openagents_chat_app::visual::CANVAS
    });
    stack(
        "desktop-shell",
        Axis::Horizontal,
        Space::None,
        vec![sidebar(state), content],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
