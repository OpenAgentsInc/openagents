//! The #9376 floors and the mapped claims resting on them, re-derived from
//! the retained draws and rows.
//!
//! openagents#9419 settled the calibration contract — a map rescales a
//! fixed selected answer and never replaces it — and asked whether the
//! settlement moved any committed number. The block spreads in
//! `docs/lev/measurements/2026-09-19-calibration-variance.md` are computed
//! from `Draw::observation`, the raw signal: no map is applied on that
//! path, so the floors could not have moved. These tests check that rather
//! than repeat it: they recompute the published per-block values, the
//! spreads, the two-sigma comparison bounds derived from them, the mapped
//! claims those floors carry, and all six committed calibration records,
//! from the committed artifacts and nothing else.
//!
//! The claims covered are exactly the ones those artifacts can carry: the
//! calibration-variance record's spreads and comparison bounds, the mapped
//! numbers it prints for `lev-base` and `lev-band`, and the records under
//! `crates/lev/calibration/`. The `support-v1` numbers that record quotes
//! from earlier runs are out of scope — the draws and rows for them are not
//! retained here, so they are recorded history and are labelled
//! unverifiable in the inventory rather than asserted.
//!
//! `docs/gym/measurements/2026-09-20-raw-floors-and-mapped-claims.md` is
//! the claim inventory this feeds. Published values are asserted at the
//! precision the record prints them: three decimals for block values,
//! four for means and standard deviations. Stored calibration records are
//! compared field by field: every count, the fitted map, the verdicts, and
//! the provenance must be identical, while the float metrics are allowed
//! `SUM_TOLERANCE` — see its comment for why a stored number can differ
//! from a recomputed one by a representable step without meaning anything
//! moved.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use gym::calibrate::{Map, Metrics, Observation, Record, score};
use gym::eval::{fit_family, mapped_observations};
use gym::row::Row;
use gym::spread::{DRAW_SCHEMA, Draws, Spread};
use gym::store::Store;
use serde_json::Value;

type MetricRead = fn(&Metrics) -> f64;

/// Absolute tolerance for historical floating-point metrics.
/// The observed differences are one representable step. Their cause is
/// unverified; this does not assert bit-exact reproduction. Counts, fitted
/// tables, provenance, and verdicts are checked exactly, and separate tests
/// verify selected-answer identity. Every nonzero delta is printed.
const SUM_TOLERANCE: f64 = 1e-15;

