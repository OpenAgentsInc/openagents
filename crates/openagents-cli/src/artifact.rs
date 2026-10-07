//! `openagents artifact`: the serialized queue for single-digest
//! artifacts such as the Everglade pack (`coder_lease::artifact`, #10763).
//! `docs/coder/runtime/artifact-queue.md` is the guide.

#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};
use coder_lease::artifact::{self, Options, Outcome, Queue, Status, Submission};
use coder_lease::{Broker, Session};
use serde_json::{Value, json};

use crate::Output;
use crate::argv::parse_command;
use crate::out::{EXIT_FAILURE, date, table};

pub(crate) const USAGE: &str = "usage: openagents artifact COMMAND [OPTIONS]
  submit NAME [--branch B] [--summary TEXT] [--no-run]
        Queue the commits on branch B (the checked-out branch by default)
        that origin/main lacks as a change to artifact NAME, then run the
        queue unless another process runs it. --summary names the change
        in the repin commit (\"Repin the Everglade pack with TEXT\"); the
        branch name by default. --no-run only queues it.
  queue [NAME] [--all]
        The pending changes, oldest first: artifact, identifier, branch,
        summary, session, and age. --all adds the landed and rejected ones,
        with each rejection's reason. Bare `openagents artifact` is queue.
  run NAME
        Run NAME's queue now, under the exclusive artifact/NAME lease:
        fetch origin/main into a scratch worktree, apply each pending change
        in order with the artifact's pinned files left out, regenerate the
        pin once, run the check, commit the repin, and push. A change that
        conflicts or fails the check comes back with its reason; the rest
        land. Exits 1 when a change was rejected.
NAME is an entry in the repository's artifacts/ registry, such as
everglade-pack or grid-pack, which names its regenerate and check commands
and its pinned files. Run the commands inside a checkout of the repository.
The queue lives under the lease root (~/.openagents/leases/artifacts/NAME/,
OPENAGENTS_LEASE_ROOT moves it), with the scratch worktree beside it.";

/// What each command does, for the chat router's command tree
/// (`coder::cli_route::tree`).
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("submit", Effect::Publishes),
    Declared::computer("queue", Effect::ReadOnly),
    Declared::computer("run", Effect::Publishes),
];

pub fn run(output: &Output, words: &[String]) -> u8 {
    let (command, rest) = match words.split_first() {
        Some((first, rest)) if !first.starts_with('-') => (first.as_str(), rest),
        _ => ("queue", words),
    };
    if matches!(command, "help" | "-h" | "--help") {
        println!("{USAGE}");
        return 0;
    }
    let broker = match Broker::from_env() {
        Ok(broker) => broker,
        Err(error) => return output.fail("artifact", &error.to_string()),
    };
    let checkout = std::env::current_dir().unwrap_or_else(|_| ".".into());
    let options = Options::new(checkout);
    match command {
        "submit" => {
            let args =
                match parse_command(rest, "submit", &["branch", "summary"], &["no-run"], 1, 1) {
                    Ok(args) => args,
                    Err(message) => return output.usage("artifact", &message, USAGE),
                };
            let name = &args.positional()[0];
            let session =
                Session::detect(&|name| std::env::var(name).ok(), &coder_lease::ancestors).id;
            let submission = match artifact::submit(
                broker.root(),
                name,
                args.option("branch"),
                args.option("summary"),
                &session,
                &options,
            ) {
                Ok(submission) => submission,
                Err(message) => return output.fail("artifact submit", &message),
            };
            if args.switch("no-run") {
                output.emit(&json!({"submitted": row(&submission)}), |_| {
                    format!(
                        "queued {} for {name}; `openagents artifact run {name}` applies it",
                        submission.id
                    )
                });
                return 0;
            }
            run_queue(output, &broker, name, &options, Some(&submission))
        }
        "queue" | "list" => {
            let args = match parse_command(rest, "queue", &[], &["all"], 0, 1) {
                Ok(args) => args,
                Err(message) => return output.usage("artifact", &message, USAGE),
            };
            list(
                output,
                &broker,
                args.positional().first(),
                args.switch("all"),
            )
        }
        "run" => {
            let args = match parse_command(rest, "run", &[], &[], 1, 1) {
                Ok(args) => args,
                Err(message) => return output.usage("artifact", &message, USAGE),
            };
            run_queue(output, &broker, &args.positional()[0], &options, None)
        }
        other => output.usage(
            "artifact",
            &format!("unknown command `{other}`; use submit, queue, or run"),
            USAGE,
        ),
    }
}

