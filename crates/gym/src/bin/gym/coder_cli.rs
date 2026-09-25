//! `gym coder`: Coder One's policy manifests, as episodes recorded them
//! and as the checkout holds them.

use gym::coder_policy::{self, Entry};
use gym::terminal_bench::Records;
use serde_json::{Value, json};
use std::env;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const HELP: &str = "\
gym coder policy  list, show, and compare Coder One policy manifests

  policy list              every distinct manifest, by content digest, with the
                           experiment arms and attempts that ran it and the
                           files that hold it
  policy show QUERY        one manifest's fields
  policy diff QUERY QUERY  the fields two manifests set differently

A QUERY is a content digest or its first six or more hex digits, a manifest
name, an experiment arm (a named configuration), or a manifest file name or
path.

  --jobs-dir PATH          local Harbor jobs (default ~/.openagents/terminal-bench/jobs)
  --traces-dir PATH        retained checkout traces
  --policies-dir PATH      manifest files (default crates/coder-one/policies)
  --no-jobs, --no-traces   leave out that source
  --json                   print versioned JSON instead of text

gym coder components [--component ID] [--json]
                           each component's isolated fixture runs beside its
                           episode invocations; see gym coder components --help
gym coder requirements [QUERY] [--json]
                           requirement maps from task.requirements: spans,
                           kinds, bindings, coverage, and recall and precision
                           measured on labeled tasks
gym coder minitasks [--run ID|latest] [--json]
                           mini-task runs with their grades and timelines;
                           see gym coder minitasks --help
gym coder prompt [ATTEMPT] [--json]
                           executor system prompt variants, or one attempt's
                           sections; see gym coder prompt --help
gym coder live [--follow] [--json]
                           attempts in progress: current component, latest
                           executor events and judgments, spend, and staleness;
                           see gym coder live --help
gym coder capabilities [--json]
                           each executor adapter's capability matrix: start,
                           observe, stop, resume, and steer, with the tests and
                           the real CLI demonstration behind each cell
gym coder briefing [QUERY] [--json]
                           what each briefing delivered, trimmed, and left out,
                           and the packer replay's before and after
gym coder matrix [--json]  the outcome matrix: task by policy, frontiers, oracles,
                           and the mini-task handoff patterns as policies
gym coder composition [QUERY] [--json]
                           Terminal-Bench attempts that ran the tunable
                           composition: route, dispatches, handoffs, checks,
                           support, repair, and the second executor
gym coder families [--policy PATH] [--json]
                           a policy's route family table recomputed from the
                           Terminal-Bench tasks and the leaderboard rows
gym coder handoff [--json] control.handoff's patterns on the mini-tasks: pass, modeled
                           cost, episode time, and the objective per policy
gym coder router [--json]  task features, router picks, and leave-one-task-out
                           regret compared with fixed and hand-written baselines
gym coder monitor [--json] control.monitor's replay over retained streams: trigger
                           precision for the rules and Jev, stale answers, and cost
gym coder coverage [--attempt JOB/TRIAL | --run ID] [--json]
                           verify.checks requirement coverage per attempt and
                           mini-task run; see gym coder coverage --help
gym coder recall [--failures] [--json]
                           check recall on the labeled Terminal-Bench set:
                           verifier failures and passes the checks flagged;
                           see gym coder recall --help
gym coder truth [--set held-out|calibration|all] [--within] [--json]
                           truthful checks: each signal's fail precision,
                           failure recall, and pass rate with intervals, and
                           the calibrated verdict compared with today's checks
                           on held-out tasks; see gym coder truth --help
gym coder truth --confirmation PATH [--executor luna|astra] [--json]
                           retained archive confirmation: frozen signal rates,
                           missing grades, and whole-task bootstrap differences
gym coder asks [ID|latest] [--json]
                           the questions `coder-one ask` answered: citations
                           checked, claims verified, cost, and time
gym coder proposals [ID|latest] [--json]
                           the changes that asks proposed: finding, change,
                           decision, and result; `approve ID` or `reject ID`
                           [--note TEXT] records a decision
