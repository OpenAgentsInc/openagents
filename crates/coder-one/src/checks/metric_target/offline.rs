//! `coder-one checks metric-target offline`: extraction on every labeled
//! task's instruction, compared with hand labels frozen before any Jev
//! answer was read.
//!
//! The comparison rule, fixed in the protocol
//! (`bench/terminal-bench/experiments/2026-09-25-metric-target/protocol.md`):
//!
//! - A task is **positive** when its label states a numeric goal, and
//!   Jev's extraction is positive when it names at least one target.
//! - On a task both call positive, Jev's **first** target, the one the
//!   lean loop holds, is **correct** when its threshold equals the first
//!   labeled target's, its direction matches, and both are absolute or
//!   both relative. The quantity is compared and reported apart.
//! - A task both call negative is a correct negative.

use std::path::Path;

use serde_json::{Value, json};

use super::cli::{OFFLINE_USD, extract_with, keep_live, modes};
use super::{Direction, Target};
use crate::component::jev::{self as jev_component, Recorded};

/// The labels file's schema.
pub const LABELS_SCHEMA: &str = "openagents.metric-target-labels.v1";

/// The recorded input tokens' cost, in dollars.
fn recorded_usd(recorded: &Recorded) -> f64 {
    recorded
        .entries
        .values()
        .filter_map(|e| e.input_tokens)
        .sum::<u64>() as f64
        * jev_component::USD_PER_MILLION_INPUT
        / 1_000_000.0
}

fn same(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0)
}

fn direction(word: &str) -> Option<Direction> {
    match word {
        "at_least" => Some(Direction::AtLeast),
        "at_most" => Some(Direction::AtMost),
        _ => None,
    }
}

/// How one extracted target compares with one labeled target.
#[must_use]
pub fn compare(target: &Target, label: &Value) -> Value {
    let threshold = label["threshold"]
        .as_f64()
        .is_some_and(|t| same(t, target.threshold));
    let direction = label["direction"]
        .as_str()
        .and_then(direction)
        .is_some_and(|d| d == target.direction);
    let relative = label["relative_to"].is_null() != target.relative.relative();
    let quantity = label["quantity"].as_str() == Some(target.quantity.word());
    json!({
        "threshold": threshold,
        "direction": direction,
        "relative": relative,
        "quantity": quantity,
        "correct": threshold && direction && relative,
    })
}

/// Runs the offline extraction and writes its records; returns the
/// summary.
///
/// # Errors
///
/// A message when the labels can't be read or a record can't be written.
#[allow(clippy::too_many_lines)]
pub async fn run(
    labels: &Path,
    out: &Path,
    recorded_path: &Path,
    jev: &str,
) -> Result<Value, String> {
    let text = std::fs::read_to_string(labels)
        .map_err(|e| format!("cannot read {}: {e}", labels.display()))?;
    let labels: Value =
        serde_json::from_str(&text).map_err(|e| format!("{} isn't JSON: {e}", labels.display()))?;
    if labels["schema"] != LABELS_SCHEMA {
        return Err(format!("the labels' schema isn't {LABELS_SCHEMA}"));
    }
    let mut recorded = Recorded::load(recorded_path)?;
    let mut rows = Vec::new();
    let mut extracted_all = Vec::new();
    let mut stopped = None;
    for task in labels["tasks"].as_array().cloned().unwrap_or_default() {
        let name = task["task"].as_str().unwrap_or_default().to_string();
        let path = task["instruction_path"].as_str().unwrap_or_default();
        let bytes = std::fs::read(path).map_err(|e| format!("cannot read {path}: {e}"))?;
        if Some(crate::accept::sha256(&bytes).as_str()) != task["instruction_sha256"].as_str() {
            return Err(format!(
                "{name}: the instruction changed since it was labeled"
            ));
        }
        let instruction = String::from_utf8_lossy(&bytes).to_string();
        let word = if jev == "live" && recorded_usd(&recorded) >= OFFLINE_USD {
            stopped = Some(format!(
                "the Jev budget ${OFFLINE_USD:.2} was reached before {name}"
            ));
            "recorded"
        } else {
            jev
        };
        let (mode, replay) = modes(word, &recorded)?;
        let extracted = extract_with(
            &mode,
            replay.as_ref(),
            &instruction,
            None,
            &format!("jev-metric-target-{name}"),
        )
        .await;
        if keep_live(&extracted.call, &name, &mut recorded) {
            recorded.save(recorded_path)?;
        }
        let labeled = task["states_numeric_goal"].as_bool().unwrap_or(false);
        let first_label = task["targets"].get(0).cloned().unwrap_or(Value::Null);
        let found = !extracted.targets.is_empty();
        let first = extracted
            .targets
            .first()
            .map(|t| compare(t, &first_label))
            .filter(|_| labeled);
        let any = labeled
            && extracted
                .targets
                .iter()
                .any(|t| compare(t, &first_label)["correct"] == true);
        let outcome = match (labeled, found, extracted.answered) {
            (_, _, false) => "unanswered",
            (true, true, _) => "true_positive",
            (true, false, _) => "false_negative",
            (false, true, _) => "false_positive",
            (false, false, _) => "true_negative",
        };
        rows.push(json!({
            "task": name,
            "labeled": labeled,
            "extracted": found,
            "outcome": outcome,
            "first": first,
            "first_correct": first.as_ref().is_some_and(|f| f["correct"] == true),
            "any_correct": any,
            "label": first_label,
            "targets": extracted.targets,
            "how": extracted.call["how"],
        }));
        extracted_all.push(json!({"task": name, "extracted": extracted}));
    }
    let count = |f: &dyn Fn(&Value) -> bool| rows.iter().filter(|r| f(r)).count();
    let totals = json!({
        "tasks": rows.len(),
        "labeled_goal": count(&|r| r["labeled"] == true),
        "extracted_goal": count(&|r| r["extracted"] == true),
        "true_positive": count(&|r| r["outcome"] == "true_positive"),
        "false_positive": count(&|r| r["outcome"] == "false_positive"),
        "false_negative": count(&|r| r["outcome"] == "false_negative"),
        "true_negative": count(&|r| r["outcome"] == "true_negative"),
        "unanswered": count(&|r| r["outcome"] == "unanswered"),
        "first_correct": count(&|r| r["first_correct"] == true),
        "any_correct": count(&|r| r["any_correct"] == true),
        "quantity_agrees": count(&|r| r["first"]["quantity"] == true),
        "jev_usd_recorded": recorded_usd(&recorded),
        "stopped": stopped,
    });
    let summary = json!({
        "schema": "openagents.metric-target-offline.v1",
        "questions": super::question_set().id,
        "questions_digest": super::question_set().digest,
        "totals": totals,
        "rows": rows,
    });
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let write = |file: &str, value: &Value| {
        let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
        crate::record::write_atomic(&out.join(file), format!("{text}\n").as_bytes())
    };
    write("extracted.json", &Value::Array(extracted_all))?;
    write("summary.json", &summary)?;
    Ok(summary)
}
