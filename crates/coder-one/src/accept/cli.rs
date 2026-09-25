//! `coder-one accept …`: define, run, and check acceptance suites, run the
//! mini-task loop, and measure offline validity.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::json;

use super::minitask::{LoopOptions, default_out, run_minitask};
use super::offline::{self, TaskOptions};
use super::{AcceptanceSuite, Docker, Local, Task, authority, discriminate, run};
use crate::component::jev::JevMode;

/// The accept commands' usage.
pub const USAGE: &str = "usage: coder-one accept minitask ID [--sessions N] [--rounds N]
                                  [--jev live|off] [--out DIR] [--echo]
       coder-one accept offline TASK [--trials NAME,…] [--image IMAGE]
                                  [--kinds KIND,…] [--grades DIR]…
                                  [--reconstruction DIR]… [--workers N]
                                  [--jobs DIR] [--rounds N] [--jev live|off]
                                  [--out DIR] [--reuse] [--facts ANATOMY.json]
                                  [--containers PREFIX] [--echo]
       coder-one accept classify RECORD… --out FILE [--jev live|off]
                                  [--contract FILE] [--tasks DIR]
       coder-one accept validity DIR… [--rows FILE] [--authority FILE]
                                  [--in-sample TASK,…] [--json]
       coder-one accept run RECORD WORKSPACE [--docker IMAGE --workdir DIR
                                  [--candidate DIR]] [--json]
       coder-one accept check RECORD
       coder-one accept grade [--traces DIR] [--grades DIR] [--out DIR]
                                  [--jev live|recorded|off] [--recorded FILE]
                                  [--workers N] [--score-sec N]

minitask sets a mini-task up, extracts its requirement map with Jev,
writes and freezes an acceptance suite with Microluna (accept.define),
then runs Microluna edit sessions until the suite is green or the
sessions run out, and grades the result with the task's own grader.

offline writes a suite for a Terminal-Bench task from its instruction, in
its warm environment image, or reuses the one frozen under --out with
--reuse, and runs it on every retained workspace of the task's trials
under --jobs, each in a fresh container with no network. It reads four
kinds: a Coder One trial's post-executor snapshot (snapshot), a Microluna
trial's final workspace, the image's working directory with the
artifacts Harbor collected over it (final), each candidate a Microluna
lean loop retained, checked against its recorded file identity
(candidate), and a reconstruction directory given with --reconstruction
(reconstruction). --kinds keeps only the kinds named. A candidate's
reward is known when its files are the submitted workspace's or when a
grade record under a --grades directory matches them. --facts gives the
writer a task-anatomy file's decisive facts and test ideas for the task,
keeping only those the instruction or the workspace supports.

offline names its containers after --containers, with a unique suffix,
so an interrupted run's containers can be found and removed.