gym coder study [ID] [--all] [--json]
                           hill-climbing studies: candidates by tier, development
                           and held-out results, frontiers, promotions, and spend";

const SCHEMA: &str = "openagents.gym.coder-policy.v1";

struct Options {
    jobs: Option<PathBuf>,
    traces: Option<PathBuf>,
    policies: Vec<PathBuf>,
    json: bool,
    positional: Vec<String>,
}

impl Options {
    fn parse(args: &[String]) -> Result<Self, String> {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let root = manifest_dir
            .canonicalize()
            .unwrap_or_else(|_| manifest_dir.to_path_buf());
        let root = root
            .parent()
            .and_then(Path::parent)
            .unwrap_or(&root)
            .to_path_buf();
        let mut options = Self {
            jobs: env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join(".openagents/terminal-bench/jobs")),
            traces: Some(root.join("bench/terminal-bench/traces")),
            policies: vec![root.join("crates/coder-one/policies")],
            json: false,
            positional: Vec::new(),
        };
        let mut explicit_policies = Vec::new();
        let mut index = 0;
        while index < args.len() {
            let argument = args[index].as_str();
            match argument {
                "--json" => options.json = true,
                "--no-jobs" => options.jobs = None,
                "--no-traces" => options.traces = None,
                "--jobs-dir" | "--traces-dir" | "--policies-dir" => {
                    let value = args
                        .get(index + 1)
                        .filter(|value| !value.starts_with("--"))
                        .ok_or_else(|| format!("{argument} needs a value"))?;
                    match argument {
                        "--jobs-dir" => options.jobs = Some(value.into()),
                        "--traces-dir" => options.traces = Some(value.into()),
                        _ => explicit_policies.push(PathBuf::from(value)),
                    }
                    index += 1;
                }
                _ if argument.starts_with('-') => return Err(format!("unknown option {argument}")),
                _ => options.positional.push(args[index].clone()),
            }
            index += 1;
        }
        if !explicit_policies.is_empty() {
            options.policies = explicit_policies;
        }
        Ok(options)
    }
}

pub fn run(args: Vec<String>) -> i32 {
    let mut output = io::stdout().lock();
    match execute(&args, &mut output) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("coder: {message}");
            2
        }
    }
}

