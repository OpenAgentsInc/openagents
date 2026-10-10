//! `openagents pr`: read, review, and merge a GitHub pull request through
//! `gh` (#11169), under the owner's approval policy
//! ([`crate::approval_gate`]).
//!
//! - `pr status N` answers its state, head commit, checks, and whether it
//!   can merge now (`can_merge`, and `why_not` when it can't): a failed or
//!   still-running check, a conflict, a draft, or branch protection.
//! - `pr diff N [--file PATH]` answers its diff pinned to the head commit it
//!   was read at, with the new-side lines each file adds
//!   ([`coder::review::parse_diff`]), bounded to fit a tool's answer.
//! - `pr review N --head SHA --findings FILE` posts one review at exactly
//!   that head. FILE is `{findings: [Finding], body}` with
//!   [`coder::review::Finding`]s; each finding anchored to lines the diff
//!   adds ([`coder::review::Scope::anchor`]) becomes an inline comment on
//!   those lines, and the rest go in the review's summary.
//! - `pr merge N --method squash|merge|rebase --head SHA` merges, refused
//!   while `can_merge` is false, and only at that head
//!   (`gh pr merge --match-head-commit`). It waits for the owner: an
//!   approval Coder recorded for exactly `OWNER/NAME#N@SHA`, or `yes` typed
//!   here.
//!
//! The group is kept out of the chat router's command tree: chats use
//! Coder's `pull_request` tool, which asks the owner before a merge.

use std::io::Write;
use std::process::{Command, Stdio};

use coder::review::{Anchor, Finding, Scope, parse_diff};
use coder_new::risk_policy;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::approval_gate;
use crate::{Args, Output};
use coder::cli_route::tree::{Declared, Effect};

pub(crate) const USAGE: &str = "usage: openagents pr COMMAND
  status N [--repo OWNER/NAME]
              Whether the pull request can merge now.
  diff N [--file PATH] [--repo OWNER/NAME]
              The diff, pinned to the head commit it was read at.
  review N --head SHA --findings FILE [--verdict comment|approve|request_changes] [--approval ID] [--repo OWNER/NAME]
              Post FILE's findings ({\"findings\": [...], \"body\": \"...\"}) at
              exactly --head: a finding on lines the diff adds is a comment on
              those lines, the others go in the summary.
  merge N --head SHA [--method squash|merge|rebase] [--approval ID] [--repo OWNER/NAME]
              Merge --head; refused while a check failed or still runs.
Reads, reviews, and merges with gh. review and merge wait for the owner
(--approval ID from Coder, used once, or yes typed here). The default
repository is this checkout's.";

/// What each command does, for the chat router's command tree.
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("status", Effect::ReadOnly),
    Declared::computer("diff", Effect::ReadOnly),
    Declared::computer("review", Effect::Publishes),
    Declared::computer("merge", Effect::Publishes),
];

/// The most diff text one answer carries, so it fits a tool's answer.
const DIFF_TEXT_MAX: usize = 40 * 1024;
/// The most findings one review posts.
const FINDINGS_MAX: usize = 100;

pub fn run(output: &Output, words: &[String]) -> u8 {
    if words.is_empty() || matches!(words[0].as_str(), "--help" | "-h" | "help") {
        println!("{USAGE}");
        return if words.is_empty() {
            crate::EXIT_USAGE
        } else {
            0
        };
    }
    let args = match Args::parse(words, &[]) {
        Ok(args) => args,
        Err(message) => return output.usage("pr", &message, USAGE),
    };
    let positional = args.positional();
    let (Some(action), Some(number)) = (positional.first(), positional.get(1)) else {
        return output.usage("pr", "name an action and a pull request number", USAGE);
    };
    if positional.len() > 2 {
        return output.usage("pr", &format!("unexpected word `{}`", positional[2]), USAGE);
    }
    let Ok(number) = number.trim_start_matches('#').parse::<u64>() else {
        return output.usage("pr", "N is a pull request number", USAGE);
    };
    let allowed: &[&str] = match action.as_str() {
        "status" => &["repo"],
        "diff" => &["repo", "file"],
        "review" => &["repo", "head", "findings", "verdict", "approval"],
        "merge" => &["repo", "head", "method", "approval"],
        other => return output.usage("pr", &format!("unknown action `{other}`"), USAGE),
    };
    if let Some(name) = args
        .option_names()
        .into_iter()
        .find(|name| !allowed.contains(name))
    {
        return output.usage("pr", &format!("unknown option `--{name}`"), USAGE);
    }
    let repo = args.option("repo").filter(|repo| !repo.is_empty());
    if repo.is_some_and(|repo| repo.starts_with('-') || repo.split('/').count() != 2) {
        return output.usage("pr", "--repo is OWNER/NAME", USAGE);
    }
    let pr = Pr { number, repo };
    let result = match action.as_str() {
        "status" => pr.status(),
        "diff" => pr.diff(args.option("file")),
        "review" => match (args.option("head"), args.option("findings")) {
            (Some(head), Some(file)) => pr.review(
                head,
                file,
                args.option("verdict").unwrap_or("comment"),
                args.option("approval"),
            ),
            _ => return output.usage("pr", "review needs --head and --findings", USAGE),
        },
        _ => match args.option("head") {
            Some(head) => pr.merge(
                head,
                args.option("method").unwrap_or("squash"),
                args.option("approval"),
            ),
            None => return output.usage("pr", "merge needs --head", USAGE),
        },
    };
    match result {
        Ok(value) => {
            output.emit(&value, render);
            0
        }
        Err(message) => output.fail("pr", &message),
    }
}

