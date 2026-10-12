//! The solver's four hands, each one constrained [`agent::Session`].

use std::path::Path;
use std::time::Instant;

use claude_agent_sdk::{SdkMcpServer, ToolResult};
use serde_json::{Value, json};

use super::{Engine, Hand, Options, Turn};
use crate::issue_run::{
    Transcript,
    agent::{self, Baseline, Session, Work},
    decide::{Briefing, Check},
    setup::{self, Base},
};

/// The read-only file tools.
pub const READ_TOOLS: [&str; 3] = ["Read", "Grep", "Glob"];
/// The Coder's tools.
pub const CODE_TOOLS: [&str; 5] = ["Read", "Edit", "Write", "Grep", "Glob"];

/// The Intaker's system prompt.
const INTAKER: &str = "You are the Intaker of a four-hand coding run. Your only job is to \
confirm the run can start. Call `environment` once and `issue` once, then answer in a few \
lines: the issue's title and what it asks in one sentence, and a last line that is exactly \
`INTAKE: ready` when git and gh are present and the issue was read, or `INTAKE: blocked: <why>` \
otherwise. A dirty checkout is not a blocker: the run works in its own fresh worktree. Do not \
plan or write code.";

/// The Intaker's system prompt on the Devin engine: its `environment` and
/// `issue` tools cannot reach an ACP session, so the harness's own answers
/// arrive inside the prompt.
const INTAKER_ACP: &str = "You are the Intaker of a four-hand coding run. Your only job is to \
confirm the run can start. The environment check and the issue are below; read them, then answer \
in a few lines: the issue's title and what it asks in one sentence, and a last line that is \
exactly `INTAKE: ready` when git, gh and a signed-in Devin are present and the issue was read, \
or `INTAKE: blocked: <why>` otherwise. A dirty checkout is not a blocker: the run works in its \
own fresh worktree. Do not plan or write code.";

/// The Briefer's system prompt.
const BRIEFER: &str = "You are the Briefer of a four-hand coding run. A deterministic \
briefing lists candidate files ranked by history and search, and candidate checks; no model has \
judged them, so you do. Read the issue and the candidates, open the files that look relevant, and \
grep or glob to find a file the issue names that the list missed. Then write the analysis the \
Coder works from, in Markdown under these headings: `## Files to change` (only the files that \
must change, each with one line on what changes there; drop candidates that don't matter), \
`## Shape of the change` (the functions, types and behavior to add or alter, concretely), \
`## Tests` (which tests to add or change, or none for a docs-only change), `## Checks` (only the \
checks that exercise the files that change: a docs or link change needs no compile check). Be \
specific and short. Read the issue's comments: a handoff there may say what is already on main \
and what is left; brief only what is left. You cannot edit files. End with exactly one line: `SIZE: small` when the \
change is a few lines in one or two files and the fix is clear, or `SIZE: large` otherwise.";

/// How big the Briefer judged the change, from its `SIZE:` line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Size {
    Small,
    Large,
}

impl Size {
    /// The last `SIZE:` line of a Briefer reply; `Large` when there is none.
    #[must_use]
    pub fn parse(reply: &str) -> Self {
        match reply
            .lines()
            .rev()
            .find_map(|line| line.trim().strip_prefix("SIZE:"))
            .map(|word| word.trim().trim_matches('`').to_ascii_lowercase())
            .as_deref()
        {
            Some("small") => Self::Small,
            _ => Self::Large,
        }
    }
}

/// The Tester's system prompt.
const TESTER: &str = "You are the Tester of a four-hand coding run. The Coder has changed the \
working copy and the harness has already run the required checks; the failures are in your \
prompt. Read the failing code (and rerun a single check with `run_check` only when you need \
more of its output), then end your answer with exactly one line: `VERDICT: coder: <what to \
fix, with file paths and the failing check's words>` when the change needs work, or `VERDICT: \
briefer: <what context is missing>` when the briefing sent the Coder to the wrong place. Never \
answer `VERDICT: pass` while a check fails. You cannot edit files.";

