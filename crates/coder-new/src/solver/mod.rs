//! `coder solver N` (#11336): four constrained agent sessions take an
//! issue to a pull request, each holding only the tools its hand of the
//! work needs. The hands run on the Claude Agent SDK by default, or on
//! Devin over ACP with `--engine devin` (#11373). `docs/coder/solver.md`
//! is the spec.
//!
//! - **Intaker** (Haiku, `environment` and `issue` tools): checks that
//!   `git`, `gh` and the Claude Code login are present and whether the
//!   checkout is clean, reads the issue, and answers `INTAKE: ready` or
//!   `INTAKE: blocked: why`. The harness then makes the fresh worktree
//!   ([`issue_run::setup::prepare`]).
//! - **Briefer** (Haiku, Read, Grep, Glob): the existing code-brief
//!   algorithm runs first ([`issue_run::decide`]); the Briefer then reads
//!   the listed files and writes an analysis of what the change should
//!   look like, which joins the briefing's system prompt.
//! - **Coder** (Opus, Read, Edit, Write, Grep, Glob): codes only.
//! - **Tester** (Haiku, Read, Grep, Glob, `run_check`): runs the checks and
//!   ends with `VERDICT: pass`, `VERDICT: coder: notes` or `VERDICT:
//!   briefer: notes`. The harness reruns the required checks itself and
//!   judges ([`issue_run::verdict`]); hand-backs are bounded by `--rounds`.
//!
//! The run shares `issue_run`'s transcript, cards, run folder, summary and
//! pull-request step, so the terminal draws it with the same widgets and
//! `coder solver --replay` plays a recording again.

pub mod cloud;
pub mod conform;
pub mod hands;
pub mod host;
pub mod ledger;
pub mod merge;
pub mod queue;
pub mod rollup;
pub mod sense;

use std::{path::PathBuf, time::Instant};

use serde_json::{Value, json};

use crate::issue_run::{
    self, Feed, Transcript,
    agent::Work,
    decide::{Briefing, Check},
    setup::{self, Base},
    verdict::{self, Outcome, Status},
};
use crate::live::Entry;

/// The mode's name, in the rail and in the conversation.
pub const LABEL: &str = "solver";

/// What the conversational Intaker is told before the conversation: help
/// the person find or pick an issue with `gh`, then name it on its own line.
const INTAKE_CHAT: &str = "You are the Intaker of solver, a four-hand coding run that takes one \
GitHub issue to a pull request. Before the run starts, help the person choose the issue. You \
can call `gh` with read-only issue and pull request commands (`issue list`, `issue view`, \
`search issues`, `pr list`, `pr view`, with flags such as `--search`, `--label`, `--state`, \
`--limit`, `--json`); {repo} For a project \
board (a URL like github.com/orgs/OWNER/projects/N), read its cards with `project item-list N \
--owner OWNER --format json --limit 200`; each card has the issue's number, title and the \
board's Status, so pick from the cards whose Status isn't Done. Search and \
read issues to answer what they ask, suggest one or two that fit (say why in a line each), and \
ask when you need to know more. Keep replies short. When the person has chosen, or clearly \
asked you to pick, end your reply with exactly one line `ISSUE: owner/repo#N` and nothing \
after it; the run starts from that line. Never write that line before they have chosen. \
Today is {today}. Pick only recent issues: lists and searches return issues created on or \
after {since} (the last {days} days); older ones describe code that has since moved. Choose \
an older issue only when the person names its number.";

/// How far back the Intaker's lists and searches reach unless asked otherwise.
pub const RECENT_DAYS: u64 = 21;

/// The prompt of one Intaker turn: its instructions, then the conversation.
#[must_use]
pub fn intake_task(messages: &[openrouter::Message], github: &str) -> String {
    let prefix = format!(
        "{}\n{}\nThe conversation:\n",
        crate::identity::prompt(crate::claude::MODEL),
        INTAKE_CHAT
            .replace("{repo}", &repo_line(github))
            .replace("{today}", &days_ago(0))
            .replace("{since}", &days_ago(RECENT_DAYS))
            .replace("{days}", &RECENT_DAYS.to_string())
    );
    format!(
        "{prefix}{}",
        crate::live::local_conversation(messages, 48 * 1024)
    )
}

/// What the Intaker is told about the repository: the chat folder's
/// GitHub repository, or (`github` empty) that it has none, so the person
/// is asked which one (#11536).
fn repo_line(github: &str) -> String {
    if github.is_empty() {
        "this folder has no GitHub remote, so before searching ask in one line which repository \
(owner/repo) the issue is in, and name it in every command."
            .to_owned()
    } else {
        format!("the repository is {github} unless the person names another.")
    }
}

/// [`intake_task`] for the Intaker on the Coder loop (#11518): its `gh`
/// is the Run tool, read-only. `github` is empty when the chat's folder
/// has no GitHub remote.
#[must_use]
pub fn intake_task_coder(messages: &[openrouter::Message], github: &str) -> String {
    let example = if github.is_empty() {
        "OWNER/REPO"
    } else {
        github
    };
    format!(
        "{}\n\nCall `gh` through the Run tool (for example `gh issue list -R {example} --search ...`), with read-only commands only; this session cannot change anything.",
        intake_task(messages, github)
    )
}

/// The issue an Intaker reply settles on: its last `ISSUE:` line.
#[must_use]
pub fn chosen(reply: &str) -> Option<(Option<String>, u64)> {
    reply
        .lines()
        .rev()
        .find_map(|line| line.trim().strip_prefix("ISSUE:"))
        .and_then(issue_reference)
}

