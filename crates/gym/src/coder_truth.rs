//! Truthful checks: each signal's discrimination on the labeled set.
//!
//! `coder-one checks truth` labels every graded retained Coder One trial
//! with its verifier reward, splits the trials by task into a calibration
//! half and a held-out half, and measures each signal the checks leave:
//! every scenario kind, requirement states, Jev support, the self-report,
//! Jev's answers over the final report, and the calibrated combined
//! verdict. It writes `openagents.coder-one.check-truth.v1` to
//! `~/.openagents/coder-one/checks-truth/summary.json`. This module reads
//! that summary and renders it: fail precision, failure recall, and the
//! pass rate when a signal says pass, each with its 95% Wilson interval.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// The schema a truth summary carries.
pub const SCHEMA: &str = "openagents.coder-one.check-truth.v1";

/// The schema of this module's JSON.
pub const VIEW_SCHEMA: &str = "openagents.gym.coder-truth.v1";

const HELP: &str = "\
gym coder truth [--dir PATH] [--set held-out|calibration|all] [--family NAME]
                [--within] [--json]

Each check signal's discrimination on the labeled Terminal-Bench set that
`coder-one checks truth` writes: fail precision (of the trials the signal
calls failed, how many the verifier failed), failure recall (of the
verifier's failures, how many it called failed), and the pass rate when it
says pass, with 95% Wilson intervals. The label set is split by task, so no
task is in both halves; the combined verdict is fitted on the calibration
half and measured on the held-out half.

  --dir PATH      the truth directory (default ~/.openagents/coder-one/checks-truth)
  --set NAME      held-out (default), calibration, or all
  --family NAME   only one family: checks, scenario, requirements, support,
                  self-report, control, report, or verdict
  --within        precision and recall on tasks the verifier both passed and
                  failed: whether a signal tells two attempts at one task apart
  --json          print versioned JSON instead of text";

/// Where `coder-one checks truth` writes by default.
#[must_use]
pub fn default_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(|home| PathBuf::from(home).join(".openagents/coder-one/checks-truth"))
}

/// Reads the summary in `dir`.
///
/// # Errors
///
/// Returns why the summary can't be read.
pub fn load(dir: &Path) -> Result<Value, String> {
    let path = dir.join("summary.json");
    let text = std::fs::read_to_string(&path).map_err(|e| {
        format!(
            "cannot read {}: {e}; run `coder-one checks truth` first",
            path.display()
        )
    })?;
    let value: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    if value["schema"] != SCHEMA {
        return Err(format!("{} isn't a {SCHEMA} summary", path.display()));
    }
    Ok(value)
}

/// A rate as `k/n p% (low–high)`, or `k/n` when `n` is zero.
fn cell(rate: &Value) -> String {
    let (k, n) = (
        rate["k"].as_u64().unwrap_or(0),
        rate["n"].as_u64().unwrap_or(0),
    );
    match (
        rate["p"].as_f64(),
        rate["low"].as_f64(),
        rate["high"].as_f64(),
    ) {
        (Some(p), Some(l), Some(h)) => format!(
            "{k:>3}/{n:<3} {:>3.0}% {:>3.0}–{:<3.0}",
            p * 100.0,
            l * 100.0,
            h * 100.0
        ),
        _ => format!("{k:>3}/{n:<3} {:>13}", "—"),
    }
}

/// The summary key of a set word.
fn set_key(set: &str) -> Option<&'static str> {
    match set {
        "held-out" | "held_out" => Some("held_out"),
        "calibration" => Some("calibration"),
        "all" => Some("all"),
        _ => None,
    }
}