/// The Tester's system prompt on the Devin engine: `run_check` cannot reach
/// an ACP session, so the checks' commands go in the prompt for its own
/// shell to rerun.
const TESTER_ACP: &str = "You are the Tester of a four-hand coding run. The Coder has changed \
the working copy and the harness has already run the required checks; the failures are in your \
prompt. Read the failing code (and rerun one of the listed check commands in your shell when you \
need more of its output), then end your answer with exactly one line: `VERDICT: coder: <what to \
fix, with file paths and the failing check's words>` when the change needs work, or `VERDICT: \
briefer: <what context is missing>` when the briefing sent the Coder to the wrong place. Never \
answer `VERDICT: pass` while a check fails. You cannot edit files.";

/// Whether `program` is on the `PATH`.
fn installed(program: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(program).is_file()))
}

/// The environment lines the Intaker's `environment` tool and the Devin
/// Intaker's prompt share: which of git, gh and the hand's engine are
/// installed, and whether the checkout is clean.
async fn environment_text(repo: &Path, engine: Engine) -> String {
    let mut lines = Vec::new();
    for program in ["git", "gh"] {
        let found = installed(program);
        lines.push(format!(
            "{program}: {}",
            if found { "present" } else { "missing" }
        ));
    }
    match engine {
        Engine::Claude => lines.push(format!(
            "claude: {}",
            if installed("claude") {
                "present"
            } else {
                "missing"
            }
        )),
        Engine::Devin => {
            lines.push(format!(
                "devin: {}",
                if acp_client::devin::binary(&|name| std::env::var_os(name)).is_some() {
                    "present"
                } else {
                    "missing"
                }
            ));
            lines.push(format!(
                "devin login: {}",
                if acp_client::devin::signed_in(&|name| std::env::var_os(name)) {
                    "stored"
                } else {
                    "missing"
                }
            ));
        }
    }
    match setup::output("git", &["status", "--porcelain"], repo).await {
        Ok(status) if status.trim().is_empty() => {
            lines.push("checkout: clean".into());
        }
        Ok(status) => lines.push(format!(
            "checkout: {} changed path(s), left alone",
            status.lines().count()
        )),
        Err(why) => lines.push(format!("checkout: {why}")),
    }
    lines.join("\n")
}

/// The issue text the Intaker's `issue` tool and the Devin Intaker's
/// prompt share.
async fn issue_text(github: &str, issue: u64) -> Result<String, String> {
    let number = issue.to_string();
    let args = [
        "issue",
        "view",
        &number,
        "-R",
        github,
        "--json",
        "title,state,body",
    ];
    setup::output("gh", &args, Path::new(".")).await
}

/// Checks as prompt lines for a Devin hand: an ACP session cannot host the
/// in-process `run_check` tool, so the commands it may run in its own
/// shell are listed instead. The harness's own check results stay the
/// run's deciding verdict either way.
fn check_lines(checks: &[Check]) -> String {
    let mut lines = String::new();
    for check in checks {
        lines.push_str(&format!("\n- `{}` ({})", check.argv.join(" "), check.id));
    }
    lines
}

/// Runs one hand's session and records it as a turn.
async fn turn(
    hand: Hand,
    round: usize,
    session: Session,
    worktree: &Path,
    folder: &Path,
    transcript: &mut Transcript,
) -> Turn {
    let work = agent::run(session, worktree, folder, transcript).await;
    let verdict = work.reply.clone();
    Turn {
        hand,
        round,
        work,
        verdict,
    }
}

