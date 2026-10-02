//! Choosing an open issue for Coder to pick up (#10206).
//!
//! When a person asks a chat to "pick one of the open issues nobody is
//! working on and take it", the router judges the message is coding work
//! and Jev judges it asks Coder to choose an issue itself
//! ([`coder_delegate::issue::Asked::Pick`]). Only then does this module
//! read the repository's open issues, a bounded record, and choose one:
//!
//! - never an issue someone holds: an assignee, a claim in the claim
//!   record ([`crate::claim::held`]: an unreleased claim comment within the
//!   repository's claim window, which any agent posts with `openagents
//!   issue claim`, or a project Status "In progress"), or an open pull
//!   request that closes it, names it, or is on a branch named for it;
//! - never one labeled as not ready for an agent (`blocked`, `umbrella`,
//!   `epic`, `needs-owner`, `question`, `wontfix`, `duplicate`);
//! - first in the repository's own pickup order ([`crate::claim::pickup`]:
//!   its project's order, a Ready status, and no open `blockedBy`, or the
//!   `coder-sized` label, oldest first); then, when none of those is free,
//!   the issues whose body names no other open issue (no dependency still
//!   open), then the shortest body, then the oldest.
//!
//! The chosen issue then runs the ordinary issue flow
//! ([`super::issue_run::Runner`]): claim, a worktree of the fetched
//! default branch, the checks, landing.

use serde_json::Value;

use super::issue_run::{Comment, Issue, Policy, Tracker, claimed};
use crate::claim::iso_seconds;

/// Labels that keep an issue out of a pickup.
pub const HELD_LABELS: &[&str] = &[
    "blocked",
    "umbrella",
    "epic",
    "needs-owner",
    "question",
    "wontfix",
    "duplicate",
];

/// An open issue as a pickup weighs it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Open {
    pub number: u64,
    pub title: String,
    pub body: String,
    pub labels: Vec<String>,
    pub assignees: Vec<String>,
    pub comments: Vec<Comment>,
}

/// An open pull request, for the issues it works.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pull {
    pub title: String,
    pub body: String,
    pub branch: String,
    /// The issues it closes when it merges.
    pub closes: Vec<u64>,
}

/// The issue a pickup chose.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picked {
    pub number: u64,
    pub title: String,
}

/// Why an open issue is not picked, or `None` when it may be.
#[must_use]
pub fn held(open: &Open, pulls: &[Pull], now: u64, claim_hours: u64) -> Option<String> {
    let number = open.number;
    if !open.assignees.is_empty() {
        return Some(format!(
            "#{number} is assigned to {}",
            open.assignees.join(", ")
        ));
    }
    if let Some(label) = open.labels.iter().find(|label| {
        HELD_LABELS
            .iter()
            .any(|held| label.eq_ignore_ascii_case(held))
    }) {
        return Some(format!("#{number} is labeled {label}"));
    }
    let issue = Issue {
        number,
        title: open.title.clone(),
        body: String::new(),
        url: String::new(),
        open: true,
        comments: open.comments.clone(),
    };
    if let Some(why) = claimed(&issue, now, claim_hours) {
        return Some(why);
    }
    if pulls.iter().any(|pull| works(pull, number)) {
        return Some(format!("#{number} has an open pull request"));
    }
    None
}

/// Whether `pull` works issue `number`: it closes it, names it, or its
/// branch is named for it.
fn works(pull: &Pull, number: u64) -> bool {
    pull.closes.contains(&number)
        || coder_delegate::issue::references(&format!("{}\n{}", pull.title, pull.body))
            .iter()
            .any(|reference| reference.number == number)
        || pull
            .branch
            .split(|c: char| !c.is_ascii_digit())
            .any(|digits| digits.parse::<u64>().ok() == Some(number))
}

/// The open issues `open`'s body names, which it may depend on.
fn open_dependencies(open: &Open, numbers: &[u64]) -> usize {
    let mut named: Vec<u64> = coder_delegate::issue::references(&open.body)
        .into_iter()
        .map(|reference| reference.number)
        .filter(|n| *n != open.number && numbers.contains(n))
        .collect();
    named.sort_unstable();
    named.dedup();
    named.len()
}

