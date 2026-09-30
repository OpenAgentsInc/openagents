//! Settings (#10021, audit CDP-22): appearance, text size, keyboard
//! shortcuts, notifications, phones and computers, and archived chats.
//!
//! What a setting means is shared Rust
//! ([`openagents_chat_app::preferences`], the shortcut list in
//! [`openagents_chat_app::commands::bindings`], and the archive list in
//! [`openagents_chat_app::chat_list::archived`]); this module only shows
//! them. The window keeps the preferences in the settings file's `app`
//! section and applies each change at once. Phones and computers is the
//! existing pairing screen ([`crate::screens`]), so a code shown there is
//! cancelled when the person leaves it for another page, as when they leave
//! the standalone screen (`DSK-01`). The app is dark only.

use crate::chrome::Update;
use crate::model::{Intent, Model};
use openagents_chat_app::preferences::{Change, Preferences, TextSize};
use openagents_chat_app::visual::{MUTED, SELECTED, SIDEBAR, TEXT};
use rust_native::style::{Space, Style, TextAlign, TextWeight};
use rust_native::{Axis, Element, Glyph, Icon, Node, TextRole};
use serde::Serialize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// The most archived chats the page lists; the rest wait for a restore.
pub const ARCHIVED_LIMIT: usize = 50;

/// One page of Settings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Pane {
    #[default]
    Appearance,
    TextSize,
    Shortcuts,
    Notifications,
    Computers,
    Archived,
}

impl Pane {
    pub const ALL: [Pane; 6] = [
        Pane::Appearance,
        Pane::TextSize,
        Pane::Shortcuts,
        Pane::Notifications,
        Pane::Computers,
        Pane::Archived,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Pane::Appearance => "Appearance",
            Pane::TextSize => "Text size",
            Pane::Shortcuts => "Keyboard shortcuts",
            Pane::Notifications => "Notifications",
            Pane::Computers => "Phones and computers",
            Pane::Archived => "Archived chats",
        }
    }

    const fn key(self) -> &'static str {
        match self {
            Pane::Appearance => "appearance",
            Pane::TextSize => "text-size",
            Pane::Shortcuts => "shortcuts",
            Pane::Notifications => "notifications",
            Pane::Computers => "computers",
            Pane::Archived => "archived",
        }
    }

    const fn glyph(self) -> Glyph {
        match self {
            Pane::Appearance => Glyph::Settings,
            Pane::TextSize => Glyph::Edit,
            Pane::Shortcuts => Glyph::Terminal,
            Pane::Notifications => Glyph::Flag,
            Pane::Computers => Glyph::Computer,
            Pane::Archived => Glyph::Archive,
        }
    }
}

/// What a click on Settings asks for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    Pane {
        pane: Pane,
    },
    TextSize {
        size: TextSize,
    },
    ReduceMotion {
        on: bool,
    },
    Notifications {
        on: bool,
    },
    /// Restore the archived chat with this ID.
    Restore {
        chat: String,
    },
}

impl Action {
    /// The preference change this action makes, if any.
    pub fn change(&self) -> Option<Change> {
        match self {
            Action::TextSize { size } => Some(Change::TextSize(*size)),
            Action::ReduceMotion { on } => Some(Change::ReduceMotion(*on)),
            Action::Notifications { on } => Some(Change::Notifications(*on)),
            Action::Pane { .. } | Action::Restore { .. } => None,
        }
    }
}

/// An archived chat the page lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Archived {
    pub id: String,
    pub title: String,
}

/// Settings' presentation state and the preferences it shows.
#[derive(Clone, Debug, Default)]
pub struct Settings {
    pub pane: Pane,
    pub preferences: Preferences,
    /// The archived chats, from the chat list ([`Archived`]).
    pub archived: Vec<Archived>,
    /// Archived chats with a restore on its way.
    pub restoring: std::collections::BTreeSet<String>,
    /// A line about the last save, when it failed.
    pub notice: Option<String>,
    /// The settings file the window keeps them in; none in a capture or a
    /// test, where they live only in memory.
    pub file: Option<std::path::PathBuf>,
    /// The "Reduce motion" preference, read by the Grid behind the window
    /// on its own schedule ([`crate::backdrop`]).
    motion: Arc<AtomicBool>,
}

