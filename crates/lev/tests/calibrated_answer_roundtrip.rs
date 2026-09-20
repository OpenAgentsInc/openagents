//! Check that the fixed-answer calibration contract survives the public wire
//! shape, end to end: estimator → `estimator::answer` → JSON → `jev` decode →
//! `gym::eval` → the stored row.
//!
//! A served map rescales the estimator's pick without replacing it, and can
//! leave a runner-up numerically larger. The answer has to arrive as a named
//! option — `choice` on a Choice, `selected` on a Noul or a Score — because
//! neither `noul` nor `score` can carry it: the first is the calibrated
//! probability of yes and the second is the weighted position, and neither
//! is allowed to change meaning to smuggle the pick across.

use gym::calibrate::{Map, Observation};
use gym::eval::{Disposition, Run, read_answer};
use gym::row::{DoorIdentity, Row};
use gym::suite::{Item, Partition};
use indexmap::IndexMap;
use lev::Kind;
use serde_json::json;

/// The fields every row of a run shares, cut down to what a wire test needs.
fn run() -> Run {
    Run {
        suite: "wire-v1".to_string(),
        suite_digest: "sha256:suite".to_string(),
        question_set: None,
        question_digest: None,
        door: "lev".to_string(),
        door_identity: DoorIdentity::published("lev", "sig:base-1", "map-v1"),
        estimator: "l2".to_string(),
        samples: Some(8),
        seed_base: Some(0),
        recorded_at: "2026-09-20T00:00:00Z".to_string(),
        gate_id: None,
        gate_digest: None,
    }
}

/// An item whose label is the answer a naive reader would score: the argmax
/// of the *mapped* distribution. Scoring `correct` against it is what proves
/// the evaluator judged the named pick, not the largest reported number.
fn item_named_after(truth: &str) -> Item {
    Item {
        id: "wire/000".to_string(),
        family: "wire".to_string(),
        kind: "any".to_string(),
        state: json!("a state"),
        question: None,
        truth: truth.to_string(),
        partition: Partition::Development,
        label_source: None,
        label_rule: None,
    }
}

/// The shared tail of every roundtrip: the wire answer decodes through the
/// SDK, reads as the evaluator's disposition, and stores as a row against an
/// item labelled with `truth`.
fn evaluated_row(wire: &serde_json::Value, truth: &str) -> (String, IndexMap<String, f64>, Row) {
    let decoded = jev::SystemOneResponse::decode(jev::RawResponse {
        status: 200,
        headers: Default::default(),
        bytes: serde_json::to_vec(
            &serde_json::json!({"model": "test", "answers": {"q": wire}, "usage": {}}),
        )
        .unwrap(),
    })
    .unwrap();
    let Disposition::Answered {
        chosen,
        distribution,
    } = read_answer(&decoded.answers["q"])
    else {
        panic!("an answer must be scored");
    };
    let row = run()
        .row(
            &item_named_after(truth),
            None,
            &Disposition::Answered {
                chosen: chosen.clone(),
                distribution: distribution.clone(),
            },
            None,
        )
        .expect("a named answer is recorded");
    (chosen, distribution, row)
}

fn assert_roundtrip(kind: Kind, pairs: &[(&str, f64)]) {
    let raw: IndexMap<String, f64> = pairs
        .iter()
        .map(|(key, value)| (key.to_string(), *value))
        .collect();
    let selected = lev::estimator::argmax(&raw).unwrap();
    let map = Map::fit(&[Observation::new(0.8, false)], 1);
    let mapped = map.apply_distribution(&raw);

    // The fixture is only exercising the contract when the rescale actually
    // moves the largest number off the selected option; assert it did rather
    // than let a weak map make the test pass on the argmax anyway.
    let implied = gym::calibrate::selected(&mapped)
        .map(|(option, _)| option.to_string())
        .expect("a non-empty mapped distribution");
    assert_ne!(
        implied, selected,
        "the fixture is meant to move the argmax: {mapped:?}"
    );

    let answer = lev::estimator::answer(kind, &mapped, &IndexMap::new(), &selected).unwrap();
    let wire = serde_json::to_value(&answer).unwrap();
    assert_eq!(
        wire.get("selected")
            .or_else(|| wire.get("choice"))
            .and_then(|field| field.as_str()),
        Some(selected.as_str()),
        "the wire names the estimator's pick: {wire}"
    );

    // The stored row: `correct` judges the named pick, so labelling the item
    // with the implied argmax makes the pick wrong — a scorer that read the
    // mapped numbers instead would call it right.
    let (chosen, distribution, row) = evaluated_row(&wire, &implied);
    assert_eq!(
        chosen, selected,
        "calibration must preserve the selected answer through serving and evaluation"
    );
    row.check().expect("the row is coherent");
    assert_eq!(row.selected.as_deref(), Some(selected.as_str()));
    assert_eq!(
        row.correct,
        Some(false),
        "the pick is scored, not the implied argmax"
    );
    assert_eq!(row.raw_top, distribution.get(&selected).copied());

    // And a map read back over the stored row still rescales the named
    // option's share, because that is what `correct` is about.
    let mapped = gym::eval::mapped_observations(std::slice::from_ref(&row), &map);
    assert_eq!(mapped.len(), 1);
    assert_eq!(mapped[0].raw, map.apply(distribution[selected.as_str()]));
    assert!(!mapped[0].correct);
}