classify gives every test of each frozen suite record an authority class
(accept::authority): executed contract, independently supported,
writer-derived, guard, or unsupported, with the evidence for it. Code
reads each test's run on the untouched workspace; Jev answers two
questions about each test that was red there. --contract names a JSON
file of executed-contract items, {DIGEST: {TEST: ITEM}}, as an
executed-contract extractor writes them. A suite already in --out keeps
its classes, so a rerun asks nothing again. The task is the path
component that names a task directory under --tasks (by default the
Terminal-Bench 4.0 checkout's tasks).

validity joins the offline records under each DIR with the check-truth
label rows and prints how often a green suite, today's checks, and the
combined verdict agree with the verifier, for Coder One snapshots,
Microluna final workspaces, and Microluna candidates apart. A trial whose
verifier reward is unknown is counted as unknown, never as a failure.
With --authority, it also prints each class's discrimination within a
task, with 95% Wilson intervals, on the digest-parity task split, and
whether the class passes the bar for power in a policy. --in-sample
moves the named tasks to the calibration side, such as the tasks whose
failures the classes were designed from.

run runs a frozen suite, from its record, on a workspace on this host
(inside a coder-boundary writing boundary) or in a Docker image. check
reports whether the suite was edited since its freeze; the exit code is 1
when it was.

grade grades every retained frozen score script with accept.grade, runs
it on every graded workspace of its task, and reports whether ranking on
the supported check lines separates passes from failures where the raw
score doesn't (`coder-one accept grade --help`).";

fn jev_mode(word: &str) -> Result<JevMode, String> {
    match word {
        "live" => Ok(JevMode::Live(crate::component::cli::live_client()?)),
        "off" => Ok(JevMode::Off),
        other => Err(format!("--jev takes live or off, not {other}")),
    }
}

/// Runs an accept command and returns the exit code.
///
/// # Errors
///
/// A message for bad arguments or a run that can't start.
#[allow(clippy::too_many_lines)]
pub async fn command(args: &[String]) -> Result<i32, String> {
    let Some((verb, rest)) = args.split_first() else {
        return Err(USAGE.to_string());
    };
    if verb == "grade" {
        return crate::grade::offline::command(rest).await;
    }
    let mut positional = Vec::new();
    let mut sessions = 4u32;
    let mut rounds = None;
    let mut jev = "live".to_string();
    let mut out = None;
    let mut echo = false;
    let mut json_output = false;
    let mut trials = Vec::new();
    let mut image = None;
    let mut reuse = false;
    let mut facts = None;
    let mut rows = None;
    let mut docker_image = None;
    let mut workdir = "/app".to_string();
    let mut candidate = None;
    let mut kinds = Vec::new();
    let mut grades = Vec::new();
    let mut reconstructions = Vec::new();
    let mut workers = 4usize;
    let mut jobs = None;
    let mut authority_file = None;
    let mut in_sample: Vec<String> = Vec::new();
    let mut contract = None;
    let mut tasks = None;
    let mut iter = rest.iter();
    while let Some(arg) = iter.next() {
        let mut value = |name: &str| {
            iter.next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match arg.as_str() {
            "--sessions" => {
                sessions = value("--sessions")?
                    .parse()
                    .map_err(|_| "--sessions takes a count")?;
            }
            "--rounds" => {
                rounds = Some(
                    value("--rounds")?
                        .parse::<u32>()
                        .map_err(|_| "--rounds takes a count")?,
                );
            }
            "--jev" => jev = value("--jev")?,
            "--out" => out = Some(PathBuf::from(value("--out")?)),
            "--echo" => echo = true,
            "--json" => json_output = true,
            "--trials" => {
                trials = value("--trials")?.split(',').map(str::to_string).collect();
            }
            "--image" => image = Some(value("--image")?),
            "--reuse" => reuse = true,
            "--facts" => facts = Some(PathBuf::from(value("--facts")?)),
            "--rows" => rows = Some(PathBuf::from(value("--rows")?)),
            "--docker" => docker_image = Some(value("--docker")?),
            "--workdir" => workdir = value("--workdir")?,
            "--candidate" => candidate = Some(PathBuf::from(value("--candidate")?)),
            "--kinds" => {
                kinds = value("--kinds")?
                    .split(',')
                    .map(offline::Kind::parse)
                    .collect::<Result<_, _>>()?;
            }
            "--grades" => grades.push(PathBuf::from(value("--grades")?)),
            "--reconstruction" => reconstructions.push(PathBuf::from(value("--reconstruction")?)),
            "--workers" => {
                workers = value("--workers")?
                    .parse()
                    .map_err(|_| "--workers takes a count")?;
            }
            "--jobs" => jobs = Some(PathBuf::from(value("--jobs")?)),
            "--authority" => authority_file = Some(PathBuf::from(value("--authority")?)),
            "--in-sample" => {
                in_sample = value("--in-sample")?
                    .split(',')
                    .map(str::to_string)
                    .collect();
            }
            "--tasks" => tasks = Some(PathBuf::from(value("--tasks")?)),
            "--contract" => contract = Some(PathBuf::from(value("--contract")?)),
            "--containers" => super::runner::name_containers(&value("--containers")?),
            other if other.starts_with("--") => return Err(format!("unknown option {other}")),
            other => positional.push(other.to_string()),
        }
    }
    match verb.as_str() {
        "minitask" => {
            let [id] = positional.as_slice() else {
                return Err(USAGE.to_string());
            };
            let out = out
                .or_else(|| default_out(id))
                .ok_or("no home directory for the run; pass --out")?;
            let mut options = LoopOptions {
                sessions,
                echo,
                ..LoopOptions::default()
            };
            if let Some(rounds) = rounds {
                options.define.max_rounds = rounds;
            }
            let result = run_minitask(id, &out, &jev_mode(&jev)?, &options).await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "task": result["task"],
                    "suite": result["suite"]["headline"],
                    "runs": result["runs"],
                    "stopped": result["stopped"],
                    "green": result["green"],
                    "grade": result["grade"],
                    "green_agrees_with_grader": result["green_agrees_with_grader"],
                    "spend_usd": result["spend_usd"],
                    "record": out.join("result.json"),
                }))
                .unwrap_or_default()
            );
            Ok(if result["grade"]["verdict"] == "passed" {
                0
            } else {
                1
            })
        }
        "offline" => {
            let [task] = positional.as_slice() else {
                return Err(USAGE.to_string());
            };
            let home = crate::credentials::openagents_dir()
                .ok_or("no home directory; pass --out")?
                .join("terminal-bench");
            let mut options = TaskOptions {
                jobs: jobs.unwrap_or_else(|| home.join("jobs")),
                tasks_dir: home.join("upstream/terminal-bench-v4.0.0/tasks"),
                out: out.unwrap_or_else(|| home.join("accept-offline")),
                image,
                reuse,
                only: trials,
                kinds,
                grades,
                reconstructions,
                workers,
                define: super::Options::default(),
                model: "gpt-6-luna".to_string(),
                writer_turns: 50,
                writer_sec: 900,
                echo,
                facts,
            };
            if let Some(rounds) = rounds {
                options.define.max_rounds = rounds;
            }
            let value = offline::task(task, &jev_mode(&jev)?, &options).await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "task": value["task"],
                    "suite": value["suite"]["headline"],
                    "writer_usd": value["suite"]["writer_usd"],
                    "jev_usd": value["suite"]["jev_usd"],
                    "trials": value["trials"].as_array().map(|t| t.iter().map(|e| json!({
                        "trial": e["trial"]["trial"],
                        "kind": e["trial"]["kind"],
                        "reward": e["trial"]["reward"],
                        "snapshot_graded": e["trial"]["snapshot_graded"],
                        "passed": e["run"]["passed"],
                        "total": e["run"]["total"],
                        "error": e["error"],
                    })).collect::<Vec<_>>()),
                }))
                .unwrap_or_default()
            );
            Ok(0)
        }
        "classify" => {
            if positional.is_empty() {
                return Err(USAGE.to_string());
            }
            let out = out.ok_or("classify needs --out FILE")?;
            let home = crate::credentials::openagents_dir()
                .ok_or("no home directory")?
                .join("terminal-bench");
            let tasks_dir =
                tasks.unwrap_or_else(|| home.join("upstream/terminal-bench-v4.0.0/tasks"));
            let contract: Contract = match contract {
                Some(path) => serde_json::from_str(
                    &std::fs::read_to_string(&path)
                        .map_err(|e| format!("cannot read {}: {e}", path.display()))?,
                )
                .map_err(|e| format!("{} is not a contract file: {e}", path.display()))?,
                None => BTreeMap::new(),
            };
            let record =
                classify(&positional, &out, &tasks_dir, &contract, &jev_mode(&jev)?).await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "out": out,
                    "suites": record.suites.iter().map(|(digest, s)| json!({
                        "task": s.task,
                        "digest": &digest[..digest.len().min(12)],
                        "classes": authority::Authority::ALL.iter().filter_map(|a| {
                            let n = s.tests.values().filter(|c| c.class == *a).count();
                            (n > 0).then(|| (a.word(), n))
                        }).collect::<BTreeMap<_, _>>(),
                        "jev_usd": s.jev_usd,
                    })).collect::<Vec<_>>(),
                    "jev_requests": record.jev_requests,
                    "jev_usd": record.jev_usd,
                }))
                .unwrap_or_default()
            );
            Ok(0)
        }
        "validity" => {
            if positional.is_empty() {
                return Err(USAGE.to_string());
            }
            let rows = rows.unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/truth/rows.jsonl")
            });
            let dirs: Vec<PathBuf> = positional.iter().map(PathBuf::from).collect();
            let joined = offline::join_all(&dirs, &rows)?;
            let mut value = offline::validity(&joined);
            let classes = match &authority_file {
                Some(path) => {
                    let record = authority::Record::load(path)?;
                    let measured = discriminate::measure(&joined, &record, &in_sample);
                    value["authority"] = measured.clone();
                    value["authority_rows"] = json!(discriminate::rows(&joined, &record));
                    Some(measured)
                }
                None => None,
            };
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&value).unwrap_or_default()
                );
            } else {
                for (set, label) in offline::SETS {
                    if value[set]["suite_green"]["trials"] == 0
                        && value[set]["suite_green"]["unknown"] == 0
                    {
                        continue;
                    }
                    println!("{label}:");
                    for signal in [
                        "suite_green",
                        "suite_complete",
                        "todays_checks",
                        "combined_verdict",
                    ] {
                        let a = &value[set][signal];
                        println!(
                            "  {signal:<17} spoke {}/{}, agreed {}, fail right {}/{}, pass right {}/{}, failures caught {}/{}, passes kept {}/{}, unknown reward {}",
                            a["spoke"],
                            a["trials"],
                            a["agreed"],
                            a["fail_right"],
                            a["fail_called"],
                            a["pass_right"],
                            a["pass_called"],
                            a["fail_right"],
                            a["failures"],
                            a["pass_right"],
                            a["passes"],
                            a["unknown"]
                        );
                    }
                }
                if let Some(measured) = &classes {
                    print_classes(measured);
                }
                for j in &joined {
                    println!(
                        "{:<60} {:<14} reward {:<4} graded {:<5} suite {:<5} ({}/{}) checks {:<5} verdict {}",
                        j.trial,
                        format!("{:?}", j.kind).to_lowercase(),
                        j.reward.map_or("?".to_string(), |r| r.to_string()),
                        j.snapshot_graded,
                        word(j.suite),
                        j.passed,
                        j.total,
                        word(j.checks),
                        word(j.verdict)
                    );
                }
            }
            Ok(0)
        }
        "run" => {
            let [record, workspace] = positional.as_slice() else {
                return Err(USAGE.to_string());
            };
            let suite = AcceptanceSuite::load(Path::new(record))?;
            let workspace = if docker_image.is_some() {
                PathBuf::from(workspace)
            } else {
                PathBuf::from(workspace)
                    .canonicalize()
                    .map_err(|e| format!("cannot use the workspace {workspace}: {e}"))?
            };
            let result = match docker_image {
                Some(image) => {
                    let runner = Docker {
                        image,
                        workdir: workdir.clone(),
                        candidate,
                        test_sec: 120,
                        dev: None,
                        setup: None,
                    };
                    run(&suite, &workspace, &runner, None, "cli").await
                }
                None => run(&suite, &workspace, &Local::writing(120), None, "cli").await,
            }
            .map_err(|tampered| tampered.to_string())?;
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&result).unwrap_or_default()
                );
            } else {
                for test in &result.tests {
                    println!(
                        "{} {} ({})",
                        if test.green { "GREEN" } else { "RED  " },
                        test.id,
                        test.requirements.join(", ")
                    );
                }
                println!("{} of {} green", result.passed, result.total);
            }
            Ok(i32::from(!result.green))
        }
        "check" => {
            let [record] = positional.as_slice() else {
                return Err(USAGE.to_string());
            };
            let suite = AcceptanceSuite::load(Path::new(record))?;
            let integrity = suite.integrity();
            println!(
                "{}",
                serde_json::to_string_pretty(&integrity).unwrap_or_default()
            );
            Ok(i32::from(!integrity.intact))
        }
        _ => Err(USAGE.to_string()),
    }
}

