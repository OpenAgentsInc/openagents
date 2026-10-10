//! `openagents issue`: the claim record every path shares (#10203,
//! [`coder::claim`]), for agents that are not Coder's own flows. A claim
//! here is the same comment marker Coder posts, the signed-in GitHub
//! user as assignee, and "In progress" on the issue's project; Coder's
//! queues, `coder-project`, and other agents read it the same way.
//!
//! A claim is also held on this computer for the caller's agent session
//! (#10764): `claim` refuses an issue another live session holds, or that
//! a fresh claim comment from another session marks, and the marker it
//! posts names the session.

use coder::claim::{self, Gh, Hub, RELEASE_MARK};
use coder::task::issue_run::{Policy, Tracker};
use serde_json::{Value, json};

use crate::{Args, Output};
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};

pub(crate) const USAGE: &str = "usage: openagents issue COMMAND [OPTIONS]
  claim N [--repo OWNER/NAME] [--note TEXT] [--force]
        Claim issue N for this agent session: hold it on this computer, then
        a claim comment naming the session, you as assignee, and
        \"In progress\" on each project it is on. Refused while another live
        session holds it or a fresh claim comment names another session;
        --force takes it anyway.
  release N [--repo OWNER/NAME] [--note TEXT] [--force]
        Release a claim: drop this session's hold, then a release comment,
        you off the assignees, and \"Ready\" (or \"Todo\") on each project.
        Refused while another live session holds it, unless --force.
  done N [--repo OWNER/NAME]
        Move a landed issue to \"Done\" on each project it is on.
  status N [--repo OWNER/NAME]
        Whether issue N is claimed: a claim comment or \"In progress\".
  pickup [--repo OWNER/NAME] [--label LABEL]
        The open issues to pick up next, in order: the repository's project
        order, Ready status, and blockedBy when it has a project; else the
        label (coder-sized), oldest first.
  create --title TEXT [--body TEXT] [--body-file FILE] [--label NAME] [--project N] [--status NAME] [--repo OWNER/NAME]
        Open an issue (--body-file - reads standard input; --label repeats or
        takes a comma list), and put it on board N with that status.
  comment N [--body TEXT] [--body-file FILE] [--repo OWNER/NAME]
        Comment on issue N.
  close N [--reason completed|not_planned] [--comment TEXT] [--comment-file FILE] [--project N] [--repo OWNER/NAME]
        Close issue N, after the comment when given, and move it to \"Done\"
        on every open board it is on (only board N with --project).
  reopen N [--comment TEXT] [--comment-file FILE] [--repo OWNER/NAME]
        Reopen issue N.
  list [--state open|closed|all] [--label NAME] [--limit N] [--repo OWNER/NAME]
        The repository's issues, newest first (30 unless --limit).
  view N [--repo OWNER/NAME]
        Issue N with its body and comments.
The repository is the checkout here unless --repo names one; field and value
names come from .openagents/coder-issues.json (`project`). create, comment,
close, reopen, list, and view use GH_TOKEN, GITHUB_TOKEN, or the GitHub CLI's
sign-in; `openagents project` lists and moves board items.";

