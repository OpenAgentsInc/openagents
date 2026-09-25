//! Audit observed failures in the executor stream selected for the submission.
//!
//! This reader never runs a command or reads the benchmark verifier. Earlier
//! errors and incomplete retained observations cannot establish a final defect.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use jev::{Noul, NoulCriteria, Questions};
use serde_json::{Value, json};

use crate::component::jev::{Ask, JevMode, ask};
use crate::record::Recorder;

pub const SCHEMA: &str = "openagents.coder-one.execution-audit.v1";

#[must_use]
pub fn questions() -> Questions {
    Questions::new()
        .with("observed", Noul::with_criteria(
            "Do the retained tool results show an actual implementation failure or incomplete required deliverable? Use observed outputs, not a hypothetical code concern or the writer's confidence. A failed search, optional diagnostic, missing verification tool, timeout alone, or unexecuted test does not establish failure. A self-written assertion counts only if its expected behavior follows from the public task.",
            NoulCriteria::new().when_true("An observed result demonstrates wrong required behavior or an unfinished required deliverable.").when_false("Only a risk, an evidence gap, an optional diagnostic failure, or an unsupported test expectation is present.")))
        .with("unresolved", Noul::with_criteria(
            "Does an observed implementation failure remain unresolved in the submitted candidate represented by this selected session and final report? Follow the order of observations and later fixes or successful checks. Do not count an earlier error followed by a fix, or infer persistence when the observations are too incomplete to establish the final state.",
            NoulCriteria::new().when_true("A concrete observed failure is left unresolved in the final submitted result.").when_false("The failure was repaired, concerns a different candidate, or the final state is not established.")))
        .with("required", Noul::with_criteria(
            "Is the behavior that actually failed mandatory under the supplied public task? Match the observed failure to an explicit functional requirement or deliverable. Reject stricter invented requirements, optional checks, an unstated input domain, missing evidence, and a test's expected value that the task does not support.",
            NoulCriteria::new().when_true("The observation violates a mandatory public task requirement.").when_false("No mandatory public task requirement is shown to fail.")))
}

#[must_use]
pub fn score(answers: &Value) -> Option<f64> {
    let mut value: f64 = 1.0;
    for key in ["observed", "unresolved", "required"] {
        let p = answers[key]["noul"].as_f64()?;
        if !p.is_finite() || !(0.0..=1.0).contains(&p) {
            return None;
        }
        value = value.min(p);
    }
    Some(value)
}

fn clip(value: &Value, chars: usize) -> String {
    let text = value
        .as_str()
        .map_or_else(|| value.to_string(), str::to_string);
    super::verdict::head_tail(&text, chars)
}

fn relevant(name: &str) -> bool {
    matches!(
        name,
        "Bash" | "Write" | "Edit" | "MultiEdit" | "run_command" | "apply_patch" | "write_file"
    )
}

/// Normalize only retained tool observations, preserving their stream order.
#[must_use]
pub fn events(text: &str) -> Vec<Value> {
    let mut calls = BTreeMap::new();
    let mut events = Vec::new();
    for (line, text) in text.lines().enumerate() {
        let Ok(value) = serde_json::from_str::<Value>(text) else {
            continue;
        };
        let event = |name: &str, args: &Value, output: &Value, error: &Value| {
            json!({
            "line":line+1,"tool":name,"arguments":clip(args,800),"output":clip(output,1200),"error":error})
        };
        if value["record"] == "step" {
            let call = &value["step"]["call"];
            if let Some(name) = call["name"].as_str().filter(|n| relevant(n)) {
                events.push(event(
                    name,
                    &call["arguments"],
                    &call["output"],
                    &call["error"],
                ));
            }
        }
        if value["type"] == "item.completed" {
            let item = &value["item"];
            match item["type"].as_str() {
                Some("command_execution") => events.push(event(
                    "command_execution",
                    &item["command"],
                    &item["aggregated_output"],
                    &json!({"exit_code":item["exit_code"]}),
                )),
                Some("file_change") => events.push(event(
                    "file_change",
                    &item["changes"],
                    &item["status"],
                    &Value::Null,
                )),
                _ => (),
            }
        }
        for block in value["message"]["content"].as_array().into_iter().flatten() {
            if block["type"] == "tool_use"
                && let (Some(id), Some(name)) = (block["id"].as_str(), block["name"].as_str())
                && relevant(name)
            {
                calls.insert(id.to_string(), (name.to_string(), block["input"].clone()));
            }
            if block["type"] == "tool_result"
                && let Some(id) = block["tool_use_id"].as_str()
                && let Some((name, args)) = calls.remove(id)
            {
                events.push(event(&name, &args, &block["content"], &block["is_error"]));
            }
        }
    }
    events
}