/// The Intaker's in-process tools: `environment` and `issue`.
fn intake_server(options: &Options) -> SdkMcpServer {
    let repo = options.inner.repo.clone();
    let (github, issue) = (options.inner.github.clone(), options.inner.issue);
    SdkMcpServer::new("intake", "1.0.0")
        .tool(
            "environment",
            "Which of git, gh and claude are installed, and whether the checkout is clean.",
            json!({"type": "object", "properties": {}}),
            move |_| {
                let repo = repo.clone();
                async move {
                    Ok(ToolResult::text(
                        environment_text(&repo, Engine::Claude).await,
                    ))
                }
            },
        )
        .tool(
            "issue",
            "The issue's title, state and body.",
            json!({"type": "object", "properties": {}}),
            move |_| {
                let github = github.clone();
                async move {
                    match issue_text(&github, issue).await {
                        Ok(text) => Ok(ToolResult::text(text)),
                        Err(why) => Ok(ToolResult::error(why)),
                    }
                }
            },
        )
}

/// The Intaker: environment and issue, no files.
pub async fn intake(options: &Options, folder: &Path, transcript: &mut Transcript) -> Turn {
    let session = match options.engine {
        Engine::Claude => Session {
            title: "Intaker".into(),
            detail: "Checks the environment and reads the issue".into(),
            model: options.briefer_model.clone(),
            timeout: options.inner.timeout,
            engine: agent::Engine::Claude {
                tools: Vec::new(),
                server: Some(intake_server(options)),
            },
            checks: Vec::new(),
            system: INTAKER.into(),
            prompt: format!(
                "Confirm the run for issue #{} in {} can start.",
                options.inner.issue, options.inner.github
            ),
            briefed: Default::default(),
            events_file: "intaker-events.jsonl".into(),
            baseline: None,
        },
        Engine::Devin => {
            // An ACP session cannot reach the in-process `environment` and
            // `issue` tools, so the harness's own answers go in its prompt.
            let environment = environment_text(&options.inner.repo, Engine::Devin).await;
            let issue = issue_text(&options.inner.github, options.inner.issue)
                .await
                .unwrap_or_else(|why| format!("The issue could not be read: {why}"));
            Session {
                title: "Intaker".into(),
                detail: "Checks the environment and reads the issue".into(),
                model: options.briefer_model.clone(),
                timeout: options.inner.timeout,
                engine: agent::Engine::Acp(agent::AcpOptions::devin(&options.briefer_model, false)),
                checks: Vec::new(),
                system: INTAKER_ACP.into(),
                prompt: format!(
                    "Confirm the run for issue #{} in {} can start.\n\n# Environment\n\n{environment}\n\n# Issue\n\n{issue}",
                    options.inner.issue, options.inner.github
                ),
                briefed: Default::default(),
                events_file: "intaker-events.jsonl".into(),
                baseline: None,
            }
        }
    };
    turn(
        Hand::Intaker,
        0,
        session,
        &options.inner.repo,
        folder,
        transcript,
    )
    .await
}

/// Intake on a named issue, in code and with no model call: the
/// environment probes, the issue's state, and the claim. The turn's verdict
/// is the same `INTAKE:` line a conversational Intaker would write, so
/// [`blocked`] and the summary read it the same way.
pub async fn intake_in_code(options: &Options, transcript: &mut Transcript) -> Turn {
    let started = Instant::now();
    transcript.notice(format!(
        "Intaker: checking the environment and issue #{} in {}.",
        options.inner.issue, options.inner.github
    ));
    let verdict = match intake_blocker(options).await {
        None => "INTAKE: ready".to_string(),
        Some(why) => format!("INTAKE: blocked: {why}"),
    };
    Turn {
        hand: Hand::Intaker,
        round: 0,
        work: Work {
            wall_ms: started.elapsed().as_millis() as u64,
            reply: Some(verdict.clone()),
            ..Default::default()
        },
        verdict: Some(verdict),
    }
}

