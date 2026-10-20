//! Settings → Shortcuts: the app's rebindable chords, with the desktop's
//! row labels and groups (`openagents-ui`'s `ShortcutId`), and the combo
//! spelling both apps store: platform-neutral, `mod` for Cmd on a Mac and
//! Ctrl elsewhere (`mod-shift-a`).
//!
//! A browser keeps some chords for itself, such as opening and closing tabs
//! and windows; [`browser_keeps`] names them, and a shortcut can't take
//! one. Where the desktop's default is one of them, the web's default is
//! the nearest free chord.

use std::collections::BTreeMap;

use rust_native::style::Style;
use rust_native::{Axis, Element, Node};
use serde::{Deserialize, Serialize};

use super::{Intent, Paints, Prefs, Remote, button, header, node, row, section, text};

/// The shortcuts the web app binds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Shortcut {
    CommandPalette,
    ToggleSidebar,
    ToggleFiles,
    NewSession,
    NextSession,
    PrevSession,
    ArchiveSession,
}

impl Shortcut {
    pub const ALL: [Shortcut; 7] = [
        Shortcut::CommandPalette,
        Shortcut::ToggleSidebar,
        Shortcut::ToggleFiles,
        Shortcut::NewSession,
        Shortcut::NextSession,
        Shortcut::PrevSession,
        Shortcut::ArchiveSession,
    ];

    /// The desktop's row label.
    pub fn label(self) -> &'static str {
        match self {
            Shortcut::CommandPalette => "Command palette",
            Shortcut::ToggleSidebar => "Toggle left sidebar",
            Shortcut::ToggleFiles => "Toggle files panel",
            Shortcut::NewSession => "New session",
            Shortcut::NextSession => "Next session",
            Shortcut::PrevSession => "Previous session",
            Shortcut::ArchiveSession => "Archive session",
        }
    }

    /// The desktop's section for the row.
    pub fn group(self) -> &'static str {
        match self {
            Shortcut::CommandPalette | Shortcut::ToggleSidebar | Shortcut::ToggleFiles => "Panels",
            _ => "Sessions",
        }
    }

    /// The default chord in a browser. The desktop's are `mod-b`, `mod-e`,
    /// `mod-n`, `ctrl-tab`, `ctrl-shift-tab`, and `mod-shift-a`; a browser
    /// keeps `mod-n` and Ctrl+Tab.
    pub fn default_combo(self) -> &'static str {
        match self {
            Shortcut::CommandPalette => "mod-k",
            Shortcut::ToggleSidebar => "mod-b",
            Shortcut::ToggleFiles => "mod-e",
            Shortcut::NewSession => "mod-alt-n",
            Shortcut::NextSession => "alt-down",
            Shortcut::PrevSession => "alt-up",
            Shortcut::ArchiveSession => "mod-alt-a",
        }
    }

    fn slug(self) -> &'static str {
        match self {
            Shortcut::CommandPalette => "command-palette",
            Shortcut::ToggleSidebar => "toggle-sidebar",
            Shortcut::ToggleFiles => "toggle-files",
            Shortcut::NewSession => "new-session",
            Shortcut::NextSession => "next-session",
            Shortcut::PrevSession => "prev-session",
            Shortcut::ArchiveSession => "archive-session",
        }
    }
}

/// The person's chords, by shortcut; one that isn't listed has its
/// default.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Keymap(BTreeMap<Shortcut, String>);

impl Keymap {
    pub fn get(&self, shortcut: Shortcut) -> &str {
        self.0
            .get(&shortcut)
            .map(String::as_str)
            .filter(|combo| !browser_keeps(combo))
            .unwrap_or(shortcut.default_combo())
    }

    /// The command assigned to a stored, platform-neutral chord.
    pub fn shortcut_for(&self, combo: &str) -> Option<Shortcut> {
        Shortcut::ALL.into_iter().find(|&s| self.get(s) == combo)
    }

    pub fn customized(&self) -> bool {
        Shortcut::ALL
            .into_iter()
            .any(|s| self.get(s) != s.default_combo())
    }

