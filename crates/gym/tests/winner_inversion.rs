//! Whether a calibration map can change which option wins.
//!
//! Two documents on `main` reached opposite conclusions, so openagents#9438
//! settled it by enumeration rather than by reading the code again. Three
//! questions, and their answers are not the same:
//!
//! 1. **Can a rescale move the argmax at all?** Yes, and exactly when the
//!    calibrated probability falls below the runner-up's share of the
//!    redistributed remainder — which requires reading a raw signal at below
//!    one half.
//! 2. **Would the acceptance gate admit such a map?** Yes. No criterion in
//!    `probability-v1` reads the map's bins, and the accuracy criterion
//!    cannot see a moved argmax because the contract fixes the answer.
//! 3. **Does any map committed here reach it on any row committed here?**
//!    No. Over every record under `crates/lev/calibration/` and every scored
//!    row under `crates/gym/results/`, no argmax moves.
//!
//! The contract that closes the gap is in `crates/gym/src/calibrate.rs`: the
//! selected option is the estimator's argmax and a map rescales it rather
//! than replacing it. These tests hold every consumer to that.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use gym::calibrate::{Bin, Map, Metrics, Observation, Record, score};
use gym::eval::{fit_family, mapped_observations, observations};
use gym::gate::{Comparison, Verdict};
use gym::row::Row;
use indexmap::IndexMap;

fn distribution(pairs: &[(&str, f64)]) -> IndexMap<String, f64> {
    pairs.iter().map(|(key, value)| ((*key).to_string(), *value)).collect()
}

/// The largest option in a distribution, however it got there.
///
/// Deliberately not `calibrate::selected`: these tests ask what the rescaled
/// distribution's own argmax is, which is the quantity the contract says not
/// to read an answer from.
fn largest(map: &IndexMap<String, f64>) -> String {
    map.iter()
        .max_by(|left, right| left.1.total_cmp(right.1))
        .map(|(option, _)| option.clone())
        .expect("a non-empty distribution")
}

fn flat_map(fitted: f64) -> Map {
    Map {
        bins: vec![Bin { lo: 0.0, hi: 1.0, fitted, count: 1 }],
        base_rate: fitted,
        fitted_on: 1,
        by_band: BTreeMap::new(),
    }
}

fn committed_rows() -> Vec<(String, Row)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("results");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("the results directory reads")
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|path| path.extension().and_then(|extension| extension.to_str()) == Some("jsonl"))
        .collect();
    files.sort();
    let mut rows = Vec::new();
    for path in files {
        let name = path.file_name().expect("a file name").to_string_lossy().to_string();
        let text = std::fs::read_to_string(&path).expect("the file reads");
        for (index, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let value: serde_json::Value = serde_json::from_str(line).expect("a line parses");
            let schema = value.get("schema").and_then(serde_json::Value::as_str).unwrap_or("");
            if schema == gym::spread::DRAW_SCHEMA {
                // A block draw records the argmax and its top frequency, not
                // the distribution, so no map is ever applied to one.
                continue;
            }
            assert!(
                gym::store::known_row_schema(schema),
                "{name} line {}: unknown schema {schema}",
                index + 1
            );
            rows.push((name.clone(), serde_json::from_value::<Row>(value).expect("a row parses")));
        }
    }
    rows
}

fn committed_records() -> Vec<(PathBuf, Record)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .join("lev")
        .join("calibration");
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&root)
        .expect("the calibration directory reads")
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    let mut records = Vec::new();
    for dir in dirs {
        records.extend(Record::load_dir(&dir).expect("the records read"));
    }
    records
}

#[test]
fn a_rescale_moves_the_argmax_in_principle() {
    // The audit's construction at `a43033cd`, rebuilt from the crate's own
    // types. One observation at 0.8 that was wrong fits a single bin to
    // Jeffreys' (0 + 0.5) / (1 + 1) = 0.25.
    let map = Map::fit(&[Observation::new(0.8, false)], 1);
    let raw = distribution(&[("yes", 0.8), ("no", 0.2)]);
    let rescaled = map.apply_distribution(&raw);

    assert!((rescaled["yes"] - 0.25).abs() < 1e-12, "{rescaled:?}");
    assert!((rescaled["no"] - 0.75).abs() < 1e-12, "{rescaled:?}");
    assert_eq!(largest(&raw), "yes");
    assert_eq!(largest(&rescaled), "no", "the rescale moved the argmax");

    // And the option the answer is read from does not move with it.
    let (selected, top) = gym::calibrate::selected(&raw).expect("a selected option");
    assert_eq!(selected, "yes");
    assert!((top - 0.8).abs() < 1e-12);
}