fn render(value: &Value) -> String {
    if let Some(diff) = value["diff"].as_str() {
        return diff.to_owned();
    }
    if value["merged"] == true {
        return format!(
            "Merged {} with {}.",
            value["subject"].as_str().unwrap_or_default(),
            value["method"].as_str().unwrap_or_default()
        );
    }
    if let Some(url) = value["review_url"].as_str() {
        return format!(
            "Posted the review ({} on lines, {} in the summary): {url}",
            value["inline"], value["in_summary"]
        );
    }
    let mut lines = vec![format!(
        "{}#{} {} ({}) at {}",
        value["repository"].as_str().unwrap_or_default(),
        value["number"],
        value["title"].as_str().unwrap_or_default(),
        value["state"].as_str().unwrap_or_default(),
        value["head"].as_str().unwrap_or_default()
    )];
    if value["can_merge"] == true {
        lines.push("It can merge now.".into());
    } else {
        for why in value["why_not"].as_array().into_iter().flatten() {
            lines.push(why.as_str().unwrap_or_default().to_owned());
        }
    }
    lines.join("\n")
}

/// One pull request.
struct Pr<'a> {
    number: u64,
    repo: Option<&'a str>,
}

/// Run `gh` with `arguments`, `input` on its stdin; its stdout, or why it
/// failed.
fn gh(arguments: &[String], input: Option<&[u8]>) -> Result<String, String> {
    let mut child = Command::new("gh")
        .args(arguments)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                "This needs the GitHub CLI (gh), signed in: https://cli.github.com".to_owned()
            } else {
                format!("gh: {error}")
            }
        })?;
    if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
        stdin
            .write_all(input)
            .map_err(|error| format!("gh: {error}"))?;
    }
    let done = child
        .wait_with_output()
        .map_err(|error| format!("gh: {error}"))?;
    if done.status.success() {
        Ok(String::from_utf8_lossy(&done.stdout).into_owned())
    } else {
        let why = String::from_utf8_lossy(&done.stderr).trim().to_owned();
        Err(if why.is_empty() {
            format!("`gh {}` failed.", arguments.join(" "))
        } else {
            why
        })
    }
}

/// `OWNER/NAME` from a pull request's URL.
fn repository_of(url: &str) -> Option<String> {
    let path = url.strip_prefix("https://github.com/")?;
    let mut parts = path.split('/');
    let (owner, name) = (parts.next()?, parts.next()?);
    (!owner.is_empty() && !name.is_empty()).then(|| format!("{owner}/{name}"))
}

