//! The nightly refit (#10387): read the joined decision readings
//! ([`super::decisions`]), and for each delegation setting propose the
//! threshold that best separates its outcome label on a fit split, then
//! adopt it only when it beats the setting's default on a held-out split.
//! Adopted values go to a versioned file serving reads
//! ([`coder_delegate::calibration`]); a fit that fails its gate is reported
//! and never adopted, and an earlier adoption stays until a new fit passes.
//!
//! Router thresholds are reported here but adopted by the router's own
//! policy (#10386), never by this file.

use super::decisions::Row;
use coder_delegate::calibration::{self, Adopted, File, SCHEMA};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Fewest joined samples on the fit split before a setting is refit.
pub const MIN_FIT: usize = 40;
/// Fewest joined samples on the held-out split.
pub const MIN_HELD_OUT: usize = 20;
/// How much better than the default a proposal must do on held-out
/// samples to be adopted.
pub const MARGIN: f64 = 0.02;
/// The active-work time past which a task counts as substantial: the
/// `recipe.hard` question's own line ("more than 10 minutes").
pub const HARD_MS: u64 = 600_000;

/// The delegation setting a gate site reads, and what its outcome label is.
#[must_use]
pub fn setting_for(site: &str) -> Option<(&'static str, Label)> {
    Some(match site {
        "recipe.hard" => ("recipe.hard", Label::Substantial),
        "recipe.check_keep" => ("recipe.check_keep", Label::RunPass),
        "terminal.asks_only" => ("terminal.asks_only", Label::RunPass),
        "system.select" => ("system.select", Label::RunPass),
        "issue_turn.plain" => ("issue_turn.plain", Label::RunPass),
        "evidence.setup" | "evidence.probe_keep" | "evidence.relevance" => {
            ("evidence.yes", Label::RunPass)
        }
        _ => return None,
    })
}

/// What a setting's decision is scored against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Label {
    /// The run's independent check passed. A proxy, not question-specific
    /// correctness.
    RunPass,
    /// The run took more than [`HARD_MS`] of wall time: the hard
    /// question's definition of substantial work.
    Substantial,
}

impl Label {
    fn name(self) -> &'static str {
        match self {
            Self::RunPass => "independent_run_pass_proxy",
            Self::Substantial => "wall_time_over_10_minutes",
        }
    }

    fn of(self, row: &Row) -> Option<bool> {
        match self {
            Self::RunPass => row.independent_pass,
            Self::Substantial => row.wall_ms.map(|ms| ms > HARD_MS),
        }
    }
}

/// A stable split: about a third of samples, by outcome identity, are
/// held out, so a sample never moves between splits across nights.
fn held_out(row: &Row) -> bool {
    use sha2::Digest as _;
    let key = format!(
        "{}|{}|{}",
        row.reading.outcome_key,
        row.task.as_deref().unwrap_or(""),
        row.request.as_deref().unwrap_or("")
    );
    sha2::Sha256::digest(key.as_bytes())[0] % 3 == 0
}

fn accuracy(samples: &[(f64, bool)], threshold: f64) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    samples
        .iter()
        .filter(|(p, label)| (*p >= threshold) == *label)
        .count() as f64
        / samples.len() as f64
}

/// The threshold with the best fit-split accuracy, nearest the default on
/// ties: candidates are midpoints between neighbouring probabilities.
fn propose(samples: &[(f64, bool)], default: f64) -> f64 {
    let mut ps: Vec<f64> = samples.iter().map(|(p, _)| *p).collect();
    ps.sort_by(f64::total_cmp);
    ps.dedup();
    let mut candidates = vec![default];
    candidates.extend(ps.windows(2).map(|w| (w[0] + w[1]) / 2.0));
    candidates.retain(|t| *t > 0.0 && *t < 1.0);
    let mut best = default;
    let mut best_acc = accuracy(samples, default);
    for t in candidates {
        let acc = accuracy(samples, t);
        if acc > best_acc + 1e-12
            || ((acc - best_acc).abs() <= 1e-12 && (t - default).abs() < (best - default).abs())
        {
            best = t;
            best_acc = acc;
        }
    }
    best
}