/// The commit on `origin/main` (or the checkout's own history when there
/// is no such ref) whose subject closes the issue, as "`hash` subject":
/// this repository's commits name their issue as `(#N)`. `None` when main
/// carries no such commit.
pub async fn landed_commit(repo: &Path, issue: u64) -> Option<String> {
    let grep = format!("(#{issue})");
    for start in ["origin/main", "HEAD"] {
        let args = [
            "log",
            start,
            "-1",
            "--fixed-strings",
            &format!("--grep={grep}"),
            "--format=%h %s",
        ];
        match setup::output("git", &args, repo).await {
            Ok(text) if !text.trim().is_empty() => return Some(text.trim().to_owned()),
            Ok(_) => return None,
            Err(_) => continue,
        }
    }
    None
}

/// The first reason intake in code cannot start the run: a missing
/// program, an issue that cannot be read or is closed, or a claim another
/// run holds.
async fn intake_blocker(options: &Options) -> Option<String> {
    let environment = environment_text(&options.inner.repo, options.engine).await;
    if let Some(line) = environment.lines().find(|line| line.ends_with(": missing")) {
        return Some(format!("{line}."));
    }
    let number = options.inner.issue.to_string();
    let issue = match issue_text(&options.inner.github, options.inner.issue).await {
        Ok(text) => text,
        Err(why) => return Some(format!("the issue could not be read: {why}")),
    };
    let state = serde_json::from_str::<Value>(&issue)
        .ok()
        .and_then(|issue| issue["state"].as_str().map(str::to_string));
    if state.as_deref() == Some("CLOSED") {
        return Some(format!("issue #{number} is closed."));
    }
    if installed("openagents") {
        let args = ["issue", "claim", &number];
        if let Err(why) = setup::output("openagents", &args, &options.inner.repo).await {
            let why = why.lines().last().unwrap_or_default().trim();
            return Some(format!("issue #{number} could not be claimed: {why}"));
        }
    }
    None
}

/// Why the Intaker's turn stops the run, if it does: the session failed,
/// or its last `INTAKE:` line says blocked.
#[must_use]
pub fn blocked(intake: &Turn) -> Option<String> {
    if let Some(error) = &intake.work.error {
        return Some(format!("The Intaker stopped: {error}"));
    }
    let reply = intake.verdict.as_deref().unwrap_or_default();
    let line = reply
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| line.starts_with("INTAKE:"))?;
    let rest = line["INTAKE:".len()..].trim();
    match rest.split_once(':') {
        Some((word, why)) if word.trim().eq_ignore_ascii_case("blocked") => {
            Some(format!("The Intaker found a blocker: {}", why.trim()))
        }
        _ => None,
    }
}

/// The Briefer: reads the briefing's files and writes its analysis into
/// the briefing's system prompt and `analysis.md`.
pub async fn brief(
    options: &Options,
    base: &Base,
    briefing: &mut Briefing,
    notes: Option<&str>,
    round: usize,
    folder: &Path,
    transcript: &mut Transcript,
) -> Turn {
    let mut prompt = format!(
        "Issue #{}: {}\n\n{}\n\n# Briefing\n\n{}\n\nWrite the analysis.",
        base.issue, base.title, base.body, briefing.markdown
    );
    if let Some(notes) = notes {
        prompt.push_str("\n\n# The Tester's notes from the last round\n\n");
        prompt.push_str(notes);
    }
    prompt.push_str(&so_far(base, round));
    let session = Session {
        title: "Briefer".into(),
        detail: "Reads the briefed files and writes what the change should look like".into(),
        model: options.briefer_model.clone(),
        timeout: options.inner.timeout,
        engine: match options.engine {
            Engine::Claude => agent::Engine::Claude {
                tools: READ_TOOLS.iter().map(|t| (*t).to_owned()).collect(),
                server: None,
            },
            Engine::Devin => {
                agent::Engine::Acp(agent::AcpOptions::devin(&options.briefer_model, false))
            }
        },
        checks: Vec::new(),
        system: BRIEFER.into(),
        prompt,
        briefed: briefing.files.iter().cloned().collect(),
        events_file: format!("briefer-{round}-events.jsonl"),
        baseline: None,
    };
    let turn = turn(
        Hand::Briefer,
        round,
        session,
        &base.worktree,
        folder,
        transcript,
    )
    .await;
    if let Some(analysis) = turn
        .work
        .reply
        .as_deref()
        .filter(|text| !text.trim().is_empty())
    {
        briefing.system = with_analysis(&briefing.system, analysis);
        let _ = std::fs::write(folder.join("analysis.md"), analysis);
    }
    turn
}

