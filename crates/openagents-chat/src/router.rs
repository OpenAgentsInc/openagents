//! The phone's side of the chat router
//! (`docs/coder/design/2026-09-28-chat-router.md`, On the wire).
//!
//! A chat turn asks the worker for routing (`router`) and says what the
//! phone can do next (`context`): which surface it is, whether a computer
//! is ready, and the app's build. Since #10077 it also says where Coder
//! runs for this chat ([`Computer`]): this device itself, with its coding
//! agents' readiness, or the phone's paired computer by the name the
//! person gave it; and the chat's project folder ([`Project`]), with its
//! path only on a computer. The context carries no credential, key, host
//! address, or amount, and every field is bounded.
//!
//! The worker answers with typed observations beside its text: the
//! judgment (which prepared answer, route, and tier), and offers (`offer`
//! feedback). An offer is never permission. The phone reads each one
//! against its own closed tables ([`Screen`], [`READ_ONLY`]) and shows it
//! as a control; nothing happens until the person taps it, and what the tap
//! does is decided here, never by the offer's own words. A label the worker
//! sends is ignored: the phone names every control itself.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The routing question set a turn asks for: `chat-router-v2`, with the
/// Gym and eval routes, their cards, and the eval offers.
pub const ROUTER: &str = "chat-router-v2";
/// The most cards one reply keeps.
const MAX_CARDS: usize = 4;
/// The most offers one reply keeps.
const MAX_OFFERS: usize = 4;
/// The most follow-up suggestions one reply keeps.
pub const MAX_FOLLOWUPS: usize = 3;
/// The longest follow-up suggestion, in characters.
const MAX_FOLLOWUP_CHARS: usize = 80;
/// The most words one proposed command has, and the longest word.
const MAX_ARGV: usize = 8;
const MAX_ARG_BYTES: usize = 200;
/// The bounds of a proposed GitHub change's words (#11167): NIP-CJ's own
/// for a `cli` offer, since a title or a comment is one word.
const MAX_GITHUB_ARGV: usize = 32;
const MAX_GITHUB_ARG_BYTES: usize = 256;
/// The most bytes of the worker's judgment the phone keeps, for a tester
/// who shares the chat.
const MAX_JUDGMENT_BYTES: usize = playtest::report::MAX_JUDGMENT_BYTES;

/// The native surface that sends this hosted conversation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Surface {
    #[default]
    Phone,
    Desktop,
    /// A terminal on this computer: the `openagents chat` command and
    /// OpenAgents Terminal, with or without a host between them and the
    /// worker ([`Caller`]).
    Terminal,
    /// The terminal on the openagents.com homepage (#10106): answers and
    /// knowledge only, never Coder, a computer, or an offer.
    Web,
}
impl Surface {
    pub fn word(self) -> &'static str {
        match self {
            Self::Phone => "phone",
            Self::Desktop => "desktop",
            Self::Terminal => "terminal",
            Self::Web => "web",
        }
    }

    /// The request's `client` word for this surface, when the caller names
    /// no program of its own ([`Context::client`]).
    pub fn client(self) -> &'static str {
        self.program().word()
    }

    /// The program that sends this surface's requests by default.
    pub fn program(self) -> ClientWord {
        match self {
            Self::Phone => ClientWord::Mobile,
            Self::Desktop => ClientWord::Desktop,
            Self::Terminal => ClientWord::Cli,
            Self::Web => ClientWord::Web,
        }
    }
}

/// The program that sends a request: the request's `client` word. Two
/// programs share the terminal surface: `openagents chat` (scripts and
/// one-shot commands) and OpenAgents Terminal (the full-screen chat), so
/// worker logs can tell them apart. A closed list; the router never reads
/// it to choose a route.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClientWord {
    #[serde(rename = "openagents-mobile")]
    Mobile,
    #[serde(rename = "openagents-desktop")]
    Desktop,
    /// The `openagents chat` command.
    #[serde(rename = "openagents-cli")]
    Cli,
    /// OpenAgents Terminal (`docs/terminal`).
    #[serde(rename = "openagents-terminal")]
    Terminal,
    #[serde(rename = "openagents-web")]
    Web,
}

impl ClientWord {
    pub fn word(self) -> &'static str {
        match self {
            Self::Mobile => "openagents-mobile",
            Self::Desktop => "openagents-desktop",
            Self::Cli => "openagents-cli",
            Self::Terminal => "openagents-terminal",
            Self::Web => "openagents-web",
        }
    }
}

/// Who asks through a host (#10108): the surface a turn comes from and the
/// program that sends it, carried over the host's control socket so the
/// router sees `terminal` for a terminal's message whichever backend carries
/// it. A host admits only the surfaces of its own computer
/// ([`Caller::local`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Caller {
    pub surface: Surface,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<ClientWord>,
}

impl Caller {
    /// The `openagents chat` command.
    pub const CLI: Self = Self {
        surface: Surface::Terminal,
        client: Some(ClientWord::Cli),
    };
    /// OpenAgents Terminal.
    pub const TERMINAL: Self = Self {
        surface: Surface::Terminal,
        client: Some(ClientWord::Terminal),
    };
    /// The desktop app, the host's own default.
    pub const DESKTOP: Self = Self {
        surface: Surface::Desktop,
        client: None,
    };

    /// Whether a program on the host's own computer can be this caller: the
    /// desktop or a terminal, never a phone or the website, which reach a
    /// host only through their own doors.
    pub fn local(self) -> bool {
        matches!(self.surface, Surface::Desktop | Surface::Terminal)
    }

    /// The request's `client` word.
    pub fn client_word(self) -> &'static str {
        self.client.unwrap_or(self.surface.program()).word()
    }
}

/// The most characters of a computer's name the context carries.
pub const MAX_COMPUTER_NAME_CHARS: usize = 64;
/// The most coding agents the context names: every engine a local run can
/// use (Codex, Claude Code, Grok Build, OpenCode, Devin), with room to grow
/// (#10113). A worker that reads fewer takes the first ones.
pub const MAX_ENGINES: usize = 8;
/// The most bytes of a project folder's name the context carries.
pub const MAX_PROJECT_NAME_BYTES: usize = 128;
/// The most bytes of a project folder's path the context carries.
pub const MAX_PROJECT_PATH_BYTES: usize = 1024;
/// The most memory notes the context carries (#11182).
pub const MAX_MEMORY_NOTES: usize = 40;
/// The most bytes of all memory notes together; the newest come first, and
/// the rest are left out.
pub const MAX_MEMORY_BYTES: usize = 16 * 1024;
/// The most bytes of one memory note's body.
pub const MAX_MEMORY_BODY_BYTES: usize = 2 * 1024;

/// A note the user saved to their account's memory (#11182): what Coder
/// remembers about them, which the web chat sends so its answers know it
/// too. Data, never an instruction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryNote {
    pub name: String,
    /// `user`, `feedback`, `project`, or `reference`.
    pub kind: String,
    pub description: String,
    pub body: String,
}

/// The `memory` array for `notes`: newest first as given, each note's name
/// and description on one line, its body cut at
/// [`MAX_MEMORY_BODY_BYTES`], at most [`MAX_MEMORY_NOTES`] within
/// [`MAX_MEMORY_BYTES`].
fn memory_json(notes: &[MemoryNote]) -> Vec<Value> {
    let one_line = |text: &str, chars: usize| -> String {
        text.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .filter(|ch| !ch.is_control())
            .take(chars)
            .collect()
    };
    let mut out = Vec::new();
    let mut bytes = 0;
    for note in notes.iter().take(MAX_MEMORY_NOTES) {
        let name = one_line(&note.name, 80);
        let body: String = {
            let body = note.body.trim();
            let mut end = body.len().min(MAX_MEMORY_BODY_BYTES);
            while !body.is_char_boundary(end) {
                end -= 1;
            }
            body[..end].to_owned()
        };
        if name.is_empty() || body.is_empty() {
            continue;
        }
        let kind = match note.kind.as_str() {
            kind @ ("user" | "feedback" | "project" | "reference") => kind,
            _ => "user",
        };
        let description = one_line(&note.description, 200);
        let size = name.len() + description.len() + body.len();
        if bytes + size > MAX_MEMORY_BYTES {
            break;
        }
        bytes += size;
        out.push(json!({"name": name, "kind": kind, "description": description, "body": body}));
    }
    out
}

/// Where Coder runs for this chat, as a turn tells the worker (#10077).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Computer {
    /// This device is itself a Coder computer: the desktop app, or
    /// `openagents chat` on a computer. `name` is the label the person gave
    /// it, when it has one.
    Here {
        name: Option<String>,
        engines: Vec<Engine>,
    },
    /// The phone's ready paired computer, by the label the person gave it,
    /// with the coding agents its presence names (#10119): empty for a
    /// computer that predates them.
    Paired { name: String, engines: Vec<Engine> },
}

/// One coding agent on this computer and whether a Coder run may use it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Engine {
    /// `codex` or `claude`: lowercase letters, digits, `-`, or `_`, at
    /// most 16 bytes.
    pub engine: String,
    pub state: EngineState,
}

