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
    let execution_status = answer.report.status.clone();
    let execution_stuck = answer.stuck;
    let mut remaining: Vec<String> = Vec::new();
    if finished && let Some(text) = review_request(&workdir, reference.number) {
        say!("issue ▸ checking the code that uses what changed");
        let mut request = text;
        // The review, then up to two fix rounds on what the host still
        // finds: sessions' own reports once said "no bug found" on a view
        // two rows off, and opened a pull request whose tests failed.
        for round in 0..=FIX_ROUNDS {
            let review = Request {
                request,
                review: true,
                artifacts: inner.artifacts.with_file_name(format!("review-{round}")),
                ..inner.clone()
            };
            let reviewed = Box::pin(crate::terminal::answer(&review, on.clone())).await;
            let (status, stuck) = reviewed_outcome(
                &execution_status,
                execution_stuck,
                Some((&reviewed.report.status, reviewed.stuck)),
            );
            answer.report.status = status;
            answer.stuck = stuck;
            answer.report.summary.is_error = Some(!matches!(
                answer.report.status,
                crate::delegate::Status::Answered
            ));
            absorb(&mut answer, reviewed);
            say!("issue ▸ running the tests and checks on the change");
            let checked = Recorder::default();
            remaining = gate(&workdir, inner.jev.as_ref(), &checked).await;
            answer.steps.extend(checked.steps());
            if remaining.is_empty() {
                say!("issue ▸ the tests pass and the checks found nothing");
                break;
            }
            say!(
                "issue ▸ {} problem{} left: {}",
                remaining.len(),
                if remaining.len() == 1 { "" } else { "s" },
                crate::judge::clip(&remaining.join("; "), 300)
            );
            if round == FIX_ROUNDS {
                break;
            }
            request = fix_request(&workdir, reference.number, &remaining);
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
    let finished = matches!(answer.report.status, crate::delegate::Status::Answered);
    let outcome = land(
        &workdir,
        &branch,
        &issue,
        &what,
        finished,
        answer.stuck,
        &remaining,
    );
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
    remaining: &[String],
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
    let problems = if remaining.is_empty() {
        String::new()
    } else {
        format!(
            "**The host's tests and checks still find problems:**\n\n{}\n\n",
            remaining
                .iter()
                .map(|problem| format!("- {}", problem.replace('\n', " ")))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };
    let body = format!(
        "{warning}{problems}{summary}\n\n{stat}\n\nCloses {}\n\n---\nOpened by Coder.",
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

/// Preserve the executor's failure, or carry the review's failure forward.
/// An unfinished requirement remains unfinished even when the reviewer answers.
fn reviewed_outcome(
    execution: &crate::delegate::Status,
    stuck: bool,
    review: Option<(&crate::delegate::Status, bool)>,
) -> (crate::delegate::Status, bool) {
    let Some((review, review_stuck)) = review else {
        return (execution.clone(), stuck);
    };
    let status = if matches!(execution, crate::delegate::Status::Answered) {
        review.clone()
    } else {
        execution.clone()
    };
    (status, stuck || review_stuck)
}

/// One place that uses a name the change touched, with the lines around
/// it as they stand in the working tree.
struct Excerpt {
    file: String,
    line: usize,
    /// The first and last line numbers shown, 1-based.
    span: (usize, usize),
    name: String,
    text: String,
}

/// The places that use the Rust functions and constants the staged
/// change touches, at most [`REVIEW_CALLERS`] of them.
fn excerpts(workdir: &Path) -> Vec<Excerpt> {
    let bare = command(workdir, "git", &["diff", "--cached", "-U0"]).unwrap_or_default();
    let mut found = Vec::new();
    for (path, name) in changed_names(workdir, &bare) {
        for (file, line) in uses(workdir, &path, &name) {
            if found.len() >= REVIEW_CALLERS {
                return found;
            }
            let Ok(text) = std::fs::read_to_string(workdir.join(&file)) else {
                continue;
            };
            let lines: Vec<&str> = text.lines().collect();
            let from = line.saturating_sub(CALLER_BEFORE + 1);
            let to = (line + CALLER_AFTER).min(lines.len());
            found.push(Excerpt {
                span: (from + 1, to),
                file,
                line,
                name: name.clone(),
                text: (from..to)
                    .map(|i| format!("{:>5} {}", i + 1, lines[i]))
                    .collect::<Vec<_>>()
                    .join("\n"),
            });
        }
    }
    found
}

/// The Noul the gate asks about each place that uses what changed; `{id}`
/// names it.
pub const DEPENDS_QUESTION: &str = "Does the code in `uses.{id}` depend on how many items, or \
which positions, the changed code in `diff` produces, such as an index offset, a fixed count, \
or a row number, and does the change alter that number or those positions without this code \
being updated to match?";

/// How sure Jev must be that a place depends on a changed count to flag
/// it. The stale `2 + cursor` offset read between 0.55 and 0.68 across
/// runs; the false alarms, 0.52 and 0.54, were on code the change had
/// edited, which the check now skips.
const DEPENDS_FLAG: f64 = 0.5;

/// The new-side line numbers a zero-context diff adds or changes, by file.
fn edited_lines(bare: &str) -> Vec<(String, usize)> {
    let mut edited = Vec::new();
    let mut file = String::new();
    for line in bare.lines() {
        if let Some(path) = line.strip_prefix("+++ b/") {
            file = path.to_string();
            continue;
        }
        let Some(hunk) = line.strip_prefix("@@ ") else {
            continue;
        };
        let Some(new) = hunk
            .split_whitespace()
            .find_map(|part| part.strip_prefix('+'))
        else {
            continue;
        };
        let mut parts = new.split(',');
        let start: usize = parts.next().and_then(|n| n.parse().ok()).unwrap_or(0);
        let count: usize = parts.next().and_then(|n| n.parse().ok()).unwrap_or(1);
        // A pure deletion still touches the line it sits beside.
        for at in start..start + count.max(1) {
            edited.push((file.clone(), at));
        }
    }
    edited
}

/// Places that use what changed and, as Jev reads them, depend on a count
/// or position the change alters without being updated: a view once
/// selected row `2 + cursor` after two lines went in above the rows, and
/// every test still passed.
async fn stale_dependents(
    workdir: &Path,
    jev: Option<&jev::Client>,
    recorder: &Recorder,
) -> Vec<String> {
    let Some(client) = jev else {
        return Vec::new();
    };
    let bare = command(workdir, "git", &["diff", "--cached", "-U0"]).unwrap_or_default();
    let edited = edited_lines(&bare);
    // Code the change already edited was updated to match; asking about it
    // once flagged a `4 + cursor` offset right after the fix.
    let found: Vec<Excerpt> = excerpts(workdir)
        .into_iter()
        .filter(|excerpt| {
            !edited.iter().any(|(file, line)| {
                *file == excerpt.file && (excerpt.span.0..=excerpt.span.1).contains(line)
            })
        })
        .collect();
    if found.is_empty() {
        return Vec::new();
    }
    let diff = command(workdir, "git", &["diff", "--cached", "-U3"]).unwrap_or_default();
    let mut questions = jev::Questions::new();
    let mut uses = serde_json::Map::new();
    for (i, excerpt) in found.iter().enumerate() {
        let id = format!("u{}", i + 1);
        uses.insert(
            id.clone(),
            json!(format!(
                "{}:{} uses `{}`\n{}",
                excerpt.file, excerpt.line, excerpt.name, excerpt.text
            )),
        );
        questions = questions.with(
            format!("depends_{}", i + 1),
            jev::Noul::new(DEPENDS_QUESTION.replace("{id}", &id)),
        );
    }
    let asked = crate::component::jev::ask(
        &crate::component::jev::JevMode::Live(client.clone()),
        recorder,
        crate::component::jev::Ask {
            component: "issue.gate",
            name: "jev_stale_dependents",
            id: "jev_stale_dependents-1".to_string(),
            state: json!({ "diff": crate::judge::clip(&diff, 10_000), "uses": uses }),
            questions,
            parent: None,
            deadline: None,
        },
    )
    .await;
    found
        .iter()
        .enumerate()
        .filter_map(|(i, excerpt)| {
            let p = asked.noul(&format!("depends_{}", i + 1))?;
            (p >= DEPENDS_FLAG).then(|| {
                format!(
                    "{}:{} depends on the number or positions of what `{}` produces, and the \
                     change alters them (Jev {p:.2}); update that code to match and add a test \
                     that checks it, such as which row a view selects",
                    excerpt.file, excerpt.line, excerpt.name
                )
            })
        })
        .collect()
}

/// Relative Markdown links the change adds that point at no file, or at
/// a heading the file doesn't have.
fn broken_links(workdir: &Path, diff: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let mut file = String::new();
    for line in diff.lines() {
        if let Some(path) = line.strip_prefix("+++ b/") {
            file = path.to_string();
            continue;
        }
        let Some(added) = line.strip_prefix('+') else {
            continue;
        };
        if !file.ends_with(".md") {
            continue;
        }
        let mut rest = added;
        while let Some(at) = rest.find("](") {
            let after = &rest[at + 2..];
            let Some(end) = after.find(')') else {
                break;
            };
            let target = &after[..end];
            rest = &after[end..];
            if target.contains("://") || target.starts_with("mailto:") || target.is_empty() {
                continue;
            }
            let (path, anchor) = target.split_once('#').unwrap_or((target, ""));
            let resolved = if path.is_empty() {
                workdir.join(&file)
            } else {
                workdir
                    .join(&file)
                    .parent()
                    .map_or_else(|| workdir.join(path), |dir| dir.join(path))
            };
            let Ok(resolved) = resolved.canonicalize() else {
                problems.push(format!(
                    "{file}: the link `{target}` points at a file that doesn't exist"
                ));
                continue;
            };
            if !resolved.starts_with(workdir.canonicalize().unwrap_or_default()) {
                problems.push(format!(
                    "{file}: the link `{target}` points outside the repository"
                ));
                continue;
            }
            if !anchor.is_empty()
                && let Ok(text) = std::fs::read_to_string(&resolved)
                && !text
                    .lines()
                    .filter(|l| l.starts_with('#'))
                    .any(|l| slug(l.trim_start_matches('#')) == anchor)
            {
                problems.push(format!(
                    "{file}: the link `{target}` names a heading the file doesn't have"
                ));
            }
        }
    }
    problems
}

/// A heading's anchor as GitHub writes it.
fn slug(heading: &str) -> String {
    heading
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'))
        .map(|c| if c == ' ' { '-' } else { c })
        .collect()
}

/// Fix rounds after the review when the host's gate still finds problems.
const FIX_ROUNDS: usize = 3;

/// Folds a follow-up session's answer into the turn's: its steps, its
/// summaries, and its cost.
fn absorb(answer: &mut Answer, mut reviewed: Answer) {
    answer.steps.append(&mut reviewed.steps);
    for summary in reviewed.summaries {
        if !answer.summaries.iter().any(|seen| seen.ends_with(&summary)) {
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

/// What the host finds wrong with the staged change, without a model:
/// failing tests in the Rust packages it touches, style problems, and
/// added figures that appear nowhere else in the repository.
async fn gate(workdir: &Path, jev: Option<&jev::Client>, recorder: &Recorder) -> Vec<String> {
    let _ = command(workdir, "git", &["add", "-A"]);
    let diff = command(workdir, "git", &["diff", "--cached", "-U0"]).unwrap_or_default();
    let mut problems = test_failures(workdir, &diff);
    problems.extend(style_problems(&diff));
    problems.extend(unsourced_figures(workdir, &diff));
    problems.extend(broken_links(workdir, &diff));
    problems.extend(stale_dependents(workdir, jev, recorder).await);
    problems.extend(unclear_text(&diff, jev, recorder).await);
    problems
}

/// The most added texts the plain-language check asks about.
const PLAIN_MAX: usize = 12;

/// The Noul the plain-language check asks about each added text.
pub const PLAIN_QUESTION: &str = "Would a reader who has never seen this project's code \
understand the text in `texts.{id}` as written: plain words and complete phrases, with no \
shorthand, cryptic abbreviations, or symbols standing in for words?";

/// Added interface strings and prose lines that, as Jev reads them, a
/// newcomer wouldn't understand: a view line once read "screen, not TB4;
/// script <10 s $0".
async fn unclear_text(diff: &str, jev: Option<&jev::Client>, recorder: &Recorder) -> Vec<String> {
    let Some(client) = jev else {
        return Vec::new();
    };
    let mut texts: Vec<(String, String)> = Vec::new();
    let mut file = String::new();
    let mut fenced = false;
    // Copy the change kept from before, such as a title it shortened, is
    // not the change's to answer for: the check once flagged a view's
    // original "Coder One mini-task runs · {} runs" and spent a fix round.
    let removed: Vec<String> = diff
        .lines()
        .filter_map(|line| line.strip_prefix('-'))
        .filter(|line| !line.starts_with("--"))
        .map(str::to_string)
        .collect();
    for line in diff.lines() {
        if let Some(path) = line.strip_prefix("+++ b/") {
            file = path.to_string();
            fenced = false;
            continue;
        }
        let Some(added) = line.strip_prefix('+') else {
            continue;
        };
        if file.ends_with(".md") && added.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        let candidates = if file.ends_with(".md") && !fenced {
            vec![strip_code(added)]
        } else if file.ends_with(".rs") && !added.trim_start().starts_with("assert") {
            string_literals(added)
        } else {
            Vec::new()
        };
        for text in candidates {
            let text = text.trim().to_string();
            if text.split_whitespace().count() >= 4
                && !removed.iter().any(|old| old.contains(&text))
                && !texts.iter().any(|(_, seen)| *seen == text)
            {
                texts.push((file.clone(), text));
            }
        }
    }
    texts.truncate(PLAIN_MAX);
    if texts.is_empty() {
        return Vec::new();
    }
    let mut questions = jev::Questions::new();
    let mut named = serde_json::Map::new();
    for (i, (_, text)) in texts.iter().enumerate() {
        let id = format!("t{}", i + 1);
        named.insert(id.clone(), json!(text));
        questions = questions.with(
            format!("plain_{}", i + 1),
            jev::Noul::new(PLAIN_QUESTION.replace("{id}", &id)),
        );
    }
    let asked = crate::component::jev::ask(
        &crate::component::jev::JevMode::Live(client.clone()),
        recorder,
        crate::component::jev::Ask {
            component: "issue.gate",
            name: "jev_plain_text",
            id: "jev_plain_text-1".to_string(),
            state: json!({ "texts": named }),
            questions,
            parent: None,
            deadline: None,
        },
    )
    .await;
    texts
        .iter()
        .enumerate()
        .filter_map(|(i, (file, text))| {
            let p = asked.noul(&format!("plain_{}", i + 1))?;
            (p < PLAIN_FLAG).then(|| {
                format!(
                    "{file}: \"{}\" reads as shorthand (Jev {p:.2}); say it in plain, complete \
                     words, and add a line rather than abbreviate",
                    crate::judge::clip(text, 120)
                )
            })
        })
        .collect()
}

/// How sure Jev must be that a newcomer understands a text for it to
/// pass. Measured on 2026-09-24: shorthand view lines read 0.04 and 0.06,
/// and a plain sentence with figures 0.38, so the line sits between.
const PLAIN_FLAG: f64 = 0.2;

/// The request for a fix round: the problems, then the diff.
fn fix_request(workdir: &Path, number: u64, problems: &[String]) -> String {
    let diff = command(workdir, "git", &["diff", "--cached", "-U3"]).unwrap_or_default();
    format!(
        "# Fix these problems in the change for issue #{number} before it lands\n\nThe host \
         found them by running the tests and checking the diff; fix each one, then run the \
         failing tests again.\n\n{}\n\n## The diff\n\n```diff\n{}\n```\n",
        problems
            .iter()
            .map(|problem| format!("- {problem}"))
            .collect::<Vec<_>>()
            .join("\n"),
        crate::judge::clip(&diff, 14_000)
    )
}

/// The Cargo packages whose directories `diff` touches, by name.
fn changed_packages(workdir: &Path, diff: &str) -> Vec<String> {
    let mut packages: Vec<String> = Vec::new();
    for line in diff.lines() {
        let Some(path) = line.strip_prefix("+++ b/") else {
            continue;
        };
        let mut dir = Path::new(path).parent();
        while let Some(at) = dir {
            let manifest = workdir.join(at).join("Cargo.toml");
            if let Ok(text) = std::fs::read_to_string(&manifest)
                && text.contains("[package]")
            {
                let name = text
                    .lines()
                    .skip_while(|l| l.trim() != "[package]")
                    .find_map(|l| {
                        l.trim()
                            .strip_prefix("name")
                            .and_then(|rest| rest.trim().strip_prefix('='))
                            .map(|v| v.trim().trim_matches('"').to_string())
                    });
                if let Some(name) = name
                    && !packages.contains(&name)
                {
                    packages.push(name);
                }
                break;
            }
            dir = at.parent();
        }
    }
    packages
}

/// The most test output kept for one failing package.
const TEST_OUTPUT_KEPT: usize = 3_000;

/// Runs each changed package's tests with every feature on, in a target
/// directory shared across issue runs, and reports the failing ones with
/// the end of their output.
fn test_failures(workdir: &Path, diff: &str) -> Vec<String> {
    let target = crate::credentials::openagents_dir()
        .map(|dir| dir.join("coder-one").join("target"))
        .unwrap_or_else(|| workdir.join("target"));
    let mut failures = Vec::new();
    for package in changed_packages(workdir, diff) {
        say!("issue ▸ running the {package} tests");
        let output = Command::new("timeout")
            .args([
                "1200",
                "cargo",
                "test",
                "-q",
                "-p",
                &package,
                "--all-features",
            ])
            .env("CARGO_TARGET_DIR", &target)
            .current_dir(workdir)
            .output();
        match output {
            Ok(out) if out.status.success() => {}
            Ok(out) => {
                let text = format!(
                    "{}{}",
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                );
                let lines: Vec<&str> = text
                    .lines()
                    .filter(|l| {
                        l.contains("FAILED")
                            || l.contains("panicked")
                            || l.starts_with("error")
                            || l.contains("assertion")
                            || l.trim_start().starts_with("left")
                            || l.trim_start().starts_with("right")
                    })
                    .collect();
                failures.push(format!(
                    "the {package} tests fail (`cargo test -p {package} --all-features`): {}",
                    crate::judge::clip(&lines.join("\n"), TEST_OUTPUT_KEPT)
                ));
            }
            Err(error) => failures.push(format!("the {package} tests could not run: {error}")),
        }
    }
    failures
}

/// Figures the change adds to prose or to strings, such as `$0.0041` or
/// `27 s`, that appear nowhere in the repository before the change: a
/// session once cited "12 Luna runs averaged 27 s and $0.0041", which no
/// file records.
fn unsourced_figures(workdir: &Path, diff: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let mut file = String::new();
    let mut seen: Vec<String> = Vec::new();
    for line in diff.lines() {
        if let Some(path) = line.strip_prefix("+++ b/") {
            file = path.to_string();
            continue;
        }
        let Some(added) = line.strip_prefix('+') else {
            continue;
        };
        let texts = if file.ends_with(".md") {
            vec![strip_code(added)]
        } else if file.ends_with(".rs") {
            string_literals(added)
        } else {
            continue;
        };
        for text in texts {
            for figure in figures(&text) {
                if seen.contains(&figure) {
                    continue;
                }
                seen.push(figure.clone());
                // A file may hold the number without its currency sign.
                let number = figure.trim_start_matches('$');
                let found = Command::new("git")
                    .args(["grep", "-q", "-F", "-e", number, "HEAD", "--", "."])
                    .current_dir(workdir)
                    .status()
                    .is_ok_and(|status| status.success());
                if !found {
                    problems.push(format!(
                        "{file}: the figure \"{figure}\" appears nowhere else in the repository; \
                         cite the file it comes from and quote it exactly, or remove it"
                    ));
                }
            }
        }
    }
    problems
}

/// The figures in `text` worth sourcing: amounts with a `$`, and numbers
/// with a decimal point. Plain small integers are left out.
fn figures(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let dollar = chars[i] == '$';
        let start = if dollar { i + 1 } else { i };
        if start < chars.len()
            && chars[start].is_ascii_digit()
            && (i == 0 || !chars[i - 1].is_alphanumeric())
        {
            let mut end = start;
            while end < chars.len() && (chars[end].is_ascii_digit() || chars[end] == '.') {
                end += 1;
            }
            let number: String = chars[start..end]
                .iter()
                .collect::<String>()
                .trim_end_matches('.')
                .to_string();
            if dollar || number.contains('.') {
                found.push(if dollar { format!("${number}") } else { number });
            }
            i = end.max(i + 1);
        } else {
            i += 1;
        }
    }
    found
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
    let mut callers = String::new();
    for excerpt in excerpts(workdir) {
        callers.push_str(&format!(
            "### {}:{} uses `{}`\n\n```\n{}\n```\n\n",
            excerpt.file, excerpt.line, excerpt.name, excerpt.text
        ));
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
                let numeric = |h: &&str| !h.is_empty() && h.chars().all(|c| c.is_ascii_digit());
                // A path has a dot or several slashes; a fraction is digits.
                let extension = word
                    .char_indices()
                    .any(|(i, c)| c == '.' && word[i + 1..].starts_with(char::is_alphabetic));
                let path = extension || halves.len() > 2;
                if halves.len() == 2 && !path && !halves.iter().all(numeric) {
                    let per =
                        halves[0].ends_with(|c: char| c.is_ascii_digit()) && alphabetic(&halves[1]);
                    problems.push(if per {
                        format!("{file}: \"{word}\" uses a slash for \"per\"; write \"per\"")
                    } else {
                        format!(
                            "{file}: \"{word}\" uses a slash between words; write them out, \
                             such as \"or\", \"and\", or \"per\""
                        )
                    });
                }
                for half in &halves {
                    let digits = half.trim_start_matches('$');
                    let lead: String = digits
                        .chars()
                        .take_while(|c| c.is_ascii_digit() || *c == '.')
                        .collect();
                    let unit = &digits[lead.len()..];
                    if !lead.is_empty() && matches!(unit, "s" | "ms" | "min") {
                        problems.push(format!(
                            "{file}: \"{half}\" needs a space between the number and its unit"
                        ));
                    }
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

    #[test]
    fn review_outcomes_reach_the_publication_decision() {
        use crate::delegate::Status;
        for failed in [
            Status::Failed(1),
            Status::TimedOut,
            Status::Refused("no".into()),
            Status::Harness("missing".into()),
        ] {
            assert_eq!(
                reviewed_outcome(&Status::Answered, false, Some((&failed, false))),
                (failed, false)
            );
        }
        assert_eq!(
            reviewed_outcome(&Status::Answered, false, Some((&Status::Answered, true))),
            (Status::Answered, true)
        );
        assert_eq!(
            reviewed_outcome(&Status::Answered, true, Some((&Status::Answered, false))),
            (Status::Answered, true)
        );
        assert_eq!(
            reviewed_outcome(&Status::Answered, false, Some((&Status::Answered, false))),
            (Status::Answered, false)
        );
        assert_eq!(
            reviewed_outcome(&Status::Answered, false, None),
            (Status::Answered, false)
        );
    }

    #[test]
    fn a_failed_review_leaves_changes_staged_without_committing_or_publishing() {
        use crate::delegate::Status;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        command(root, "git", &["init", "-q"]).unwrap();
        command(root, "git", &["config", "user.email", "test@example.com"]).unwrap();
        command(root, "git", &["config", "user.name", "test"]).unwrap();
        command(root, "git", &["commit", "--allow-empty", "-qm", "base"]).unwrap();
        let before = command(root, "git", &["rev-parse", "HEAD"]).unwrap();
        std::fs::write(root.join("change.txt"), "pending review").unwrap();
        let (status, stuck) =
            reviewed_outcome(&Status::Answered, false, Some((&Status::Failed(1), false)));
        let issue = Fetched {
            url: "https://github.com/example/example/issues/1".into(),
            title: "Example".into(),
            body: String::new(),
        };
        let result = land(
            root,
            "codex/review-test",
            &issue,
            "Worked.",
            status == Status::Answered,
            stuck,
            &[],
        )
        .unwrap();
        assert!(result.contains("not committed"), "{result}");
        assert_eq!(
            command(root, "git", &["rev-parse", "HEAD"]).unwrap(),
            before
        );
        assert_eq!(
            command(root, "git", &["diff", "--cached", "--name-only"])
                .unwrap()
                .trim(),
            "change.txt"
        );
        // No remote or gh setup: reaching either publication operation fails this test.
    }

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
    fn broken_links_find_missing_files_and_headings() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("docs/guides")).unwrap();
        std::fs::create_dir_all(root.join("docs/bench")).unwrap();
        std::fs::write(
            root.join("docs/bench/r.md"),
            "# R\n\n## Mini-task results\n",
        )
        .unwrap();
        std::fs::write(root.join("docs/guides/g.md"), "text\n").unwrap();
        let diff = "+++ b/docs/guides/g.md\n\
            +See [ok](../bench/r.md#mini-task-results), [far](../../../bench/r.md),\n\
            +[gone](../bench/x.md), and [heading](../bench/r.md#nothing).\n";
        let problems = broken_links(root, diff);
        assert_eq!(problems.len(), 3, "{problems:#?}");
        assert!(problems[0].contains("../../../bench/r.md"));
        assert!(problems[1].contains("../bench/x.md"));
        assert!(problems[2].contains("#nothing"));
    }

    #[test]
    fn slashes_between_words_and_units_without_spaces_are_flagged() {
        let diff = "+++ b/src/v.rs\n+    \"Scripted: about 1s/$0, 3/4 passed, see docs/a/b.md or a.json/b\"\n";
        let problems = style_problems(diff);
        assert_eq!(problems.len(), 2, "{problems:#?}");
        assert!(problems[0].contains("slash between words"), "{problems:#?}");
    }

    #[test]
    fn edited_lines_read_hunk_ranges() {
        let bare = "+++ b/src/a.rs\n@@ -700 +700 @@ fn x\n+++ b/src/b.rs\n@@ -3,0 +4,2 @@\n";
        assert_eq!(
            edited_lines(bare),
            [
                ("src/a.rs".to_string(), 700),
                ("src/b.rs".to_string(), 4),
                ("src/b.rs".to_string(), 5)
            ]
        );
    }

    #[test]
    fn figures_are_amounts_and_decimals() {
        assert_eq!(
            figures("12 Luna runs averaged 27 s and $0.0041; 22.0 s and $0 for v1.2, or 1.5 s."),
            ["$0.0041", "22.0", "$0", "1.5"]
        );
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
