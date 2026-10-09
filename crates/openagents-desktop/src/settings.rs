//! Settings (#10021, audit CDP-22): appearance, text size, keyboard
//! shortcuts, notifications, Coder, phones and computers, and archived
//! chats.
//!
//! What a setting means is shared Rust
//! ([`openagents_chat_app::preferences`], the shortcut list in
//! [`openagents_chat_app::commands::bindings`], and the archive list in
//! [`openagents_chat_app::chat_list::archived`]); this module only shows
//! them. The window keeps the preferences in the settings file's `app`
//! section and applies each change at once. Coder's page (#10070) shows
//! and changes the file's `coder` section, whose meaning and validation
//! are `coder::task::settings`'s: the window reads it into [`CoderChoices`]
//! and writes a change only through that loader. Phones and computers is the
//! existing pairing screen ([`crate::screens`]), so a code shown there is
//! cancelled when the person leaves it for another page, as when they leave
//! the standalone screen (`DSK-01`). The app is dark only.

use crate::chrome::Update;
use crate::model::{Intent, Model};
use openagents_chat_app::preferences::{Change, Preferences, TextSize, ThemeChoice};
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
    Coder,
    /// The person's own model provider keys (BYOK, #10176).
    Providers,
    Computers,
    /// The background rules this computer runs.
    Background,
    Archived,
}

impl Pane {
    pub const ALL: [Pane; 9] = [
        Pane::Appearance,
        Pane::TextSize,
        Pane::Shortcuts,
        Pane::Notifications,
        Pane::Coder,
        Pane::Providers,
        Pane::Computers,
        Pane::Background,
        Pane::Archived,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Pane::Appearance => "Appearance",
            Pane::TextSize => "Text size",
            Pane::Shortcuts => "Keyboard shortcuts",
            Pane::Notifications => "Notifications",
            Pane::Coder => "Coder",
            Pane::Providers => "Model providers",
            Pane::Computers => "Phones and computers",
            Pane::Background => "Background",
            Pane::Archived => "Archived chats",
        }
    }

    const fn key(self) -> &'static str {
        match self {
            Pane::Appearance => "appearance",
            Pane::TextSize => "text-size",
            Pane::Shortcuts => "shortcuts",
            Pane::Notifications => "notifications",
            Pane::Coder => "coder",
            Pane::Providers => "providers",
            Pane::Computers => "computers",
            Pane::Background => "background",
            Pane::Archived => "archived",
        }
    }

    const fn glyph(self) -> Glyph {
        match self {
            Pane::Appearance => Glyph::Settings,
            Pane::TextSize => Glyph::Edit,
            Pane::Shortcuts => Glyph::Terminal,
            Pane::Notifications => Glyph::Flag,
            Pane::Coder => Glyph::Key,
            Pane::Providers => Glyph::Key,
            Pane::Computers => Glyph::Computer,
            Pane::Background => Glyph::History,
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
    /// Coder Light, Coder Noir, or the system's appearance (#11028).
    Theme {
        choice: ThemeChoice,
    },
    ReduceMotion {
        on: bool,
    },
    Notifications {
        on: bool,
    },
    /// Play Coder's sounds, or mute them.
    Sounds {
        on: bool,
    },
    /// Coder's `coder.start`: wait for **Run Coder**, or start at once.
    CoderStart {
        ask_first: bool,
    },
    /// Let the agent with this settings name (`codex`, `claude`, …) run
    /// Coder here, or not.
    CoderAgent {
        agent: String,
        on: bool,
    },
    /// Restore the archived chat with this ID.
    Restore {
        chat: String,
    },
    /// Add the provider's key from the clipboard (`openrouter`, `vercel`,
    /// or `typesafe`), after the provider accepts it. The key is never
    /// shown.
    ProviderPaste {
        provider: String,
    },
    /// Test the provider's stored key now.
    ProviderTest {
        provider: String,
    },
    /// Sign in to OpenRouter in the browser and keep the key it returns
    /// (OAuth PKCE, `model_access::connect`); nothing to paste.
    ProviderConnect,
    /// Remove the provider's key.
    ProviderRemove {
        provider: String,
    },
    /// Run every model call on the person's own keys, or on OpenAgents.
    ProvidersMine {
        on: bool,
    },
    /// Pause a background rule, or resume it.
    Background {
        rule: String,
        resume: bool,
    },
}

impl Action {
    /// The preference change this action makes, if any.
    pub fn change(&self) -> Option<Change> {
        match self {
            Action::TextSize { size } => Some(Change::TextSize(*size)),
            Action::Theme { choice } => Some(Change::Theme(*choice)),
            Action::ReduceMotion { on } => Some(Change::ReduceMotion(*on)),
            Action::Notifications { on } => Some(Change::Notifications(*on)),
            Action::Sounds { on } => Some(Change::Sounds(*on)),
            Action::Pane { .. }
            | Action::Restore { .. }
            | Action::CoderStart { .. }
            | Action::CoderAgent { .. }
            | Action::ProviderPaste { .. }
            | Action::ProviderTest { .. }
            | Action::ProviderConnect
            | Action::ProviderRemove { .. }
            | Action::ProvidersMine { .. }
            | Action::Background { .. } => None,
        }
    }
}

/// An archived chat the page lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Archived {
    pub id: String,
    pub title: String,
}