/// The UTC date `days` days before today, as `YYYY-MM-DD`.
#[must_use]
pub fn days_ago(days: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    civil_date(now / 86_400 - days.min(now / 86_400))
}

/// The `YYYY-MM-DD` date of a day count since 1970-01-01 (Hinnant's algorithm).
#[must_use]
pub fn civil_date(days: u64) -> String {
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// `args` held to issues created on or after `since`: an issue or pull
/// request list gains a `created:>=` qualifier in its search, a search gains
/// `--created`, unless the command already names a date or one issue.
#[must_use]
pub fn recent_only(mut args: Vec<String>, since: &str) -> Vec<String> {
    let dated = args.iter().any(|a| {
        a.contains("created:")
            || a.contains("updated:")
            || a.contains("closed:")
            || a.starts_with("--created")
            || a.starts_with("--updated")
            || a.starts_with("--closed")
    });
    if dated {
        return args;
    }
    let qualifier = format!("created:>={since}");
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["issue" | "pr", "list", ..] => {
            if let Some(at) = args.iter().position(|a| a == "--search" || a == "-S")
                && let Some(query) = args.get_mut(at + 1)
            {
                *query = format!("{query} {qualifier}");
            } else if let Some(at) = args.iter().position(|a| a.starts_with("--search=")) {
                args[at] = format!("{} {qualifier}", args[at]);
            } else {
                args.push("--search".into());
                args.push(qualifier);
            }
        }
        ["search", "issues" | "prs", ..] => {
            args.push(format!("--created=>={since}"));
        }
        _ => {}
    }
    args
}

/// Whether `args` is a read-only `gh` issue or pull request command.
#[must_use]
pub fn gh_read_only(args: &[String]) -> bool {
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    matches!(
        words.as_slice(),
        ["issue" | "pr", "list" | "view" | "status", ..]
            | ["search", "issues" | "prs", ..]
            | ["project", "list" | "view" | "item-list" | "field-list", ..]
    ) && !words.iter().any(|w| matches!(*w, "--web" | "-w"))
}