/// A coding agent's readiness, without its account or usage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineState {
    Ready,
    NotSignedIn,
    /// Signed in, but at or near a usage limit.
    Limited,
    /// Installed or signed in here, but the person turned it off in
    /// Coder's settings (`coder.disabled`, #10113, #10184): agents are
    /// opt-out, so this is only ever the person's own choice. A worker
    /// that does not know the word leaves the engine out.
    NotEnabled,
}

impl EngineState {
    /// The state's wire word, as `context.computer.engines` and a host's
    /// presence (#10119) carry it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::NotSignedIn => "not_signed_in",
            Self::Limited => "limited",
            Self::NotEnabled => "not_enabled",
        }
    }

    /// The state a wire word names, or `None` for a word this build does
    /// not know.
    #[must_use]
    pub fn of_word(word: &str) -> Option<Self> {
        [
            Self::Ready,
            Self::NotSignedIn,
            Self::Limited,
            Self::NotEnabled,
        ]
        .into_iter()
        .find(|state| state.word() == word)
    }
}

impl Engine {
    /// The agents a Coder run here would weigh, from the run's own
    /// prediction ([`crate::coder_events::Runner`]): the one that runs is
    /// ready, the ones passed over are not signed in or limited.
    #[must_use]
    pub fn from_runner(runner: &crate::coder_events::Runner) -> Vec<Engine> {
        use crate::coder_events::{PassedOver, Runner};
        let mut engines = Vec::new();
        let mut push = |engine: &str, state| {
            if !engines.iter().any(|known: &Engine| known.engine == engine) {
                engines.push(Engine {
                    engine: engine.to_owned(),
                    state,
                });
            }
        };
        match runner {
            Runner::Runs {
                provider, passed, ..
            } => {
                for over in passed {
                    let state = match over.why {
                        PassedOver::NotSignedIn => EngineState::NotSignedIn,
                        // Not a readiness: the person turned it off (#10076, #10184).
                        PassedOver::NotAllowed => continue,
                        _ => EngineState::Limited,
                    };
                    push(&over.provider, state);
                }
                push(provider, EngineState::Ready);
            }
            Runner::NotSignedIn { providers } => {
                for provider in providers {
                    push(provider, EngineState::NotSignedIn);
                }
            }
            Runner::NoCapacity { .. } => {}
        }
        engines.retain(Engine::bounded);
        engines.truncate(MAX_ENGINES);
        engines
    }

    /// Whether the engine's word is a bounded word the wire carries.
    #[must_use]
    pub fn bounded(&self) -> bool {
        (1..=16).contains(&self.engine.len())
            && self
                .engine
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
    }
}

/// The chat's project folder: its name, and on a computer its path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    pub name: String,
    /// The folder's absolute path, sent only from a computer ([`Computer::Here`]).
    pub path: Option<String>,
}

impl Project {
    /// The project for `folder`, an absolute path: its last component names
    /// it. `None` for a path with no name.
    #[must_use]
    pub fn at(folder: &str) -> Option<Project> {
        let name = std::path::Path::new(folder)
            .file_name()?
            .to_str()?
            .to_owned();
        Some(Project {
            name,
            path: Some(folder.to_owned()),
        })
    }
}

/// The most bytes of a finished run's summary the context carries.
pub const MAX_RUN_SUMMARY_BYTES: usize = 4 * 1024;
/// The most changed files the context names.
pub const MAX_RUN_FILES: usize = 32;
/// The longest changed file's path, in bytes.
pub const MAX_RUN_PATH_BYTES: usize = 512;
/// The most commands the context names, the last ones run.
pub const MAX_RUN_COMMANDS: usize = 16;
/// The longest command, in bytes: its first line, cut.
pub const MAX_RUN_COMMAND_BYTES: usize = 200;
/// The longest model name, in bytes.
pub const MAX_RUN_MODEL_BYTES: usize = 64;

/// How the chat's Coder run ended its last turn (#10094), or that the turn
/// is still going (#10143).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunEnding {
    /// It finished with a result.
    Finished,
    /// It ended without finishing.
    Failed,
    /// The person or the host stopped it.
    Stopped,
    /// Its turn is still going: started, queued, or working.
    Running,
    /// Its turn waits for the person's answer or approval.
    Waiting,
}

impl RunEnding {
    /// The word the wire carries.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Finished => "finished",
            Self::Failed => "failed",
            Self::Stopped => "stopped",
            Self::Running => "running",
            Self::Waiting => "waiting",
        }
    }
}

/// One file the run's last turn changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunFile {
    pub path: String,
    /// `added`, `modified`, `deleted`, or `renamed`.
    pub status: String,
}

/// The result of the chat's Coder run, once its turn has ended, as a turn
/// tells the worker (#10094): how it ended, which engine ran it, its
/// summary, the files it changed, and the commands it ran. The chat answers
/// questions about the run from it, and the router decides whether a
/// follow-up is a question for the chat or more work for Coder's next turn.
/// These are the person's own data: the worker gives the summary, files,
/// and commands only to the chat model's instructions; Jev reads only how
/// the run ended. Every field is bounded ([`CoderRun::json`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoderRun {
    pub ending: RunEnding,
    /// The turn that ended, from one.
    pub turn: usize,
    /// The engine's word, such as `codex` or `claude`.
    pub engine: Option<String>,
    pub model: Option<String>,
    /// What Coder said it did, or why it failed or stopped.
    pub summary: String,
    pub files: Vec<RunFile>,
    /// The turn's commands, oldest first.
    pub commands: Vec<String>,
}

