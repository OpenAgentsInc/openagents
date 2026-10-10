//! NIP-CJ conversation feedback a client renders as its own controls:
//! offers, cards, and the test-set draft (`nips/openagents/NIP-CJ.md`,
//! "Conversation jobs").
//!
//! An **offer** is an action the client may put one tap in front of; a
//! **card** is a closed, typed display record the client draws with its
//! own controls, never as model text; a **draft** is the caller's
//! test-set draft (`openagents.eval-draft.v1`), which the caller keeps and
//! resends each turn. All three are observations: the worker takes no
//! action for an offer, a card grants nothing, and a draft is data, never
//! an instruction. Everything here is a closed union: an offer, screen,
//! card, or field this version doesn't know refuses, so a client never
//! renders what it can't show exactly.
//!
//! The eval offers, cards, and draft belong to NIP-EVAL's extension
//! evaluation profile ([`crate::eval_ext`]).

use std::collections::BTreeSet;

use serde_json::{Map, Value, json};

use crate::contracts::{
    ArtifactRef, ContractError, DefinitionRef, RefusalCode, jcs, parse_artifact, parse_definition,
};
use crate::eval_ext::{
    self, CaseKind, EventPointer, Headline, MAX_CASES, MAX_RUNS, Verdict, event_pointer,
    valid_case_id,
};
use crate::kb::{malformed, mismatch, reject, require, text, unsupported};
use crate::kinds;

/// The draft's schema.
pub const DRAFT_SCHEMA: &str = "openagents.eval-draft.v1";
/// The most bytes a draft's canonical JSON may have.
pub const MAX_DRAFT_BYTES: usize = 64 * 1024;
/// The most tests a draft may hold.
pub const MAX_DRAFT_CASES: usize = 16;
/// The most graders one draft test may hold.
pub const MAX_DRAFT_GRADERS: usize = 16;
/// The most catalog tools a chat-made tool may turn on.
pub const MAX_USES: usize = 8;
/// The most characters a label may have.
pub const MAX_LABEL_CHARS: usize = 80;
/// The longest deck id an `open_presentation` offer names, in bytes.
pub const MAX_DECK_BYTES: usize = 64;
/// The most items a news card may carry.
pub const MAX_NEWS: usize = 5;
/// The most awards a credit card may list.
pub const MAX_CREDITS: usize = 50;
/// The conversation payload versions these bodies ride in.
pub const VERSIONS: &[u64] = &[1, 2];

/// A screen of the app an offer may open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    AccountComputers,
    AccountKeys,
    AccountPlaytest,
    AccountReportProblem,
    Wallet,
    /// A result's details (SCR-20).
    GymResult,
    /// The **Add to the Gym** confirmation (SCR-21).
    GymPublish,
    /// A test set's tests.
    GymTestSet,
    /// The Gym in the Verse, at its EVALS board: published results and the
    /// agents' notes (**See the board**).
    VerseGym,
    /// The desktop app's Map page: the route map (#10085). Sent only to a
    /// desktop turn.
    RoutesMap,
    /// The Verse: the Grid world (**Enter the Grid**).
    Verse,
}

impl Screen {
    /// Every screen, in order.
    pub const ALL: [Screen; 11] = [
        Screen::AccountComputers,
        Screen::AccountKeys,
        Screen::AccountPlaytest,
        Screen::AccountReportProblem,
        Screen::Wallet,
        Screen::GymResult,
        Screen::GymPublish,
        Screen::GymTestSet,
        Screen::VerseGym,
        Screen::RoutesMap,
        Screen::Verse,
    ];

    /// The word the wire carries.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Screen::AccountComputers => "account.computers",
            Screen::AccountKeys => "account.keys",
            Screen::AccountPlaytest => "account.playtest",
            Screen::AccountReportProblem => "account.report_problem",
            Screen::Wallet => "wallet",
            Screen::GymResult => "gym.result",
            Screen::GymPublish => "gym.publish",
            Screen::GymTestSet => "gym.test_set",
            Screen::VerseGym => "verse.gym",
            Screen::RoutesMap => "routes.map",
            Screen::Verse => "verse",
        }
    }

    /// The screen a word names; an unknown word is `None`.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        Screen::ALL.into_iter().find(|s| s.word() == word)
    }
}

/// What a proposed command does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    ReadOnly,
    LocalWrite,
    Publishes,
    Grants,
    Spends,
    Secret,
    LongRunning,
}

impl Effect {
    const ALL: [Effect; 7] = [
        Effect::ReadOnly,
        Effect::LocalWrite,
        Effect::Publishes,
        Effect::Grants,
        Effect::Spends,
        Effect::Secret,
        Effect::LongRunning,
    ];

    /// The word the wire carries.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Effect::ReadOnly => "read_only",
            Effect::LocalWrite => "local_write",
            Effect::Publishes => "publishes",
            Effect::Grants => "grants",
            Effect::Spends => "spends",
            Effect::Secret => "secret",
            Effect::LongRunning => "long_running",
        }
    }

    /// The effect a word names.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        Effect::ALL.into_iter().find(|e| e.word() == word)
    }
}

/// Where a proposed command runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunsOn {
    ThisDevice,
    ConnectedComputer,
    Screen,
}

impl RunsOn {
    /// The word the wire carries.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            RunsOn::ThisDevice => "this_device",
            RunsOn::ConnectedComputer => "connected_computer",
            RunsOn::Screen => "screen",
        }
    }

    /// Where a word says.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "this_device" => Some(RunsOn::ThisDevice),
            "connected_computer" => Some(RunsOn::ConnectedComputer),
            "screen" => Some(RunsOn::Screen),
            _ => None,
        }
    }
}

