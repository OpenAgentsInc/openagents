//! The settings window's pages as Rust Native views (#12288): one
//! description of each page's rows, with the desktop's titles, details, and
//! choices, that a GPUI shell mounts through [`crate::rust_native_gpui`].
//! The GPUI web app at `apps/openagents/web/chat` draws its Settings from
//! these; the desktop's own pages live in `openagents-ui`'s `settings`
//! module and keep the same words.
//!
//! A page is data: [`Prefs`] holds the choices that belong to the device
//! (the browser keeps them in its storage), [`Remote`] what the host read
//! from a computer or the account, and each activation resolves to an
//! [`Intent`]. [`Prefs::apply`] makes the device changes; the host sends
//! the rest to the computer or the account.

pub mod account;
pub mod admin;
/// Agents and the landing queue (#12330).
pub mod agents;
pub mod appearance;
/// CAD documents and configurators (#12334).
pub mod cad;
pub mod cloud;
/// Earn on a paired computer (#12294).
pub mod earn;
/// A Lab document (#12333).
pub mod lab;
pub mod memory;
/// Projects (#12331).
pub mod projects;
pub mod providers;
pub mod schedules;
pub mod shortcuts;
/// Traces (#12329).
pub mod traces;

use rust_native::style::{Color, Style, TextWeight};
use rust_native::{Axis, Element, Node, TextRole};
use serde::{Deserialize, Serialize};

pub use appearance::Appearance;
pub use shortcuts::{Keymap, Shortcut};

/// The pages, in the desktop's order (`SettingsSection::ALL`). The web
/// shows the ones a browser can change; the rest stay on the desktop.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Page {
    #[default]
    General,
    /// Each computer's agents, updates, and accounts (#12283).
    Providers,
    Claude,
    Data,
    Appearance,
    Notifications,
    Shortcuts,
    Files,
    Archived,
    /// Earn on a paired computer (#12294).
    Earn,
    /// Your ID: the account's keys, sign out, and delete (#12290).
    Account,
    Memory,
    Schedules,
}

impl Page {
    pub const ALL: [Page; 13] = [
        Page::General,
        Page::Providers,
        Page::Claude,
        Page::Data,
        Page::Appearance,
        Page::Notifications,
        Page::Shortcuts,
        Page::Files,
        Page::Archived,
        Page::Earn,
        Page::Account,
        Page::Memory,
        Page::Schedules,
    ];

    /// The desktop's nav label for the page.
    pub fn label(self) -> &'static str {
        match self {
            Page::General => "General",
            Page::Providers => "Providers",
            Page::Claude => "Claude",
            Page::Data => "Data",
            Page::Appearance => "Appearance",
            Page::Notifications => "Notifications",
            Page::Shortcuts => "Shortcuts",
            Page::Files => "Files",
            Page::Archived => "Archived sessions",
            Page::Earn => "Earn",
            Page::Account => "Your ID",
            Page::Memory => "Memory",
            Page::Schedules => "Schedules",
        }
    }

    /// The page's address segment, as the desktop's `OPENAGENTS_OPEN_ROUTE`
    /// names it (`settings/<slug>`).
    pub fn slug(self) -> &'static str {
        match self {
            Page::General => "general",
            Page::Providers => "providers",
            Page::Claude => "claude",
            Page::Data => "data",
            Page::Appearance => "appearance",
            Page::Notifications => "notifications",
            Page::Shortcuts => "shortcuts",
            Page::Files => "files",
            Page::Archived => "archived",
            Page::Earn => "earn",
            Page::Account => "account",
            Page::Memory => "memory",
            Page::Schedules => "schedules",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Page> {
        Page::ALL
            .into_iter()
            .find(|page| page.slug() == slug)
            .or(match slug {
                "conversations" => Some(Page::General),
                "id" | "identity" => Some(Page::Account),
                _ => None,
            })
    }

    /// The nav groups are separated by spacing alone, as on the desktop.
    pub fn starts_group(self) -> bool {
        matches!(self, Page::Files | Page::Account)
    }
}

/// The choices that belong to this device. The web keeps them in the
/// browser's storage under [`Prefs::STORAGE_KEY`]; a field this build
/// doesn't know is dropped, and a missing one takes its default.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Prefs {
    /// Send with Cmd/Ctrl+Enter; Enter adds a line.
    pub send_with_modifier: bool,
    /// Escape stops a running answer when no menu is open.
    pub escape_stops_agent: bool,
    /// Session sounds, and which events play one.
    pub sound_enabled: bool,
    pub sound_completion: bool,
    pub sound_input: bool,
    pub sound_attention: bool,
    /// Browser notifications, and whether they show only while the tab is
    /// in the background.
    pub notifications_enabled: bool,
    pub notifications_background_only: bool,
    /// The files panel's choices, which the panel keeps under its own key
    /// ([`FilesPrefs::STORAGE_KEY`]).
    #[serde(skip)]
    pub files: FilesPrefs,
    pub keymap: Keymap,
    pub appearance: Appearance,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            send_with_modifier: false,
            escape_stops_agent: false,
            sound_enabled: true,
            sound_completion: true,
            sound_input: true,
            sound_attention: true,
            notifications_enabled: true,
            notifications_background_only: true,
            files: FilesPrefs::default(),
            keymap: Keymap::default(),
            appearance: Appearance::default(),
        }
    }
}

