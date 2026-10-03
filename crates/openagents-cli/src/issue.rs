//! `openagents issue`: the claim record every path shares (#10203,
//! [`coder::claim`]), for agents that are not Coder's own flows. A claim
//! here is the same comment marker Coder posts, the signed-in GitHub
//! user as assignee, and "In progress" on the issue's project; Coder's
//! queues, `coder-project`, and other agents read it the same way.

use coder::claim::{self, CLAIM_MARK, Gh, Hub, RELEASE_MARK};
use coder::task::issue_run::{Policy, Tracker};
use serde_json::{Value, json};

use crate::{Args, Output};
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};

pub(crate) const USAGE: &str = "usage: openagents issue COMMAND [OPTIONS]
  claim N [--repo OWNER/NAME] [--note TEXT]
        Claim issue N: a claim comment, you as assignee, and \"In progress\"
        on each project it is on.
  release N [--repo OWNER/NAME] [--note TEXT]
        Release a claim: a release comment, you off the assignees, and
        \"Ready\" (or \"Todo\") on each project it is on.
  done N [--repo OWNER/NAME]
        Move a landed issue to \"Done\" on each project it is on.
  status N [--repo OWNER/NAME]
        Whether issue N is claimed: a claim comment or \"In progress\".
  pickup [--repo OWNER/NAME] [--label LABEL]
        The open issues to pick up next, in order: the repository's project
        order, Ready status, and blockedBy when it has a project; else the
        label (coder-sized), oldest first.
The repository is the checkout here unless --repo names one; field and value
names come from .openagents/coder-issues.json (`project`).";

/// What each command above does and where the phone runs it, for the
/// chat router's command tree (`coder::cli_route::tree`).
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("claim", Effect::Publishes),
    Declared::computer("release", Effect::Publishes),
    Declared::computer("done", Effect::Publishes),
    Declared::computer("status", Effect::ReadOnly),
    Declared::computer("pickup", Effect::ReadOnly),
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
    if !matches!(
        command.as_str(),
        "claim" | "release" | "done" | "status" | "pickup"
    ) {
        return output.usage("issue", &format!("unknown command `{command}`"), USAGE);
    }
    let args = match Args::parse(rest, &[]) {
        Ok(args) => args,
        Err(message) => return output.usage("issue", &message, USAGE),
    };
    let number = || -> Result<u64, String> {
        args.positional()
            .first()
            .map(|word| word.trim_start_matches('#'))
            .ok_or_else(|| format!("`issue {command}` needs an issue number"))?
            .parse::<u64>()
            .map_err(|_| "the issue is a number, such as 10203".to_owned())
    };
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
                "claim" => {
                    let note = args.option("note").map(|note| format!("{note}\n\n"));
                    let body = format!(
                        "Claimed: an agent is working on this.\n\n{}{CLAIM_MARK} cli -->",
                        note.unwrap_or_default()
                    );
                    said(
                        "claim",
                        &repository,
                        number,
                        claim::claim(&hub, &repository, number, &body, &policy.project),
                    )
                }
                "release" => {
                    let body = format!(
                        "{} {RELEASE_MARK}",
                        args.option("note").unwrap_or("Released the claim.")
                    );
                    said(
                        "release",
                        &repository,
                        number,
                        claim::release(&hub, &repository, number, Some(&body), &policy.project),
                    )
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