/// Where an eval runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Where {
    /// The hosted runner, on our computers.
    Hosted,
    /// The person's connected computer.
    ConnectedComputer,
}

impl Where {
    /// The word the wire carries.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Where::Hosted => "hosted",
            Where::ConnectedComputer => "connected_computer",
        }
    }

    /// Where a word says.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "hosted" => Some(Where::Hosted),
            "connected_computer" => Some(Where::ConnectedComputer),
            _ => None,
        }
    }
}

/// The size of an eval: tests, runs per arm, and arms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub cases: u64,
    pub runs: u64,
    pub arms: u64,
}

/// The suite a `start_eval` offer runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuiteSource {
    /// A published suite's NIP-EXT release.
    Published(EventPointer),
    /// The caller's draft, which the client sends with its request.
    Draft,
}

/// The tool a `start_eval` offer tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubjectSource {
    /// A catalog tool or a published extension.
    Definition(Box<DefinitionRef>),
    /// The tool in the caller's draft.
    Draft,
}

/// An action the client may render for the person to tap. An observation,
/// never permission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Offer {
    /// Dispatch Coder to the connected computer with this conversation.
    /// `engine` is the coding engine the person asked for, when the
    /// router's typed `engine` reading named one (#10076): a preference
    /// the start puts first, never permission to run an engine the
    /// computer's owner did not allow. `None` is no preference.
    ///
    /// `plan` is the router's typed dispatch plan (#10183): one run, or
    /// one run on each of `plan.runs`, read-only or not. The default is
    /// one run that may change files, the offer as it was before.
    RunCoder {
        label: String,
        engine: Option<Engine>,
        plan: Plan,
    },
    /// Open a screen of the app.
    OpenScreen { screen: Screen, label: String },
    /// Run an `openagents` command after a confirm.
    Cli {
        argv: Vec<String>,
        effect: Effect,
        runs_on: RunsOn,
    },
    /// Run a test set against a tool: the tap makes the client send its
    /// own signed execution request.
    StartEval {
        suite: SuiteSource,
        subject: SubjectSource,
        size: Size,
        at: Where,
        label: String,
    },
    /// Publish a result the caller holds: the tap opens a confirmation
    /// first.
    PublishEval { report: ArtifactRef, label: String },
    /// Open a deck in the desktop app's slide viewer: `deck` is a deck id
    /// (lowercase ASCII letters, digits, and hyphens), which the client
    /// opens only when its own deck list has it.
    OpenPresentation { deck: String, label: String },
}

impl Offer {
    /// The word the wire carries in `offer`.
    #[must_use]
    pub fn word(&self) -> &'static str {
        match self {
            Offer::RunCoder { .. } => "run_coder",
            Offer::OpenScreen { .. } => "open_screen",
            Offer::Cli { .. } => "cli",
            Offer::StartEval { .. } => "start_eval",
            Offer::PublishEval { .. } => "publish_eval",
            Offer::OpenPresentation { .. } => "open_presentation",
        }
    }
}

/// How many Coder runs a `run_coder` offer starts, on which engines, and
/// with what access (#10183). The router fills it only from its typed
/// readings, never from text.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Plan {
    /// Empty: one run, as before. Otherwise one run on each engine, in
    /// order: 2 to [`MAX_PLAN_RUNS`] distinct engines.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runs: Vec<Engine>,
    /// The runs only read: Coder starts them under a boundary that
    /// writes nothing in the worktree and seals Git, whatever the
    /// computer's access setting.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub read_only: bool,
    /// The person asked for one combined summary of the runs' results,
    /// which the chat writes once they all end.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub summarize: bool,
}

/// The most runs one plan starts: one per engine.
pub const MAX_PLAN_RUNS: usize = Engine::ALL.len();

impl Plan {
    /// One run that may change files: the offer before plans.
    #[must_use]
    pub fn is_single(&self) -> bool {
        self.runs.is_empty()
    }

    /// The plan's runs, each a distinct engine, 2 to [`MAX_PLAN_RUNS`],
    /// or none.
    #[must_use]
    pub fn valid(&self) -> bool {
        self.runs.is_empty()
            || (2..=MAX_PLAN_RUNS).contains(&self.runs.len())
                && self
                    .runs
                    .iter()
                    .enumerate()
                    .all(|(at, engine)| !self.runs[..at].contains(engine))
    }
}

/// A coding engine a `run_coder` offer may name as the person's request
/// (#10076). A closed set: an engine word this version doesn't know
/// refuses the offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Engine {
    #[serde(rename = "codex")]
    Codex,
    #[serde(rename = "claude_code")]
    ClaudeCode,
    #[serde(rename = "grok_build")]
    GrokBuild,
    #[serde(rename = "opencode")]
    OpenCode,
    #[serde(rename = "devin")]
    Devin,
}

impl Engine {
    /// Every engine, in the order the router lists them.
    pub const ALL: [Engine; 5] = [
        Engine::Codex,
        Engine::ClaudeCode,
        Engine::GrokBuild,
        Engine::OpenCode,
        Engine::Devin,
    ];

    /// The word the wire carries in a `run_coder` offer's `engine`.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Engine::Codex => "codex",
            Engine::ClaudeCode => "claude_code",
            Engine::GrokBuild => "grok_build",
            Engine::OpenCode => "opencode",
            Engine::Devin => "devin",
        }
    }

    /// The engine an exact wire word names.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        Engine::ALL.into_iter().find(|engine| engine.word() == word)
    }

    /// The engine's name as a person reads it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Engine::Codex => "Codex",
            Engine::ClaudeCode => "Claude Code",
            Engine::GrokBuild => "Grok Build",
            Engine::OpenCode => "OpenCode",
            Engine::Devin => "Devin",
        }
    }
}

