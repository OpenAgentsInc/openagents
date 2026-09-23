//! `coder-one proposal run|issue`: an approved proposal's measurement,
//! and a code proposal's drafted issue.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::{
    Kind, MINITASK_FILE, MiniSpec, POLICY_FILE, RESULT_FILE, RESULT_SCHEMA, load, stage, unapproved,
};
use crate::policy::Manifest;

/// The proposal commands' usage.
pub const USAGE: &str = "\
usage: coder-one proposal run ID [--live --quota-usd USD [--task TASK]...
                                  --artifact PATH [--profile ID] [--base-arm AGENT]
                                  [--min-free-disk-gb N] [--without-claude] [--plan]]
                                 [--dir DIR] [--repo DIR] [--json]
       coder-one proposal issue ID [--dir DIR]

A proposal comes from a `coder-one ask` answer, and runs only after a person
approves it with `gym coder proposals approve ID` or from the Gym terminal.

run materializes the approved change and runs its mini stage: for a policy
or check, each cited task's mini-task with its good and bad scripts under
the base manifest and the proposal's, in seconds and with no model; for a
mini-task, the grader on the good and bad candidates. With --live, a policy
or check whose mini stage didn't regress then starts a targeted
`tbench experiment` from bench/terminal-bench: the base manifest's agent
profile against the proposal's manifest, 3 attempts per task, on the tasks
the proposal expects to change and never others. --task narrows them. The
job profile is the one the source runs' job names start with, such as
panel, when they share one; otherwise tb4, and --profile names another.
--quota-usd is required and budgets the Claude quota. --artifact names the
Coder One build both arms run, such as ./scripts/build-coder-one-linux.sh
prints. --without-claude runs both arms with their Claude Code handoff
removed, as experiment ID-codex, so no trial waits for a host-wide Claude
slot. --plan prints the schedule and checks credentials without starting a
trial.

issue prints a questions or code proposal's drafted issue, for a person to
file. Proposals are read from ~/.openagents/coder-one/proposals unless --dir
names another directory; the result is written to result.json beside the
proposal, and `gym coder proposals ID` shows the finding, the change, and
the result.";

/// Runs a proposal command and returns the exit code.
///
/// # Errors
///
/// Returns a message for bad arguments or a proposal that can't run.
pub async fn command(args: &[String]) -> Result<i32, String> {
    let Some((verb, rest)) = args.split_first() else {
        return Err(USAGE.to_string());
    };
    let mut positional = Vec::new();
    let mut dir = None;
    let mut repo = None;
    let mut live = false;
    let mut quota = None;
    let mut tasks = Vec::new();
    let mut profile = None;
    let mut base_arm = None;
    let mut disk = None;
    let mut plan_only = false;
    let mut artifact = None;
    let mut no_claude = false;
    let mut json_out = false;
    let mut iter = rest.iter();
    while let Some(arg) = iter.next() {
        let mut value = |name: &str| {
            iter.next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match arg.as_str() {
            "--dir" => dir = Some(PathBuf::from(value("--dir")?)),
            "--repo" => repo = Some(PathBuf::from(value("--repo")?)),
            "--live" => live = true,
            "--quota-usd" => {
                quota = Some(
                    value("--quota-usd")?
                        .parse::<f64>()
                        .map_err(|_| "--quota-usd takes dollars")?,
                );
            }
            "--task" => tasks.push(value("--task")?),
            "--profile" => profile = Some(value("--profile")?),
            "--base-arm" => base_arm = Some(value("--base-arm")?),
            "--min-free-disk-gb" => {
                disk = Some(
                    value("--min-free-disk-gb")?
                        .parse::<f64>()
                        .map_err(|_| "--min-free-disk-gb takes a number")?,
                );
            }
            "--plan" => plan_only = true,
            "--without-claude" => no_claude = true,
            "--artifact" => artifact = Some(PathBuf::from(value("--artifact")?)),
            "--json" => json_out = true,
            "--help" | "-h" | "help" => {
                println!("{USAGE}");
                return Ok(0);
            }
            other if other.starts_with("--") => return Err(format!("unknown option {other}")),
            other => positional.push(other.to_string()),
        }
    }
    let root = dir
        .or_else(super::default_dir)
        .ok_or("no --dir and no HOME to read proposals from")?;
    let [id] = positional.as_slice() else {
        return Err(format!("proposal {verb} needs one proposal ID\n{USAGE}"));
    };
    let (proposal_dir, proposal) = load(&root, id)?;
    match verb.as_str() {
        "issue" => {
            let draft = &proposal["issue_draft"];
            if draft.is_null() {
                return Err(format!(
                    "the {} proposal {} runs as a measurement and has no drafted issue",
                    proposal["kind"].as_str().unwrap_or("?"),
                    proposal["id"].as_str().unwrap_or("?")
                ));
            }
            println!(
                "{}\n\n{}",
                draft["title"].as_str().unwrap_or(""),
                draft["body"].as_str().unwrap_or("")
            );
            Ok(0)
        }
        "run" => {
            let repo = match repo {
                Some(repo) => repo,
                None => std::env::current_dir().map_err(|error| error.to_string())?,
            };
            let live = if live {
                let quota = quota.ok_or("--live needs --quota-usd: the Claude quota budget")?;
                if quota <= 0.0 {
                    return Err("--quota-usd must be positive".to_string());
                }
                let expected: Vec<String> = proposal["expected_tasks"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect();
                if let Some(extra) = tasks.iter().find(|t| !expected.contains(t)) {
                    return Err(format!(
                        "--task {extra} isn't a task the proposal expects to change ({}); a live \
                         stage never widens past them",
                        expected.join(", ")
                    ));
                }
                Some(stage::Live {
                    repo: repo.clone(),
                    profile: profile
                        .clone()
                        .unwrap_or_else(|| stage::profile_of(&repo, &proposal)),
                    quota_usd: quota,
                    tasks: if tasks.is_empty() { expected } else { tasks },
                    base_arm,
                    min_free_disk_gb: disk,
                    plan_only,
                    without_claude: no_claude,
                    artifact: match artifact {
                        Some(path) => {
                            let sha = stage::sha256_file(&path)?;
                            Some((path, sha))
                        }
                        None => None,
                    },
                })
            } else {
                None
            };
            let result = run(&proposal_dir, &proposal, live.as_ref()).await?;
            if json_out {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&result).map_err(|error| error.to_string())?
                );
            } else {
                print!("{}", text(&proposal, &result));
            }
            let regressed = result["mini"]["verdict"] == "regressed"
                || result["mini"]["verdict"] == "does-not-reproduce";
            Ok(i32::from(regressed))
        }
        _ => Err(USAGE.to_string()),
    }
}

