//! Shared chat commands, key scopes, and bounded overlay navigation.
use openagents_chat::basic_chats::Summary;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    NewChat,
    Search,
    Settings,
    Computers,
    Grid,
    /// The desktop's Map page (#10085).
    Map,
    Saved,
    Stop,
    Palette,
    Menu,
    Switch(String),
    Cycle(bool),
    Rename,
    Pin,
    Archive,
    Restore,
    Dismiss,
    /// **Give feedback** on the selected text ([`crate::feedback`]).
    Feedback,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Window,
    Editor,
    Composing,
    Overlay,
}

/// The platform adapter supplies Cmd on macOS and Ctrl on other desktops.
pub fn shortcut(
    key: &str,
    command: bool,
    control: bool,
    shift: bool,
    scope: Scope,
) -> Option<Action> {
    if matches!(scope, Scope::Composing | Scope::Overlay) {
        return None;
    }
    if key == "ContextMenu" || (key == "F10" && shift) {
        return Some(Action::Menu);
    }
    if key == "Tab" {
        return control.then_some(Action::Cycle(shift));
    }
    if !command {
        return None;
    }
    if shift {
        return None;
    }
    match key.to_ascii_lowercase().as_str() {
        "n" => Some(Action::NewChat),
        "f" => Some(Action::Search),
        "k" => Some(Action::Palette),
        "," => Some(Action::Settings),
        "." => Some(Action::Stop),
        _ => None,
    }
}

/// Display-only labels for the shortcuts admitted by `shortcut`.
pub fn badge(action: &Action, macos: bool) -> Option<&'static str> {
    match (action, macos) {
        (Action::NewChat, true) => Some("⌘N"),
        (Action::NewChat, false) => Some("Ctrl+N"),
        (Action::Settings, true) => Some("⌘,"),
        (Action::Settings, false) => Some("Ctrl+,"),
        _ => None,
    }
}

/// One shortcut [`shortcut`] admits, as a Settings list shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    pub label: &'static str,
    pub key: &'static str,
    /// Cmd on macOS, Ctrl elsewhere.
    pub command: bool,
    /// The physical Control key on every platform.
    pub control: bool,
    pub shift: bool,
    pub action: Action,
}

/// Every shortcut [`shortcut`] admits in a window, in the order a list
/// shows them. Read-only: the keys are not rebindable.
pub fn bindings() -> Vec<Binding> {
    let binding = |label, key, command, shift, action| Binding {
        label,
        key,
        command,
        control: key == "Tab",
        shift,
        action,
    };
    vec![
        binding("New chat", "N", true, false, Action::NewChat),
        binding("Search chats", "F", true, false, Action::Search),
        binding("Commands", "K", true, false, Action::Palette),
        binding("Settings", ",", true, false, Action::Settings),
        binding("Stop receiving reply", ".", true, false, Action::Stop),
        binding("Next chat", "Tab", false, false, Action::Cycle(false)),
        binding("Previous chat", "Tab", false, true, Action::Cycle(true)),
        binding("Chat actions", "F10", false, true, Action::Menu),
    ]
}