/// The summary as text lines: the label set, one row per signal, then the
/// verdict against today's checks on the held-out half.
#[must_use]
pub fn lines(summary: &Value, set: &str, family: Option<&str>, within: bool) -> Vec<String> {
    let key = set_key(set).unwrap_or("held_out");
    let labels = &summary["label_set"];
    let mut out = vec![
        "Truthful checks on the labeled Terminal-Bench set".to_string(),
        format!(
            "  {} graded trials ({} passes, {} failures) on {} tasks · report answers on {}",
            labels["trials"],
            labels["passes"],
            labels["failures"],
            labels["tasks"],
            summary["report_answers"]
        ),
        format!(
            "  calibration: {} trials on {} tasks · held-out: {} trials on {} tasks ({} failures)",
            labels["calibration"]["trials"],
            labels["calibration"]["tasks"],
            labels["held_out"]["trials"],
            labels["held_out"]["tasks"],
            labels["held_out"]["failures"]
        ),
        String::new(),
        format!(
            "  {:<42} {:<23} {:<23} {:<23} {}",
            format!(
                "signal ({}{})",
                key.replace('_', "-"),
                if within { ", within task" } else { "" }
            ),
            "fail precision",
            "failure recall",
            if within { "" } else { "pass when \"pass\"" },
            "separates"
        ),
    ];
    for m in summary["signals"][key].as_array().into_iter().flatten() {
        if family.is_some_and(|f| m["family"] != f) {
            continue;
        }
        let (precision, recall) = if within {
            (&m["within_fail_precision"], &m["within_failure_recall"])
        } else {
            (&m["fail_precision"], &m["failure_recall"])
        };
        if precision["n"] == 0 && m["pass_when_pass"]["n"] == 0 {
            continue;
        }
        out.push(format!(
            "  {:<42} {:<23} {:<23} {:<23} {}",
            m["signal"].as_str().unwrap_or_default(),
            cell(precision),
            cell(recall),
            if within {
                String::new()
            } else {
                cell(&m["pass_when_pass"])
            },
            if m["separates"] == true { "yes" } else { "no" }
        ));
    }
    out.push(String::new());
    out.push("  Held-out half: the combined verdict compared with today's checks".to_string());
    for (label, name) in [
        ("combined verdict", "verdict"),
        ("today's checks", "todays_checks"),
    ] {
        let m = &summary["held_out"][name];
        if m.is_null() {
            continue;
        }
        out.push(format!(
            "    {label:<17} fail precision {} · failure recall {} · pass when \"pass\" {}",
            cell(&m["fail_precision"]).trim(),
            cell(&m["failure_recall"]).trim(),
            cell(&m["pass_when_pass"]).trim()
        ));
    }
    let stated = &summary["verdict"]["stated"];
    if !stated.is_null() {
        out.push(format!(
            "    The verdict states its held-out precision: fail {}/{}, pass {}/{}.",
            stated["fail"][0], stated["fail"][1], stated["pass"][0], stated["pass"][1]
        ));
    }
    out
}