/// `system` with the Briefer's analysis as its last section, replacing an
/// earlier round's.
#[must_use]
pub fn with_analysis(system: &str, analysis: &str) -> String {
    const HEADING: &str = "\n\n## Briefer's analysis\n\n";
    let head = system.split(HEADING).next().unwrap_or(system);
    format!("{head}{HEADING}{}", analysis.trim())
}

/// The Coder: Opus, file tools only.
pub async fn code(
    options: &Options,
    base: &Base,
    briefing: &Briefing,
    notes: Option<&str>,
    model: &str,
    round: usize,
    folder: &Path,
    transcript: &mut Transcript,
) -> Turn {
    let mut prompt = briefing.prompt.clone();
    if let Some(notes) = notes {
        prompt.push_str(
            "\n\n# The Tester's notes\n\nThe last round's change did not pass. Fix this:\n\n",
        );
        prompt.push_str(notes);
    }
    prompt.push_str(&so_far(base, round));
    match options.engine {
        Engine::Claude => prompt.push_str(
            "\n\nYou code: change the files, and reply with what you changed. You have no shell, but `run_check` compiles the changed crates (`check:` entries, and `wasm:` for a browser build's wasm32 half); run it after your edits and fix what fails before you finish. The Tester runs the full checks and tests after you.",
        ),
        Engine::Devin => {
            prompt.push_str(
                "\n\nYou code: change the files, and reply with what you changed. After your edits, run the compile checks in the working copy yourself and fix what fails before you finish:",
            );
            prompt.push_str(&check_lines(&compile_checks(briefing)));
            prompt.push_str(
                "\nThe harness runs the full checks and tests after you; its results decide the run.",
            );
        }
    }
    let session = Session {
        title: "Coder".into(),
        detail: "Changes the working copy from the briefing and the analysis".into(),
        model: model.to_owned(),
        timeout: options.inner.timeout,
        engine: match options.engine {
            Engine::Claude => agent::Engine::Claude {
                tools: CODE_TOOLS.iter().map(|t| (*t).to_owned()).collect(),
                server: None,
            },
            Engine::Devin => agent::Engine::Acp(agent::AcpOptions::devin(model, true)),
        },
        checks: compile_checks(briefing),
        system: briefing.system.clone(),
        prompt,
        briefed: briefing.files.iter().cloned().collect(),
        events_file: format!("coder-{round}-events.jsonl"),
        baseline: match options.engine {
            Engine::Claude => Some(Baseline::of(base)),
            Engine::Devin => None,
        },
    };
    turn(
        Hand::Coder,
        round,
        session,
        &base.worktree,
        folder,
        transcript,
    )
    .await
}

/// The most diff a later round's prompt carries.
const SO_FAR_MAX: usize = 24 * 1024;

/// For a later round: the working copy's change so far against the base,
/// marked as the earlier rounds' own work, so a hand doesn't take its own
/// edit for code that was already there.
#[must_use]
pub fn so_far(base: &Base, round: usize) -> String {
    if round == 0 {
        return String::new();
    }
    let diff = setup::working_diff(&base.worktree, &base.base);
    so_far_text(&diff)
}

/// The "change so far" section for `diff`.
#[must_use]
pub fn so_far_text(diff: &str) -> String {
    if diff.trim().is_empty() {
        return "\n\n# The change so far\n\nThe earlier rounds left no change in the working copy."
            .into();
    }
    let mut shown: String = diff.chars().take(SO_FAR_MAX).collect();
    if shown.len() < diff.len() {
        shown.push_str("\n[diff cut short]");
    }
    format!(
        "\n\n# The change so far\n\nThis is the earlier rounds' own change against the base (it is not in the base; the issue is not already fixed because of it). Build on it or correct it:\n\n```diff\n{shown}\n```"
    )
}

