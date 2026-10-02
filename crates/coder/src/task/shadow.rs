//! The shadow baseline (#10209): what a real Coder task would have cost
//! through the raw engine.
//!
//! When the person turns it on (`coder.shadow`, a percent of runs; off by
//! default), a sample of this computer's finished Coder runs also runs once
//! through the raw engine on its own defaults: Claude Code (`claude -p`),
//! or Codex (`codex exec`) when Codex ran the task. The baseline gets the
//! person's request as they wrote it, in a scratch clone of the same commit
//! with no remote, so it can't push, and its changes are never applied: the
//! clone is deleted once its record is written. Both sides land in one
//! record (`shadow/records.jsonl` beside the task store): cost (the routed
//! run's engine plus Jev, the baseline's own figure), wall time, turns, and,
//! when the recipe kept checks for the task, whether each side passes them.
//!
//! - **Sampling** is the task ID's own bits ([`sampled`]): the same task
//!   always gets the same answer, and nothing random is kept.
//! - **Budget** is only the person's (`coder.shadow_budget_usd`): once the
//!   recorded baselines reach it no new one starts; unset, there is no cap.
//!   One baseline runs at a time.
//! - **Eligible** runs are a first turn that ended (not a continuation, an
//!   issue flow, or a run that asked a question), whose engine was Codex or
//!   Claude Code, in a checkout with a base commit.
//! - **Detached.** The baseline is a shell script started in its own process
//!   group, so it outlives the terminal; [`finalize`] (run by `openagents
//!   shadow`) reads what it left, runs the checks, writes the record, and
//!   removes the clone.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::Digest as _;

/// A record's schema.
pub const SCHEMA: &str = "openagents.coder.shadow-baseline.v1";
/// A pending baseline's spec.
pub const SPEC_SCHEMA: &str = "openagents.coder.shadow-spec.v1";
/// How long one kept check may run on each side.
const CHECK_SECONDS: u64 = 300;
/// At most this many of the recipe's kept checks are run.
const MAX_CHECKS: usize = 2;

/// The shadow folder beside the task store `store`.
#[must_use]
pub fn dir(store: &Path) -> PathBuf {
    store.with_file_name("shadow")
}

/// Whether `task` falls in a `percent` sample: the first two bytes of the
/// task ID's SHA-256, modulo 100, below `percent`.
#[must_use]
pub fn sampled(task: &str, percent: u8) -> bool {
    let digest = sha2::Sha256::digest(task.as_bytes());
    u16::from_be_bytes([digest[0], digest[1]]) % 100 < u16::from(percent)
}

/// What a pending baseline runs, written before it starts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub schema: String,
    pub task: String,
    #[serde(default)]
    pub thread: Option<String>,
    pub checkout: String,
    pub base: String,
    /// The routed run's worktree, where its kept checks run.
    pub routed_worktree: String,
    /// The routed run's engine (`claude`, `codex`).
    pub routed_engine: String,
    /// The routed run's trajectory, where the recipe's kept checks are.
    #[serde(default)]
    pub trajectory: Option<String>,
    /// The routed run's cost in micro-dollars, when known.
    #[serde(default)]
    pub routed_cost_microusd: Option<u64>,
    /// The baseline's engine: `claude` or `codex`.
    pub engine: String,
    /// The model the baseline's engine names in its own configuration,
    /// when it names one (Codex), for pricing.
    #[serde(default)]
    pub model: Option<String>,
    /// The request as the person wrote it.
    pub request: String,
    /// Unix milliseconds the baseline started.
    pub started_ms: u64,
}

/// Why a finished run did or did not get a baseline.
#[derive(Clone, Debug, PartialEq)]
pub enum Decision {
    Off,
    NotSampled,
    Ineligible(&'static str),
    /// Another baseline is still running.
    Busy,
    OverBudget {
        spent_usd: f64,
        budget_usd: f64,
    },
    Started(PathBuf),
}

/// One finished run, as [`offer`] needs it.
#[derive(Clone, Debug)]
pub struct Finished<'a> {
    pub task: &'a str,
    pub thread: Option<&'a str>,
    pub turn: usize,
    /// The task's prompt (the handoff the chat wrote).
    pub prompt: &'a str,
    pub checkout: &'a str,
    pub worktree: &'a str,
    pub base: &'a str,
    pub provider: &'a str,
    /// The run's ending (`completed`, `checks_passed`, ...).
    pub ending: &'a str,
    pub cost_microusd: Option<u64>,
    pub trajectory: Option<&'a Path>,
    /// The run is an issue flow (it lands, so it is never shadowed).
    pub issue_flow: bool,
}