#[test]
fn calibrated_choice_preserves_the_fixed_answer_on_the_wire() {
    assert_roundtrip(Kind::Choice, &[("yes", 0.8), ("no", 0.2)]);
}

#[test]
fn calibrated_noul_preserves_the_fixed_answer_on_the_wire() {
    assert_roundtrip(Kind::Noul, &[("no", 0.2), ("yes", 0.8)]);
}

#[test]
fn calibrated_score_preserves_the_fixed_answer_on_the_wire() {
    assert_roundtrip(Kind::Score, &[("0", 0.8), ("1", 0.15), ("2", 0.05)]);
}

#[test]
fn an_uncalibrated_answer_reads_the_same_either_way() {
    // The fallback half of the contract: an answer that names no `selected`
    // is an uncalibrated one, where the pick is what the numbers already
    // say. Old wire shapes and doors that never fit a map keep working.
    let wire = serde_json::to_value(lev::api::Answer::Score {
        score: 0.62,
        confidence: 0.5,
        selected: None,
        legend: IndexMap::new(),
        probabilities: [
            ("0".to_string(), 0.5),
            ("1".to_string(), 0.375),
            ("2".to_string(), 0.125),
        ]
        .into_iter()
        .collect(),
    })
    .unwrap();
    assert!(
        !wire.as_object().unwrap().contains_key("selected"),
        "{wire}"
    );

    let decoded = jev::SystemOneResponse::decode(jev::RawResponse {
        status: 200,
        headers: Default::default(),
        bytes: serde_json::to_vec(
            &serde_json::json!({"model": "test", "answers": {"q": wire}, "usage": {}}),
        )
        .unwrap(),
    })
    .unwrap();
    let Disposition::Answered { chosen, .. } = read_answer(&decoded.answers["q"]) else {
        panic!("an answer must be scored");
    };
    assert_eq!(chosen, "0", "the argmax of an uncalibrated distribution");
}

#[test]
fn a_tied_raw_maximum_serves_the_last_tied_level_through_the_wire() {
    // The convention a tie is read under — the last of the equal leaders —
    // is part of the pick, so it has to survive the wire like any other
    // selected option. The fixture is the shape the ordinality probe found
    // common on Lev: an eight-sample estimator answering in halves, tied
    // between the top two levels.
    let raw: IndexMap<String, f64> = [("0", 0.0), ("1", 0.0), ("2", 0.5), ("3", 0.5), ("4", 0.0)]
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect();
    assert_eq!(
        gym::calibrate::tied(&raw),
        2,
        "an exact tie on the raw grid"
    );
    let selected = lev::estimator::argmax(&raw).unwrap();
    assert_eq!(selected, "3", "the last of the equal leaders is the pick");

    // Jeffreys smoothing fits the lone observation's 0.5 to (0 + 0.5) /
    // (1 + 1) = 0.25: the tied pick drops to a quarter of the mass and the
    // other leader inherits 0.75.
    let map = Map::fit(&[Observation::new(0.5, false)], 1);
    let mapped = map.apply_distribution(&raw);
    assert_eq!(mapped["3"], 0.25);
    assert_eq!(mapped["2"], 0.75);
    assert_eq!(
        gym::calibrate::selected(&mapped).unwrap().0,
        "2",
        "the rescale moved the argmax off the tied pick"
    );

    let answer = lev::estimator::answer(Kind::Score, &mapped, &IndexMap::new(), &selected).unwrap();
    let wire = serde_json::to_value(&answer).unwrap();
    assert_eq!(wire["selected"], "3", "the wire names the tied pick");
    // 0·0 + 1·0 + 2·0.75 + 3·0.25 + 4·0: the weighted position sits between
    // the tied levels, nearer the one that is not the answer.
    assert_eq!(wire["score"], 2.25);

    // Label the item with the level the mapped numbers imply: a scorer that
    // read the distribution instead of the named pick would call this right.
    let (chosen, _distribution, row) = evaluated_row(&wire, "2");
    assert_eq!(chosen, "3", "the tied pick survived the wire");
    row.check().expect("the row is coherent");
    assert_eq!(row.selected.as_deref(), Some("3"));
    assert_eq!(
        row.correct,
        Some(false),
        "the pick is scored, not the implied argmax"
    );
    assert_eq!(row.raw_top, Some(0.25), "the share the answer carried");

    // A map read back over the stored row rescales the named option's share:
    // the one-bin table reads any signal as 0.25.
    let observations = gym::eval::mapped_observations(&[row], &map);
    assert_eq!(observations.len(), 1);
    assert_eq!(observations[0].raw, 0.25);
    assert!(!observations[0].correct);
}