fn read_json(path: &Path) -> Result<Value, String> {
    serde_json::from_str(
        &std::fs::read_to_string(path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?,
    )
    .map_err(|error| format!("{} isn't JSON: {error}", path.display()))
}

/// Runs an approved proposal's mini stage and, with `live`, starts its
/// live stage. Writes and returns the result.
///
/// # Errors
///
/// Returns a message when the proposal isn't approved, needs code, or a
/// stage can't start.
pub async fn run(
    dir: &Path,
    proposal: &Value,
    live: Option<&stage::Live>,
) -> Result<Value, String> {
    if let Some(why) = unapproved(dir, proposal) {
        return Err(format!("the proposal doesn't run: {why}"));
    }
    let kind = Kind::parse(proposal["kind"].as_str().unwrap_or_default())?;
    if kind.needs_code() {
        return Err(format!(
            "a {} proposal needs code, so nothing runs; `coder-one proposal issue {}` prints its drafted issue",
            kind.word(),
            proposal["id"].as_str().unwrap_or("ID")
        ));
    }
    let out = dir.join("mini");
    let mini = match kind {
        Kind::Policy | Kind::Check => {
            let base_file = proposal["materialized"]["base_file"]
                .as_str()
                .ok_or("the proposal has no base manifest")?;
            let base = Manifest::parse(
                &std::fs::read_to_string(base_file)
                    .map_err(|error| format!("cannot read {base_file}: {error}"))?,
            )?;
            if base.digest()
                != proposal["materialized"]["base_digest"]
                    .as_str()
                    .unwrap_or("")
            {
                return Err(format!(
                    "{base_file} changed since the proposal was written; its digest no longer \
                     matches, so ask again"
                ));
            }
            let candidate: Manifest = serde_json::from_value(read_json(&dir.join(POLICY_FILE))?)
                .map_err(|error| format!("{POLICY_FILE} isn't a manifest: {error}"))?;
            if candidate.digest() != proposal["materialized"]["digest"].as_str().unwrap_or("") {
                return Err(format!("{POLICY_FILE} doesn't match the approved digest"));
            }
            stage::mini_policy(proposal, &base, &candidate, &out).await
        }
        Kind::Minitask => {
            let spec: MiniSpec = serde_json::from_value(read_json(&dir.join(MINITASK_FILE))?)
                .map_err(|error| format!("{MINITASK_FILE} isn't a mini-task: {error}"))?;
            stage::mini_task(&spec, &out).await?
        }
        Kind::Questions | Kind::Code => unreachable!("refused above"),
    };
    let mut result = json!({
        "schema": RESULT_SCHEMA,
        "proposal": proposal["id"],
        "proposal_digest": proposal["digest"],
        "ask": proposal["ask"]["id"],
        "mini": mini,
        "live": Value::Null,
        "updated_at": atif::document::iso(atif::now_ms()),
    });
    // A live stage already started stays recorded.
    if let Ok(previous) = read_json(&dir.join(RESULT_FILE))
        && previous["proposal_digest"] == proposal["digest"]
        && !previous["live"].is_null()
        && previous["live"]["plan_only"] != true
    {
        result["live"] = previous["live"].clone();
        result["superseded"] = previous["superseded"].clone();
    }
    let write = |result: &Value| {
        crate::record::write_atomic(
            &dir.join(RESULT_FILE),
            serde_json::to_string_pretty(result)
                .unwrap_or_default()
                .as_bytes(),
        )
    };
    write(&result)?;
    if let Some(live) = live {
        if result["mini"]["verdict"] == "regressed" {
            return Err(format!(
                "the mini stage regressed, so the live stage doesn't start: {}",
                result["mini"]["findings"]
            ));
        }
        let same = !result["live"].is_null()
            && (result["live"]["variant"]["without_claude"] == true) == live.without_claude;
        if same && !live.plan_only {
            return Err(format!(
                "the live stage already started as experiment {}; `gym terminal-bench experiment \
                 report {}` reads it, and `uv run tbench experiment run --id {}` resumes it",
                result["live"]["experiment"],
                result["live"]["experiment"],
                result["live"]["experiment"]
            ));
        }
        let started = stage::live(proposal, dir, live).await?;
        if live.plan_only {
            // A plan is shown, never recorded over a stage that started.
            result["live"] = started;
            return Ok(result);
        }
        if !result["live"].is_null() && !live.plan_only {
            // The other variant's experiment stays on record; stop it with
            // `uv run tbench experiment stop --id ID` if it shouldn't run.
            let mut superseded = result["superseded"].as_array().cloned().unwrap_or_default();
            superseded.push(result["live"].take());
            result["superseded"] = json!(superseded);
        }
        if !live.plan_only || result["live"].is_null() {
            result["live"] = started;
        }
        result["updated_at"] = json!(atif::document::iso(atif::now_ms()));
        write(&result)?;
    }
    Ok(result)
}

/// A result as text.
#[must_use]
pub fn text(proposal: &Value, result: &Value) -> String {
    let mut out = format!(
        "Proposal {} ({}): {}\n",
        proposal["id"].as_str().unwrap_or("?"),
        proposal["kind"].as_str().unwrap_or("?"),
        proposal["title"].as_str().unwrap_or("")
    );
    let mini = &result["mini"];
    out.push_str(&format!(
        "Mini stage: {} in {:.1}s. {}\n",
        mini["verdict"].as_str().unwrap_or("?"),
        mini["milliseconds"].as_f64().unwrap_or(0.0) / 1000.0,
        mini["note"].as_str().unwrap_or("")
    ));
    for row in mini["runs"].as_array().into_iter().flatten() {
        if row["arm"].is_string() {
            out.push_str(&format!(
                "  {:<22} {:<5} {:<9} {:<7} checks failed {}\n",
                row["task"].as_str().unwrap_or(""),
                row["script"].as_str().unwrap_or(""),
                row["arm"].as_str().unwrap_or(""),
                row["verdict"].as_str().unwrap_or(""),
                row["checks_failed"]
                    .as_u64()
                    .map_or("-".to_string(), |n| n.to_string())
            ));
        } else {
            out.push_str(&format!(
                "  grader on the {} candidate: {}\n",
                row["candidate"].as_str().unwrap_or(""),
                if row["passed"] == true {
                    "passed"
                } else {
                    "failed"
                }
            ));
        }
    }
    for finding in mini["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        out.push_str(&format!("  - {finding}\n"));
    }
    for limit in mini["limits"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        out.push_str(&format!("  Limit: {limit}.\n"));
    }
    let live = &result["live"];
    if !live.is_null() {
        out.push_str(&format!(
            "Live stage: experiment {} {}: {} against {} on {}, {} attempts each, quota ${}.\n  {}\n",
            live["experiment"].as_str().unwrap_or("?"),
            if live["plan_only"] == true { "planned" } else { "started" },
            live["experiment"].as_str().unwrap_or("?"),
            live["baseline"].as_str().unwrap_or("?"),
            live["tasks"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", "),
            live["attempts"],
            live["quota_usd"],
            live["report"].as_str().unwrap_or("")
        ));
    }
    out
}