/// The block draws one results file holds, every line accounted for.
///
/// Nothing is silently skipped: a line that does not parse, or that parses
/// under a schema that is not a block draw, stops the file rather than
/// quietly shrinking the grid it claims to cover. Each draw is also checked
/// against the suite it names — the item exists, the split and family are
/// the suite's, and the label the draw records is the suite's truth —
/// because a floor derived from draws whose labels had drifted would be a
/// number about nothing.
fn draws_of_file(name: &str) -> Draws {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("results")
        .join(name);
    let text = std::fs::read_to_string(&path).expect("the results file reads");

    // The suite the draws name, read as raw JSON: `support-v2` predates the
    // typed `Partition` enum, so `Suite::load` cannot read it, but its
    // items still say what was asked and what the truth was.
    let suite_text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("suites")
            .join("support-v2.json"),
    )
    .expect("the named suite reads");
    let suite: Value = serde_json::from_str(&suite_text).expect("the suite parses");
    let items: std::collections::BTreeMap<&str, &Value> = suite["items"]
        .as_array()
        .expect("a suite lists items")
        .iter()
        .map(|item| (item["id"].as_str().expect("an item id"), item))
        .collect();

    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let where_at = format!("{name} line {}", index + 1);
        let value: Value = serde_json::from_str(line)
            .unwrap_or_else(|error| panic!("{where_at}: not JSON: {error}"));
        let schema = value.get("schema").and_then(Value::as_str).unwrap_or("");
        assert_eq!(schema, DRAW_SCHEMA, "{where_at}: unknown schema {schema}");
        let draw: gym::spread::Draw =
            serde_json::from_value(value).expect("a draw parses under its own schema");

        let item = items
            .get(draw.item.as_str())
            .unwrap_or_else(|| panic!("{where_at}: '{}' is not a support-v2 item", draw.item));
        assert_eq!(draw.suite, "support-v2", "{where_at}: another suite's draw");
        assert_eq!(
            draw.split,
            item["split"].as_str().expect("an item split"),
            "{where_at}: the item's split is not the suite's"
        );
        assert_eq!(
            draw.family,
            item["family"].as_str().expect("an item family"),
            "{where_at}: the item's family is not the suite's"
        );
        assert_eq!(
            draw.truth,
            item["truth"].as_str().expect("an item label"),
            "{where_at}: the recorded truth is not the suite's label"
        );
        assert_eq!(
            draw.correct,
            draw.choice == draw.truth,
            "{where_at}: `correct` is not about the draw's own choice"
        );
        assert!(
            draw.top > 0.0 && draw.top <= 1.0 && draw.samples > 0,
            "{where_at}: top {} over {} samples is not a frequency",
            draw.top,
            draw.samples
        );
    }
    let draws = Draws::parse(&text).expect("every line a draw");

    // The grid, exactly: every split is a rectangle of items the suite owns,
    // identical across blocks and across doors. `complete` checks a block
    // holds every item the other blocks of its split hold; comparing the
    // per-door item sets to the suite's checks the rectangle against what
    // was asked rather than against itself.
    for door in ["lev-base", "lev-band"] {
        for split in ["calibration", "evaluation"] {
            // The calibration side legitimately holds one block — it is
            // fitted on rather than reported over, so it needs no spread.
            if split == "evaluation" {
                draws
                    .is_a_spread(door, split)
                    .unwrap_or_else(|error| panic!("{door} {split}: {error}"));
            }
            draws
                .complete(door, split)
                .unwrap_or_else(|error| panic!("{door} {split}: {error}"));
            let expected: BTreeSet<&str> = items
                .iter()
                .filter(|(_, item)| item["split"].as_str() == Some(split))
                .map(|(id, _)| *id)
                .collect();
            for block in draws.blocks(door, split) {
                let held: BTreeSet<&str> = draws
                    .draws_of(door, split, None, block)
                    .iter()
                    .map(|draw| draw.item.as_str())
                    .collect();
                assert_eq!(
                    held, expected,
                    "{door} {split} block {block}: the grid is not the suite's"
                );
            }
        }
    }
    draws
}

/// Every evaluation row one results file holds, with the receipt chain
/// verified. A malformed line, an unknown schema, or a broken chain is an
/// error from the store itself — the same read `gym` makes.
fn rows_of_file(name: &str) -> Vec<Row> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("results")
        .join(name);
    let store = Store::at(&path);
    store
        .verified_rows()
        .expect("the chain holds and every line is a known schema")
        .into_iter()
        .map(|value| serde_json::from_value::<Row>(value).expect("a row parses"))
        .collect()
}