impl Prefs {
    /// Where the web keeps them.
    pub const STORAGE_KEY: &'static str = "openagents.settings.v1";

    /// Read stored JSON; anything unreadable is the defaults.
    pub fn from_json(text: Option<&str>) -> Self {
        text.and_then(|text| serde_json::from_str(text).ok())
            .unwrap_or_default()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Make the device change an intent names. Answers whether anything
    /// changed, so the host saves only then.
    pub fn apply(&mut self, intent: &Intent) -> bool {
        let before = self.clone();
        match intent {
            Intent::SendWithModifier(on) => self.send_with_modifier = *on,
            Intent::EscapeStopsAgent(on) => self.escape_stops_agent = *on,
            Intent::Sound(which, on) => match which {
                Sound::All => self.sound_enabled = *on,
                Sound::Completion => self.sound_completion = *on,
                Sound::Input => self.sound_input = *on,
                Sound::Attention => self.sound_attention = *on,
            },
            Intent::Notifications(on) => self.notifications_enabled = *on,
            Intent::NotificationsBackgroundOnly(on) => self.notifications_background_only = *on,
            Intent::Files(change) => self.files.apply(*change),
            Intent::Shortcut(change) => self.keymap.apply(change),
            Intent::RestoreShortcuts => {
                self.keymap = Keymap::default();
                self.escape_stops_agent = false;
                self.send_with_modifier = false;
            }
            Intent::Appearance(change) => self.appearance.apply(change),
            _ => {}
        }
        *self != before
    }

    /// Whether an event's sound plays.
    pub fn plays(&self, sound: Sound) -> bool {
        self.sound_enabled
            && match sound {
                Sound::All => true,
                Sound::Completion => self.sound_completion,
                Sound::Input => self.sound_input,
                Sound::Attention => self.sound_attention,
            }
    }
}

/// The files panel's choices (Settings → Files), which the panel reads
/// (`apps/openagents/web/files`): the same fields under the same key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FilesPrefs {
    pub autosave: bool,
    pub autosave_delay_ms: u64,
    pub word_wrap: bool,
    pub show_hidden: bool,
}

impl Default for FilesPrefs {
    fn default() -> Self {
        Self {
            autosave: false,
            autosave_delay_ms: 900,
            word_wrap: false,
            show_hidden: false,
        }
    }
}

impl FilesPrefs {
    /// Where the files panel keeps them.
    pub const STORAGE_KEY: &'static str = "openagents.files.settings.v1";
    /// The autosave delays the desktop offers.
    pub const DELAYS: [u64; 5] = [300, 600, 900, 1_500, 3_000];

    pub fn from_json(text: Option<&str>) -> Self {
        let mut files: Self = text
            .and_then(|text| serde_json::from_str(text).ok())
            .unwrap_or_default();
        if !Self::DELAYS.contains(&files.autosave_delay_ms) {
            files.autosave_delay_ms = Self::default().autosave_delay_ms;
        }
        files
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn apply(&mut self, change: FilesChange) {
        match change {
            FilesChange::Autosave(on) => self.autosave = on,
            FilesChange::Delay(ms) if Self::DELAYS.contains(&ms) => self.autosave_delay_ms = ms,
            FilesChange::Delay(_) => {}
            FilesChange::WordWrap(on) => self.word_wrap = on,
            FilesChange::ShowHidden(on) => self.show_hidden = on,
        }
    }
}

/// A Files setting's change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilesChange {
    Autosave(bool),
    Delay(u64),
    WordWrap(bool),
    ShowHidden(bool),
}

/// A delay as people read it ("900 ms", "1.5 s").
fn delay_label(ms: u64) -> String {
    if ms >= 1_000 {
        format!("{} s", ms as f32 / 1_000.)
    } else {
        format!("{ms} ms")
    }
}

/// Which session sound a toggle names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sound {
    /// The master switch.
    All,
    /// A task finished.
    Completion,
    /// An agent asks for input.
    Input,
    /// Errors and disconnections.
    Attention,
}