/// One setting's refit: counts, the proposal, held-out accuracies, and
/// whether it is adopted and why.
fn refit_setting(
    name: &str,
    label: Label,
    default: f64,
    rows: &[&Row],
) -> (Value, Option<Adopted>) {
    let labelled: Vec<(&Row, f64, bool)> = rows
        .iter()
        .filter_map(|r| {
            let p = r.reading.raw_probability.as_f64()?;
            Some((*r, p, label.of(r)?))
        })
        .collect();
    let (held, fit): (Vec<_>, Vec<_>) = labelled.iter().partition(|(r, _, _)| held_out(r));
    let fit: Vec<(f64, bool)> = fit.iter().map(|(_, p, l)| (*p, *l)).collect();
    let held: Vec<(f64, bool)> = held.iter().map(|(_, p, l)| (*p, *l)).collect();
    let both = |s: &[(f64, bool)]| s.iter().any(|x| x.1) && s.iter().any(|x| !x.1);
    let unmeasured = coder_delegate::decision::UNMEASURED.contains(&name);
    let mut out = json!({
        "setting": name, "label": label.name(), "default": default,
        "n": rows.len(), "labelled_n": labelled.len(),
        "fit_n": fit.len(), "held_out_n": held.len(),
        "unmeasured_default": unmeasured,
    });
    let reason = if fit.len() < MIN_FIT || held.len() < MIN_HELD_OUT {
        Some(format!(
            "not enough data yet: needs {MIN_FIT} fit and {MIN_HELD_OUT} held-out labelled samples"
        ))
    } else if !both(&fit) || !both(&held) {
        Some("not enough data yet: both outcomes must appear in each split".into())
    } else {
        None
    };
    if let Some(reason) = reason {
        out["adopt"] = json!(false);
        out["reason"] = json!(reason);
        return (out, None);
    }
    let proposed = propose(&fit, default);
    let at_default = accuracy(&held, default);
    let at_proposed = accuracy(&held, proposed);
    let adopt = proposed != default && at_proposed >= at_default + MARGIN;
    out["proposed"] = json!(proposed);
    out["held_out_accuracy_default"] = json!(at_default);
    out["held_out_accuracy_proposed"] = json!(at_proposed);
    out["adopt"] = json!(adopt);
    out["reason"] = json!(if adopt {
        "beats the default on held-out samples".to_owned()
    } else if proposed == default {
        "the default is already the best fit".to_owned()
    } else {
        format!("held-out gain under {MARGIN}: kept the default")
    });
    let adopted = adopt.then(|| Adopted {
        threshold: proposed,
        default,
        fit_n: fit.len(),
        held_out_n: held.len(),
        held_out_accuracy_default: at_default,
        held_out_accuracy: at_proposed,
        label: label.name().into(),
    });
    (out, adopted)
}

/// The defaults the refit compares against, by setting name.
fn defaults() -> BTreeMap<&'static str, f64> {
    use coder_delegate::decision as d;
    [
        d::RECIPE_HARD,
        d::RECIPE_CHECK_KEEP,
        d::TERMINAL_ASKS_ONLY,
        d::SYSTEM_SELECT,
        d::ISSUE_TURN_PLAIN,
        d::EVIDENCE_YES,
    ]
    .iter()
    .map(|s| (s.name, s.default.value()))
    .collect()
}

/// Refit every delegation setting from `rows`, merging adoptions into
/// `previous`. The report, and the file to write when anything changed.
#[must_use]
pub fn refit(rows: &[Row], previous: &File, now: &str) -> (Value, Option<File>) {
    let mut by_setting: BTreeMap<&str, (Label, Vec<&Row>)> = BTreeMap::new();
    let mut router: BTreeMap<String, usize> = BTreeMap::new();
    for row in rows {
        match setting_for(&row.reading.site) {
            Some((name, label)) => by_setting
                .entry(name)
                .or_insert((label, vec![]))
                .1
                .push(row),
            None => *router.entry(row.reading.site.clone()).or_default() += 1,
        }
    }
    let mut next = previous.clone();
    next.schema = SCHEMA.into();
    let mut settings = Vec::new();
    for (name, default) in defaults() {
        let (label, rows) = by_setting
            .get(name)
            .map(|(l, r)| (*l, r.clone()))
            .unwrap_or((setting_for(name).map_or(Label::RunPass, |s| s.1), vec![]));
        let (mut report, adopted) = refit_setting(name, label, default, &rows);
        if let Some(adopted) = adopted {
            next.settings.insert(name.to_owned(), adopted);
        } else if let Some(kept) = previous.settings.get(name) {
            report["kept_adopted"] = json!(kept.threshold);
        }
        report["in_effect"] = json!(next.threshold(name).unwrap_or(default));
        settings.push(report);
    }
    let changed = next.settings != previous.settings;
    if changed {
        next.version = previous.version + 1;
        next.fitted_at = now.to_owned();
    }
    let report = json!({
        "schema": "openagents.efficiency.refit.v1",
        "min_fit": MIN_FIT, "min_held_out": MIN_HELD_OUT, "margin": MARGIN,
        "settings": settings,
        "router_sites": router,
        "router_note": "Router thresholds are refit by the router's policy (#10386), not adopted here.",
        "version": next.version, "changed": changed,
    });
    (report, changed.then_some(next))
}

/// Where adopted-settings versions are written.
#[must_use]
pub fn dir() -> Option<PathBuf> {
    calibration::path().and_then(|p| p.parent().map(Path::to_path_buf))
}