/// Check the rows of one file against the suite they claim to be about:
/// every row names a real item on the partition the suite gives it, each
/// door covers the open partitions exactly once, and each door's rows agree
/// on the question set, estimator, and identity they ran under.
fn assert_rows_cover_the_suite(rows: &[Row], suite: &gym::suite::Suite) {
    let items: std::collections::BTreeMap<&str, &gym::suite::Item> = suite
        .items
        .iter()
        .map(|item| (item.id.as_str(), item))
        .collect();
    let open: BTreeSet<&str> = items
        .values()
        .filter(|item| item.partition != gym::suite::Partition::Locked)
        .map(|item| item.id.as_str())
        .collect();

    let doors: BTreeSet<&str> = rows.iter().map(|row| row.door.as_str()).collect();
    assert!(!doors.is_empty(), "the file holds no rows");
    for door in doors {
        let mine: Vec<&Row> = rows.iter().filter(|row| row.door == door).collect();
        let covered: BTreeSet<&str> = mine.iter().map(|row| row.item_id.as_str()).collect();
        assert_eq!(
            covered, open,
            "{door}: the open partitions are not covered exactly"
        );
        assert_eq!(
            mine.len(),
            covered.len(),
            "{door}: an item is recorded twice — a second trial is a different seed block"
        );
        for row in &mine {
            assert_eq!(
                row.suite_digest, suite.digest,
                "{door} {}: another suite",
                row.item_id
            );
            let item = items.get(row.item_id.as_str()).unwrap_or_else(|| {
                panic!("{door}: '{}' is not a {} item", row.item_id, suite.name)
            });
            assert_eq!(row.family, item.family, "{door} {}", row.item_id);
            assert_eq!(
                row.split,
                item.partition.as_str(),
                "{door} {}: a row on the wrong partition would let a fitted-on number read as held-out",
                row.item_id
            );
            assert_eq!(
                row.label_source,
                item.evidence(),
                "{door} {}: the label's provenance moved between the suite and the row",
                row.item_id
            );
            row.check().unwrap_or_else(|error| {
                panic!(
                    "{door} {}: the row contradicts itself: {error}",
                    row.item_id
                )
            });
        }
        // One run's identity, stated once per door: the estimator, its seed
        // block, and the question set it was served must agree on every
        // row, or the rows are not one measurement.
        for key in [
            "estimator",
            "question_set",
            "question_digest",
            "door_identity",
        ] {
            let distinct: BTreeSet<String> = mine
                .iter()
                .map(|row| match key {
                    "estimator" => row.estimator.clone(),
                    "question_set" => row.question_set.clone().unwrap_or_default(),
                    "question_digest" => row.question_digest.clone().unwrap_or_default(),
                    _ => format!("{:?}", row.door_identity),
                })
                .collect();
            assert_eq!(
                distinct.len(),
                1,
                "{door}: {key} is not one value across the run"
            );
        }
    }
}

/// Every committed calibration record, with its path.
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

/// Published values are printed rounded, so a re-derived value clears its
/// published one within half a unit of the last printed place, plus slack
/// for the rounding of the printed place itself.
fn assert_near(actual: f64, published: f64, tol: f64, what: &str) {
    assert!(
        (actual - published).abs() <= tol,
        "{what}: re-derived {actual:.6}, published {published}"
    );
}

/// Asserts a recomputed float against a stored one: identical, or within
/// [`SUM_TOLERANCE`] with the distance printed, so a step-level difference
/// is reported rather than hidden and anything larger is a failure.
fn assert_same_metric(recomputed: f64, stored: f64, what: &str) {
    let delta = (recomputed - stored).abs();
    assert!(
        delta <= SUM_TOLERANCE,
        "{what}: stored {stored}, recomputed {recomputed} (Δ {delta:e})"
    );
    if delta != 0.0 {
        println!(
            "{what}: stored {stored}, recomputed {recomputed} (Δ {delta:e}, within tolerance)"
        );
    }
}

/// Asserts a recomputed [`Metrics`] against a stored one: the counts are
/// exact — a different item count or confident-error count is a different
/// result — and each float is the stored number or within
/// [`SUM_TOLERANCE`] of it.
fn assert_same_metrics(recomputed: &Metrics, stored: &Metrics, what: &str) {
    assert_eq!(
        recomputed.items, stored.items,
        "{what}: the item count moved"
    );
    assert_eq!(
        recomputed.confident_errors, stored.confident_errors,
        "{what}: the confident-error count moved"
    );
    for (field, recomputed, stored) in [
        ("accuracy", recomputed.accuracy, stored.accuracy),
        ("ece", recomputed.ece, stored.ece),
        ("brier", recomputed.brier, stored.brier),
        ("nll", recomputed.nll, stored.nll),
    ] {
        assert_same_metric(recomputed, stored, &format!("{what} {field}"));
    }
}

