//! Terminal-Bench evidence commands and the pinned harness entry point.

use gym::terminal_bench::{Attempt, ComparisonGroup, Evidence, Records};
use serde_json::{Value, json};
use std::env;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

const HELP: &str = "\
gym terminal-bench  inspect local runs or call the pinned Harbor harness

Evidence commands (read-only; add --json for structured output):
  overview                 list sources, statuses, coverage, and task/arm groups
  compare [--task ID] [--arm ID]  compare groups and their member attempts
  attempt JOB TRIAL        inspect one attempt and its measurements
  evidence JOB TRIAL       inspect one attempt's files and digest states
  evidence --missing       list every attempt's missing streams, artifacts, and files
  history                 list every attempt, newest first
  runbooks                list the operating documents

Source options for evidence commands:
  --jobs-dir PATH          local Harbor jobs (default ~/.openagents/terminal-bench/jobs)
  --traces-dir PATH        retained checkout traces
  --samples-dir PATH       checked and nested resilience samples
  --no-jobs | --no-traces | --no-samples  omit one source
  --json                   print versioned JSON instead of text

Harness commands (forward remaining flags to the pinned tbench package):
  doctor                  check local requirements; --smoke reaches the network
  tasks                   list tasks or run `tasks checkout`
  profiles                list job profiles
  materialize             write a pinned job config without starting Harbor
  run                     start a new pinned Harbor job
  resume                  resume a pinned Harbor job
  inspect-job JOB         show one job's trial results
  collect JOB             rebuild one job's attempt records
  report                  write the harness comparison report

  --uv PATH               uv executable (default: uv)
  --harness-dir PATH      tbench package directory (default: this checkout)

Run and resume can start containers and reach providers. Credentials come
from the harness's environment, never CLI flags. See
docs/coder/terminal-bench.md and docs/gym/terminal-bench-cli.md.";

const SCHEMA: &str = "openagents.gym.terminal-bench-cli.v1";

#[derive(Default)]
struct SourceOptions {
    jobs: Option<PathBuf>,
    traces: Option<PathBuf>,
    samples: Option<PathBuf>,
    json: bool,
    missing: bool,
    task: Option<String>,
    arm: Option<String>,
    positional: Vec<String>,
}

impl SourceOptions {
    fn defaults() -> Self {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench");
        Self {
            jobs: env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join(".openagents/terminal-bench/jobs")),
            traces: Some(repo.join("traces")),
            samples: Some(repo.join("samples")),
            ..Self::default()
        }
    }

    fn parse(command: &str, args: &[String]) -> Result<Self, String> {
        let mut options = Self::defaults();
        let mut index = 0;
        while index < args.len() {
            let argument = args[index].as_str();
            match argument {
                "--json" => options.json = true,
                "--missing" if command == "evidence" => options.missing = true,
                "--no-jobs" => options.jobs = None,
                "--no-traces" => options.traces = None,
                "--no-samples" => options.samples = None,
                "--jobs-dir" | "--traces-dir" | "--samples-dir" | "--task" | "--arm" => {
                    let value = args
                        .get(index + 1)
                        .filter(|value| !value.starts_with("--"))
                        .ok_or_else(|| format!("{argument} needs a value"))?;
                    match argument {
                        "--jobs-dir" => options.jobs = Some(value.into()),
                        "--traces-dir" => options.traces = Some(value.into()),
                        "--samples-dir" => options.samples = Some(value.into()),
                        "--task" => options.task = Some(value.clone()),
                        _ => options.arm = Some(value.clone()),
                    }
                    index += 1;
                }
                _ if argument.starts_with('-') => return Err(format!("unknown option {argument}")),
                _ => options.positional.push(args[index].clone()),
            }
            index += 1;
        }
        let expected = if options.missing {
            0
        } else if matches!(command, "attempt" | "evidence") {
            2
        } else {
            0
        };
        if options.positional.len() != expected {
            return Err(format!("{command} expects {expected} positional values"));
        }
        if command != "compare" && (options.task.is_some() || options.arm.is_some()) {
            return Err("--task and --arm apply only to compare".to_owned());
        }
        Ok(options)
    }

    fn load(&self) -> Records {
        Records::load(
            self.jobs.as_deref(),
            self.traces.as_deref(),
            self.samples.as_deref(),
        )
    }
}