#[test]
fn the_argmax_moves_exactly_when_the_calibrated_probability_falls_below_the_runner_ups_share() {
    // `rescale` gives the selected option `c` and hands each loser
    // `(1 - c) * v / rest`. The largest loser therefore lands at
    // `(1 - c) * m / rest`, and it passes the selected option exactly when
    // `c < m / (m + rest)`. Because `m <= rest`, that threshold never
    // exceeds one half: a map that reads every signal at or above 0.5 cannot
    // move an argmax whatever the distribution looks like, and `Map::fit`
    // produces a bin below 0.5 whenever fewer than half its observations
    // were right.
    let shapes = [
        distribution(&[("a", 0.8), ("b", 0.2)]),
        distribution(&[("a", 0.51), ("b", 0.49)]),
        distribution(&[("a", 1.0), ("b", 0.0)]),
        distribution(&[("a", 0.5), ("b", 0.3), ("c", 0.2)]),
        distribution(&[("a", 0.4), ("b", 0.35), ("c", 0.25)]),
        distribution(&[("a", 0.6), ("b", 0.4), ("c", 0.0)]),
        distribution(&[("a", 0.34), ("b", 0.33), ("c", 0.33)]),
        distribution(&[("a", 1.0), ("b", 0.0), ("c", 0.0)]),
        distribution(&[("a", 0.7), ("b", 0.1), ("c", 0.1), ("d", 0.1)]),
    ];

    let mut moves = 0;
    for shape in &shapes {
        let (selected, _) = gym::calibrate::selected(shape).expect("a selected option");
        let rest: f64 = shape.iter().filter(|(key, _)| *key != selected).map(|(_, v)| *v).sum();
        let runner_up = shape
            .iter()
            .filter(|(key, _)| *key != selected)
            .map(|(_, value)| *value)
            .fold(0.0_f64, f64::max);
        // With nothing left to share in proportion, `rescale` spreads the
        // remainder evenly and the threshold is one over the option count.
        let threshold = if rest > 0.0 {
            runner_up / (runner_up + rest)
        } else {
            1.0 / shape.len() as f64
        };
        assert!(threshold <= 0.5 + 1e-12, "the threshold cannot exceed one half: {threshold}");

        for step in 0..=100 {
            let calibrated = f64::from(step) / 100.0;
            let rescaled = flat_map(calibrated).apply_distribution(shape);
            let moved = largest(&rescaled) != selected;
            let tie = (calibrated - threshold).abs() < 1e-9;
            if moved {
                moves += 1;
                assert!(
                    calibrated <= 0.5 + 1e-9,
                    "the argmax moved at {calibrated}, above one half: {shape:?}"
                );
            }
            // On the threshold itself the selected option and the runner-up
            // land on the same number and `max_by` returns the last of the
            // equal maxima, so which option is largest depends on the order
            // the estimator listed them in rather than on any probability.
            // Everywhere else the predicate and the behaviour agree exactly.
            assert!(
                moved == (calibrated < threshold) || tie,
                "calibrated {calibrated}, threshold {threshold}, moved {moved}: {shape:?}"
            );
        }
    }
    assert!(moves > 0, "the sweep never moved an argmax, so it tested nothing");
}

