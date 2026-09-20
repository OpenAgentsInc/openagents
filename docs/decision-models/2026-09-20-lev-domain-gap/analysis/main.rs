use gym::{
    calibrate::{Observation, score},
    row::Row,
    store::Store,
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
fn emit(label: &str, rows: &[(String, String, Option<Observation>)], expected: usize) {
    assert_eq!(rows.len(), expected, "{label} coverage");
    assert_eq!(
        rows.iter().map(|x| &x.0).collect::<BTreeSet<_>>().len(),
        expected,
        "duplicate items"
    );
    let families: BTreeSet<_> = rows.iter().map(|r| r.1.as_str()).collect();
    for family in std::iter::once("all").chain(families) {
        let selected: Vec<_> = rows
            .iter()
            .filter(|r| family == "all" || r.1 == family)
            .collect();
        let obs: Vec<_> = selected.iter().filter_map(|r| r.2.clone()).collect();
        let correct = obs.iter().filter(|o| o.correct).count();
        println!(
            "{}",
            json!({"selection":label,"family":family,"rows":selected.len(),"answered":obs.len(),"refused":selected.len()-obs.len(),"correct":correct,"accuracy_all_rows":correct as f64/selected.len() as f64,"panel_answered":score(&obs)})
        );
    }
}
fn read(path: &Path) -> Vec<Row> {
    Store::at(path)
        .verified_rows()
        .unwrap()
        .into_iter()
        .map(|v| {
            let r: Row = serde_json::from_value(v).unwrap();
            r.check().unwrap();
            r
        })
        .collect()
}
fn obs(r: &Row) -> Option<Observation> {
    if r.answered {
        Some(Observation::new(r.raw_top.unwrap(), r.correct.unwrap()))
    } else {
        None
    }
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let root = Path::new(&args[1]);
    let old: Value =
        serde_json::from_slice(&std::fs::read(root.join("support-v2-suite.json")).unwrap())
            .unwrap();
    let new: Value = serde_json::from_slice(
        &std::fs::read(root.join("support-v2-three-way-suite.json")).unwrap(),
    )
    .unwrap();
    let lookup: BTreeMap<_, _> = old["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| (v["id"].as_str().unwrap(), v))
        .collect();
    let mut open = BTreeSet::new();
    let mut untrained = BTreeSet::new();
    for v in new["items"].as_array().unwrap() {
        let id = v["id"].as_str().unwrap();
        for k in ["state", "question", "truth", "family", "kind"] {
            assert_eq!(v[k], lookup[id][k]);
        }
        if v["partition"] != "locked" {
            open.insert(id.to_string());
            if lookup[id]["split"] == "evaluation" {
                untrained.insert(id.to_string());
            }
        }
    }
    assert_eq!(open.len(), 157);
    assert_eq!(untrained.len(), 79);
    let original = read(&root.join("historical-support-v2-three-way.jsonl"));
    for door in ["lev-base", "lev-adapted@1"] {
        for (label, ids) in [("open157", &open), ("untrained79", &untrained)] {
            let rows = original
                .iter()
                .filter(|r| r.door == door && ids.contains(&r.item_id))
                .map(|r| (r.item_id.clone(), r.family.clone(), obs(r)))
                .collect::<Vec<_>>();
            emit(&format!("historical/{door}/{label}"), &rows, ids.len());
        }
    }
    let draws = gym::spread::Draws::parse(
        &std::fs::read_to_string(root.join("historical-support-v2-calibration-blocks.jsonl"))
            .unwrap(),
    )
    .unwrap();
    for (label, ids) in [("open157", &open), ("untrained79", &untrained)] {
        let rows = draws
            .rows()
            .iter()
            .filter(|r| r.door == "lev-band" && r.block == 0 && ids.contains(&r.item))
            .map(|r| {
                assert_eq!(r.refused, 0);
                assert_eq!(r.samples, 8);
                assert_eq!(r.truth, lookup[r.item.as_str()]["truth"].as_str().unwrap());
                (r.item.clone(), r.family.clone(), Some(r.observation()))
            })
            .collect::<Vec<_>>();
        emit(
            &format!("historical/lev-band/block0/{label}"),
            &rows,
            ids.len(),
        );
    }
    let old_ood = read(&root.join("historical-coder-turns-v1.jsonl"));
    for door in ["lev-base", "lev-adapted"] {
        let rows = old_ood
            .iter()
            .filter(|r| r.door == door && r.split == "development")
            .map(|r| (r.item_id.clone(), r.family.clone(), obs(r)))
            .collect::<Vec<_>>();
        emit(&format!("historical/{door}/coder-turns-v1"), &rows, 130);
    }
    for path in &args[2..] {
        let rows = read(Path::new(path));
        let suite = rows.first().unwrap().suite.clone();
        let first = rows.first().unwrap();
        let door = first.door.clone();
        assert!(rows.iter().all(|r| r.suite == suite
            && r.door == door
            && r.door_identity == first.door_identity
            && r.question_digest == first.question_digest
            && r.estimator == "l2"
            && r.samples == Some(8)
            && r.seed_base == Some(0)));
        let specification: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("{suite}-suite.json"))).unwrap(),
        )
        .unwrap();
        let expected_ids: BTreeSet<String> = specification["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| {
                if suite == "coder-turns-v1" {
                    item["partition"] == "development"
                } else {
                    item["partition"] != "locked"
                }
            })
            .map(|item| item["id"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(
            rows.iter()
                .map(|r| r.item_id.clone())
                .collect::<BTreeSet<_>>(),
            expected_ids
        );
        assert!(
            rows.iter()
                .all(|r| r.suite_digest == specification["digest"].as_str().unwrap())
        );
        if suite == "support-v2-three-way" {
            for (label, ids) in [("open157", &open), ("untrained79", &untrained)] {
                let selected = rows
                    .iter()
                    .filter(|r| ids.contains(&r.item_id))
                    .map(|r| (r.item_id.clone(), r.family.clone(), obs(r)))
                    .collect::<Vec<_>>();
                emit(&format!("new/{door}/{label}"), &selected, ids.len());
            }
        } else {
            let expected = match suite.as_str() {
                "coder-turns-v1" => 130,
                "external-v1" => 160,
                _ => panic!("unexpected suite"),
            };
            let selected = rows
                .iter()
                .map(|r| (r.item_id.clone(), r.family.clone(), obs(r)))
                .collect::<Vec<_>>();
            emit(&format!("new/{door}/{suite}"), &selected, expected);
        }
    }
}