/// Runs the queue and reports it; with `submitted`, also where that
/// submission stands.
fn run_queue(
    output: &Output,
    broker: &Broker,
    name: &str,
    options: &Options,
    submitted: Option<&Submission>,
) -> u8 {
    let outcome = match artifact::run(broker, name, options) {
        Ok(outcome) => outcome,
        Err(message) => return output.fail("artifact run", &message),
    };
    let mine = submitted.and_then(|submission| {
        Queue::new(broker.root(), name)
            .get(&submission.id)
            .ok()
            .flatten()
    });
    let (value, failed) = match &outcome {
        Outcome::Busy { session, pid } => (
            json!({
                "artifact": name,
                "busy": {"session": session, "pid": pid},
                "submitted": mine.as_ref().map(row),
            }),
            false,
        ),
        Outcome::Ran { landed, rejected } => (
            json!({
                "artifact": name,
                "landed": landed.iter().map(row).collect::<Vec<_>>(),
                "rejected": rejected.iter().map(row).collect::<Vec<_>>(),
                "submitted": mine.as_ref().map(row),
            }),
            match &mine {
                Some(mine) => mine.status == Status::Rejected,
                None => !rejected.is_empty(),
            },
        ),
    };
    output.emit(&value, |value| render_run(value, name));
    if failed { EXIT_FAILURE } else { 0 }
}

fn render_run(value: &Value, name: &str) -> String {
    let mut lines = Vec::new();
    if let Some(busy) = value.get("busy") {
        lines.push(format!(
            "artifact/{name} is held by session {} (process {}); that run applies the pending changes. See them with `openagents artifact queue {name}`.",
            busy["session"].as_str().unwrap_or("?"),
            busy["pid"]
        ));
    }
    for item in value["landed"].as_array().into_iter().flatten() {
        lines.push(format!(
            "landed {} ({}) in {}",
            item["summary"].as_str().unwrap_or(""),
            item["branch"].as_str().unwrap_or(""),
            item["landed"]
                .as_str()
                .unwrap_or("")
                .get(..12)
                .unwrap_or("")
        ));
    }
    for item in value["rejected"].as_array().into_iter().flatten() {
        lines.push(format!(
            "rejected {} ({}): {}",
            item["summary"].as_str().unwrap_or(""),
            item["branch"].as_str().unwrap_or(""),
            item["reason"].as_str().unwrap_or("")
        ));
    }
    if value.get("busy").is_none()
        && value["landed"].as_array().is_none_or(Vec::is_empty)
        && value["rejected"].as_array().is_none_or(Vec::is_empty)
    {
        lines.push(format!("nothing is pending for {name}"));
    }
    lines.join("\n")
}

fn list(output: &Output, broker: &Broker, name: Option<&String>, all: bool) -> u8 {
    let queues = match name {
        Some(name) => vec![Queue::new(broker.root(), name)],
        None => match artifact::queues(broker.root()) {
            Ok(queues) => queues,
            Err(message) => return output.fail("artifact queue", &message),
        },
    };
    let mut rows = Vec::new();
    for queue in queues {
        let submissions = match queue.list() {
            Ok(submissions) => submissions,
            Err(message) => return output.fail("artifact queue", &message),
        };
        rows.extend(
            submissions
                .iter()
                .filter(|submission| all || submission.status == Status::Pending)
                .map(row),
        );
    }
    output.emit(&json!({"changes": rows}), render_list);
    0
}

fn render_list(value: &Value) -> String {
    let changes = value["changes"].as_array().cloned().unwrap_or_default();
    if changes.is_empty() {
        return "no changes are pending".to_owned();
    }
    let mut rows = vec![
        [
            "ARTIFACT",
            "ID",
            "STATUS",
            "BRANCH",
            "SUMMARY",
            "SESSION",
            "SUBMITTED",
        ]
        .map(str::to_owned)
        .to_vec(),
    ];
    let mut reasons = Vec::new();
    for change in &changes {
        let text = |key: &str| change[key].as_str().unwrap_or("").to_owned();
        rows.push(vec![
            text("artifact"),
            text("id"),
            text("status"),
            text("branch"),
            text("summary"),
            text("session"),
            date(change["submitted_at_ms"].as_u64().unwrap_or(0) / 1_000),
        ]);
        if let Some(reason) = change["reason"].as_str() {
            reasons.push(format!("{}: {reason}", text("id")));
        }
    }
    let mut text = table(&rows);
    for reason in reasons {
        text.push_str(&format!("\n{reason}"));
    }
    text
}

fn row(submission: &Submission) -> Value {
    json!({
        "artifact": submission.artifact,
        "id": submission.id,
        "status": submission.status.as_str(),
        "branch": submission.branch,
        "commit": submission.commit,
        "summary": submission.summary,
        "session": submission.session,
        "submitted_at_ms": submission.submitted_at_ms,
        "reason": submission.reason,
        "landed": submission.landed,
    })
}