/// Whether `deck` is a deck id an `open_presentation` offer may name:
/// lowercase ASCII letters, digits, and hyphens, at most
/// [`MAX_DECK_BYTES`].
#[must_use]
pub fn deck_like(deck: &str) -> bool {
    (1..=MAX_DECK_BYTES).contains(&deck.len())
        && deck
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn label(object: &Map<String, Value>) -> Result<String, ContractError> {
    let value = text(object, "label")?;
    if value.trim().is_empty() || value.chars().count() > MAX_LABEL_CHARS {
        return Err(malformed("label"));
    }
    Ok(value)
}

fn envelope<'a>(
    value: &'a Value,
    kind: &str,
) -> Result<(u64, &'a Map<String, Value>), ContractError> {
    let object = value.as_object().ok_or_else(|| malformed(kind))?;
    let version = object
        .get("v")
        .and_then(Value::as_u64)
        .ok_or_else(|| malformed("v"))?;
    if !VERSIONS.contains(&version) {
        return Err(ContractError::new(RefusalCode::UnsupportedVersion, "v"));
    }
    if object
        .get("requires")
        .and_then(Value::as_array)
        .is_none_or(|r| !r.is_empty())
    {
        return Err(unsupported("requires"));
    }
    if object.get("type").and_then(Value::as_str) != Some(kind) {
        return Err(mismatch("type"));
    }
    Ok((version, object))
}

fn size(value: &Value) -> Result<Size, ContractError> {
    let object = value.as_object().ok_or_else(|| malformed("size"))?;
    reject(object, &["cases", "runs", "arms"])?;
    let count = |key: &str| {
        object
            .get(key)
            .and_then(Value::as_u64)
            .ok_or_else(|| malformed(format!("size.{key}")))
    };
    Ok(Size {
        cases: count("cases")?,
        runs: count("runs")?,
        arms: count("arms")?,
    })
}

/// Reads one `offer` feedback body. Exact enum values only: an offer,
/// screen, effect, or place this version doesn't know refuses, as does a
/// field an offer doesn't have. A hosted `start_eval` must fit the hosted
/// runner's bounds.
///
/// # Errors
///
/// [`RefusalCode::UnsupportedFeature`] for an offer the client doesn't
/// know; other codes for a malformed body.
pub fn parse_offer(value: &Value) -> Result<(u64, Offer), ContractError> {
    let (version, object) = envelope(value, "offer")?;
    let word = text(object, "offer")?;
    let common = ["v", "requires", "type", "offer"];
    let allow = |extra: &[&str]| {
        let mut keys: Vec<&str> = common.to_vec();
        keys.extend_from_slice(extra);
        reject(object, &keys)
    };
    let offer = match word.as_str() {
        "run_coder" => {
            allow(&[
                "target",
                "label",
                "engine",
                "runs",
                "read_only",
                "summarize",
            ])?;
            if object.get("target").and_then(Value::as_str) != Some("connected_computer") {
                return Err(unsupported("target"));
            }
            let engine = match object.get("engine") {
                None => None,
                Some(value) => Some(
                    value
                        .as_str()
                        .and_then(Engine::parse)
                        .ok_or_else(|| unsupported("engine"))?,
                ),
            };
            let runs = match object.get("runs") {
                None => Vec::new(),
                Some(value) => value
                    .as_array()
                    .ok_or_else(|| unsupported("runs"))?
                    .iter()
                    .map(|word| word.as_str().and_then(Engine::parse))
                    .collect::<Option<Vec<Engine>>>()
                    .ok_or_else(|| unsupported("runs"))?,
            };
            let flag = |key: &str| match object.get(key) {
                None => Ok(false),
                Some(Value::Bool(flag)) => Ok(*flag),
                Some(_) => Err(unsupported(key)),
            };
            let plan = Plan {
                runs,
                read_only: flag("read_only")?,
                summarize: flag("summarize")?,
            };
            if !plan.valid() || object.contains_key("runs") && plan.runs.is_empty() {
                return Err(unsupported("runs"));
            }
            Offer::RunCoder {
                label: label(object)?,
                engine,
                plan,
            }
        }
        "open_screen" => {
            allow(&["screen", "label"])?;
            let screen =
                Screen::parse(&text(object, "screen")?).ok_or_else(|| unsupported("screen"))?;
            Offer::OpenScreen {
                screen,
                label: label(object)?,
            }
        }
        "cli" => {
            allow(&["argv", "effect", "runs_on", "confirm"])?;
            if object.get("confirm") != Some(&Value::Bool(true)) {
                return Err(malformed("confirm: a command is offered with a confirm"));
            }
            let argv: Vec<String> = require(object, "argv")?
                .as_array()
                .ok_or_else(|| malformed("argv"))?
                .iter()
                .map(|w| {
                    w.as_str()
                        .filter(|w| {
                            !w.is_empty() && w.len() <= 256 && !w.chars().any(char::is_control)
                        })
                        .map(str::to_string)
                        .ok_or_else(|| malformed("argv"))
                })
                .collect::<Result<_, _>>()?;
            if argv.is_empty() || argv.len() > 32 {
                return Err(malformed("argv"));
            }
            Offer::Cli {
                argv,
                effect: Effect::parse(&text(object, "effect")?)
                    .ok_or_else(|| unsupported("effect"))?,
                runs_on: RunsOn::parse(&text(object, "runs_on")?)
                    .ok_or_else(|| unsupported("runs_on"))?,
            }
        }
        "start_eval" => {
            allow(&["suite", "subject", "size", "where", "label"])?;
            let suite = match require(object, "suite")? {
                Value::String(s) if s == "draft" => SuiteSource::Draft,
                value => SuiteSource::Published(event_pointer(value, kinds::EXT_RELEASE, "suite")?),
            };
            let subject = match require(object, "subject")? {
                Value::String(s) if s == "draft" => SubjectSource::Draft,
                value => SubjectSource::Definition(Box::new(parse_definition(value)?)),
            };
            let size = size(require(object, "size")?)?;
            let at = Where::parse(&text(object, "where")?).ok_or_else(|| unsupported("where"))?;
            match at {
                Where::Hosted => {
                    eval_ext::check_hosted_size(size.cases, size.runs, size.arms)?;
                }
                Where::ConnectedComputer => {
                    if size.cases == 0
                        || size.cases > MAX_CASES as u64
                        || !(1..=MAX_RUNS).contains(&size.runs)
                        || !(1..=2).contains(&size.arms)
                    {
                        return Err(malformed("size"));
                    }
                }
            }
            Offer::StartEval {
                suite,
                subject,
                size,
                at,
                label: label(object)?,
            }
        }
        "publish_eval" => {
            allow(&["report", "label"])?;
            let report = parse_artifact(require(object, "report")?)?;
            if report.schema.as_deref() != Some(crate::kb::REPORT_SCHEMA) {
                return Err(mismatch("report: schema"));
            }
            Offer::PublishEval {
                report,
                label: label(object)?,
            }
        }
        "open_presentation" => {
            allow(&["deck", "label"])?;
            let deck = text(object, "deck")?;
            if !deck_like(&deck) {
                return Err(malformed("deck"));
            }
            Offer::OpenPresentation {
                deck,
                label: label(object)?,
            }
        }
        other => return Err(unsupported(format!("offer {other}"))),
    };
    Ok((version, offer))
}

