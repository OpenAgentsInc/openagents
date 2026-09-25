//! `coder-one checks contract …`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use super::host::Local;
use super::{Plan, extract, report, run};
use crate::component::jev::{JevMode, Recorded, RecordedAnswer};
use crate::record::Recorder;

/// The contract commands' usage.
pub const USAGE: &str = "usage: coder-one checks contract plan --instruction FILE [--task NAME]
                                       [--workdir DIR] [--jev off|recorded|live]
                                       [--recorded FILE] [--out FILE]
       coder-one checks contract literal-plan --instruction FILE [--task NAME]
                                       [--workdir DIR] [--out FILE]
       coder-one checks contract literal-run --plan FILE [--out FILE]
       coder-one checks contract run --plan FILE [--out FILE]
       coder-one checks contract offline TASK... --out DIR [--jobs DIR] [--tasks DIR]
                                       [--image IMAGE] [--grades DIR]... [--reconstruction DIR]...
                                       [--kinds snapshot,final,candidate,reconstruction]
                                       [--trials NAME,...] [--exclude-job TEXT]...
                                       [--workers N] [--jev off|recorded|live] [--reuse-plan]
       coder-one checks contract executed TASK... --out DIR [the offline options]
                                       [--command-sec N] [--budget-sec N]

plan extracts, by code, the items a task's instruction states (commands,
example invocations, output paths, formats, and exit statuses) from the
instruction and the untouched workspace at --workdir (/app by default),
which it reads as given, and writes the plan. run runs a plan's items in
its working directory, each command inside a writing boundary under
supervise, and prints one report. offline makes each task's plan in a
fresh container of its image and runs it on every retained workspace that
`coder-one accept offline` reads, each in its own container with no
network; it writes <out>/<task>/plan.json, contract.json, and labels.json.
Jev settles only what the words leave open, recorded in
<out>/jev-recorded.json (or --recorded) and replayed from it; --jev off
leaves those items not executable. executed is verify.executed on the same
workspaces, with no model: in the untouched container it runs the commands
the instruction names and a compile or import of the package, then runs
them on every retained workspace and judges each against its untouched
outcome; it writes <out>/<task>/executed.json and labels.json.
literal-plan extracts explicit required output paths and byte limits without a
model. literal-run checks their sealed plan through file metadata only. These
separate versioned artifacts call fail or abstain, never task completion; they
do not change the ordinary plan command or a runtime policy.";

fn jev_modes(word: &str, recorded: &Recorded) -> Result<(JevMode, Option<JevMode>), String> {
    let replay = JevMode::Recorded(recorded.clone());
    match word {
        "off" => Ok((JevMode::Off, None)),
        "recorded" => Ok((replay, None)),
        "live" => {
            let dir = crate::credentials::openagents_dir().ok_or("HOME is not set")?;
            let key = crate::credentials::jev_key(|name| std::env::var(name).ok(), &dir)?;
            let client = crate::credentials::jev_client(&key.secret)?;
            Ok((JevMode::Live(client), Some(replay)))
        }
        other => Err(format!("--jev is off, recorded, or live, not {other}")),
    }
}

/// Adds a plan's live answers to `recorded`; returns how many it added.
fn keep_live(plan: &Plan, recorded: &mut Recorded) -> usize {
    let mut added = 0;
    for asked in &plan.jev {
        if asked["how"] != "live" || asked["answers"].is_null() {
            continue;
        }
        let Some(key) = asked["key"].as_str() else {
            continue;
        };
        recorded.entries.insert(
            key.to_string(),
            RecordedAnswer {
                name: "jev_contract_span".to_string(),
                model: crate::credentials::JEV_MODEL.to_string(),
                answers: asked["answers"].clone(),
                input_tokens: asked["input_tokens"].as_u64(),
                output_tokens: asked["output_tokens"].as_u64(),
                milliseconds: asked["milliseconds"].as_u64(),
                source: format!("checks contract: {}", plan.task),
            },
        );
        added += 1;
    }
    added
}

fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    crate::record::write_atomic(path, format!("{text}\n").as_bytes())
}

