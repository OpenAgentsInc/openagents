//! The calibrated-change golden is an authored target: the path a small
//! change should take once calibration lands. These tests keep it honest:
//! it reads back as ATIF, says it is authored, and its projected totals
//! agree with its own steps and with the observed baseline it cites.

use coderbench::{GoldenMeta, Provenance, goldens_dir};

const NAME: &str = "calibrated-change-bottle-etag";

fn projection() -> serde_json::Value {
    let path = goldens_dir().join(format!("{NAME}.projection.json"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn the_projected_golden_reads_back_as_atif() {
    let path = goldens_dir().join(format!("{NAME}.atif.jsonl"));
    let recording = atif::log::read_whole(&path).expect("the golden reads as a whole ATIF log");
    assert_eq!(recording.session.id, "projected-calibrated-bottle-etag");
    let names: Vec<String> = recording
        .steps
        .iter()
        .filter_map(|step| step.call.as_ref().map(|call| call.name.clone()))
        .collect();
    assert_eq!(
        names,
        [
            "route",
            "classify",
            "choose_engine",
            "delegate",
            "independent_check",
            "recalibration_record"
        ],
        "the calibrated path, in order"
    );
}

#[test]
fn the_projected_golden_says_it_is_authored() {
    let meta = GoldenMeta::load(&goldens_dir().join(format!("{NAME}.meta.json")))
        .expect("the golden declares its provenance");
    assert_eq!(meta.task, NAME);
    assert_eq!(
        meta.provenance,
        Provenance::Authored,
        "a projection must never claim to be observed"
    );
    assert!(meta.note.contains("projection.json"));
}

#[test]
fn every_projected_step_says_where_its_number_comes_from() {
    let projection = projection();
    let steps = projection["projected_steps"].as_array().unwrap();
    assert!(!steps.is_empty());
    for step in steps {
        assert!(
            !step["derivation"].as_str().unwrap_or("").is_empty(),
            "{step} has no derivation"
        );
    }
    for gain in projection["projected_study_wide_vs_raw_claude"]
        .as_object()
        .unwrap()
        .values()
    {
        assert!(!gain["derivation"].as_str().unwrap_or("").is_empty());
    }
}

#[test]
fn the_projected_total_matches_its_steps_and_the_baseline() {
    let projection = projection();
    let total = &projection["projected_total_this_task"];
    let steps = projection["projected_steps"].as_array().unwrap();
    let cost: f64 = steps.iter().map(|s| s["cost_usd"].as_f64().unwrap()).sum();
    // classify runs concurrently with route, so it adds no wall time.
    let wall: f64 = steps
        .iter()
        .filter(|s| !s["step"].as_str().unwrap().starts_with("classify"))
        .map(|s| s["wall_s"].as_f64().unwrap())
        .sum();
    assert!((cost - total["cost_usd"].as_f64().unwrap()).abs() < 0.002);
    assert!((wall - total["wall_s"].as_f64().unwrap()).abs() < 0.5);
    let raw = &projection["baseline"]["per_task_bottle_etag"]["raw-claude"];
    let ratio = total["cost_usd"].as_f64().unwrap() / raw["cost_usd"].as_f64().unwrap();
    let stated = total["vs_raw_claude"]["cost"].as_f64().unwrap();
    assert!((ratio - stated).abs() < 0.01, "{ratio} vs {stated}");
    let time = total["wall_s"].as_f64().unwrap() / raw["wall_s"].as_f64().unwrap();
    assert!((time - total["vs_raw_claude"]["time"].as_f64().unwrap()).abs() < 0.01);
}