/// One agent Coder can run, as Coder's page shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoderAgent {
    /// The name the settings file uses (`codex`, `claude`, …).
    pub key: String,
    /// The name a person reads.
    pub name: String,
    /// It may run.
    pub on: bool,
    /// Why it can't be turned on here, when it can't.
    pub blocked: Option<String>,
}

/// Coder's own settings on this computer, as its page shows them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum CoderChoices {
    /// Not read yet, or not on this computer (a capture or a preview).
    #[default]
    Unknown,
    /// The file can't be read: a run refuses and a coding reply only
    /// offers, so nothing here changes it.
    Unreadable(String),
    Read {
        /// `coder.start: ask_first`.
        ask_first: bool,
        /// Every agent: those allowed first, in the order Coder tries
        /// them, then the rest.
        agents: Vec<CoderAgent>,
    },
}

/// One provider row on the Model providers page. Never the key: only its
/// last four characters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderRow {
    /// `openrouter`, `vercel`, or `typesafe`.
    pub provider: String,
    /// The name a person reads.
    pub name: String,
    /// The key's last four characters, when one is added.
    pub last_four: Option<String>,
    /// The last test's line ("Your OpenRouter key works.").
    pub line: Option<String>,
    /// Where the person makes a key.
    pub page: String,
}

/// The person's own model providers (BYOK), as the page shows them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Providers {
    pub rows: Vec<ProviderRow>,
    /// Every model call runs on the person's keys.
    pub mine: bool,
    /// Why "Use my keys for everything" can't be turned on now.
    pub mine_blocked: Option<String>,
    /// "Running on OpenAgents." or "Running on your keys."
    pub status: String,
    /// The provider whose key is being tested or connected off the window's
    /// thread, and what the page says meanwhile ("Testing your OpenRouter
    /// key…"); its buttons wait until the answer comes back.
    pub busy: Option<(String, String)>,
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
    /// Coder's own settings ([`CoderChoices`]).
    pub coder: CoderChoices,
    /// The person's own model providers ([`Providers`]); empty until read.
    pub providers: Providers,
    /// The background rules, read when the page opens.
    pub background: Vec<crate::background_pane::Rule>,
    /// The settings file the window keeps them in; none in a capture or a
    /// test, where they live only in memory.
    pub file: Option<std::path::PathBuf>,
    /// The "Reduce motion" preference, read by the Grid on the Verse page
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
        node.style.foreground = Some(openagents_chat_app::visual::current().muted);
    }
    node
}