/// What a settings control asks for.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intent {
    Open(Page),
    SendWithModifier(bool),
    EscapeStopsAgent(bool),
    Sound(Sound, bool),
    Notifications(bool),
    NotificationsBackgroundOnly(bool),
    Files(FilesChange),
    Shortcut(shortcuts::Change),
    RestoreShortcuts,
    Appearance(appearance::Change),
    /// The computer whose settings the computer cards show.
    Computer(String),
    /// Thread naming: `None` follows each session's own agent.
    TitleHarness(Option<String>),
    TitleModel(Option<String>),
    /// Worktrees: the folder browser, and the folder to use.
    WorktreeBrowse(Option<String>),
    WorktreeUse(String),
    WorktreeDefault,
    /// Archived sessions.
    Unarchive(String),
    Delete(String),
    DeleteAllArchived,
    ConfirmDeleteAll(bool),
    ArchivedPage(usize),
    /// Your ID (#12290).
    Account(account::Action),
    /// Site administration (#12335).
    Admin(admin::Action),
    /// Earn (#12294).
    Earn(earn::Action),
    /// Providers (#12283).
    Providers(providers::Action),
    /// Traces (#12329).
    Traces(traces::Action),
    /// Agents (#12330).
    Agents(agents::Action),
    /// Projects (#12331).
    Projects(projects::Action),
    /// A Lab document (#12333).
    Lab(lab::Action),
    /// CAD documents and configurators (#12334).
    Cad(cad::Action),
    Cloud(cloud::Action),
    Memory(memory::Action),
    Schedules(schedules::Action),
}

/// The theme's colors for the forms, which the host reads from its theme
/// (as the decision panel's `Paints`), so the views stay theme-free.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Paints {
    pub text: Color,
    pub muted: Color,
    pub faint: Color,
    pub card: Color,
    pub border: Color,
    pub danger: Color,
}

impl Default for Paints {
    fn default() -> Self {
        Self {
            text: Color::rgb(236, 236, 236),
            muted: Color::rgb(160, 160, 160),
            faint: Color::rgb(110, 110, 110),
            card: Color::rgb(28, 28, 28),
            border: Color::rgb(48, 48, 48),
            danger: Color::rgb(235, 87, 87),
        }
    }
}

/// A value the host is reading, has, or couldn't read.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Load<T> {
    #[default]
    Idle,
    Loading,
    Ready(T),
    Failed(String),
}

impl<T> Load<T> {
    pub fn ready(&self) -> Option<&T> {
        match self {
            Load::Ready(value) => Some(value),
            _ => None,
        }
    }
}

/// A choice in a list: its id and the words it shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Named {
    pub id: String,
    pub name: String,
}

impl Named {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
        }
    }
}

/// Thread naming on one computer (`GetTitleSettings`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Titles {
    /// `None` follows each session's agent.
    pub harness: Option<String>,
    pub model: Option<String>,
    pub harnesses: Vec<Named>,
    /// The chosen harness's models.
    pub models: Vec<Named>,
}

/// Worktree locations on one computer (`GetWorktreeSettings`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Worktrees {
    pub custom: bool,
    pub effective: String,
    pub default: String,
    /// Set by `OPENAGENTS_WORKTREES_DIR` on the computer: it wins.
    pub environment: Option<String>,
    /// The folder browser while choosing: the folder and its subfolders.
    pub browsing: Option<(String, Vec<Named>, bool)>,
}

/// An archived chat.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Archived {
    pub id: String,
    pub title: String,
    /// Where it ran and when, such as "Mac mini · 3 days ago".
    pub detail: String,
}

/// What the host read for the pages that aren't device choices.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Remote {
    /// The account's computers; the computer cards show one at a time.
    pub computers: Vec<Named>,
    pub computer: Option<String>,
    pub titles: Load<Titles>,
    pub worktrees: Load<Worktrees>,
    pub archived: Vec<Archived>,
    pub archived_page: usize,
    pub confirm_delete_all: bool,
    pub deleting: bool,
    pub account: account::State,
    pub cloud: cloud::State,
    /// Earn on the chosen computer (#12294).
    pub earn: earn::State,
    /// Providers: the chosen computer's agents and accounts (#12283).
    pub providers: providers::State,
    pub memory: memory::State,
    pub schedules: schedules::State,
    /// The browser lets the page show notifications (`Notification`
    /// permission), or `None` when it asks first.
    pub notifications_allowed: Option<bool>,
    /// The current platform names its primary modifier "⌘" (a Mac).
    pub mac: bool,
    /// The shortcut being recorded, and the last refusal.
    pub recording: Option<Shortcut>,
    pub shortcut_notice: Option<String>,
    /// The theme library's last import answer.
    pub theme_notice: Option<String>,
}

/// The view of `page`.
pub fn page(page: Page, prefs: &Prefs, remote: &Remote, paints: &Paints) -> Node<Intent> {
    let body = match page {
        Page::Memory => memory::page(&remote.memory, paints),
        Page::Schedules => schedules::page(&remote.schedules, paints),
        Page::General => general(prefs, remote, paints),
        Page::Appearance => appearance::page(&prefs.appearance, remote, paints),
        Page::Notifications => notifications(prefs, remote, paints),
        Page::Shortcuts => shortcuts::page(prefs, remote, paints),
        Page::Files => files(prefs, paints),
        Page::Archived => archived(remote, paints),
        Page::Earn => earn::page(remote, paints),
        Page::Account => account::page(&remote.account, paints),
        Page::Claude => cloud::claude(&remote.cloud, paints),
        Page::Data => cloud::data(&remote.cloud, remote.account.signed_in, paints),
        Page::Providers => providers::page(
            &remote.providers,
            &remote.computers,
            remote.computer.as_deref(),
            paints,
        ),
    };
    column(&format!("settings-{}", page.slug()), body)
}