/// Executed-contract items by suite digest and test ID.
type Contract = BTreeMap<String, BTreeMap<String, serde_json::Value>>;

/// Classifies each suite record in `records` and writes the classes to
/// `out`, keeping the suites `out` already holds.
async fn classify(
    records: &[String],
    out: &Path,
    tasks_dir: &Path,
    contract: &Contract,
    jev: &JevMode,
) -> Result<authority::Record, String> {
    let mut record = if out.is_file() {
        authority::Record::load(out)?
    } else {
        authority::Record {
            schema: authority::SCHEMA.to_string(),
            thresholds: Some(authority::Thresholds::default()),
            ..authority::Record::default()
        }
    };
    let thresholds = record.thresholds.unwrap_or_default();
    let none = BTreeMap::new();
    for path in records {
        let path = Path::new(path);
        let mut suite = AcceptanceSuite::load(path)?;
        let beside = path.with_file_name(
            path.file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_suffix(".accept.json"))
                .ok_or_else(|| format!("{} isn't a NAME.accept.json record", path.display()))?,
        );
        if beside.is_dir() && suite.dir != beside {
            // A retained suite is read from where it is now; its digest
            // still has to match its files.
            suite.dir.clone_from(&beside);
            for test in &mut suite.tests {
                test.source = std::fs::read_to_string(beside.join(&test.path)).unwrap_or_default();
            }
        }
        if !suite.integrity().intact {
            return Err(format!(
                "{}: the suite's files don't match its digest",
                path.display()
            ));
        }
        if record.suites.contains_key(&suite.digest) {
            continue;
        }
        let task = task_of(path, tasks_dir)
            .ok_or_else(|| format!("{}: no path component names a task", path.display()))?;
        let instruction = std::fs::read_to_string(tasks_dir.join(&task).join("instruction.md"))
            .map_err(|e| format!("cannot read {task}'s instruction: {e}"))?;
        if super::sha256(instruction.as_bytes()) != suite.instruction_sha256 {
            crate::say::line(&format!(
                "accept ▸ {}: written from another version of {task}'s instruction",
                path.display()
            ));
        }
        let items = contract.get(&suite.digest).unwrap_or(&none);
        let spent = authority::classify_suite(
            &mut suite,
            &Task {
                title: task.clone(),
                instruction,
            },
            items,
            jev,
            &crate::record::Recorder::default(),
            &thresholds,
        )
        .await;
        crate::say::line(&format!(
            "accept ▸ {task} {}: {} tests classified, {} Jev requests, ${:.4}",
            &suite.digest[..12],
            suite.authority.len(),
            spent.jev_requests,
            spent.jev_usd
        ));
        record.jev_requests += spent.jev_requests;
        record.jev_usd += spent.jev_usd;
        record.suites.insert(
            suite.digest.clone(),
            authority::SuiteClasses {
                task,
                record: path.display().to_string(),
                tests: suite.authority,
                jev_requests: spent.jev_requests,
                jev_usd: spent.jev_usd,
            },
        );
        // Written after every suite, so an interrupted run keeps what it
        // paid for.
        let text = serde_json::to_string_pretty(&record).map_err(|e| e.to_string())?;
        crate::record::write_atomic(out, format!("{text}\n").as_bytes())?;
    }
    Ok(record)
}