/// At most `max` bytes of `text`, cut at a character boundary.
fn cut(text: &str, max: usize) -> String {
    let text = text.trim();
    if text.len() <= max {
        return text.to_owned();
    }
    let mut end = max.saturating_sub('…'.len_utf8());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

/// A bounded word: lowercase letters, digits, `-`, or `_`, 1 to 16 bytes.
fn word_like(word: &str) -> bool {
    (1..=16).contains(&word.len())
        && word
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

impl CoderRun {
    /// The request's `context.coder_run`: the summary cut to
    /// [`MAX_RUN_SUMMARY_BYTES`] (line breaks kept, other control
    /// characters dropped), at most [`MAX_RUN_FILES`] files and the last
    /// [`MAX_RUN_COMMANDS`] commands, each command its first line cut to
    /// [`MAX_RUN_COMMAND_BYTES`]. A path past its bound or with a control
    /// character, or an engine that is not a bounded word, is left out.
    #[must_use]
    pub fn json(&self) -> Value {
        let summary: String = self
            .summary
            .chars()
            .filter(|ch| !ch.is_control() || *ch == '\n' || *ch == '\t')
            .collect();
        let commands: Vec<String> = self
            .commands
            .iter()
            .filter_map(|command| {
                let line = command.lines().find(|line| !line.trim().is_empty())?;
                let line: String = line.chars().filter(|ch| !ch.is_control()).collect();
                (!line.trim().is_empty()).then(|| cut(&line, MAX_RUN_COMMAND_BYTES))
            })
            .collect();
        let skip = commands.len().saturating_sub(MAX_RUN_COMMANDS);
        let mut value = json!({
            "ending": self.ending.word(),
            "turn": self.turn.clamp(1, 9_999),
            "summary": cut(&summary, MAX_RUN_SUMMARY_BYTES),
            "files": self
                .files
                .iter()
                .filter(|file| {
                    plain(&file.path, MAX_RUN_PATH_BYTES, MAX_RUN_PATH_BYTES)
                        && word_like(&file.status)
                })
                .take(MAX_RUN_FILES)
                .map(|file| json!({"path": file.path, "status": file.status}))
                .collect::<Vec<_>>(),
            "commands": &commands[skip..],
        });
        if let Some(engine) = self.engine.as_deref().filter(|word| word_like(word)) {
            value["engine"] = json!(engine);
        }
        if let Some(model) = self
            .model
            .as_deref()
            .filter(|model| plain(model, MAX_RUN_MODEL_BYTES, MAX_RUN_MODEL_BYTES))
        {
            value["model"] = json!(model);
        }
        value
    }
}

/// Printable, not blank, and at most `max_bytes` bytes and `max_chars`
/// characters.
fn plain(text: &str, max_bytes: usize, max_chars: usize) -> bool {
    !text.trim().is_empty()
        && text.len() <= max_bytes
        && text.chars().count() <= max_chars
        && !text.chars().any(char::is_control)
}

impl Computer {
    fn json(&self) -> Option<Value> {
        let name_ok =
            |name: &str| plain(name, 4 * MAX_COMPUTER_NAME_CHARS, MAX_COMPUTER_NAME_CHARS);
        let wire = |engines: &[Engine]| {
            engines
                .iter()
                .filter(|engine| engine.bounded())
                .take(MAX_ENGINES)
                .map(|engine| json!({"engine": engine.engine, "state": engine.state.word()}))
                .collect::<Vec<_>>()
        };
        match self {
            Self::Here { name, engines } => {
                let mut value = json!({
                    "place": "here",
                    "engines": wire(engines),
                });
                if let Some(name) = name.as_deref().filter(|name| name_ok(name)) {
                    value["name"] = json!(name);
                }
                Some(value)
            }
            // `engines` only when the computer named some (#10119), so a
            // phone paired with an older host sends what it always did.
            Self::Paired { name, engines } => name_ok(name).then(|| {
                let mut value = json!({"place": "paired", "name": name});
                let engines = wire(engines);
                if !engines.is_empty() {
                    value["engines"] = json!(engines);
                }
                value
            }),
        }
    }
}

/// What a turn tells the worker about the phone.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Context {
    pub surface: Surface,
    /// The program that sends the request, when it is not the surface's
    /// default ([`Surface::program`]): OpenAgents Terminal on the terminal
    /// surface. Never part of the context the router reads.
    pub client: Option<ClientWord>,
    /// A computer this device may operate is ready, so dispatching Coder
    /// is one tap.
    pub computer_ready: bool,
    /// The app's version and build, as `1.0.0 (19)`.
    pub app_build: Option<String>,
    /// Where Coder runs for this chat (#10077): this device, or the
    /// phone's paired computer. `None` on a phone with no computer ready.
    pub computer: Option<Computer>,
    /// The chat's project folder. Its path is sent only with
    /// [`Computer::Here`].
    pub project: Option<Project>,
    /// The chat's Coder run (#10094): once its turn has ended, what it
    /// did, for the chat to answer about and the router to decide whether
    /// a follow-up is more work for it; while it runs or waits for an
    /// answer, that it does (#10143). `None` in a chat with no run.
    pub coder_run: Option<CoderRun>,
    /// The conversation's open test-set draft (`openagents.eval-draft.v1`),
    /// which the phone keeps and resends each turn: the request's `draft`,
    /// beside `context`, never inside it. Data, never an instruction.
    pub draft: Option<Value>,
    /// The result of a try or a full run of that draft, as the request's
    /// `tried` (`{runs, with, without, total, verdict, report, cases}`).
    pub tried: Option<Value>,
    /// Results a check must not be offered, as the request's `skip`: the
    /// trainer's own and the ones it already checked (public `3189` IDs).
    pub skip: Vec<String>,
    /// A dispatch plan's runs once they all ended (#10183), each as
    /// [`CoderRun::json`] made it: a request that carries them asks the
    /// worker for one combined summary of them.
    pub runs: Vec<Value>,
    /// The user's own memory notes from their account, newest first
    /// (#11182): the web chat sends them so its answers know what Coder
    /// knows. Empty sends none.
    pub memory: Vec<MemoryNote>,
}

impl Context {
    /// The request's `client` word.
    pub fn client_word(&self) -> &'static str {
        Caller {
            surface: self.surface,
            client: self.client,
        }
        .client_word()
    }

    /// The request's `context` object: bounded, and without a key, host
    /// address, or amount. A computer's name and the project folder are
    /// the person's own words and paths, each within its bound or left out.
    pub fn json(&self) -> Value {
        let mut context = json!({
            "surface": self.surface.word(),
            "computer_ready": self.computer_ready,
        });
        if let Some(build) = self.app_build.as_deref().filter(|build| build_like(build)) {
            context["app_build"] = json!(build);
        }
        if let Some(computer) = self.computer.as_ref().and_then(Computer::json) {
            context["computer"] = computer;
        }
        if let Some(project) = &self.project
            && plain(
                &project.name,
                MAX_PROJECT_NAME_BYTES,
                MAX_PROJECT_NAME_BYTES,
            )
        {
            let mut value = json!({"name": project.name});
            // A path is sent only from the computer it names.
            if self.here()
                && let Some(path) = project
                    .path
                    .as_deref()
                    .filter(|path| plain(path, MAX_PROJECT_PATH_BYTES, MAX_PROJECT_PATH_BYTES))
            {
                value["path"] = json!(path);
            }
            context["project"] = value;
        }
        if let Some(run) = &self.coder_run {
            context["coder_run"] = run.json();
        }
        if !self.runs.is_empty() {
            context["runs"] = json!(
                self.runs
                    .iter()
                    .take(nostr::cj_conversation::MAX_PLAN_RUNS)
                    .collect::<Vec<_>>()
            );
        }
        let memory = memory_json(&self.memory);
        if !memory.is_empty() {
            context["memory"] = json!(memory);
        }
        context
    }

    /// This device is itself the computer Coder runs on.
    #[must_use]
    pub fn here(&self) -> bool {
        matches!(self.computer, Some(Computer::Here { .. }))
    }
}

/// `1.0.0 (19)`: digits, dots, a space, and parentheses only.
fn build_like(text: &str) -> bool {
    (1..=24).contains(&text.len())
        && text
            .chars()
            .all(|ch| ch.is_ascii_digit() || " .()".contains(ch))
}

/// A screen an offer may open: the phone's own table, the same words as
/// the worker's `coder::router::Screen`. An offer naming any other screen is
/// set aside.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Screen {
    /// The Wallet tab.
    Wallet,
    /// Account > Computers.
    Computers,
    /// Account > Identity keys. The chat never shows a key itself.
    Keys,
    /// Account > Playtest.
    Playtest,
    /// Report a problem.
    Report,
    /// The Gym in the Verse, at its EVALS board: **See the board**.
    VerseGym,
    /// The person's own latest result (`SCR-05`), which only the phone
    /// holds: **See your result**.
    GymResult,
    /// Add to the Gym (`SCR-20`) for the person's latest result.
    GymPublish,
    /// The test set of the conversation's card or draft (`SCR-21`).
    GymTestSet,
    /// The desktop app's Map page (#10085).
    RoutesMap,
    /// The Verse: the Grid world (**Enter the Grid**).
    Verse,
}

impl Screen {
    /// The worker's `screen` word for this screen.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Wallet => "wallet",
            Self::Computers => "account.computers",
            Self::Keys => "account.keys",
            Self::Playtest => "account.playtest",
            Self::Report => "account.report_problem",
            Self::VerseGym => "verse.gym",
            Self::GymResult => "gym.result",
            Self::GymPublish => "gym.publish",
            Self::GymTestSet => "gym.test_set",
            Self::RoutesMap => "routes.map",
            Self::Verse => "verse",
        }
    }

    /// The screen an offer's exact `screen` value names.
    fn parse(word: &str) -> Option<Self> {
        Some(match word {
            "wallet" => Screen::Wallet,
            "account.computers" => Screen::Computers,
            "account.keys" => Screen::Keys,
            "account.playtest" => Screen::Playtest,
            "account.report_problem" => Screen::Report,
            "verse.gym" => Screen::VerseGym,
            "gym.result" => Screen::GymResult,
            "gym.publish" => Screen::GymPublish,
            "gym.test_set" => Screen::GymTestSet,
            "routes.map" => Screen::RoutesMap,
            "verse" => Screen::Verse,
            _ => return None,
        })
    }
}

/// Where a proposed command runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunsOn {
    /// The phone's own Rust core answers it.
    ThisDevice,
    /// The ready computer runs it.
    ConnectedComputer,
}

/// The `openagents` commands the phone chat may propose: read-only ones
/// only (the owner's decision for the phone), as `(group, subcommands)`.
/// Anything else, whatever the offer's `effect` says, is set aside.
/// `ext` is the older name of `plugin`: the worker sends `ext list`, which
/// every phone and every `openagents` knows, and a phone accepts either
/// (#10089).
pub const READ_ONLY: &[(&str, &[&str])] = &[
    ("computer", &["list", "show", "workspaces"]),
    ("verse", &["who", "quests", "board", "xp"]),
    ("kb", &["search"]),
    ("cap", &["list"]),
    ("prg", &["list"]),
    ("plugin", &["list"]),
    ("ext", &["list"]),
    ("session", &["list"]),
];

/// A `cli` offer's command without the program's name, when its words are
/// bounded and printable: what a computer looks up in its own command tree
/// before it runs anything (#10170).
pub fn command_of(payload: &Value) -> Option<Vec<String>> {
    let mut argv: Vec<String> = payload["argv"]
        .as_array()?
        .iter()
        .map(|word| word.as_str().map(str::to_owned))
        .collect::<Option<_>>()?;
    if argv.first().is_some_and(|word| word == "openagents") {
        argv.remove(0);
    }
    // A GitHub change the website shows on a confirm card (#11167) carries
    // its title and text as words, so it keeps NIP-CJ's own bounds.
    let (max_words, max_bytes) = if github_actions::argv::is_github(&argv) {
        (MAX_GITHUB_ARGV, MAX_GITHUB_ARG_BYTES)
    } else {
        (MAX_ARGV, MAX_ARG_BYTES)
    };
    ((1..=max_words).contains(&argv.len())
        && argv.iter().all(|word| {
            !word.is_empty() && word.len() <= max_bytes && !word.chars().any(char::is_control)
        }))
    .then_some(argv)
}