    pub(super) fn apply(&mut self, change: &Change) {
        match change {
            Change::Set(shortcut, combo) => {
                if combo == shortcut.default_combo() {
                    self.0.remove(shortcut);
                } else {
                    self.0.insert(*shortcut, combo.clone());
                }
            }
            Change::Reset(shortcut) => {
                self.0.remove(shortcut);
            }
            Change::Record(_) | Change::StopRecording => {}
        }
    }

    /// The shortcut other than `shortcut` already on `combo`.
    pub fn owner(&self, shortcut: Shortcut, combo: &str) -> Option<Shortcut> {
        Shortcut::ALL
            .into_iter()
            .find(|&other| other != shortcut && self.get(other) == combo)
    }

    /// Why `combo` can't go to `shortcut`, in the desktop's words.
    pub fn refusal(&self, shortcut: Shortcut, combo: &str, mac: bool) -> Option<String> {
        if combo == "mod-enter" {
            return Some(format!(
                "{} is reserved for the composer.",
                display_combo_on(mac, combo)
            ));
        }
        if browser_keeps(combo) {
            return Some(format!(
                "Your browser keeps {} for itself.",
                display_combo_on(mac, combo)
            ));
        }
        if !combo.contains('-') && !combo.starts_with('f') {
            return Some("Use Control, Alt, Shift, or Command with the key.".into());
        }
        self.owner(shortcut, combo).map(|owner| {
            format!(
                "{} is already assigned to {}.",
                display_combo_on(mac, combo),
                owner.label()
            )
        })
    }
}

/// A shortcut change.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Change {
    /// Start listening for the next chord for this shortcut.
    Record(Shortcut),
    StopRecording,
    Set(Shortcut, String),
    Reset(Shortcut),
}

/// Chords a browser handles before the page sees them, or won't let a page
/// take: new and closed tabs and windows, tab switching, and quitting.
pub fn browser_keeps(combo: &str) -> bool {
    const KEPT: &[&str] = &[
        "mod-n",
        "mod-shift-n",
        "mod-t",
        "mod-shift-t",
        "mod-w",
        "mod-shift-w",
        "mod-q",
        "mod-tab",
        "mod-shift-tab",
        "ctrl-tab",
        "ctrl-shift-tab",
        "mod-l",
        "mod-r",
        "mod-shift-r",
        "mod-shift-a",
        "mod-shift-b",
        "mod-shift-o",
        "mod-shift-j",
        "mod-shift-i",
        "mod-alt-i",
        "mod-alt-j",
    ];
    KEPT.contains(&combo)
        || combo
            .strip_prefix("mod-")
            .is_some_and(|key| key.len() == 1 && key.as_bytes()[0].is_ascii_digit())
}

/// The stored spelling of a keystroke, or `None` for a lone modifier.
/// `mac` decides whether Ctrl is the primary modifier (`mod`) or its own.
/// The desktop's recorder spells chords the same way.
pub fn combo_from_keystroke_on(
    mac: bool,
    ctrl: bool,
    alt: bool,
    shift: bool,
    cmd: bool,
    key: &str,
) -> Option<String> {
    // GPUI names arrows without the browser's `Arrow` prefix.
    let key = match key {
        "ArrowUp" => "up",
        "ArrowDown" => "down",
        "ArrowLeft" => "left",
        "ArrowRight" => "right",
        " " => "space",
        "Meta" => "platform",
        other => other,
    };
    let key = key.trim().to_lowercase();
    if key.is_empty()
        || matches!(
            key.as_str(),
            "ctrl" | "control" | "alt" | "shift" | "cmd" | "platform" | "fn"
        )
    {
        return None;
    }
    let mut parts: Vec<&str> = Vec::new();
    // On macOS Ctrl stays its own modifier rather than folding into "mod".
    let ctrl_is_primary = ctrl && !mac;
    if cmd || ctrl_is_primary {
        parts.push("mod");
    }
    if ctrl && !ctrl_is_primary {
        parts.push("ctrl");
    }
    if alt {
        parts.push("alt");
    }
    if shift {
        parts.push("shift");
    }
    parts.push(&key);
    Some(parts.join("-"))
}

