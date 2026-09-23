//! The fixture extractor: a retained attempt becomes one fixture directory,
//! with one input per component and every Jev answer recorded.
//!
//! It reads the ATIF trajectory an episode retained. Every decision call
//! carries its full request, so the Jev states, the questions, and the
//! answers come back exactly; the delegate call carries the briefing text,
//! which [`pack::parse`] reads back into the packer's inputs. What the
//! trajectory doesn't hold, such as the bytes of a file the briefing
//! omitted or a probe output past what Jev read, stays out, and the fixture
//! says so in its source notes.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::jev::{Recorded, RecordedAnswer, key};
use super::{FIXTURE_SCHEMA, Fixture, RECORDED_FILE, evidence::Task, pack};
use crate::delegate::Briefing;

/// The schema of a fixture directory's `source.json`.
pub const SOURCE_SCHEMA: &str = "openagents.coder-one.fixture-source.v1";

/// What one extraction wrote.
#[derive(Clone, Debug, PartialEq)]
pub struct Extracted {
    pub dir: PathBuf,
    /// The components with an input, in episode order.
    pub components: Vec<String>,
    /// How many Jev answers were recorded.
    pub recorded: usize,
    /// Whether the packer's input rebuilds the retained briefing exactly,
    /// when a briefing was retained.
    pub reproduces: Option<bool>,
}

/// One call from a retained trajectory: its name, arguments, output, and
/// the step's extras.
struct Retained {
    id: String,
    name: String,
    arguments: Value,
    extra: Value,
    output: Option<String>,
    completed: bool,
    step: Value,
}