/// Whether `argv` (without `openagents`) is a read-only command in
/// [`READ_ONLY`], with bounded, printable words.
pub fn read_only(argv: &[String]) -> bool {
    (2..=MAX_ARGV).contains(&argv.len())
        && READ_ONLY
            .iter()
            .any(|(group, leaves)| argv[0] == *group && leaves.contains(&argv[1].as_str()))
        && argv.iter().all(|word| {
            !word.is_empty() && word.len() <= MAX_ARG_BYTES && !word.chars().any(char::is_control)
        })
}

/// What the worker offered beside a reply, as the phone read it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "offer", rename_all = "snake_case")]
pub enum Offer {
    /// Run Coder on the ready computer with the conversation, or, with
    /// none, connect one.
    RunCoder,
    /// Open one of the phone's screens.
    OpenScreen { screen: Screen },
    /// Run a read-only `openagents` command, after a tap.
    Cli { argv: Vec<String>, runs_on: RunsOn },
    /// Run a test set against a tool, after a tap on the card's button:
    /// the offer body as NIP-CJ's own parser accepted it.
    StartEval { body: Value },
    /// Add a result the phone holds to the Gym, after `SCR-20`'s button.
    PublishEval { body: Value },
    /// Open a deck in the desktop app's slide viewer (`open_presentation`,
    /// #10058). The worker's `presentation.open` route and its `deck`
    /// reading chose `deck`, a bounded id; the desktop opens it only when
    /// `openagents_deck::decks()` lists it. A surface with no slide viewer
    /// says [`PRESENTATION_ELSEWHERE`] instead.
    OpenPresentation { deck: String },
}

/// What a surface without the slide viewer says to an
/// [`Offer::OpenPresentation`]: the worker's own `presentation.elsewhere`
/// line.
pub const PRESENTATION_ELSEWHERE: &str =
    "Decks open in the OpenAgents desktop app, so we can't show one here.";

impl Offer {
    /// The worker payload [`Offer::parse`] accepts. A page carries this
    /// shape, whose screen words are [`Screen::word`].
    #[must_use]
    pub fn wire(&self) -> Value {
        match self {
            Self::RunCoder => json!({"offer": "run_coder"}),
            Self::OpenScreen { screen } => json!({"offer": "open_screen", "screen": screen.word()}),
            Self::Cli { argv, runs_on } => json!({
                "offer": "cli",
                "effect": "read_only",
                "argv": argv,
                "runs_on": match runs_on {
                    RunsOn::ThisDevice => "this_device",
                    RunsOn::ConnectedComputer => "connected_computer",
                },
            }),
            Self::StartEval { body } | Self::PublishEval { body } => body.clone(),
            // NIP-CJ's own body, as the worker sends it, so it parses back.
            Self::OpenPresentation { deck } => nostr::cj_conversation::offer_feedback(
                &nostr::cj_conversation::Offer::OpenPresentation {
                    deck: deck.clone(),
                    label: "Open the deck".into(),
                },
                2,
            )
            .unwrap_or(Value::Null),
        }
    }

    /// Reads one `offer` feedback payload. Exact enum values only; an
    /// unknown offer, screen, or command, or a command that is not
    /// read-only, is `None`.
    pub fn parse(payload: &Value) -> Option<Self> {
        match payload["offer"].as_str()? {
            "run_coder" => Some(Offer::RunCoder),
            "open_screen" => Screen::parse(payload["screen"].as_str()?)
                .map(|screen| Offer::OpenScreen { screen }),
            "cli" => {
                if payload["effect"].as_str() != Some("read_only") {
                    return None;
                }
                let mut argv: Vec<String> = payload["argv"]
                    .as_array()?
                    .iter()
                    .map(|word| word.as_str().map(str::to_owned))
                    .collect::<Option<_>>()?;
                // The command's own name is implied.
                if argv.first().is_some_and(|word| word == "openagents") {
                    argv.remove(0);
                }
                let runs_on = match payload["runs_on"].as_str() {
                    Some("this_device") => RunsOn::ThisDevice,
                    Some("connected_computer") => RunsOn::ConnectedComputer,
                    _ => return None,
                };
                read_only(&argv).then_some(Offer::Cli { argv, runs_on })
            }
            // The eval offers are read by NIP-CJ's own parser, whole: an
            // offer it refuses is set aside.
            "start_eval" => match nostr::cj_conversation::parse_offer(payload).ok()? {
                (_, nostr::cj_conversation::Offer::StartEval { .. }) => Some(Offer::StartEval {
                    body: bare(payload),
                }),
                _ => None,
            },
            "publish_eval" => match nostr::cj_conversation::parse_offer(payload).ok()? {
                (_, nostr::cj_conversation::Offer::PublishEval { .. }) => {
                    Some(Offer::PublishEval {
                        body: bare(payload),
                    })
                }
                _ => None,
            },
            // Read by NIP-CJ's own parser: a deck id it refuses is set
            // aside.
            "open_presentation" => match nostr::cj_conversation::parse_offer(payload).ok()? {
                (_, nostr::cj_conversation::Offer::OpenPresentation { deck, .. }) => {
                    Some(Offer::OpenPresentation { deck })
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// A `start_eval` offer as NIP-CJ reads it.
    pub fn start_eval(&self) -> Option<StartEval> {
        let Offer::StartEval { body } = self else {
            return None;
        };
        match nostr::cj_conversation::parse_offer(body).ok()? {
            (
                _,
                nostr::cj_conversation::Offer::StartEval {
                    suite,
                    subject,
                    size,
                    at,
                    ..
                },
            ) => Some(StartEval {
                suite,
                subject,
                size,
                at,
            }),
            _ => None,
        }
    }

    /// The command as the person reads it.
    pub fn command_line(argv: &[String]) -> String {
        let words: Vec<String> = argv
            .iter()
            .map(|word| {
                if word
                    .chars()
                    .any(|ch| ch.is_whitespace() || "'\"$`\\".contains(ch))
                {
                    format!("'{}'", word.replace('\'', "'\\''"))
                } else {
                    word.clone()
                }
            })
            .collect();
        format!("openagents {}", words.join(" "))
    }
}

/// The dispatch plan a `run_coder` offer payload carries (#10183), as
/// NIP-CJ's parser reads it: several runs, or read-only. `None` for one
/// run that may change files, or an offer the parser refuses.
#[must_use]
pub fn plan_of(payload: &Value) -> Option<nostr::cj_conversation::Plan> {
    match nostr::cj_conversation::parse_offer(payload).ok()? {
        (_, nostr::cj_conversation::Offer::RunCoder { plan, .. })
            if !plan.is_single() || plan.read_only =>
        {
            Some(plan)
        }
        _ => None,
    }
}

/// The engine a `run_coder` offer payload names (#10076): one exact word
/// of NIP-CJ's closed set, else none. A word this build doesn't know is no
/// preference, and the offer still shows.
#[must_use]
pub fn engine_of(payload: &Value) -> Option<nostr::cj_conversation::Engine> {
    payload["engine"]
        .as_str()
        .and_then(nostr::cj_conversation::Engine::parse)
}

/// An offer body without the worker's label, which the phone never shows:
/// it names every control itself. The body still parses, with an empty
/// label replaced by the phone's own word.
fn bare(payload: &Value) -> Value {
    let mut body = payload.clone();
    if let Some(object) = body.as_object_mut() {
        object.insert("label".into(), json!("offer"));
    }
    body
}

/// A `start_eval` offer: which test set, which tool, how big, and where
/// the worker suggests it runs. The phone decides where it runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartEval {
    pub suite: nostr::cj_conversation::SuiteSource,
    pub subject: nostr::cj_conversation::SubjectSource,
    pub size: nostr::cj_conversation::Size,
    pub at: nostr::cj_conversation::Where,
}

/// A suggested next question under a prepared answer: tapping it sends
/// its words as the person's message.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Followup {
    /// The prepared answer it leads to, as the bank names it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer: Option<String>,
    pub label: String,
}

/// Reads a `followups` array: objects with a one-line `label` of at most
/// 80 characters and an optional bank `id` or `answer`. Anything else is
/// set aside.
fn followups(value: &Value) -> Vec<Followup> {
    let mut kept: Vec<Followup> = vec![];
    for entry in value.as_array().into_iter().flatten() {
        let Some(label) = entry["label"].as_str().map(str::trim) else {
            continue;
        };
        if label.is_empty()
            || label.chars().count() > MAX_FOLLOWUP_CHARS
            || label.chars().any(char::is_control)
            || kept.iter().any(|kept| kept.label == label)
        {
            continue;
        }
        let answer = entry["answer"]
            .as_str()
            .or_else(|| entry["id"].as_str())
            .filter(|id| tag_like(id))
            .map(str::to_owned);
        kept.push(Followup {
            answer,
            label: label.to_owned(),
        });
        if kept.len() == MAX_FOLLOWUPS {
            break;
        }
    }
    kept
}

/// The most plugin cards one reply keeps.
pub const MAX_PLUGIN_CARDS: usize = 12;

/// Reads a result's `plugins` array: package slugs (`project-map`), each a
/// short lowercase id, kept once each and at most [`MAX_PLUGIN_CARDS`].
/// Anything else is set aside.
fn plugin_slugs(value: &Value) -> Vec<String> {
    let mut kept: Vec<String> = vec![];
    for slug in value.as_array().into_iter().flatten() {
        let Some(slug) = slug
            .as_str()
            .filter(|slug| slug.len() <= 64 && tag_like(slug))
        else {
            continue;
        };
        if !kept.iter().any(|kept| kept == slug) {
            kept.push(slug.to_owned());
        }
        if kept.len() == MAX_PLUGIN_CARDS {
            break;
        }
    }
    kept
}

/// A bank id, `id@version`, route, or tier word: short, lowercase ASCII.
fn tag_like(text: &str) -> bool {
    (1..=96).contains(&text.len())
        && text
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._@-:".contains(&b))
}

/// What the router said about one reply: kept with it, shown subtly, and
/// sent only when the person shares the chat or marks the answer wrong.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Meta {
    /// What the worker decided to show first (`canned`, `opener`,
    /// `model`, or a later tier).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,
    /// The prepared answer that is the text, as `id@version`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bank: Option<String>,
    /// The judgment feedback as it arrived, bounded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub judgment: Option<String>,
    /// Full local decision evidence, separate from the bounded shareable judgment.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decisions: Vec<route_contract::decision::DecisionReading>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub offers: Vec<Offer>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub followups: Vec<Followup>,
    /// The Gym's cards (NIP-CJ `card` feedback), each body as NIP-CJ's own
    /// parser accepted it; a newer card of one kind replaces the older.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cards: Vec<Value>,
    /// Who would run Coder on the computer this reply offers it on
    /// ([`crate::coder_events::Runner`]), as that computer predicts it now.
    /// The computer sets it when it shows the reply
    /// ([`crate::delegation::attach_runner`]); nothing the worker sends
    /// sets it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runner: Option<crate::coder_events::Runner>,
    /// The coding engine the person asked for, from the worker's
    /// `run_coder` offer (#10076): the router's typed `engine` reading,
    /// never text. A request the start puts first, not permission.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<nostr::cj_conversation::Engine>,
    /// The `openagents` command the worker's `cli` offer proposed
    /// (#10170), whatever its effect, without the program's name: bounded,
    /// printable words. The worker's own effect word is not kept; a
    /// computer runs it only after reading the command's effect from its
    /// own command tree ([`crate::client::Coder::effect`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<Vec<String>>,
    /// The dispatch plan the worker's `run_coder` offer carried (#10183):
    /// several runs, one per engine, read-only or not, as NIP-CJ's own
    /// parser read it. `None` is one run, as before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<nostr::cj_conversation::Plan>,
    /// The step of making a plugin this reply served (#10177), from the
    /// result's typed `plugin` field ([`crate::plugin_flow::Flow::parse`]):
    /// what this computer does next, never read from the reply's text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin: Option<crate::plugin_flow::Flow>,
    /// The catalog plugins this reply shows as cards, by package slug
    /// (`docs/web/plugin-card.md`), from the result's typed `plugins`
    /// field: bounded ids a surface looks up in its own copy of the
    /// catalog, never words to show.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plugins: Vec<String>,
    /// The person's own keys this turn went with (BYOK `mine`, #10176),
    /// by provider and fingerprint, never the key: the worker ran the
    /// turn's model and Jev on them. Empty when it ran on ours. This
    /// computer sets it when it seals the keys; nothing the worker sends
    /// sets it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub payer_keys: Vec<model_access::KeyPrint>,
    /// The first model provider missed this turn and another model
    /// answered it (#11132), from the result's typed `switched` field as
    /// NIP-CJ's own parser read it: shown as one short line beside the
    /// answer ([`nostr::cj_conversation::switched::Switched::line`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub switched: Option<nostr::cj_conversation::switched::Switched>,
}

impl Meta {
    /// The reply is a prepared answer from the bank, not model text.
    pub fn canned(&self) -> bool {
        self.tier.as_deref() == Some("canned") && self.answer.is_some()
    }