// ---------------------------------------------------------------------
// The pages
// ---------------------------------------------------------------------

fn general(prefs: &Prefs, remote: &Remote, paints: &Paints) -> Vec<Node<Intent>> {
    let mac = remote.mac;
    let modifier = if mac { "⌘ Enter" } else { "Ctrl Enter" };
    let mut out = vec![
        header("general-title", "General", None, paints),
        card(
            "general-card",
            vec![
                row(
                    "send",
                    "Send messages with",
                    None,
                    segmented(
                        "send",
                        &[
                            ("Enter", Intent::SendWithModifier(false)),
                            (modifier, Intent::SendWithModifier(true)),
                        ],
                        usize::from(prefs.send_with_modifier),
                    ),
                    paints,
                ),
                row(
                    "escape",
                    "Stop agent with Escape",
                    Some("When no dialog or menu is open."),
                    vec![toggle(
                        "escape-toggle",
                        prefs.escape_stops_agent,
                        Intent::EscapeStopsAgent(!prefs.escape_stops_agent),
                    )],
                    paints,
                ),
            ],
            paints,
        ),
    ];
    out.extend(computer_cards(remote, paints));
    out
}

/// Thread naming and Worktrees, which each computer keeps for itself.
fn computer_cards(remote: &Remote, paints: &Paints) -> Vec<Node<Intent>> {
    if remote.computers.is_empty() {
        return vec![section(
            "computer",
            "Your computers",
            vec![row(
                "computer-none",
                "Thread naming and worktrees",
                Some(
                    "Each computer keeps these. Add a computer from the OpenAgents app to change them here.",
                ),
                Vec::new(),
                paints,
            )],
            paints,
        )];
    }
    let chosen = remote
        .computer
        .as_deref()
        .and_then(|id| remote.computers.iter().position(|c| c.id == id));
    let mut rows = Vec::new();
    if remote.computers.len() > 1 || chosen.is_none() {
        let options: Vec<(&str, Intent)> = remote
            .computers
            .iter()
            .map(|c| (c.name.as_str(), Intent::Computer(c.id.clone())))
            .collect();
        rows.push(wide_row(
            "computer-pick",
            "Computer",
            Some("Thread naming and worktrees are kept by each computer."),
            segmented("computer", &options, chosen.unwrap_or(usize::MAX)),
            paints,
        ));
    }
    let mut out = Vec::new();
    if !rows.is_empty() {
        out.push(card("computer-card", rows, paints));
    }
    if chosen.is_none() {
        return out;
    }
    out.push(thread_naming(&remote.titles, paints));
    out.push(worktrees(&remote.worktrees, paints));
    out
}

fn loading_line(key: &str, load: &Load<impl Sized>, paints: &Paints) -> Option<Node<Intent>> {
    match load {
        Load::Idle | Load::Loading => Some(text(key, "Loading…", paints.muted)),
        Load::Failed(error) => Some(text(key, error, paints.muted)),
        Load::Ready(_) => None,
    }
}

fn thread_naming(load: &Load<Titles>, paints: &Paints) -> Node<Intent> {
    let Some(titles) = load.ready() else {
        let line = loading_line("titles-state", load, paints)
            .unwrap_or_else(|| text("titles-state", "", paints.muted));
        return card(
            "titles-card",
            vec![row("titles", "Thread naming", None, vec![line], paints)],
            paints,
        );
    };
    let follows = titles.harness.is_none();
    let description = if follows {
        "Each thread is named by its own agent."
    } else {
        "A small model keeps titles fast and cheap."
    };
    let mut agents: Vec<(&str, Intent)> = vec![("Session agent", Intent::TitleHarness(None))];
    agents.extend(
        titles
            .harnesses
            .iter()
            .map(|h| (h.name.as_str(), Intent::TitleHarness(Some(h.id.clone())))),
    );
    let chosen = match &titles.harness {
        None => 0,
        Some(id) => titles
            .harnesses
            .iter()
            .position(|h| &h.id == id)
            .map_or(usize::MAX, |ix| ix + 1),
    };
    let mut rows = vec![wide_row(
        "titles",
        "Thread naming",
        Some(description),
        segmented("titles-agent", &agents, chosen),
        paints,
    )];
    if !follows && !titles.models.is_empty() {
        let mut models: Vec<(&str, Intent)> = vec![("Cheapest", Intent::TitleModel(None))];
        models.extend(
            titles
                .models
                .iter()
                .map(|m| (m.name.as_str(), Intent::TitleModel(Some(m.id.clone())))),
        );
        let chosen = match &titles.model {
            None => 0,
            Some(id) => titles
                .models
                .iter()
                .position(|m| &m.id == id)
                .map_or(usize::MAX, |ix| ix + 1),
        };
        rows.push(wide_row(
            "titles-model-row",
            "Model",
            None,
            segmented("titles-model", &models, chosen),
            paints,
        ));
    }
    card("titles-card", rows, paints)
}