/// `gym coder truth`.
///
/// # Errors
///
/// Returns a message for a bad argument or an unreadable summary.
pub fn command(args: &[String], out: &mut impl std::io::Write) -> Result<i32, String> {
    let mut dir = default_dir();
    let mut set = "held-out".to_string();
    let mut family = None;
    let mut within = false;
    let mut json_output = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = |name: &str| {
            iter.next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match arg.as_str() {
            "help" | "--help" | "-h" => {
                writeln!(out, "{HELP}").map_err(|e| e.to_string())?;
                return Ok(0);
            }
            "--json" => json_output = true,
            "--within" => within = true,
            "--dir" => dir = Some(value("--dir")?.into()),
            "--set" => set = value("--set")?,
            "--family" => family = Some(value("--family")?),
            other => return Err(format!("unknown option {other}\n\n{HELP}")),
        }
    }
    if set_key(&set).is_none() {
        return Err(format!(
            "--set must be held-out, calibration, or all, not {set}"
        ));
    }
    let dir = dir.ok_or("no --dir and no HOME")?;
    let summary = load(&dir)?;
    if json_output {
        let value = json!({ "schema": VIEW_SCHEMA, "dir": dir, "summary": summary });
        serde_json::to_writer_pretty(&mut *out, &value).map_err(|e| e.to_string())?;
        writeln!(out).map_err(|e| e.to_string())?;
    } else {
        for line in lines(&summary, &set, family.as_deref(), within) {
            writeln!(out, "{line}").map_err(|e| e.to_string())?;
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rate(k: u64, n: u64) -> Value {
        if n == 0 {
            return json!({ "k": k, "n": n, "p": null, "low": null, "high": null });
        }
        let p = k as f64 / n as f64;
        json!({ "k": k, "n": n, "p": p, "low": (p - 0.1).max(0.0), "high": (p + 0.1).min(1.0) })
    }

    fn measure(
        signal: &str,
        family: &str,
        fp: (u64, u64),
        rec: (u64, u64),
        pass: (u64, u64),
    ) -> Value {
        json!({
            "signal": signal, "family": family, "meaning": "",
            "fail_precision": rate(fp.0, fp.1), "failure_recall": rate(rec.0, rec.1),
            "pass_when_pass": rate(pass.0, pass.1), "pass_when_not_fail": rate(0, 0),
            "within_fail_precision": rate(fp.0, fp.1), "within_failure_recall": rate(rec.0, 40),
            "separates": fp.0 * 2 > fp.1,
        })
    }

    fn summary() -> Value {
        let held = vec![
            measure("checks.final", "checks", (6, 11), (6, 60), (69, 97)),
            measure(
                "scenario.generic.parse",
                "scenario",
                (0, 0),
                (0, 60),
                (0, 0),
            ),
            measure("verdict.combined", "verdict", (22, 37), (22, 60), (11, 13)),
        ];
        json!({
            "schema": SCHEMA,
            "label_set": {
                "trials": 317, "passes": 195, "failures": 122, "tasks": 58,
                "calibration": { "trials": 132, "tasks": 26, "failures": 62 },
                "held_out": { "trials": 185, "tasks": 32, "failures": 60 },
            },
            "report_answers": 317,
            "verdict": { "stated": { "fail": [22, 37], "pass": [11, 13] } },
            "signals": { "held_out": held.clone(), "calibration": held.clone(), "all": held },
            "held_out": { "verdict": held_measure(22, 37), "todays_checks": held_measure(6, 11) },
        })
    }

    fn held_measure(k: u64, n: u64) -> Value {
        measure("x", "x", (k, n), (k, 60), (1, 1))
    }

    #[test]
    fn the_view_shows_each_signal_and_the_verdict_against_todays_checks() {
        let text = lines(&summary(), "held-out", None, false).join("\n");
        assert!(text.contains("317 graded trials (195 passes, 122 failures) on 58 tasks"));
        assert!(text.contains("checks.final"));
        assert!(text.contains(" 22/37   59%"));
        // A signal that never spoke is left out.
        assert!(!text.contains("scenario.generic.parse"));
        assert!(
            text.contains("The verdict states its held-out precision: fail 22/37, pass 11/13.")
        );
        let only = lines(&summary(), "held-out", Some("verdict"), false).join("\n");
        assert!(!only.contains("checks.final  "));
        let within = lines(&summary(), "held-out", None, true).join("\n");
        assert!(within.contains("within task"));
    }

    #[test]
    fn the_command_reads_the_summary_and_prints_json() {
        let dir = std::env::temp_dir().join(format!("gym-truth-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("summary.json"), summary().to_string()).unwrap();
        let mut out = Vec::new();
        let code = command(
            &[
                "--dir".to_string(),
                dir.to_string_lossy().into_owned(),
                "--json".to_string(),
            ],
            &mut out,
        )
        .unwrap();
        assert_eq!(code, 0);
        let value: Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(value["schema"], VIEW_SCHEMA);
        assert!(command(&["--set".to_string(), "nope".to_string()], &mut Vec::new()).is_err());
        let _ = std::fs::remove_dir_all(&dir);
        assert!(load(Path::new("/nonexistent-truth")).is_err());
    }
}
