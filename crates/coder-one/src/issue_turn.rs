//! A terminal turn that works a GitHub issue.
//!
//! When a request names an issue to work, such as "do #9597", or "do it"
//! after a reply that proposed one, the turn runs the issue flow instead of
//! editing the user's checkout: it clones the issue's repository fresh
//! under `~/.openagents/coder-one/runs/`, works on a new branch there
//! through the same Microluna loop a terminal change runs, and, when the
//! loop finishes with changes, commits them, pushes the branch, and opens a
//! draft pull request. Every step says a progress line, so the terminal
//! draws the run as it goes.
//!
//! Code finds the issue references in the request and the conversation;
//! Jev only chooses among them, or answers `none`. A turn that names no
//! issue asks Jev nothing.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;

use crate::record::Recorder;
use crate::say::say;
use crate::terminal::{Answer, Progress, Request};

/// The most issue references offered to Jev.
const CANDIDATES_MAX: usize = 12;

/// The Choice question that picks the issue a request asks to work.
pub const WORKS_ISSUE: &str = "Which GitHub issue does the request in `request` ask the \
agent to work on now: implement, fix, resolve, or otherwise do what the issue asks? Take the \
conversation in `earlier` into account: a request such as \"do it\", \"go ahead\", or \
\"continue\" asks for the issue the conversation just proposed doing. Answer none when the \
request only asks about issues, such as counting, listing, summarizing, or comparing them, or \
asks for no work on any issue.";

/// An issue reference found in text: the repository when a URL named
/// one, and the number.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reference {
    pub repository: Option<String>,
    pub number: u64,
}

impl Reference {
    /// The option name Jev chooses.
    fn option(&self) -> String {
        format!("#{}", self.number)
    }
}

/// Every issue reference in `text`, in order and without repeats: GitHub
/// issue URLs and `#N`.
#[must_use]
pub fn references(text: &str) -> Vec<Reference> {
    let mut found: Vec<Reference> = Vec::new();
    let mut push = |reference: Reference| {
        if let Some(seen) = found.iter_mut().find(|r| r.number == reference.number) {
            if seen.repository.is_none() {
                seen.repository = reference.repository;
            }
        } else {
            found.push(reference);
        }
    };
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let rest = &text[index..];
        if let Some(tail) = rest.strip_prefix("https://github.com/")
            && let Some((repository, number, used)) = issue_url(tail)
        {
            push(Reference {
                repository: Some(repository),
                number,
            });
            index += "https://github.com/".len() + used;
            continue;
        }
        let starts_word = index == 0 || !(bytes[index - 1].is_ascii_alphanumeric());
        // "issue 9597" and "issues 9597" name an issue as surely as "#9597".
        if starts_word && let Some(number) = issue_word(rest) {
            push(Reference {
                repository: None,
                number,
            });
        }
        if bytes[index] == b'#' && starts_word {
            let digits: String = rest[1..].chars().take_while(char::is_ascii_digit).collect();
            let ends_word = rest[1 + digits.len()..]
                .chars()
                .next()
                .is_none_or(|c| !c.is_ascii_alphanumeric());
            if (1..=7).contains(&digits.len())
                && ends_word
                && let Ok(number) = digits.parse()
            {
                push(Reference {
                    repository: None,
                    number,
                });
                index += 1 + digits.len();
                continue;
            }
        }
        index += rest.chars().next().map_or(1, char::len_utf8);
    }
    found
}

/// Reads `issue N` or `issues N` at the start of `rest`, in any case.
fn issue_word(rest: &str) -> Option<u64> {
    let head = rest.get(..5)?;
    if !head.eq_ignore_ascii_case("issue") {
        return None;
    }
    let after = rest[5..].strip_prefix(['s', 'S']).unwrap_or(&rest[5..]);
    let spaced = after.trim_start_matches([' ', '\t']);
    if spaced.len() == after.len() {
        return None;
    }
    let digits: String = spaced.chars().take_while(char::is_ascii_digit).collect();
    let ends_word = spaced[digits.len()..]
        .chars()
        .next()
        .is_none_or(|c| !c.is_ascii_alphanumeric());
    ((1..=7).contains(&digits.len()) && ends_word)
        .then(|| digits.parse().ok())
        .flatten()
}