#[test]
fn a_band_conditioned_map_preserves_the_fixed_answer_on_the_wire() {
    // The same contract under the banded API: `Map::fit_banded` keeps a
    // per-band table once a band holds ITEMS_PER_BIN observations, and
    // `apply_distribution_banded` rescales the fixed pick through the band's
    // own table. Fifteen wrong "likely" answers at 0.8 fit that band's
    // occupied bin to (0 + 0.5) / (15 + 1) = 0.03125.
    let fits: Vec<Observation> = (0..15)
        .map(|_| Observation::banded(0.8, false, "likely"))
        .collect();
    let map = Map::fit_banded(&fits);
    assert!(
        map.by_band.contains_key("likely"),
        "the band table was fitted"
    );

    let raw: IndexMap<String, f64> = [("0", 0.8), ("1", 0.15), ("2", 0.05)]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    let selected = lev::estimator::argmax(&raw).unwrap();
    assert_eq!(selected, "0");

    // The pick carries 0.03125; the remaining 0.96875 splits in the observed
    // 3:1 proportion, and the runner-up leads the reported numbers. The
    // proportions are not exact dyadics, so the shared-out shares are pinned
    // to a tolerance rather than bit equality.
    let mapped = map.apply_distribution_banded(&raw, Some("likely"));
    assert_eq!(mapped["0"], 0.03125);
    assert!(
        (mapped["1"] - 0.7265625).abs() < 1e-12,
        "0.96875 in proportion: {}",
        mapped["1"]
    );
    assert!(
        (mapped["2"] - 0.2421875).abs() < 1e-12,
        "0.96875 in proportion: {}",
        mapped["2"]
    );
    assert_eq!(
        gym::calibrate::selected(&mapped).unwrap().0,
        "1",
        "the band-conditioned rescale moved the argmax off the pick"
    );

    let answer = lev::estimator::answer(Kind::Score, &mapped, &IndexMap::new(), &selected).unwrap();
    let wire = serde_json::to_value(&answer).unwrap();
    assert_eq!(
        wire["selected"], "0",
        "the wire names the pick under a banded map"
    );
    let score = wire["score"].as_f64().expect("the weighted position");
    assert!(
        (score - 1.2109375).abs() < 1e-12,
        "1·0.7265625 + 2·0.2421875: {score}"
    );

    let (chosen, _distribution, row) = evaluated_row(&wire, "1");
    assert_eq!(
        chosen, "0",
        "the pick survived a band-conditioned inversion"
    );
    row.check().expect("the row is coherent");
    assert_eq!(row.selected.as_deref(), Some("0"));
    assert_eq!(row.correct, Some(false));
    assert_eq!(row.raw_top, Some(0.03125));

    // `mapped_observations` reads a stored row through the map's pooled
    // table. The pooled table saw only 0.8 signals, so the named option's
    // stored 0.03125 lands in an empty bin and reads the base rate, 0.0.
    let observations = gym::eval::mapped_observations(&[row], &map);
    assert_eq!(observations.len(), 1);
    assert_eq!(
        observations[0].raw, 0.0,
        "the named option's share through the pooled table"
    );
    assert!(!observations[0].correct);
}