/// Why a pull request can't merge now, from `gh pr view --json`'s fields;
/// empty when it can.
pub(crate) fn why_not(view: &Value) -> Vec<String> {
    let mut why = Vec::new();
    match view["state"].as_str() {
        Some("OPEN") => {}
        Some(state) => why.push(format!("It is {}.", state.to_lowercase())),
        None => why.push("Its state could not be read.".into()),
    }
    if view["isDraft"] == true {
        why.push("It is a draft.".into());
    }
    match view["mergeable"].as_str() {
        Some("CONFLICTING") => why.push("It conflicts with its base branch.".into()),
        Some("UNKNOWN") => why.push(
            "GitHub is still working out whether it merges cleanly; try again in a minute.".into(),
        ),
        _ => {}
    }
    for check in view["statusCheckRollup"].as_array().into_iter().flatten() {
        let name = check["name"]
            .as_str()
            .or_else(|| check["context"].as_str())
            .unwrap_or("a check");
        // A check run has a status and a conclusion; a commit status has a
        // state.
        let (running, failed) = if let Some(state) = check["state"].as_str() {
            (
                matches!(state, "PENDING" | "EXPECTED"),
                matches!(state, "FAILURE" | "ERROR"),
            )
        } else {
            let completed = check["status"].as_str() == Some("COMPLETED");
            let conclusion = check["conclusion"].as_str().unwrap_or_default();
            (
                !completed,
                completed && !matches!(conclusion, "SUCCESS" | "NEUTRAL" | "SKIPPED"),
            )
        };
        if failed {
            why.push(format!("The check {name} failed."));
        } else if running {
            why.push(format!("The check {name} is still running."));
        }
    }
    if why.is_empty() {
        match view["mergeStateStatus"].as_str() {
            Some("BLOCKED") => why.push(
                "Branch protection blocks it (a required review or check is missing).".into(),
            ),
            Some("DIRTY") => why.push("It conflicts with its base branch.".into()),
            _ => {}
        }
    }
    why
}