fn calls(trajectory: &Value) -> Vec<Retained> {
    let mut out = Vec::new();
    for step in trajectory
        .get("steps")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let results: Vec<&Value> = step
            .pointer("/observation/results")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .collect();
        for call in step
            .get("tool_calls")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let id = call
                .get("tool_call_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let result = results
                .iter()
                .find(|result| result.get("source_call_id").and_then(Value::as_str) == Some(&id));
            out.push(Retained {
                name: call
                    .get("function_name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                arguments: call.get("arguments").cloned().unwrap_or(Value::Null),
                extra: call.get("extra").cloned().unwrap_or(Value::Null),
                output: result
                    .and_then(|result| result.get("content"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
                completed: result
                    .and_then(|result| result.pointer("/extra/status"))
                    .and_then(Value::as_str)
                    == Some("completed"),
                step: step.clone(),
                id,
            });
        }
    }
    out
}

fn task(state: &Value) -> Task {
    Task {
        title: state
            .pointer("/issue/title")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        body: state
            .pointer("/issue/body")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    }
}

fn answers(call: &Retained) -> Option<Value> {
    call.completed
        .then(|| serde_json::from_str::<Value>(call.output.as_deref()?).ok())
        .flatten()
}

fn noul(answers: Option<&Value>, id: &str) -> Option<f64> {
    answers?.get(id)?.get("noul")?.as_f64()
}

fn strings(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

/// Extracts the attempt whose ATIF trajectory is at `trajectory` into
/// `out`. `source` names the attempt in the fixture's provenance.
///
/// # Errors
///
/// Returns a message when the trajectory doesn't read or a fixture can't
/// be written.
pub fn extract(trajectory: &Path, source: &Value, out: &Path) -> Result<Extracted, String> {
    let bytes = std::fs::read(trajectory)
        .map_err(|error| format!("cannot read {}: {error}", trajectory.display()))?;
    let document: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("{} is not JSON: {error}", trajectory.display()))?;
    if document.get("schema_version").and_then(Value::as_str) != Some("ATIF-v1.7") {
        return Err(format!(
            "{} is not an ATIF-v1.7 trajectory",
            trajectory.display()
        ));
    }
    let calls = calls(&document);
    std::fs::create_dir_all(out)
        .map_err(|error| format!("cannot create {}: {error}", out.display()))?;
    let trace = trajectory.display().to_string();
    let provenance = |call: &Retained| json!({ "trace": source, "call": call.id });
    let mut recorded = Recorded::empty();
    let mut notes: Vec<String> = Vec::new();
    for call in calls.iter().filter(|call| call.name.starts_with("jev_")) {
        let Some(answers) = answers(call) else {
            notes.push(format!("{} has no answers; nothing recorded", call.id));
            continue;
        };
        let state = call.arguments.get("state").cloned().unwrap_or(Value::Null);
        let questions = call
            .arguments
            .get("questions")
            .cloned()
            .unwrap_or(Value::Null);
        let usage = call.step.pointer("/extra/jev_usage");
        recorded.entries.insert(
            key(&state, &questions),
            RecordedAnswer {
                name: call.name.clone(),
                model: call
                    .extra
                    .get("model")
                    .and_then(Value::as_str)
                    .unwrap_or(crate::credentials::JEV_MODEL)
                    .to_string(),
                answers,
                input_tokens: usage.and_then(|u| u.get("input_tokens")?.as_u64()),
                output_tokens: usage.and_then(|u| u.get("output_tokens")?.as_u64()),
                milliseconds: call
                    .step
                    .pointer("/extra/duration_ms")
                    .and_then(Value::as_u64),
                source: format!(
                    "{}#{}",
                    source
                        .get("trial")
                        .and_then(Value::as_str)
                        .unwrap_or(&trace),
                    call.id
                ),
            },
        );
    }

    // The briefing, when the attempt delegated: the packer's input and what
    // it delivered, which the other components' retained outputs read.
    let delegate = calls.iter().find(|call| call.name == "delegate");
    let briefing_record = delegate.and_then(|call| call.extra.get("briefing").cloned());
    let delivered: Vec<String> = briefing_record
        .as_ref()
        .map(|record| {
            let mut items = strings(record.get("included"));
            items.extend(
                strings(record.get("omitted"))
                    .into_iter()
                    .filter_map(|item| Some(item.rsplit_once(" (")?.0.to_string())),
            );
            items
        })
        .unwrap_or_default();
    let delivered_files: Vec<&str> = delivered
        .iter()
        .filter_map(|item| item.strip_prefix("file "))
        .collect();

    let mut components = Vec::new();
    let mut write = |component: &str, call: &Retained, input: Value, retained: Value| {
        components.push(component.to_string());
        Fixture {
            schema: FIXTURE_SCHEMA.to_string(),
            component: component.to_string(),
            source: provenance(call),
            input,
            retained,
        }
        .save(out)
    };

    if let Some(call) = calls.iter().find(|call| call.name == "jev_setup") {
        let state = &call.arguments["state"];
        let commands = strings(state.get("setup"));
        let answers = answers(call);
        let approved: Vec<&String> = commands
            .iter()
            .enumerate()
            .filter(|(i, _)| {
                noul(answers.as_ref(), &format!("setup_{i}"))
                    .is_some_and(|p| p >= super::evidence::YES)
            })
            .map(|(_, command)| command)
            .collect();
        write(
            "evidence.setup",
            call,
            json!({ "task": task(state), "commands": commands }),
            json!({ "approved": approved }),
        )?;
    }

    if let Some(call) = calls.iter().find(|call| call.name == "jev_probe") {
        let state = &call.arguments["state"];
        let probes = state.get("probes").cloned().unwrap_or(json!([]));
        let commands: Vec<String> = probes
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|probe| probe.get("command")?.as_str().map(str::to_string))
            .collect();
        let kept: Vec<&str> = delivered_files
            .iter()
            .filter_map(|path| path.strip_prefix("$ "))
            .filter(|command| commands.iter().any(|c| c == command))
            .collect();
        notes.push("Probe outputs are what Jev read: each is clipped to 3,000 characters, so the keep budget counts at most that much per probe.".to_string());
        let battery: Vec<(String, String)> = probes
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|probe| {
                Some((
                    probe.get("command")?.as_str()?.to_string(),
                    probe.get("output")?.as_str()?.to_string(),
                ))
            })
            .collect();
        let instruction = format!(
            "{}\n{}",
            state
                .pointer("/issue/title")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            state
                .pointer("/issue/body")
                .and_then(Value::as_str)
                .unwrap_or_default()
        );
        notes.push("The planner's facts are inferred from the retained battery: a named path it neither listed nor read reads as missing, and a README probe that printed reads as one README.".to_string());
        write(
            "evidence.probes.planner",
            call,
            json!({
                "facts": crate::probes::facts_from_battery(&instruction, &battery),
                "params": crate::probes::PlanParams::default(),
            }),
            json!({ "commands": commands }),
        )?;
        write(
            "evidence.probes.selector",
            call,
            json!({ "task": task(state), "probes": probes }),
            if briefing_record.is_some() {
                json!({ "kept": kept })
            } else {
                Value::Null
            },
        )?;
    }

    let surveys: Vec<&Retained> = calls
        .iter()
        .filter(|call| call.name == "jev_survey")
        .collect();
    if let Some(first) = surveys.first() {
        let candidates: Vec<Value> = surveys
            .iter()
            .flat_map(|call| {
                call.arguments
                    .pointer("/state/files")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default()
            })
            .collect();
        let read: Vec<&str> = delivered_files
            .iter()
            .copied()
            .filter(|path| !path.starts_with("$ "))
            .collect();
        write(
            "evidence.select",
            first,
            json!({ "task": task(&first.arguments["state"]), "candidates": candidates }),
            if briefing_record.is_some() {
                json!({ "read": read })
            } else {
                Value::Null
            },
        )?;
    }

    let mut reproduces = None;
    if let (Some(call), Some(record)) = (delegate, briefing_record.as_ref()) {
        let text = call
            .arguments
            .get("prompt")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let included = strings(record.get("included"));
        let omitted = strings(record.get("omitted"));
        let cap = record
            .get("cap")
            .and_then(Value::as_u64)
            .and_then(|cap| usize::try_from(cap).ok())
            .unwrap_or(crate::delegate::BRIEFING_CAP);
        match pack::parse(text, &included, &omitted) {
            Ok(inputs) => {
                let rebuilt = Briefing::build(&inputs, cap);
                let same =
                    Some(rebuilt.sha256().as_str()) == record.get("sha256").and_then(Value::as_str);
                reproduces = Some(same);
                if !omitted.is_empty() {
                    notes.push(format!(
                        "The briefing omitted {} items; each keeps its name and size, and its content is placeholder text.",
                        omitted.len()
                    ));
                }
                write(
                    "evidence.pack",
                    call,
                    json!({ "inputs": inputs, "cap": cap }),
                    record.clone(),
                )?;
            }
            Err(error) => notes.push(format!("the briefing doesn't read back: {error}")),
        }
    }

    if let Some(call) = calls.iter().find(|call| call.name == "jev_close") {
        let state = &call.arguments["state"];
        write(
            "verify.close",
            call,
            json!({
                "task": task(state),
                "criteria": strings(state.get("criteria")),
                "report": state.pointer("/delegate/report").and_then(Value::as_str).unwrap_or_default(),
                "changes": state.pointer("/delegate/changes").and_then(Value::as_str).unwrap_or_default(),
            }),
            json!({ "done": noul(answers(call).as_ref(), "done") }),
        )?;
    }

    recorded.save(&out.join(RECORDED_FILE))?;
    let text = serde_json::to_string_pretty(&json!({
        "schema": SOURCE_SCHEMA,
        "source": source,
        "trajectory_sha256": sha256_hex(&bytes),
        "components": components,
        "recorded_answers": recorded.entries.len(),
        "briefing_reproduces": reproduces,
        "notes": notes,
    }))
    .map_err(|error| error.to_string())?;
    crate::record::write_atomic(&out.join("source.json"), format!("{text}\n").as_bytes())?;
    Ok(Extracted {
        dir: out.to_path_buf(),
        components,
        recorded: recorded.entries.len(),
        reproduces,
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Extracts the attempts in one retained trace directory, each trial's
/// `<trial>.json` trajectory beside its `<trial>.episode` directory, into
/// `out`.
///
/// # Errors
///
/// Returns a message when the directory doesn't list or an extraction
/// fails.
pub fn extract_trace(dir: &Path, out: &Path) -> Result<Vec<Extracted>, String> {
    let name = dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut trials: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|error| format!("cannot list {}: {error}", dir.display()))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    trials.sort();
    let mut extracted = Vec::new();
    for trajectory in trials {
        let trial = trajectory
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        let reward = std::fs::read(dir.join(format!("{trial}.episode/harbor-result.json")))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .and_then(|result| result.pointer("/verifier_result/rewards/reward").cloned());
        let source = json!({
            "trace": name,
            "trial": trial,
            "task": trial.split("__").next().unwrap_or_default(),
            "reward": reward,
        });
        extracted.push(extract(&trajectory, &source, out)?);
    }
    Ok(extracted)
}

/// Extracts every retained attempt under `traces` whose directory name
/// contains `arm` into `out/<directory>`. Each trace directory holds one
/// attempt's `<trial>.json` trajectory and its `<trial>.episode` directory.
///
/// # Errors
///
/// Returns a message when `traces` doesn't list, or an extraction fails.
pub fn extract_tree(traces: &Path, arm: &str, out: &Path) -> Result<Vec<Extracted>, String> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(traces)
        .map_err(|error| format!("cannot list {}: {error}", traces.display()))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_dir()
                && path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().contains(arm))
        })
        .collect();
    dirs.sort();
    let mut extracted = Vec::new();
    for dir in dirs {
        let name = dir
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        extracted.extend(extract_trace(&dir, &out.join(&name))?);
    }
    Ok(extracted)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn traces() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench/traces")
    }

    #[tokio::test]
    async fn every_retained_v3_luna_attempt_extracts_and_replays_in_under_a_minute() {
        let started = std::time::Instant::now();
        let out = std::env::temp_dir().join(format!("coder-one-extract-{}", atif::now_ms()));
        let extracted = extract_tree(&traces(), "coder-one-jevprobe3-luna", &out).unwrap();
        assert_eq!(extracted.len(), 24);
        for one in &extracted {
            assert_eq!(one.reproduces, Some(true), "{}", one.dir.display());
            for component in [
                "evidence.probes.planner",
                "evidence.probes.selector",
                "evidence.pack",
                "verify.close",
            ] {
                assert!(
                    one.components.iter().any(|c| c == component),
                    "{} has no {component}",
                    one.dir.display()
                );
            }
            assert!(one.recorded >= 2);
        }
        assert!(
            extracted
                .iter()
                .any(|one| one.components.iter().any(|c| c == "evidence.setup"))
        );
        // The pack and the keep question rerun on all 24 with recorded Jev
        // and reproduce the retained runs.
        for id in [
            "evidence.probes.planner",
            "evidence.probes.selector",
            "evidence.pack",
        ] {
            let component = super::super::find(id).unwrap();
            let dirs = super::super::fixtures_for(&out, id);
            assert_eq!(dirs.len(), 24);
            let suite = super::super::suite(
                component.as_ref(),
                &dirs,
                &super::super::JevChoice::Recorded,
                &crate::record::Recorder::default(),
                false,
            )
            .await
            .unwrap();
            let flag = if id == "evidence.pack" {
                "reproduces_retained"
            } else {
                "matches_retained"
            };
            assert_eq!(suite.summary()["metrics"][flag]["true"], json!(24), "{id}");
            assert!(suite.summary()["jev"].get("miss").is_none());
        }
        assert!(started.elapsed() < std::time::Duration::from_secs(60));
        let _ = std::fs::remove_dir_all(out);
    }

    #[test]
    fn extraction_round_trips_to_the_checked_in_fixture() {
        let out = std::env::temp_dir().join(format!("coder-one-roundtrip-{}", atif::now_ms()));
        let name = "extended--coder-one-jevprobe3-luna--log-summary-date-ranges";
        let extracted = extract_trace(&traces().join(name), &out.join(name)).unwrap();
        assert_eq!(extracted.len(), 1);
        let checked_in = super::super::default_fixtures().join(name);
        for file in [
            "evidence.probes.planner.json",
            "evidence.probes.selector.json",
            "evidence.select.json",
            "evidence.pack.json",
            "verify.close.json",
            "source.json",
        ] {
            let fresh = std::fs::read_to_string(out.join(name).join(file)).unwrap();
            let kept = std::fs::read_to_string(checked_in.join(file)).unwrap();
            assert_eq!(fresh, kept, "{file} differs from the checked-in fixture");
        }
        // Components with no retained call, such as `exec.system`, add
        // live answers of their own; every extracted answer is still kept.
        let fresh = Recorded::load(&out.join(name).join(RECORDED_FILE)).unwrap();
        let kept = Recorded::load(&checked_in.join(RECORDED_FILE)).unwrap();
        for (key, answer) in &fresh.entries {
            assert_eq!(kept.entries.get(key), Some(answer), "{key} differs");
        }
        let _ = std::fs::remove_dir_all(out);
    }
}