/// A test in a draft: the exact file texts the runner writes out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftCase {
    pub id: String,
    pub kind: CaseKind,
    /// The whole `prompt.md`.
    pub prompt: String,
    /// `graders/<name>.md` as `(name, whole file)`, in name order.
    pub graders: Vec<(String, String)>,
}

/// The tool a draft tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftTool {
    pub name: String,
    pub summary: String,
    /// An existing tool: a catalog tool or a published extension.
    pub catalog: Option<DefinitionRef>,
    /// A chat-made tool's plain-language guidance: a skill.
    pub skill: Option<String>,
    /// The catalog tools a chat-made tool turns on, as qualified IDs.
    pub uses: Vec<String>,
}

/// A verified test-set draft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    pub tool: DraftTool,
    pub cases: Vec<DraftCase>,
}

fn bounded(object: &Map<String, Value>, key: &str, max: usize) -> Result<String, ContractError> {
    let value = text(object, key)?;
    if value.chars().count() > max {
        return Err(ContractError::new(RefusalCode::LimitExceeded, key));
    }
    Ok(value)
}

fn draft_tool(value: &Value) -> Result<DraftTool, ContractError> {
    let object = value.as_object().ok_or_else(|| malformed("tool"))?;
    reject(object, &["name", "summary", "catalog", "skill", "uses"])?;
    let name = bounded(object, "name", 80)?;
    if name.trim().is_empty() {
        return Err(malformed("tool.name"));
    }
    let summary = bounded(object, "summary", 400)?;
    let catalog = match require(object, "catalog")? {
        Value::Null => None,
        value => Some(parse_definition(value)?),
    };
    let skill = match require(object, "skill")? {
        Value::Null => None,
        Value::String(s) if !s.trim().is_empty() && s.len() <= 16 * 1024 => Some(s.clone()),
        _ => return Err(malformed("tool.skill")),
    };
    let uses: Vec<String> = require(object, "uses")?
        .as_array()
        .ok_or_else(|| malformed("tool.uses"))?
        .iter()
        .map(|u| {
            u.as_str()
                .map(str::to_string)
                .ok_or_else(|| malformed("tool.uses"))
        })
        .collect::<Result<_, _>>()?;
    if uses.len() > MAX_USES
        || uses.iter().collect::<BTreeSet<_>>().len() != uses.len()
        || uses.iter().any(|id| !eval_ext::valid_qualified(id))
    {
        return Err(malformed("tool.uses"));
    }
    match (&catalog, &skill) {
        (Some(_), None) if uses.is_empty() => {}
        (None, Some(_)) => {}
        _ => {
            return Err(malformed(
                "tool: a catalog tool names its definition; a chat-made tool has a skill and may turn on catalog tools",
            ));
        }
    }
    Ok(DraftTool {
        name,
        summary,
        catalog,
        skill,
        uses,
    })
}