/// The decision for `run` under `coder`'s shadow settings, without
/// starting anything.
#[must_use]
pub fn decide(store: &Path, coder: &super::settings::Coder, run: &Finished<'_>) -> Decision {
    let Some(percent) = coder.shadow_percent else {
        return Decision::Off;
    };
    if run.turn != 1 {
        return Decision::Ineligible("a continuation");
    }
    if run.issue_flow {
        return Decision::Ineligible("an issue flow");
    }
    if !matches!(
        run.ending,
        "model_finished" | "checks_passed" | "completed" | "finished"
    ) {
        return Decision::Ineligible("a run that did not finish");
    }
    if !matches!(run.provider, "claude" | "codex") {
        return Decision::Ineligible("an engine without a raw baseline");
    }
    if run.base.is_empty() || run.checkout.is_empty() {
        return Decision::Ineligible("no base commit");
    }
    if !sampled(run.task, percent) {
        return Decision::NotSampled;
    }
    let shadow = dir(store);
    if pending(&shadow).next().is_some() {
        return Decision::Busy;
    }
    if let Some(cents) = coder.shadow_budget_cents {
        let spent_usd: f64 = records(&shadow)
            .iter()
            .filter_map(|r| r["baseline"]["cost_usd"].as_f64())
            .sum();
        let budget_usd = cents as f64 / 100.0;
        if spent_usd >= budget_usd {
            return Decision::OverBudget {
                spent_usd,
                budget_usd,
            };
        }
    }
    Decision::Started(shadow.join("pending").join(run.task))
}

/// Decide for `run` and, when it is sampled, start its baseline.
#[must_use]
pub fn offer(store: &Path, coder: &super::settings::Coder, run: &Finished<'_>) -> Decision {
    let decision = decide(store, coder, run);
    let Decision::Started(folder) = &decision else {
        return decision;
    };
    match start(folder, run) {
        Ok(()) => decision,
        Err(_) => {
            let _ = std::fs::remove_dir_all(folder);
            Decision::Ineligible("the scratch copy could not be made")
        }
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

fn git(dir: &Path, args: &[&str]) -> std::io::Result<()> {
    let status = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!("git {args:?} failed")))
    }
}

/// The raw engine's binary on this computer.
fn binary(engine: &str) -> Option<PathBuf> {
    if engine == "claude" {
        return super::autostart::claude_binary();
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(engine))
        .find(|candidate| candidate.is_file())
}

/// The model Codex's own configuration names (`model = "..."` in
/// `$CODEX_HOME/config.toml`, else `~/.codex/config.toml`).
fn codex_model() -> Option<String> {
    let home = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".codex")))?;
    let text = std::fs::read_to_string(home.join("config.toml")).ok()?;
    text.lines()
        .take_while(|line| !line.trim_start().starts_with('['))
        .find_map(|line| {
            let (key, value) = line.split_once('=')?;
            (key.trim() == "model").then(|| value.trim().trim_matches('"').to_owned())
        })
}

/// Shell-quote `text` for a POSIX shell.
fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

