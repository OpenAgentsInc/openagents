//! The efficiency report (#10210): routed against raw delegation, from
//! recorded runs only.
//!
//! Three sources, each kept apart because each answers a different
//! question:
//!
//! - **Study rows**: the standing Gym study (#10162,
//!   `bench/efficiency/study.py`) and the #10209 shadow-baseline studies it
//!   grew from. The same pinned tasks run through raw Claude Code, raw
//!   Codex, and OpenAgents' routed paths, and an independent check decides
//!   every pass. Only rows of one study are compared with each other.
//! - **Shadow records**: a sample of this computer's own Coder runs, each
//!   run once more through the raw engine (`coder::task::shadow`), paired.
//! - **Route records**: every routed Coder run on this computer
//!   (`~/.openagents/routes`), with no raw side: pass rate, cost per
//!   checked outcome, and time to a checked result, by engine and class.
//!
//! Cost is never shown in normal app use; this report (`openagents
//! efficiency`, the terminal's `/efficiency`, and openagents.com/efficiency)
//! is where it is shown. Every figure is computed here from rows; nothing
//! is typed in by hand.

pub mod decisions;
pub mod refit;

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Value, json};

/// The report's JSON schema.
pub const SCHEMA: &str = "openagents.efficiency.v1";

/// How many resamples each bootstrap interval takes.
const RESAMPLES: usize = 2000;

/// The arm every study arm is compared with.
pub const BASELINE: &str = "raw-claude";

/// One published study: its rows, compiled in, and what it measured.
pub struct Published {
    pub name: &'static str,
    pub label: &'static str,
    /// Where its rows and write-up live in the repository.
    pub source: &'static str,
    pub rows: &'static str,
}

/// The studies whose rows are committed, oldest first. A new standing run
/// adds its `bench/efficiency/results/RUN.jsonl` here.
pub const PUBLISHED: &[Published] = &[
    Published {
        name: "2026-10-02-shadow-baseline",
        label: "#10209: routed Microcoder loop, recipe on and off, at feb4bc8270; lean Claude session (#10246) at 6b4dbe827d",
        source: "docs/cost/2026-10-02-shadow-baseline-measurement.md",
        rows: include_str!("../../../docs/cost/2026-10-02-shadow-baseline/collected.jsonl"),
    },
    Published {
        name: "2026-10-02-prompt-cache",
        label: "#10244: routed Claude loop after the prompt-cache fix, at 9513636c45",
        source: "docs/cost/2026-10-02-shadow-baseline-measurement.md#follow-up-the-prompt-cache-fix-10244",
        rows: include_str!("../../../docs/cost/2026-10-02-shadow-baseline/collected-10244.jsonl"),
    },
    Published {
        name: "2026-10-03",
        label: "Standing study (#10162): raw Claude Code, raw Codex, routed default, lean session, at 6b4dbe827d",
        source: "bench/efficiency/README.md",
        rows: include_str!("../../../bench/efficiency/results/2026-10-03.jsonl"),
    },
    Published {
        name: "2026-10-03b",
        label: "Standing study (#10162): raw Claude Code, raw Codex, routed default, lean session after the start-up cut (#10279), at 0565629714",
        source: "bench/efficiency/README.md#results-2026-10-03b",
        rows: include_str!("../../../bench/efficiency/results/2026-10-03b.jsonl"),
    },
];

/// One run, from any source.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub arm: String,
    /// `raw` or `routed`.
    pub mode: String,
    pub engine: Option<String>,
    pub task: Option<String>,
    pub class: String,
    /// Whether an independent check ran and decided the result.
    pub checked: bool,
    pub passed: bool,
    pub cost_usd: Option<f64>,
    pub wall_s: Option<f64>,
}

/// A point estimate and its 95% interval.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Estimate {
    pub point: f64,
    pub low: f64,
    pub high: f64,
}

impl Estimate {
    fn json(self) -> Value {
        json!({"point": self.point, "low": self.low, "high": self.high})
    }
}