fn execute(args: &[String], out: &mut impl Write) -> Result<i32, String> {
    if args.first().map(String::as_str) == Some("asks") {
        return gym::coder_asks::command(&args[1..], out);
    }
    if args.first().map(String::as_str) == Some("proposals") {
        return gym::coder_proposals::command(&args[1..], out);
    }
    if args.first().map(String::as_str) == Some("components") {
        return gym::coder_components::command(&args[1..], out);
    }
    if args.first().map(String::as_str) == Some("briefing") {
        return gym::coder_briefing::command(&args[1..], out);
    }
    if args.first().map(String::as_str) == Some("requirements") {
        return gym::coder_requirements::command(&args[1..], out);
    }
    if args.first().map(String::as_str) == Some("coverage") {
        return gym::coder_coverage::command(&args[1..], out);
    }
    if args.first().map(String::as_str) == Some("recall") {
        return gym::coder_recall::command(&args[1..], out);
    }
    if args.first().map(String::as_str) == Some("truth") {
        return gym::coder_truth::command(&args[1..], out);
    }
    if args.first().map(String::as_str) == Some("minitasks") {
        return gym::coder_minitasks::command(&args[1..], out);
    }
    if args.first().map(String::as_str) == Some("live") {
        return gym::coder_live::command(&args[1..], out);
    }
    if args.first().map(String::as_str) == Some("capabilities") {
        return gym::coder_capabilities::command(&args[1..], out);
    }
    if args.first().map(String::as_str) == Some("prompt") {
        return gym::coder_prompt::command(&args[1..], out);
    }
    if args.first().map(String::as_str) == Some("matrix") {
        return gym::coder_matrix::command(&args[1..], out);
    }
    if args.first().map(String::as_str) == Some("router") {
        return gym::coder_router::command(&args[1..], out);
    }
    if args.first().map(String::as_str) == Some("study") {
        return gym::coder_study::command(&args[1..], out);
    }
    if args.first().map(String::as_str) == Some("monitor") {
        return gym::coder_monitor::command(&args[1..], out);
    }
    if args.first().map(String::as_str) == Some("handoff") {
        return gym::coder_handoff::command(&args[1..], out);
    }
    if args.first().map(String::as_str) == Some("families") {
        return gym::coder_families::command(&args[1..], out);
    }
    if args.first().map(String::as_str) == Some("composition") {
        return gym::coder_composition::command(&args[1..], out);
    }
    let (Some("policy"), Some(command)) = (
        args.first().map(String::as_str),
        args.get(1).map(String::as_str),
    ) else {
        writeln!(out, "{HELP}").map_err(|error| error.to_string())?;
        return Ok(if args.is_empty() { 0 } else { 2 });
    };
    if matches!(command, "help" | "--help" | "-h") {
        writeln!(out, "{HELP}").map_err(|error| error.to_string())?;
        return Ok(0);
    }
    let options = Options::parse(&args[2..])?;
    let expected = match command {
        "list" => 0,
        "show" => 1,
        "diff" => 2,
        other => return Err(format!("unknown policy command {other}\n\n{HELP}")),
    };
    if options.positional.len() != expected {
        return Err(format!(
            "policy {command} takes {expected} arguments; got {}",
            options.positional.len()
        ));
    }
    let records = Records::load(options.jobs.as_deref(), options.traces.as_deref(), None);
    let dirs: Vec<&Path> = options.policies.iter().map(PathBuf::as_path).collect();
    let (entries, errors) = coder_policy::catalog(&records, &dirs);
    let value = match command {
        "list" => json!({
            "schema": SCHEMA,
            "command": "list",
            "policies": entries.values().map(entry_value).collect::<Vec<_>>(),
            "errors": errors,
        }),
        "show" => {
            let entry = coder_policy::find(&entries, &options.positional[0])?;
            let mut value = entry_value(entry);
            value["fields"] = fields_value(&entry.record.manifest);
            value["manifest"] = entry.record.manifest.clone();
            json!({ "schema": SCHEMA, "command": "show", "policy": value })
        }
        _ => {
            let a = coder_policy::find(&entries, &options.positional[0])?;
            let b = coder_policy::find(&entries, &options.positional[1])?;
            let changed = coder_policy::diff(&a.record.manifest, &b.record.manifest);
            let total = coder_policy::fields(&a.record.manifest)
                .keys()
                .chain(coder_policy::fields(&b.record.manifest).keys())
                .collect::<std::collections::BTreeSet<_>>()
                .len();
            json!({
                "schema": SCHEMA,
                "command": "diff",
                "a": entry_value(a),
                "b": entry_value(b),
                "same_digest": a.record.digest == b.record.digest,
                "fields_compared": total,
                "differences": changed.iter().map(|(field, left, right)| json!({
                    "field": field,
                    "a": left,
                    "b": right,
                })).collect::<Vec<_>>(),
            })
        }
    };
    if options.json {
        writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?
        )
        .map_err(|error| error.to_string())?;
    } else {
        render(&value, out).map_err(|error| error.to_string())?;
    }
    Ok(0)
}

fn entry_value(entry: &Entry) -> Value {
    json!({
        "digest": entry.record.digest,
        "name": entry.record.name,
        "source": entry.record.source,
        "overrides": entry.record.overrides,
        "files": entry.files,
        "arms": entry.arms,
        "attempts": entry.attempts,
    })
}

fn fields_value(manifest: &Value) -> Value {
    Value::Object(coder_policy::fields(manifest).into_iter().collect())
}

fn label(entry: &Value) -> String {
    format!(
        "{} {}",
        entry["digest"].as_str().map_or("?", coder_policy::short),
        entry["name"].as_str().unwrap_or("unnamed")
    )
}