fn start(folder: &Path, run: &Finished<'_>) -> std::io::Result<()> {
    let engine = if run.provider == "codex" {
        "codex"
    } else {
        "claude"
    };
    let bin = binary(engine).ok_or_else(|| std::io::Error::other("no engine binary"))?;
    std::fs::create_dir_all(folder)?;
    let work = folder.join("work");
    let work_text = work.to_string_lossy().into_owned();
    git(
        folder,
        &[
            "clone",
            "--quiet",
            "--shared",
            "--no-checkout",
            run.checkout,
            &work_text,
        ],
    )?;
    git(&work, &["checkout", "--quiet", "--detach", run.base])?;
    // No remote: the baseline can't push what it changes.
    let _ = git(&work, &["remote", "remove", "origin"]);
    let request = openagents_chat::basic_chats::handoff_request(run.prompt)
        .unwrap_or_else(|| run.prompt.to_owned());
    let spec = Spec {
        schema: SPEC_SCHEMA.into(),
        task: run.task.into(),
        thread: run.thread.map(str::to_owned),
        checkout: run.checkout.into(),
        base: run.base.into(),
        routed_worktree: run.worktree.into(),
        routed_engine: run.provider.into(),
        trajectory: run.trajectory.map(|p| p.display().to_string()),
        routed_cost_microusd: run.cost_microusd,
        engine: engine.into(),
        model: (engine == "codex").then(codex_model).flatten(),
        request: request.clone(),
        started_ms: now_ms(),
    };
    std::fs::write(folder.join("request.txt"), &request)?;
    std::fs::write(
        folder.join("spec.json"),
        serde_json::to_vec_pretty(&spec).map_err(std::io::Error::other)?,
    )?;
    // The raw engine on its own defaults: only what a headless run needs
    // (its output format, and approval of its own permission asks, as the
    // routed run has full access).
    let command = if engine == "claude" {
        format!(
            "{} -p \"$(cat {})\" --output-format stream-json --verbose --dangerously-skip-permissions",
            quote(&bin.to_string_lossy()),
            quote(&folder.join("request.txt").to_string_lossy())
        )
    } else {
        format!(
            "{} exec --json --dangerously-bypass-approvals-and-sandbox --skip-git-repo-check \"$(cat {})\"",
            quote(&bin.to_string_lossy()),
            quote(&folder.join("request.txt").to_string_lossy())
        )
    };
    let script = format!(
        "#!/bin/sh\ncd {work} || exit 1\nGIT_TERMINAL_PROMPT=0 {command} > {out} 2> {err} < /dev/null\necho $? > {exit}.tmp && mv {exit}.tmp {exit}\n",
        work = quote(&work_text),
        out = quote(&folder.join("out.jsonl").to_string_lossy()),
        err = quote(&folder.join("err.txt").to_string_lossy()),
        exit = quote(&folder.join("exit").to_string_lossy()),
    );
    let script_path = folder.join("run.sh");
    std::fs::write(&script_path, script)?;
    let mut process = std::process::Command::new("/bin/sh");
    process
        .arg(&script_path)
        .current_dir(&work)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        process.process_group(0);
    }
    process.spawn()?;
    Ok(())
}