/// Score a group's held-out blocks through a fitted map, the way
/// `report_one_fixed_map` does: the map reads each observation's raw
/// signal, and the outcome the observation carries does not move.
fn mapped_blocks(
    draws: &Draws,
    door: &str,
    family: Option<&str>,
    blocks: &[u64],
    apply: &dyn Fn(&Observation) -> f64,
) -> (Vec<Metrics>, Vec<Metrics>) {
    let mut raw = Vec::new();
    let mut mapped = Vec::new();
    for block in blocks {
        let held = draws.observations(door, "evaluation", family, *block);
        let through: Vec<Observation> = held
            .iter()
            .map(|o| Observation::new(apply(o), o.correct))
            .collect();
        let before = score(&held);
        let after = score(&through);
        // The fixed-answer contract, checked rather than assumed: a map
        // rescales the selected answer's probability, so the answer's
        // outcome cannot move and mapped accuracy is raw accuracy.
        assert!(
            (after.accuracy - before.accuracy).abs() < 1e-12,
            "{door} {family:?} block {block}: the map moved accuracy from {} to {}",
            before.accuracy,
            after.accuracy
        );
        raw.push(before);
        mapped.push(after);
    }
    (raw, mapped)
}

#[test]
#[allow(clippy::cast_precision_loss)]
fn the_raw_block_spreads_reproduce_from_the_draws() {
    // The floors the gates read. `Draw::observation` is the raw signal and
    // `report_blocks` scores it directly, so these spreads were never on
    // the mapped path — but a claim that can be recomputed should be
    // recomputed rather than trusted.
    let draws = draws_of_file("support-v2-calibration-blocks.jsonl");
    let blocks = draws.blocks("lev-base", "evaluation");
    assert_eq!(blocks, (0..8).collect::<Vec<u64>>());
    assert_eq!(
        draws.blocks("lev-band", "evaluation"),
        (0..4).collect::<Vec<u64>>()
    );
    // The claims fit on block 0's calibration draws; that block must exist
    // for each door, and the band door's claim names it the only one.
    assert!(draws.blocks("lev-base", "calibration").contains(&0));
    assert_eq!(draws.blocks("lev-band", "calibration"), vec![0]);

    // The published per-block rows of the base door's evaluation split.
    let published: [(f64, f64, f64, f64, usize); 8] = [
        (0.765, 0.106, 0.154, 1.952, 6),
        (0.796, 0.101, 0.152, 1.947, 6),
        (0.796, 0.147, 0.173, 3.019, 10),
        (0.796, 0.112, 0.155, 1.453, 4),
        (0.745, 0.166, 0.182, 1.519, 4),
        (0.796, 0.147, 0.169, 3.004, 10),
        (0.786, 0.102, 0.155, 1.953, 6),
        (0.765, 0.097, 0.150, 1.436, 4),
    ];
    let mut per_block = Vec::new();
    for (block, want) in blocks.iter().zip(published) {
        let metrics = score(&draws.observations("lev-base", "evaluation", None, *block));
        assert_eq!(metrics.items, 98);
        assert_eq!(
            metrics.confident_errors, want.4,
            "block {block} confident errors"
        );
        assert_near(metrics.accuracy, want.0, 0.0006, "accuracy");
        assert_near(metrics.ece, want.1, 0.0006, "ece");
        assert_near(metrics.brier, want.2, 0.0006, "brier");
        assert_near(metrics.nll, want.3, 0.0006, "nll");
        per_block.push(metrics);
    }

    // The published spreads: mean, standard deviation, lowest, highest.
    let spreads: [(&str, MetricRead, f64, f64, f64, f64); 5] = [
        ("accuracy", |m| m.accuracy, 0.7806, 0.0197, 0.745, 0.796),
        ("ece", |m| m.ece, 0.1221, 0.0266, 0.097, 0.166),
        ("brier", |m| m.brier, 0.1611, 0.0119, 0.150, 0.182),
        ("nll", |m| m.nll, 2.0355, 0.6428, 1.436, 3.019),
        (
            "confident_errors",
            |m| m.confident_errors as f64,
            6.2500,
            2.4928,
            4.0,
            10.0,
        ),
    ];
    for (name, read, mean, sd, low, high) in spreads {
        let values: Vec<f64> = per_block.iter().map(read).collect();
        let spread = Spread::over(&values).expect("eight blocks are a spread");
        assert_near(spread.mean, mean, 0.00006, name);
        assert_near(spread.sd, sd, 0.00006, name);
        assert_near(spread.low, low, 0.0006, name);
        assert_near(spread.high, high, 0.0006, name);
    }
}