/// The most lines of one failing check's output a hand-back carries.
const ERROR_LINES: usize = 40;

/// The failing checks' errors, bounded, as a section for a hand-back.
#[must_use]
pub fn failing_output(outputs: &[(String, bool, String)]) -> String {
    let mut out = String::new();
    for (id, _, text) in outputs.iter().filter(|(_, ok, _)| !ok) {
        let lines: Vec<&str> = text.lines().collect();
        let errors: Vec<&str> = lines
            .iter()
            .copied()
            .filter(|line| {
                line.contains("error") || line.contains("FAILED") || line.contains("panicked")
            })
            .take(ERROR_LINES)
            .collect();
        let shown = if errors.is_empty() {
            lines[lines.len().saturating_sub(ERROR_LINES)..].join("\n")
        } else {
            errors.join("\n")
        };
        out.push_str(&format!("\n\n`{id}` failed:\n```\n{shown}\n```"));
    }
    out
}

/// Words in a failed check's output that mean the computer failed, not the
/// change: a linker that died, a full disk, a killed process. Handing those
/// back to the Coder wastes rounds on a fault no edit can fix.
const ENVIRONMENT_FAULTS: [&str; 4] = [
    "No space left on device",
    "linking with `cc` failed",
    "SIGKILL",
    "signal: 9",
];

/// The first environment fault among the failed checks, as "`id`: line".
#[must_use]
pub fn environment_fault(outputs: &[(String, bool, String)]) -> Option<String> {
    outputs
        .iter()
        .filter(|(_, ok, _)| !ok)
        .find_map(|(id, _, text)| {
            text.lines()
                .find(|line| ENVIRONMENT_FAULTS.iter().any(|fault| line.contains(fault)))
                .map(|line| format!("`{id}`: {}.", line.trim().trim_end_matches('.')))
        })
}

/// The briefing's compile checks (`check:`, e.g. `cargo check -p`), the
/// only checks the Coder may run: it compiles as it works, and leaves the
/// tests to the Tester.
pub fn compile_checks(briefing: &Briefing) -> Vec<crate::issue_run::decide::Check> {
    briefing
        .checks
        .iter()
        .filter(|check| check.id.starts_with("check:") || check.id.starts_with("wasm:"))
        .cloned()
        .collect()
}

/// The Tester: read-only file tools and `run_check`, told what failed.
pub async fn test(
    options: &Options,
    base: &Base,
    briefing: &Briefing,
    failures: &str,
    round: usize,
    folder: &Path,
    transcript: &mut Transcript,
) -> Turn {
    let diff = setup::working_diff(&base.worktree, &base.base);
    let diff = diff.chars().take(60_000).collect::<String>();
    let (system, prompt) = match options.engine {
        Engine::Claude => (
            TESTER.to_owned(),
            format!(
                "Issue #{}: {}\n\nThe Coder's change:\n\n```diff\n{}\n```\n\nThe harness ran the required checks. What failed:{}\n\nGive your verdict.",
                base.issue, base.title, diff, failures
            ),
        ),
        Engine::Devin => (
            TESTER_ACP.to_owned(),
            format!(
                "Issue #{}: {}\n\nThe Coder's change:\n\n```diff\n{}\n```\n\nThe harness ran the required checks. What failed:{}\n\nRerun a check in the working copy when you need more of its output:{}\n\nGive your verdict.",
                base.issue,
                base.title,
                diff,
                failures,
                check_lines(&briefing.checks)
            ),
        ),
    };
    let session = Session {
        title: "Tester".into(),
        detail: "Runs the briefing's checks and hands back what fails".into(),
        model: options.tester_model.clone(),
        timeout: options.inner.timeout,
        engine: match options.engine {
            Engine::Claude => agent::Engine::Claude {
                tools: READ_TOOLS.iter().map(|t| (*t).to_owned()).collect(),
                server: None,
            },
            Engine::Devin => {
                agent::Engine::Acp(agent::AcpOptions::devin(&options.tester_model, false))
            }
        },
        checks: briefing.checks.clone(),
        system,
        prompt,
        briefed: briefing.files.iter().cloned().collect(),
        events_file: format!("tester-{round}-events.jsonl"),
        baseline: match options.engine {
            Engine::Claude => Some(Baseline::of(base)),
            Engine::Devin => None,
        },
    };
    turn(
        Hand::Tester,
        round,
        session,
        &base.worktree,
        folder,
        transcript,
    )
    .await
}

