//! Distinguish a reported, current task failure from a verification caveat.
//!
//! This development component reads the selected report, not the grader. It
//! asks about three narrower conditions instead of predicting overall acceptance.

use std::path::PathBuf;

use jev::{Noul, NoulCriteria, Questions};
use serde_json::{Value, json};

use crate::component::jev::{Ask, JevMode, ask};
use crate::record::Recorder;

#[must_use]
pub fn questions() -> Questions {
    Questions::new()
        .with("observed", Noul::with_criteria(
            "Does the report explicitly state that a concrete required behavior failed, a required result is wrong, or a required deliverable was not completed? Count a stated failed test, infeasible required output, wrong value, missing implementation, or unfinished requirement. Do not count uncertainty about hidden tests, limited test coverage, inability to use an optional verification tool, a design assumption, a hypothetical failure, or lack of proof of correctness.",
            NoulCriteria::new().when_true("The report states an actual failure or uncompleted deliverable, not merely uncertainty.")
                .when_false("It reports success or only a caveat, risk, ambiguity, assumption, or verification limit.")))
        .with("current", Noul::with_criteria(
            "Does that concrete failure or unfinished deliverable remain in the final submitted result according to the report? A failed earlier attempt followed by a stated successful fix is not a current failure. A possible remaining defect is not an observed remaining defect.",
            NoulCriteria::new().when_true("A concrete failure is explicitly left unresolved in the final result.")
                .when_false("No concrete failure is stated, it was fixed, or only uncertainty remains.")))
        .with("required", Noul::with_criteria(
            "Is the reported failed or unfinished behavior mandatory under the supplied task? Match it to what the task actually asks. An optional extension, extra check, unrelated environment issue, or stricter interpretation not required by the task does not count.",
            NoulCriteria::new().when_true("A mandatory task requirement is explicitly unmet.")
                .when_false("No mandatory unmet requirement is established.")))
}

/// Evidence strength, not a calibrated probability of failure.
#[must_use]
pub fn score(answers: &Value) -> Option<f64> {
    let mut result: f64 = 1.0;
    for key in ["observed", "current", "required"] {
        let p = answers[key]["noul"].as_f64()?;
        if !p.is_finite() || !(0.0..=1.0).contains(&p) {
            return None;
        }
        result = result.min(p);
    }
    Some(result)
}

/// Ask about one selected report without loading any benchmark labels.
pub async fn assess(
    task: &str,
    report: &str,
    mode: &JevMode,
    recorder: &Recorder,
    id: &str,
) -> Value {
    let state = super::verdict::report_state(task, report);
    let digest = crate::component::jev::key(&state, &json!(questions()));
    let asked = ask(
        mode,
        recorder,
        Ask {
            component: "verify.report-audit",
            name: "jev_concrete_report_failure",
            id: id.to_string(),
            state: state.clone(),
            questions: questions(),
            parent: None,
            deadline: None,
        },
    )
    .await;
    json!({"schema":"openagents.coder-one.report-audit.v1","digest":digest,
        "state":state,"questions":questions(),"answers":asked.answers,
        "score":asked.answers.as_ref().and_then(score),"error":asked.error,
        "input_tokens":asked.input_tokens,"milliseconds":asked.milliseconds,
        "steps":recorder.steps()})
}