impl Settings {
    /// Settings showing `preferences`.
    pub fn with(preferences: Preferences) -> Settings {
        let settings = Settings {
            preferences,
            ..Settings::default()
        };
        settings
            .motion
            .store(preferences.reduce_motion, Ordering::Relaxed);
        settings
    }

    /// Shows `preferences` in place of the ones shown.
    pub fn replace(&mut self, preferences: Preferences) {
        self.preferences = preferences;
        self.motion
            .store(preferences.reduce_motion, Ordering::Relaxed);
    }

    /// Makes `change`; `true` when anything changed.
    pub fn apply(&mut self, change: Change) -> bool {
        let changed = self.preferences.apply(change);
        self.motion
            .store(self.preferences.reduce_motion, Ordering::Relaxed);
        changed
    }

    /// The "Reduce motion" preference, shared with the backdrop.
    pub fn motion(&self) -> Arc<AtomicBool> {
        self.motion.clone()
    }

    /// Replaces the archived list; a chat no longer archived is no longer
    /// being restored.
    pub fn set_archived(&mut self, archived: Vec<Archived>) {
        self.restoring
            .retain(|id| archived.iter().any(|row| &row.id == id));
        self.archived = archived;
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
    if role == TextRole::Status {
        node.style.foreground = Some(MUTED);
    }
    node
}

fn title(key: &str, value: &str) -> Node<Intent> {
    let mut node = text(key, value, TextRole::Body);
    node.style.weight = Some(TextWeight::Bold);
    node.style.foreground = Some(TEXT);
    node
}

fn button(
    key: &str,
    label: impl Into<String>,
    action: Action,
    glyph: Option<Glyph>,
    selected: bool,
    enabled: bool,
) -> Node<Intent> {
    let mut node = node(
        key,
        Element::Button {
            label: label.into(),
            enabled,
            icon: glyph.map(|glyph| Icon {
                glyph,
                circular: false,
                pill: false,
            }),
            shortcut: None,
            intent: Intent::Settings { action },
        },
    );
    node.style = Style {
        background: Some(if selected { SELECTED } else { SIDEBAR }),
        foreground: Some(if selected { TEXT } else { MUTED }),
        align: Some(TextAlign::Start),
        weight: Some(TextWeight::Normal),
        radius: Some(8),
        button_padding: Some([10, 6]),
        ..Style::default()
    };
    node
}

/// A choice in a row of choices: as wide as its label.
fn chip(mut node: Node<Intent>) -> Node<Intent> {
    node.style.intrinsic_width = Some(true);
    // A start-aligned button fills its row.
    node.style.align = None;
    node
}

fn toggle(key: &str, label: &str, on: bool, action: Action) -> Node<Intent> {
    chip(button(
        key,
        label,
        action,
        Some(if on { Glyph::Checked } else { Glyph::Unchecked }),
        on,
        true,
    ))
}

fn appearance(settings: &Settings) -> Vec<Node<Intent>> {
    let on = settings.preferences.reduce_motion;
    vec![
        title("settings-theme-title", "Theme"),
        text(
            "settings-theme-line",
            "Dark. OpenAgents always uses its dark theme, whatever this computer is set to.",
            TextRole::Status,
        ),
        title("settings-motion-title", "Motion"),
        toggle(
            "settings-reduce-motion",
            "Reduce motion",
            on,
            Action::ReduceMotion { on: !on },
        ),
        text(
            "settings-motion-line",
            "Keeps the Grid behind the window still. Your computer's own Reduce motion setting also does.",
            TextRole::Status,
        ),
    ]
}

fn text_size(settings: &Settings) -> Vec<Node<Intent>> {
    let current = settings.preferences.text_size;
    let choices = TextSize::ALL
        .into_iter()
        .map(|size| {
            chip(button(
                &format!("settings-text-{}", size.label().to_lowercase()),
                size.label(),
                Action::TextSize { size },
                (size == current).then_some(Glyph::Check),
                size == current,
                true,
            ))
        })
        .collect();
    vec![
        title("settings-text-title", "Text size"),
        stack("settings-text-choices", Axis::Wrap, Space::Sm, choices),
        text(
            "settings-text-sample",
            "Chats and these pages draw their text at this size.",
            TextRole::Body,
        ),
        text(
            "settings-text-line",
            format!("{}% of the default size.", current.percent()),
            TextRole::Status,
        ),
    ]
}

/// The window's own shortcut besides the shared ones.
const SIDEBAR_SHORTCUT: openagents_chat_app::commands::Binding =
    openagents_chat_app::commands::Binding {
        label: "Show or hide the sidebar",
        key: "B",
        command: true,
        shift: false,
        action: openagents_chat_app::commands::Action::Dismiss,
    };

fn shortcuts() -> Vec<Node<Intent>> {
    let macos = cfg!(target_os = "macos");
    let mut rows = vec![title("settings-shortcuts-title", "Keyboard shortcuts")];
    let mut bindings = openagents_chat_app::commands::bindings();
    bindings.push(SIDEBAR_SHORTCUT);
    for (index, binding) in bindings.iter().enumerate() {
        let mut keys = text(
            &format!("settings-shortcut-{index}-keys"),
            openagents_chat_app::commands::chord(binding, macos),
            TextRole::Body,
        );
        keys.style.align = Some(TextAlign::End);
        keys.style.foreground = Some(TEXT);
        rows.push(stack(
            &format!("settings-shortcut-{index}"),
            Axis::Horizontal,
            Space::Md,
            vec![
                text(
                    &format!("settings-shortcut-{index}-label"),
                    binding.label,
                    TextRole::Body,
                ),
                keys,
            ],
        ));
    }
    rows.push(text(
        "settings-shortcuts-line",
        "Shortcuts can't be changed yet.",
        TextRole::Status,
    ));
    rows
}

fn notifications(settings: &Settings) -> Vec<Node<Intent>> {
    let on = settings.preferences.notifications;
    let mut rows = vec![
        title("settings-notifications-title", "Notifications"),
        toggle(
            "settings-notifications",
            "Notify me about Coder",
            on,
            Action::Notifications { on: !on },
        ),
        text(
            "settings-notifications-line",
            "When Coder asks you something, finishes, or fails while OpenAgents isn't in front. A notification names the chat, never a message.",
            TextRole::Status,
        ),
    ];
    if !cfg!(target_os = "linux") {
        rows.push(text(
            "settings-notifications-platform",
            format!(
                "OpenAgents doesn't show notifications on this {} yet.",
                crate::words::COMPUTER
            ),
            TextRole::Status,
        ));
    }
    rows
}

fn archived(settings: &Settings) -> Vec<Node<Intent>> {
    let mut rows = vec![title("settings-archived-title", "Archived chats")];
    if settings.archived.is_empty() {
        rows.push(text(
            "settings-archived-empty",
            "No archived chats. Archiving a chat keeps it here.",
            TextRole::Status,
        ));
        return rows;
    }
    for (index, chat) in settings.archived.iter().take(ARCHIVED_LIMIT).enumerate() {
        let restoring = settings.restoring.contains(&chat.id);
        let mut name = text(
            &format!("settings-archived-{index}-title"),
            chat.title.clone(),
            TextRole::Body,
        );
        name.style.foreground = Some(TEXT);
        rows.push(stack(
            &format!("settings-archived-{index}"),
            Axis::Horizontal,
            Space::Md,
            vec![
                name,
                chip(button(
                    &format!("settings-archived-{index}-restore"),
                    if restoring { "Restoring…" } else { "Restore" },
                    Action::Restore {
                        chat: chat.id.clone(),
                    },
                    Some(Glyph::Restore),
                    false,
                    !restoring,
                )),
            ],
        ));
    }
    if settings.archived.len() > ARCHIVED_LIMIT {
        rows.push(text(
            "settings-archived-more",
            format!(
                "{} more. Restore a chat to see the next.",
                settings.archived.len() - ARCHIVED_LIMIT
            ),
            TextRole::Status,
        ));
    }
    rows
}

/// Settings' version line and, when a newer release is offered, its line
/// and button.
fn update_rows(update: Option<&Update>) -> Vec<Node<Intent>> {
    let mut rows = vec![text(
        "settings-version",
        format!("Version {}", env!("CARGO_PKG_VERSION")),
        TextRole::Status,
    )];
    if let Some(update) = update {
        let (line, label) = if update.ready {
            (
                format!("OpenAgents {} is ready.", update.version),
                format!("Restart to update to {}", update.version),
            )
        } else {
            (
                format!("OpenAgents {} is available.", update.version),
                format!("Download {}", update.version),
            )
        };
        rows.push(text("settings-update-line", line, TextRole::Body));
        let mut button = node(
            "settings-update",
            Element::Button {
                label,
                enabled: true,
                icon: None,
                shortcut: None,
                intent: Intent::Navigate {
                    action: crate::chrome::Action::Update,
                },
            },
        );
        button.style.background = Some(SELECTED);
        button.style.foreground = Some(TEXT);
        button.style.radius = Some(8);
        rows.push(button);
    }
    rows
}

/// The Settings page: its pages as a row of choices, the chosen page, and
/// the version.
pub fn view(
    settings: &Settings,
    live: bool,
    update: Option<&Update>,
    model: &Model,
    now: u64,
) -> Node<Intent> {
    let tabs = Pane::ALL
        .into_iter()
        .map(|pane| {
            chip(button(
                &format!("settings-pane-{}", pane.key()),
                pane.label(),
                Action::Pane { pane },
                Some(pane.glyph()),
                pane == settings.pane,
                true,
            ))
        })
        .collect();
    let body = match settings.pane {
        Pane::Appearance => appearance(settings),
        Pane::TextSize => text_size(settings),
        Pane::Shortcuts => shortcuts(),
        Pane::Notifications => notifications(settings),
        Pane::Computers => vec![crate::screens::root(model, now)],
        Pane::Archived => archived(settings),
    };
    let mut rows = vec![
        stack("settings-panes", Axis::Wrap, Space::Sm, tabs),
        stack("settings-page", Axis::Vertical, Space::Sm, body),
    ];
    if let Some(notice) = &settings.notice {
        rows.push(text("settings-notice", notice.clone(), TextRole::Status));
    }
    rows.push(text(
        "shell-settings-line",
        if live {
            "Chats are encrypted on this computer."
        } else {
            "Chats in this preview are examples."
        },
        TextRole::Status,
    ));
    rows.extend(update_rows(update));
    stack("shell-settings", Axis::Vertical, Space::Md, rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Agent, Screen};

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
    fn every_page_validates_and_uses_plain_words() {
        let model = Model::new(std::time::Instant::now(), Screen::Home, Agent::Enabled);
        let mut settings = Settings::default();
        settings.set_archived(
            (0..ARCHIVED_LIMIT + 3)
                .map(|n| Archived {
                    id: format!("{n:032x}"),
                    title: format!("Old chat {n}"),
                })
                .collect(),
        );
        for pane in Pane::ALL {
            settings.pane = pane;
            let view = view(&settings, true, None, &model, 0);
            for value in crate::screens::words(&view) {
                assert!(crate::words::banned_in(&value).is_empty(), "{value}");
            }
            assert!(find(&view, &format!("settings-pane-{}", pane.key())).is_some());
            rust_native::View::new("settings-test", 1, view)
                .validate()
                .expect("valid view");
        }
        settings.pane = Pane::Archived;
        let view = view(&settings, true, None, &model, 0);
        assert!(find(&view, "settings-archived-more").is_some());
        assert!(find(&view, &format!("settings-archived-{ARCHIVED_LIMIT}")).is_none());
    }

    #[test]
    fn toggles_ask_for_the_opposite_and_share_reduce_motion() {
        let model = Model::new(std::time::Instant::now(), Screen::Home, Agent::Enabled);
        let mut settings = Settings::with(Preferences::default());
        let motion = settings.motion();
        settings.pane = Pane::Appearance;
        let view = view(&settings, true, None, &model, 0);
        let Some(Node {
            element: Element::Button { intent, .. },
            ..
        }) = find(&view, "settings-reduce-motion")
        else {
            panic!("no toggle")
        };
        let Intent::Settings { action } = intent.clone() else {
            panic!("not a setting")
        };
        assert_eq!(action, Action::ReduceMotion { on: true });
        assert!(settings.apply(action.change().unwrap()));
        assert!(motion.load(Ordering::Relaxed));
        assert!(!settings.apply(action.change().unwrap()));
    }

    #[test]
    fn a_restored_chat_leaves_the_restoring_set() {
        let mut settings = Settings::default();
        let row = Archived {
            id: "a".repeat(32),
            title: "Old".into(),
        };
        settings.set_archived(vec![row.clone()]);
        settings.restoring.insert(row.id.clone());
        settings.set_archived(vec![row.clone()]);
        assert!(settings.restoring.contains(&row.id));
        settings.set_archived(vec![]);
        assert!(settings.restoring.is_empty());
    }
}