fn seen(entry: &Value) -> String {
    let files: Vec<&str> = entry["files"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let arms: Vec<&str> = entry["arms"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let mut parts = vec![format!("{} attempts", entry["attempts"])];
    if !arms.is_empty() {
        parts.push(format!("experiment arms {}", arms.join(", ")));
    }
    if !files.is_empty() {
        parts.push(format!("files {}", files.join(", ")));
    }
    parts.join(" · ")
}

fn render(value: &Value, out: &mut impl Write) -> io::Result<()> {
    match value["command"].as_str() {
        Some("list") => {
            let policies = value["policies"].as_array().cloned().unwrap_or_default();
            writeln!(out, "{} policy manifests", policies.len())?;
            for entry in &policies {
                writeln!(out, "{}", label(entry))?;
                writeln!(out, "  {}", seen(entry))?;
            }
            for error in value["errors"].as_array().into_iter().flatten() {
                writeln!(out, "error: {}", error.as_str().unwrap_or("?"))?;
            }
        }
        Some("show") => {
            let entry = &value["policy"];
            writeln!(out, "policy {}", label(entry))?;
            writeln!(
                out,
                "content digest {}",
                entry["digest"].as_str().unwrap_or("?")
            )?;
            writeln!(out, "{}", seen(entry))?;
            for line in entry["overrides"].as_array().into_iter().flatten() {
                writeln!(out, "override: {}", line.as_str().unwrap_or("?"))?;
            }
            for (field, value) in entry["fields"].as_object().into_iter().flatten() {
                writeln!(out, "  {field} = {value}")?;
            }
        }
        Some("diff") => {
            writeln!(out, "a {}", label(&value["a"]))?;
            writeln!(out, "b {}", label(&value["b"]))?;
            let differences = value["differences"].as_array().cloned().unwrap_or_default();
            if differences.is_empty() {
                writeln!(out, "All {} fields match.", value["fields_compared"])?;
            } else {
                for difference in &differences {
                    writeln!(
                        out,
                        "  {}: a {}, b {}",
                        difference["field"].as_str().unwrap_or("?"),
                        difference["a"],
                        difference["b"]
                    )?;
                }
                writeln!(
                    out,
                    "{} of {} fields differ.",
                    differences.len(),
                    value["fields_compared"]
                )?;
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str]) -> (i32, String) {
        let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
        let mut out = Vec::new();
        let code = execute(&args, &mut out).unwrap();
        (code, String::from_utf8(out).unwrap())
    }

    #[test]
    fn policy_diff_names_exactly_the_executor_profile_and_directions() {
        let (code, text) = run(&[
            "policy",
            "diff",
            "coder-one-jevprobe3-luna",
            "jevprobe2-opus-lean-low-5m.json",
            "--no-jobs",
            "--no-traces",
        ]);
        assert_eq!(code, 0);
        assert!(
            text.contains("policy.executor.agent: a \"codex\", b \"claude-code\""),
            "{text}"
        );
        assert!(
            text.contains("policy.brief.directions: a \"batch-checked\", b \"batch\""),
            "{text}"
        );
        assert!(text.contains("7 of "), "{text}");

        let (_, json) = run(&[
            "policy",
            "diff",
            "coder-one-jevprobe3-luna",
            "coder-one-jevprobe2-opus-lean-low-5m",
            "--no-jobs",
            "--no-traces",
            "--json",
        ]);
        let value: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["schema"], SCHEMA);
        assert_eq!(value["differences"].as_array().unwrap().len(), 7);
        assert_eq!(value["same_digest"], false);
    }

    #[test]
    fn policy_show_and_list_read_the_checked_in_manifests() {
        let (_, text) = run(&["policy", "list", "--no-jobs", "--no-traces"]);
        assert!(text.contains("coder-one-jevprobe3-luna"), "{text}");
        let (_, json) = run(&[
            "policy",
            "show",
            "coder-one-jevprobe3-luna",
            "--no-jobs",
            "--no-traces",
            "--json",
        ]);
        let value: Value = serde_json::from_str(&json).unwrap();
        let digest = value["policy"]["digest"].as_str().unwrap().to_owned();
        assert_eq!(
            value["policy"]["fields"]["policy.executor.model"],
            "gpt-6-luna"
        );
        let (_, text) = run(&["policy", "show", &digest[..8], "--no-jobs", "--no-traces"]);
        assert!(text.contains("policy.evidence.probes = \"v2\""), "{text}");
    }
}
