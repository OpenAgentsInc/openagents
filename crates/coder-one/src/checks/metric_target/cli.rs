//! `coder-one checks metric-target …`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};

use super::{Harness, Protocol, Run, Runner, Side, Target};
use crate::component::jev::{JevMode, Recorded, RecordedAnswer};
use crate::record::Recorder;

/// The metric-target commands' usage.
pub const USAGE: &str = "usage: coder-one checks metric-target extract --instruction FILE
                                            [--workspace DIR] [--jev off|recorded|live]
                                            [--recorded FILE] [--questions FILE]
                                            [--host-limit-sec N]
       coder-one checks metric-target offline --labels FILE --out DIR
                                            [--jev recorded|live] [--recorded FILE]
                                            [--questions FILE] [--host-limit task|none]
                                            [--host-margin-sec N]
       coder-one checks metric-target measure --workspace DIR --target FILE
                                            --harness COMMAND [--sided]
                                            [--reference-dir DIR] [--warmup N]
                                            [--repeats N] [--run-sec N] [--budget-sec N]

extract finds the numbers in the instruction at --instruction, has Jev judge
each one (crates/coder-one/questions/metric-target.json), and prints the targets it states;
with --workspace it also asks which provided script measures them. offline
does the same for every task in a labels file (its instruction_path and
instruction_sha256 must still match), writes <out>/extracted.json and
<out>/summary.json, which compares the answers with the hand labels, and
keeps every live answer in --recorded (default <out>/jev-recorded.json).
It stops asking once the recorded input tokens reach $0.10.

--questions replaces the embedded question set with an earlier version,
such as the one a measurement froze. --host-limit-sec is the host's own
time limit for the task, in seconds: a sentence that only restates it isn't
a goal. offline's --host-limit task reads each task's agent timeout from the
task.toml beside its instruction and takes --host-margin-sec from it
(default 120, the margin the Terminal-Bench adapter keeps before it sets
the episode deadline); none, the default, gives no limit. measure runs
--harness (a shell command, from --workspace) under the measurement
protocol for the target in --target (a JSON target, as extract prints it)
and prints the measurement; --sided passes candidate or reference as the
last argument.";

/// The Jev budget of an offline run, in dollars.
pub const OFFLINE_USD: f64 = 0.10;

pub(super) fn modes(word: &str, recorded: &Recorded) -> Result<(JevMode, Option<JevMode>), String> {
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

/// Keeps a live answer in `recorded`.
pub(super) fn keep_live(call: &Value, source: &str, recorded: &mut Recorded) -> bool {
    if call["how"] != "live" || call["answers"].is_null() {
        return false;
    }
    let Some(key) = call["key"].as_str() else {
        return false;
    };
    recorded.entries.insert(
        key.to_string(),
        RecordedAnswer {
            name: "jev_metric_target".to_string(),
            model: crate::credentials::JEV_MODEL.to_string(),
            answers: call["answers"].clone(),
            input_tokens: call["input_tokens"].as_u64(),
            output_tokens: call["output_tokens"].as_u64(),
            milliseconds: call["milliseconds"].as_u64(),
            source: format!("checks metric-target: {source}"),
        },
    );
    true
}

/// What an offline extraction knows besides the instruction.
#[derive(Clone, Copy, Default)]
pub(super) struct Known<'a> {
    /// The host's own time limit for the task.
    pub host_limit: Option<Duration>,
    /// The question set's wording; `None` for the embedded one.
    pub questions: Option<&'a super::QuestionSet>,
}

/// Extracts with the recorded answer when `replay` has the request, and
/// with `jev` otherwise.
pub(super) async fn extract_with(
    jev: &JevMode,
    replay: Option<&JevMode>,
    instruction: &str,
    workdir: Option<&Path>,
    id: &str,
    known: Known<'_>,
) -> super::Extracted {
    let context = super::Context {
        component: "checks.metric_target",
        id: id.to_string(),
        deadline: None,
        host_limit: known.host_limit,
        questions: known.questions,
    };
    let key = super::request_key(&super::plan(&context, instruction, workdir).request);
    let mode = match replay {
        Some(JevMode::Recorded(recorded)) if recorded.entries.contains_key(&key) => {
            replay.unwrap_or(jev)
        }
        _ => jev,
    };
    super::extract(mode, &Recorder::default(), &context, instruction, workdir).await
}