    /// The router served the terminal's standing-rule line (#10157): the
    /// `standing.rule` route with its `standing.rule` answer, which the
    /// computer follows with the compiled rule. Exact enum values, never
    /// the reply's text.
    pub fn standing(&self) -> bool {
        self.route.as_deref() == Some("standing.rule")
            && self
                .answer
                .as_deref()
                .and_then(|answer| answer.split('@').next())
                == Some("standing.rule")
    }

    /// Nothing to keep.
    pub fn is_empty(&self) -> bool {
        *self == Meta::default()
    }

    /// Takes a `judgment` feedback payload.
    pub fn judged(&mut self, payload: &Value) {
        self.decisions = payload
            .get("decisions")
            .cloned()
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        let mut shareable = payload.clone();
        if let Some(object) = shareable.as_object_mut() {
            object.remove("decisions");
        }
        let text = shareable.to_string();
        if text.len() <= MAX_JUDGMENT_BYTES {
            self.judgment = Some(text);
        }
        self.take_words(payload);
        self.take_followups(payload);
    }

    /// Takes an `offer` feedback payload.
    pub fn offered(&mut self, payload: &Value) {
        if payload["offer"].as_str() == Some("cli")
            && self.command.is_none()
            && let Some(argv) = command_of(payload)
        {
            self.command = Some(argv);
        }
        if let Some(offer) = Offer::parse(payload)
            && self.offers.len() < MAX_OFFERS
            && !self.offers.contains(&offer)
        {
            if offer == Offer::RunCoder {
                self.engine = engine_of(payload);
                self.plan = plan_of(payload);
            }
            self.offers.push(offer);
        }
    }

    /// Takes a `card` feedback payload: kept only when NIP-CJ's parser
    /// reads it, so the phone never draws a card it can't show exactly.
    pub fn carded(&mut self, payload: &Value) {
        let Ok((_, card)) = nostr::cj_conversation::parse_card(payload) else {
            return;
        };
        let word = card.word();
        if let Some(at) = self
            .cards
            .iter()
            .position(|kept| kept["card"].as_str() == Some(word))
        {
            self.cards[at] = payload.clone();
        } else if self.cards.len() < MAX_CARDS {
            self.cards.push(payload.clone());
        }
    }

    /// The cards, read again.
    pub fn parsed_cards(&self) -> Vec<nostr::cj_conversation::Card> {
        self.cards
            .iter()
            .filter_map(|card| nostr::cj_conversation::parse_card(card).ok())
            .map(|(_, card)| card)
            .collect()
    }

    /// Takes a result's router fields; they outrank the judgment's.
    pub fn resulted(&mut self, payload: &Value) {
        // A result whose model is the bank is a prepared answer, even from
        // a worker that names no tier.
        if payload["model"]
            .as_str()
            .is_some_and(|model| model.starts_with("bank:"))
        {
            self.tier = Some("canned".into());
        }
        self.take_words(payload);
        self.take_followups(payload);
        self.switched = nostr::cj_conversation::switched::Switched::parse(
            &payload["switched"],
            payload["model"].as_str(),
        );
        if let Some(flow) = crate::plugin_flow::Flow::parse(&payload["plugin"]) {
            self.plugin = Some(flow);
        }
        if payload["plugins"].is_array() {
            self.plugins = plugin_slugs(&payload["plugins"]);
        }
    }

    fn take_words(&mut self, payload: &Value) {
        let word = |field: &str| {
            payload[field]
                .as_str()
                .filter(|w| tag_like(w))
                .map(str::to_owned)
        };
        for (slot, field) in [
            (&mut self.tier, "tier"),
            (&mut self.answer, "answer"),
            (&mut self.route, "route"),
            (&mut self.bank, "bank"),
        ] {
            if let Some(value) = word(field) {
                *slot = Some(value);
            }
        }
        // A judgment's `answer` is the argmax even when the reply is not
        // that answer; only a canned tier keeps it as the text's source.
        if self.tier.as_deref() != Some("canned") {
            self.answer = None;
        }
    }

