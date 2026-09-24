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
    let (inner, workdir, branch, issue) = match prepared {
        Ok(prepared) => prepared,
        Err(why) => {
            say!("issue ▸ could not start the issue flow: {why}");
            // The ordinary turn answers instead, in the user's checkout.
            let fallback = Request {
                issues: false,
                ..request.clone()
            };
            return Box::pin(crate::terminal::answer(&fallback, on)).await;
        }
    };
    say!(
        "issue ▸ working #{} in {} on {branch}",
        reference.number,
        workdir.display()
    );
    let mut answer = Box::pin(crate::terminal::answer(&inner, on)).await;
    let mut before = recorder.steps();
    before.append(&mut answer.steps);
    answer.steps = before;

    let reply = answer.report.summary.result.clone().unwrap_or_default();
    let finished = matches!(answer.report.status, crate::delegate::Status::Answered);
    let outcome = land(&workdir, &branch, &issue, &reply, finished);
    let closing = match outcome {
        Ok(line) => line,
        Err(why) => format!("The run's changes stay in {}: {why}", workdir.display()),
    };
    say!("issue ▸ {closing}");
    answer.report.summary.result = Some(if reply.trim().is_empty() {
        closing
    } else {
        format!("{}\n\n{closing}", reply.trim())
    });
    answer
}

/// Fetches the issue, clones its repository, and branches: the inner
/// turn's request, the checkout, the branch, and the issue.
fn prepare(
    request: &Request,
    reference: &Reference,
) -> Result<(Request, PathBuf, String, Fetched), String> {
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
    say!("issue ▸ reading {repository}#{}", reference.number);
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
            "--depth",
            "1",
            "--quiet",
        ],
    )?;
    let branch = format!("coder/issue-{number}-{stamp}");
    command(&workdir, "git", &["checkout", "-q", "-b", &branch])?;

    let inner = Request {
        workdir: workdir.clone(),
        request: format!(
            "Resolve GitHub issue {}: {}\n\n{}",
            issue.url,
            issue.title,
            issue.body.trim()
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
    Ok((inner, workdir, branch, issue))
}

/// Commits what the run changed, pushes the branch, and opens a draft
/// pull request. Returns the closing line.
fn land(
    workdir: &Path,
    branch: &str,
    issue: &Fetched,
    reply: &str,
    finished: bool,
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
            "No changes, so no pull request. The checkout is {}.",
            workdir.display()
        ));
    }
    let stat = command(workdir, "git", &["diff", "--cached", "--shortstat"])?;
    say!("issue ▸ changed {}", stat.trim());
    if !finished {
        return Ok(format!(
            "The run did not finish, so its changes are staged but not committed in {}.",
            workdir.display()
        ));
    }
    let summary = if reply.trim().is_empty() {
        "Opened by Coder.".to_string()
    } else {
        reply.trim().to_string()
    };
    command(
        workdir,
        "git",
        &["commit", "-q", "-m", &issue.title, "-m", &summary],
    )?;
    say!("issue ▸ pushing {branch}");
    command(workdir, "git", &["push", "-q", "-u", "origin", branch])?;
    let body = format!("{summary}\n\nCloses {}\n\n---\nOpened by Coder.", issue.url);
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
    fn a_pull_request_url_is_not_an_issue() {
        assert!(numbers("https://github.com/o/r/pull/7").is_empty());
    }

    #[test]
    fn text_with_no_reference_finds_none() {
        assert!(references("how many open issues are there").is_empty());
    }
}