/// The conversational Intaker's one tool: `gh`, read-only.
#[must_use]
pub fn intake_chat_server(github: &str) -> claude_agent_sdk::SdkMcpServer {
    let github = github.to_owned();
    claude_agent_sdk::SdkMcpServer::new("intake", "1.0.0").tool(
        "gh",
        "Run a read-only gh command: issue list|view|status, pr list|view|status, search issues|prs, project list|view|item-list|field-list. Pass the words after `gh` as `args`.",
        json!({"type": "object", "properties": {"args": {"type": "array", "items": {"type": "string"}}}, "required": ["args"]}),
        move |input: Value| {
            let github = github.clone();
            async move {
                let args: Vec<String> = input["args"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect();
                if !gh_read_only(&args) {
                    return Ok(claude_agent_sdk::ToolResult::error(
                        "Only gh issue/pr list, view, status and search issues/prs are allowed here.",
                    ));
                }
                let mut args = recent_only(args, &days_ago(RECENT_DAYS));
                // Searches and project boards take no repository flag.
                let unscoped = matches!(args.first().map(String::as_str), Some("search" | "project"));
                if !unscoped
                    && !github.is_empty()
                    && !args.iter().any(|a| a == "-R" || a == "--repo")
                {
                    args.push("-R".into());
                    args.push(github);
                }
                let words: Vec<&str> = args.iter().map(String::as_str).collect();
                Ok(
                    match crate::issue_run::setup::output("gh", &words, std::path::Path::new("."))
                        .await
                    {
                        Ok(text) => claude_agent_sdk::ToolResult::text(text),
                        Err(why) => claude_agent_sdk::ToolResult::error(why),
                    },
                )
            }
        },
    )
}

pub const USAGE: &str = "Usage: coder solver N [OPTIONS]

Solves issue N to a pull request in four hands, each a Claude agent with
only the tools it needs: the Intaker checks the environment and reads the
issue, the Briefer runs the code-brief algorithm and writes what the change
should look like, the Coder (Opus) codes, and the Tester runs the checks and
hands back to the Coder or the Briefer when they fail. When the checks pass
and the issue is open, the run opens a pull request.

Options:
  --repo PATH           The repository checkout to read (default: this directory).
  --github OWNER/R      The GitHub repository of the issue (default: the repository's
                        GitHub remote, origin first; asked for when it has none).
  --engine NAME         The agent each hand runs on: claude (four Claude Code
                        sessions), opencode (four OpenCode sessions under
                        Coder's plugin), coder (four Coder-loop sessions on the
                        OpenAgents gateway) or devin (four `devin acp` sessions
                        on this computer's Devin login). Default: claude while
                        the Claude Code plugin is on and signed in, then
                        opencode while it is installed and its plugin is on,
                        otherwise coder.
  --rounds R            How many times the Tester may hand work back (default 2).
  --decisions DOOR      Which decision door judges the run: the gate before any
                        hand works, the briefing's file relevance and change
                        kind, what made a failed check fail, and each hunk of
                        the change (#11484). gateway (default: our hosted Clef
                        on openagents.com's H100 and A100, keyless and free;
                        with it the briefing weighs 60 files, not 12), jev
                        (TypeSafe's API), clef (this computer's Clef), haiku-arm
                        (a constrained Claude Haiku answering the same typed
                        question, no tools), subq (SubQ Snap, SUBQ_API_KEY; a
                        public preview), or off (the ranking stands, the Haiku
                        Briefer judges it, and the Tester reads every failure).
                        No other door is asked. Or one door per point:
                        kind=jev,relevance=clef.
  --no-gate             Work the issue even when the door judges it already
                        landed, no longer current, or not actionable.
  --no-review           Open the pull request without asking about each hunk.
  --no-pr               Stop after the checks; open no pull request.
  --coder-model NAME    The Coder's model on every round. Without it, a change the
                        Briefer judges small starts on sonnet and a large one, or
                        any hand-back, uses claude-opus-5-5 (with --engine devin,
                        Devin's own model).
  --briefer-model NAME  The Intaker's and Briefer's model (default haiku; Devin's
                        own model with --engine devin).
  --tester-model NAME   The Tester's model (default haiku; Devin's own model with
                        --engine devin).
  --files K             Files in the briefing (default 6).
  --timeout SECS        How long each hand may work (default 1800).
  --cloud               Run on a Google Cloud box of its own instead of this computer,
                        every hand on Gemini 4 Argon, as your OpenAgents account;
                        prints the progress and the pull request (with --json, as
                        JSON lines; with --detach, only the run's page).
  --plain               Print the run as text instead of opening the terminal screen.
  --json                With --plain: one JSON line per transcript change.
  --snapshot FILE       Also save a picture of the whole run as an SVG file.
  --replay FILE         Play a recorded run (its events.jsonl) again.
  --speed X             Play a recording X times faster (default 1).
  --state DIR           Where runs live (default ~/.openagents/coder/solver-runs).

coder solver watch [--since REV] [--decisions DOOR] [--repo PATH]
  For each commit on origin/main since the last watch, asks whether it did
  the work of an open issue whose text names a file or crate it touched.
  One JSON line per commit and issue asked; likely_landed at p >= 0.8.

coder solver ledger [--state DIR] [--repo PATH]
  One JSON line per finished run: issue, status, pull request, files
  touched, the main commit that landed it, and the unlanded runs that
  touch the same files.";

/// The agent the hands run on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Engine {
    /// Four `claude_agent_sdk` sessions on this computer's Claude Code
    /// login.
    #[default]
    Claude,
    /// Four `devin acp` sessions on this computer's Devin login (#11373).
    Devin,
    /// Four Coder-loop sessions on the OpenAgents gateway, held to the
    /// run's worktree by the same fence and floors as any Coder chat
    /// (#11518).
    Coder,
    /// Four `opencode run` sessions under Coder's OpenCode plugin, held to
    /// the run's worktree by the same fence, floors and worktree guard
    /// (#11533).
    OpenCode,
}

impl Engine {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Devin => "devin",
            Self::Coder => "coder",
            Self::OpenCode => "opencode",
        }
    }

    /// The engine a run takes when none is named, in the harness order
    /// every Coder surface uses (#11533): Claude Code while its plugin is
    /// on and this computer is signed in to it, then OpenCode while its
    /// plugin is on, it is installed and its free model isn't rate
    /// limited, otherwise the Coder loop on the gateway (#11518).
    #[must_use]
    pub fn default_here() -> Self {
        // Tests never ask this computer; they keep the long-standing default.
        if cfg!(test) {
            return Self::Claude;
        }
        let on = |plugin: &str| {
            crate::home::data_dir()
                .is_none_or(|dir| crate::bundled_settings::saved_enabled(&dir, plugin))
        };
        Self::pick(
            on(crate::claude::PLUGIN) && crate::claude::available(),
            Self::opencode_here(on(crate::opencode_chat::PLUGIN)),
        )
    }

    /// The order itself: Claude Code, OpenCode, the Coder loop.
    #[must_use]
    pub const fn pick(claude: bool, opencode: bool) -> Self {
        if claude {
            Self::Claude
        } else if opencode {
            Self::OpenCode
        } else {
            Self::Coder
        }
    }

    /// Whether OpenCode can take a run here: its plugin `on`, installed,
    /// and its free model not rate limited.
    #[must_use]
    pub fn opencode_here(on: bool) -> bool {
        on && crate::opencode_chat::installed() && !crate::opencode_chat::rate_limited()
    }

    /// Whether the hands read their checks and tools from the prompt (an
    /// engine that can't host Coder's in-process `run_check`).
    #[must_use]
    pub const fn prompted(self) -> bool {
        matches!(self, Self::Devin | Self::Coder | Self::OpenCode)
    }
}

/// Which hand works.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hand {
    Intaker,
    Briefer,
    Coder,
    Tester,
}

impl Hand {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Intaker => "intaker",
            Self::Briefer => "briefer",
            Self::Coder => "coder",
            Self::Tester => "tester",
        }
    }
}

/// The solver's settings: `issue_run`'s, which the shared steps read, plus
/// the hands' models and the hand-back bound.
#[derive(Clone, Debug)]
pub struct Options {
    pub inner: issue_run::Options,
    pub rounds: usize,
    pub json: bool,
    /// The agent the hands run on.
    pub engine: Engine,
    pub briefer_model: String,
    pub tester_model: String,
    /// The Coder's model for a change the Briefer judged small, when no
    /// `--coder-model` was given; `None` when the Coder's model is fixed.
    /// A hand-back always goes to the full Coder model.
    pub small_coder: Option<String>,
    /// Stop before any hand when the door clearly judges the issue done,
    /// stale or blocked (#11484).
    pub gate: bool,
    /// Ask about each hunk once the checks pass (#11484).
    pub review: bool,
}