    fn take_followups(&mut self, payload: &Value) {
        let read = followups(&payload["followups"]);
        if !read.is_empty() {
            self.followups = read;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_context_names_no_host_and_bounds_the_build() {
        let context = Context {
            computer_ready: true,
            app_build: Some("1.0.0 (19)".into()),
            ..Context::default()
        };
        assert_eq!(
            context.json(),
            json!({"surface": "phone", "computer_ready": true, "app_build": "1.0.0 (19)"})
        );
        let odd = Context {
            computer_ready: false,
            app_build: Some("Studio Mac".into()),
            ..Context::default()
        };
        assert_eq!(
            odd.json(),
            json!({"surface": "phone", "computer_ready": false})
        );
    }

    /// `open_presentation` is a typed offer read by NIP-CJ's parser: the
    /// worker's feedback body parses to its deck id, and a deck that is
    /// not a bounded id, or a body NIP-CJ refuses, is set aside (#10058).
    #[test]
    fn open_presentation_is_a_typed_offer_with_a_bounded_deck() {
        let body = |deck: &str| {
            json!({"v": 2, "requires": [], "type": "offer", "offer": "open_presentation",
                   "deck": deck, "label": "Open the deck"})
        };
        let offer = Offer::parse(&body("three-devdays-later"));
        assert_eq!(
            offer,
            Some(Offer::OpenPresentation {
                deck: "three-devdays-later".into()
            })
        );
        let long = "a".repeat(65);
        for deck in ["", "Three Days", "../etc", long.as_str()] {
            assert_eq!(Offer::parse(&body(deck)), None, "{deck}");
        }
        // The bare form, with no NIP-CJ envelope, is not an offer.
        assert_eq!(
            Offer::parse(&json!({"offer": "open_presentation", "deck": "three-devdays-later"})),
            None
        );
        let mut meta = Meta::default();
        meta.offered(&body("test-time-capabilities"));
        assert_eq!(
            meta.offers,
            [Offer::OpenPresentation {
                deck: "test-time-capabilities".into()
            }]
        );
    }

    /// A `run_coder` offer's `engine` is kept beside the offer as a typed
    /// value; an unknown word is no preference (#10076).
    /// A `run_coder` offer's plan (#10183) is kept on the reply's meta as
    /// NIP-CJ read it; a plain offer keeps none.
    #[test]
    fn a_run_coder_offer_keeps_its_plan() {
        use nostr::cj_conversation::{Engine, Plan};
        let mut meta = Meta::default();
        meta.offered(&json!({
            "v": 2, "requires": [], "type": "offer", "offer": "run_coder",
            "target": "connected_computer", "label": "Run Coder",
            "runs": ["codex", "claude_code", "grok_build"], "read_only": true, "summarize": true,
        }));
        assert_eq!(meta.offers, [Offer::RunCoder]);
        assert_eq!(
            meta.plan,
            Some(Plan {
                runs: vec![Engine::Codex, Engine::ClaudeCode, Engine::GrokBuild],
                read_only: true,
                summarize: true,
            })
        );
        let mut plain = Meta::default();
        plain.offered(&json!({
            "v": 2, "requires": [], "type": "offer", "offer": "run_coder",
            "target": "connected_computer", "label": "Run Coder",
        }));
        assert_eq!(plain.offers, [Offer::RunCoder]);
        assert_eq!(plain.plan, None);
    }

    #[test]
    fn a_run_coder_offer_keeps_the_engine_asked_for() {
        use nostr::cj_conversation::Engine;
        let mut meta = Meta::default();
        meta.offered(
            &json!({"v": 2, "requires": [], "type": "offer", "offer": "run_coder",
            "target": "connected_computer", "label": "Run Coder", "engine": "claude_code"}),
        );
        assert_eq!(meta.offers, [Offer::RunCoder]);
        assert_eq!(meta.engine, Some(Engine::ClaudeCode));
        let kept = serde_json::to_value(&meta).unwrap();
        assert_eq!(kept["engine"], "claude_code");
        assert_eq!(serde_json::from_value::<Meta>(kept).unwrap(), meta);
        let mut plain = Meta::default();
        plain.offered(&json!({"offer": "run_coder", "engine": "gemini_cli"}));
        assert_eq!((plain.offers.len(), plain.engine), (1, None));
        // Another offer never sets it.
        let mut other = Meta::default();
        other.offered(&json!({"offer": "open_screen", "screen": "wallet", "engine": "codex"}));
        assert_eq!(other.engine, None);
    }

    /// Offers are read against the phone's own tables: a screen it does
    /// not know, a command that is not read-only, or an effect other than
    /// `read_only` is set aside, and the worker's label is ignored.
    #[test]
    fn offers_pass_only_the_phones_own_tables() {
        let read = |value: Value| Offer::parse(&value);
        assert_eq!(
            read(json!({"offer": "run_coder", "target": "connected_computer", "label": "x"})),
            Some(Offer::RunCoder)
        );
        assert_eq!(
            read(
                json!({"offer": "open_screen", "screen": "account.computers",
                "label": "Delete everything"})
            ),
            Some(Offer::OpenScreen {
                screen: Screen::Computers
            })
        );
        assert_eq!(
            read(json!({"offer": "open_screen", "screen": "wallet"})),
            Some(Offer::OpenScreen {
                screen: Screen::Wallet
            })
        );
        // `crates/coder/src/router.rs` names exactly these screens.
        for word in [
            "account.computers",
            "account.keys",
            "account.playtest",
            "account.report_problem",
            "wallet",
            "verse.gym",
        ] {
            assert!(Screen::parse(word).is_some(), "{word}");
        }
        // NIP-CJ's `verse.gym`: See the board.
        assert_eq!(
            read(json!({"offer": "open_screen", "screen": "verse.gym", "label": "See the board"})),
            Some(Offer::OpenScreen {
                screen: Screen::VerseGym
            })
        );
        assert_eq!(
            read(json!({"offer": "open_screen", "screen": "settings.danger"})),
            None
        );
        assert_eq!(
            read(json!({"offer": "cli", "argv": ["computer", "list"],
                "effect": "read_only", "runs_on": "this_device", "confirm": true})),
            Some(Offer::Cli {
                argv: vec!["computer".into(), "list".into()],
                runs_on: RunsOn::ThisDevice
            })
        );
        // The worker calls it read-only; the phone's table does not.
        for argv in [
            json!(["wallet", "pay", "lnbc1"]),
            json!(["computer", "approve", "host"]),
            json!(["wallet", "export"]),
            json!(["computer"]),
            json!(["computer", "list\u{7}"]),
        ] {
            assert_eq!(
                read(json!({"offer": "cli", "argv": argv, "effect": "read_only",
                    "runs_on": "this_device"})),
                None,
                "{argv}"
            );
        }
        assert_eq!(
            read(
                json!({"offer": "cli", "argv": ["computer", "list"], "effect": "publishes",
                "runs_on": "this_device"})
            ),
            None
        );
        assert_eq!(read(json!({"offer": "pay", "amount": 5000})), None);
        assert_eq!(
            Offer::command_line(&["kb".into(), "search".into(), "docker cp".into()]),
            "openagents kb search 'docker cp'"
        );
    }

    #[test]
    fn a_result_another_provider_answered_keeps_the_switch() {
        let mut meta = Meta::default();
        meta.resulted(&json!({"v": 2, "type": "result", "text": "Paris.",
            "model": "z-ai/glm-5.3-flash",
            "switched": {"provider": "openrouter", "model": "google/gemini-3.8-flash",
                "why": "timeout"}}));
        let switched = meta.switched.clone().expect("the switch is kept");
        assert_eq!(
            switched.line(),
            "OpenRouter didn't answer in time, so z-ai/glm-5.3-flash answered instead."
        );
        // It survives storage, and a result without one keeps none.
        let kept: Meta = serde_json::from_value(serde_json::to_value(&meta).unwrap()).unwrap();
        assert_eq!(kept.switched, Some(switched));
        let mut plain = Meta::default();
        plain.resulted(&json!({"type": "result", "text": "Paris.", "model": "m"}));
        assert_eq!(plain.switched, None);
        // A word this version doesn't know drops the field.
        plain.resulted(&json!({"type": "result", "text": "Paris.", "model": "m",
            "switched": {"provider": "azure", "model": "x", "why": "timeout"}}));
        assert_eq!(plain.switched, None);
    }

    #[test]
    fn a_canned_result_keeps_its_answer_and_followups() {
        let mut meta = Meta::default();
        meta.judged(&json!({"v": 2, "type": "judgment", "verdict": "respond",
            "tier": "model", "answer": "meta.model@1", "answer_p": 0.4}));
        // The argmax answer is not the text of a model reply.
        assert_eq!(meta.answer, None);
        assert!(!meta.canned());
        meta.resulted(
            &json!({"v": 2, "type": "result", "text": "Our chat runs on …",
            "model": "bank:chat-answers-v1", "tier": "canned", "answer": "meta.model@1",
            "route": "meta", "bank": "chat-answers-v1@9f2c",
            "followups": [{"id": "meta.privacy", "label": "Is this chat private?"},
                {"label": ""}, {"label": "x".repeat(81)}, "meta.pricing",
                {"id": "meta.pricing", "label": "What does it cost?"},
                {"label": "Is this chat private?"}]}),
        );
        assert!(meta.canned());
        assert_eq!(meta.answer.as_deref(), Some("meta.model@1"));
        assert_eq!(meta.route.as_deref(), Some("meta"));
        let labels: Vec<&str> = meta.followups.iter().map(|f| f.label.as_str()).collect();
        assert_eq!(labels, ["Is this chat private?", "What does it cost?"]);
        assert!(meta.judgment.as_deref().unwrap().contains("\"judgment\""));
        // Today's worker names a bank answer by its model alone.
        let mut older = Meta::default();
        older.resulted(
            &json!({"type": "result", "text": "Hi!", "model": "bank:chat-answers-v1",
            "answer": "smalltalk.hello@1"}),
        );
        assert!(older.canned());
    }

    /// A result's `plugins` are package slugs, kept once each and bounded;
    /// anything that is not a short lowercase id is set aside, and the
    /// chips' part of the record keeps them (docs/web/plugin-card.md).
    #[test]
    fn a_result_keeps_its_plugin_slugs() {
        let mut meta = Meta::default();
        meta.resulted(&json!({"type": "result", "text": "These are the plugins…",
            "model": "bank:chat-answers-v1", "tier": "canned", "answer": "plugins.web@1",
            "plugins": ["project-map", "code-finder", "project-map", "Not A Slug", 7,
                "x".repeat(65)]}));
        assert_eq!(meta.plugins, ["project-map", "code-finder"]);
        assert_eq!(
            crate::suggestions::chip_meta(&meta).plugins,
            ["project-map", "code-finder"]
        );
        let many: Vec<String> = (0..20).map(|n| format!("plugin-{n}")).collect();
        let mut bounded = Meta::default();
        bounded.resulted(&json!({"type": "result", "plugins": many}));
        assert_eq!(bounded.plugins.len(), MAX_PLUGIN_CARDS);
        let mut none = Meta::default();
        none.resulted(&json!({"type": "result", "text": "Hi!"}));
        assert!(none.plugins.is_empty());
    }

    /// The phone's read-only list is the owner's list the worker's CLI
    /// route offers from (`coder::cli_route::gate::PHONE_COMMANDS`), which
    /// this crate cannot depend on: read it from its source. It also takes
    /// `ext list`, the wire name the worker sends for `plugin list`
    /// (`coder::cli_route::tree::WIRE_NAMES`).
    #[test]
    fn the_read_only_list_matches_the_worker_phone_list() {
        let gate = include_str!("../../coder/src/cli_route/gate.rs");
        let start = gate
            .find("pub const PHONE_COMMANDS")
            .expect("the worker's phone list");
        let body = &gate[start..start + gate[start..].find("];").expect("its end")];
        let mut worker: Vec<String> = body
            .lines()
            .filter_map(|line| {
                line.trim()
                    .strip_prefix('"')?
                    .strip_suffix("\",")
                    .map(str::to_owned)
            })
            .collect();
        worker.push("ext list".to_string());
        worker.sort();
        let mut phone: Vec<String> = READ_ONLY
            .iter()
            .flat_map(|(group, leaves)| leaves.iter().map(move |leaf| format!("{group} {leaf}")))
            .collect();
        phone.sort();
        assert!(!worker.is_empty());
        assert_eq!(phone, worker);
    }

    /// A plugin listing card is accepted under either name (#10089).
    #[test]
    fn a_plugin_listing_is_read_only_under_either_name() {
        let words = |text: &str| text.split(' ').map(str::to_owned).collect::<Vec<_>>();
        assert!(read_only(&words("ext list")));
        assert!(read_only(&words("ext list --limit 5")));
        assert!(read_only(&words("plugin list")));
        assert!(!read_only(&words("ext eval run DIR")));
        assert!(!read_only(&words("plugin test run DIR")));
        for argv in [words("ext list"), words("plugin list")] {
            let offer = Offer::Cli {
                argv,
                runs_on: RunsOn::ConnectedComputer,
            };
            assert_eq!(Offer::parse(&offer.wire()), Some(offer));
        }
    }

    #[test]
    fn an_offers_wire_form_parses_back() {
        assert_eq!(Offer::parse(&Offer::RunCoder.wire()), Some(Offer::RunCoder));
        for screen in [
            Screen::Wallet,
            Screen::Computers,
            Screen::Keys,
            Screen::Playtest,
            Screen::Report,
            Screen::VerseGym,
            Screen::GymResult,
            Screen::GymPublish,
            Screen::GymTestSet,
        ] {
            let offer = Offer::OpenScreen { screen };
            assert_eq!(Offer::parse(&offer.wire()), Some(offer));
        }
        let command = Offer::Cli {
            argv: vec!["computer".into(), "list".into()],
            runs_on: RunsOn::ConnectedComputer,
        };
        assert_eq!(Offer::parse(&command.wire()), Some(command));
        let deck = Offer::OpenPresentation {
            deck: "three-devdays-later".into(),
        };
        assert_eq!(Offer::parse(&deck.wire()), Some(deck));
    }
}

#[cfg(test)]
mod computer_context_tests {
    use super::*;
    use crate::coder_events::{Passed, PassedOver, Runner};

    /// The desktop's context is exactly the worker's fixture
    /// (`crates/coder/fixtures/nip-cj/router-request-computer.json`), which
    /// `coder::router::wire` reads back: the surface, this computer with
    /// its agents' readiness from the run's own prediction, and the
    /// project folder with its path (#10077).
    #[test]
    fn a_computer_context_matches_the_worker_fixture() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../coder/fixtures/nip-cj/router-request-computer.json"
        ))
        .unwrap();
        let runner = Runner::Runs {
            provider: "claude".into(),
            model: "opus".into(),
            passed: vec![Passed {
                provider: "codex".into(),
                why: PassedOver::NearLimit { used_percent: 97 },
            }],
            requested: None,
        };
        let context = Context {
            surface: Surface::Desktop,
            computer_ready: true,
            computer: Some(Computer::Here {
                name: Some("Studio Mac".into()),
                engines: Engine::from_runner(&runner),
            }),
            project: Project::at("/Users/someone/work/openagents"),
            ..Context::default()
        };
        let request = crate::basic_coder::payload(
            &[crate::basic_coder::Turn::user("whats your working dir")],
            &context,
        );
        assert_eq!(request["context"], fixture["context"]);
        assert_eq!(request["client"], fixture["client"]);
        assert_eq!(
            request["instructions"],
            crate::basic_coder::INSTRUCTIONS_ON_COMPUTER
        );
        assert!(
            !crate::basic_coder::INSTRUCTIONS_ON_COMPUTER.contains("a computer the user connects")
        );
    }