pub fn run(args: Vec<String>) -> i32 {
    let mut output = io::stdout().lock();
    let mut errors = io::stderr().lock();
    match execute(&args, &mut output, &mut errors) {
        Ok(code) => code,
        Err(message) => {
            let _ = writeln!(errors, "terminal-bench: {message}");
            2
        }
    }
}

fn execute(args: &[String], out: &mut impl Write, err: &mut impl Write) -> Result<i32, String> {
    let Some((command, rest)) = args.split_first() else {
        writeln!(out, "{HELP}").map_err(|error| error.to_string())?;
        return Ok(0);
    };
    if matches!(command.as_str(), "help" | "--help" | "-h") {
        writeln!(out, "{HELP}").map_err(|error| error.to_string())?;
        return Ok(0);
    }
    if matches!(
        command.as_str(),
        "doctor"
            | "tasks"
            | "profiles"
            | "materialize"
            | "run"
            | "resume"
            | "inspect-job"
            | "collect"
            | "report"
    ) {
        return harness(command, rest, err);
    }
    if !matches!(
        command.as_str(),
        "overview" | "compare" | "attempt" | "evidence" | "history" | "runbooks"
    ) {
        return Err(format!("unknown command {command}; use --help"));
    }
    let options = SourceOptions::parse(command, rest)?;
    let records = options.load();
    let value = match command.as_str() {
        "overview" => overview(&records),
        "compare" => comparisons(&records, options.task.as_deref(), options.arm.as_deref()),
        "attempt" => attempt_value(find_attempt(&records, &options.positional)?),
        "evidence" if options.missing => missing_evidence(&records),
        "evidence" => {
            let attempt = find_attempt(&records, &options.positional)?;
            json!({"job":attempt.job,"trial":attempt.trial,"health":attempt.evidence_health(),"files":attempt.evidence.iter().map(evidence_value).collect::<Vec<_>>()})
        }
        "history" => history(&records),
        "runbooks" => runbooks(),
        _ => unreachable!(),
    };
    if options.json {
        serde_json::to_writer_pretty(
            &mut *out,
            &json!({
                "schema": SCHEMA,
                "view": if options.missing { "evidence-missing" } else { command.as_str() },
                "data": value,
                "read_errors": records.errors,
            }),
        )
        .map_err(|error| error.to_string())?;
        writeln!(out).map_err(|error| error.to_string())?;
    } else {
        let view = if options.missing {
            "missing"
        } else {
            command.as_str()
        };
        render_text(view, &value, &records, out).map_err(|error| error.to_string())?;
    }
    Ok(0)
}

fn find_attempt<'a>(records: &'a Records, identity: &[String]) -> Result<&'a Attempt, String> {
    records
        .attempts
        .iter()
        .find(|attempt| attempt.job == identity[0] && attempt.trial == identity[1])
        .ok_or_else(|| {
            format!(
                "no attempt for job {} and trial {}",
                identity[0], identity[1]
            )
        })
}

fn missing_evidence(records: &Records) -> Value {
    let attempts: Vec<_> = records
        .attempts
        .iter()
        .filter_map(|attempt| {
            let missing = attempt.missing_evidence();
            (!missing.is_empty()).then(|| {
                json!({
                    "source": attempt.source,
                    "job": attempt.job,
                    "trial": attempt.trial,
                    "task": attempt.task,
                    "arm": attempt.arm,
                    "missing": missing.into_iter().map(evidence_value).collect::<Vec<_>>(),
                })
            })
        })
        .collect();
    json!({
        "attempts_total": records.attempts.len(),
        "attempts_with_missing": attempts.len(),
        "files_missing": attempts.iter().map(|a| a["missing"].as_array().map_or(0, Vec::len)).sum::<usize>(),
        "attempts": attempts,
    })
}