impl Pr<'_> {
    fn with_repo(&self, mut arguments: Vec<String>) -> Vec<String> {
        if let Some(repo) = self.repo {
            arguments.extend(["--repo".to_owned(), repo.to_owned()]);
        }
        arguments
    }

    fn status(&self) -> Result<Value, String> {
        let text = gh(
            &self.with_repo(vec![
                "pr".into(),
                "view".into(),
                self.number.to_string(),
                "--json".into(),
                "number,title,state,isDraft,url,headRefOid,baseRefName,mergeable,\
                 mergeStateStatus,reviewDecision,statusCheckRollup"
                    .into(),
            ]),
            None,
        )?;
        let view: Value = serde_json::from_str(&text)
            .map_err(|_| "gh answered something other than the pull request's JSON.")?;
        let repository = self
            .repo
            .map(str::to_owned)
            .or_else(|| view["url"].as_str().and_then(repository_of))
            .ok_or("The pull request's repository could not be read.")?;
        let why = why_not(&view);
        let checks: Vec<Value> = view["statusCheckRollup"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|check| {
                json!({
                    "name": check["name"].as_str().or_else(|| check["context"].as_str()),
                    "status": check["state"].as_str().or_else(|| check["conclusion"]
                        .as_str()
                        .filter(|c| !c.is_empty()))
                        .or_else(|| check["status"].as_str()),
                })
            })
            .collect();
        Ok(json!({
            "repository": repository,
            "number": self.number,
            "title": view["title"],
            "state": view["state"],
            "draft": view["isDraft"],
            "url": view["url"],
            "head": view["headRefOid"],
            "base": view["baseRefName"],
            "review_decision": view["reviewDecision"],
            "checks": checks,
            "can_merge": why.is_empty(),
            "why_not": why,
        }))
    }

    /// The pull request's diff text and its head, read so the head did not
    /// move in between.
    fn pinned_diff(&self) -> Result<(Value, String), String> {
        let before = self.status()?;
        let diff = gh(
            &self.with_repo(vec!["pr".into(), "diff".into(), self.number.to_string()]),
            None,
        )?;
        let after = self.status()?;
        if before["head"] != after["head"] {
            return Err("The pull request moved while its diff was read; read it again.".into());
        }
        Ok((after, diff))
    }

    fn diff(&self, file: Option<&str>) -> Result<Value, String> {
        let (status, diff) = self.pinned_diff()?;
        let files = parse_diff(&diff);
        let listed: Vec<Value> = files
            .iter()
            .map(|file| json!({"path": file.path, "added": file.added}))
            .collect();
        let mut text = String::new();
        let mut truncated = false;
        for section in files
            .iter()
            .filter(|section| file.is_none_or(|path| section.path == path))
        {
            if text.len() + section.text.len() > DIFF_TEXT_MAX {
                truncated = true;
                break;
            }
            text.push_str(&section.text);
            truncated |= section.truncated;
        }
        if let Some(path) = file
            && !files.iter().any(|section| section.path == path)
        {
            return Err(format!("The pull request does not change {path}."));
        }
        Ok(json!({
            "repository": status["repository"],
            "number": self.number,
            "title": status["title"],
            "head": status["head"],
            "base": status["base"],
            "files": listed,
            "diff": text,
            "truncated": truncated,
            "note": if truncated {
                "The diff was cut to fit; read one file at a time with --file PATH."
            } else {
                ""
            },
        }))
    }

    fn review(
        &self,
        head: &str,
        file: &str,
        verdict: &str,
        approval: Option<&str>,
    ) -> Result<Value, String> {
        let event = match verdict {
            "comment" => "COMMENT",
            "approve" => "APPROVE",
            "request_changes" => "REQUEST_CHANGES",
            _ => return Err("--verdict is comment, approve, or request_changes.".into()),
        };
        let document: ReviewFile = serde_json::from_slice(
            &std::fs::read(file).map_err(|error| format!("{file}: {error}"))?,
        )
        .map_err(|error| format!("{file} is not a findings document: {error}"))?;
        if document.findings.len() > FINDINGS_MAX {
            return Err(format!("A review posts at most {FINDINGS_MAX} findings."));
        }
        let (status, diff) = self.pinned_diff()?;
        let repository = status["repository"].as_str().unwrap_or_default().to_owned();
        let current = status["head"].as_str().unwrap_or_default();
        if current != head {
            return Err(format!(
                "The pull request moved to {current} since its diff was read at {head}; read it \
                 again before reviewing."
            ));
        }
        let subject = format!("{repository}#{}@{head}", self.number);
        let policy = approval_gate::policy_here()?;
        let opened = approval_gate::open(
            &policy,
            risk_policy::PR_REVIEW,
            &subject,
            approval,
            &format!("Post this review on {subject}?"),
        )?;
        let request = review_request(&diff, head, &document, event);
        let body = serde_json::to_vec(&request.payload).map_err(|error| error.to_string())?;
        let answer = gh(
            &[
                "api".into(),
                "--method".into(),
                "POST".into(),
                format!("repos/{repository}/pulls/{}/reviews", self.number),
                "--input".into(),
                "-".into(),
            ],
            Some(body.as_slice()),
        )?;
        let posted: Value = serde_json::from_str(&answer).unwrap_or(Value::Null);
        Ok(json!({
            "posted": true,
            "subject": subject,
            "review_id": posted["id"],
            "review_url": posted["html_url"],
            "verdict": verdict,
            "inline": request.inline,
            "in_summary": request.in_summary,
            "approval": opened.json(),
        }))
    }

    fn merge(&self, head: &str, method: &str, approval: Option<&str>) -> Result<Value, String> {
        let flag = match method {
            "squash" => "--squash",
            "merge" => "--merge",
            "rebase" => "--rebase",
            _ => return Err("--method is squash, merge, or rebase.".into()),
        };
        let status = self.status()?;
        if status["can_merge"] != true {
            let why = status["why_not"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(" ");
            return Err(format!("Not merged: {why}"));
        }
        let current = status["head"].as_str().unwrap_or_default();
        if current != head {
            return Err(format!(
                "Not merged: the pull request moved to {current} since {head} was approved."
            ));
        }
        let repository = status["repository"].as_str().unwrap_or_default();
        let subject = format!("{repository}#{}@{head}", self.number);
        let policy = approval_gate::policy_here()?;
        let opened = approval_gate::open(
            &policy,
            risk_policy::PR_MERGE,
            &subject,
            approval,
            &format!(
                "Merge {subject} (\"{}\") with {method}?",
                status["title"].as_str().unwrap_or_default()
            ),
        )?;
        gh(
            &[
                "pr".into(),
                "merge".into(),
                self.number.to_string(),
                "--repo".into(),
                repository.to_owned(),
                flag.into(),
                "--match-head-commit".into(),
                head.to_owned(),
            ],
            None,
        )?;
        Ok(json!({
            "merged": true,
            "subject": subject,
            "method": method,
            "approval": opened.json(),
        }))
    }
}

/// A review's findings file.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewFile {
    #[serde(default)]
    findings: Vec<Finding>,
    #[serde(default)]
    body: String,
}

/// The review GitHub is sent, and how its findings were placed.
struct ReviewRequest {
    payload: Value,
    inline: usize,
    in_summary: usize,
}

fn finding_text(finding: &Finding) -> String {
    let mut text = format!("**{}**: {}", finding.severity, finding.summary);
    if let Some(evidence) = finding.evidence.as_deref().filter(|e| !e.trim().is_empty()) {
        text.push_str("\n\n");
        text.push_str(evidence);
    }
    text
}