    /// Every coding agent on this computer goes on the wire with its own
    /// state, a not-enabled one too, exactly as the worker's fixture reads
    /// it (#10113); more than [`MAX_ENGINES`] are cut.
    #[test]
    fn every_engine_here_goes_on_the_wire() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../coder/fixtures/nip-cj/router-request-engines.json"
        ))
        .unwrap();
        let engine = |engine: &str, state| Engine {
            engine: engine.into(),
            state,
        };
        let context = Context {
            surface: Surface::Terminal,
            client: Some(ClientWord::Terminal),
            computer_ready: true,
            computer: Some(Computer::Here {
                name: None,
                engines: vec![
                    engine("codex", EngineState::Ready),
                    engine("claude", EngineState::Limited),
                    engine("grok", EngineState::Ready),
                    engine("devin", EngineState::NotEnabled),
                    engine("opencode", EngineState::NotEnabled),
                ],
            }),
            ..Context::default()
        };
        let request = crate::basic_coder::payload(
            &[crate::basic_coder::Turn::user(
                "which coding agents can you use",
            )],
            &context,
        );
        assert_eq!(request["context"], fixture["context"]);
        assert_eq!(request["client"], fixture["client"]);
        let many = Context {
            computer: Some(Computer::Here {
                name: None,
                engines: (0..MAX_ENGINES + 3)
                    .map(|n| engine(&format!("agent{n}"), EngineState::Ready))
                    .collect(),
            }),
            ..Context::default()
        };
        assert_eq!(
            many.json()["computer"]["engines"].as_array().unwrap().len(),
            MAX_ENGINES
        );
    }

    /// A phone sends the coding agents its paired computer's presence
    /// names (#10119), exactly as the worker's fixture reads them, within
    /// [`MAX_ENGINES`]; with none, the context is what it always was.
    #[test]
    fn a_phone_names_its_computers_engines() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../coder/fixtures/nip-cj/router-request-paired-engines.json"
        ))
        .unwrap();
        let engine = |engine: &str, state| Engine {
            engine: engine.into(),
            state,
        };
        let context = Context {
            computer_ready: true,
            computer: Some(Computer::Paired {
                name: "macbook-pro-m5".into(),
                engines: vec![
                    engine("codex", EngineState::Ready),
                    engine("claude", EngineState::Ready),
                    engine("grok", EngineState::Ready),
                    engine("opencode", EngineState::NotEnabled),
                    engine("devin", EngineState::NotEnabled),
                ],
            }),
            ..Context::default()
        };
        let request = crate::basic_coder::payload(
            &[crate::basic_coder::Turn::user(
                "what coding agents are connected?",
            )],
            &context,
        );
        assert_eq!(request["context"], fixture["context"]);
        assert_eq!(request["client"], fixture["client"]);
        let many = Context {
            computer: Some(Computer::Paired {
                name: "macbook-pro-m5".into(),
                engines: (0..MAX_ENGINES + 3)
                    .map(|n| engine(&format!("agent{n}"), EngineState::Ready))
                    .collect(),
            }),
            ..Context::default()
        };
        assert_eq!(
            many.json()["computer"]["engines"].as_array().unwrap().len(),
            MAX_ENGINES
        );
        for word in ["ready", "not_signed_in", "limited", "not_enabled"] {
            assert_eq!(EngineState::of_word(word).unwrap().word(), word);
        }
        assert_eq!(EngineState::of_word("busy"), None);
    }

    /// A follow-up in a chat whose Coder run finished carries the run's
    /// result as typed context, exactly as the worker's fixture reads it
    /// (#10094).
    #[test]
    fn a_coder_run_context_matches_the_worker_fixture() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../coder/fixtures/nip-cj/router-request-coder-run.json"
        ))
        .unwrap();
        let context = Context {
            surface: Surface::Desktop,
            computer_ready: true,
            computer: Some(Computer::Here {
                name: None,
                engines: vec![Engine {
                    engine: "codex".into(),
                    state: EngineState::Ready,
                }],
            }),
            coder_run: Some(CoderRun {
                ending: RunEnding::Finished,
                turn: 1,
                engine: Some("codex".into()),
                model: Some("gpt-6-luna".into()),
                summary: "I looked at the project and changed nothing. It holds a Rust \
                          workspace with 40 crates.\u{7}"
                    .into(),
                files: vec![
                    RunFile {
                        path: "NOTE.md".into(),
                        status: "added".into(),
                    },
                    RunFile {
                        path: "bad\npath".into(),
                        status: "added".into(),
                    },
                ],
                commands: vec!["ls".into(), "cargo metadata --no-deps\n--more".into()],
            }),
            ..Context::default()
        };
        let request = crate::basic_coder::payload(
            &[
                crate::basic_coder::Turn::user("do a test delegation to claude"),
                crate::basic_coder::Turn::assistant("Starting Claude Code on this.", None),
                crate::basic_coder::Turn::user("summarize what happened"),
            ],
            &context,
        );
        assert_eq!(request["context"], fixture["context"]);
        // Every bound holds: a long summary is cut, and only the last
        // commands are named.
        let long = CoderRun {
            ending: RunEnding::Failed,
            turn: 3,
            engine: Some("Not A Word".into()),
            model: None,
            summary: "é".repeat(MAX_RUN_SUMMARY_BYTES),
            files: vec![],
            commands: (0..40).map(|n| format!("echo {n}")).collect(),
        }
        .json();
        assert!(long["summary"].as_str().unwrap().len() <= MAX_RUN_SUMMARY_BYTES);
        assert_eq!(long["commands"].as_array().unwrap().len(), MAX_RUN_COMMANDS);
        assert_eq!(long["commands"][0], "echo 24");
        assert!(long.get("engine").is_none());
        assert_eq!(long["ending"], "failed");
    }

    /// A turn sent while the chat's Coder run is still going carries it,
    /// with the words the worker reads (#10143).
    #[test]
    fn a_running_coder_run_is_sent() {
        for (ending, word) in [
            (RunEnding::Running, "running"),
            (RunEnding::Waiting, "waiting"),
        ] {
            let context = Context {
                coder_run: Some(CoderRun {
                    ending,
                    turn: 1,
                    engine: None,
                    model: None,
                    summary: "Reading acceptance-repo".into(),
                    files: vec![],
                    commands: vec![],
                }),
                ..Context::default()
            };
            let run = &context.json()["coder_run"];
            assert_eq!(run["ending"], word);
            assert_eq!(run["summary"], "Reading acceptance-repo");
        }
    }

    /// A phone names its paired computer and the project by name, never a
    /// path; a name or path past its bound, or with a control character,
    /// is left out rather than cut.
    #[test]
    fn a_phone_names_its_computer_and_never_a_path() {
        let context = Context {
            computer_ready: true,
            computer: Some(Computer::Paired {
                name: "Studio Mac".into(),
                engines: vec![],
            }),
            project: Some(Project {
                name: "openagents".into(),
                path: Some("/Users/someone/work/openagents".into()),
            }),
            ..Context::default()
        };
        assert_eq!(
            context.json(),
            json!({"surface": "phone", "computer_ready": true,
                   "computer": {"place": "paired", "name": "Studio Mac"},
                   "project": {"name": "openagents"}})
        );
        assert_eq!(
            crate::basic_coder::payload(&[crate::basic_coder::Turn::user("hi")], &context)["instructions"],
            crate::basic_coder::INSTRUCTIONS
        );
        let odd = Context {
            surface: Surface::Desktop,
            computer: Some(Computer::Here {
                name: Some("x".repeat(MAX_COMPUTER_NAME_CHARS + 1)),
                engines: vec![
                    Engine {
                        engine: "Codex!".into(),
                        state: EngineState::Ready,
                    },
                    Engine {
                        engine: "codex".into(),
                        state: EngineState::NotSignedIn,
                    },
                ],
            }),
            project: Some(Project {
                name: "bad\u{7}".into(),
                path: Some("/tmp/x".into()),
            }),
            ..Context::default()
        };
        assert_eq!(
            odd.json(),
            json!({"surface": "desktop", "computer_ready": false,
                   "computer": {"place": "here",
                                "engines": [{"engine": "codex", "state": "not_signed_in"}]}})
        );
        let long = Context {
            computer: Some(Computer::Here {
                name: None,
                engines: vec![],
            }),
            project: Some(Project {
                name: "deep".into(),
                path: Some(format!("/{}", "a".repeat(MAX_PROJECT_PATH_BYTES))),
            }),
            ..Context::default()
        };
        assert_eq!(long.json()["project"], json!({"name": "deep"}));
        assert_eq!(
            Engine::from_runner(&Runner::NotSignedIn {
                providers: vec!["codex".into(), "claude".into()]
            }),
            [
                Engine {
                    engine: "codex".into(),
                    state: EngineState::NotSignedIn
                },
                Engine {
                    engine: "claude".into(),
                    state: EngineState::NotSignedIn
                },
            ]
        );
    }
}