fn evidence_value(evidence: &Evidence) -> Value {
    json!({
        "kind": evidence.kind,
        "path": evidence.path,
        "state": evidence.state.label(),
        "note": evidence.note,
    })
}

fn attempt_value(attempt: &Attempt) -> Value {
    json!({
        "source": attempt.source,
        "job": attempt.job,
        "trial": attempt.trial,
        "task": attempt.task,
        "arm": attempt.arm,
        "profile": attempt.profile,
        "kind": attempt.kind,
        "commit": attempt.commit,
        "checksum": attempt.checksum,
        "architecture": attempt.architecture,
        "host": attempt.host,
        "image_state": attempt.image_state,
        "agent": attempt.agent,
        "model": attempt.model,
        "artifact": attempt.artifact,
        "reward": attempt.reward,
        "terminal_status": attempt.status,
        "display_status": attempt.display_status(),
        "started_at": attempt.started_at,
        "phases_ms": {
            "environment_setup": attempt.phases_ms[0],
            "agent_setup": attempt.phases_ms[1],
            "agent_execution": attempt.phases_ms[2],
            "verifier": attempt.phases_ms[3],
            "total": attempt.phases_ms[4],
        },
        "usage": {
            "input_tokens": attempt.input_tokens,
            "cache_tokens": attempt.cache_tokens,
            "output_tokens": attempt.output_tokens,
            "coverage": attempt.usage_coverage,
        },
        "cost": {
            "amount_usd": attempt.cost_usd,
            "provenance": attempt.cost_provenance,
            "components": attempt.costs.iter().map(|(name, amount, provenance)| json!({"name":name,"amount_usd":amount,"provenance":provenance})).collect::<Vec<_>>(),
        },
        "counts": attempt.counts.iter().map(|(name, count)| json!({"name":name,"value":count})).collect::<Vec<_>>(),
        "evidence_health": attempt.evidence_health(),
        "evidence": attempt.evidence.iter().map(evidence_value).collect::<Vec<_>>(),
        "notes": attempt.notes,
    })
}

fn overview(records: &Records) -> Value {
    let mut coverage = std::collections::BTreeMap::<&str, usize>::new();
    for attempt in &records.attempts {
        *coverage.entry(&attempt.usage_coverage).or_default() += 1;
    }
    let groups = ComparisonGroup::from_records(records)
        .iter()
        .map(|group| json!({
            "task": group.task,
            "arm": group.arm,
            "pin": group.pin,
            "attempts": group.attempts.iter().map(|&index| {
                let attempt = &records.attempts[index];
                json!({"job":attempt.job,"trial":attempt.trial,"reward":attempt.reward,"status":attempt.display_status(),"evidence_health":attempt.evidence_health()})
            }).collect::<Vec<_>>(),
        }))
        .collect::<Vec<_>>();
    json!({
        "sources": records.sources,
        "attempts_total": records.attempts.len(),
        "groups_total": groups.len(),
        "status_counts": records.status_counts(),
        "usage_coverage": coverage,
        "controls": records.attempts.iter().filter(|attempt| attempt.is_control()).count(),
        "latest_started_at": records.attempts.iter().filter_map(|attempt| attempt.started_at.as_ref()).max(),
        "report_label": records.report_label,
        "report_warnings": records.report_warnings,
        "groups": groups,
    })
}