/// Wilson's 95% interval for `k` of `n`.
#[must_use]
pub fn wilson(k: usize, n: usize) -> Option<Estimate> {
    if n == 0 {
        return None;
    }
    let (k, n, z) = (k as f64, n as f64, 1.96_f64);
    let p = k / n;
    let d = 1.0 + z * z / n;
    let c = (p + z * z / (2.0 * n)) / d;
    let h = z * (p * (1.0 - p) / n + z * z / (4.0 * n * n)).sqrt() / d;
    Some(Estimate {
        point: p,
        low: (c - h).max(0.0),
        high: (c + h).min(1.0),
    })
}

/// SplitMix64: a seeded generator, so every interval is reproducible.
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z % n as u64) as usize
    }
}

fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    let mid = v.len() / 2;
    Some(if v.len() % 2 == 0 {
        (v[mid - 1] + v[mid]) / 2.0
    } else {
        v[mid]
    })
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

/// The 2.5th and 97.5th percentiles of `samples`.
fn interval(point: f64, mut samples: Vec<f64>) -> Estimate {
    samples.retain(|s| s.is_finite());
    if samples.is_empty() {
        return Estimate {
            point,
            low: point,
            high: point,
        };
    }
    samples.sort_by(f64::total_cmp);
    let at = |q: f64| samples[((q * samples.len() as f64) as usize).min(samples.len() - 1)];
    Estimate {
        point,
        low: at(0.025),
        high: at(0.975),
    }
}

/// Rows grouped by task, so a resample keeps each task's share.
fn by_task(rows: &[&Row]) -> Vec<Vec<Row>> {
    let mut groups: BTreeMap<String, Vec<Row>> = BTreeMap::new();
    for row in rows {
        groups
            .entry(row.task.clone().unwrap_or_default())
            .or_default()
            .push((*row).clone());
    }
    groups.into_values().collect()
}

