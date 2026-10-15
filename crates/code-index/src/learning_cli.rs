//! Explicit local learning commands. No history upload or implicit model calls.

use code_index::{evaluation, learning, recipes::Artifact};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub fn run(args: &[String]) -> Result<Option<Value>, String> {
    let Some(command) = args.first().map(String::as_str) else {
        return Ok(None);
    };
    if !matches!(
        command,
        "feedback"
            | "train-selector"
            | "compare-selector"
            | "promote-selector"
            | "rollback-selector"
    ) {
        return Ok(None);
    }
    let value = |key: &str| {
        args.iter()
            .position(|a| a == key)
            .and_then(|i| args.get(i + 1))
            .filter(|s| !s.starts_with("--"))
            .cloned()
    };
    let path = |key: &str| {
        value(key)
            .map(PathBuf::from)
            .ok_or_else(|| format!("Choose a file or directory with {key} PATH."))
    };
    if command == "rollback-selector" {
        learning::rollback(
            &path("--store")?,
            &value("--digest").ok_or("Choose the saved selector with --digest HASH.")?,
            &value("--expected").ok_or("Choose the current selector with --expected HASH.")?,
        )?;
        return Ok(Some(json!({"rolled_back":true})));
    }
    let partitions: learning::Partitions = code_index::read_json(&path("--partitions")?)?;
    if command == "feedback" {
        let corpus = value("--corpus")
            .map(|p| code_index::read_json::<code_index::episodes::Corpus>(Path::new(&p)))
            .transpose()?;
        let feedback = learning::collect(&path("--observations")?, &partitions, corpus.as_ref())?;
        code_index::private::write(
            &path("--out")?,
            "feedback.json",
            &serde_json::to_vec(&feedback).map_err(|e| e.to_string())?,
        )?;
        return Ok(Some(
            json!({"observations":feedback.observations.len(),"misses":feedback.observations.iter().filter(|o|o.status!="matched" || !o.required_roles_complete).count(),"notices":feedback.notices}),
        ));
    }
    let artifact: Artifact = code_index::read_json(&path("--index")?)?;
    let plan: evaluation::Plan = code_index::read_json(&path("--plan")?)?;
    let report: evaluation::Report = code_index::read_json(&path("--report")?)?;
    if command == "train-selector" {
        if plan.purpose != evaluation::Purpose::Training {
            return Err("Choose a training plan; evaluation cases cannot train a selector.".into());
        }
        let run = path("--run-dir")?;
        let out = path("--out")?;
        let target = path("--target")?;
        let mut targets = Vec::new();
        for trial in report.trials.iter().filter(|t| t.accepted) {
            let name = code_index::digest(
                format!(
                    "{}:{}:{}:{}",
                    trial.plan_digest, trial.case, trial.arm, trial.repetition
                )
                .as_bytes(),
            );
            let patch =
                std::fs::read(run.join(&name).join("change.patch")).map_err(|e| e.to_string())?;
            let replay = evaluation::verify_for_learning(
                &plan,
                trial,
                &patch,
                &out.join("replays").join(name),
                &target,
            )?;
            targets.extend(learning::admit(
                replay,
                &partitions,
                args.iter().any(|a| a == "--fixture"),
            )?);
        }
        let selector = learning::train(&artifact, &partitions, &targets)?;
        code_index::private::write(
            &out,
            "targets.json",
            &serde_json::to_vec(&targets).map_err(|e| e.to_string())?,
        )?;
        code_index::private::write(
            &out,
            "selector.json",
            &serde_json::to_vec(&selector).map_err(|e| e.to_string())?,
        )?;
        return Ok(Some(
            json!({"targets":targets.len(),"training_groups":selector.training_groups.len(),"fixture_only":selector.fixture_only,"selector_digest":atif::digest(&serde_json::to_value(selector).map_err(|e|e.to_string())?)}),
        ));
    }
    let selector: learning::Selector = code_index::read_json(&path("--candidate")?)?;
    let cases: Vec<learning::TestCase> = code_index::read_json(&path("--cases")?)?;
    let baseline = value("--baseline")
        .map(|p| code_index::read_json::<learning::Selector>(Path::new(&p)))
        .transpose()?;
    let gain = plan
        .selector_minimum_gain
        .ok_or("Freeze selector_minimum_gain in the evaluation plan.")?;
    let receipt = learning::compare(
        &selector,
        baseline.as_ref(),
        &artifact,
        &partitions,
        &cases,
        &plan,
        &report,
        gain,
    )?;
    if command == "compare-selector" {
        code_index::private::write(
            &path("--out")?,
            "comparison.json",
            &serde_json::to_vec(&receipt).map_err(|e| e.to_string())?,
        )?;
        return Ok(Some(
            json!({"passed":receipt.passed,"candidate_recall":receipt.candidate_recall,"baseline_recall":receipt.baseline_recall}),
        ));
    }
    let saved: learning::SelectorReceipt = code_index::read_json(&path("--receipt")?)?;
    let id = learning::promote(
        &path("--store")?,
        &selector,
        &saved,
        value("--expected").as_deref(),
        &artifact,
        &partitions,
        &cases,
        &plan,
        &report,
    )?;
    Ok(Some(json!({"active_selector":id})))
}