/// The Tester's last word.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    /// Back to the Coder with these notes.
    Coder(String),
    /// Back to the Briefer with these notes.
    Briefer(String),
}

impl Verdict {
    /// Reads the last `VERDICT:` line of a reply. A reply without one, or
    /// with a word it does not know, is a hand-back to the Coder quoting
    /// the reply, so a silent Tester never passes a change.
    #[must_use]
    pub fn parse(reply: &str) -> Self {
        let Some(line) = reply
            .lines()
            .rev()
            .map(str::trim)
            .find(|line| line.starts_with("VERDICT:"))
        else {
            return Self::Coder(format!(
                "The Tester gave no verdict. Its reply: {}",
                reply.trim()
            ));
        };
        let rest = line["VERDICT:".len()..].trim();
        let (word, notes) = rest
            .split_once(':')
            .map_or((rest, ""), |(word, notes)| (word.trim(), notes.trim()));
        match word.to_ascii_lowercase().as_str() {
            "pass" => Self::Pass,
            "briefer" => Self::Briefer(notes.to_owned()),
            "coder" => Self::Coder(notes.to_owned()),
            _ => Self::Coder(format!("The Tester said: {rest}")),
        }
    }
}

/// One hand-back between hands, as a record rather than prose: who gets
/// it, which checks failed, the error lines the Tester read, and the
/// Tester's own hypothesis of what to fix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HandBack {
    pub target: Hand,
    pub failing_checks: Vec<String>,
    pub evidence_lines: Vec<String>,
    pub hypothesis: String,
}

impl HandBack {
    #[must_use]
    pub fn new(verdict: &Verdict, failing_checks: Vec<String>, errors: &str) -> Option<Self> {
        let (target, hypothesis) = match verdict {
            Verdict::Pass => return None,
            Verdict::Coder(text) => (Hand::Coder, text),
            Verdict::Briefer(text) => (Hand::Briefer, text),
        };
        Some(Self {
            target,
            failing_checks,
            evidence_lines: errors
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_owned)
                .collect(),
            hypothesis: hypothesis.trim().to_owned(),
        })
    }

    /// The record as it lands in `events.jsonl` and the trace.
    #[must_use]
    pub fn json(&self, round: usize) -> Value {
        json!({
            "round": round,
            "target": self.target.as_str(),
            "failing_checks": self.failing_checks,
            "evidence_lines": self.evidence_lines,
            "hypothesis": self.hypothesis,
        })
    }

    /// Whether this hand-back repeats `earlier`: the same checks fail and
    /// the Tester says the same thing about them, so another round of the
    /// same repair would loop.
    #[must_use]
    pub fn repeats(&self, earlier: &Self) -> bool {
        self.target == earlier.target
            && self.failing_checks == earlier.failing_checks
            && self.hypothesis.eq_ignore_ascii_case(&earlier.hypothesis)
    }
}

/// The hands' known costs added up; `None` when none reported one.
#[must_use]
pub fn total_cost(turns: &[Turn]) -> Option<f64> {
    let known: Vec<f64> = turns.iter().filter_map(|turn| turn.work.cost_usd).collect();
    (!known.is_empty()).then(|| known.iter().sum())
}