impl Options {
    /// Reads `coder solver`'s arguments.
    ///
    /// # Errors
    /// An option is unknown, lacks its value, or the value is not valid.
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let mut rounds = 2;
        let mut json = false;
        let mut open_pr = true;
        let mut engine = Engine::default_here();
        let mut briefer_model: Option<String> = None;
        let mut tester_model: Option<String> = None;
        let mut small_coder = Some("sonnet".to_owned());
        // By default our hosted Clef pool judges the run (Clef first,
        // keyless and free, #11484); --decisions names one door and only
        // that door.
        let mut decisions = "gateway".to_owned();
        let mut gate = true;
        let mut review = true;
        let mut inner_args: Vec<String> =
            vec!["--state".into(), default_state().display().to_string()];
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            let mut value = |name: &str| {
                args.next()
                    .cloned()
                    .ok_or(format!("{name} needs a value. Run coder solver --help."))
            };
            match arg.as_str() {
                "--rounds" => {
                    rounds = value("--rounds")?
                        .parse::<usize>()
                        .map_err(|_| "--rounds takes a number.".to_owned())?
                        .min(10);
                }
                "--no-pr" => open_pr = false,
                "--no-gate" => gate = false,
                "--no-review" => review = false,
                "--json" => json = true,
                "--decisions" => {
                    decisions = value("--decisions")?;
                    let (all, kind, relevance) = issue_run::parse_deciders(&decisions)
                        .map_err(|why| why.replace("--decider", "--decisions"))?;
                    if [all, kind, relevance].contains(&Some(issue_run::Decider::Auto)) {
                        return Err(
                            "--decisions takes gateway, jev, clef, haiku-arm, subq or off, not auto."
                                .into(),
                        );
                    }
                }
                "--engine" => {
                    engine = match value("--engine")?.as_str() {
                        "claude" => Engine::Claude,
                        "devin" => Engine::Devin,
                        "coder" => Engine::Coder,
                        "opencode" => Engine::OpenCode,
                        other => {
                            return Err(format!(
                                "--engine takes claude, opencode, coder or devin, not {other}."
                            ));
                        }
                    };
                }
                "--coder-model" => {
                    small_coder = None;
                    inner_args.push("--model".into());
                    inner_args.push(value("--coder-model")?);
                }
                "--briefer-model" => briefer_model = Some(value("--briefer-model")?),
                "--tester-model" => tester_model = Some(value("--tester-model")?),
                "--model" | "--open-pr" | "--fix" | "--decider" => {
                    return Err(format!("Unknown option {arg}. Run coder solver --help."));
                }
                other => inner_args.push(other.to_owned()),
            }
        }
        inner_args.push("--decider".into());
        inner_args.push(decisions);
        let mut inner = issue_run::Options::parse(&inner_args)
            .map_err(|why| why.replace("coder issue-run", "coder solver"))?;
        inner.open_pr = open_pr;
        inner.engine = engine;
        // On the Coder engine every hand runs the gateway's own router; a
        // model flag still fixes the hand it names.
        if engine == Engine::Coder && small_coder.is_some() {
            inner.model = crate::models::GATEWAY_AUTO.to_owned();
            small_coder = Some(crate::models::GATEWAY_AUTO.to_owned());
        }
        // On the Devin engine every hand's model defaults to the Devin
        // CLI's own; a model flag still fixes the hand it names.
        if engine == Engine::Devin && small_coder.is_some() {
            inner.model = acp_client::devin::DEFAULT_MODEL.to_owned();
            small_coder = Some(acp_client::devin::DEFAULT_MODEL.to_owned());
        }
        // On the OpenCode engine every hand runs the person's OpenCode
        // model; a model flag still fixes the hand it names.
        if engine == Engine::OpenCode && small_coder.is_some() {
            inner.model = crate::opencode_chat::model(None);
            small_coder = Some(inner.model.clone());
        }
        let model_default = |given: Option<String>| {
            given.unwrap_or_else(|| {
                if engine == Engine::Devin {
                    acp_client::devin::DEFAULT_MODEL.to_owned()
                } else if engine == Engine::Coder {
                    crate::models::GATEWAY_AUTO.to_owned()
                } else if engine == Engine::OpenCode {
                    crate::opencode_chat::model(None)
                } else {
                    "haiku".to_owned()
                }
            })
        };
        Ok(Self {
            inner,
            rounds,
            json,
            engine,
            briefer_model: model_default(briefer_model),
            tester_model: model_default(tester_model),
            small_coder,
            gate,
            review,
        })
    }
}

impl Options {
    /// Moves the run to OpenCode (#11533): every hand on the person's
    /// OpenCode model.
    pub fn use_opencode(&mut self) {
        let model = crate::opencode_chat::model(None);
        self.engine = Engine::OpenCode;
        self.inner.engine = Engine::OpenCode;
        self.inner.model.clone_from(&model);
        self.small_coder = Some(model.clone());
        self.briefer_model.clone_from(&model);
        self.tester_model = model;
    }

    /// Moves the run to the Coder loop on the gateway (#11518): every hand
    /// on the gateway's own router.
    pub fn use_coder(&mut self) {
        let auto = crate::models::GATEWAY_AUTO.to_owned();
        self.engine = Engine::Coder;
        self.inner.engine = Engine::Coder;
        self.inner.model.clone_from(&auto);
        self.small_coder = Some(auto.clone());
        self.briefer_model.clone_from(&auto);
        self.tester_model = auto;
    }
}

/// The Coder's model for `round`: the small model for a change the Briefer
/// judged small on the first try, the full model otherwise and after any
/// hand-back.
#[must_use]
pub fn coder_model(options: &Options, size: hands::Size, round: usize) -> String {
    match &options.small_coder {
        Some(small) if size == hands::Size::Small && round == 0 => small.clone(),
        _ => options.inner.model.clone(),
    }
}

/// `~/.openagents/coder/solver-runs`.
fn default_state() -> PathBuf {
    crate::home::data_path("solver-runs")
        .unwrap_or_else(|| std::env::temp_dir().join("coder-solver-runs"))
}