fn worktrees(load: &Load<Worktrees>, paints: &Paints) -> Node<Intent> {
    let Some(settings) = load.ready() else {
        let line = loading_line("worktrees-state", load, paints)
            .unwrap_or_else(|| text("worktrees-state", "", paints.muted));
        return section(
            "worktrees",
            "Worktrees",
            vec![row(
                "worktrees-row",
                "Custom worktree location",
                None,
                vec![line],
                paints,
            )],
            paints,
        );
    };
    let mut detail = settings.effective.clone();
    if settings.environment.is_some() {
        detail.push_str(" · Set by OPENAGENTS_WORKTREES_DIR");
    }
    let mut controls = Vec::new();
    if settings.environment.is_none() {
        if settings.custom {
            controls.push(button(
                "worktrees-default",
                "Reset to default",
                Intent::WorktreeDefault,
            ));
        }
        if settings.browsing.is_none() {
            controls.push(button(
                "worktrees-choose",
                "Choose…",
                Intent::WorktreeBrowse(Some(settings.effective.clone())),
            ));
        }
    }
    let mut rows = vec![row(
        "worktrees-row",
        "Custom worktree location",
        Some(&detail),
        controls,
        paints,
    )];
    if let Some((folder, children, truncated)) = &settings.browsing {
        let mut list: Vec<Node<Intent>> = vec![node(
            "worktrees-browse-head",
            Style {
                gap_points: Some(8),
                ..Style::default()
            },
            Element::Stack {
                axis: Axis::Horizontal,
                children: vec![
                    text("worktrees-folder", folder, paints.text),
                    button(
                        "worktrees-use",
                        "Use this folder",
                        Intent::WorktreeUse(folder.clone()),
                    ),
                    button(
                        "worktrees-up",
                        "Up",
                        Intent::WorktreeBrowse(Some(parent(folder))),
                    ),
                    button("worktrees-cancel", "Cancel", Intent::WorktreeBrowse(None)),
                ],
            },
        )];
        if children.is_empty() {
            list.push(text("worktrees-empty", "No subfolders", paints.muted));
        }
        let shown: Vec<Node<Intent>> = children
            .iter()
            .take(60)
            .enumerate()
            .map(|(ix, child)| {
                button(
                    &format!("worktrees-dir-{ix}"),
                    &child.name,
                    Intent::WorktreeBrowse(Some(child.id.clone())),
                )
            })
            .collect();
        if !shown.is_empty() {
            list.push(node(
                "worktrees-dirs",
                Style::default(),
                Element::Stack {
                    axis: Axis::Wrap,
                    children: shown,
                },
            ));
        }
        if *truncated || children.len() > 60 {
            list.push(text(
                "worktrees-more",
                "More folders exist than are shown here.",
                paints.muted,
            ));
        }
        rows.push(node(
            "worktrees-browser",
            Style {
                gap_points: Some(8),
                ..Style::default()
            },
            Element::Stack {
                axis: Axis::Vertical,
                children: list,
            },
        ));
    }
    section("worktrees", "Worktrees", rows, paints)
}

/// The folder above `path`, or `path` at the top.
fn parent(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    match trimmed.rfind('/') {
        Some(0) => "/".into(),
        Some(ix) => trimmed[..ix].into(),
        None => path.into(),
    }
}