/// Measure only the explicitly selected partition of a retained row manifest.
///
/// # Errors
/// Returns invalid arguments, missing credentials, or unreadable records.
pub async fn command(args: &[String]) -> Result<i32, String> {
    let mut args = args.iter();
    let (mut jobs, mut rows, mut out) = (None, None, None);
    let (mut trial_dir, mut input) = (None, None);
    let mut partition = "calibration".to_string();
    while let Some(arg) = args.next() {
        let value = args.next().ok_or("report-audit needs paired options")?;
        match arg.as_str() {
            "--jobs" => jobs = Some(PathBuf::from(value)),
            "--rows" => rows = Some(PathBuf::from(value)),
            "--out" => out = Some(PathBuf::from(value)),
            "--partition" => partition.clone_from(value),
            "--trial-dir" => trial_dir = Some(PathBuf::from(value)),
            "--input" => input = Some(PathBuf::from(value)),
            _ => return Err(format!("Unknown report-audit option {arg}")),
        }
    }
    if !["calibration", "held-out", "all"].contains(&partition.as_str()) {
        return Err("report-audit partition must be calibration, held-out, or all".to_string());
    }
    let out = out.ok_or("report-audit needs --out")?;
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let dir = crate::credentials::openagents_dir().ok_or("HOME is not set")?;
    let key = crate::credentials::jev_key(|name| std::env::var(name).ok(), &dir)?;
    let mode = JevMode::Live(crate::credentials::jev_client(&key.secret)?);
    if let Some(trial) = trial_dir {
        if rows.is_some() || jobs.is_some() {
            return Err("Use either --trial-dir or --rows and --jobs".to_string());
        }
        let input: super::review::Input = serde_json::from_str(
            &std::fs::read_to_string(input.ok_or("--trial-dir needs --input")?)
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let episode = trial.join("agent/episode");
        let composition: Value = serde_json::from_str(
            &std::fs::read_to_string(episode.join("artifacts/composition.json"))
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let selected = super::truth::final_report(&episode, &composition);
        let id = trial
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("Trial directory has no name")?;
        let path = out.join(format!("{id}.json"));
        if path.exists() {
            return Err(
                "Report audit already exists; preserve it and choose another output".to_string(),
            );
        }
        let mut record = if let Some(report) = selected.report {
            assess(&input.task, &report, &mode, &Recorder::default(), id).await
        } else {
            json!({"schema":"openagents.coder-one.report-audit.v1","score":null,"error":selected.unavailable})
        };
        record["trial"] = json!(id);
        record["job"] = json!(
            trial
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|s| s.to_str())
        );
        record["report_source"] = json!(selected.source);
        crate::record::write_atomic(
            &path,
            &serde_json::to_vec_pretty(&record).map_err(|e| e.to_string())?,
        )?;
        println!("Wrote report audit to {}", path.display());
        return Ok(0);
    }
    if input.is_some() {
        return Err("--input needs --trial-dir".to_string());
    }
    let rows = super::truth::read_rows(&rows.ok_or("report-audit needs --rows")?)?;
    let jobs = jobs.ok_or("report-audit needs --jobs")?;
    for row in rows {
        let split = serde_json::to_value(row.split).map_err(|e| e.to_string())?;
        if partition != "all" && split != partition {
            continue;
        }
        let Some(loaded) = super::truth::load(&row.job, &jobs.join(&row.job).join(&row.trial))
        else {
            continue;
        };
        let (Some(task), Some(report)) = (loaded.instruction, loaded.report) else {
            continue;
        };
        let state = super::verdict::report_state(&task, &report);
        let digest = crate::component::jev::key(&state, &json!(questions()));
        let path = out.join(format!("{}.json", row.trial));
        if path.exists() {
            let v: Value =
                serde_json::from_str(&std::fs::read_to_string(&path).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            if v["digest"] != digest {
                return Err(format!("{} belongs to a different request", path.display()));
            }
            continue;
        }
        let recorder = Recorder::default();
        let mut record = assess(&task, &report, &mode, &recorder, &row.trial).await;
        record["job"] = json!(row.job);
        record["trial"] = json!(row.trial);
        crate::record::write_atomic(
            &path,
            &serde_json::to_vec_pretty(&record).map_err(|e| e.to_string())?,
        )?;
    }
    println!("Wrote {} report audits to {}", partition, out.display());
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn an_unknown_current_failure_cannot_inherit_other_answers() {
        assert_eq!(
            score(&json!({"observed":{"noul":0.99},"required":{"noul":0.99}})),
            None
        );
        assert_eq!(
            score(
                &json!({"observed":{"noul":0.99},"required":{"noul":0.99},"current":{"noul":0.1}})
            ),
            Some(0.1)
        );
    }
}
