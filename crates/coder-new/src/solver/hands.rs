//! The solver's four hands, each one constrained [`agent::Session`].

use std::path::Path;

use claude_agent_sdk::{SdkMcpServer, ToolResult};
use serde_json::json;

use super::{Hand, Options, Turn};
use crate::issue_run::{
    Transcript,
    agent::{self, Session},
    decide::Briefing,
    setup::{self, Base},
};

/// The read-only file tools.
pub const READ_TOOLS: [&str; 3] = ["Read", "Grep", "Glob"];
/// The Coder's tools.
pub const CODE_TOOLS: [&str; 5] = ["Read", "Edit", "Write", "Grep", "Glob"];

/// The programs the Intaker's `environment` tool looks for.
const PROGRAMS: [&str; 3] = ["git", "gh", "claude"];

/// The Intaker's system prompt.
const INTAKER: &str = "You are the Intaker of a four-hand coding run. Your only job is to \
confirm the run can start. Call `environment` once and `issue` once, then answer in a few \
lines: the issue's title and what it asks in one sentence, and a last line that is exactly \
`INTAKE: ready` when git and gh are present and the issue was read, or `INTAKE: blocked: <why>` \
otherwise. A dirty checkout is not a blocker: the run works in its own fresh worktree. Do not \
plan or write code.";

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
specific and short. You cannot edit files.";

/// The Tester's system prompt.
const TESTER: &str = "You are the Tester of a four-hand coding run. The Coder has changed the \
working copy. Run the briefing's checks with `run_check`, read what fails, and end your answer \
with exactly one line: `VERDICT: pass` when every check passes, `VERDICT: coder: <what to fix, \
with file paths and the failing check's words>` when the change needs work, or `VERDICT: \
briefer: <what context is missing>` when the briefing sent the Coder to the wrong place. You \
cannot edit files.";

/// Whether `program` is on the `PATH`.
fn installed(program: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(program).is_file()))
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
                    let mut lines = Vec::new();
                    for program in PROGRAMS {
                        let found = installed(program);
                        lines.push(format!(
                            "{program}: {}",
                            if found { "present" } else { "missing" }
                        ));
                    }
                    match setup::output("git", &["status", "--porcelain"], &repo).await {
                        Ok(status) if status.trim().is_empty() => {
                            lines.push("checkout: clean".into());
                        }
                        Ok(status) => lines.push(format!(
                            "checkout: {} changed path(s), left alone",
                            status.lines().count()
                        )),
                        Err(why) => lines.push(format!("checkout: {why}")),
                    }
                    Ok(ToolResult::text(lines.join("\n")))
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
                    let number = issue.to_string();
                    let args = [
                        "issue",
                        "view",
                        &number,
                        "-R",
                        &github,
                        "--json",
                        "title,state,body",
                    ];
                    match setup::output("gh", &args, Path::new(".")).await {
                        Ok(text) => Ok(ToolResult::text(text)),
                        Err(why) => Ok(ToolResult::error(why)),
                    }
                }
            },
        )
}

/// The Intaker: environment and issue, no files.
pub async fn intake(options: &Options, folder: &Path, transcript: &mut Transcript) -> Turn {
    let session = Session {
        title: "Intaker".into(),
        detail: "Checks the environment and reads the issue".into(),
        model: options.briefer_model.clone(),
        timeout: options.inner.timeout,
        tools: Vec::new(),
        checks: Vec::new(),
        server: Some(intake_server(options)),
        system: INTAKER.into(),
        prompt: format!(
            "Confirm the run for issue #{} in {} can start.",
            options.inner.issue, options.inner.github
        ),
        briefed: Default::default(),
        events_file: "intaker-events.jsonl".into(),
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
    let session = Session {
        title: "Briefer".into(),
        detail: "Reads the briefed files and writes what the change should look like".into(),
        model: options.briefer_model.clone(),
        timeout: options.inner.timeout,
        tools: READ_TOOLS.iter().map(|t| (*t).to_owned()).collect(),
        checks: Vec::new(),
        server: None,
        system: BRIEFER.into(),
        prompt,
        briefed: briefing.files.iter().cloned().collect(),
        events_file: format!("briefer-{round}-events.jsonl"),
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
    prompt.push_str(
        "\n\nYou code: change the files, and reply with what you changed. You have no shell, but `run_check` compiles the changed crates (`check:` entries only); run it after your edits and fix what fails before you finish. The Tester runs the full checks and tests after you.",
    );
    let session = Session {
        title: "Coder".into(),
        detail: "Changes the working copy from the briefing and the analysis".into(),
        model: options.inner.model.clone(),
        timeout: options.inner.timeout,
        tools: CODE_TOOLS.iter().map(|t| (*t).to_owned()).collect(),
        checks: compile_checks(briefing),
        server: None,
        system: briefing.system.clone(),
        prompt,
        briefed: briefing.files.iter().cloned().collect(),
        events_file: format!("coder-{round}-events.jsonl"),
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

/// The Tester: read-only file tools and `run_check`.
pub async fn test(
    options: &Options,
    base: &Base,
    briefing: &Briefing,
    round: usize,
    folder: &Path,
    transcript: &mut Transcript,
) -> Turn {
    let diff = setup::working_diff(&base.worktree, &base.base);
    let session = Session {
        title: "Tester".into(),
        detail: "Runs the briefing's checks and hands back what fails".into(),
        model: options.tester_model.clone(),
        timeout: options.inner.timeout,
        tools: READ_TOOLS.iter().map(|t| (*t).to_owned()).collect(),
        checks: briefing.checks.clone(),
        server: None,
        system: TESTER.into(),
        prompt: format!(
            "Issue #{}: {}\n\nThe Coder's change:\n\n```diff\n{}\n```\n\nRun the checks and give your verdict.",
            base.issue,
            base.title,
            diff.chars().take(60_000).collect::<String>()
        ),
        briefed: briefing.files.iter().cloned().collect(),
        events_file: format!("tester-{round}-events.jsonl"),
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

/// The hands' known costs added up; `None` when none reported one.
#[must_use]
pub fn total_cost(turns: &[Turn]) -> Option<f64> {
    let known: Vec<f64> = turns.iter().filter_map(|turn| turn.work.cost_usd).collect();
    (!known.is_empty()).then(|| known.iter().sum())
}