fn read(path: &Path) -> Result<Value, String> {
    serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

fn build(row: &Value, jobs: &Path, records: &Path) -> Result<Value, String> {
    let job = row["job"].as_str().ok_or("Missing job")?;
    let trial = row["trial"].as_str().ok_or("Missing trial")?;
    if [job, trial].iter().any(|s| {
        !matches!(
            Path::new(s).components().collect::<Vec<_>>().as_slice(),
            [std::path::Component::Normal(_)]
        )
    }) {
        return Err("Job and trial must be single directory names".into());
    }
    let episode = jobs.join(job).join(trial).join("agent/episode");
    let composition = read(&episode.join("artifacts/composition.json"))?;
    let selected = super::truth::final_report(&episode, &composition);
    let source = selected.source.ok_or_else(|| {
        selected
            .unavailable
            .unwrap_or_else(|| "No selected stream".into())
    })?;
    let relative = if let Some((base, number)) = source.split_once("#session-") {
        format!("{}-{number}.atif.jsonl", base.trim_end_matches(".json"))
    } else {
        source.clone()
    };
    let path = episode
        .join(&relative)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if !path.starts_with(episode.canonicalize().map_err(|e| e.to_string())?) {
        return Err("Selected stream leaves its episode".into());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let observations = events(&text);
    if observations.is_empty() {
        return Err("No selected tool observations were retained".into());
    }
    let audit = read(
        &records
            .join(format!(
                "report-audit-{}",
                row["split"].as_str().ok_or("Missing split")?
            ))
            .join(format!("{trial}.json")),
    )?;
    let task = audit["public_task"]
        .as_str()
        .ok_or("No public task is retained")?;
    let kept: Vec<_> = observations.iter().rev().take(12).rev().cloned().collect();
    let state = json!({"task":task,"selected_report":super::verdict::head_tail(selected.report.as_deref().unwrap_or_default(),8000),
        "observations":kept,"coverage":"Only the last 12 observed tools in the selected executor session are supplied. Arguments and output may be excerpted with a middle-omitted marker. Missing observations prove nothing. Earlier failures may have been fixed. A report of a check is not independent execution evidence."});
    if serde_json::to_vec(&state).map_err(|e| e.to_string())?.len() > 64_000 {
        return Err("Execution-audit state exceeds 64 KB".into());
    }
    Ok(
        json!({"state":state,"source":source,"source_digest":atif::digest(&json!(text)),"total_events":observations.len(),"retained_events":kept.len()}),
    )
}

/// Ask only about the selected session, retaining all source and decision identity.
///
/// # Errors
/// Returns invalid arguments, unreadable manifests, credentials, or storage errors.
pub async fn command(args: &[String]) -> Result<i32, String> {
    let (mut manifest, mut jobs, mut records, mut out) = (None, None, None, None);
    let mut partition = "calibration".to_string();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let value = args.next().ok_or("execution-audit needs paired options")?;
        match arg.as_str() {
            "--manifest" => manifest = Some(PathBuf::from(value)),
            "--jobs" => jobs = Some(PathBuf::from(value)),
            "--records" => records = Some(PathBuf::from(value)),
            "--out" => out = Some(PathBuf::from(value)),
            "--partition" => partition.clone_from(value),
            _ => return Err(format!("Unknown execution-audit option {arg}")),
        }
    }
    let manifest = read(&manifest.ok_or("execution-audit needs --manifest")?)?;
    let jobs = jobs.ok_or("execution-audit needs --jobs")?;
    let records = records.ok_or("execution-audit needs --records")?;
    let out = out.ok_or("execution-audit needs --out")?;
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let home = crate::credentials::openagents_dir().ok_or("HOME is not set")?;
    let key = crate::credentials::jev_key(|name| std::env::var(name).ok(), &home)?;
    let mode = JevMode::Live(crate::credentials::jev_client(&key.secret)?);
    let rows: Vec<_> = manifest
        .as_array()
        .ok_or("Manifest must be an array")?
        .iter()
        .filter(|r| r["split"] == partition)
        .collect();
    let results: Vec<Result<(), String>> =
        futures_util::stream::iter(rows.into_iter().map(|row| {
            let (jobs, records, out, mode) = (&jobs, &records, &out, &mode);
            async move {
                let id = row["trial"].as_str().ok_or("Missing trial")?;
                let path = out.join(format!("{id}.json"));
                if path.exists() {
                    return Ok(());
                }
                let recorder = Recorder::default();
                let mut value = match build(row, jobs, records) {
                    Ok(mut packet) => {
                        let state = packet["state"].clone();
                        let asked = ask(
                            mode,
                            &recorder,
                            Ask {
                                component: "verify.execution-audit",
                                name: "jev_unresolved_execution_failure",
                                id: id.into(),
                                state: state.clone(),
                                questions: questions(),
                                parent: None,
                                deadline: None,
                            },
                        )
                        .await;
                        packet["questions"] = json!(questions());
                        packet["digest"] =
                            json!(crate::component::jev::key(&state, &json!(questions())));
                        packet["score"] = json!(asked.answers.as_ref().and_then(score));
                        packet["answers"] = json!(asked.answers);
                        packet["error"] = json!(asked.error);
                        packet["input_tokens"] = json!(asked.input_tokens);
                        packet["milliseconds"] = json!(asked.milliseconds);
                        packet["steps"] = json!(recorder.steps());
                        packet
                    }
                    Err(error) => json!({"score":null,"error":error}),
                };
                value["schema"] = json!(SCHEMA);
                value["job"] = row["job"].clone();
                value["trial"] = json!(id);
                crate::record::write_atomic(
                    &path,
                    &serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?,
                )?;
                println!("{id}: retained execution audit");
                Ok(())
            }
        }))
        .buffer_unordered(4)
        .collect()
        .await;
    for result in results {
        result?;
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn observations_keep_order_and_exclude_reasoning_and_unmatched_results() {
        let lines = [
            json!({"type":"assistant","message":{"content":[{"type":"tool_use","id":"a","name":"Bash","input":{"command":"python check.py"}}]}}),
            json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"other","content":"FAILED"}]}}),
            json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"a","content":"AssertionError","is_error":true}]}}),
            json!({"record":"step","step":{"call":{"name":"apply_patch","arguments":{"patch":"fix"},"output":"Applied"}}}),
            json!({"type":"item.completed","item":{"type":"command_execution","command":"python check.py","aggregated_output":"OK","exit_code":0}}),
        ];
        let result = events(
            &lines
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join("\n"),
        );
        assert_eq!(result.len(), 3);
        assert_eq!(result[0]["output"], "AssertionError");
        assert_eq!(result[2]["error"]["exit_code"], 0);
        assert_eq!(result[1]["tool"], "apply_patch");
    }
    #[test]
    fn missing_or_invalid_judgments_cannot_supply_a_score() {
        assert!(score(&json!({"observed":{"noul":1},"unresolved":{"noul":1}})).is_none());
        assert_eq!(
            score(
                &json!({"observed":{"noul":0.9},"unresolved":{"noul":0.3},"required":{"noul":0.8}})
            ),
            Some(0.3)
        );
        assert!(questions().validate().is_ok());
    }
}