/// The question set at `path`, when one is given.
fn questions_from(path: Option<String>) -> Result<Option<super::QuestionSet>, String> {
    path.map(|path| {
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("cannot read {path}: {e}"))?;
        super::QuestionSet::parse(&text).map_err(|e| format!("{path}: {e}"))
    })
    .transpose()
}

/// Runs a harness command from a workspace, with no boundary: for
/// measuring retained workspaces offline.
struct LocalRunner {
    workdir: PathBuf,
    harness: Harness,
    reference_dir: Option<PathBuf>,
}

impl Runner for LocalRunner {
    async fn run(&self, side: Side, wall: Duration) -> Run {
        let mut args = self.harness.command.clone();
        if self.harness.sided {
            args.push(side.word().to_string());
        }
        let mut command = std::process::Command::new("/usr/bin/env");
        command
            .args(&args)
            .current_dir(&self.workdir)
            .env("METRIC_SIDE", side.word());
        if let Some(dir) = &self.reference_dir {
            command.env("METRIC_REFERENCE_DIR", dir);
        }
        microluna::tools::withhold_credentials(&mut command);
        let ended = supervise::Job::from_command(command)
            .bounded(supervise::Limits::within(wall).keeping(64 * 1024))
            .run()
            .await;
        let stdout = ended.stdout.marked();
        let stderr = ended.stderr.marked();
        Run {
            side,
            ok: ended.ending.success() && !ended.stdout.truncated,
            printed: super::parse_metric(&stdout),
            seconds: ended.elapsed.as_secs_f64(),
            tail: crate::judge::clip(
                &format!("{}\n{}", stdout.trim_end(), stderr.trim_end()),
                1_500,
            ),
        }
    }
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    crate::record::write_atomic(path, format!("{text}\n").as_bytes())
}