fn title(key: &str, value: &str) -> Node<Intent> {
    let mut node = text(key, value, TextRole::Body);
    node.style.weight = Some(TextWeight::Bold);
    node.style.foreground = Some(openagents_chat_app::visual::current().text);
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
        background: Some(if selected {
            openagents_chat_app::visual::current().selected
        } else {
            openagents_chat_app::visual::current().sidebar
        }),
        foreground: Some(if selected {
            openagents_chat_app::visual::current().text
        } else {
            openagents_chat_app::visual::current().muted
        }),
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
    let current = settings.preferences.theme;
    let themes = ThemeChoice::ALL
        .into_iter()
        .map(|choice| {
            chip(button(
                &format!("settings-theme-{}", choice.as_str()),
                choice.label(),
                Action::Theme { choice },
                (choice == current).then_some(Glyph::Check),
                choice == current,
                true,
            ))
        })
        .collect();
    vec![
        title("settings-theme-title", "Theme"),
        stack("settings-theme-choices", Axis::Wrap, Space::Sm, themes),
        text(
            "settings-theme-line",
            match current {
                ThemeChoice::System => "Light or dark, as this computer is set.",
                ThemeChoice::Light => "Coder Light, whatever this computer is set to.",
                ThemeChoice::Dark => "Dark, whatever this computer is set to.",
            },
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
            // The Verse shows only in a preview build (#11120).
            if crate::preview::ON {
                "Keeps the Grid on the Verse page still. Your computer's own Reduce motion setting also does."
            } else {
                "Fewer animations. Your computer's own Reduce motion setting also does."
            },
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
        control: false,
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
        keys.style.foreground = Some(openagents_chat_app::visual::current().text);
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
    let sounds = settings.preferences.sounds;
    vec![
        title("settings-notifications-title", "Notifications"),
        toggle(
            "settings-notifications",
            "Notify me about Coder",
            on,
            Action::Notifications { on: !on },
        ),
        text(
            "settings-notifications-line",
            "When Coder asks you something, finishes, or fails while OpenAgents isn't in front.",
            TextRole::Status,
        ),
        toggle(
            "settings-sounds",
            "Play sounds",
            sounds,
            Action::Sounds { on: !sounds },
        ),
        text(
            "settings-sounds-line",
            "A short sound when Coder finishes, asks you something, or fails, once each time.",
            TextRole::Status,
        ),
    ]
}

fn coder(settings: &Settings, model: &Model) -> Vec<Node<Intent>> {
    let mut rows = vec![title("settings-coder-title", "Coder on this computer")];
    // The engines in full, with each usage window's reset; the sidebar
    // shows one condensed row each and opens this page (#10072).
    if let Some(report) = &model.engine {
        rows.push(openagents_chat_app::engine::strip(report));
    }
    if let Some(note) = &model.engine_note {
        rows.push(text("settings-engine-note", note.clone(), TextRole::Status));
    }
    let (ask_first, agents) = match &settings.coder {
        CoderChoices::Unknown => {
            rows.push(text(
                "settings-coder-unknown",
                "Coder's settings show here when OpenAgents runs on your computer.",
                TextRole::Status,
            ));
            return rows;
        }
        CoderChoices::Unreadable(why) => {
            rows.push(text(
                "settings-coder-unreadable",
                format!(
                    "Coder's settings can't be read, so a coding reply only offers Coder. Fix or remove the file to change them here: {why}"
                ),
                TextRole::Status,
            ));
            return rows;
        }
        CoderChoices::Read { ask_first, agents } => (*ask_first, agents),
    };
    rows.push(title(
        "settings-coder-start-title",
        "When a reply is coding work",
    ));
    rows.push(stack(
        "settings-coder-start",
        Axis::Wrap,
        Space::Sm,
        [(false, "Start at once"), (true, "Ask first")]
            .into_iter()
            .map(|(asks, label)| {
                chip(button(
                    &format!(
                        "settings-coder-{}",
                        if asks { "ask-first" } else { "at-once" }
                    ),
                    label,
                    Action::CoderStart { ask_first: asks },
                    (asks == ask_first).then_some(Glyph::Check),
                    asks == ask_first,
                    true,
                ))
            })
            .collect(),
    ));
    rows.push(text(
        "settings-coder-start-line",
        if ask_first {
            "The reply offers Run Coder, and Coder starts when you choose it."
        } else {
            "Coder starts right away when you ask for coding work, here or on your phone."
        },
        TextRole::Status,
    ));
    rows.push(title("settings-coder-agents-title", "Agents Coder may run"));
    let allowed = agents.iter().filter(|agent| agent.on).count();
    for agent in agents {
        let only = agent.on && allowed == 1;
        let mut toggle = toggle(
            &format!("settings-coder-agent-{}", agent.key),
            &agent.name,
            agent.on,
            Action::CoderAgent {
                agent: agent.key.clone(),
                on: !agent.on,
            },
        );
        if let Element::Button { enabled, .. } = &mut toggle.element {
            *enabled = !only && (agent.on || agent.blocked.is_none());
        }
        rows.push(toggle);
        if let Some(why) = agent.blocked.as_ref().filter(|_| !agent.on) {
            rows.push(text(
                &format!("settings-coder-agent-{}-line", agent.key),
                why.clone(),
                TextRole::Status,
            ));
        }
    }
    rows.push(text(
        "settings-coder-agents-line",
        "Coder uses the agents signed in on this computer, top first, when each has room. Turn one off to keep Coder from using it. At least one stays on.",
        TextRole::Status,
    ));
    rows
}

/// The Model providers page (BYOK, #10176): one row per provider with its
/// last four characters and its state, Add from clipboard, Test, and
/// Remove; the switch; and the status line. A key is never drawn.
fn providers(settings: &Settings) -> Vec<Node<Intent>> {
    let providers = &settings.providers;
    let mut rows = vec![title(
        "settings-providers-title",
        "Your own model providers",
    )];
    if providers.rows.is_empty() {
        rows.push(text(
            "settings-providers-unknown",
            "Your providers show here when OpenAgents runs on your computer.",
            TextRole::Status,
        ));
        return rows;
    }
    rows.push(text(
        "settings-providers-line",
        "Add an OpenRouter, Vercel AI Gateway, or TypeSafe API key to run chat replies, Jev, Microcoder, and embeddings on your own account. Copy the key, then choose Add from clipboard.",
        TextRole::Status,
    ));
    // One test or sign-in at a time: every key button waits for it.
    let free = providers.busy.is_none();
    for row in &providers.rows {
        let id = &row.provider;
        let state = match &row.last_four {
            Some(last) => format!("{}: added, ends in {last}", row.name),
            None => format!("{}: not added", row.name),
        };
        rows.push(title(&format!("settings-provider-{id}"), &state));
        let mut actions = Vec::new();
        // The easiest add: sign in to OpenRouter, nothing to paste.
        if id == "openrouter" && row.last_four.is_none() {
            actions.push(chip(button(
                "settings-provider-openrouter-connect",
                "Connect OpenRouter",
                Action::ProviderConnect,
                Some(Glyph::Key),
                false,
                free,
            )));
        }
        actions.push(chip(button(
            &format!("settings-provider-{id}-paste"),
            if row.last_four.is_some() {
                "Replace from clipboard"
            } else {
                "Add from clipboard"
            },
            Action::ProviderPaste {
                provider: id.clone(),
            },
            Some(Glyph::Edit),
            false,
            free,
        )));
        if row.last_four.is_some() {
            actions.push(chip(button(
                &format!("settings-provider-{id}-test"),
                "Test",
                Action::ProviderTest {
                    provider: id.clone(),
                },
                None,
                false,
                free,
            )));
            actions.push(chip(button(
                &format!("settings-provider-{id}-remove"),
                "Remove",
                Action::ProviderRemove {
                    provider: id.clone(),
                },
                None,
                false,
                free,
            )));
        }
        rows.push(stack(
            &format!("settings-provider-{id}-actions"),
            Axis::Wrap,
            Space::Sm,
            actions,
        ));
        let busy = providers
            .busy
            .as_ref()
            .filter(|(provider, _)| provider == id)
            .map(|(_, line)| line);
        if let Some(line) = busy.or(row.line.as_ref()) {
            rows.push(text(
                &format!("settings-provider-{id}-line"),
                line.clone(),
                TextRole::Status,
            ));
        }
        rows.push(text(
            &format!("settings-provider-{id}-page"),
            format!("Make one at {}", row.page),
            TextRole::Status,
        ));
    }
    let mut switch = toggle(
        "settings-providers-mine",
        "Use my keys for everything",
        providers.mine,
        Action::ProvidersMine {
            on: !providers.mine,
        },
    );
    if let Element::Button { enabled, .. } = &mut switch.element {
        *enabled = providers.mine || providers.mine_blocked.is_none();
    }
    rows.push(switch);
    if let Some(why) = providers.mine_blocked.as_ref().filter(|_| !providers.mine) {
        rows.push(text(
            "settings-providers-blocked",
            why.clone(),
            TextRole::Status,
        ));
    }
    rows.push(text(
        "settings-providers-status",
        providers.status.clone(),
        TextRole::Status,
    ));
    rows
}

fn background(settings: &Settings, now: u64) -> Vec<Node<Intent>> {
    let mut rows = vec![title("settings-background-title", "Background")];
    if settings.background.is_empty() {
        rows.push(text(
            "settings-background-empty",
            "No background rules here.",
            TextRole::Status,
        ));
        return rows;
    }
    for rule in &settings.background {
        let key = format!("settings-background-{}", rule.id);
        let mut name = text(&format!("{key}-name"), rule.name.clone(), TextRole::Body);
        name.style.foreground = Some(openagents_chat_app::visual::current().text);
        let mut head = vec![name];
        if rule.status != crate::background_pane::Status::Broken {
            let resume = rule.resumes();
            head.push(toggle(
                &format!("{key}-toggle"),
                "On",
                !resume,
                Action::Background {
                    rule: rule.id.clone(),
                    resume,
                },
            ));
        }
        rows.push(stack(&key, Axis::Horizontal, Space::Md, head));
        rows.push(text(
            &format!("{key}-line"),
            rule.line(now),
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
        name.style.foreground = Some(openagents_chat_app::visual::current().text);
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
        button.style.background = Some(openagents_chat_app::visual::current().selected);
        button.style.foreground = Some(openagents_chat_app::visual::current().text);
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
        Pane::Coder => coder(settings, model),
        Pane::Providers => providers(settings),
        Pane::Computers => vec![crate::screens::root(model, now)],
        Pane::Background => background(settings, now),
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
                // The Model providers page names the person's own API
                // keys, the providers' word for them (#10176); nothing
                // else there is jargon.
                let banned: Vec<String> = crate::words::banned_in(&value)
                    .into_iter()
                    .filter(|word| {
                        !(pane == Pane::Providers && word.to_lowercase().starts_with("key"))
                    })
                    .collect();
                assert!(banned.is_empty(), "{value}");
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

    /// The Model providers page shows each provider's last four
    /// characters and never a key, and the switch is off while blocked.
    #[test]
    fn the_providers_page_never_draws_a_key() {
        let model = Model::new(std::time::Instant::now(), Screen::Home, Agent::Enabled);
        let mut settings = Settings {
            pane: Pane::Providers,
            ..Settings::default()
        };
        settings.providers = Providers {
            rows: vec![ProviderRow {
                provider: "openrouter".into(),
                name: "OpenRouter".into(),
                last_four: Some("abcd".into()),
                line: Some("Your OpenRouter key works.".into()),
                page: "https://openrouter.ai/settings/keys".into(),
            }],
            mine: false,
            mine_blocked: None,
            status: "Running on OpenAgents.".into(),
            busy: None,
        };
        let rendered = view(&settings, true, None, &model, 0);
        let words = crate::screens::words(&rendered).join(" ");
        assert!(words.contains("ends in abcd"), "{words}");
        assert!(words.contains("Running on OpenAgents."), "{words}");
        let Some(Node {
            element: Element::Button { intent, .. },
            ..
        }) = find(&rendered, "settings-providers-mine")
        else {
            panic!("no switch")
        };
        assert_eq!(
            intent.clone(),
            Intent::Settings {
                action: Action::ProvidersMine { on: true }
            }
        );
        settings.providers.mine_blocked =
            Some("Add an OpenRouter or Vercel AI Gateway key first.".into());
        let rendered = view(&settings, true, None, &model, 0);
        let Some(Node {
            element: Element::Button { enabled, .. },
            ..
        }) = find(&rendered, "settings-providers-mine")
        else {
            panic!("no switch")
        };
        assert!(!enabled);
    }

    /// While a key is tested or OpenRouter connects off the window's
    /// thread, the row says so and every key button waits; Connect
    /// OpenRouter shows only while no OpenRouter key is added.
    #[test]
    fn the_background_page_lists_rules_with_their_last_run_and_a_switch() {
        use crate::background_pane::{Rule, Status};
        let model = Model::new(std::time::Instant::now(), Screen::Home, Agent::Enabled);
        let mut settings = Settings {
            pane: Pane::Background,
            ..Settings::default()
        };
        let rendered = view(&settings, true, None, &model, 0);
        assert!(find(&rendered, "settings-background-empty").is_some());
        settings.background = vec![
            Rule {
                id: "disk".into(),
                name: "Disk cleanup".into(),
                status: Status::On,
                last: Some("Freed 4 GB: 2 old build folders.".into()),
                when: Some(1000 - 120),
            },
            Rule {
                id: "usage".into(),
                name: "Daily usage summary".into(),
                status: Status::Off,
                last: None,
                when: None,
            },
            Rule {
                id: "bad".into(),
                name: "bad".into(),
                status: Status::Broken,
                last: Some("does not read".into()),
                when: None,
            },
        ];
        let rendered = view(&settings, true, None, &model, 1000);
        let intent = |id: &str| match find(&rendered, id) {
            Some(Node {
                element: Element::Button { intent, .. },
                ..
            }) => intent.clone(),
            _ => panic!("no {id}"),
        };
        assert_eq!(
            intent("settings-background-disk-toggle"),
            Intent::Settings {
                action: Action::Background {
                    rule: "disk".into(),
                    resume: false
                }
            }
        );
        assert_eq!(
            intent("settings-background-usage-toggle"),
            Intent::Settings {
                action: Action::Background {
                    rule: "usage".into(),
                    resume: true
                }
            }
        );
        assert!(find(&rendered, "settings-background-bad-toggle").is_none());
        let words = crate::screens::words(&rendered).join(" ");
        assert!(
            words.contains("On · Freed 4 GB: 2 old build folders. · 2 min ago"),
            "{words}"
        );
        assert!(crate::words::banned_in(&words).is_empty(), "{words}");
    }

    #[test]
    fn a_key_test_in_flight_holds_the_buttons() {
        let model = Model::new(std::time::Instant::now(), Screen::Home, Agent::Enabled);
        let mut settings = Settings {
            pane: Pane::Providers,
            ..Settings::default()
        };
        settings.providers = Providers {
            rows: vec![ProviderRow {
                provider: "openrouter".into(),
                name: "OpenRouter".into(),
                last_four: None,
                line: None,
                page: "https://openrouter.ai/settings/keys".into(),
            }],
            mine: false,
            mine_blocked: None,
            status: "Running on OpenAgents.".into(),
            busy: None,
        };
        let enabled = |view: &Node<Intent>, id: &str| match find(view, id) {
            Some(Node {
                element: Element::Button { enabled, .. },
                ..
            }) => *enabled,
            _ => panic!("no {id}"),
        };
        let rendered = view(&settings, true, None, &model, 0);
        assert!(enabled(&rendered, "settings-provider-openrouter-connect"));
        assert!(enabled(&rendered, "settings-provider-openrouter-paste"));
        settings.providers.busy =
            Some(("openrouter".into(), "Testing your OpenRouter key…".into()));
        let rendered = view(&settings, true, None, &model, 0);
        assert!(!enabled(&rendered, "settings-provider-openrouter-connect"));
        assert!(!enabled(&rendered, "settings-provider-openrouter-paste"));
        let words = crate::screens::words(&rendered).join(" ");
        assert!(words.contains("Testing your OpenRouter key…"), "{words}");
        settings.providers.busy = None;
        settings.providers.rows[0].last_four = Some("abcd".into());
        let rendered = view(&settings, true, None, &model, 0);
        assert!(find(&rendered, "settings-provider-openrouter-connect").is_none());
    }

    #[test]
    fn shortcut_rows_show_control_for_chat_navigation() {
        let model = Model::new(std::time::Instant::now(), Screen::Home, Agent::Enabled);
        let mut settings = Settings::default();
        settings.pane = Pane::Shortcuts;
        let rendered = view(&settings, true, None, &model, 0);
        for (index, label, chord) in [
            (5, "Next chat", "Ctrl+Tab"),
            (6, "Previous chat", "Ctrl+Shift+Tab"),
        ] {
            let row = find(&rendered, &format!("settings-shortcut-{index}")).unwrap();
            let words = crate::screens::words(row);
            assert!(words.iter().any(|word| word == label), "{words:?}");
            assert!(words.iter().any(|word| word == chord), "{words:?}");
        }
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