#[test]
#[allow(clippy::cast_precision_loss)]
fn the_two_sigma_bounds_are_derived_quantities_not_the_spreads_themselves() {
    // openagents#9419 asks for the two to be kept apart. The adopted values
    // — ECE 0.0266, Brier 0.0119, log loss 0.6428 — are the raw block
    // standard deviations. The bound a one-block comparison has to clear is
    // `sigmas * sd * sqrt(1/b + 1/c)`, which at two sigma and one block on
    // each side is `sd * 2 * sqrt(2)` — a different and larger number.
    let draws = draws_of_file("support-v2-calibration-blocks.jsonl");
    let blocks = draws.blocks("lev-base", "evaluation");
    let per_block: Vec<Metrics> = blocks
        .iter()
        .map(|block| score(&draws.observations("lev-base", "evaluation", None, *block)))
        .collect();
    let bounds: [(&str, MetricRead, f64); 4] = [
        ("accuracy", |m| m.accuracy, 0.056),
        ("ece", |m| m.ece, 0.075),
        ("brier", |m| m.brier, 0.034),
        ("nll", |m| m.nll, 1.818),
    ];
    for (name, read, published_bound) in bounds {
        let values: Vec<f64> = per_block.iter().map(read).collect();
        let bound = Spread::over(&values)
            .and_then(|spread| spread.detectable(2.0, 1, 1))
            .expect("a bound exists over eight blocks");
        assert_near(bound, published_bound, 0.0006, name);
    }
    // Confident errors publish the bound as a count rather than a decimal:
    // "seven". 2 * 2.4928 * sqrt(2) is 7.05.
    let counts: Vec<f64> = per_block
        .iter()
        .map(|m| m.confident_errors as f64)
        .collect();
    let bound = Spread::over(&counts)
        .and_then(|spread| spread.detectable(2.0, 1, 1))
        .expect("a bound exists");
    assert_near(bound, 7.05, 0.01, "confident_errors");
    assert_eq!(bound.round(), 7.0, "the published bound is seven");
}