/// An issue the solver conversation was given: `N`, `#N`, or
/// `owner/repo#N`. `None` when the text names none.
#[must_use]
pub fn issue_reference(text: &str) -> Option<(Option<String>, u64)> {
    let text = text.trim();
    if let Some((repo, number)) = text.split_once('#') {
        let number = number.trim().parse::<u64>().ok()?;
        let repo = repo.trim();
        if repo.is_empty() {
            return Some((None, number));
        }
        let (owner, name) = repo.split_once('/')?;
        if owner.is_empty() || name.is_empty() || name.contains('/') {
            return None;
        }
        return Some((Some(repo.to_owned()), number));
    }
    text.parse::<u64>().ok().map(|number| (None, number))
}

/// Starts a run and returns the feed the terminal draws from.
#[must_use]
pub fn start(options: Options) -> Feed {
    let replay = options.inner.replay.clone();
    let speed = options.inner.speed;
    let made = if replay.is_some() {
        Err(String::new())
    } else {
        issue_run::new_run(&options.inner)
    };
    let log = made
        .as_ref()
        .ok()
        .and_then(|run| std::fs::File::create(run.folder.join("events.jsonl")).ok());
    let (feed, mut transcript) = issue_run::channel(log);
    // A Coder-loop hand's turn is deep; give the run room (#11518).
    // Without a thread the closure, and the feed's sender with it, is
    // dropped, so the feed ends as a stopped run's does.
    let _ = std::thread::Builder::new()
        .stack_size(issue_run::RUN_STACK)
        .spawn(move || {
            if let Some(replay) = replay {
                transcript.replay(&replay, speed);
                return;
            }
            let run = match made {
                Ok(run) => run,
                Err(why) => {
                    transcript.notice(format!("The run could not start: {why}"));
                    return;
                }
            };
            transcript.busy(true);
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build();
            match runtime {
                Ok(runtime) => runtime.block_on(drive(&options, &run, &mut transcript)),
                Err(_) => transcript.notice("The run could not start its worker threads."),
            }
            transcript.busy(false);
        });
    feed
}

/// What one hand's session did, for the summary.
#[derive(Clone, Debug)]
pub struct Turn {
    pub hand: Hand,
    pub round: usize,
    pub work: Work,
    /// The Tester's verdict, the Intaker's intake, or nothing.
    pub verdict: Option<String>,
}

impl Turn {
    fn json(&self) -> Value {
        json!({
            "hand": self.hand.as_str(),
            "round": self.round,
            "wall_ms": self.work.wall_ms,
            "turns": self.work.turns,
            "cost_usd": self.work.cost_usd,
            "input_tokens": self.work.input_tokens,
            "output_tokens": self.work.output_tokens,
            "checks": self.work.checks,
            "error": self.work.error,
            "verdict": self.verdict,
        })
    }
}