fn draft_case(value: &Value) -> Result<DraftCase, ContractError> {
    let object = value.as_object().ok_or_else(|| malformed("case"))?;
    reject(object, &["id", "kind", "prompt", "graders"])?;
    let id = text(object, "id")?;
    if !valid_case_id(&id) {
        return Err(malformed("case.id"));
    }
    let kind = CaseKind::parse(&text(object, "kind")?).ok_or_else(|| unsupported("case.kind"))?;
    let prompt = text(object, "prompt")?;
    if prompt.trim().is_empty() {
        return Err(malformed("case.prompt"));
    }
    let items = require(object, "graders")?
        .as_array()
        .ok_or_else(|| malformed("case.graders"))?;
    if items.is_empty() || items.len() > MAX_DRAFT_GRADERS {
        return Err(malformed("case.graders: 1 to 16"));
    }
    let mut graders: Vec<(String, String)> = Vec::new();
    for item in items {
        let grader = item.as_object().ok_or_else(|| malformed("grader"))?;
        reject(grader, &["name", "text"])?;
        let name = text(grader, "name")?;
        if !valid_case_id(&name) || name.len() > 64 {
            return Err(malformed("grader.name"));
        }
        let body = text(grader, "text")?;
        if body.trim().is_empty() {
            return Err(malformed("grader.text"));
        }
        if graders.last().is_some_and(|(last, _)| *last >= name) {
            return Err(malformed("graders: names are unique and in order"));
        }
        graders.push((name, body));
    }
    Ok(DraftCase {
        id,
        kind,
        prompt,
        graders,
    })
}

/// Checks a draft: the closed shape, a tool that is either a catalog tool
/// or a chat-made skill, unique test IDs, and at most 64 KiB of canonical
/// JSON.
///
/// # Errors
///
/// [`RefusalCode::LimitExceeded`] for an oversize draft; other codes for a
/// malformed one.
pub fn parse_draft(value: &Value) -> Result<Draft, ContractError> {
    if jcs(value)?.len() > MAX_DRAFT_BYTES {
        return Err(ContractError::new(
            RefusalCode::LimitExceeded,
            "the draft is over 64 KiB",
        ));
    }
    let object = value.as_object().ok_or_else(|| malformed("draft"))?;
    reject(object, &["v", "tool", "cases"])?;
    if object.get("v").and_then(Value::as_str) != Some(DRAFT_SCHEMA) {
        return Err(ContractError::new(
            RefusalCode::UnsupportedVersion,
            "draft.v",
        ));
    }
    let tool = draft_tool(require(object, "tool")?)?;
    let items = require(object, "cases")?
        .as_array()
        .ok_or_else(|| malformed("cases"))?;
    if items.len() > MAX_DRAFT_CASES {
        return Err(ContractError::new(RefusalCode::LimitExceeded, "cases"));
    }
    let mut cases = Vec::new();
    let mut seen = BTreeSet::new();
    for item in items {
        let case = draft_case(item)?;
        if !seen.insert(case.id.clone()) {
            return Err(ContractError::new(RefusalCode::Conflict, "case.id"));
        }
        cases.push(case);
    }
    Ok(Draft { tool, cases })
}

/// A draft's JSON, checked.
///
/// # Errors
///
/// As [`parse_draft`].
pub fn draft_value(draft: &Draft) -> Result<Value, ContractError> {
    let tool = &draft.tool;
    let value = json!({
        "v": DRAFT_SCHEMA,
        "tool": {
            "name": tool.name,
            "summary": tool.summary,
            "catalog": tool.catalog.as_ref().map(definition_value),
            "skill": tool.skill,
            "uses": tool.uses,
        },
        "cases": draft.cases.iter().map(|c| json!({
            "id": c.id,
            "kind": c.kind.word(),
            "prompt": c.prompt,
            "graders": c.graders.iter().map(|(name, text)| json!({"name": name, "text": text})).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    });
    parse_draft(&value)?;
    Ok(value)
}

/// A published result, in one line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultLine {
    /// The `3189` it comes from.
    pub publication: EventPointer,
    pub headline: Headline,
    pub verdict: Verdict,
}

/// Where a news item comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// A signed record.
    Event(EventPointer),
    /// A repository path, such as the app's changelog.
    Path(String),
}

/// One news item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewsItem {
    pub title: String,
    pub line: String,
    pub source: Source,
}

/// One award on a credit card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreditItem {
    /// Confirmed: a referee signed the award. Pending: the rule holds and
    /// no award is signed yet.
    pub confirmed: bool,
    pub role: String,
    pub xp: u64,
    pub title: String,
    /// The `3193`, when confirmed.
    pub award: Option<EventPointer>,
}

/// Where a capability can be used from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    /// From the chat itself.
    Chat,
    /// Only in a Coder run on a computer.
    Coder,
}

impl Reach {
    /// The word the wire carries.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Reach::Chat => "chat",
            Reach::Coder => "coder",
        }
    }

    /// The reach a word names.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "chat" => Some(Reach::Chat),
            "coder" => Some(Reach::Coder),
            _ => None,
        }
    }
}

/// The admitted capability nearest a request that none covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Closest {
    pub name: String,
    pub summary: String,
    pub reach: Reach,
}

/// How a person can add the capability a `capability` card says is
/// missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Add {
    /// Make one in the chat's authoring interview.
    Author,
    /// Open the Gym.
    Gym,
}

impl Add {
    /// The word the wire carries.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Add::Author => "author",
            Add::Gym => "gym",
        }
    }

    /// The way a word names.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "author" => Some(Add::Author),
            "gym" => Some(Add::Gym),
            _ => None,
        }
    }
}