/// How `binding` reads on this platform. Chat cycling uses Control everywhere.
/// Use words because not every app font draws the ⌘ symbol.
pub fn chord(binding: &Binding, macos: bool) -> String {
    let mut parts = vec![];
    if binding.control {
        parts.push("Ctrl");
    } else if binding.command {
        parts.push(if macos { "Cmd" } else { "Ctrl" });
    }
    if binding.shift {
        parts.push("Shift");
    }
    parts.push(binding.key);
    parts.join("+")
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub key: String,
    pub label: String,
    pub hint: &'static str,
    pub action: Action,
    pub enabled: bool,
}
fn entry(key: &str, label: &str, hint: &'static str, action: Action, enabled: bool) -> Entry {
    Entry {
        key: key.into(),
        label: label.into(),
        hint,
        action,
        enabled,
    }
}
pub fn registry(chats: &[Summary], selected: Option<&str>, busy: bool) -> Vec<Entry> {
    let current = chats.iter().find(|row| Some(row.id.as_str()) == selected);
    let mut entries = vec![
        entry("new", "New chat", "Cmd/Ctrl+N", Action::NewChat, true),
        entry("search", "Search chats", "Cmd/Ctrl+F", Action::Search, true),
        entry("settings", "Settings", "Cmd/Ctrl+,", Action::Settings, true),
        entry(
            "map",
            "Open the map",
            "How OpenAgents routes requests, and its gaps",
            Action::Map,
            true,
        ),
        entry(
            "stop",
            "Stop receiving reply",
            "Cmd/Ctrl+.",
            Action::Stop,
            busy,
        ),
        entry(
            "rename",
            "Rename chat",
            "Edit this chat's saved title",
            Action::Rename,
            current.is_some(),
        ),
        entry(
            "pin",
            if current.is_some_and(|row| row.pinned) {
                "Unpin chat"
            } else {
                "Pin chat"
            },
            "Keep this chat at the top of the list",
            Action::Pin,
            current.is_some(),
        ),
        entry(
            "archive",
            "Archive chat",
            "Keep the conversation in Archived",
            Action::Archive,
            current.is_some_and(|row| !row.archived),
        ),
        entry(
            "restore",
            "Restore chat",
            "Move this conversation out of Archived",
            Action::Restore,
            current.is_some_and(|row| row.archived),
        ),
    ];
    entries.extend(crate::chat_list::search(chats, "").into_iter().map(|row| {
        entry(
            &format!("switch-{}", row.id),
            &format!("Switch to {}", row.title),
            "Open this saved conversation",
            Action::Switch(row.id.clone()),
            true,
        )
    }));
    entries
}

/// The context menu's **Give feedback** item, offered first while text is
/// selected (#10127).
pub fn feedback_entry() -> Entry {
    entry(
        "feedback",
        playtest::feedback::BUTTON,
        "Comment on the selected text",
        Action::Feedback,
        true,
    )
}