#[test]
fn the_fixed_map_claims_on_lev_base_reproduce() {
    // Every published mapped number on the base door was produced the same
    // way: fit on block 0's calibration draws, score each evaluation block
    // through `Map::apply` on the raw signal. That path never touched
    // `eval::mapped_observations`, so the contract settlement cannot have
    // moved it — and here it is, recomputed.
    let draws = draws_of_file("support-v2-calibration-blocks.jsonl");
    let blocks = draws.blocks("lev-base", "evaluation");

    // The suite map, fitted on all 98 calibration items of block 0.
    let fit_on = draws.observations("lev-base", "calibration", None, 0);
    assert_eq!(fit_on.len(), 98);
    let map = Map::fit_auto(&fit_on);
    let (raw, mapped) = mapped_blocks(&draws, "lev-base", None, &blocks, &|o| map.apply(o.raw));
    assert_near(mapped[0].ece, 0.071, 0.0006, "suite block 0 ece");
    assert_near(mapped[0].brier, 0.157, 0.0006, "suite block 0 brier");
    assert_near(mapped[0].nll, 0.481, 0.0006, "suite block 0 nll");
    assert_eq!(mapped[0].confident_errors, 0);
    // The published spread of the mapped log loss: the claim that a mapped
    // number carries far less seed noise than the raw one.
    let nll: Vec<f64> = mapped.iter().map(|m| m.nll).collect();
    let spread = Spread::over(&nll).expect("a spread");
    assert_near(spread.mean, 0.477, 0.0006, "mapped nll mean");
    assert_near(spread.sd, 0.0182, 0.00006, "mapped nll sd");
    let raw_nll: Vec<f64> = raw.iter().map(|m| m.nll).collect();
    assert_near(
        Spread::over(&raw_nll).expect("a spread").sd,
        0.6428,
        0.00006,
        "raw nll sd, the floor the mapped spread is quoted against",
    );

    // The routing map, fitted on the family's 50 calibration items: the
    // shape the committed `lev-base/routing.json` record was fitted in.
    let fit_on = draws.observations("lev-base", "calibration", Some("routing"), 0);
    assert_eq!(fit_on.len(), 50);
    let map = Map::fit_auto(&fit_on);
    let (_, mapped) = mapped_blocks(&draws, "lev-base", Some("routing"), &blocks, &|o| {
        map.apply(o.raw)
    });
    assert_near(mapped[0].ece, 0.024, 0.0006, "routing block 0 ece");
    assert_near(mapped[0].brier, 0.133, 0.0006, "routing block 0 brier");
    assert_near(mapped[0].nll, 0.431, 0.0006, "routing block 0 nll");
    assert_eq!(mapped[0].confident_errors, 0);
    let nll: Vec<f64> = mapped.iter().map(|m| m.nll).collect();
    let spread = Spread::over(&nll).expect("a spread");
    assert_near(spread.mean, 0.433, 0.0006, "routing mapped nll mean");
    assert_near(spread.sd, 0.0188, 0.00006, "routing mapped nll sd");
    let ece: Vec<f64> = mapped.iter().map(|m| m.ece).collect();
    let spread = Spread::over(&ece).expect("a spread");
    assert_near(spread.mean, 0.022, 0.0006, "routing mapped ece mean");
    assert_near(spread.sd, 0.0098, 0.00006, "routing mapped ece sd");

    // The two thin families, block 0 only: urgency pooled and severity
    // pooled, where the map spends rather than earns.
    let fit_on = draws.observations("lev-base", "calibration", Some("urgency"), 0);
    assert_eq!(fit_on.len(), 30);
    let map = Map::fit_auto(&fit_on);
    let (raw, mapped) = mapped_blocks(&draws, "lev-base", Some("urgency"), &blocks, &|o| {
        map.apply(o.raw)
    });
    assert_near(mapped[0].ece, 0.025, 0.0006, "urgency block 0 ece");
    assert_near(mapped[0].brier, 0.196, 0.0006, "urgency block 0 brier");
    assert_near(mapped[0].nll, 0.582, 0.0006, "urgency block 0 nll");
    // The published Brier claim: the map loses a little on this family.
    let brier: Vec<f64> = mapped.iter().map(|m| m.brier).collect();
    assert_near(
        Spread::over(&brier).expect("a spread").mean,
        0.185,
        0.0006,
        "urgency mapped brier mean",
    );
    let raw_brier: Vec<f64> = raw.iter().map(|m| m.brier).collect();
    assert_near(
        Spread::over(&raw_brier).expect("a spread").mean,
        0.182,
        0.0006,
        "urgency raw brier mean",
    );

    let fit_on = draws.observations("lev-base", "calibration", Some("severity"), 0);
    assert_eq!(fit_on.len(), 18);
    let map = Map::fit_auto(&fit_on);
    let (_, mapped) = mapped_blocks(&draws, "lev-base", Some("severity"), &blocks, &|o| {
        map.apply(o.raw)
    });
    assert_near(mapped[0].ece, 0.120, 0.0006, "severity block 0 ece");
    assert_near(mapped[0].brier, 0.215, 0.0006, "severity block 0 brier");
    assert_near(mapped[0].nll, 0.639, 0.0006, "severity block 0 nll");
}