#[test]
fn an_observation_reads_the_selected_options_probability_rather_than_the_largest() {
    // The defect openagents#9438 settled: reading the rescaled argmax pairs
    // a probability the door never claimed for this answer with this
    // answer's outcome. Here the estimator chose `yes` and was wrong, and
    // the map says an answer like this is right a quarter of the time. The
    // observation is (0.25, false) — a hedged answer that missed — not
    // (0.75, false), which would record a confident error at a number that
    // belongs to `no`.
    let map = Map::fit(&[Observation::new(0.8, false)], 1);
    let row = Row::new("audit", "digest", "one", "audit")
        .scored(distribution(&[("yes", 0.8), ("no", 0.2)]), false);

    let observed = mapped_observations(&[row], &map);
    assert_eq!(observed.len(), 1);
    assert!((observed[0].raw - 0.25).abs() < 1e-12, "{:?}", observed[0]);
    assert!(!observed[0].correct);
    assert_eq!(score(&observed).confident_errors, 0, "0.25 is not a confident claim");
}

#[test]
fn no_map_moves_the_accuracy_of_the_rows_it_scores() {
    // A consequence of the contract rather than a coincidence: the map
    // rescales a fixed answer, so the answer's outcome cannot change and
    // accuracy through a map is the raw accuracy. The gate's
    // `accuracy_does_not_fall` criterion is therefore vacuous by design, and
    // it is worth knowing that it is by design.
    let rows: Vec<Row> = [
        (distribution(&[("yes", 0.8), ("no", 0.2)]), false),
        (distribution(&[("yes", 0.9), ("no", 0.1)]), true),
        (distribution(&[("a", 0.5), ("b", 0.3), ("c", 0.2)]), true),
        (distribution(&[("a", 0.4), ("b", 0.35), ("c", 0.25)]), false),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (shape, correct))| {
        Row::new("audit", "digest", index.to_string(), "audit").scored(shape, correct)
    })
    .collect();

    let raw = score(&observations(&rows)).accuracy;
    for step in 0..=20 {
        let map = flat_map(f64::from(step) / 20.0);
        let mapped = score(&mapped_observations(&rows, &map)).accuracy;
        assert!((mapped - raw).abs() < 1e-12, "a map moved accuracy from {raw} to {mapped}");
    }
}

#[test]
fn the_probability_gate_admits_a_map_that_moves_the_argmax() {
    // Whether the gate is what stands between this repository and the
    // defect. It is not, and nothing in `probability-v1` reads the map's
    // bins, so this is a property of the rule rather than of the data.
    //
    // Forty binary items, the estimator unanimous on every one and right on
    // ten. The fitted table reads that signal at roughly a quarter, which is
    // both a large calibration win and low enough to move an argmax.
    let family: Vec<Row> = (0..80)
        .map(|index| {
            Row::new("audit", "digest", index.to_string(), "audit")
                .scored(distribution(&[("yes", 1.0), ("no", 0.0)]), index % 4 == 0)
        })
        .collect();
    let (fit_on, score_on) = family.split_at(40);

    let gate = gym::gate::load("probability-v1").expect("the committed probability gate");
    let fit = fit_family("audit", fit_on, score_on, &gate);

    let lowest = fit.map.bins.iter().map(|bin| bin.fitted).fold(f64::INFINITY, f64::min);
    println!(
        "the fitted table is {:?}\nraw {:?}\nthrough the map {:?}",
        fit.map.bins, fit.raw, fit.calibrated
    );
    assert!(lowest < 0.5, "the fitted table reads {lowest}, which cannot move an argmax");

    let raw = distribution(&[("yes", 1.0), ("no", 0.0)]);
    assert_eq!(
        largest(&fit.map.apply_distribution(&raw)),
        "no",
        "this map does not move the argmax, so it proves nothing"
    );

    assert_eq!(
        fit.outcome.verdict,
        Verdict::Passed,
        "the gate refused a map that moves the argmax, for {}",
        fit.verdict()
    );
    assert!(
        (fit.raw.accuracy - fit.calibrated.accuracy).abs() < 1e-12,
        "the gate's accuracy criterion sees no move, which is why it cannot refuse this"
    );
}