/// The whole run: intake, briefing, then coding and testing rounds.
async fn drive(options: &Options, run: &issue_run::Run, transcript: &mut Transcript) {
    transcript.push(Entry::User(format!(
        "Solve issue #{} in {} to a pull request.",
        options.inner.issue, options.inner.github
    )));
    let started = Instant::now();
    let folder = run.folder.as_path();
    let mut end = End {
        options,
        run,
        base: None,
        briefing: None,
        turns: Vec::new(),
        checks: Vec::new(),
        required: None,
        rounds: 0,
        decisions: decisions_value(
            options.inner.kind_decider(),
            options.inner.relevance_decider(),
        ),
        verdict: None,
        judgments: Vec::new(),
        started,
    };

    // Intaker.
    let intake = hands::intake_in_code(options, transcript).await;
    end.turns.push(intake.clone());
    if let Some(why) = hands::blocked(&intake) {
        return end
            .conclude(Outcome::stopped(Status::SetupFailed, why), transcript)
            .await;
    }
    if let Some(commit) = hands::landed_commit(&options.inner.repo, options.inner.issue).await {
        return end
            .conclude(
                Outcome::stopped(
                    Status::AlreadyDone,
                    format!("Main already has a commit for this issue: {commit}"),
                ),
                transcript,
            )
            .await;
    }
    end.verdict = queue::verdict(
        &options.inner.github,
        &options.inner.repo,
        folder.parent().unwrap_or(folder),
        options.inner.issue,
        &run.id,
        options.inner.kind_decider(),
    )
    .await;
    if let Some(entry) = &end.verdict {
        end.judgments.extend(sense::gate_rows(entry));
        if let Some(outcome) = sense::gate(entry).filter(|_| options.gate) {
            return end.conclude(outcome, transcript).await;
        }
    }
    if let Some(entry) = end
        .verdict
        .as_ref()
        .filter(|entry| entry.verdict == "in_flight")
    {
        transcript.notice(format!(
            "Another run already works this issue: {}.",
            entry.why
        ));
    }
    let Some(base) = setup::prepare(&options.inner, folder, transcript).await else {
        let outcome = if transcript.cancelled() {
            Outcome::stopped(Status::Cancelled, "The run was stopped during intake.")
        } else {
            Outcome::stopped(
                Status::SetupFailed,
                "The issue, its base commit, or the worktree could not be prepared.",
            )
        };
        return end.conclude(outcome, transcript).await;
    };
    end.base = Some(base.clone());

    // Briefer.
    let Some(mut briefing) =
        issue_run::decide::decide(&options.inner, &base, folder, transcript).await
    else {
        let outcome = if transcript.cancelled() {
            Outcome::stopped(Status::Cancelled, "The run was stopped in the briefing.")
        } else {
            Outcome::stopped(
                Status::DecisionFailed,
                "The code-brief steps stopped before a briefing existed.",
            )
        };
        return end.conclude(outcome, transcript).await;
    };
    let mut notes: Option<String> = None;
    let briefer = hands::brief(
        options,
        &base,
        &mut briefing,
        notes.as_deref(),
        0,
        folder,
        transcript,
    )
    .await;
    let size = hands::Size::parse(briefer.work.reply.as_deref().unwrap_or_default());
    end.turns.push(briefer);
    // A browser-build crate's wasm32 half compiles only for that target.
    let wasm = setup::briefed_wasm_checks(&base.worktree, &briefing);
    briefing.checks.extend(wasm);
    end.briefing = Some(briefing.clone());

    // Coder and Tester, with bounded hand-backs.
    let required: Vec<String> = briefing
        .checks
        .iter()
        .filter(|check| check.required && !check.id.starts_with("wasm:"))
        .map(|check| check.id.clone())
        .collect();
    let mut round = 0;
    let mut reviewed = !options.review;
    let mut hand_backs: Vec<hands::HandBack> = Vec::new();
    let outcome = loop {
        if transcript.cancelled() {
            break Outcome::stopped(Status::Cancelled, "The run was stopped.");
        }
        end.rounds = round;
        let model = coder_model(options, size, round);
        let coder = hands::code(
            options,
            &base,
            &briefing,
            notes.as_deref(),
            &model,
            round,
            folder,
            transcript,
        )
        .await;
        let coder_error = coder.work.error.clone();
        end.turns.push(coder);
        if let Some(error) = coder_error {
            break Outcome::stopped(Status::Failed, format!("The Coder stopped: {error}"));
        }
        // The harness runs the required checks first: every crate the change
        // touched builds (and a browser build's wasm half), its Markdown's
        // links resolve, plus the briefing's own checks. A clean run ends
        // here, with no Tester session to pay for; the Tester reads failures.
        let mut checked = briefing.clone();
        let changed = setup::change_checks(&base.worktree, &base.base).await;
        let mut required = required.clone();
        merge_change_checks(&mut checked.checks, &mut required, changed);
        end.required = Some(required.clone());
        let outputs = issue_run::agent::final_check_outputs(&base, &checked, &[], transcript).await;
        let errors = hands::failing_output(&outputs);
        end.checks = outputs
            .iter()
            .map(|(id, ok, _)| (id.clone(), *ok))
            .collect();
        let judged = verdict::judge(transcript.cancelled(), None, &required, &end.checks);
        if judged.status == Status::Passed || judged.status == Status::Unchecked {
            // One review of the change's hunks before it is delivered: a
            // clear stray hunk sends the Coder one round back naming it.
            if !reviewed && round < options.rounds {
                reviewed = true;
                let diff = setup::working_diff(&base.worktree, &base.base);
                let answers = sense::review(
                    options.inner.kind_decider(),
                    &base,
                    &diff,
                    &mut end.judgments,
                    transcript,
                )
                .await;
                if let Some(text) = sense::review_notes(&answers) {
                    round += 1;
                    transcript.notice(format!("Round {round}: back to the Coder. {text}"));
                    notes = Some(text);
                    continue;
                }
            }
            break judged;
        }
        if transcript.cancelled() {
            break Outcome::stopped(Status::Cancelled, "The run was stopped.");
        }
        if let Some(fault) = hands::environment_fault(&outputs) {
            break Outcome::stopped(
                Status::Failed,
                format!(
                    "The computer, not the change, failed a check: {fault} Fix it and run again."
                ),
            );
        }
        // The door reads each failure first; when it is sure of every one,
        // the hand-back needs no Tester session.
        let triaged = sense::triage(
            options.inner.kind_decider(),
            &base,
            &briefing,
            &outputs,
            &mut end.judgments,
            transcript,
        )
        .await;
        let failing = outputs.iter().filter(|(_, ok, _)| !ok).count();
        let verdict = match sense::route(&triaged, failing) {
            Some(sense::Route::Environment(why)) => {
                break Outcome::stopped(Status::Failed, format!("{why} Fix it and run again."));
            }
            Some(sense::Route::Coder(text)) => {
                transcript.notice(text.clone());
                hands::Verdict::Coder(text)
            }
            Some(sense::Route::Briefer(text)) => {
                transcript.notice(text.clone());
                hands::Verdict::Briefer(text)
            }
            None => {
                // The Tester may rerun any check the harness ran: those the
                // change's crates call for, not only the briefing's.
                let tester =
                    hands::test(options, &base, &checked, &errors, round, folder, transcript).await;
                let verdict = hands::Verdict::parse(tester.verdict.as_deref().unwrap_or_default());
                end.turns.push(tester);
                verdict
            }
        };
        if transcript.cancelled() {
            break Outcome::stopped(Status::Cancelled, "The run was stopped.");
        }
        let failed: Vec<String> = end
            .checks
            .iter()
            .filter(|(_, ok)| !ok)
            .map(|(id, _)| id.clone())
            .collect();
        let back = match verdict {
            hands::Verdict::Pass | hands::Verdict::Coder(_) if failed.is_empty() => {
                hands::Verdict::Coder(judged.reason.clone().unwrap_or_default())
            }
            hands::Verdict::Pass => hands::Verdict::Coder(format!(
                "The harness's final checks failed: {}.",
                failed.join(", ")
            )),
            other => other,
        };
        if let Some(record) = hands::HandBack::new(&back, failed.clone(), &errors) {
            transcript.push(Entry::Tool {
                name: "hand_back".into(),
                input: record.json(round),
                output: Value::Null,
                running: false,
            });
            if hand_backs
                .last()
                .is_some_and(|earlier| record.repeats(earlier))
            {
                break Outcome::stopped(
                    Status::Failed,
                    format!(
                        "The same checks ({}) failed twice with the same diagnosis, so another round would repeat it: {}",
                        record.failing_checks.join(", "),
                        record.hypothesis
                    ),
                );
            }
            hand_backs.push(record);
        }
        // The next hand sees the errors themselves, not only the check's name.
        let back = match back {
            hands::Verdict::Coder(text) => hands::Verdict::Coder(format!("{text}{errors}")),
            hands::Verdict::Briefer(text) => hands::Verdict::Briefer(format!("{text}{errors}")),
            hands::Verdict::Pass => hands::Verdict::Pass,
        };
        if round >= options.rounds {
            break Outcome::stopped(
                Status::Failed,
                format!(
                    "{} after {} hand-back{}.",
                    judged.reason.unwrap_or_default().trim_end_matches('.'),
                    options.rounds,
                    if options.rounds == 1 { "" } else { "s" }
                ),
            );
        }
        round += 1;
        match back {
            hands::Verdict::Briefer(text) => {
                transcript.notice(format!("Round {round}: back to the Briefer. {text}"));
                notes = Some(text);
                let briefer = hands::brief(
                    options,
                    &base,
                    &mut briefing,
                    notes.as_deref(),
                    round,
                    folder,
                    transcript,
                )
                .await;
                end.turns.push(briefer);
                end.briefing = Some(briefing.clone());
            }
            hands::Verdict::Coder(text) => {
                transcript.notice(format!("Round {round}: back to the Coder. {text}"));
                notes = Some(text);
            }
            hands::Verdict::Pass => unreachable!("a pass never hands back"),
        }
    };
    end.conclude(outcome, transcript).await;
}