fn comparisons(records: &Records, task: Option<&str>, arm: Option<&str>) -> Value {
    let groups = ComparisonGroup::from_records(records)
        .into_iter()
        .filter(|group| task.is_none_or(|task| group.task == task))
        .filter(|group| arm.is_none_or(|arm| group.arm == arm))
        .map(|group| {
            let members: Vec<_> = group.attempts.iter().map(|&index| &records.attempts[index]).collect();
            let graded: Vec<_> = members.iter().filter_map(|attempt| attempt.reward).collect();
            let fresh: Vec<_> = members.iter().filter(|attempt| attempt.kind == "fresh" && attempt.reward.is_some()).collect();
            let binary = fresh.iter().all(|attempt| matches!(attempt.reward, Some(0.0 | 1.0)));
            let complete = members.iter().all(|attempt| attempt.has_complete_comparison_identity());
            let interval = if complete && binary && fresh.len() >= 3 {
                let successes = fresh.iter().filter(|attempt| attempt.reward == Some(1.0)).count();
                let (low, high) = wilson_95(successes, fresh.len());
                Some(json!({"method":"wilson_95","low":low,"high":high,"successes":successes,"denominator":fresh.len()}))
            } else { None };
            json!({
                "task": group.task,
                "arm": group.arm,
                "pin": group.pin,
                "attempts_total": members.len(),
                "graded_denominator": graded.len(),
                "reward_mean": if graded.is_empty() { None } else { Some(graded.iter().sum::<f64>() / graded.len() as f64) },
                "fresh_graded_denominator": fresh.len(),
                "fresh_grades_are_binary": binary,
                "controlled_identity": complete,
                "interval": interval,
                "development_observation": true,
                "agent_time_ms_range": range_u64(&fresh.iter().filter_map(|attempt| attempt.phases_ms[2]).collect::<Vec<_>>()),
                "cost_usd_range": range_f64(&fresh.iter().filter_map(|attempt| attempt.cost_usd).collect::<Vec<_>>()),
                "attempts": members.iter().map(|attempt| attempt_value(attempt)).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    json!({"groups":groups})
}

fn range_u64(values: &[u64]) -> Option<[u64; 2]> {
    Some([*values.iter().min()?, *values.iter().max()?])
}

fn range_f64(values: &[f64]) -> Option<[f64; 2]> {
    Some([
        values.iter().copied().reduce(f64::min)?,
        values.iter().copied().reduce(f64::max)?,
    ])
}

fn wilson_95(successes: usize, trials: usize) -> (f64, f64) {
    let p = successes as f64 / trials as f64;
    let z = 1.96_f64;
    let denominator = 1.0 + z * z / trials as f64;
    let center = (p + z * z / (2.0 * trials as f64)) / denominator;
    let radius = z
        * ((p * (1.0 - p) / trials as f64) + z * z / (4.0 * (trials * trials) as f64)).sqrt()
        / denominator;
    ((center - radius).max(0.0), (center + radius).min(1.0))
}

fn history(records: &Records) -> Value {
    let mut attempts: Vec<_> = records.attempts.iter().collect();
    attempts.sort_by(|left, right| right.started_at.cmp(&left.started_at));
    json!({"attempts": attempts.iter().map(|attempt| attempt_value(attempt)).collect::<Vec<_>>()})
}

fn runbooks() -> Value {
    json!({"paths": [
        "docs/coder/terminal-bench.md",
        "docs/terminal-bench/runbook.md",
        "docs/terminal-bench/resilience.md",
        "docs/terminal-bench/coder-one-delegate-runbook.md",
        "docs/terminal-bench/README.md",
        "docs/coder/terminal-bench-contract.md",
        "docs/gym/terminal-bench-tui.md",
        "docs/gym/terminal-bench-cli.md",
    ]})
}

fn render_text(
    command: &str,
    value: &Value,
    records: &Records,
    out: &mut impl Write,
) -> io::Result<()> {
    match command {
        "overview" => {
            writeln!(
                out,
                "Terminal-Bench: {} attempts, {} groups",
                value["attempts_total"], value["groups_total"]
            )?;
            writeln!(out, "Sources: {}", records.sources.join(" · "))?;
            writeln!(
                out,
                "Status: {}",
                records
                    .status_counts()
                    .iter()
                    .map(|(name, count)| format!("{name} {count}"))
                    .collect::<Vec<_>>()
                    .join(" · ")
            )?;
            writeln!(
                out,
                "Controls: {} · latest: {}",
                value["controls"],
                value["latest_started_at"].as_str().unwrap_or("unknown")
            )?;
            if let Some(coverage) = value["usage_coverage"].as_object() {
                writeln!(
                    out,
                    "Usage coverage: {}",
                    coverage
                        .iter()
                        .map(|(name, count)| format!("{name} {count}"))
                        .collect::<Vec<_>>()
                        .join(" · ")
                )?;
            }
            writeln!(
                out,
                "Report: {}",
                records.report_label.as_deref().unwrap_or("not loaded")
            )?;
            for group in value["groups"].as_array().into_iter().flatten() {
                writeln!(
                    out,
                    "{} / {} · {} attempts · {}",
                    group["task"].as_str().unwrap_or("?"),
                    group["arm"].as_str().unwrap_or("?"),
                    group["attempts"].as_array().map_or(0, Vec::len),
                    group["pin"].as_str().unwrap_or("?")
                )?;
            }
            for warning in &records.report_warnings {
                writeln!(out, "REPORT WARNING: {warning}")?;
            }
        }
        "compare" => {
            for group in value["groups"].as_array().into_iter().flatten() {
                writeln!(
                    out,
                    "{} / {} · {}",
                    group["task"].as_str().unwrap_or("?"),
                    group["arm"].as_str().unwrap_or("?"),
                    group["pin"].as_str().unwrap_or("?")
                )?;
                writeln!(
                    out,
                    "  {} attempt{}; {} graded; reward mean {}; development observation",
                    group["attempts_total"],
                    if group["attempts_total"] == 1 {
                        ""
                    } else {
                        "s"
                    },
                    group["graded_denominator"],
                    show(&group["reward_mean"])
                )?;
                if group["interval"].is_null() {
                    writeln!(
                        out,
                        "  No controlled pass-rate interval (requires at least three fresh binary grades and complete identities)."
                    )?;
                } else {
                    writeln!(
                        out,
                        "  Wilson 95% [{:.2}, {:.2}] over {} fresh graded attempts",
                        group["interval"]["low"].as_f64().unwrap_or(0.0),
                        group["interval"]["high"].as_f64().unwrap_or(0.0),
                        group["interval"]["denominator"]
                    )?;
                }
                writeln!(
                    out,
                    "  Observed fresh spread: agent time {} ms · cost {} USD",
                    range_text(&group["agent_time_ms_range"]),
                    range_text(&group["cost_usd_range"])
                )?;
                for attempt in group["attempts"].as_array().into_iter().flatten() {
                    writeln!(
                        out,
                        "  {} / {} · reward {} · {} · agent/total {} / {} ms · cost {} ({}) · tokens {}/{} ({}) · {}",
                        attempt["job"].as_str().unwrap_or("?"),
                        attempt["trial"].as_str().unwrap_or("?"),
                        show(&attempt["reward"]),
                        attempt["display_status"].as_str().unwrap_or("?"),
                        show(&attempt["phases_ms"]["agent_execution"]),
                        show(&attempt["phases_ms"]["total"]),
                        show(&attempt["cost"]["amount_usd"]),
                        attempt["cost"]["provenance"].as_str().unwrap_or("?"),
                        show(&attempt["usage"]["input_tokens"]),
                        show(&attempt["usage"]["output_tokens"]),
                        attempt["usage"]["coverage"].as_str().unwrap_or("?"),
                        attempt["evidence_health"].as_str().unwrap_or("?")
                    )?;
                }
            }
        }
        "attempt" => {
            writeln!(
                out,
                "{} / {} · {} / {}",
                value["job"].as_str().unwrap_or("?"),
                value["trial"].as_str().unwrap_or("?"),
                value["task"].as_str().unwrap_or("?"),
                value["arm"].as_str().unwrap_or("?")
            )?;
            writeln!(
                out,
                "Reward {} · status {} · model {} · artifact {}",
                show(&value["reward"]),
                value["display_status"].as_str().unwrap_or("?"),
                show(&value["model"]),
                show(&value["artifact"])
            )?;
            writeln!(
                out,
                "Commit {} · checksum {} · architecture {} · host {} · image {}",
                show(&value["commit"]),
                show(&value["checksum"]),
                show(&value["architecture"]),
                show(&value["host"]),
                show(&value["image_state"])
            )?;
            writeln!(
                out,
                "Timing (ms): environment {} · install {} · agent {} · verifier {} · total {}",
                show(&value["phases_ms"]["environment_setup"]),
                show(&value["phases_ms"]["agent_setup"]),
                show(&value["phases_ms"]["agent_execution"]),
                show(&value["phases_ms"]["verifier"]),
                show(&value["phases_ms"]["total"])
            )?;
            writeln!(
                out,
                "Usage: in {} · cache {} · out {} · coverage {}",
                show(&value["usage"]["input_tokens"]),
                show(&value["usage"]["cache_tokens"]),
                show(&value["usage"]["output_tokens"]),
                show(&value["usage"]["coverage"])
            )?;
            writeln!(
                out,
                "Cost: {} · {}",
                show(&value["cost"]["amount_usd"]),
                show(&value["cost"]["provenance"])
            )?;
            for component in value["cost"]["components"].as_array().into_iter().flatten() {
                writeln!(
                    out,
                    "  {}: {} ({})",
                    show(&component["name"]),
                    show(&component["amount_usd"]),
                    show(&component["provenance"])
                )?;
            }
            for count in value["counts"].as_array().into_iter().flatten() {
                writeln!(out, "  {}: {}", show(&count["name"]), show(&count["value"]))?;
            }
            writeln!(
                out,
                "Evidence: {} files · {}",
                value["evidence"].as_array().map_or(0, Vec::len),
                show(&value["evidence_health"])
            )?;
            for note in value["notes"].as_array().into_iter().flatten() {
                writeln!(out, "Note: {}", show(note))?;
            }
        }
        "evidence" => {
            writeln!(
                out,
                "{} / {} · {}",
                show(&value["job"]),
                show(&value["trial"]),
                show(&value["health"])
            )?;
            for file in value["files"].as_array().into_iter().flatten() {
                writeln!(
                    out,
                    "{} · {} · {}",
                    show(&file["kind"]),
                    show(&file["state"]),
                    show(&file["path"])
                )?;
            }
        }
        "missing" => {
            writeln!(
                out,
                "Missing evidence: {} files across {} of {} attempts",
                value["files_missing"], value["attempts_with_missing"], value["attempts_total"]
            )?;
            for attempt in value["attempts"].as_array().into_iter().flatten() {
                writeln!(
                    out,
                    "{} / {} · {} / {}",
                    show(&attempt["job"]),
                    show(&attempt["trial"]),
                    show(&attempt["task"]),
                    show(&attempt["arm"])
                )?;
                for file in attempt["missing"].as_array().into_iter().flatten() {
                    let note = file["note"]
                        .as_str()
                        .map_or(String::new(), |note| format!(" ({note})"));
                    writeln!(
                        out,
                        "  {} · {}{}",
                        show(&file["kind"]),
                        show(&file["path"]),
                        note
                    )?;
                }
            }
        }
        "history" => {
            for attempt in value["attempts"].as_array().into_iter().flatten() {
                writeln!(
                    out,
                    "{} · {} / {} · {} / {} · reward {} · {}",
                    show(&attempt["started_at"]),
                    show(&attempt["task"]),
                    show(&attempt["arm"]),
                    show(&attempt["job"]),
                    show(&attempt["trial"]),
                    show(&attempt["reward"]),
                    show(&attempt["display_status"])
                )?;
            }
        }
        "runbooks" => {
            for path in value["paths"].as_array().into_iter().flatten() {
                writeln!(out, "{}", show(path))?;
            }
        }
        _ => {}
    }
    for error in &records.errors {
        writeln!(out, "READ ERROR: {error}")?;
    }
    Ok(())
}

fn show(value: &Value) -> String {
    match value {
        Value::Null => "—".to_owned(),
        Value::String(text) => text.clone(),
        _ => value.to_string(),
    }
}

fn range_text(value: &Value) -> String {
    value.as_array().map_or("—".to_owned(), |pair| {
        if pair.len() == 2 {
            format!("{} to {}", show(&pair[0]), show(&pair[1]))
        } else {
            "—".to_owned()
        }
    })
}

fn harness(command: &str, args: &[String], err: &mut impl Write) -> Result<i32, String> {
    let mut uv = PathBuf::from("uv");
    let mut directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench");
    let mut forwarded = Vec::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--uv" | "--harness-dir" => {
                let value = args
                    .get(index + 1)
                    .filter(|value| !value.starts_with("--"))
                    .ok_or_else(|| format!("{} needs a path", args[index]))?;
                if args[index] == "--uv" {
                    uv = value.into();
                } else {
                    directory = value.into();
                }
                index += 1;
            }
            "--json" => return Err("--json applies only to evidence commands".to_owned()),
            _ => forwarded.push(args[index].clone()),
        }
        index += 1;
    }
    if !directory.join("pyproject.toml").is_file() {
        return Err(format!("no tbench package at {}", directory.display()));
    }
    let subcommand = match command {
        "inspect-job" => "inspect",
        "report" => "compare",
        _ => command,
    };
    let status = Command::new(uv)
        .current_dir(directory)
        .arg("run")
        .arg("tbench")
        .arg(subcommand)
        .args(forwarded)
        .status()
        .map_err(|error| format!("could not start the tbench harness: {error}"))?;
    if let Some(code) = status.code() {
        Ok(code)
    } else {
        writeln!(err, "terminal-bench: harness ended from a signal")
            .map_err(|error| error.to_string())?;
        Ok(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn json_keeps_missing_reward_separate_from_zero_cost_and_refusal() {
        let args = strings(&["history", "--no-jobs", "--no-traces", "--json"]);
        let mut output = Vec::new();
        let code = execute(&args, &mut output, &mut Vec::new()).unwrap();
        assert_eq!(code, 0);
        let value: Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(value["schema"], SCHEMA);
        let attempts = value["data"]["attempts"].as_array().unwrap();
        assert!(
            attempts
                .iter()
                .any(|attempt| attempt["terminal_status"] == "timeout"
                    && attempt["reward"].is_null())
        );
        assert!(attempts.iter().any(|attempt| attempt["reward"] == 0.0));
    }

    #[test]
    fn attempt_and_evidence_select_the_same_job_and_trial() {
        let identity = ["smoke--coder-v05--fix-git", "fix-git__aQ6BJAN"];
        for command in ["attempt", "evidence"] {
            let mut args = strings(&[
                command,
                identity[0],
                identity[1],
                "--no-jobs",
                "--no-traces",
                "--json",
            ]);
            let mut output = Vec::new();
            assert_eq!(execute(&args, &mut output, &mut Vec::new()).unwrap(), 0);
            let value: Value = serde_json::from_slice(&output).unwrap();
            assert_eq!(value["data"]["job"], identity[0]);
            assert_eq!(value["data"]["trial"], identity[1]);
            args[1] = "missing".to_owned();
            assert!(execute(&args, &mut Vec::new(), &mut Vec::new()).is_err());
        }
    }

    #[test]
    fn evidence_missing_lists_unretained_files_per_attempt() {
        let temp = tempfile::tempdir().unwrap();
        let episode = temp
            .path()
            .join("smoke--coder-one-x--task/task__abc.episode");
        std::fs::create_dir_all(&episode).unwrap();
        std::fs::write(
            episode.join("harbor-result.json"),
            br#"{"task_name":"terminal-bench/task"}"#,
        )
        .unwrap();
        std::fs::write(
            temp.path().join("smoke--coder-one-x--task/task__abc.json"),
            b"{}",
        )
        .unwrap();
        std::fs::write(
            episode.join("manifest.json"),
            br#"{"contract":"openagents.coder.episode.v1","files":{"stream":{"path":"artifacts/delegate-1.stream.jsonl","sha256":"00"}}}"#,
        )
        .unwrap();
        let traces = temp.path().to_str().unwrap();
        let args = strings(&[
            "evidence",
            "--missing",
            "--no-jobs",
            "--no-samples",
            "--traces-dir",
            traces,
            "--json",
        ]);
        let mut output = Vec::new();
        assert_eq!(execute(&args, &mut output, &mut Vec::new()).unwrap(), 0);
        let value: Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(value["view"], "evidence-missing");
        assert_eq!(value["data"]["attempts_with_missing"], 1);
        let missing = &value["data"]["attempts"][0]["missing"][0];
        assert_eq!(missing["kind"], "stream");
        assert!(
            missing["path"]
                .as_str()
                .unwrap()
                .ends_with("artifacts/delegate-1.stream.jsonl")
        );
        let mut text = Vec::new();
        execute(&args[..args.len() - 1], &mut text, &mut Vec::new()).unwrap();
        let text = String::from_utf8(text).unwrap();
        assert!(
            text.contains("Missing evidence: 1 files across 1 of 1 attempts"),
            "{text}"
        );
        assert!(
            text.contains("smoke--coder-one-x--task / task__abc"),
            "{text}"
        );
    }

    #[test]
    fn text_comparison_keeps_trials_and_development_denominators() {
        let args = strings(&[
            "compare",
            "--task",
            "terminal-bench/fix-git",
            "--arm",
            "oracle",
            "--no-jobs",
            "--no-traces",
        ]);
        let mut output = Vec::new();
        assert_eq!(execute(&args, &mut output, &mut Vec::new()).unwrap(), 0);
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("1 attempt; 1 graded"), "{text}");
        assert!(text.contains("No controlled pass-rate interval"), "{text}");
        assert!(text.contains("smoke--oracle / fix-git__7TEC9XV"), "{text}");
    }

    #[cfg(unix)]
    #[test]
    fn run_forwards_flags_and_exit_status_to_an_executable_without_a_shell() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("pyproject.toml"),
            "[project]\nname='fake'\nversion='0'\n",
        )
        .unwrap();
        let script = temp.path().join("fake-uv");
        std::fs::write(
            &script,
            "#!/bin/sh\nprintf '%s\\n' \"$PWD\" \"$@\" > invocation.txt\nexit 7\n",
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&script, permissions).unwrap();
        let args = strings(&[
            "run",
            "--profile",
            "smoke",
            "--agent",
            "oracle",
            "--task",
            "fix-git",
            "--uv",
            script.to_str().unwrap(),
            "--harness-dir",
            temp.path().to_str().unwrap(),
        ]);
        assert_eq!(harness("run", &args[1..], &mut Vec::new()).unwrap(), 7);
        let observed = std::fs::read_to_string(temp.path().join("invocation.txt")).unwrap();
        let canonical = temp.path().canonicalize().unwrap();
        assert_eq!(
            observed.lines().collect::<Vec<_>>(),
            [
                canonical.to_str().unwrap(),
                "run",
                "tbench",
                "run",
                "--profile",
                "smoke",
                "--agent",
                "oracle",
                "--task",
                "fix-git"
            ]
        );
    }
}