/// A closed display record. Every number comes from a record the worker
/// verified, and the card cites it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Card {
    /// A request calls for a capability that isn't admitted yet: the
    /// closest admitted one, if any, and how to add one. It carries
    /// nothing of the message.
    Capability { closest: Option<Closest>, add: Add },
    /// A tool, its plain description, and its latest verified result.
    Tool {
        name: String,
        summary: String,
        definition: Option<DefinitionRef>,
        latest: Option<ResultLine>,
    },
    /// The returned draft.
    Draft(Draft),
    /// A run's progress.
    Run {
        request: EventPointer,
        at: Where,
        completed: u64,
        planned: u64,
    },
    /// A result's headline, verdict, and report.
    Result {
        headline: Headline,
        verdict: Verdict,
        report: ArtifactRef,
        publication: Option<EventPointer>,
    },
    /// What's new in the Gym.
    News(Vec<NewsItem>),
    /// A result waiting for a check.
    Check {
        tool: String,
        line: ResultLine,
        confirms: u64,
        disputes: u64,
    },
    /// Awards from the reader's ledger.
    Credit { total: u64, awards: Vec<CreditItem> },
}

impl Card {
    /// The word the wire carries in `card`.
    #[must_use]
    pub fn word(&self) -> &'static str {
        match self {
            Card::Tool { .. } => "tool",
            Card::Draft(_) => "draft",
            Card::Run { .. } => "run",
            Card::Result { .. } => "result",
            Card::News(_) => "news",
            Card::Check { .. } => "check",
            Card::Credit { .. } => "credit",
            Card::Capability { .. } => "capability",
        }
    }
}

/// The card types this version renders.
pub const CARDS: &[&str] = &[
    "tool",
    "draft",
    "run",
    "result",
    "news",
    "check",
    "credit",
    "capability",
];

fn headline(value: &Value) -> Result<Headline, ContractError> {
    let object = value.as_object().ok_or_else(|| malformed("headline"))?;
    reject(object, &["subject_passed", "baseline_passed", "total"])?;
    let count = |key: &str| {
        object
            .get(key)
            .and_then(Value::as_u64)
            .ok_or_else(|| malformed(format!("headline.{key}")))
    };
    let total = count("total")?;
    let subject_passed = count("subject_passed")?;
    let baseline_passed = match require(object, "baseline_passed")? {
        Value::Null => None,
        v => Some(
            v.as_u64()
                .ok_or_else(|| malformed("headline.baseline_passed"))?,
        ),
    };
    if total == 0 || subject_passed > total || baseline_passed.is_some_and(|b| b > total) {
        return Err(malformed("headline"));
    }
    Ok(Headline {
        subject_passed,
        baseline_passed,
        total,
    })
}

fn headline_value(h: &Headline) -> Value {
    json!({"subject_passed": h.subject_passed, "baseline_passed": h.baseline_passed, "total": h.total})
}

fn verdict(object: &Map<String, Value>) -> Result<Verdict, ContractError> {
    Verdict::parse(&text(object, "verdict")?).ok_or_else(|| unsupported("verdict"))
}

fn result_line(value: &Value) -> Result<ResultLine, ContractError> {
    let object = value.as_object().ok_or_else(|| malformed("latest"))?;
    reject(object, &["publication", "headline", "verdict"])?;
    Ok(ResultLine {
        publication: event_pointer(
            require(object, "publication")?,
            kinds::EVAL_DECLARATION,
            "publication",
        )?,
        headline: headline(require(object, "headline")?)?,
        verdict: verdict(object)?,
    })
}

fn result_line_value(line: &ResultLine) -> Value {
    json!({
        "publication": line.publication.to_value(),
        "headline": headline_value(&line.headline),
        "verdict": line.verdict.word(),
    })
}

fn short(object: &Map<String, Value>, key: &str, max: usize) -> Result<String, ContractError> {
    let value = bounded(object, key, max)?;
    if value.trim().is_empty() {
        return Err(malformed(key));
    }
    Ok(value)
}