/// The summary's cost covers every hand, not only the last Coder's session:
/// the Intaker, Briefer and Tester are Claude sessions too, and the footer
/// and the run list read `cost.total_usd`.
pub fn with_hands_cost(summary: &mut Value, turns: &[Turn]) {
    let Some(total) = hands::total_cost(turns) else {
        return;
    };
    let decisions = summary["cost"]["components"]["decisions"]["usd"]
        .as_f64()
        .unwrap_or(0.0);
    let hands: serde_json::Map<String, Value> =
        [Hand::Intaker, Hand::Briefer, Hand::Coder, Hand::Tester]
            .iter()
            .map(|hand| {
                let usd: f64 = turns
                    .iter()
                    .filter(|turn| turn.hand == *hand)
                    .filter_map(|turn| turn.work.cost_usd)
                    .sum();
                (
                    hand.as_str().to_owned(),
                    json!({"usd": usd, "unknown_reason": null}),
                )
            })
            .collect();
    summary["agent_usd"] = json!(total);
    summary["cost"]["components"]["agent"] = json!({"usd": total, "unknown_reason": null});
    summary["cost"]["components"]["hands"] = Value::Object(hands);
    summary["cost"]["known_subtotal_usd"] = json!(total + decisions);
    summary["cost"]["total_usd"] = json!(total + decisions);
}

/// What a run has when it ends.
struct End<'a> {
    options: &'a Options,
    run: &'a issue_run::Run,
    base: Option<Base>,
    briefing: Option<Briefing>,
    turns: Vec<Turn>,
    checks: Vec<(String, bool)>,
    /// The required checks of the last round's harness run: the briefing's
    /// plus every crate the change touched (#11505). `None` before any.
    required: Option<Vec<String>>,
    rounds: usize,
    /// The decision door that judged the briefing, as `--decisions` named
    /// it: one name, or `{"kind": door, "relevance": door}` when mixed.
    decisions: Value,
    /// The queue's verdict on the issue when the run started (#11388).
    verdict: Option<queue::Entry>,
    /// The run's own judgment rows beyond the briefing's: the gate, failure
    /// triage and hunk review (#11484).
    judgments: Vec<Value>,
    started: Instant,
}