/// The pending baselines' folders.
fn pending(shadow: &Path) -> impl Iterator<Item = PathBuf> {
    std::fs::read_dir(shadow.join("pending"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.join("spec.json").is_file())
}

/// Every record written so far.
#[must_use]
pub fn records(shadow: &Path) -> Vec<Value> {
    std::fs::read_to_string(shadow.join("records.jsonl"))
        .map(|text| {
            text.lines()
                .filter_map(|line| serde_json::from_str(line).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// The checks the recipe kept for the routed run, from its trajectory.
fn kept_checks(trajectory: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(trajectory) else {
        return Vec::new();
    };
    for line in text
        .lines()
        .filter(|line| line.contains("\"delegate_recipe\""))
    {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let kept = &value["step"]["extensions"]["delegate_recipe"]["run"]["checks"]["kept"];
        if let Some(list) = kept.as_array() {
            return list
                .iter()
                .filter_map(|c| c.as_str().or_else(|| c["command"].as_str()))
                .take(MAX_CHECKS)
                .map(str::to_owned)
                .collect();
        }
    }
    Vec::new()
}

/// Whether `command` exits 0 in `dir` within [`CHECK_SECONDS`].
fn passes(command: &str, dir: &Path) -> Option<bool> {
    if !dir.is_dir() {
        return None;
    }
    let mut child = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(command)
        .current_dir(dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let deadline = std::time::Instant::now() + Duration::from_secs(CHECK_SECONDS);
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            return Some(status.success());
        }
        if std::time::Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Some(false);
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// What the baseline's output says it cost and did.
#[must_use]
pub fn baseline_summary(engine: &str, model: Option<&str>, output: &str) -> Value {
    if engine == "codex" {
        let summary = coder_delegate::delegate::Summary::parse_codex(output, model.unwrap_or(""));
        let tokens = |key| summary.tokens(key);
        json!({
            "engine": "codex",
            "model": model,
            "finished": summary.has_result,
            "cost_usd": summary.total_cost_usd,
            "cost_basis": "list_price_from_tokens",
            "turns": summary.num_turns,
            "input_tokens": tokens("input_tokens"),
            "cached_input_tokens": tokens("cached_input_tokens"),
            "output_tokens": tokens("output_tokens"),
        })
    } else {
        let summary = coder_delegate::delegate::Summary::parse(output);
        let tokens = |key| summary.tokens(key);
        let input = [
            "input_tokens",
            "cache_creation_input_tokens",
            "cache_read_input_tokens",
        ]
        .iter()
        .map(|key| tokens(key).unwrap_or(0))
        .sum::<u64>();
        json!({
            "engine": "claude",
            "model": summary.model,
            "finished": summary.has_result && summary.is_error != Some(true),
            "cost_usd": summary.total_cost_usd,
            "cost_basis": "reported",
            "turns": summary.num_turns,
            "input_tokens": input,
            "cache_read_input_tokens": tokens("cache_read_input_tokens"),
            "output_tokens": tokens("output_tokens"),
        })
    }
}

/// Finish every baseline that has ended: write its record and delete its
/// scratch copy. Returns the records written now.
#[must_use]
pub fn finalize(store: &Path) -> Vec<Value> {
    let shadow = dir(store);
    let journal = openagents_chat::route::Journal::beside(store);
    let mut written = Vec::new();
    for folder in pending(&shadow).collect::<Vec<_>>() {
        let Ok(exit) = std::fs::read_to_string(folder.join("exit")) else {
            continue;
        };
        let Some(spec) = std::fs::read(folder.join("spec.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Spec>(&bytes).ok())
        else {
            continue;
        };
        let ended_ms = std::fs::metadata(folder.join("exit"))
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or_else(now_ms, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX));
        let output = std::fs::read_to_string(folder.join("out.jsonl")).unwrap_or_default();
        let mut baseline = baseline_summary(&spec.engine, spec.model.as_deref(), &output);
        baseline["exit"] = json!(exit.trim().parse::<i64>().ok());
        baseline["wall_ms"] = json!(ended_ms.saturating_sub(spec.started_ms));
        let work = folder.join("work");
        baseline["files_changed"] = json!(changed_files(&work));
        // The routed side, from its route record (cost and wall time).
        let run = spec
            .thread
            .as_deref()
            .and_then(|thread| journal.of_task(thread, &spec.task))
            .and_then(|record| record.runs.into_iter().find(|run| run.task == spec.task));
        let routed_cost = run
            .as_ref()
            .and_then(|run| run.cost_microusd)
            .or(spec.routed_cost_microusd);
        let checks = spec
            .trajectory
            .as_deref()
            .map(|t| kept_checks(Path::new(t)))
            .unwrap_or_default();
        let checked: Vec<Value> = checks
            .iter()
            .map(|command| {
                json!({
                    "command": command,
                    "routed": passes(command, Path::new(&spec.routed_worktree)),
                    "baseline": passes(command, &work),
                })
            })
            .collect();
        let record = json!({
            "schema": SCHEMA,
            "task": spec.task,
            "thread": spec.thread,
            "checkout": spec.checkout,
            "base": spec.base,
            "routed": {
                "engine": spec.routed_engine,
                "cost_usd": routed_cost.map(|m| m as f64 / 1e6),
                "wall_ms": run.as_ref().and_then(|run| run.wall_ms),
            },
            "baseline": baseline,
            "checks": checked,
            "started_ms": spec.started_ms,
            "applied": false,
        });
        if append(&shadow, &record).is_ok() {
            let _ = std::fs::remove_dir_all(&folder);
            written.push(record);
        }
    }
    written
}

fn changed_files(work: &Path) -> Option<usize> {
    let output = std::process::Command::new("git")
        .args(["status", "--porcelain", "-uall"])
        .current_dir(work)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).lines().count())
}

fn append(shadow: &Path, record: &Value) -> std::io::Result<()> {
    use std::io::Write as _;
    std::fs::create_dir_all(shadow)?;
    let mut line = serde_json::to_vec(record).map_err(std::io::Error::other)?;
    line.push(b'\n');
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(shadow.join("records.jsonl"))?;
    file.write_all(&line)
}

fn median(mut values: Vec<f64>) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(f64::total_cmp);
    let mid = values.len() / 2;
    Some(if values.len() % 2 == 0 {
        (values[mid - 1] + values[mid]) / 2.0
    } else {
        values[mid]
    })
}

/// Totals and medians over `records`, routed against baseline, on the
/// pairs where both sides' figure is known.
#[must_use]
pub fn report(records: &[Value]) -> Value {
    let pairs = |side: &str, key: &str| -> Vec<(f64, f64)> {
        records
            .iter()
            .filter_map(|r| Some((r["routed"][key].as_f64()?, r[side][key].as_f64()?)))
            .collect()
    };
    let cost = pairs("baseline", "cost_usd");
    let wall: Vec<(f64, f64)> = pairs("baseline", "wall_ms")
        .into_iter()
        .map(|(a, b)| (a / 1000.0, b / 1000.0))
        .collect();
    let side = |pairs: &[(f64, f64)]| {
        let routed: f64 = pairs.iter().map(|p| p.0).sum();
        let baseline: f64 = pairs.iter().map(|p| p.1).sum();
        json!({
            "n": pairs.len(),
            "routed_total": routed,
            "baseline_total": baseline,
            "routed_median": median(pairs.iter().map(|p| p.0).collect()),
            "baseline_median": median(pairs.iter().map(|p| p.1).collect()),
            "saving": (baseline > 0.0).then(|| 1.0 - routed / baseline),
        })
    };
    let count = |who: &str, want: bool| {
        records
            .iter()
            .flat_map(|r| r["checks"].as_array().cloned().unwrap_or_default())
            .filter(|c| c[who].as_bool() == Some(want))
            .count()
    };
    json!({
        "records": records.len(),
        "cost_usd": side(&cost),
        "wall_s": side(&wall),
        "checks": {
            "routed_passed": count("routed", true),
            "routed_failed": count("routed", false),
            "baseline_passed": count("baseline", true),
            "baseline_failed": count("baseline", false),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run<'a>(task: &'a str) -> Finished<'a> {
        Finished {
            task,
            thread: Some("t"),
            turn: 1,
            prompt: "fix it",
            checkout: "/tmp/repo",
            worktree: "/tmp/wt",
            base: "abc",
            provider: "claude",
            ending: "model_finished",
            cost_microusd: Some(1),
            trajectory: None,
            issue_flow: false,
        }
    }

    #[test]
    fn sampling_is_the_task_ids_own_and_scales_with_the_percent() {
        let ids: Vec<String> = (0..1000).map(|i| format!("{i:064x}")).collect();
        let at = |p| ids.iter().filter(|id| sampled(id, p)).count();
        assert_eq!(at(100), 1000);
        assert!((50..150).contains(&at(10)), "{}", at(10));
        assert_eq!(sampled(&ids[3], 37), sampled(&ids[3], 37));
    }

    #[test]
    fn it_is_off_unless_set_and_skips_what_it_cannot_compare() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("tasks");
        let mut coder = super::super::settings::Coder::default();
        assert_eq!(decide(&store, &coder, &run("a")), Decision::Off);
        coder.shadow_percent = Some(100);
        assert!(matches!(
            decide(&store, &coder, &run("a")),
            Decision::Started(_)
        ));
        let mut later = run("a");
        later.turn = 2;
        assert!(matches!(
            decide(&store, &coder, &later),
            Decision::Ineligible(_)
        ));
        let mut issue = run("a");
        issue.issue_flow = true;
        assert!(matches!(
            decide(&store, &coder, &issue),
            Decision::Ineligible(_)
        ));
        let mut grok = run("a");
        grok.provider = "grok";
        assert!(matches!(
            decide(&store, &coder, &grok),
            Decision::Ineligible(_)
        ));
        let mut asked = run("a");
        asked.ending = "question";
        assert!(matches!(
            decide(&store, &coder, &asked),
            Decision::Ineligible(_)
        ));
    }

    #[test]
    fn one_runs_at_a_time_and_the_persons_budget_stops_new_ones() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("tasks");
        let shadow = super::dir(&store);
        let mut coder = super::super::settings::Coder::default();
        coder.shadow_percent = Some(100);
        let busy = shadow.join("pending/x");
        std::fs::create_dir_all(&busy).unwrap();
        std::fs::write(busy.join("spec.json"), "{}").unwrap();
        assert_eq!(decide(&store, &coder, &run("a")), Decision::Busy);
        std::fs::remove_dir_all(&busy).unwrap();
        append(
            &shadow,
            &json!({"routed": {}, "baseline": {"cost_usd": 0.6}}),
        )
        .unwrap();
        coder.shadow_budget_cents = Some(100);
        assert!(matches!(
            decide(&store, &coder, &run("a")),
            Decision::Started(_)
        ));
        append(
            &shadow,
            &json!({"routed": {}, "baseline": {"cost_usd": 0.5}}),
        )
        .unwrap();
        assert!(matches!(
            decide(&store, &coder, &run("a")),
            Decision::OverBudget { .. }
        ));
        coder.shadow_budget_cents = None;
        assert!(matches!(
            decide(&store, &coder, &run("a")),
            Decision::Started(_)
        ));
    }

    #[test]
    fn a_claude_baseline_reads_its_reported_cost_and_tokens() {
        let stream = concat!(
            r#"{"type":"system","subtype":"init","model":"claude-opus-5-5","session_id":"s"}"#,
            "\n",
            r#"{"type":"result","subtype":"success","is_error":false,"num_turns":4,"total_cost_usd":0.42,"usage":{"input_tokens":10,"cache_creation_input_tokens":100,"cache_read_input_tokens":1000,"output_tokens":50}}"#,
            "\n"
        );
        let summary = baseline_summary("claude", None, stream);
        assert_eq!(summary["cost_usd"], 0.42);
        assert_eq!(summary["turns"], 4);
        assert_eq!(summary["input_tokens"], 1110);
        assert_eq!(summary["finished"], true);
    }

    #[test]
    fn the_report_pairs_known_figures_and_counts_checks() {
        let records = vec![
            json!({"routed": {"cost_usd": 0.2, "wall_ms": 10_000}, "baseline": {"cost_usd": 0.5, "wall_ms": 20_000},
                   "checks": [{"routed": true, "baseline": false}]}),
            json!({"routed": {"cost_usd": 0.4, "wall_ms": 30_000}, "baseline": {"cost_usd": 0.5, "wall_ms": 30_000},
                   "checks": [{"routed": true, "baseline": true}]}),
            json!({"routed": {"cost_usd": null}, "baseline": {"cost_usd": 0.9}, "checks": []}),
        ];
        let report = report(&records);
        assert_eq!(report["cost_usd"]["n"], 2);
        assert!((report["cost_usd"]["saving"].as_f64().unwrap() - 0.4).abs() < 1e-9);
        assert_eq!(report["wall_s"]["routed_median"], 20.0);
        assert_eq!(report["checks"]["routed_passed"], 2);
        assert_eq!(report["checks"]["baseline_failed"], 1);
    }

    #[test]
    fn kept_checks_come_from_the_recipe_record() {
        let dir = tempfile::tempdir().unwrap();
        let trajectory = dir.path().join("t.atif.jsonl");
        std::fs::write(
            &trajectory,
            concat!(
                r#"{"record":"session"}"#,
                "\n",
                r#"{"record":"step","step":{"extensions":{"delegate_recipe":{"run":{"checks":{"kept":["pytest -q","make test","x"]}}}}}}"#,
                "\n"
            ),
        )
        .unwrap();
        assert_eq!(kept_checks(&trajectory), vec!["pytest -q", "make test"]);
        assert_eq!(passes("exit 0", dir.path()), Some(true));
        assert_eq!(passes("exit 3", dir.path()), Some(false));
    }
}