#[test]
fn the_band_conditioned_claim_on_lev_band_reproduces() {
    // The largest mapped claim in the directory: conditioning the map on
    // the trained certainty band cut the band adapter's block-0 log loss
    // from 2.601 to 0.388 where the pooled map gave 0.499. Four evaluation
    // blocks, fitted on the door's one calibration block.
    let draws = draws_of_file("support-v2-calibration-blocks.jsonl");
    let blocks = draws.blocks("lev-band", "evaluation");
    assert_eq!(blocks.len(), 4);
    let fit_on = draws.observations("lev-band", "calibration", None, 0);
    assert_eq!(fit_on.len(), 98);

    let banded = Map::fit_banded(&fit_on);
    let bands: Vec<&str> = banded.by_band.keys().map(String::as_str).collect();
    assert_eq!(
        bands,
        ["almost certain", "likely"],
        "the published band tables; a band missing here changes the claim"
    );
    let pooled = Map::fit_auto(&fit_on);

    let (raw, conditioned) = mapped_blocks(&draws, "lev-band", None, &blocks, &|o| {
        banded.apply_banded(o.raw, o.band.as_deref())
    });
    let (_, pooled_blocks) =
        mapped_blocks(&draws, "lev-band", None, &blocks, &|o| pooled.apply(o.raw));

    assert_near(
        raw[0].nll,
        2.601,
        0.0006,
        "raw block 0 nll, the claim's baseline",
    );
    assert_near(conditioned[0].ece, 0.069, 0.0006, "banded block 0 ece");
    assert_near(conditioned[0].brier, 0.104, 0.0006, "banded block 0 brier");
    assert_near(
        conditioned[0].nll,
        0.388,
        0.0006,
        "banded block 0 nll, the claim",
    );
    assert_eq!(conditioned[0].confident_errors, 11);
    assert_near(
        pooled_blocks[0].nll,
        0.499,
        0.0006,
        "pooled block 0 nll, the road not taken",
    );

    let nll: Vec<f64> = conditioned.iter().map(|m| m.nll).collect();
    let spread = Spread::over(&nll).expect("a spread");
    assert_near(spread.mean, 0.409, 0.0006, "banded nll mean");
    assert_near(spread.sd, 0.0204, 0.00006, "banded nll sd");
    let ece: Vec<f64> = conditioned.iter().map(|m| m.ece).collect();
    let spread = Spread::over(&ece).expect("a spread");
    assert_near(spread.mean, 0.071, 0.0006, "banded ece mean");
    assert_near(spread.sd, 0.0021, 0.00006, "banded ece sd");
}