impl End<'_> {
    /// Writes the summary, opens the pull request when the run passed and
    /// the options ask, and captures the run as a trace.
    async fn conclude(self, outcome: Outcome, transcript: &mut Transcript) {
        let folder = self.run.folder.as_path();
        let coder = self
            .turns
            .iter()
            .rev()
            .find(|turn| turn.hand == Hand::Coder)
            .map(|turn| turn.work.clone());
        let mut summary = setup::summary(&setup::Report {
            options: &self.options.inner,
            run: self.run,
            base: self.base.as_ref(),
            briefing: self.briefing.as_ref(),
            work: coder.as_ref(),
            checks: &self.checks,
            outcome: &outcome,
            started: self.started,
            folder,
        });
        let queue = ledger::queue_position(
            self.run.folder.parent().unwrap_or(folder),
            &self.options.inner.repo,
            &self.run.id,
            &summary["changed"],
        )
        .await;
        let delivery = if self.options.inner.open_pr {
            match &self.base {
                Some(base) => {
                    let model = coder.as_ref().and_then(|work| work.model.clone());
                    setup::delivery(
                        &self.options.inner,
                        base,
                        &outcome,
                        transcript,
                        model.as_deref(),
                    )
                    .await
                }
                None => {
                    transcript.notice("No pull request: the run has no working copy.");
                    setup::Delivery::default()
                }
            }
        } else {
            setup::Delivery::default()
        };
        let url = delivery.url;
        let total = hands::total_cost(&self.turns);
        let mut rows: Vec<Value> = self
            .briefing
            .as_ref()
            .map(|briefing| briefing.judgments.clone())
            .unwrap_or_default();
        rows.extend(self.judgments.iter().cloned());
        let judgments = sense::labeled(&rows, &summary["outcome"]["status"], &summary["changed"]);
        summary["solver"] = json!({
            "hands": self.turns.iter().map(Turn::json).collect::<Vec<_>>(),
            "rounds": self.rounds,
            "decisions": self.decisions,
            "cost_usd": total,
            "pull_request": url,
            "queue": queue,
            "judgments": judgments,
            "verdict": self.verdict.as_ref().map(queue::Entry::json),
            "merge": if delivery.status.is_empty() { "none" } else { delivery.status },
        });
        with_hands_cost(&mut summary, &self.turns);
        if let Some(required) = &self.required {
            summary["required_checks"] = json!(required);
        }
        // The reply states where the run ended and the harness's final
        // checks; the last Coder's own message, written before those checks
        // ran, may be rounds old (#11505), so it is kept beside it.
        summary["coder_reply"] = summary["reply"].take();
        summary["reply"] = json!(final_reply(&outcome, &self.checks, self.rounds));
        if self.base.is_some() && self.briefing.is_some() {
            let diff = self
                .base
                .as_ref()
                .map(|base| setup::working_diff(&base.worktree, &base.base))
                .unwrap_or_default();
            transcript.push(Entry::Tool {
                name: issue_run::SUMMARY.into(),
                input: summary.clone(),
                output: json!({"diff": diff}),
                running: false,
            });
        }
        if let Err(why) = setup::write_atomic(
            &folder.join("summary.json"),
            &serde_json::to_vec_pretty(&summary).unwrap_or_default(),
        ) {
            transcript.notice(format!("{why}. The worktree is kept."));
        }
        issue_run::capture(&self.options.inner, folder, transcript).await;
        if let Some(base) = &self.base {
            issue_run::agent::Baseline::of(base).remove().await;
        }
        issue_run::agent::sweep_shared_target().await;
        let worktree = self
            .base
            .as_ref()
            .map(|base| format!(" Working copy: {}.", base.worktree.display()))
            .unwrap_or_default();
        let pr = url
            .map(|url| format!(" Pull request: {url}."))
            .unwrap_or_default();
        transcript.notice(format!(
            "Solver ended: {}{}.{pr}{worktree} Recording: {}.",
            outcome.status.as_str(),
            outcome
                .reason
                .as_deref()
                .map(|why| format!(" ({why})"))
                .unwrap_or_default(),
            folder.join("events.jsonl").display()
        ));
    }
}

/// The run's last word: its outcome and the harness's final checks, in
/// plain words. It replaces the last Coder's message, which in #11495's run
/// asked for checks the harness had already passed.
#[must_use]
pub fn final_reply(outcome: &Outcome, checks: &[(String, bool)], rounds: usize) -> String {
    let mut reply = match outcome.status {
        Status::Passed => "The change passed".to_owned(),
        Status::Failed => "The change failed".to_owned(),
        Status::Unchecked => "The change is unchecked".to_owned(),
        other => format!("The run ended: {}", other.as_str().replace('_', " ")),
    };
    if !checks.is_empty() {
        reply.push_str(&format!(
            " after {} round{}",
            rounds + 1,
            if rounds == 0 { "" } else { "s" }
        ));
    }
    reply.push('.');
    if let Some(reason) = &outcome.reason {
        reply.push_str(&format!(" {}", reason.trim()));
        if !reply.ends_with('.') {
            reply.push('.');
        }
    }
    let ids = |ok: bool| -> Vec<&str> {
        checks
            .iter()
            .filter(|(_, passed)| *passed == ok)
            .map(|(id, _)| id.as_str())
            .collect()
    };
    let (passed, failed) = (ids(true), ids(false));
    if checks.is_empty() {
        reply.push_str(" No final check ran.");
    } else {
        if !failed.is_empty() {
            reply.push_str(&format!("\nFailed: {}.", failed.join(", ")));
        }
        if !passed.is_empty() {
            reply.push_str(&format!("\nPassed: {}.", passed.join(", ")));
        }
    }
    reply
}

/// Adds the checks the change itself calls for to `checks` and their ids to
/// `required`. A check the briefing already lists as optional (its
/// `test:<crate>` for a briefed crate) becomes required: the harness runs only
/// required checks, so leaving the briefing's copy optional meant a touched
/// crate's tests never ran and the run failed on "no result" (#11331).
fn merge_change_checks(checks: &mut Vec<Check>, required: &mut Vec<String>, changed: Vec<Check>) {
    for check in changed {
        if !required.contains(&check.id) {
            required.push(check.id.clone());
        }
        match checks.iter_mut().find(|have| have.id == check.id) {
            Some(have) => have.required |= check.required,
            None => checks.push(check),
        }
    }
}

#[cfg(test)]
mod tests;

/// The briefing's judgment rows with the run's outcome filled in, so each
/// decision can be read beside what the run came to (#11387).
#[must_use]
pub fn joined(judgments: &[Value], outcome: &Value) -> Vec<Value> {
    judgments
        .iter()
        .cloned()
        .map(|mut row| {
            row["outcome"] = outcome.clone();
            row
        })
        .collect()
}

/// `--decisions` as the summary states it: one door name when every point
/// uses the same door, else the door per point.
#[must_use]
pub fn decisions_value(kind: issue_run::Decider, relevance: issue_run::Decider) -> Value {
    if kind == relevance {
        json!(kind.name())
    } else {
        json!({"kind": kind.name(), "relevance": relevance.name()})
    }
}