/// Write `file` as `settings-vN.json` beside the current file, then make it
/// current atomically.
///
/// # Errors
/// The directory or files could not be written.
pub fn write(file: &File) -> Result<PathBuf, String> {
    let current = calibration::path().ok_or("no home directory for the calibration file")?;
    let dir = current
        .parent()
        .ok_or("the calibration file has no directory")?;
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let text = serde_json::to_string_pretty(file).map_err(|e| e.to_string())?;
    let versioned = dir.join(format!("settings-v{}.json", file.version));
    std::fs::write(&versioned, &text)
        .map_err(|e| format!("cannot write {}: {e}", versioned.display()))?;
    let tmp = dir.join(".current.json.tmp");
    std::fs::write(&tmp, &text).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &current)
        .map_err(|e| format!("cannot replace {}: {e}", current.display()))?;
    Ok(versioned)
}

/// One line for the background view.
#[must_use]
pub fn line(report: &Value) -> String {
    let settings = report["settings"].as_array().cloned().unwrap_or_default();
    let adopted: Vec<String> = settings
        .iter()
        .filter(|s| s["adopt"] == json!(true))
        .map(|s| {
            format!(
                "{} {:.2}",
                s["setting"].as_str().unwrap_or(""),
                s["proposed"].as_f64().unwrap_or(0.0)
            )
        })
        .collect();
    let waiting = settings
        .iter()
        .filter(|s| {
            s["reason"]
                .as_str()
                .is_some_and(|r| r.starts_with("not enough data"))
        })
        .count();
    if adopted.is_empty() {
        format!(
            "No threshold changed; {waiting} of {} settings need more joined outcomes.",
            settings.len()
        )
    } else {
        format!(
            "Adopted {} (version {}); {waiting} settings need more joined outcomes.",
            adopted.join(", "),
            report["version"]
        )
    }
}