#[test]
fn every_committed_calibration_record_regenerates_from_the_store() {
    // The six records under `crates/lev/calibration/` are the mapped claims
    // with the most downstream weight: `lev-serve` serves them. Each is
    // regenerated here from the retained rows — the map refitted on the
    // calibration split, scored on the rest, judged by the committed gate —
    // and compared field by field: the map, the counts, the verdict, and
    // the provenance must be identical, and each float metric must be the
    // stored number or within `SUM_TOLERANCE` of it.
    let gate = gym::gate::load("probability-v1").expect("the committed gate");
    let suite = gym::suite::support_v2_three_way().expect("the suite loads");
    let rows = rows_of_file("support-v2-three-way.jsonl");
    assert_rows_cover_the_suite(&rows, &suite);
    let records = committed_records();
    assert_eq!(records.len(), 6, "the committed record set");

    for (path, record) in &records {
        let name = format!("{} {}", record.door, record.family);
        assert_eq!(
            record.gate_id.as_deref(),
            Some("probability-v1"),
            "{}",
            path.display()
        );
        assert!(
            record
                .gate_digest
                .as_deref()
                .is_some_and(|digest| gate.has_digest(digest)),
            "{name}: the record names neither the current gate nor a reviewed equivalent identity",
        );
        assert_eq!(
            record.suite_digest, suite.digest,
            "{name}: the record pins a different suite than the rows do",
        );
        let mine: Vec<Row> = rows
            .iter()
            .filter(|row| {
                row.door == record.door
                    && row.family == record.family
                    && row.suite_digest == record.suite_digest
            })
            .cloned()
            .collect();
        // The record's provenance must be the rows' provenance: same
        // estimator, same seed block, same door identity — a regenerated
        // number about a different run is a different claim.
        for row in &mine {
            assert_eq!(
                row.estimator, record.estimator_config.estimator,
                "{name} {}: another estimator",
                row.item_id
            );
            assert_eq!(
                row.samples,
                Some(record.estimator_config.samples),
                "{name} {}: another sample count",
                row.item_id
            );
            assert_eq!(
                row.seed_base,
                Some(record.estimator_config.seed_base),
                "{name} {}: another seed block",
                row.item_id
            );
            assert_eq!(
                row.door_identity, record.door_identity,
                "{name} {}: another door identity",
                row.item_id
            );
        }
        let fit_on: Vec<Row> = mine
            .iter()
            .filter(|row| row.split == record.partition_id)
            .cloned()
            .collect();
        let score_on: Vec<Row> = mine
            .iter()
            .filter(|row| row.split != record.partition_id)
            .cloned()
            .collect();
        assert!(
            !fit_on.is_empty() && !score_on.is_empty(),
            "{name}: the store holds no rows for this door and family",
        );

        let fit = fit_family(&record.family, &fit_on, &score_on, &gate);
        assert_eq!(fit.map, record.map, "{name}: the refitted table differs");
        // The raw metrics were never on the mapped path, but they are still
        // running sums written by a binary whose operand order is not
        // recorded — `lev-adapted@1` severity's raw ECE stored as
        // 0.24999999999999992 and recomputes as 0.24999999999999994, one
        // representable step. The mapped metrics are the contract's
        // surface. For both, the counts are exact and the floats are the
        // stored number or within a representable step of it.
        assert_same_metrics(&fit.raw, &record.raw_metrics, &format!("{name} raw"));
        assert_same_metrics(
            &fit.calibrated,
            &record.calibrated_metrics,
            &format!("{name} mapped"),
        );
        assert_eq!(
            fit.admitted(),
            record.admitted,
            "{name}: the verdict changed"
        );
        assert_eq!(
            fit.verdict(),
            record.verdict,
            "{name}: the verdict line changed"
        );
        println!(
            "{name}: regenerated ({})",
            if record.admitted {
                "admitted"
            } else {
                "not admitted"
            }
        );
    }
}

#[test]
fn mapped_observations_and_the_draw_path_agree_where_they_overlap() {
    // The two ways a mapped number is produced in this repository: the
    // draws path, `Map::apply` on a draw's top frequency, and the store
    // path, `mapped_observations` reading the selected option's rescaled
    // probability out of the retained distribution. The contract change
    // touched only the second. On rows whose raw signal equals the
    // selected option's share — every committed row — the two compute the
    // same number, which is what lets the draws-derived claims stand.
    let rows = rows_of_file("support-v2-three-way.jsonl");
    let scored: Vec<Row> = rows
        .into_iter()
        .filter(|row| row.is_scored() && row.distribution.is_some())
        .collect();
    assert!(!scored.is_empty());
    let records = committed_records();
    for (_, record) in &records {
        let mine: Vec<Row> = scored
            .iter()
            .filter(|row| row.door == record.door)
            .cloned()
            .collect();
        let observed = mapped_observations(&mine, &record.map);
        let direct: Vec<Observation> = mine
            .iter()
            .filter_map(|row| row.distribution.as_ref())
            .zip(observed.iter())
            .map(|(distribution, observation)| {
                let (option, _) = gym::calibrate::selected(distribution).expect("a scored row");
                Observation::new(
                    record.map.apply_distribution(distribution)[option],
                    observation.correct,
                )
            })
            .collect();
        assert_eq!(observed, direct, "{}: the two paths disagree", record.door);
    }
}