/// Runs a contract command.
///
/// # Errors
///
/// A message for bad arguments or unreadable inputs.
#[allow(clippy::too_many_lines)]
pub async fn command(args: &[String]) -> Result<i32, String> {
    let Some((verb, rest)) = args.split_first() else {
        return Err(USAGE.to_string());
    };
    let mut positional = Vec::new();
    let mut flags: Vec<(String, String)> = Vec::new();
    let mut reuse_plan = false;
    let mut rest = rest.iter();
    while let Some(arg) = rest.next() {
        if arg == "--reuse-plan" {
            reuse_plan = true;
        } else if arg.starts_with("--") {
            let value = rest.next().ok_or(format!("{arg} needs a value"))?;
            flags.push((arg.clone(), value.clone()));
        } else {
            positional.push(arg.clone());
        }
    }
    let one = |name: &str| {
        flags
            .iter()
            .rev()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
    };
    let many = |name: &str| {
        flags
            .iter()
            .filter(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
            .collect::<Vec<_>>()
    };
    let known = [
        "--instruction",
        "--task",
        "--workdir",
        "--jev",
        "--recorded",
        "--out",
        "--plan",
        "--jobs",
        "--tasks",
        "--image",
        "--grades",
        "--reconstruction",
        "--kinds",
        "--trials",
        "--exclude-job",
        "--workers",
        "--command-sec",
        "--budget-sec",
    ];
    if let Some((unknown, _)) = flags.iter().find(|(k, _)| !known.contains(&k.as_str())) {
        return Err(format!("unknown option {unknown}\n{USAGE}"));
    }
    let jev = one("--jev").unwrap_or_else(|| "recorded".to_string());
    match verb.as_str() {
        "literal-plan" | "literal-run" => {
            if !positional.is_empty()
                || reuse_plan
                || flags.iter().any(|(name, _)| {
                    !["--instruction", "--task", "--workdir", "--out", "--plan"]
                        .contains(&name.as_str())
                })
            {
                return Err("literal artifact commands accept only their documented file options; they never ask Jev".into());
            }
            let value = if verb == "literal-plan" {
                let path = one("--instruction").ok_or("literal-plan needs --instruction")?;
                let instruction =
                    std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?;
                let workdir = one("--workdir").unwrap_or_else(|| "/app".to_string());
                let task = one("--task").unwrap_or_else(|| "task".to_string());
                let plan = super::literals::plan(&task, &instruction, &workdir);
                plan.verify()?;
                serde_json::to_value(plan).map_err(|e| e.to_string())?
            } else {
                let path = one("--plan").ok_or("literal-run needs --plan")?;
                let plan: super::literals::Plan = serde_json::from_str(
                    &std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?,
                )
                .map_err(|e| format!("{path} is not a literal artifact plan: {e}"))?;
                let host = Local {
                    workdir: PathBuf::from(&plan.workdir),
                };
                super::literals::run(&plan, &plan.workdir, &host).await?
            };
            match one("--out") {
                Some(out) => write_json(Path::new(&out), &value)?,
                None => println!(
                    "{}",
                    serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?
                ),
            }
            Ok(0)
        }
        "plan" => {
            let path = one("--instruction").ok_or("plan needs --instruction")?;
            let instruction = std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?;
            let workdir = one("--workdir").unwrap_or_else(|| "/app".to_string());
            let task = one("--task").unwrap_or_else(|| "task".to_string());
            let recorded_path =
                PathBuf::from(one("--recorded").unwrap_or_else(|| "jev-recorded.json".to_string()));
            let mut recorded = Recorded::load(&recorded_path)?;
            let (mode, replay) = jev_modes(&jev, &recorded)?;
            let host = Local {
                workdir: PathBuf::from(&workdir),
            };
            let plan = extract::plan(
                &task,
                &instruction,
                &workdir,
                &host,
                &mode,
                replay.as_ref(),
                &Recorder::default(),
            )
            .await;
            if keep_live(&plan, &mut recorded) > 0 {
                recorded.save(&recorded_path)?;
            }
            match one("--out") {
                Some(out) => write_json(Path::new(&out), &plan)?,
                None => println!(
                    "{}",
                    serde_json::to_string_pretty(&plan).map_err(|e| e.to_string())?
                ),
            }
            Ok(0)
        }
        "run" => {
            let path = one("--plan").ok_or("run needs --plan")?;
            let plan: Plan = serde_json::from_str(
                &std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?,
            )
            .map_err(|e| format!("{path} is not a plan: {e}"))?;
            let host = Local {
                workdir: PathBuf::from(&plan.workdir),
            };
            let results = run(&plan, &host).await;
            let value = report(&plan, &plan.workdir, &results);
            match one("--out") {
                Some(out) => write_json(Path::new(&out), &value)?,
                None => println!(
                    "{}",
                    serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?
                ),
            }
            Ok(0)
        }
        "offline" | "executed" => {
            if positional.is_empty() {
                return Err(format!("{verb} needs at least one TASK\n{USAGE}"));
            }
            let out = PathBuf::from(one("--out").ok_or(format!("{verb} needs --out"))?);
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .ok_or("HOME is not set")?
                .join(".openagents/terminal-bench");
            let kinds = match one("--kinds") {
                Some(list) => list
                    .split(',')
                    .map(crate::accept::offline::Kind::parse)
                    .collect::<Result<Vec<_>, _>>()?,
                None => Vec::new(),
            };
            let options = super::offline::Options {
                jobs: one("--jobs").map_or_else(|| home.join("jobs"), PathBuf::from),
                tasks_dir: one("--tasks").map_or_else(
                    || home.join("upstream/terminal-bench-v4.0.0/tasks"),
                    PathBuf::from,
                ),
                out: out.clone(),
                image: one("--image"),
                grades: many("--grades").into_iter().map(PathBuf::from).collect(),
                reconstructions: many("--reconstruction")
                    .into_iter()
                    .map(PathBuf::from)
                    .collect(),
                only: one("--trials")
                    .map(|t| t.split(',').map(str::to_string).collect())
                    .unwrap_or_default(),
                kinds,
                exclude_jobs: many("--exclude-job"),
                workers: one("--workers").and_then(|w| w.parse().ok()).unwrap_or(2),
                reuse_plan,
            };
            if verb == "executed" {
                let seconds = |name: &str, default: u64| -> Result<u64, String> {
                    one(name).map_or(Ok(default), |v| {
                        v.parse()
                            .ok()
                            .filter(|n| *n > 0)
                            .ok_or(format!("{name} is a whole number of seconds, not {v}"))
                    })
                };
                let wall =
                    Duration::from_secs(seconds("--command-sec", super::executed::COMMAND_SEC)?);
                let budget = Duration::from_secs(seconds("--budget-sec", 300)?);
                let mut failed = 0;
                for task in &positional {
                    if let Err(error) =
                        super::offline::executed_task(task, &options, wall, budget).await
                    {
                        failed += 1;
                        crate::say::line(&format!("executed ▸ {task}: {error}"));
                    }
                }
                return Ok(i32::from(failed > 0));
            }
            let recorded_path =
                one("--recorded").map_or_else(|| out.join("jev-recorded.json"), PathBuf::from);
            let mut recorded = Recorded::load(&recorded_path)?;
            let mut failed = 0;
            for task in &positional {
                let (mode, replay) = jev_modes(&jev, &recorded)?;
                match super::offline::task(task, &mode, replay.as_ref(), &options).await {
                    Ok(_) => {
                        let plan: Option<Plan> =
                            std::fs::read_to_string(out.join(task).join("plan.json"))
                                .ok()
                                .and_then(|t| serde_json::from_str(&t).ok());
                        if let Some(plan) = plan
                            && keep_live(&plan, &mut recorded) > 0
                        {
                            recorded.save(&recorded_path)?;
                        }
                    }
                    Err(error) => {
                        failed += 1;
                        crate::say::line(&format!("contract ▸ {task}: {error}"));
                    }
                }
            }
            Ok(i32::from(failed > 0))
        }
        _ => Err(USAGE.to_string()),
    }
}

/// The plan's items as one line each, for a terminal.
#[must_use]
pub fn lines(plan: &Plan) -> Vec<String> {
    plan.items
        .iter()
        .map(|i| {
            let what = i
                .command
                .as_deref()
                .or(i.path.as_deref())
                .unwrap_or_default();
            match &i.not_executable {
                Some(why) => format!("{} {} {what}: not executable, {why}", i.id, i.kind.word()),
                None => format!("{} {} {what}", i.id, i.kind.word()),
            }
        })
        .collect()
}