/// The review for `diff` at `head`: each finding anchored to lines the diff
/// adds is an inline comment on them, and every other finding is a line of
/// the summary, under the path (and lines) it named.
fn review_request(diff: &str, head: &str, document: &ReviewFile, event: &str) -> ReviewRequest {
    let files = parse_diff(diff);
    let scope = Scope {
        base: String::new(),
        tip: head.to_owned(),
        input_digest: String::new(),
        diff_digest: String::new(),
        diff: String::new(),
        paths: files.iter().map(|file| file.path.clone()).collect(),
        excluded: Vec::new(),
        files,
    };
    let mut comments = Vec::new();
    let mut summary = Vec::new();
    for finding in &document.findings {
        match (&finding.span, scope.anchor(finding)) {
            (Some(span), Anchor::Anchored) => {
                let mut comment = json!({
                    "path": finding.path,
                    "line": span.end,
                    "side": "RIGHT",
                    "body": finding_text(finding),
                });
                if span.start < span.end {
                    comment["start_line"] = json!(span.start);
                    comment["start_side"] = json!("RIGHT");
                }
                comments.push(comment);
            }
            (span, _) => {
                let place = match span {
                    Some(span) => format!("{}:{}-{}", finding.path, span.start, span.end),
                    None => finding.path.clone(),
                };
                summary.push(format!("- `{place}` {}", finding_text(finding)));
            }
        }
    }
    let mut body = document.body.trim().to_owned();
    if !summary.is_empty() {
        if !body.is_empty() {
            body.push_str("\n\n");
        }
        body.push_str(&summary.join("\n"));
    }
    let (inline, in_summary) = (comments.len(), summary.len());
    ReviewRequest {
        payload: json!({
            "commit_id": head,
            "event": event,
            "body": body,
            "comments": comments,
        }),
        inline,
        in_summary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIFF: &str = "diff --git a/src/lib.rs b/src/lib.rs
index 1..2 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,2 +1,4 @@
 fn a() {}
+fn b() {}
+fn c() {}
 fn d() {}
";

    #[test]
    fn findings_on_added_lines_are_inline_and_the_rest_go_in_the_summary() {
        let document: ReviewFile = serde_json::from_value(json!({
            "body": "Two notes.",
            "findings": [
                {"path":"src/lib.rs","span":{"start":2,"end":3},"severity":"high","summary":"b and c clash"},
                {"path":"src/lib.rs","span":{"start":1,"end":1},"severity":"low","summary":"context line"},
                {"path":"README.md","severity":"low","summary":"not in the diff"}
            ]
        }))
        .unwrap();
        let request = review_request(DIFF, "abc123", &document, "COMMENT");
        assert_eq!((request.inline, request.in_summary), (1, 2));
        let payload = &request.payload;
        assert_eq!(payload["commit_id"], "abc123");
        assert_eq!(payload["comments"][0]["path"], "src/lib.rs");
        assert_eq!(payload["comments"][0]["start_line"], 2);
        assert_eq!(payload["comments"][0]["line"], 3);
        let body = payload["body"].as_str().unwrap();
        assert!(body.starts_with("Two notes."));
        assert!(body.contains("README.md") && body.contains("src/lib.rs:1-1"));
    }

    #[test]
    fn a_failed_or_running_check_refuses_the_merge() {
        let passing = json!({"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE",
            "mergeStateStatus":"CLEAN","statusCheckRollup":[
                {"__typename":"CheckRun","name":"tests","status":"COMPLETED","conclusion":"SUCCESS"},
                {"__typename":"StatusContext","context":"ci/lint","state":"SUCCESS"}]});
        assert!(why_not(&passing).is_empty());
        let failed = json!({"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE",
            "statusCheckRollup":[
                {"name":"tests","status":"COMPLETED","conclusion":"FAILURE"},
                {"name":"build","status":"IN_PROGRESS","conclusion":""},
                {"context":"ci/lint","state":"ERROR"}]});
        let why = why_not(&failed);
        assert!(why.iter().any(|w| w.contains("tests failed")), "{why:?}");
        assert!(why.iter().any(|w| w.contains("build is still running")));
        assert!(why.iter().any(|w| w.contains("ci/lint failed")));
        let closed = json!({"state":"MERGED","isDraft":false,"statusCheckRollup":[]});
        assert!(!why_not(&closed).is_empty());
        assert_eq!(
            repository_of("https://github.com/acme/app/pull/7").as_deref(),
            Some("acme/app")
        );
    }
}
