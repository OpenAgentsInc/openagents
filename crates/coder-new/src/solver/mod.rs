//! `coder solver N` (#11336): four constrained Claude Agent SDK sessions
//! take an issue to a pull request, each holding only the tools its hand
//! of the work needs. `docs/coder/solver.md` is the spec.
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
//! - **Tester** (Sonnet, Read, Grep, Glob, `run_check`): runs the checks and
//!   ends with `VERDICT: pass`, `VERDICT: coder: notes` or `VERDICT:
//!   briefer: notes`. The harness reruns the required checks itself and
//!   judges ([`issue_run::verdict`]); hand-backs are bounded by `--rounds`.
//!
//! The run shares `issue_run`'s transcript, cards, run folder, summary and
//! pull-request step, so the terminal draws it with the same widgets and
//! `coder solver --replay` plays a recording again.

pub mod hands;

use std::{path::PathBuf, time::Instant};

use serde_json::{Value, json};

use crate::issue_run::{
    self, Feed, Transcript,
    agent::Work,
    decide::Briefing,
    setup::{self, Base},
    verdict::{self, Outcome, Status},
};
use crate::live::Entry;

/// The mode's name, in the rail and in the conversation.
pub const LABEL: &str = "solver";

/// The repository solver works in when the person names none.
pub const DEFAULT_GITHUB: &str = "OpenAgentsInc/openagents";

/// What the conversational Intaker is told before the conversation: help
/// the person find or pick an issue with `gh`, then name it on its own line.
const INTAKE_CHAT: &str = "You are the Intaker of solver, a four-hand coding run that takes one \
GitHub issue to a pull request. Before the run starts, help the person choose the issue. You \
can call `gh` with read-only issue and pull request commands (`issue list`, `issue view`, \
`search issues`, `pr list`, `pr view`, with flags such as `--search`, `--label`, `--state`, \
`--limit`, `--json`); the repository is {repo} unless the person names another. For a project \
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
        crate::identity::prompt(crate::microclaude::MODEL),
        INTAKE_CHAT
            .replace("{repo}", github)
            .replace("{today}", &days_ago(0))
            .replace("{since}", &days_ago(RECENT_DAYS))
            .replace("{days}", &RECENT_DAYS.to_string())
    );
    format!(
        "{prefix}{}",
        crate::live::local_conversation(messages, 48 * 1024)
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
                if !unscoped && !args.iter().any(|a| a == "-R" || a == "--repo") {
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
  --github OWNER/R      The GitHub repository of the issue (default: OpenAgentsInc/openagents).
  --rounds R            How many times the Tester may hand work back (default 2).
  --no-pr               Stop after the checks; open no pull request.
  --coder-model NAME    The Coder's model on every round. Without it, a change the
                        Briefer judges small starts on sonnet and a large one, or
                        any hand-back, uses claude-opus-5-5.
  --briefer-model NAME  The Intaker's and Briefer's model (default haiku).
  --tester-model NAME   The Tester's model (default sonnet).
  --files K             Files in the briefing (default 6).
  --timeout SECS        How long each hand may work (default 1800).
  --plain               Print the run as text instead of opening the terminal screen.
  --json                With --plain: one JSON line per transcript change.
  --snapshot FILE       Also save a picture of the whole run as an SVG file.
  --replay FILE         Play a recorded run (its events.jsonl) again.
  --speed X             Play a recording X times faster (default 1).
  --state DIR           Where runs live (default ~/.openagents/coder-new/solver-runs).";

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
    pub briefer_model: String,
    pub tester_model: String,
    /// The Coder's model for a change the Briefer judged small, when no
    /// `--coder-model` was given; `None` when the Coder's model is fixed.
    /// A hand-back always goes to the full Coder model.
    pub small_coder: Option<String>,
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
        let mut briefer_model = "haiku".to_owned();
        let mut tester_model = "sonnet".to_owned();
        let mut small_coder = Some("sonnet".to_owned());
        // Claude only: the briefing's ranking is deterministic and the Haiku
        // Briefer judges it; no decision-model door is asked.
        let mut inner_args: Vec<String> = vec![
            "--state".into(),
            default_state().display().to_string(),
            "--decider".into(),
            "off".into(),
        ];
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
                "--json" => json = true,
                "--coder-model" => {
                    small_coder = None;
                    inner_args.push("--model".into());
                    inner_args.push(value("--coder-model")?);
                }
                "--briefer-model" => briefer_model = value("--briefer-model")?,
                "--tester-model" => tester_model = value("--tester-model")?,
                "--model" | "--open-pr" | "--fix" | "--decider" => {
                    return Err(format!("Unknown option {arg}. Run coder solver --help."));
                }
                other => inner_args.push(other.to_owned()),
            }
        }
        let mut inner = issue_run::Options::parse(&inner_args)
            .map_err(|why| why.replace("coder issue-run", "coder solver"))?;
        inner.open_pr = open_pr;
        Ok(Self {
            inner,
            rounds,
            json,
            briefer_model,
            tester_model,
            small_coder,
        })
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