/// A stored combo as GPUI binds it on this platform (`mod` to `cmd` or
/// `ctrl`).
pub fn platform_combo_on(mac: bool, combo: &str) -> String {
    let primary = if mac { "cmd" } else { "ctrl" };
    combo
        .split('-')
        .map(|part| if part == "mod" { primary } else { part })
        .collect::<Vec<_>>()
        .join("-")
}

/// A combo as the shortcuts table shows it (`mod-s` is "Cmd+S" or
/// "Ctrl+S").
pub fn display_combo_on(mac: bool, combo: &str) -> String {
    combo
        .split('-')
        .map(|part| match part {
            "mod" => if mac { "Cmd" } else { "Ctrl" }.to_string(),
            "alt" => if mac { "Opt" } else { "Alt" }.to_string(),
            "shift" => "Shift".to_string(),
            other => {
                let mut chars = other.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    None => String::new(),
                }
            }
        })
        .collect::<Vec<_>>()
        .join("+")
}

pub(super) fn page(prefs: &Prefs, remote: &Remote, paints: &Paints) -> Vec<Node<Intent>> {
    let keymap = &prefs.keymap;
    let customized = keymap.customized() || prefs.escape_stops_agent || prefs.send_with_modifier;
    let mut out = vec![node(
        "shortcuts-head",
        Style {
            gap_points: Some(16),
            ..Style::default()
        },
        Element::Stack {
            axis: Axis::Horizontal,
            children: vec![
                node(
                    "shortcuts-head-words",
                    Style {
                        fill_height: Some(true),
                        ..Style::default()
                    },
                    Element::Stack {
                        axis: Axis::Vertical,
                        children: vec![header(
                            "shortcuts-title",
                            "Keyboard shortcuts",
                            Some("Click a binding, then press the new key combination."),
                            paints,
                        )],
                    },
                ),
                node(
                    "shortcuts-restore-column",
                    Style {
                        intrinsic_width: Some(true),
                        ..Style::default()
                    },
                    Element::Stack {
                        axis: Axis::Vertical,
                        children: vec![node(
                            "shortcuts-restore",
                            Style::default(),
                            Element::Button {
                                label: "Restore defaults".into(),
                                enabled: customized && remote.recording.is_none(),
                                icon: None,
                                shortcut: None,
                                intent: Intent::RestoreShortcuts,
                            },
                        )],
                    },
                ),
            ],
        },
    )];
    for group in ["Panels", "Sessions"] {
        let rows = Shortcut::ALL
            .into_iter()
            .filter(|s| s.group() == group)
            .map(|shortcut| {
                let combo = keymap.get(shortcut);
                let recording = remote.recording == Some(shortcut);
                let slug = shortcut.slug();
                let mut controls = Vec::new();
                if combo != shortcut.default_combo() && !recording {
                    controls.push(button(
                        &format!("shortcut-{slug}-reset"),
                        "Reset",
                        Intent::Shortcut(Change::Reset(shortcut)),
                    ));
                }
                controls.push(node(
                    &format!("shortcut-{slug}-combo"),
                    Style::default(),
                    Element::Choice {
                        label: if recording {
                            "Press keys…".into()
                        } else {
                            display_combo_on(remote.mac, combo)
                        },
                        selected: recording,
                        enabled: true,
                        intent: Intent::Shortcut(if recording {
                            Change::StopRecording
                        } else {
                            Change::Record(shortcut)
                        }),
                        children: Vec::new(),
                    },
                ));
                row(
                    &format!("shortcut-{slug}"),
                    shortcut.label(),
                    None,
                    controls,
                    paints,
                )
            })
            .collect();
        out.push(section(
            &format!("shortcuts-{}", group.to_lowercase()),
            group,
            rows,
            paints,
        ));
    }
    let helper = if remote.recording.is_some() {
        "Press Escape to cancel.".to_string()
    } else if let Some(notice) = &remote.shortcut_notice {
        notice.clone()
    } else {
        "Shortcuts must be unique.".to_string()
    };
    out.push(text("shortcuts-helper", &helper, paints.muted));
    out
}