fn notifications(prefs: &Prefs, remote: &Remote, paints: &Paints) -> Vec<Node<Intent>> {
    let sound = prefs.sound_enabled;
    let sub = |key: &str, title: &str, which: Sound, on: bool| {
        row(
            key,
            title,
            None,
            vec![if sound {
                toggle(&format!("{key}-toggle"), on, Intent::Sound(which, !on))
            } else {
                disabled_toggle(&format!("{key}-toggle"), on)
            }],
            paints,
        )
    };
    let notify = prefs.notifications_enabled;
    let mut desktop = vec![row(
        "notify",
        "Browser notifications",
        match remote.notifications_allowed {
            Some(false) if notify => Some(
                "Your browser blocks notifications from this site. Allow them in its site settings.",
            ),
            None if notify => Some("Your browser asks the first time."),
            _ => None,
        },
        vec![toggle(
            "notify-toggle",
            notify,
            Intent::Notifications(!notify),
        )],
        paints,
    )];
    let background = prefs.notifications_background_only;
    desktop.push(row(
        "notify-background",
        "Only when in the background",
        None,
        vec![if notify {
            toggle(
                "notify-background-toggle",
                background,
                Intent::NotificationsBackgroundOnly(!background),
            )
        } else {
            disabled_toggle("notify-background-toggle", background)
        }],
        paints,
    ));
    vec![
        header("notifications-title", "Notifications", None, paints),
        section("notify-section", "Browser", desktop, paints),
        section(
            "sounds",
            "Sounds",
            vec![
                row(
                    "sound",
                    "Session sounds",
                    None,
                    vec![toggle(
                        "sound-toggle",
                        sound,
                        Intent::Sound(Sound::All, !sound),
                    )],
                    paints,
                ),
                sub(
                    "sound-completion",
                    "Task completed",
                    Sound::Completion,
                    prefs.sound_completion,
                ),
                sub(
                    "sound-input",
                    "Input required",
                    Sound::Input,
                    prefs.sound_input,
                ),
                sub(
                    "sound-attention",
                    "Errors and disconnections",
                    Sound::Attention,
                    prefs.sound_attention,
                ),
            ],
            paints,
        ),
    ]
}

fn files(prefs: &Prefs, paints: &Paints) -> Vec<Node<Intent>> {
    let files = prefs.files;
    let switch = |key: &str, on: bool, change: fn(bool) -> FilesChange| {
        toggle(key, on, Intent::Files(change(!on)))
    };
    let labels: Vec<String> = FilesPrefs::DELAYS
        .iter()
        .map(|ms| delay_label(*ms))
        .collect();
    let delays: Vec<(&str, Intent)> = FilesPrefs::DELAYS
        .iter()
        .zip(&labels)
        .map(|(ms, label)| (label.as_str(), Intent::Files(FilesChange::Delay(*ms))))
        .collect();
    let mut rows = vec![row(
        "files-autosave",
        "Autosave",
        None,
        vec![switch(
            "files-autosave-toggle",
            files.autosave,
            FilesChange::Autosave,
        )],
        paints,
    )];
    if files.autosave {
        rows.push(wide_row(
            "files-delay",
            "Autosave delay",
            None,
            segmented(
                "files-delay",
                &delays,
                FilesPrefs::DELAYS
                    .iter()
                    .position(|ms| *ms == files.autosave_delay_ms)
                    .unwrap_or(usize::MAX),
            ),
            paints,
        ));
    }
    rows.push(row(
        "files-wrap",
        "Word wrap",
        None,
        vec![switch(
            "files-wrap-toggle",
            files.word_wrap,
            FilesChange::WordWrap,
        )],
        paints,
    ));
    rows.push(row(
        "files-all",
        "Show hidden and ignored files",
        None,
        vec![switch(
            "files-all-toggle",
            files.show_hidden,
            FilesChange::ShowHidden,
        )],
        paints,
    ));
    vec![
        header("files-title", "Files", None, paints),
        card("files-card", rows, paints),
    ]
}

/// Archived chats per page, as the desktop pages them.
pub const ARCHIVED_PAGE: usize = 25;

fn archived(remote: &Remote, paints: &Paints) -> Vec<Node<Intent>> {
    let mut out = vec![header(
        "archived-title",
        "Archived sessions",
        Some("Hidden from the sidebar until restored."),
        paints,
    )];
    if remote.archived.is_empty() {
        out.push(card(
            "archived-empty",
            vec![
                text("archived-none", "Nothing archived", paints.text),
                text(
                    "archived-hint",
                    "Right-click a session in the sidebar to archive it.",
                    paints.muted,
                ),
            ],
            paints,
        ));
        return out;
    }
    let pages = remote.archived.len().div_ceil(ARCHIVED_PAGE);
    let at = remote.archived_page.min(pages.saturating_sub(1));
    let rows: Vec<Node<Intent>> = remote
        .archived
        .iter()
        .skip(at * ARCHIVED_PAGE)
        .take(ARCHIVED_PAGE)
        .enumerate()
        .map(|(ix, chat)| {
            row(
                &format!("archived-{ix}"),
                &chat.title,
                Some(&chat.detail),
                vec![button(
                    &format!("archived-{ix}-unarchive"),
                    "Unarchive",
                    Intent::Unarchive(chat.id.clone()),
                )],
                paints,
            )
        })
        .collect();
    out.push(card("archived-list", rows, paints));
    let mut footer = Vec::new();
    if pages > 1 {
        footer.push(enabled_button(
            "archived-prev",
            "Previous",
            at > 0,
            Intent::ArchivedPage(at.saturating_sub(1)),
        ));
        footer.push(text(
            "archived-at",
            &format!("{} of {pages}", at + 1),
            paints.muted,
        ));
        footer.push(enabled_button(
            "archived-next",
            "Next",
            at + 1 < pages,
            Intent::ArchivedPage(at + 1),
        ));
    }
    if remote.deleting {
        footer.push(text("archived-deleting", "Deleting…", paints.muted));
    } else if remote.confirm_delete_all {
        footer.push(text(
            "archived-confirm",
            "Delete all archived sessions?",
            paints.text,
        ));
        footer.push(danger_button(
            "archived-delete-yes",
            "Delete all",
            Intent::DeleteAllArchived,
            paints,
        ));
        footer.push(button(
            "archived-delete-no",
            "Cancel",
            Intent::ConfirmDeleteAll(false),
        ));
    } else {
        footer.push(button(
            "archived-delete",
            "Delete all",
            Intent::ConfirmDeleteAll(true),
        ));
    }
    out.push(node(
        "archived-footer",
        Style {
            gap_points: Some(8),
            ..Style::default()
        },
        Element::Stack {
            axis: Axis::Horizontal,
            children: footer,
        },
    ));
    out
}