/// The task a record belongs to: the nearest path component, or the part
/// of it before `__`, that names a directory under `tasks_dir`.
fn task_of(path: &Path, tasks_dir: &Path) -> Option<String> {
    path.ancestors().find_map(|dir| {
        let name = dir.file_name()?.to_str()?;
        let task = name.split("__").next()?;
        (!task.is_empty() && tasks_dir.join(task).join("instruction.md").is_file())
            .then(|| task.to_string())
    })
}

/// Prints each class's discrimination, set by set and split by split.
fn print_classes(measured: &serde_json::Value) {
    let text = |r: &serde_json::Value| {
        serde_json::from_value::<crate::checks::truth::Rate>(r.clone())
            .map_or_else(|_| "-".to_string(), |r| r.text())
    };
    println!("authority classes, within task (groups of one task and one suite):");
    for (set, label) in [
        ("graded", "workspaces the verifier graded"),
        ("known_reward", "every workspace with a known reward"),
    ] {
        for split in ["all", "calibration", "held_out"] {
            println!("  {label}, {split}:");
            for class in authority::Authority::ALL {
                let c = &measured["sets"][set][split][class.word()];
                if c["rows"] == 0 {
                    continue;
                }
                println!(
                    "    {:<24} rows {} tasks {}; mixed tasks {} groups {}; passes green {}; failures red {}; pairs +{} -{} ={} order {}; separating groups {}",
                    class.word(),
                    c["rows"],
                    c["tasks"],
                    c["mixed_tasks"],
                    c["mixed_groups"],
                    text(&c["passes_green"]),
                    text(&c["failures_red"]),
                    c["concordant"],
                    c["discordant"],
                    c["tied"],
                    text(&c["order"]),
                    c["separating_groups"]
                );
            }
        }
    }
    println!("the bar, on graded workspaces of held-out tasks:");
    for class in authority::Authority::ALL {
        let v = &measured["verdicts"][class.word()];
        println!(
            "  {:<24} {}: {}",
            class.word(),
            if v["passes"] == true {
                "passes"
            } else {
                "doesn't pass"
            },
            v["why"].as_str().unwrap_or_default()
        );
    }
}

fn word(says: Option<crate::checks::truth::Says>) -> &'static str {
    match says {
        Some(crate::checks::truth::Says::Pass) => "pass",
        Some(crate::checks::truth::Says::Fail) => "fail",
        None => "-",
    }
}