/// Reads one `card` feedback body.
///
/// # Errors
///
/// [`RefusalCode::UnsupportedFeature`] for a card type the client doesn't
/// know; other codes for a malformed card.
pub fn parse_card(value: &Value) -> Result<(u64, Card), ContractError> {
    let (version, object) = envelope(value, "card")?;
    let word = text(object, "card")?;
    let allow = |extra: &[&str]| {
        let mut keys = vec!["v", "requires", "type", "card"];
        keys.extend_from_slice(extra);
        reject(object, &keys)
    };
    let card = match word.as_str() {
        "tool" => {
            allow(&["name", "summary", "definition", "latest"])?;
            Card::Tool {
                name: short(object, "name", 80)?,
                summary: bounded(object, "summary", 400)?,
                definition: match require(object, "definition")? {
                    Value::Null => None,
                    v => Some(parse_definition(v)?),
                },
                latest: match require(object, "latest")? {
                    Value::Null => None,
                    v => Some(result_line(v)?),
                },
            }
        }
        "draft" => {
            allow(&["draft"])?;
            Card::Draft(parse_draft(require(object, "draft")?)?)
        }
        "run" => {
            allow(&["request", "where", "completed", "planned"])?;
            let count = |key: &str| {
                object
                    .get(key)
                    .and_then(Value::as_u64)
                    .ok_or_else(|| malformed(key))
            };
            let (completed, planned) = (count("completed")?, count("planned")?);
            if planned == 0 || completed > planned {
                return Err(malformed("run: completed is at most planned"));
            }
            Card::Run {
                request: event_pointer(
                    require(object, "request")?,
                    kinds::CJ_EXECUTION_REQUEST,
                    "request",
                )?,
                at: Where::parse(&text(object, "where")?).ok_or_else(|| unsupported("where"))?,
                completed,
                planned,
            }
        }
        "result" => {
            allow(&["headline", "verdict", "report", "publication"])?;
            let report = parse_artifact(require(object, "report")?)?;
            if report.schema.as_deref() != Some(crate::kb::REPORT_SCHEMA) {
                return Err(mismatch("report: schema"));
            }
            Card::Result {
                headline: headline(require(object, "headline")?)?,
                verdict: verdict(object)?,
                report,
                publication: match require(object, "publication")? {
                    Value::Null => None,
                    v => Some(event_pointer(v, kinds::EVAL_DECLARATION, "publication")?),
                },
            }
        }
        "news" => {
            allow(&["items"])?;
            let items = require(object, "items")?
                .as_array()
                .ok_or_else(|| malformed("items"))?;
            if items.is_empty() || items.len() > MAX_NEWS {
                return Err(malformed("items: 1 to 5"));
            }
            let mut out = Vec::new();
            for item in items {
                let item = item.as_object().ok_or_else(|| malformed("item"))?;
                reject(item, &["title", "line", "event", "path"])?;
                let source = match (require(item, "event")?, require(item, "path")?) {
                    (Value::Null, Value::String(path))
                        if !path.is_empty()
                            && path.len() <= 256
                            && !path.starts_with('/')
                            && !path.split('/').any(|p| p == "..") =>
                    {
                        Source::Path(path.clone())
                    }
                    (event, Value::Null) if !event.is_null() => Source::Event(
                        event_pointer_any(event).ok_or_else(|| malformed("item.event"))?,
                    ),
                    _ => {
                        return Err(malformed(
                            "item: every news item cites exactly one event or path",
                        ));
                    }
                };
                out.push(NewsItem {
                    title: short(item, "title", 120)?,
                    line: short(item, "line", 400)?,
                    source,
                });
            }
            Card::News(out)
        }
        "check" => {
            allow(&[
                "tool",
                "publication",
                "headline",
                "verdict",
                "confirms",
                "disputes",
            ])?;
            let count = |key: &str| {
                object
                    .get(key)
                    .and_then(Value::as_u64)
                    .ok_or_else(|| malformed(key))
            };
            Card::Check {
                tool: short(object, "tool", 80)?,
                line: ResultLine {
                    publication: event_pointer(
                        require(object, "publication")?,
                        kinds::EVAL_DECLARATION,
                        "publication",
                    )?,
                    headline: headline(require(object, "headline")?)?,
                    verdict: verdict(object)?,
                },
                confirms: count("confirms")?,
                disputes: count("disputes")?,
            }
        }
        "credit" => {
            allow(&["total", "awards"])?;
            let total = object
                .get("total")
                .and_then(Value::as_u64)
                .ok_or_else(|| malformed("total"))?;
            let items = require(object, "awards")?
                .as_array()
                .ok_or_else(|| malformed("awards"))?;
            if items.len() > MAX_CREDITS {
                return Err(ContractError::new(RefusalCode::LimitExceeded, "awards"));
            }
            let mut awards = Vec::new();
            for item in items {
                let item = item.as_object().ok_or_else(|| malformed("award"))?;
                reject(item, &["status", "role", "xp", "title", "award"])?;
                let confirmed = match text(item, "status")?.as_str() {
                    "confirmed" => true,
                    "pending" => false,
                    _ => return Err(unsupported("award.status")),
                };
                let award = match require(item, "award")? {
                    Value::Null => None,
                    v => Some(event_pointer(v, kinds::XP_AWARD, "award")?),
                };
                if confirmed != award.is_some() {
                    return Err(mismatch(
                        "a confirmed award cites its 3193; a pending one none",
                    ));
                }
                awards.push(CreditItem {
                    confirmed,
                    role: short(item, "role", 64)?,
                    xp: item
                        .get("xp")
                        .and_then(Value::as_u64)
                        .ok_or_else(|| malformed("award.xp"))?,
                    title: short(item, "title", 200)?,
                    award,
                });
            }
            let confirmed: u64 = awards.iter().filter(|a| a.confirmed).map(|a| a.xp).sum();
            if total != confirmed {
                return Err(mismatch("total: the confirmed awards' XP"));
            }
            Card::Credit { total, awards }
        }
        "capability" => {
            allow(&["status", "closest", "add"])?;
            if text(object, "status")? != "missing" {
                return Err(unsupported("capability.status"));
            }
            let closest = match require(object, "closest")? {
                Value::Null => None,
                value => {
                    let item = value.as_object().ok_or_else(|| malformed("closest"))?;
                    reject(item, &["name", "summary", "reach"])?;
                    Some(Closest {
                        name: short(item, "name", 80)?,
                        summary: short(item, "summary", 400)?,
                        reach: Reach::parse(&text(item, "reach")?)
                            .ok_or_else(|| unsupported("closest.reach"))?,
                    })
                }
            };
            Card::Capability {
                closest,
                add: Add::parse(&text(object, "add")?).ok_or_else(|| unsupported("add"))?,
            }
        }
        other => return Err(unsupported(format!("card {other}"))),
    };
    Ok((version, card))
}

fn event_pointer_any(value: &Value) -> Option<EventPointer> {
    let kind = value.get("kind").and_then(Value::as_u64)?;
    event_pointer(value, u16::try_from(kind).ok()?, "event").ok()
}

/// A DefinitionRef's JSON, closed as the contracts read it.
#[must_use]
pub fn definition_value(definition: &DefinitionRef) -> Value {
    let mut value = json!({
        "id": definition.id,
        "artifact": artifact_value(&definition.artifact),
    });
    if let Some(event) = &definition.event {
        value["event"] = json!({"id": event.id, "pubkey": event.pubkey, "kind": event.kind});
    }
    value
}