/// Local profile actions reuse the desktop's existing navigation authority.
pub fn profile_registry() -> Vec<Entry> {
    vec![
        entry(
            "computers",
            "Phones and computers",
            "",
            Action::Computers,
            true,
        ),
        entry("grid", "Verse", "", Action::Grid, true),
        entry("map", "Map", "", Action::Map, true),
        entry("saved", "Saved sessions", "", Action::Saved, true),
        entry("commands", "Commands", "Cmd/Ctrl+K", Action::Palette, true),
    ]
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Palette,
    Menu,
    ConfirmArchive,
    Profile,
}
#[derive(Default)]
pub struct Overlay {
    pub kind: Option<Kind>,
    pub query: String,
    pub selected: usize,
}
impl Overlay {
    pub fn open(&mut self, kind: Kind) {
        self.kind = Some(kind);
        self.query.clear();
        self.selected = 0;
    }
    pub fn close(&mut self) {
        self.kind = None;
        self.query.clear();
        self.selected = 0;
    }
    pub fn entries(&self, registry: &[Entry]) -> Vec<Entry> {
        if self.kind == Some(Kind::ConfirmArchive) {
            return vec![
                entry(
                    "confirm",
                    "Archive this chat",
                    "Find it later in Archived",
                    Action::Archive,
                    registry
                        .iter()
                        .any(|e| e.action == Action::Archive && e.enabled),
                ),
                entry("dismiss", "Cancel", "Escape", Action::Dismiss, true),
            ];
        }
        let query = self.query.trim().to_lowercase();
        registry
            .iter()
            .filter(|entry| {
                (self.kind != Some(Kind::Menu)
                    || matches!(
                        entry.action,
                        Action::Rename
                            | Action::Pin
                            | Action::Archive
                            | Action::Restore
                            | Action::Feedback
                    ))
                    && (query.is_empty() || entry.label.to_lowercase().contains(&query))
            })
            .cloned()
            .collect()
    }
    pub fn navigate_entries(&mut self, backwards: bool, entries: &[Entry]) {
        for _ in 0..entries.len() {
            self.navigate(backwards, entries.len());
            if entries[self.selected].enabled {
                break;
            }
        }
    }
    pub fn navigate(&mut self, backwards: bool, count: usize) {
        self.selected = if count == 0 {
            0
        } else if backwards {
            (self.selected + count - 1) % count
        } else {
            (self.selected + 1) % count
        };
    }
    pub fn window(&self, count: usize) -> std::ops::Range<usize> {
        self.window_at(count, 5)
    }
    pub fn window_at(&self, count: usize, visible: usize) -> std::ops::Range<usize> {
        let visible = visible.clamp(1, 16);
        let start = self
            .selected
            .min(count.saturating_sub(1))
            .saturating_sub(visible / 2)
            .min(count.saturating_sub(visible));
        start..(start + visible).min(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keys_respect_composition_and_overlay_scopes() {
        for scope in [Scope::Window, Scope::Editor] {
            assert_eq!(
                shortcut("n", true, true, false, scope),
                Some(Action::NewChat)
            );
            assert_eq!(
                shortcut("k", true, true, false, scope),
                Some(Action::Palette)
            );
        }
        for scope in [Scope::Composing, Scope::Overlay] {
            assert!(shortcut("n", true, true, false, scope).is_none());
        }
        assert!(shortcut("n", false, false, false, Scope::Window).is_none());
        assert_eq!(
            shortcut("f", true, true, false, Scope::Editor),
            Some(Action::Search)
        );
        assert_eq!(
            shortcut(",", true, true, false, Scope::Window),
            Some(Action::Settings)
        );
        assert_eq!(
            shortcut(".", true, true, false, Scope::Editor),
            Some(Action::Stop)
        );
        assert_eq!(
            shortcut("Tab", false, true, true, Scope::Window),
            Some(Action::Cycle(true))
        );
    }
    #[test]
    fn every_listed_shortcut_is_one_the_window_admits() {
        let bindings = bindings();
        assert_eq!(bindings.len(), 8);
        for binding in &bindings {
            assert_eq!(
                shortcut(
                    binding.key,
                    binding.command,
                    binding.control,
                    binding.shift,
                    Scope::Window
                ),
                Some(binding.action.clone()),
                "{}",
                binding.label
            );
        }
        let previous = &bindings[6];
        assert_eq!(chord(previous, true), "Ctrl+Shift+Tab");
        assert_eq!(chord(previous, false), "Ctrl+Shift+Tab");
        assert_eq!(chord(&bindings[0], true), "Cmd+N");
        assert_eq!(chord(&bindings[7], false), "Shift+F10");
    }

    #[test]
    fn chat_cycle_requires_physical_control_on_every_platform() {
        for command in [false, true] {
            for shift in [false, true] {
                for scope in [Scope::Window, Scope::Editor] {
                    assert_eq!(
                        shortcut("Tab", command, true, shift, scope),
                        Some(Action::Cycle(shift))
                    );
                    assert_eq!(shortcut("Tab", command, false, shift, scope), None);
                }
                for scope in [Scope::Composing, Scope::Overlay] {
                    assert_eq!(shortcut("Tab", command, true, shift, scope), None);
                }
            }
        }
        for binding in bindings()
            .iter()
            .filter(|b| matches!(b.action, Action::Cycle(_)))
        {
            assert!(binding.control);
            assert!(!binding.command);
            for macos in [false, true] {
                assert_eq!(
                    chord(binding, macos),
                    if binding.shift {
                        "Ctrl+Shift+Tab"
                    } else {
                        "Ctrl+Tab"
                    }
                );
            }
        }
    }

    #[test]
    fn overlays_wrap_focus_and_keep_windows_bounded() {
        let entries = registry(&[], None, false);
        let mut overlay = Overlay::default();
        overlay.open(Kind::Palette);
        let choices = overlay.entries(&entries);
        assert!(
            !choices
                .iter()
                .find(|e| e.action == Action::Stop)
                .unwrap()
                .enabled
        );
        overlay.navigate(true, choices.len());
        assert_eq!(overlay.selected, choices.len() - 1);
        assert!(overlay.window(512).len() <= 5);
        assert_eq!(overlay.window_at(512, 10).len(), 10);
        assert_eq!(overlay.window_at(512, usize::MAX).len(), 16);
        assert_eq!(overlay.window_at(0, 10).len(), 0);
        overlay.query = "setting".into();
        assert_eq!(overlay.entries(&entries).len(), 1);
        overlay.open(Kind::ConfirmArchive);
        assert!(!overlay.entries(&entries)[0].enabled);
        overlay.close();
        assert!(overlay.kind.is_none());
    }
}