/// What each command above does and where the phone runs it, for the
/// chat router's command tree (`coder::cli_route::tree`).
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("claim", Effect::Publishes),
    Declared::computer("release", Effect::Publishes),
    Declared::computer("done", Effect::Publishes),
    Declared::computer("status", Effect::ReadOnly),
    Declared::computer("pickup", Effect::ReadOnly),
    Declared::computer("create", Effect::Publishes),
    Declared::computer("comment", Effect::Publishes),
    Declared::computer("close", Effect::Publishes),
    Declared::computer("reopen", Effect::Publishes),
    Declared::computer("list", Effect::ReadOnly),
    Declared::computer("view", Effect::ReadOnly),
];

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        return output.usage("issue", "a command is required", USAGE);
    };
    if matches!(command.as_str(), "help" | "-h" | "--help") {
        println!("{USAGE}");
        return 0;
    }
    // Refuse an unknown command before anything asks gh about the checkout.
    let github_verb = crate::github_verbs::ISSUE_VERBS.contains(&command.as_str());
    if !github_verb
        && !matches!(
            command.as_str(),
            "claim" | "release" | "done" | "status" | "pickup"
        )
    {
        return output.usage("issue", &format!("unknown command `{command}`"), USAGE);
    }
    let args = match Args::parse(rest, &["force"]) {
        Ok(args) => args,
        Err(message) => return output.usage("issue", &message, USAGE),
    };
    if github_verb {
        return github(output, "issue", command, &args, USAGE);
    }
    let number = || -> Result<u64, String> {
        args.positional()
            .first()
            .map(|word| word.trim_start_matches('#'))
            .ok_or_else(|| {
                format!("N (issue number) is required for `openagents issue {command}`.")
            })?
            .parse::<u64>()
            .map_err(|_| "the issue is a number, such as 10203".to_owned())
    };
    if command != "pickup" {
        if let Err(message) = number() {
            return output.usage("issue", &message, USAGE);
        }
    }
    let (repository, policy) = match place(args.option("repo")) {
        Ok(place) => place,
        Err(message) => return output.fail("issue", &message),
    };
    let hub = Gh;
    let value = match command.as_str() {
        "claim" | "release" | "done" | "status" => {
            let number = match number() {
                Ok(number) => number,
                Err(message) => return output.usage("issue", &message, USAGE),
            };
            match command.as_str() {
                "claim" | "release" => {
                    let leases = match coder_lease::root_from_env() {
                        Ok(root) => root,
                        Err(message) => return output.fail("issue", &message),
                    };
                    let claimant = coder_lease::claims::Claimant::detect();
                    let done = hold(
                        &hub,
                        &leases,
                        command == "claim",
                        &repository,
                        number,
                        &policy,
                        &claimant,
                        args.option("note"),
                        unix_now(),
                        args.switch("force"),
                    );
                    match done {
                        Ok(lines) => said(command, &repository, number, lines),
                        Err(message) => return output.fail("issue", &message),
                    }
                }
                "done" => said(
                    "done",
                    &repository,
                    number,
                    claim::done(&hub, &repository, number, &policy.project),
                ),
                _ => match status(&hub, &repository, number, &policy) {
                    Ok(value) => value,
                    Err(message) => return output.fail("issue", &message),
                },
            }
        }
        "pickup" => match claim::pickup(&hub, &repository, &policy.project, args.option("label")) {
            Ok((issues, from)) => json!({
                "command": "pickup",
                "repository": repository,
                "issues": issues,
                "order": from,
            }),
            Err(message) => return output.fail("issue", &message),
        },
        _ => return output.usage("issue", "unknown command", USAGE),
    };
    output.emit(&value, render);
    0
}

/// `openagents project COMMAND`.
pub fn project(output: &Output, words: &[String]) -> u8 {
    let usage = crate::github_verbs::PROJECT_USAGE;
    let Some((command, rest)) = words.split_first() else {
        return output.usage("project", "a command is required", usage);
    };
    if matches!(command.as_str(), "help" | "-h" | "--help") {
        println!("{usage}");
        return 0;
    }
    if !matches!(command.as_str(), "list" | "add" | "move") {
        return output.usage("project", &format!("unknown command `{command}`"), usage);
    }
    match Args::parse(rest, &["every-repo"]) {
        Ok(args) => github(output, "project", command, &args, usage),
        Err(message) => output.usage("project", &message, usage),
    }
}

/// Runs a [`crate::github_verbs`] command of `group` (`issue` or
/// `project`) with this computer's GitHub sign-in.
fn github(output: &Output, group: &str, command: &str, args: &Args, usage: &str) -> u8 {
    let (repository, policy) = match place(args.option("repo")) {
        Ok(place) => place,
        Err(message) => return output.fail(group, &message),
    };
    let rest = match crate::github_verbs::TokenRest::here() {
        Ok(rest) => rest,
        Err(message) => return output.fail(group, &message),
    };
    let done = if group == "project" {
        crate::github_verbs::project(&rest, command, args, &repository, &policy)
    } else {
        crate::github_verbs::issue(&rest, command, args, &repository, &policy)
    };
    match done {
        Ok(value) => {
            output.emit(&value, crate::github_verbs::render);
            0
        }
        Err(message) if message.starts_with("the issue number is required") => {
            output.usage(group, &message, usage)
        }
        Err(message) => output.fail(group, &message),
    }
}

/// The repository and its policy: `--repo`, or the checkout here.
fn place(named: Option<&str>) -> Result<(String, Policy), String> {
    let here = std::env::current_dir().map_err(|_| "this command has no working directory")?;
    let top = coder::task::local::checkout(&here)
        .ok()
        .map(|checkout| checkout.top);
    let policy = match &top {
        Some(top) => Policy::load(top)?,
        None => Policy::default(),
    };
    let repository = match (named, &top) {
        (Some(named), _) => named.to_owned(),
        (None, Some(top)) => Gh.repository(top).map_err(|_| {
            "This checkout's origin isn't a GitHub repository; pass --repo OWNER/NAME.".to_owned()
        })?,
        (None, None) => {
            return Err("run this in a checkout of the repository, or pass --repo".into());
        }
    };
    Ok((repository, policy))
}

