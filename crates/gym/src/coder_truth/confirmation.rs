//! Read the frozen archive measurement without running checks or inference.

use std::{io::Read, path::Path};

use serde_json::{Value, json};

const SCHEMA: &str = "openagents.archive-confirmation-measurement.v1";
const MAX_BYTES: u64 = 16 * 1024 * 1024;

fn count(value: &Value, key: &str) -> Result<u64, String> {
    value[key]
        .as_u64()
        .ok_or_else(|| format!("confirmation measurement has no valid {key} count"))
}

fn rate(value: &Value) -> Result<String, String> {
    let (k, n) = (count(value, "correct")?, count(value, "total")?);
    if k > n {
        return Err("confirmation rate numerator exceeds its denominator".to_string());
    }
    if n == 0 {
        if !value["value"].is_null() || !value["wilson_95"].is_null() {
            return Err("an empty confirmation rate must stay undefined".to_string());
        }
        return Ok("0/0 undefined".to_string());
    }
    let p = value["value"]
        .as_f64()
        .ok_or("confirmation rate has no value")?;
    let interval = interval(&value["wilson_95"], 0.0, 1.0)?;
    #[allow(clippy::cast_precision_loss)]
    let expected = k as f64 / n as f64;
    if (p - expected).abs() > 1e-9 || p + 1e-9 < interval.0 || p - 1e-9 > interval.1 {
        return Err("confirmation rate does not match its counts or interval".to_string());
    }
    Ok(format!(
        "{k}/{n} {:.1}% ({:.1}–{:.1}%)",
        p * 100.0,
        interval.0 * 100.0,
        interval.1 * 100.0
    ))
}

fn interval(value: &Value, min: f64, max: f64) -> Result<(f64, f64), String> {
    let a = value.as_array().filter(|a| a.len() == 2);
    let pair = a.and_then(|a| Some((a[0].as_f64()?, a[1].as_f64()?)));
    match pair {
        Some((low, high)) if min <= low && low <= high && high <= max => Ok((low, high)),
        _ => Err("confirmation measurement has an invalid interval".to_string()),
    }
}

fn group_lines(group: &Value) -> Result<Vec<String>, String> {
    let (trials, graded, passes, failures, tasks) = (
        count(group, "trials")?,
        count(group, "graded")?,
        count(group, "passes")?,
        count(group, "failures")?,
        count(group, "tasks")?,
    );
    let missing = group["unknown_outcomes"]
        .as_array()
        .ok_or("confirmation measurement has no missing-outcome list")?;
    if passes.checked_add(failures) != Some(graded)
        || graded.checked_add(missing.len() as u64) != Some(trials)
        || tasks > graded
    {
        return Err("confirmation coverage counts disagree".to_string());
    }
    let mut lines = vec![format!(
        "  {graded}/{trials} graded · {passes} passes · {failures} failures · {tasks} tasks · {} missing grades",
        missing.len()
    )];
    let signals = group["signals"]
        .as_object()
        .ok_or("confirmation measurement has no signals")?;
    for required in [
        "checks.final",
        "verdict.combined",
        "checks.public-files",
        "verdict.executed",
    ] {
        if !signals.contains_key(required) {
            return Err(format!("confirmation measurement is missing {required}"));
        }
    }
    for (name, signal) in signals {
        if count(&signal["failure_recall"], "total")? != failures
            || count(&signal["fail_precision"], "correct")?
                != count(&signal["failure_recall"], "correct")?
            || count(&signal["fail_precision"], "total")? > graded
            || count(signal, "unknown")? > graded
        {
            return Err(format!(
                "confirmation signal {name} has inconsistent coverage"
            ));
        }
        lines.push(format!(
            "  {name}: fail precision {} · failure recall {} · {} unknown calls",
            rate(&signal["fail_precision"])?,
            rate(&signal["failure_recall"])?,
            signal["unknown"]
        ));
    }
    Ok(lines)
}