/// Reads `owner/name/issues/N` at the start of `tail`: the repository,
/// the number, and the bytes used.
fn issue_url(tail: &str) -> Option<(String, u64, usize)> {
    let mut parts = tail.splitn(4, '/');
    let owner = parts.next()?;
    let name = parts.next()?;
    if parts.next()? != "issues" {
        return None;
    }
    let digits: String = parts
        .next()?
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    let number = digits.parse().ok()?;
    let valid = |part: &str| {
        !part.is_empty()
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    (valid(owner) && valid(name)).then(|| {
        (
            format!("{owner}/{name}"),
            number,
            owner.len() + name.len() + "/issues/".len() + 1 + digits.len(),
        )
    })
}

/// The issue this turn asks to work, when it asks to work one. The
/// request's own references come first, then the conversation's, the most
/// recent first; with none, Jev is not asked.
pub async fn asked(request: &Request, recorder: &Recorder) -> Option<Reference> {
    let client = request.jev.clone()?;
    let mut candidates = references(&request.request);
    let mut earlier = references(&request.earlier);
    earlier.reverse();
    for reference in earlier {
        if !candidates.iter().any(|c| c.number == reference.number) {
            candidates.push(reference);
        }
    }
    candidates.truncate(CANDIDATES_MAX);
    if candidates.is_empty() {
        return None;
    }
    let mut choice = jev::Choice::new(WORKS_ISSUE, indexmap::IndexMap::new()).option(
        "none",
        "The request asks for no work on an issue, or only asks about issues.",
    );
    for candidate in &candidates {
        choice = choice.option(
            candidate.option(),
            format!("Work issue {} now.", candidate.option()),
        );
    }
    let answer = crate::component::jev::ask(
        &crate::component::jev::JevMode::Live(client),
        recorder,
        crate::component::jev::Ask {
            component: "route.issue",
            name: "jev_works_issue",
            id: "jev_works_issue-1".to_string(),
            state: json!({
                "request": request.request,
                "earlier": crate::judge::clip(&request.earlier, 4_000),
            }),
            questions: jev::Questions::new().with("issue", choice),
            parent: None,
            deadline: None,
        },
    )
    .await;
    let chosen = answer.choice("issue")?;
    candidates.into_iter().find(|c| c.option() == chosen)
}

/// The issue as `gh` returns it.
struct Fetched {
    url: String,
    title: String,
    body: String,
}

/// Runs the issue flow for `reference` and returns the turn's answer.
/// `recorder` holds what the turn recorded before the flow began, the
/// choice of issue among it.
pub async fn run(
    request: &Request,
    reference: Reference,
    on: Rc<dyn Fn(Progress)>,
    recorder: &Recorder,
) -> Answer {
    let prepared = prepare(request, &reference);
    let (inner, workdir, source, branch, issue) = match prepared {
        Ok(prepared) => prepared,
        Err(why) => {
            say!(
                "issue ▸ couldn't set up the issue ({why}), so Coder answers in this checkout instead"
            );
            // The ordinary turn answers instead, in the user's checkout.
            let fallback = Request {
                issues: false,
                ..request.clone()
            };
            return Box::pin(crate::terminal::answer(&fallback, on)).await;
        }
    };
    say!(
        "issue ▸ working on #{} in {}, on branch {branch}",
        reference.number,
        workdir.display()
    );
    let mut answer = Box::pin(crate::terminal::answer(&inner, on.clone())).await;
    let mut before = recorder.steps();
    before.append(&mut answer.steps);
    answer.steps = before;

    // One more session reviews the change against the code that relies on
    // it before it lands: #9602 added two lines above a list's rows, and
    // the view that highlights row `2 + cursor` went two rows off with
    // every test green.
    let finished = matches!(answer.report.status, crate::delegate::Status::Answered);
    if finished && let Some(text) = review_request(&workdir, reference.number) {
        say!("issue ▸ checking the code that uses what changed");
        let review = Request {
            request: text,
            review: true,
            artifacts: inner.artifacts.with_file_name("review"),
            ..inner.clone()
        };
        let mut reviewed = Box::pin(crate::terminal::answer(&review, on)).await;
        answer.steps.append(&mut reviewed.steps);
        for summary in reviewed.summaries {
            if !answer.summaries.contains(&summary) {
                answer.summaries.push(format!("Review: {summary}"));
            }
        }
        if let (Some(before), Some(added)) = (
            answer
                .usage
                .pointer("/cost/amount_usd")
                .and_then(serde_json::Value::as_f64),
            reviewed
                .usage
                .pointer("/cost/amount_usd")
                .and_then(serde_json::Value::as_f64),
        ) && let Some(cost) = answer.usage.pointer_mut("/cost/amount_usd")
        {
            *cost = json!(before + added);
        }
    }

    let reply = answer.report.summary.result.clone().unwrap_or_default();
    // The pull request says what every session did: the last reply alone
    // once read "R3 is already addressed", about a run that changed three
    // files.
    let what = if answer.summaries.is_empty() {
        reply.trim().to_string()
    } else {
        answer
            .summaries
            .iter()
            .map(|summary| format!("- {summary}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let outcome = land(&workdir, &branch, &issue, &what, finished, answer.stuck);
    if outcome
        .as_ref()
        .is_ok_and(|line| line.starts_with("Opened"))
        && let Some(source) = &source
    {
        // The branch is pushed, so the worktree has nothing left to keep.
        let _ = command(
            source,
            "git",
            &["worktree", "remove", "--force", &workdir.to_string_lossy()],
        );
    }
    let closing = match outcome {
        Ok(line) => line,
        Err(why) => format!(
            "Coder couldn't open a pull request ({why}). The changes are in {}.",
            workdir.display()
        ),
    };
    say!("issue ▸ {closing}");
    answer.report.summary.result = Some(if what.is_empty() {
        closing
    } else {
        format!("{what}\n\n{closing}")
    });
    answer
}

/// Fetches the issue, clones its repository, and branches: the inner
/// turn's request, the checkout, the branch, and the issue.
fn prepare(
    request: &Request,
    reference: &Reference,
) -> Result<(Request, PathBuf, Option<PathBuf>, String, Fetched), String> {
    let repository = match &reference.repository {
        Some(repository) => repository.clone(),
        None => command(
            &request.workdir,
            "gh",
            &[
                "repo",
                "view",
                "--json",
                "nameWithOwner",
                "-q",
                ".nameWithOwner",
            ],
        )?
        .trim()
        .to_string(),
    };
    say!(
        "issue ▸ fetching {repository}#{} from GitHub",
        reference.number
    );
    let number = reference.number.to_string();
    let json = command(
        &request.workdir,
        "gh",
        &[
            "issue",
            "view",
            &number,
            "-R",
            &repository,
            "--json",
            "url,title,body,state",
        ],
    )?;
    let value: serde_json::Value =
        serde_json::from_str(&json).map_err(|error| format!("unexpected gh output: {error}"))?;
    if value["state"].as_str() == Some("CLOSED") {
        return Err(format!("{repository}#{number} is closed"));
    }
    let issue = Fetched {
        url: value["url"].as_str().unwrap_or_default().to_string(),
        title: value["title"].as_str().unwrap_or_default().to_string(),
        body: value["body"].as_str().unwrap_or_default().to_string(),
    };
    say!("issue ▸ #{number}: {}", issue.title);

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    let home = crate::credentials::openagents_dir().ok_or("HOME is not set")?;
    let run_dir = home
        .join("coder-one")
        .join("runs")
        .join(format!("{}-{number}-{stamp}", repository.replace('/', "-")));
    let workdir = run_dir.join("repo");
    std::fs::create_dir_all(&run_dir)
        .map_err(|error| format!("cannot create {}: {error}", run_dir.display()))?;
    let branch = format!("coder/issue-{number}-{stamp}");
    // A local checkout of the same repository lends a worktree: a fetch and
    // a checkout take seconds, and a push from full history is quick. A
    // shallow clone took 16 s, and pushing from it 50 s more.
    let source = local_checkout(&request.workdir, &repository);
    if let Some(source) = &source {
        let base = default_branch(source);
        say!(
            "issue ▸ creating a worktree of {} from origin/{base} in {}",
            source.display(),
            workdir.display()
        );
        command(source, "git", &["fetch", "-q", "origin", &base])?;
        command(
            source,
            "git",
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                &branch,
                &workdir.to_string_lossy(),
                &format!("origin/{base}"),
            ],
        )?;
    } else {
        say!("issue ▸ cloning {repository} into {}", workdir.display());
        command(
            &run_dir,
            "gh",
            &[
                "repo",
                "clone",
                &repository,
                &workdir.to_string_lossy(),
                "--",
                "--quiet",
            ],
        )?;
        command(&workdir, "git", &["checkout", "-q", "-b", &branch])?;
    }

    let inner = Request {
        workdir: workdir.clone(),
        // The title rides as a heading, which the requirement map reads as
        // context: as a line of its own it became a requirement that
        // duplicated the body's, and two sessions split one job.
        request: format!(
            "# Issue #{}: {}\n\n{}\n\n{}",
            reference.number,
            issue.title,
            issue.body.trim(),
            issue.url
        ),
        earlier: String::new(),
        resume: None,
        read_only: false,
        clarify: false,
        issues: false,
        issue: true,
        artifacts: run_dir.join("artifacts"),
        ..request.clone()
    };
    Ok((inner, workdir, source, branch, issue))
}

/// The top of the git checkout at `dir` when its `origin` is
/// `repository` on GitHub.
fn local_checkout(dir: &Path, repository: &str) -> Option<PathBuf> {
    let top = command(dir, "git", &["rev-parse", "--show-toplevel"]).ok()?;
    let top = PathBuf::from(top.trim());
    let origin = command(&top, "git", &["remote", "get-url", "origin"]).ok()?;
    let origin = origin.trim().trim_end_matches('/').trim_end_matches(".git");
    let named = origin
        .rsplit_once("github.com")
        .map(|(_, path)| path.trim_start_matches([':', '/']))?;
    named.eq_ignore_ascii_case(repository).then_some(top)
}

/// The branch `origin/HEAD` names in `checkout`, or `main`.
fn default_branch(checkout: &Path) -> String {
    command(
        checkout,
        "git",
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
    )
    .ok()
    .and_then(|name| name.trim().strip_prefix("origin/").map(str::to_string))
    .unwrap_or_else(|| "main".to_string())
}

/// Commits what the run changed, pushes the branch, and opens a draft
/// pull request. Returns the closing line.
fn land(
    workdir: &Path,
    branch: &str,
    issue: &Fetched,
    reply: &str,
    finished: bool,
    stuck: bool,
) -> Result<String, String> {
    command(workdir, "git", &["add", "-A"])?;
    let changed = !Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .current_dir(workdir)
        .status()
        .map_err(|error| format!("cannot run git: {error}"))?
        .success();
    if !changed {
        return Ok(format!(
            "Nothing changed, so there's no pull request. The checkout is in {}.",
            workdir.display()
        ));
    }
    let stat = command(workdir, "git", &["diff", "--cached", "--shortstat"])?;
    say!("issue ▸ {}", stat.trim());
    if !finished {
        return Ok(format!(
            "The work didn't finish, so the changes are staged but not committed in {}.",
            workdir.display()
        ));
    }
    let summary = if reply.trim().is_empty() {
        "Opened by Coder.".to_string()
    } else {
        reply.trim().to_string()
    };
    let stat = stat.trim();
    command(
        workdir,
        "git",
        &["commit", "-q", "-m", &issue.title, "-m", &summary],
    )?;
    say!("issue ▸ pushing branch {branch}");
    command(workdir, "git", &["push", "-q", "-u", "origin", branch])?;
    // A loop that gave up on a requirement says so first, so nobody
    // reads the pull request as finished work.
    let warning = if stuck {
        "**Coder gave up on part of the issue. Review this as unfinished work.**\n\n"
    } else {
        ""
    };
    let body = format!(
        "{warning}{summary}\n\n{stat}\n\nCloses {}\n\n---\nOpened by Coder.",
        issue.url
    );
    let pr = command(
        workdir,
        "gh",
        &[
            "pr",
            "create",
            "--draft",
            "--head",
            branch,
            "--title",
            &issue.title,
            "--body",
            &body,
        ],
    )?;
    Ok(format!("Opened draft pull request {}.", pr.trim()))
}

/// The most changed names whose callers the review reads.
const REVIEW_NAMES: usize = 8;
/// The most caller excerpts the review reads.
const REVIEW_CALLERS: usize = 16;
/// Lines of a caller shown before and after its call.
const CALLER_BEFORE: usize = 20;
const CALLER_AFTER: usize = 10;

/// The review session's request: the staged diff and, for each Rust
/// function or constant the diff changed, excerpts of the code that uses
/// it. `None` when nothing changed.
fn review_request(workdir: &Path, number: u64) -> Option<String> {
    command(workdir, "git", &["add", "-A"]).ok()?;
    let diff = command(workdir, "git", &["diff", "--cached", "-U3"]).ok()?;
    if diff.trim().is_empty() {
        return None;
    }
    let bare = command(workdir, "git", &["diff", "--cached", "-U0"]).ok()?;
    let names = changed_names(workdir, &bare);
    let mut callers = String::new();
    let mut shown = 0;
    for (path, name) in &names {
        for (file, line) in uses(workdir, path, name) {
            if shown >= REVIEW_CALLERS {
                break;
            }
            let Ok(text) = std::fs::read_to_string(workdir.join(&file)) else {
                continue;
            };
            let lines: Vec<&str> = text.lines().collect();
            let from = line.saturating_sub(CALLER_BEFORE + 1);
            let to = (line + CALLER_AFTER).min(lines.len());
            let excerpt: Vec<String> = (from..to)
                .map(|i| format!("{:>5} {}", i + 1, lines[i]))
                .collect();
            callers.push_str(&format!(
                "### {file}:{line} uses `{name}`\n\n```\n{}\n```\n\n",
                excerpt.join("\n")
            ));
            shown += 1;
        }
    }
    if callers.is_empty() {
        callers = "No caller outside the change was found.\n".to_string();
    }
    let problems = style_problems(&diff);
    let problems = if problems.is_empty() {
        String::new()
    } else {
        format!(
            "## Style problems the host found in the added lines\n\nFix each one.\n\n{}\n\n",
            problems
                .iter()
                .map(|problem| format!("- {problem}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };
    Some(format!(
        "# Review the change for issue #{number} before it lands\n\n{problems}## The diff\n\n```diff\n{}\n```\n\n## Code that uses what changed\n\n{callers}",
        crate::judge::clip(&diff, 14_000)
    ))
}

/// The longest text line a terminal view should draw.
const UI_LINE_MAX: usize = 100;

/// Style problems in the lines `diff` adds: a slash standing for "or"
/// in prose, a Markdown line that breaks a hyphenated word, and a Rust
/// string literal longer than a terminal line. Each names its file.
fn style_problems(diff: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let mut file = String::new();
    let mut fenced = false;
    let mut joined = String::new();
    for line in diff.lines() {
        if let Some(path) = line.strip_prefix("+++ b/") {
            file = path.to_string();
            fenced = false;
            continue;
        }
        let Some(added) = line.strip_prefix('+') else {
            if let Some(kept) = line.strip_prefix(' ')
                && kept.trim_start().starts_with("```")
            {
                fenced = !fenced;
            }
            continue;
        };
        let markdown = file.ends_with(".md");
        let rust = file.ends_with(".rs");
        if markdown && added.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        let prose: Vec<String> = if markdown && !fenced {
            vec![strip_code(added)]
        } else if rust {
            string_literals(added)
        } else {
            Vec::new()
        };
        for text in &prose {
            for word in text.split_whitespace() {
                let word = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '/');
                let halves: Vec<&str> = word.split('/').collect();
                let alphabetic = |h: &&str| h.len() >= 2 && h.chars().all(char::is_alphabetic);
                if halves.len() >= 2 && halves.iter().all(alphabetic) {
                    problems.push(format!(
                        "{file}: \"{word}\" uses a slash for \"or\"; write the words out"
                    ));
                } else if halves.len() == 2
                    && halves[0].ends_with(|c: char| c.is_ascii_digit())
                    && alphabetic(&halves[1])
                {
                    problems.push(format!(
                        "{file}: \"{word}\" uses a slash for \"per\"; write \"per\""
                    ));
                }
            }
            if let Some(at) = text.find(['~', '≈'])
                && text[at..]
                    .chars()
                    .skip(1)
                    .collect::<String>()
                    .trim_start()
                    .starts_with(|c: char| c.is_ascii_digit())
            {
                problems.push(format!(
                    "{file}: \"~\" or \"≈\" before a number; write \"about\""
                ));
            }
        }
        if markdown && !fenced {
            let trimmed = added.trim_end();
            let before = trimmed.chars().rev().nth(1);
            if trimmed.ends_with('-') && before.is_some_and(char::is_alphabetic) {
                problems.push(format!(
                    "{file}: a line ends in \"{}\", which breaks a hyphenated word across lines; Markdown renders it with a space",
                    trimmed.split_whitespace().last().unwrap_or_default()
                ));
            }
        }
        if rust {
            // A string split into pieces, one per line as in `concat!`,
            // renders as one line: the pieces are measured together.
            let trimmed = added.trim().trim_end_matches(',');
            let lone = trimmed.starts_with('"')
                && trimmed.ends_with('"')
                && string_literals(trimmed).len() == 1;
            if lone {
                joined.push_str(&string_literals(trimmed).concat());
                continue;
            }
            let mut literals = string_literals(added);
            if !joined.is_empty() {
                literals.push(std::mem::take(&mut joined));
            }
            for literal in literals {
                if literal.chars().count() > UI_LINE_MAX {
                    problems.push(format!(
                        "{file}: a {}-character string is longer than a {UI_LINE_MAX}-column terminal line: \"{}…\"",
                        literal.chars().count(),
                        literal.chars().take(40).collect::<String>()
                    ));
                }
            }
        }
    }
    if joined.chars().count() > UI_LINE_MAX {
        problems.push(format!(
            "{file}: a {}-character string is longer than a {UI_LINE_MAX}-column terminal line: \"{}…\"",
            joined.chars().count(),
            joined.chars().take(40).collect::<String>()
        ));
    }
    problems.dedup();
    problems
}

/// `line` without its inline code spans and link targets.
fn strip_code(line: &str) -> String {
    let mut out = String::new();
    // Odd pieces between backticks are code.
    for (i, part) in line.split('`').enumerate() {
        if i % 2 == 0 {
            out.push_str(part);
            out.push(' ');
        }
    }
    // A link target or bare URL is a path, not prose.
    out.split_whitespace()
        .filter(|word| !word.contains("://") && !word.contains("]("))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The contents of the double-quoted string literals on one Rust line.
fn string_literals(line: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut current: Option<String> = None;
    let mut escaped = false;
    for c in line.chars() {
        match &mut current {
            Some(text) => {
                if escaped {
                    text.push(c);
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == '"' {
                    found.push(current.take().unwrap_or_default());
                } else {
                    text.push(c);
                }
            }
            None if c == '"' => current = Some(String::new()),
            None => {}
        }
    }
    found
}

/// The Rust functions and constants whose bodies or definitions the
/// zero-context diff `bare` touches, with their files: the nearest
/// `fn`, `const`, or `static` at or above each hunk's first new line.
fn changed_names(workdir: &Path, bare: &str) -> Vec<(String, String)> {
    let mut names: Vec<(String, String)> = Vec::new();
    let mut path = String::new();
    for line in bare.lines() {
        if let Some(file) = line.strip_prefix("+++ b/") {
            path = file.to_string();
            continue;
        }
        let Some(hunk) = line.strip_prefix("@@ ") else {
            continue;
        };
        if !path.ends_with(".rs") {
            continue;
        }
        let Some(start) = hunk
            .split_whitespace()
            .find_map(|part| part.strip_prefix('+'))
            .and_then(|part| part.split(',').next())
            .and_then(|n| n.parse::<usize>().ok())
        else {
            continue;
        };
        let Ok(text) = std::fs::read_to_string(workdir.join(&path)) else {
            continue;
        };
        let lines: Vec<&str> = text.lines().collect();
        let found = (0..start.min(lines.len()))
            .rev()
            .find_map(|i| defined_name(lines[i]));
        if let Some(name) = found
            && !names.iter().any(|(p, n)| *p == path && *n == name)
        {
            names.push((path.clone(), name));
        }
        if names.len() >= REVIEW_NAMES {
            break;
        }
    }
    names
}

/// The name a Rust line defines, when it defines a function, a constant,
/// or a static.
fn defined_name(line: &str) -> Option<String> {
    let words: Vec<&str> = line
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty())
        .collect();
    let at = words
        .iter()
        .position(|w| matches!(*w, "fn" | "const" | "static"))?;
    let name = words.get(at + 1)?;
    // `const fn name` names the function, not a constant called `fn`.
    let name = if *name == "fn" {
        words.get(at + 2)?
    } else {
        name
    };
    (!line.trim_start().starts_with("//")).then(|| (*name).to_string())
}

/// Where `name`, defined in `path`, is used: `stem::name` anywhere, and
/// the bare name when it is rare enough to mean this definition. Other
/// files come first; the definition itself is left out.
fn uses(workdir: &Path, path: &str, name: &str) -> Vec<(String, usize)> {
    let stem = Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let grep = |pattern: &str, fixed: bool| -> Vec<(String, usize)> {
        let mut args = vec!["grep", "-n", "-w"];
        if fixed {
            args.push("-F");
        }
        args.extend([pattern, "--", "*.rs"]);
        command(workdir, "git", &args)
            .unwrap_or_default()
            .lines()
            .filter(|hit| {
                defined_name(hit.splitn(3, ':').nth(2).unwrap_or_default()).as_deref() != Some(name)
            })
            .filter_map(|hit| {
                let mut parts = hit.splitn(3, ':');
                let file = parts.next()?.to_string();
                let line = parts.next()?.parse().ok()?;
                Some((file, line))
            })
            .collect()
    };
    let mut found = grep(&format!("{stem}::{name}"), true);
    let bare = grep(name, true);
    if bare.len() <= 15 {
        for hit in bare {
            if !found.contains(&hit) {
                found.push(hit);
            }
        }
    }
    found.sort_by_key(|(file, _)| file == path);
    found
}

/// Runs a command to completion and returns its standard output, or its
/// standard error as the failure.
fn command(dir: &Path, program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|error| format!("cannot run {program}: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(format!(
            "{program} {} failed: {}",
            args.first().unwrap_or(&""),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numbers(text: &str) -> Vec<u64> {
        references(text).into_iter().map(|r| r.number).collect()
    }

    #[test]
    fn references_read_urls_and_hash_numbers_in_order() {
        let text = "Do #9597, “Docs” (https://github.com/OpenAgentsInc/openagents/issues/9597). \
                    Then #12 or issue#3, not a#4 or #5x.";
        let found = references(text);
        assert_eq!(numbers(text), [9597, 12]);
        assert_eq!(
            found[0].repository.as_deref(),
            Some("OpenAgentsInc/openagents")
        );
        assert_eq!(found[1].repository, None);
    }

    #[test]
    fn the_word_issue_before_a_number_names_one() {
        assert_eq!(numbers("work on issue 9597"), [9597]);
        assert_eq!(numbers("Issues 12 and #13"), [12, 13]);
        assert!(numbers("issue9597, issue 12a, tissue 5").is_empty());
    }

    #[test]
    fn defined_names_read_functions_constants_and_statics() {
        assert_eq!(
            defined_name("pub fn lines(runs: &[Run])").as_deref(),
            Some("lines")
        );
        assert_eq!(
            defined_name("pub const HEADER: &str = \"x\";").as_deref(),
            Some("HEADER")
        );
        assert_eq!(
            defined_name("    pub const fn executes(self) -> bool {").as_deref(),
            Some("executes")
        );
        assert_eq!(defined_name("    let x = 1;"), None);
        assert_eq!(defined_name("// fn commented()"), None);
    }

    #[test]
    fn the_review_finds_a_changed_function_and_its_callers() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let git = |args: &[&str]| command(root, "git", args).unwrap();
        git(&["init", "-q"]);
        git(&["config", "user.email", "t@example.com"]);
        git(&["config", "user.name", "t"]);
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("src/view.rs"),
            "pub fn lines() -> Vec<String> {\n    vec![\"head\".into()]\n}\n",
        )
        .unwrap();
        std::fs::write(
            root.join("src/app.rs"),
            "fn selected(cursor: usize) -> usize {\n    1 + cursor\n}\nfn draw() {\n    let rows = crate::view::lines();\n}\n",
        )
        .unwrap();
        git(&["add", "-A"]);
        git(&["commit", "-q", "-m", "base"]);
        std::fs::write(
            root.join("src/view.rs"),
            "pub fn lines() -> Vec<String> {\n    vec![\"head\".into(), \"note\".into()]\n}\n",
        )
        .unwrap();
        let text = review_request(root, 7).unwrap();
        assert!(text.contains("# Review the change for issue #7"), "{text}");
        assert!(text.contains("src/app.rs:5 uses `lines`"), "{text}");
        assert!(text.contains("1 + cursor"), "{text}");
    }

    #[test]
    fn style_problems_find_slashes_broken_hyphens_and_long_strings() {
        let long = "x ".repeat(60);
        let diff = format!(
            "+++ b/docs/a.md\n+Grades are pass/fail, see `a/b` and https://x.io/a/b.\n+a hidden-from-the-\n+- a list item\n+++ b/src/v.rs\n+    lines.push(\"{long}\".into());\n+    let path = \"src/main.rs\";\n"
        );
        let problems = style_problems(&diff);
        assert_eq!(problems.len(), 3, "{problems:#?}");
        assert!(problems[0].contains("\"pass/fail\""));
        assert!(problems[1].contains("hidden-from-the-"));
        assert!(problems[2].contains("120-character string"));
        let split = format!(
            "+++ b/src/v.rs\n+    concat!(\n+        \"{}\",\n+        \"{}\"\n+    )\n",
            "a".repeat(60),
            "b".repeat(60)
        );
        let problems = style_problems(&split);
        assert_eq!(problems.len(), 1, "{problems:#?}");
        assert!(problems[0].contains("120-character string"));
    }

    #[test]
    fn style_problems_find_per_slashes_and_tildes() {
        let diff = "+++ b/src/v.rs\n+    \"scripted ≈1 s; Opus $0.0537/run, 3/4 passed\"\n";
        let problems = style_problems(diff);
        assert_eq!(problems.len(), 2, "{problems:#?}");
        assert!(problems[0].contains("\"per\""));
        assert!(problems[1].contains("\"about\""));
    }

    #[test]
    fn parts_cut_a_requirement_into_its_clauses() {
        let got = crate::micro::parts(
            "- R1 (deliverable): A short explanation, in the docs and in the Gym's view, of what they are, how long they take, what they cost, and how they relate to TB4: a fast screen.",
        );
        assert!(got.contains(&"how long they take".to_string()), "{got:?}");
        assert!(got.contains(&"what they cost".to_string()), "{got:?}");
        assert!(got.contains(&"in the Gym's view".to_string()), "{got:?}");
    }

    #[test]
    fn a_pull_request_url_is_not_an_issue() {
        assert!(numbers("https://github.com/o/r/pull/7").is_empty());
    }

    #[test]
    fn text_with_no_reference_finds_none() {
        assert!(references("how many open issues are there").is_empty());
    }
}