/// Resample runs within each task.
fn resample(groups: &[Vec<Row>], rng: &mut Rng) -> Vec<Row> {
    groups
        .iter()
        .flat_map(|g| {
            (0..g.len())
                .map(|_| g[rng.below(g.len())].clone())
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Everything spent, failures included, over the checked passes: what one
/// checked result cost. `None` with no pass or an unpriced run.
fn cost_per_pass(rows: &[Row]) -> Option<f64> {
    let passes = rows.iter().filter(|r| r.passed).count();
    let total: Option<f64> = rows.iter().map(|r| r.cost_usd).sum();
    (passes > 0).then_some(total? / passes as f64)
}

/// The median wall time of the runs that passed.
fn time_to_pass(rows: &[Row]) -> Option<f64> {
    let walls: Vec<f64> = rows
        .iter()
        .filter(|r| r.passed)
        .filter_map(|r| r.wall_s)
        .collect();
    median(&walls)
}

fn boot(rows: &[&Row], seed: u64, stat: fn(&[Row]) -> Option<f64>) -> Option<Estimate> {
    let owned: Vec<Row> = rows.iter().map(|r| (*r).clone()).collect();
    let point = stat(&owned)?;
    let groups = by_task(rows);
    let mut rng = Rng(seed);
    let samples = (0..RESAMPLES)
        .filter_map(|_| stat(&resample(&groups, &mut rng)))
        .collect();
    Some(interval(point, samples))
}

/// One arm's figures.
#[must_use]
pub fn arm_stats(arm: &str, rows: &[&Row]) -> Value {
    let n = rows.len();
    let checked = rows.iter().filter(|r| r.checked).count();
    let passed = rows.iter().filter(|r| r.passed).count();
    let priced: Vec<f64> = rows.iter().filter_map(|r| r.cost_usd).collect();
    let walls: Vec<f64> = rows.iter().filter_map(|r| r.wall_s).collect();
    let mut engines: Vec<String> = rows.iter().filter_map(|r| r.engine.clone()).collect();
    engines.sort();
    engines.dedup();
    json!({
        "arm": arm,
        "mode": rows.first().map(|r| r.mode.clone()),
        "engines": engines,
        "n": n,
        "checked": checked,
        "passed": passed,
        "pass_rate": wilson(passed, checked).map(Estimate::json),
        "cost_total_usd": (priced.len() == n).then(|| priced.iter().sum::<f64>()),
        "unpriced": n - priced.len(),
        "cost_per_checked_usd": boot(rows, 10210, cost_per_pass).map(Estimate::json),
        "time_to_checked_s": boot(rows, 10162, time_to_pass).map(Estimate::json),
        "median_wall_s": median(&walls),
    })
}

/// `arm` against `base`: the sum over the tasks both ran of per-task
/// means, as a ratio, with a bootstrap interval that resamples runs within
/// each task (the #10209 method).
#[must_use]
pub fn ratio(
    arm: &[&Row],
    base: &[&Row],
    key: fn(&Row) -> Option<f64>,
) -> Option<(Estimate, usize)> {
    let tasks = |rows: &[&Row]| {
        let mut out: BTreeMap<String, Vec<f64>> = BTreeMap::new();
        for r in rows {
            if let (Some(t), Some(v)) = (&r.task, key(r)) {
                out.entry(t.clone()).or_default().push(v);
            }
        }
        out
    };
    let (a, b) = (tasks(arm), tasks(base));
    let shared: Vec<&String> = a.keys().filter(|t| b.contains_key(*t)).collect();
    if shared.is_empty() {
        return None;
    }
    let total = |m: &BTreeMap<String, Vec<f64>>| shared.iter().map(|t| mean(&m[*t])).sum::<f64>();
    let point = total(&a) / total(&b);
    let mut rng = Rng(10209);
    let pick = |m: &BTreeMap<String, Vec<f64>>, rng: &mut Rng| {
        shared
            .iter()
            .map(|t| {
                let v = &m[*t];
                mean(
                    &(0..v.len())
                        .map(|_| v[rng.below(v.len())])
                        .collect::<Vec<_>>(),
                )
            })
            .sum::<f64>()
    };
    let samples = (0..RESAMPLES)
        .map(|_| {
            let x = pick(&a, &mut rng);
            x / pick(&b, &mut rng)
        })
        .collect();
    Some((interval(point, samples), shared.len()))
}

/// What a ratio's interval says, in words.
fn verdict(estimate: Estimate, what: &str) -> String {
    let pct = |r: f64| ((r - 1.0).abs() * 100.0).round();
    if estimate.high < 1.0 {
        format!(
            "{}% {}",
            pct(estimate.point),
            if what == "cost" { "cheaper" } else { "faster" }
        )
    } else if estimate.low > 1.0 {
        format!(
            "{}% {}",
            pct(estimate.point),
            if what == "cost" { "dearer" } else { "slower" }
        )
    } else {
        "no measurable difference".into()
    }
}

/// The arm's engine, from its row or its name.
fn arm_engine(arm: &str, row: &Value) -> Option<String> {
    row["engine"]
        .as_str()
        .map(str::to_owned)
        .or_else(|| row["engines"][0].as_str().map(str::to_owned))
        .or_else(|| {
            ["claude", "codex"]
                .into_iter()
                .find(|e| arm.contains(e) || (*e == "claude" && arm.contains("lean")))
                .map(str::to_owned)
        })
}

/// The class of a study task when its row does not name one.
fn study_class(task: &str) -> &'static str {
    match task {
        "mi-seekable" | "mi-one" | "bottle-etag" => "repository",
        _ => "terminal-bench",
    }
}

/// A study's JSONL rows (the harness's `collect` output).
#[must_use]
pub fn study_rows(text: &str) -> Vec<Row> {
    text.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|v| {
            let arm = v["arm"].as_str()?.to_owned();
            let task = v["task"].as_str()?.to_owned();
            Some(Row {
                mode: v["mode"].as_str().map_or_else(
                    || arm.split('-').next().unwrap_or("").to_owned(),
                    str::to_owned,
                ),
                engine: arm_engine(&arm, &v),
                class: v["task_class"]
                    .as_str()
                    .map_or_else(|| study_class(&task).to_owned(), str::to_owned),
                checked: true,
                passed: v["passed"].as_bool().unwrap_or(false),
                cost_usd: v["cost_usd"].as_f64(),
                wall_s: v["wall_s"].as_f64(),
                task: Some(task),
                arm,
            })
        })
        .collect()
}

/// One study's section of the report.
#[must_use]
pub fn study(name: &str, label: &str, source: &str, rows: &[Row]) -> Value {
    let mut arms: Vec<String> = Vec::new();
    for r in rows {
        if !arms.contains(&r.arm) {
            arms.push(r.arm.clone());
        }
    }
    // Raw arms first, then routed, each in the order first seen.
    arms.sort_by_key(|a| (!a.starts_with("raw"), a != BASELINE));
    let of = |arm: &str, class: Option<&str>| -> Vec<&Row> {
        rows.iter()
            .filter(|r| r.arm == arm && class.is_none_or(|c| r.class == c))
            .collect()
    };
    let mut classes: Vec<String> = rows.iter().map(|r| r.class.clone()).collect();
    classes.sort();
    classes.dedup();
    let compare = |arm: &str, baseline: &str| -> Option<Value> {
        let base = of(baseline, None);
        if base.is_empty() || arm == baseline {
            return None;
        }
        let rows = of(arm, None);
        let (cost, tasks) = ratio(&rows, &base, |r| r.cost_usd)?;
        let (time, _) = ratio(&rows, &base, |r| r.wall_s)?;
        Some(json!({
            "arm": arm,
            "baseline": baseline,
            "tasks": tasks,
            "cost_ratio": cost.json(),
            "cost": verdict(cost, "cost"),
            "time_ratio": time.json(),
            "time": verdict(time, "time"),
        }))
    };
    // Every arm against raw Claude Code, then each routed arm against the
    // raw arm of its own engine: what routing alone changed.
    let mut comparisons: Vec<Value> = arms.iter().filter_map(|a| compare(a, BASELINE)).collect();
    for a in &arms {
        let engines: Vec<&Row> = of(a, None);
        let engine = engines.first().and_then(|r| r.engine.clone());
        let single = engines.iter().all(|r| r.engine == engine);
        if let (Some(engine), true, false) = (engine, single, a.starts_with("raw")) {
            let raw = format!("raw-{engine}");
            if raw != BASELINE {
                comparisons.extend(compare(a, &raw));
            }
        }
    }
    json!({
        "name": name,
        "label": label,
        "source": source,
        "runs": rows.len(),
        "tasks": rows.iter().filter_map(|r| r.task.clone()).collect::<std::collections::BTreeSet<_>>().len(),
        "arms": arms.iter().map(|a| arm_stats(a, &of(a, None))).collect::<Vec<_>>(),
        "classes": classes.iter().map(|c| json!({
            "class": c,
            "arms": arms.iter().filter(|a| !of(a, Some(c)).is_empty()).map(|a| arm_stats(a, &of(a, Some(c)))).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "comparisons": comparisons,
    })
}

/// The shadow records (`coder::task::shadow`), paired routed against raw.
#[must_use]
pub fn shadow(records: &[Value]) -> Value {
    let side = |r: &Value, who: &str| -> Option<bool> {
        let checks = r["checks"].as_array()?;
        if checks.is_empty() {
            return None;
        }
        let results: Vec<Option<bool>> = checks.iter().map(|c| c[who].as_bool()).collect();
        results
            .iter()
            .all(Option::is_some)
            .then(|| results.iter().all(|p| *p == Some(true)))
    };
    let pairs: Vec<(f64, f64, f64, f64)> = records
        .iter()
        .filter_map(|r| {
            Some((
                r["routed"]["cost_usd"].as_f64()?,
                r["baseline"]["cost_usd"].as_f64()?,
                r["routed"]["wall_ms"].as_f64()? / 1000.0,
                r["baseline"]["wall_ms"].as_f64()? / 1000.0,
            ))
        })
        .collect();
    let paired = |pick: fn(&(f64, f64, f64, f64)) -> (f64, f64)| -> Option<Value> {
        if pairs.is_empty() {
            return None;
        }
        let sum = |v: &[(f64, f64, f64, f64)]| {
            let (a, b) = v
                .iter()
                .map(pick)
                .fold((0.0, 0.0), |s, p| (s.0 + p.0, s.1 + p.1));
            a / b
        };
        let mut rng = Rng(10209);
        let samples = (0..RESAMPLES)
            .map(|_| {
                sum(&(0..pairs.len())
                    .map(|_| pairs[rng.below(pairs.len())])
                    .collect::<Vec<_>>())
            })
            .collect();
        Some(interval(sum(&pairs), samples).json())
    };
    let count = |who: &str, want: bool| {
        records
            .iter()
            .filter(|r| side(r, who) == Some(want))
            .count()
    };
    json!({
        "records": records.len(),
        "pairs": pairs.len(),
        "cost_ratio": paired(|p| (p.0, p.1)),
        "time_ratio": paired(|p| (p.2, p.3)),
        "routed_checked": count("routed", true) + count("routed", false),
        "routed_passed": count("routed", true),
        "raw_checked": count("baseline", true) + count("baseline", false),
        "raw_passed": count("baseline", true),
    })
}

/// The class the delegate recipe gave a task, from its first turn's
/// trajectory in `store`.
fn recipe_class(store: &Path, task: &str) -> Option<String> {
    let text = std::fs::read_to_string(store.join(format!("{task}.1.atif.jsonl"))).ok()?;
    text.lines()
        .filter(|l| l.contains("\"delegate_recipe\""))
        .find_map(|l| {
            let v: Value = serde_json::from_str(l).ok()?;
            v["step"]["extensions"]["delegate_recipe"]["run"]["class"]["class"]
                .as_str()
                .map(str::to_owned)
        })
}

/// The router's own class of the plan (`repository_change`, ...).
fn plan_class(record: &route_contract::record::RouteRecord) -> Option<String> {
    let value = serde_json::to_value(&record.result).ok()?;
    value["plan"]["class"]["kind"].as_str().map(str::to_owned)
}

/// Every settled routed Coder run in the route journal beside `store`.
#[must_use]
pub fn route_rows(store: &Path) -> Vec<Row> {
    use route_contract::lifecycle::{CheckLabel, Lifecycle};
    let journal = openagents_chat::route::Journal::beside(store);
    let dir = store.with_file_name("routes");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut threads: Vec<String> = entries
        .flatten()
        .filter_map(|e| {
            e.file_name()
                .to_str()?
                .strip_suffix(".jsonl")
                .map(str::to_owned)
        })
        .collect();
    threads.sort();
    let mut rows = Vec::new();
    for thread in threads {
        for record in journal.records(&thread) {
            for run in &record.runs {
                let settled = matches!(
                    run.projection.state,
                    Lifecycle::Completed | Lifecycle::Failed
                );
                if !settled {
                    continue;
                }
                let check = run.projection.check;
                rows.push(Row {
                    arm: "routed".into(),
                    mode: "routed".into(),
                    engine: run.engine.clone(),
                    task: Some(run.task.clone()),
                    class: recipe_class(store, &run.task)
                        .or_else(|| plan_class(&record))
                        .unwrap_or_else(|| "unclassed".into()),
                    checked: matches!(check, CheckLabel::Verified | CheckLabel::CheckFailed),
                    passed: check == CheckLabel::Verified,
                    cost_usd: run.cost_microusd.map(|m| m as f64 / 1e6),
                    wall_s: run.wall_ms.map(|m| m as f64 / 1000.0),
                });
            }
        }
    }
    rows
}

/// This computer's routed runs, by engine and class.
#[must_use]
pub fn runs(rows: &[Row]) -> Value {
    let mut keys: Vec<(String, String)> = rows
        .iter()
        .map(|r| {
            (
                r.engine
                    .clone()
                    .unwrap_or_else(|| "unknown (legacy engine not recorded)".into()),
                r.class.clone(),
            )
        })
        .collect();
    keys.sort();
    keys.dedup();
    let groups: Vec<Value> = keys
        .iter()
        .map(|(engine, class)| {
            let of: Vec<&Row> = rows
                .iter()
                .filter(|r| {
                    r.engine
                        .as_deref()
                        .unwrap_or("unknown (legacy engine not recorded)")
                        == engine
                        && &r.class == class
                })
                .collect();
            let mut stats = arm_stats(&format!("{engine} · {class}"), &of);
            stats["engine"] = json!(engine);
            stats["class"] = json!(class);
            stats
        })
        .collect();
    json!({"runs": rows.len(), "unchecked": rows.iter().filter(|r| !r.checked).count(), "groups": groups})
}

/// The published studies, then any extra ones, as report sections.
#[must_use]
pub fn studies(extra: &[(String, String)]) -> Vec<Value> {
    PUBLISHED
        .iter()
        .map(|p| study(p.name, p.label, p.source, &study_rows(p.rows)))
        .chain(extra.iter().map(|(name, text)| {
            study(
                name,
                "Study rows given on the command line",
                name,
                &study_rows(text),
            )
        }))
        .filter(|s| s["runs"].as_u64() > Some(0))
        .collect()
}

/// The whole report: the studies, the shadow pairs, and the routed runs.
#[must_use]
pub fn report(studies: &[Value], shadow_records: &[Value], route_rows: &[Row]) -> Value {
    json!({
        "schema": SCHEMA,
        "studies": studies,
        "shadow": shadow(shadow_records),
        "runs": runs(route_rows),
        "findings": findings(&studies),
    })
}

/// An arm as people read it.
#[must_use]
pub fn arm_label(arm: &str) -> &str {
    match arm {
        "raw-claude" => "raw Claude Code",
        "raw-codex" => "raw Codex",
        "routed-default" => "routed default (Codex, recipe on)",
        "routed-lean" | "routed-claude-lean" => "lean Claude Code session",
        "routed-claude" | "routed-claude-on" => "routed Claude loop",
        "routed-claude-off" => "routed Claude loop, recipe off",
        "routed-codex" | "routed-codex-on" => "routed Codex",
        "routed-codex-off" => "routed Codex, recipe off",
        other => other,
    }
}

/// The latest study's comparisons in words, wins and losses alike.
#[must_use]
pub fn findings(studies: &[Value]) -> Vec<String> {
    let Some(latest) = studies.last() else {
        return vec!["No study rows yet.".into()];
    };
    let mut out = Vec::new();
    // The headline: the shipped default against raw Claude Code, its time
    // said as plainly as its cost.
    if let Some(c) = latest["comparisons"].as_array().and_then(|c| {
        c.iter()
            .find(|c| c["arm"] == "routed-default" && c["baseline"] == BASELINE)
    }) {
        let (cost, time) = (&c["cost_ratio"], &c["time_ratio"]);
        let passes = |arm: &str| {
            latest["arms"]
                .as_array()
                .and_then(|a| a.iter().find(|a| a["arm"] == arm))
                .map_or_else(String::new, |a| format!("{}/{}", a["passed"], a["checked"]))
        };
        let cost_words = if cost["high"].as_f64().is_some_and(|h| h < 1.0) {
            format!("was {}", c["cost"].as_str().unwrap_or(""))
        } else if cost["low"].as_f64().is_some_and(|l| l > 1.0) {
            format!("was {}", c["cost"].as_str().unwrap_or(""))
        } else {
            "cost about the same".into()
        };
        let time_words = if time["low"].as_f64().is_some_and(|l| l > 1.0) {
            format!(
                "took {} longer, so routing does not win on time yet",
                c["time"].as_str().unwrap_or("").replace(" slower", "")
            )
        } else if time["high"].as_f64().is_some_and(|h| h < 1.0) {
            format!("was {}", c["time"].as_str().unwrap_or(""))
        } else {
            "took about as long".into()
        };
        out.push(format!(
            "Headline: against raw Claude Code, the routed default {cost_words} and {time_words} \
             (passed {} against {}).",
            passes("routed-default"),
            passes(BASELINE),
        ));
    }
    for c in latest["comparisons"].as_array().into_iter().flatten() {
        let arm = c["arm"].as_str().unwrap_or("");
        let r = |k: &str| {
            format!(
                "{:.2}× ({:.2}–{:.2})",
                c[k]["point"].as_f64().unwrap_or(f64::NAN),
                c[k]["low"].as_f64().unwrap_or(f64::NAN),
                c[k]["high"].as_f64().unwrap_or(f64::NAN)
            )
        };
        out.push(format!(
            "{} against {} on {} tasks: cost {} — {}; time {} — {}.",
            arm_label(arm),
            arm_label(c["baseline"].as_str().unwrap_or(BASELINE)),
            c["tasks"],
            r("cost_ratio"),
            c["cost"].as_str().unwrap_or(""),
            r("time_ratio"),
            c["time"].as_str().unwrap_or(""),
        ));
    }
    out
}

fn est(v: &Value, f: fn(f64) -> String) -> String {
    match (v["point"].as_f64(), v["low"].as_f64(), v["high"].as_f64()) {
        (Some(p), Some(l), Some(h)) => format!("{} ({}–{})", f(p), f(l), f(h)),
        _ => "–".into(),
    }
}

/// One arm as one line: passes, cost per checked result, time to it.
#[must_use]
pub fn arm_line(a: &Value) -> String {
    if a["checked"].as_u64() == Some(0) {
        return "no independent check · – per checked result · – to a checked result".into();
    }
    format!(
        "{}/{} passed · {} per checked result · {} to a checked result",
        a["passed"],
        a["checked"],
        est(&a["cost_per_checked_usd"], |x| format!("${x:.3}")),
        est(&a["time_to_checked_s"], |x| format!("{x:.0} s")),
    )
}

/// Explain local evidence without equating executor completion with a checked pass.
pub const LOCAL_EVIDENCE_NOTES: &[&str] = &[
    "Completed executor runs without independent checks are unchecked, not checked passes.",
    "Pass counts include historical independent check verdicts, not executor exits or agent-reported test results.",
    "Unknown engines are legacy records with no engine recorded; journals are retained without guessing an engine.",
];

/// The report as text.
#[must_use]
pub fn text(report: &Value, all: bool) -> String {
    let mut out = Vec::new();
    let studies = report["studies"].as_array().cloned().unwrap_or_default();
    let shown: Vec<&Value> = if all {
        studies.iter().collect()
    } else {
        studies.last().into_iter().collect()
    };
    for s in shown {
        out.push(format!(
            "Study {} ({} runs on {} tasks): {}",
            s["name"].as_str().unwrap_or(""),
            s["runs"],
            s["tasks"],
            s["label"].as_str().unwrap_or("")
        ));
        for a in s["arms"].as_array().into_iter().flatten() {
            out.push(format!(
                "  {:<34} {}{}",
                arm_label(a["arm"].as_str().unwrap_or("")),
                arm_line(a),
                a["cost_total_usd"]
                    .as_f64()
                    .map_or_else(String::new, |c| format!(" · ${c:.2} in all")),
            ));
        }
        for c in s["classes"].as_array().into_iter().flatten() {
            out.push(format!("  by class: {}", c["class"].as_str().unwrap_or("")));
            for a in c["arms"].as_array().into_iter().flatten() {
                out.push(format!(
                    "    {:<32} {}",
                    arm_label(a["arm"].as_str().unwrap_or("")),
                    arm_line(a)
                ));
            }
        }
        out.push(format!("  source: {}", s["source"].as_str().unwrap_or("")));
    }
    if !all && studies.len() > 1 {
        out.push(format!(
            "({} earlier studies: openagents efficiency --all)",
            studies.len() - 1
        ));
    }
    out.push("Findings (95% intervals; a ratio below 1 favors the arm):".into());
    for f in report["findings"].as_array().into_iter().flatten() {
        out.push(format!("  - {}", f.as_str().unwrap_or("")));
    }
    let sh = &report["shadow"];
    if sh["pairs"].as_u64().unwrap_or(0) > 0 {
        out.push(format!(
            "Shadow baselines on this computer: {} pairs; cost routed/raw {}; time {}; checks passed routed {}/{}, raw {}/{}",
            sh["pairs"],
            est(&sh["cost_ratio"], |x| format!("{x:.2}×")),
            est(&sh["time_ratio"], |x| format!("{x:.2}×")),
            sh["routed_passed"], sh["routed_checked"], sh["raw_passed"], sh["raw_checked"],
        ));
    } else {
        out.push("Shadow baselines on this computer: none yet (turn them on with `openagents settings set coder.shadow 10`).".into());
    }
    let runs = &report["runs"];
    out.push(format!(
        "Routed runs on this computer: {} settled, {} with no independent check",
        runs["runs"], runs["unchecked"]
    ));
    for g in runs["groups"].as_array().into_iter().flatten() {
        out.push(format!(
            "  {:<34} {} run{} · {}{}",
            g["arm"].as_str().unwrap_or(""),
            g["n"],
            if g["n"].as_u64() == Some(1) { "" } else { "s" },
            arm_line(g),
            g["cost_total_usd"]
                .as_f64()
                .map_or_else(String::new, |c| format!(" · ${c:.2} in all")),
        ));
    }
    out.extend(LOCAL_EVIDENCE_NOTES.iter().map(|line| (*line).to_owned()));
    out.join("\n")
}

#[cfg(test)]
#[path = "efficiency_tests.rs"]
mod tests;