fn lines(report: &Value, executor: Option<&str>) -> Result<Vec<String>, String> {
    if report["schema"] != SCHEMA {
        return Err(format!("expected a {SCHEMA} measurement"));
    }
    if executor.is_some_and(|e| !matches!(e, "luna" | "astra")) {
        return Err("--executor must be luna or astra".to_string());
    }
    // Validate the complete cohort even when displaying just one executor.
    let all = group_lines(&report["all"])?;
    for e in ["luna", "astra"] {
        group_lines(&report["by_executor"][e])?;
    }
    for key in ["trials", "graded", "passes", "failures"] {
        let luna = count(&report["by_executor"]["luna"], key)?;
        let astra = count(&report["by_executor"]["astra"], key)?;
        if luna.checked_add(astra) != Some(count(&report["all"], key)?) {
            return Err(format!(
                "executor {key} counts disagree with the full cohort"
            ));
        }
    }
    let mut output = vec!["Retained archive confirmation measurement".to_string()];
    for key in ["prediction_sha256", "labels_sha256"] {
        let digest = report[key]
            .as_str()
            .filter(|d| d.len() == 64 && d.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or_else(|| format!("confirmation measurement has no valid {key}"))?;
        output.push(format!("  {key}: {digest}"));
    }
    let group = if let Some(e) = executor {
        output.push(format!("  Executor: {e}"));
        output.extend(group_lines(&report["by_executor"][e])?);
        &report["by_executor"][e]
    } else {
        output.extend(all);
        &report["all"]
    };
    let resamples = count(report, "bootstrap_resamples")?;
    let comparisons = group["paired_task_bootstrap"]
        .as_object()
        .ok_or("confirmation measurement has no paired task intervals")?;
    output.push("  Differences resample whole tasks; positive favors the new signal.".to_string());
    for (signal, baselines) in comparisons {
        let baselines = baselines
            .as_object()
            .ok_or("invalid bootstrap comparisons")?;
        for (baseline, metrics) in baselines {
            for metric in ["precision", "recall"] {
                let record = &metrics[metric];
                let valid = count(record, "valid_resamples")?;
                let undefined = count(record, "undefined_resamples")?;
                if valid.checked_add(undefined) != Some(resamples) {
                    return Err("bootstrap resample counts disagree".to_string());
                }
                let range = if valid == 0 && record["percentile_95"].is_null() {
                    "undefined".to_string()
                } else {
                    let (low, high) = interval(&record["percentile_95"], -1.0, 1.0)?;
                    format!(
                        "{:+.1} to {:+.1} percentage points",
                        low * 100.0,
                        high * 100.0
                    )
                };
                output.push(format!(
                    "    {signal} vs {baseline}, {metric}: {range}; {undefined}/{resamples} undefined resamples"
                ));
            }
        }
    }
    let within = executor.map_or(&report["within_task"], |e| {
        &report["within_task_by_executor"][e]
    });
    for (signal, row) in within
        .as_object()
        .ok_or("missing within-task comparisons")?
    {
        let pairs = count(row, "pairs")?;
        let score = row["concordance"].as_f64();
        let text = match (pairs, score) {
            (0, None) => "undefined (no mixed-outcome task)".to_string(),
            (n, Some(p)) if n > 0 && (0.0..=1.0).contains(&p) => {
                format!(
                    "{:.1}% across {n} dependent pairs; ties count one half",
                    p * 100.0
                )
            }
            _ => return Err("invalid within-task concordance".to_string()),
        };
        output.push(format!("  {signal}, within-task failure ranking: {text}"));
    }
    if let Some(notes) = report["notes"].as_array() {
        for note in notes.iter().filter_map(Value::as_str) {
            output.push(format!("  {note}"));
        }
    }
    output.push(
        "  Reading this report does not verify its source files or promote a runtime rule."
            .to_string(),
    );
    Ok(output)
}

pub(super) fn command(
    path: &Path,
    executor: Option<&str>,
    json_output: bool,
    out: &mut impl std::io::Write,
) -> Result<i32, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("confirmation measurement exceeds 16 MiB".to_string());
    }
    let report: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let rendered = lines(&report, executor)?;
    if json_output {
        let view = json!({"schema": "openagents.gym.coder-truth-confirmation.v1",
                          "path": path, "executor": executor, "report": report});
        serde_json::to_writer_pretty(&mut *out, &view).map_err(|e| e.to_string())?;
        writeln!(out).map_err(|e| e.to_string())?;
    } else {
        for line in rendered {
            writeln!(out, "{line}").map_err(|e| e.to_string())?;
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Value {
        let empty = json!({"correct": 0, "total": 0, "value": null, "wilson_95": null});
        let missed = json!({"correct": 0, "total": 1, "value": 0.0, "wilson_95": [0.0, 0.79]});
        let signal = json!({"fail_precision": empty, "failure_recall": missed, "unknown": 2});
        let metric =
            json!({"valid_resamples": 0, "undefined_resamples": 10000, "percentile_95": null});
        let group = json!({
            "trials": 3, "graded": 2, "passes": 1, "failures": 1, "tasks": 1,
            "unknown_outcomes": ["unavailable"],
            "signals": {"checks.final": signal, "verdict.combined": signal,
                        "checks.public-files": signal, "verdict.executed": signal},
            "paired_task_bootstrap": {"verdict.executed": {"checks.final": {
                "precision": metric, "recall": metric}}}
        });
        let within = json!({"verdict.executed": {"pairs": 1, "concordance": 0.5}});
        let mut all = group.clone();
        all["trials"] = json!(6);
        all["graded"] = json!(4);
        all["passes"] = json!(2);
        all["failures"] = json!(2);
        all["unknown_outcomes"] = json!(["unavailable-luna", "unavailable-astra"]);
        for signal in all["signals"].as_object_mut().unwrap().values_mut() {
            signal["failure_recall"]["total"] = json!(2);
            signal["unknown"] = json!(4);
        }
        json!({"schema": SCHEMA, "prediction_sha256": "a".repeat(64),
               "labels_sha256": "b".repeat(64), "bootstrap_resamples": 10000,
               "all": all, "by_executor": {"luna": group, "astra": group},
               "within_task": within, "within_task_by_executor": {"luna": within, "astra": within},
               "notes": ["Synthetic display fixture; no measured model result."]})
    }

    #[test]
    fn missing_evidence_and_undefined_rates_stay_visible() {
        let text = lines(&fixture(), None).unwrap().join("\n");
        assert!(text.contains("4/6 graded"));
        assert!(text.contains("2 missing grades"));
        assert!(text.contains("fail precision 0/0 undefined"));
        assert!(text.contains("failure recall 0/2 0.0%"));
        assert!(text.contains("10000/10000 undefined resamples"));
        assert!(text.contains("50.0% across 1 dependent pairs"));
        assert!(text.contains("does not verify its source files or promote"));
    }

    #[test]
    fn refuses_inconsistent_counts_intervals_and_identity() {
        for (pointer, value) in [
            ("/all/failures", json!(0)),
            (
                "/all/signals/verdict.executed/fail_precision/value",
                json!(0),
            ),
            (
                "/all/signals/verdict.executed/failure_recall/total",
                json!(0),
            ),
            (
                "/all/signals/verdict.executed/failure_recall/wilson_95",
                json!([0.8, 0.2]),
            ),
            ("/prediction_sha256", json!("missing")),
            ("/bootstrap_resamples", json!(9999)),
        ] {
            let mut report = fixture();
            *report.pointer_mut(pointer).unwrap() = value;
            assert!(lines(&report, None).is_err(), "accepted {pointer}");
        }
    }

    #[test]
    fn wilson_rounding_at_zero_and_one_is_not_a_corrupt_rate() {
        // These are actual floating-point outputs from the retained Python
        // measurement, where Wilson endpoints differ from 0 or 1 by one ULP.
        let zero = json!({"correct": 0, "total": 11, "value": 0.0,
                          "wilson_95": [2.7755575615628914e-17, 0.2588400172488141]});
        let one = json!({"correct": 6, "total": 6, "value": 1.0,
                         "wilson_95": [0.6096569663469354, 0.9999999999999999]});
        assert!(rate(&zero).unwrap().contains("0/11 0.0%"));
        assert!(rate(&one).unwrap().contains("6/6 100.0%"));
    }

    #[test]
    fn command_reads_a_report_and_rejects_mixed_population_options() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("measurement.json");
        std::fs::write(&path, fixture().to_string()).unwrap();
        let args = vec![
            "--confirmation".to_string(),
            path.display().to_string(),
            "--executor".to_string(),
            "luna".to_string(),
            "--json".to_string(),
        ];
        let mut out = Vec::new();
        assert_eq!(crate::coder_truth::command(&args, &mut out).unwrap(), 0);
        let result: Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(result["executor"], "luna");
        assert_eq!(
            result["report"]["all"]["unknown_outcomes"],
            json!(["unavailable-luna", "unavailable-astra"])
        );
        for option in ["--within", "--set", "--dir", "--family"] {
            let mut invalid = args.clone();
            invalid.push(option.to_string());
            if option != "--within" {
                invalid.push("all".to_string());
            }
            assert!(crate::coder_truth::command(&invalid, &mut Vec::new()).is_err());
        }
        assert!(lines(&fixture(), Some("unknown")).is_err());
        assert!(
            crate::coder_truth::command(
                &["--executor".to_string(), "luna".to_string()],
                &mut Vec::new()
            )
            .is_err()
        );
    }
}