/// Runs a metric-target command.
///
/// # Errors
///
/// A message for bad arguments or unreadable inputs.
#[allow(clippy::too_many_lines)]
pub async fn command(args: &[String]) -> Result<i32, String> {
    let Some((verb, rest)) = args.split_first() else {
        return Err(USAGE.to_string());
    };
    let mut flags: Vec<(String, String)> = Vec::new();
    let mut sided = false;
    let mut rest = rest.iter();
    while let Some(arg) = rest.next() {
        if arg == "--sided" {
            sided = true;
        } else if arg.starts_with("--") {
            let value = rest.next().ok_or(format!("{arg} needs a value"))?;
            flags.push((arg.clone(), value.clone()));
        } else {
            return Err(format!("unexpected argument {arg}\n{USAGE}"));
        }
    }
    let known = [
        "--instruction",
        "--workspace",
        "--jev",
        "--recorded",
        "--labels",
        "--out",
        "--target",
        "--harness",
        "--reference-dir",
        "--warmup",
        "--repeats",
        "--run-sec",
        "--budget-sec",
        "--questions",
        "--host-limit",
        "--host-limit-sec",
        "--host-margin-sec",
    ];
    if let Some((unknown, _)) = flags.iter().find(|(k, _)| !known.contains(&k.as_str())) {
        return Err(format!("unknown option {unknown}\n{USAGE}"));
    }
    let one = |name: &str| {
        flags
            .iter()
            .rev()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
    };
    let number = |name: &str, default: u64| -> Result<u64, String> {
        one(name).map_or(Ok(default), |v| {
            v.parse().map_err(|_| format!("{name} takes a number"))
        })
    };
    match verb.as_str() {
        "extract" => {
            let path = one("--instruction").ok_or("extract needs --instruction")?;
            let instruction =
                std::fs::read_to_string(&path).map_err(|e| format!("cannot read {path}: {e}"))?;
            let recorded_path =
                PathBuf::from(one("--recorded").unwrap_or_else(|| "jev-recorded.json".to_string()));
            let mut recorded = Recorded::load(&recorded_path)?;
            let jev = one("--jev").unwrap_or_else(|| "recorded".to_string());
            let (mode, replay) = modes(&jev, &recorded)?;
            let workspace = one("--workspace").map(PathBuf::from);
            let questions = questions_from(one("--questions"))?;
            let host_limit = one("--host-limit-sec")
                .map(|v| v.parse::<u64>().map(Duration::from_secs))
                .transpose()
                .map_err(|_| "--host-limit-sec takes a number of seconds".to_string())?;
            let extracted = extract_with(
                &mode,
                replay.as_ref(),
                &instruction,
                workspace.as_deref(),
                "jev-metric-target",
                Known {
                    host_limit,
                    questions: questions.as_ref(),
                },
            )
            .await;
            if keep_live(&extracted.call, &path, &mut recorded) {
                recorded.save(&recorded_path)?;
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&extracted).map_err(|e| e.to_string())?
            );
            Ok(0)
        }
        "offline" => {
            let labels = PathBuf::from(one("--labels").ok_or("offline needs --labels")?);
            let out = PathBuf::from(one("--out").ok_or("offline needs --out")?);
            let recorded_path =
                one("--recorded").map_or_else(|| out.join("jev-recorded.json"), PathBuf::from);
            let jev = one("--jev").unwrap_or_else(|| "recorded".to_string());
            let questions = questions_from(one("--questions"))?;
            let host_margin = match one("--host-limit").as_deref() {
                None | Some("none") => None,
                Some("task") => Some(Duration::from_secs(number("--host-margin-sec", 120)?)),
                Some(other) => return Err(format!("--host-limit is task or none, not {other}")),
            };
            let summary = super::offline::run(
                &labels,
                &out,
                &recorded_path,
                &jev,
                &super::offline::Options {
                    questions: questions.as_ref(),
                    host_margin,
                },
            )
            .await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&summary["totals"]).map_err(|e| e.to_string())?
            );
            Ok(0)
        }
        "measure" => {
            let workspace = PathBuf::from(one("--workspace").ok_or("measure needs --workspace")?);
            let target_path = one("--target").ok_or("measure needs --target")?;
            let target: Target = serde_json::from_str(
                &std::fs::read_to_string(&target_path)
                    .map_err(|e| format!("cannot read {target_path}: {e}"))?,
            )
            .map_err(|e| format!("{target_path} is not a target: {e}"))?;
            let command = one("--harness").ok_or("measure needs --harness")?;
            let protocol = Protocol {
                warmup: u32::try_from(number("--warmup", 1)?).unwrap_or(1),
                repeats: u32::try_from(number("--repeats", 5)?).unwrap_or(5),
                run_sec: number("--run-sec", 120)?,
                budget_sec: number("--budget-sec", 600)?,
            };
            let runner = LocalRunner {
                workdir: workspace,
                harness: Harness {
                    source: super::Source::Provided,
                    command: vec!["sh".to_string(), "-c".to_string(), command.clone()],
                    sided,
                    frozen: None,
                    digest: None,
                },
                reference_dir: one("--reference-dir").map(PathBuf::from),
            };
            // `sh -c COMMAND side` makes the side `$0`; pass it as `$1`.
            let runner = if sided {
                LocalRunner {
                    harness: Harness {
                        command: vec![
                            "sh".to_string(),
                            "-c".to_string(),
                            command.clone(),
                            "harness".to_string(),
                        ],
                        ..runner.harness
                    },
                    ..runner
                }
            } else {
                runner
            };
            let measured = super::measure(
                &runner,
                &target,
                &protocol,
                Duration::from_secs(protocol.budget_sec),
            )
            .await;
            let value = json!({
                "target": target,
                "harness": command,
                "protocol": protocol,
                "measurement": measured,
                "verdict": measured.verdict(&target),
                "confident": measured.confident(&target),
                "line": measured.line(&target),
            });
            match one("--out") {
                Some(path) => write_json(Path::new(&path), &value)?,
                None => println!(
                    "{}",
                    serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?
                ),
            }
            Ok(0)
        }
        _ => Err(USAGE.to_string()),
    }
}