/// An ArtifactRef's JSON, without locator hints.
#[must_use]
pub fn artifact_value(artifact: &ArtifactRef) -> Value {
    let mut value = json!({
        "digest": artifact.digest,
        "size": artifact.size,
        "media_type": artifact.media_type,
    });
    if let Some(schema) = &artifact.schema {
        value["schema"] = json!(schema);
    }
    if let Some(event) = &artifact.event {
        value["event"] = json!({"id": event.id, "pubkey": event.pubkey, "kind": event.kind});
    }
    value
}

fn body(version: u64, kind: &str, word: &str) -> Value {
    json!({"v": version, "requires": [], "type": kind, kind: word})
}

/// The `offer` feedback body for `offer` at payload `version`, checked.
///
/// # Errors
///
/// As [`parse_offer`].
pub fn offer_feedback(offer: &Offer, version: u64) -> Result<Value, ContractError> {
    let mut value = body(version, "offer", offer.word());
    match offer {
        Offer::RunCoder {
            label,
            engine,
            plan,
        } => {
            value["target"] = json!("connected_computer");
            value["label"] = json!(label);
            if let Some(engine) = engine {
                value["engine"] = json!(engine.word());
            }
            if !plan.runs.is_empty() {
                value["runs"] = json!(plan.runs.iter().map(|e| e.word()).collect::<Vec<_>>());
            }
            if plan.read_only {
                value["read_only"] = json!(true);
            }
            if plan.summarize {
                value["summarize"] = json!(true);
            }
        }
        Offer::OpenScreen { screen, label } => {
            value["screen"] = json!(screen.word());
            value["label"] = json!(label);
        }
        Offer::Cli {
            argv,
            effect,
            runs_on,
        } => {
            value["argv"] = json!(argv);
            value["effect"] = json!(effect.word());
            value["runs_on"] = json!(runs_on.word());
            value["confirm"] = json!(true);
        }
        Offer::StartEval {
            suite,
            subject,
            size,
            at,
            label,
        } => {
            value["suite"] = match suite {
                SuiteSource::Published(release) => release.to_value(),
                SuiteSource::Draft => json!("draft"),
            };
            value["subject"] = match subject {
                SubjectSource::Definition(definition) => definition_value(definition),
                SubjectSource::Draft => json!("draft"),
            };
            value["size"] = json!({"cases": size.cases, "runs": size.runs, "arms": size.arms});
            value["where"] = json!(at.word());
            value["label"] = json!(label);
        }
        Offer::PublishEval { report, label } => {
            value["report"] = artifact_value(report);
            value["label"] = json!(label);
        }
        Offer::OpenPresentation { deck, label } => {
            value["deck"] = json!(deck);
            value["label"] = json!(label);
        }
    }
    parse_offer(&value)?;
    Ok(value)
}

/// The `card` feedback body for `card` at payload `version`, checked.
///
/// # Errors
///
/// As [`parse_card`].
pub fn card_feedback(card: &Card, version: u64) -> Result<Value, ContractError> {
    let mut value = body(version, "card", card.word());
    match card {
        Card::Tool {
            name,
            summary,
            definition,
            latest,
        } => {
            value["name"] = json!(name);
            value["summary"] = json!(summary);
            value["definition"] = definition.as_ref().map_or(Value::Null, definition_value);
            value["latest"] = latest.as_ref().map_or(Value::Null, result_line_value);
        }
        Card::Draft(draft) => value["draft"] = draft_value(draft)?,
        Card::Run {
            request,
            at,
            completed,
            planned,
        } => {
            value["request"] = request.to_value();
            value["where"] = json!(at.word());
            value["completed"] = json!(completed);
            value["planned"] = json!(planned);
        }
        Card::Result {
            headline,
            verdict,
            report,
            publication,
        } => {
            value["headline"] = headline_value(headline);
            value["verdict"] = json!(verdict.word());
            value["report"] = artifact_value(report);
            value["publication"] = publication
                .as_ref()
                .map_or(Value::Null, EventPointer::to_value);
        }
        Card::News(items) => {
            value["items"] = items
                .iter()
                .map(|item| {
                    let (event, path) = match &item.source {
                        Source::Event(e) => (e.to_value(), Value::Null),
                        Source::Path(p) => (Value::Null, json!(p)),
                    };
                    json!({"title": item.title, "line": item.line, "event": event, "path": path})
                })
                .collect();
        }
        Card::Check {
            tool,
            line,
            confirms,
            disputes,
        } => {
            value["tool"] = json!(tool);
            value["publication"] = line.publication.to_value();
            value["headline"] = headline_value(&line.headline);
            value["verdict"] = json!(line.verdict.word());
            value["confirms"] = json!(confirms);
            value["disputes"] = json!(disputes);
        }
        Card::Credit { total, awards } => {
            value["total"] = json!(total);
            value["awards"] = awards
                .iter()
                .map(|a| {
                    json!({
                        "status": if a.confirmed { "confirmed" } else { "pending" },
                        "role": a.role,
                        "xp": a.xp,
                        "title": a.title,
                        "award": a.award.as_ref().map_or(Value::Null, EventPointer::to_value),
                    })
                })
                .collect();
        }
        Card::Capability { closest, add } => {
            value["status"] = json!("missing");
            value["closest"] = closest.as_ref().map_or(Value::Null, |closest| {
                json!({
                    "name": closest.name,
                    "summary": closest.summary,
                    "reach": closest.reach.word(),
                })
            });
            value["add"] = json!(add.word());
        }
    }
    parse_card(&value)?;
    Ok(value)
}

pub mod switched;

#[cfg(test)]
mod tests;