// ---------------------------------------------------------------------
// Pieces the pages share
// ---------------------------------------------------------------------

/// A node key for a row named by outside data (an id, a repository, a
/// computer's name): `prefix-raw` when `raw` is a valid key already, else
/// its key characters and a short hash of it, so two names never share one.
pub(crate) fn key(prefix: &str, raw: &str) -> String {
    let plain = format!("{prefix}-{raw}");
    let valid = |b: u8| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b':');
    if plain.len() <= 96 && plain.bytes().all(valid) {
        return plain;
    }
    let hash = raw.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    });
    let kept: String = raw
        .bytes()
        .filter(|b| valid(*b))
        .take(60)
        .map(char::from)
        .collect();
    format!("{prefix}-{kept}-{hash:016x}")
}

pub(crate) fn node(key: &str, style: Style, element: Element<Intent>) -> Node<Intent> {
    Node {
        key: key.into(),
        style,
        element,
    }
}

/// The page's column.
pub(crate) fn column(key: &str, children: Vec<Node<Intent>>) -> Node<Intent> {
    node(
        key,
        Style {
            gap_points: Some(20),
            padding_points: Some([24, 24, 40, 24]),
            ..Style::default()
        },
        Element::Stack {
            axis: Axis::Vertical,
            children,
        },
    )
}

pub(crate) fn text(key: &str, value: &str, color: Color) -> Node<Intent> {
    node(
        key,
        Style {
            foreground: Some(color),
            ..Style::default()
        },
        Element::Text {
            value: value.into(),
            role: TextRole::Body,
        },
    )
}

/// The page's title and an optional line under it.
pub(crate) fn header(
    key: &str,
    title: &str,
    subtitle: Option<&str>,
    paints: &Paints,
) -> Node<Intent> {
    let mut children = vec![node(
        &format!("{key}-text"),
        Style {
            foreground: Some(paints.text),
            weight: Some(TextWeight::Semibold),
            text_size: Some(20),
            ..Style::default()
        },
        Element::Text {
            value: title.into(),
            role: TextRole::Heading,
        },
    )];
    if let Some(subtitle) = subtitle {
        children.push(text(&format!("{key}-sub"), subtitle, paints.muted));
    }
    node(
        key,
        Style {
            gap_points: Some(4),
            ..Style::default()
        },
        Element::Stack {
            axis: Axis::Vertical,
            children,
        },
    )
}

/// A card of rows.
pub(crate) fn card(key: &str, rows: Vec<Node<Intent>>, paints: &Paints) -> Node<Intent> {
    node(
        key,
        Style {
            background: Some(paints.card),
            border: Some(paints.border),
            radius: Some(12),
            padding_points: Some([4, 16, 4, 16]),
            gap_points: Some(0),
            ..Style::default()
        },
        Element::Stack {
            axis: Axis::Vertical,
            children: rows,
        },
    )
}

/// A small label over a card.
pub(crate) fn section(
    key: &str,
    title: &str,
    rows: Vec<Node<Intent>>,
    paints: &Paints,
) -> Node<Intent> {
    node(
        key,
        Style {
            gap_points: Some(8),
            ..Style::default()
        },
        Element::Stack {
            axis: Axis::Vertical,
            children: vec![
                node(
                    &format!("{key}-label"),
                    Style {
                        foreground: Some(paints.muted),
                        weight: Some(TextWeight::Medium),
                        padding_points: Some([0, 4, 0, 4]),
                        ..Style::default()
                    },
                    Element::Text {
                        value: title.into(),
                        role: TextRole::Body,
                    },
                ),
                card(&format!("{key}-card"), rows, paints),
            ],
        },
    )
}