/// The free issues among `issues`, best first: those in `ordered` (the
/// repository's pickup order) in that order, then the rest by
/// independence, body length, and age. Empty, with why, when none is
/// free.
///
/// # Errors
/// Every open issue is held, or there is none.
pub fn choose(
    issues: &[Open],
    pulls: &[Pull],
    ordered: &[u64],
    now: u64,
    claim_hours: u64,
) -> Result<Vec<Picked>, String> {
    if issues.is_empty() {
        return Err("there is no open issue".into());
    }
    let numbers: Vec<u64> = issues.iter().map(|open| open.number).collect();
    let mut free: Vec<&Open> = issues
        .iter()
        .filter(|open| held(open, pulls, now, claim_hours).is_none())
        .collect();
    free.sort_by_key(|open| {
        (
            ordered
                .iter()
                .position(|n| *n == open.number)
                .unwrap_or(usize::MAX),
            open_dependencies(open, &numbers),
            open.body.len(),
            open.number,
        )
    });
    if free.is_empty() {
        return Err(format!(
            "all {} open issues are claimed, assigned, in a pull request, or held",
            issues.len()
        ));
    }
    Ok(free
        .into_iter()
        .map(|open| Picked {
            number: open.number,
            title: open.title.clone(),
        })
        .collect())
}

/// Read the repository's open issues, pull requests, and pickup order, and
/// choose one; a candidate whose project Status says another holds it is
/// passed over.
///
/// # Errors
/// GitHub cannot be read, or no issue is free.
pub fn pick(
    tracker: &dyn Tracker,
    repository: &str,
    now: u64,
    policy: &Policy,
) -> Result<Picked, String> {
    let issues = tracker.open_issues(repository)?;
    let pulls = tracker.open_pulls(repository)?;
    // An order that cannot be read leaves the fallback order.
    let ordered = crate::claim::pickup(tracker, repository, &policy.project, None)
        .map(|(numbers, _)| numbers)
        .unwrap_or_default();
    let mut said = None;
    for candidate in choose(&issues, &pulls, &ordered, now, policy.claim_hours)? {
        let items = tracker
            .items(repository, candidate.number, &policy.project.field)
            .unwrap_or_default();
        let comments = issues
            .iter()
            .find(|open| open.number == candidate.number)
            .map(|open| open.comments.clone())
            .unwrap_or_default();
        match crate::claim::held(
            candidate.number,
            &comments,
            &items,
            now,
            policy.claim_hours,
            &policy.project,
        ) {
            None => return Ok(candidate),
            Some(why) => said = Some(why),
        }
    }
    Err(said.unwrap_or_else(|| "no open issue is free".into()))
}

fn names(value: &Value, key: &str, field: &str) -> Vec<String> {
    value[key]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item[field].as_str().map(str::to_owned))
        .collect()
}

/// `gh issue list --json number,title,body,labels,assignees,comments`.
///
/// # Errors
/// The text is not that list.
pub fn parse_issues(text: &str) -> Result<Vec<Open>, String> {
    let value: Value =
        serde_json::from_str(text).map_err(|error| format!("unexpected gh output: {error}"))?;
    let list = value.as_array().ok_or("unexpected gh output: not a list")?;
    Ok(list
        .iter()
        .filter_map(|issue| {
            Some(Open {
                number: issue["number"].as_u64()?,
                title: issue["title"].as_str().unwrap_or_default().to_owned(),
                body: issue["body"].as_str().unwrap_or_default().to_owned(),
                labels: names(issue, "labels", "name"),
                assignees: names(issue, "assignees", "login"),
                comments: issue["comments"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|comment| Comment {
                        body: comment["body"].as_str().unwrap_or_default().to_owned(),
                        at: comment["createdAt"]
                            .as_str()
                            .and_then(iso_seconds)
                            .unwrap_or(0),
                    })
                    .collect(),
            })
        })
        .collect())
}

/// `gh pr list --json title,body,headRefName,closingIssuesReferences`.
///
/// # Errors
/// The text is not that list.
pub fn parse_pulls(text: &str) -> Result<Vec<Pull>, String> {
    let value: Value =
        serde_json::from_str(text).map_err(|error| format!("unexpected gh output: {error}"))?;
    let list = value.as_array().ok_or("unexpected gh output: not a list")?;
    Ok(list
        .iter()
        .map(|pull| Pull {
            title: pull["title"].as_str().unwrap_or_default().to_owned(),
            body: pull["body"].as_str().unwrap_or_default().to_owned(),
            branch: pull["headRefName"].as_str().unwrap_or_default().to_owned(),
            closes: pull["closingIssuesReferences"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|issue| issue["number"].as_u64())
                .collect(),
        })
        .collect())
}

#[cfg(test)]
#[path = "issue_pick_tests.rs"]
mod tests;