#[test]
fn no_committed_map_moves_the_argmax_of_any_committed_row() {
    let records = committed_records();
    let rows = committed_rows();
    assert!(!records.is_empty(), "no calibration records to check");
    assert!(!rows.is_empty(), "no result rows to check");

    let mut pairs = 0_usize;
    let mut moved: Vec<String> = Vec::new();
    for (path, record) in &records {
        for (file, row) in &rows {
            if !row.is_scored() {
                continue;
            }
            let Some(raw) = row.distribution.as_ref() else { continue };
            pairs += 1;
            let (selected, _) = gym::calibrate::selected(raw).expect("a selected option");
            let rescaled = record.map.apply_distribution(raw);
            if largest(&rescaled) != selected {
                moved.push(format!(
                    "{} on {file} {}: {raw:?} to {rescaled:?}",
                    path.display(),
                    row.item_id
                ));
            }
        }
    }
    println!("{pairs} map-and-row pairs over {} committed records", records.len());
    assert!(
        moved.is_empty(),
        "a committed map moves an argmax, so the record in docs/lev/calibration.md needs \
         re-reading:\n{}",
        moved.join("\n")
    );
}

#[test]
fn no_map_the_gate_admits_over_the_committed_store_moves_an_argmax() {
    // The operational question, refitted rather than assumed: for every
    // door and family the store holds on both sides of a partition, fit the
    // map the harness would fit, judge it with the committed rule, and check
    // the rows it was scored on.
    let gate = gym::gate::load("probability-v1").expect("the committed probability gate");

    let mut groups: BTreeMap<(String, String, String), Vec<Row>> = BTreeMap::new();
    for (file, row) in committed_rows() {
        groups.entry((file, row.door.clone(), row.family.clone())).or_default().push(row);
    }

    let mut judged = 0_usize;
    let mut admitted_and_moving: Vec<String> = Vec::new();
    for ((file, door, family), rows) in &groups {
        let fit_on: Vec<Row> =
            rows.iter().filter(|row| row.split == "calibration").cloned().collect();
        let score_on: Vec<Row> =
            rows.iter().filter(|row| row.split != "calibration").cloned().collect();
        if fit_on.is_empty() || score_on.is_empty() {
            continue;
        }
        judged += 1;
        let fit = fit_family(family, &fit_on, &score_on, &gate);
        let moves = score_on
            .iter()
            .filter(|row| row.is_scored())
            .filter(|row| {
                row.distribution.as_ref().is_some_and(|raw| {
                    let (selected, _) = gym::calibrate::selected(raw).expect("a selected option");
                    largest(&fit.map.apply_distribution(raw)) != selected
                })
            })
            .count();
        let lowest = fit.map.bins.iter().map(|bin| bin.fitted).fold(f64::INFINITY, f64::min);
        println!(
            "{file} {door} {family}: fitted on {}, scored {}, lowest bin {lowest:.3}, \
             {moves} argmax moves, {}",
            fit.map.fitted_on,
            observations(&score_on).len(),
            fit.verdict()
        );
        if fit.outcome.verdict == Verdict::Passed && moves > 0 {
            admitted_and_moving.push(format!("{file} {door} {family}: {moves} moves"));
        }
    }

    assert!(judged > 0, "no group had rows on both sides of a partition");
    assert!(
        admitted_and_moving.is_empty(),
        "a map the gate admits moves an argmax on rows it was scored on:\n{}",
        admitted_and_moving.join("\n")
    );
}

#[test]
fn a_gate_verdict_over_scores_alone_cannot_see_a_map_that_moves_the_argmax() {
    // Stated as a property of the rule, so that a later criterion which does
    // read the map has somewhere to announce itself. Two comparisons with
    // identical scores get identical verdicts, whatever the maps behind them
    // did, because `Comparison` carries scores and an item count and nothing
    // about the table.
    let gate = gym::gate::load("probability-v1").expect("the committed probability gate");
    let raw = Metrics {
        accuracy: 0.25,
        ece: 0.75,
        brier: 0.75,
        nll: 20.0,
        confident_errors: 30,
        items: 40,
    };
    let calibrated = Metrics {
        accuracy: 0.25,
        ece: 0.006,
        brier: 0.188,
        nll: 0.563,
        confident_errors: 0,
        items: 40,
    };
    let outcome = gate
        .judge(&Comparison::new("audit", raw.scores(), calibrated.scores()).fitted_on(40));
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert!(
        !outcome.criteria.iter().any(|criterion| criterion.name.contains("argmax")),
        "a criterion now reads the map; openagents#9438's record needs updating"
    );
}