/// `~/.openagents/coder-new/solver-runs`.
fn default_state() -> PathBuf {
    model_access::store::openagents_dir()
        .map(|root| root.join("coder-new").join("solver-runs"))
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
    std::thread::spawn(move || {
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
        rounds: 0,
        started,
    };

    // Intaker.
    let intake = hands::intake(options, folder, transcript).await;
    end.turns.push(intake.clone());
    if let Some(why) = hands::blocked(&intake) {
        return end
            .conclude(Outcome::stopped(Status::SetupFailed, why), transcript)
            .await;
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
        let tester = hands::test(options, &base, &briefing, round, folder, transcript).await;
        let verdict = hands::Verdict::parse(tester.verdict.as_deref().unwrap_or_default());
        // The Tester can't edit, so what it saw pass still holds.
        let passed: Vec<String> = tester
            .work
            .checks
            .iter()
            .filter(|(_, ok)| *ok)
            .map(|(id, _)| id.clone())
            .collect();
        end.turns.push(tester);
        if transcript.cancelled() {
            break Outcome::stopped(Status::Cancelled, "The run was stopped.");
        }
        // The change decides what must pass: every crate it touched builds
        // (and a browser build's wasm half), its Markdown's links resolve,
        // plus the briefing's own non-Rust required checks. The briefed
        // crates' checks still run, but a docs change isn't held to them.
        let mut checked = briefing.clone();
        let changed = setup::change_checks(&base.worktree, &base.base).await;
        let mut required = required.clone();
        for check in changed {
            if !required.contains(&check.id) {
                required.push(check.id.clone());
            }
            if !checked.checks.iter().any(|have| have.id == check.id) {
                checked.checks.push(check);
            }
        }
        let outputs =
            issue_run::agent::final_check_outputs(&base, &checked, &passed, transcript).await;
        let errors = hands::failing_output(&outputs);
        end.checks = outputs.into_iter().map(|(id, ok, _)| (id, ok)).collect();
        let judged = verdict::judge(transcript.cancelled(), None, &required, &end.checks);
        if judged.status == Status::Passed || judged.status == Status::Unchecked {
            break judged;
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
    rounds: usize,
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
        let url = if self.options.inner.open_pr {
            match &self.base {
                Some(base) => setup::open_pr(&self.options.inner, base, &outcome, transcript).await,
                None => {
                    transcript.notice("No pull request: the run has no working copy.");
                    None
                }
            }
        } else {
            None
        };
        let total = hands::total_cost(&self.turns);
        summary["solver"] = json!({
            "hands": self.turns.iter().map(Turn::json).collect::<Vec<_>>(),
            "rounds": self.rounds,
            "cost_usd": total,
            "pull_request": url,
        });
        with_hands_cost(&mut summary, &self.turns);
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

#[cfg(test)]
mod tests;