/// Claims (`take`) or releases issue `number` for `claimant` through
/// [`claim::take`] and [`claim::give_back`].
#[allow(clippy::too_many_arguments)]
fn hold<H: Hub + ?Sized>(
    hub: &H,
    leases: &std::path::Path,
    take: bool,
    repository: &str,
    number: u64,
    policy: &Policy,
    claimant: &coder_lease::claims::Claimant,
    note: Option<&str>,
    now: u64,
    force: bool,
) -> Result<Vec<String>, String> {
    let hours = policy.claim_hours;
    if take {
        let note = note.map(|note| format!("{note}\n\n")).unwrap_or_default();
        let body = format!(
            "Claimed: an agent is working on this.\n\n{note}{}",
            claim::marker("cli", &claimant.session)
        );
        claim::take(
            hub,
            leases,
            repository,
            number,
            &body,
            &policy.project,
            claimant,
            now,
            hours,
            force,
        )
    } else {
        let body = format!("{} {RELEASE_MARK}", note.unwrap_or("Released the claim."));
        claim::give_back(
            hub,
            leases,
            repository,
            number,
            Some(&body),
            &policy.project,
            &claimant.session,
            now,
            hours,
            force,
        )
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

fn status(hub: &Gh, repository: &str, number: u64, policy: &Policy) -> Result<Value, String> {
    let comments = hub.comments(repository, number)?;
    let items = hub.items(repository, number, &policy.project.field)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let held = claim::held(
        number,
        &comments,
        &items,
        now,
        policy.claim_hours,
        &policy.project,
    );
    Ok(json!({
        "command": "status",
        "repository": repository,
        "issue": number,
        "claimed": held.is_some(),
        "why": held,
        "projects": items
            .iter()
            .map(|item| json!({"project": item.project_title, "status": item.status}))
            .collect::<Vec<_>>(),
    }))
}

fn said(command: &str, repository: &str, number: u64, lines: Vec<String>) -> Value {
    json!({
        "command": command,
        "repository": repository,
        "issue": number,
        "said": lines,
    })
}

fn render(value: &Value) -> String {
    match value["command"].as_str() {
        Some("pickup") => {
            let issues: Vec<String> = value["issues"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_u64)
                .map(|number| format!("#{number}"))
                .collect();
            format!(
                "{} ({}): {}",
                value["repository"].as_str().unwrap_or(""),
                value["order"].as_str().unwrap_or(""),
                if issues.is_empty() {
                    "nothing to pick up".to_owned()
                } else {
                    issues.join(", ")
                }
            )
        }
        Some("status") => {
            let mut text = match value["why"].as_str() {
                Some(why) => format!("Claimed: {why}."),
                None => format!("#{} is not claimed.", value["issue"]),
            };
            for project in value["projects"].as_array().into_iter().flatten() {
                text.push_str(&format!(
                    "\nOn \"{}\": {}",
                    project["project"].as_str().unwrap_or(""),
                    project["status"].as_str().unwrap_or("no status")
                ));
            }
            text
        }
        _ => value["said"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder::claim::fake::Fake;
    use coder_lease::claims::Claimant;

    fn session(id: &str, pid: Option<u32>) -> Claimant {
        Claimant {
            session: id.into(),
            agent: "claude-code".into(),
            pid,
        }
    }

    #[test]
    fn claim_and_release_hold_the_issue_for_the_session() {
        let leases = tempfile::tempdir().unwrap();
        let root = leases.path();
        let github = Fake::plain("octo");
        github.issue(5, None, &[], &[]);
        let policy = Policy::default();
        let repo = "acme/app";
        let a = session("claude-code:a", Some(std::process::id()));
        let b = session("claude-code:b", None);
        let now = github.now;
        let said = hold(
            &github,
            root,
            true,
            repo,
            5,
            &policy,
            &a,
            Some("why"),
            now,
            false,
        )
        .unwrap();
        assert!(said[0].contains("session claude-code:a"), "{said:?}");
        let comment = &github.state(5).comments[0].body;
        assert!(comment.contains("why") && comment.contains("cli session=claude-code:a -->"));
        let refused = hold(&github, root, true, repo, 5, &policy, &b, None, now, false);
        assert!(refused.unwrap_err().contains("session claude-code:a"));
        assert!(hold(&github, root, false, repo, 5, &policy, &b, None, now, false).is_err());
        hold(&github, root, false, repo, 5, &policy, &a, None, now, false).unwrap();
        hold(&github, root, true, repo, 5, &policy, &b, None, now, false).unwrap();
    }
}