/// One row: its title and detail on the left, its controls on the right.
pub(crate) fn row(
    key: &str,
    title: &str,
    detail: Option<&str>,
    controls: Vec<Node<Intent>>,
    paints: &Paints,
) -> Node<Intent> {
    let mut words = vec![node(
        &format!("{key}-title"),
        Style {
            foreground: Some(paints.text),
            weight: Some(TextWeight::Medium),
            text_size: Some(13),
            ..Style::default()
        },
        Element::Text {
            value: title.into(),
            role: TextRole::Body,
        },
    )];
    if let Some(detail) = detail.filter(|d| !d.is_empty()) {
        words.push(text(&format!("{key}-detail"), detail, paints.muted));
    }
    let mut children = vec![node(
        &format!("{key}-words"),
        Style {
            fill_height: Some(true),
            gap_points: Some(2),
            ..Style::default()
        },
        Element::Stack {
            axis: Axis::Vertical,
            children: words,
        },
    )];
    if !controls.is_empty() {
        // A column around the controls keeps them at their own height
        // beside a taller title and detail.
        children.push(node(
            &format!("{key}-controls-column"),
            Style {
                intrinsic_width: Some(true),
                ..Style::default()
            },
            Element::Stack {
                axis: Axis::Vertical,
                children: vec![node(
                    &format!("{key}-controls"),
                    Style {
                        gap_points: Some(6),
                        ..Style::default()
                    },
                    Element::Stack {
                        axis: Axis::Wrap,
                        children: controls,
                    },
                )],
            },
        ));
    }
    node(
        key,
        Style {
            padding_points: Some([12, 0, 12, 0]),
            gap_points: Some(16),
            ..Style::default()
        },
        Element::Stack {
            axis: Axis::Horizontal,
            children,
        },
    )
}

/// A row whose choices go under its title, for more than fit beside it.
pub(crate) fn wide_row(
    key: &str,
    title: &str,
    detail: Option<&str>,
    controls: Vec<Node<Intent>>,
    paints: &Paints,
) -> Node<Intent> {
    let mut children = Vec::new();
    children.push(node(
        &format!("{key}-title"),
        Style {
            foreground: Some(paints.text),
            weight: Some(TextWeight::Medium),
            text_size: Some(13),
            ..Style::default()
        },
        Element::Text {
            value: title.into(),
            role: TextRole::Body,
        },
    ));
    if let Some(detail) = detail.filter(|d| !d.is_empty()) {
        children.push(text(&format!("{key}-detail"), detail, paints.muted));
    }
    children.push(node(
        &format!("{key}-controls"),
        Style {
            gap_points: Some(6),
            padding_points: Some([6, 0, 0, 0]),
            ..Style::default()
        },
        Element::Stack {
            axis: Axis::Wrap,
            children: controls,
        },
    ));
    node(
        key,
        Style {
            padding_points: Some([12, 0, 12, 0]),
            gap_points: Some(2),
            ..Style::default()
        },
        Element::Stack {
            axis: Axis::Vertical,
            children,
        },
    )
}

pub(crate) fn button(key: &str, label: &str, intent: Intent) -> Node<Intent> {
    enabled_button(key, label, true, intent)
}

pub(crate) fn enabled_button(
    key: &str,
    label: &str,
    enabled: bool,
    intent: Intent,
) -> Node<Intent> {
    node(
        key,
        Style::default(),
        Element::Button {
            label: label.into(),
            enabled,
            icon: None,
            shortcut: None,
            intent,
        },
    )
}

pub(crate) fn danger_button(
    key: &str,
    label: &str,
    intent: Intent,
    paints: &Paints,
) -> Node<Intent> {
    node(
        key,
        Style {
            foreground: Some(paints.danger),
            ..Style::default()
        },
        Element::Button {
            label: label.into(),
            enabled: true,
            icon: None,
            shortcut: None,
            intent,
        },
    )
}

/// An on/off switch: a choice that reads its state.
pub(crate) fn toggle(key: &str, on: bool, intent: Intent) -> Node<Intent> {
    node(
        key,
        Style::default(),
        Element::Choice {
            label: if on { "On" } else { "Off" }.into(),
            selected: on,
            enabled: true,
            intent,
            children: Vec::new(),
        },
    )
}

/// A switch whose parent setting is off.
fn disabled_toggle(key: &str, on: bool) -> Node<Intent> {
    node(
        key,
        Style::default(),
        Element::Choice {
            label: if on { "On" } else { "Off" }.into(),
            selected: false,
            enabled: false,
            intent: Intent::Open(Page::Notifications),
            children: Vec::new(),
        },
    )
}

/// One of several choices; `chosen` is the selected one's position.
pub(crate) fn segmented(key: &str, options: &[(&str, Intent)], chosen: usize) -> Vec<Node<Intent>> {
    options
        .iter()
        .enumerate()
        .map(|(ix, (label, intent))| {
            node(
                &format!("{key}-{ix}"),
                Style::default(),
                Element::Choice {
                    label: (*label).into(),
                    selected: ix == chosen,
                    enabled: true,
                    intent: intent.clone(),
                    children: Vec::new(),
                },
            )
        })
        .collect()
}

#[cfg(test)]
mod tests;