#[cfg(test)]
mod memory_tests {
    use super::*;

    fn note(name: &str, body: &str) -> MemoryNote {
        MemoryNote {
            name: name.into(),
            kind: "feedback".into(),
            description: "One\nline".into(),
            body: body.into(),
        }
    }

    #[test]
    fn memory_travels_bounded_and_only_when_there_is_some() {
        assert!(Context::default().json().get("memory").is_none());
        let context = Context {
            surface: Surface::Web,
            memory: vec![
                note("Prefers  tabs", "Indent with tabs."),
                note("", "No name, left out."),
                MemoryNote {
                    kind: "mood".into(),
                    ..note("Odd kind", &"x".repeat(MAX_MEMORY_BODY_BYTES + 50))
                },
            ],
            ..Context::default()
        };
        let memory = context.json()["memory"].as_array().unwrap().clone();
        assert_eq!(memory.len(), 2);
        assert_eq!(memory[0]["name"], "Prefers tabs");
        assert_eq!(memory[0]["kind"], "feedback");
        assert_eq!(memory[0]["description"], "One line");
        assert_eq!(memory[1]["kind"], "user");
        assert_eq!(
            memory[1]["body"].as_str().unwrap().len(),
            MAX_MEMORY_BODY_BYTES
        );
        let many = Context {
            memory: (0..100)
                .map(|n| note(&format!("Note {n}"), &"y".repeat(1000)))
                .collect(),
            ..Context::default()
        };
        let memory = many.json()["memory"].as_array().unwrap().clone();
        assert!(memory.len() < 17 && !memory.is_empty());
        assert_eq!(memory[0]["name"], "Note 0");
    }
}

#[cfg(test)]
mod desktop_context_tests {
    #[test]
    fn desktop_requests_identify_the_surface_without_claiming_computer_authority() {
        let context = super::Context {
            surface: super::Surface::Desktop,
            ..Default::default()
        };
        let request =
            crate::basic_coder::payload(&[crate::basic_coder::Turn::user("Hello")], &context);
        assert_eq!(request["context"]["surface"], "desktop");
        assert_eq!(request["client"], "openagents-desktop");
        assert_eq!(request["context"]["computer_ready"], false);
        assert!(
            !request["instructions"]
                .as_str()
                .unwrap()
                .contains("on their phone")
        );
    }
}

#[cfg(test)]
mod decision_evidence_tests {
    use super::*;
    #[test]
    fn local_readings_survive_the_shareable_judgment_bound() {
        let r = route_contract::decision::DecisionReading::new(
            "read_only",
            "READ_ONLY_CONFIDENCE",
            "jev-pinned",
            0.7,
            0.7,
            true,
        )
        .unwrap();
        let payload =
            serde_json::json!({"tier":"model", "model":"jev-pinned", "decisions": vec![r; 40]});
        assert!(payload.to_string().len() > MAX_JUDGMENT_BYTES);
        let mut meta = Meta::default();
        meta.judged(&payload);
        assert_eq!(meta.decisions.len(), 40);
        assert!(meta.judgment.as_ref().unwrap().len() <= MAX_JUDGMENT_BYTES);
        let bound = crate::route::decisions(Some(&meta), "req-fixture");
        assert!(bound.iter().all(|r| r.outcome_key == "req-fixture"));
        assert!(crate::route::decisions(None, "legacy").is_empty());
    }
}