/// The plain-words report.
#[must_use]
pub fn text(report: &Value) -> String {
    let mut lines = vec![
        "Refit · decision thresholds from joined run outcomes".to_owned(),
        format!(
            "A setting is refit with at least {} fit and {} held-out labelled samples, and adopted only when it beats its default on the held-out samples by {}.",
            report["min_fit"], report["min_held_out"], report["margin"]
        ),
    ];
    for s in report["settings"].as_array().into_iter().flatten() {
        let flag = if s["unmeasured_default"] == json!(true) {
            " (default unmeasured)"
        } else {
            ""
        };
        let mut line = format!(
            "{}{flag} · default {} · in effect {} · n={} labelled={} · {}",
            s["setting"].as_str().unwrap_or(""),
            s["default"],
            s["in_effect"],
            s["n"],
            s["labelled_n"],
            s["reason"].as_str().unwrap_or("")
        );
        if let (Some(p), Some(d), Some(a)) = (
            s["proposed"].as_f64(),
            s["held_out_accuracy_default"].as_f64(),
            s["held_out_accuracy_proposed"].as_f64(),
        ) {
            line.push_str(&format!(
                " · proposed {p:.3} · held-out accuracy {:.1}% → {:.1}%",
                100.0 * d,
                100.0 * a
            ));
        }
        lines.push(line);
    }
    let router = report["router_sites"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    if !router.is_empty() {
        lines.push(format!(
            "Router gates seen: {} — {}",
            router
                .iter()
                .map(|(k, v)| format!("{k} ({v})"))
                .collect::<Vec<_>>()
                .join(", "),
            report["router_note"].as_str().unwrap_or("")
        ));
    }
    lines.join("\n")
}

/// The published summary openagents.com/efficiency shows, as committed
/// by `openagents efficiency refit --export bench/efficiency/decisions/latest.json`.
pub const PUBLISHED: &str = include_str!("../../../../bench/efficiency/decisions/latest.json");

/// The published summary, parsed; `Null` if the committed file is malformed.
#[must_use]
pub fn published() -> Value {
    serde_json::from_str(PUBLISHED).unwrap_or(Value::Null)
}

/// The public summary openagents.com/efficiency shows: per question n,
/// accuracy at threshold, and reliability, without request or task text.
#[must_use]
pub fn public(decisions: &Value, refit: &Value) -> Value {
    let questions: Vec<Value> = decisions["questions"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|q| {
            json!({"question": q["question"], "site": q["site"], "threshold": q["threshold"],
                "n": q["n"], "checked_n": q["checked_n"],
                "accuracy_at_threshold": q["accuracy_at_threshold"],
                "reliability": q["reliability"]})
        })
        .collect();
    json!({"schema": "openagents.efficiency.decisions.public.v1",
        "label": decisions["label"], "questions": questions,
        "settings": refit["settings"], "min_fit": refit["min_fit"],
        "min_held_out": refit["min_held_out"]})
}

#[cfg(test)]
mod tests {
    use super::*;
    use route_contract::decision::DecisionReading;

    fn row(site: &str, p: f64, key: usize, pass: Option<bool>, wall_ms: Option<u64>) -> Row {
        let mut reading = DecisionReading::new("q", site, "jev-test", p, 0.8, p >= 0.8).unwrap();
        reading.outcome_key = format!("k{key}");
        Row {
            reading,
            request: None,
            task: Some(format!("t{key}")),
            independent_pass: pass,
            cost_microusd: None,
            wall_ms,
        }
    }

    #[test]
    fn too_few_samples_adopt_nothing_and_say_so() {
        let rows: Vec<Row> = (0..10)
            .map(|i| row("recipe.hard", 0.9, i, Some(true), Some(700_000)))
            .collect();
        let (report, file) = refit(&rows, &File::default(), "now");
        assert!(file.is_none());
        let hard = report["settings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["setting"] == "recipe.hard")
            .unwrap();
        assert_eq!(hard["adopt"], false);
        assert!(
            hard["reason"]
                .as_str()
                .unwrap()
                .starts_with("not enough data")
        );
        assert!(text(&report).contains("recipe.hard"));
        assert!(line(&report).starts_with("No threshold changed"));
    }

    #[test]
    fn a_separating_fit_that_beats_the_default_held_out_is_adopted_and_versioned() {
        // Tasks over 10 minutes read 0.6–0.7; short ones 0.1–0.3. The
        // default 0.8 calls every long task easy; ~0.45 separates them.
        let rows: Vec<Row> = (0..300)
            .map(|i| {
                let long = i % 2 == 0;
                let p = if long {
                    0.6 + (i % 10) as f64 / 100.0
                } else {
                    0.1 + (i % 20) as f64 / 100.0
                };
                row(
                    "recipe.hard",
                    p,
                    i,
                    Some(true),
                    Some(if long { 900_000 } else { 60_000 }),
                )
            })
            .collect();
        let (report, file) = refit(&rows, &File::default(), "2026-10-03T04:30:00Z");
        let file = file.expect("adopted");
        assert_eq!(file.version, 1);
        let t = file.threshold("recipe.hard").unwrap();
        assert!(t > 0.3 && t < 0.6, "{t}");
        let hard = report["settings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["setting"] == "recipe.hard")
            .unwrap();
        assert_eq!(hard["adopt"], true);
        assert!(
            hard["held_out_accuracy_proposed"].as_f64().unwrap()
                > hard["held_out_accuracy_default"].as_f64().unwrap()
        );
        // A second night with the same data changes nothing.
        let (_, again) = refit(&rows, &file, "later");
        assert!(again.is_none());
        assert!(line(&report).starts_with("Adopted recipe.hard"));
    }

    #[test]
    fn a_fit_that_does_not_beat_the_default_held_out_keeps_the_earlier_adoption() {
        // Noise: probability says nothing about the label.
        let rows: Vec<Row> = (0..300)
            .map(|i| {
                row(
                    "evidence.relevance",
                    (i % 97) as f64 / 100.0,
                    i,
                    Some(i % 3 == 0),
                    None,
                )
            })
            .collect();
        let mut previous = File {
            schema: SCHEMA.into(),
            version: 4,
            ..File::default()
        };
        previous.settings.insert(
            "evidence.yes".into(),
            Adopted {
                threshold: 0.55,
                default: 0.5,
                fit_n: 60,
                held_out_n: 30,
                held_out_accuracy_default: 0.6,
                held_out_accuracy: 0.7,
                label: "x".into(),
            },
        );
        let (report, file) = refit(&rows, &previous, "now");
        let ev = report["settings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["setting"] == "evidence.yes")
            .unwrap();
        if ev["adopt"] == false {
            assert!(file.is_none(), "nothing changed, nothing written");
            assert_eq!(ev["kept_adopted"], 0.55);
            assert_eq!(ev["in_effect"], 0.55);
        }
        assert_eq!(ev["unmeasured_default"], true);
    }

    #[test]
    fn the_published_summary_parses_and_names_every_setting() {
        let p = published();
        assert_eq!(p["schema"], "openagents.efficiency.decisions.public.v1");
        let names: Vec<_> = p["settings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["setting"].as_str().unwrap().to_owned())
            .collect();
        for name in defaults().keys() {
            assert!(names.iter().any(|n| n == name), "{name}");
        }
    }

    #[test]
    fn router_sites_are_reported_not_adopted() {
        let rows = vec![row("route.answer", 0.7, 1, Some(true), None)];
        let (report, file) = refit(&rows, &File::default(), "now");
        assert!(file.is_none());
        assert_eq!(report["router_sites"]["route.answer"], 1);
    }
}
