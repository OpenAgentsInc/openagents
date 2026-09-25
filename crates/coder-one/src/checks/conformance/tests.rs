use std::path::PathBuf;

use serde_json::{Value, json};

use super::*;
use crate::checks::contract::host::Contained;
use crate::component::jev::{Recorded, RecordedAnswer};

fn workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/conformance/workspace")
}

fn have_python() -> bool {
    std::process::Command::new("python3")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Each entry, the reference function that follows it, the departure
/// that doesn't, and the property the departure must fail.
const PAIRS: [(&str, &str, &str, &str); 11] = [
    (
        "cosine-similarity",
        "cosine_similarity",
        "cosine_similarity",
        "known_angle",
    ),
    (
        "cosine-distance",
        "cosine_distance",
        "cosine_distance",
        "orthogonal",
    ),
    (
        "l2-normalize",
        "l2_normalize",
        "l2_normalize",
        "zero_vector",
    ),
    (
        "euclidean-distance",
        "euclidean",
        "euclidean",
        "not_squared",
    ),
    (
        "mmd-squared-unbiased",
        "mmd2_unbiased",
        "mmd2_biased",
        "identical_below_zero",
    ),
    (
        "ks-two-sample-statistic",
        "ks_statistic",
        "ks_statistic",
        "half_overlap",
    ),
    ("population-stability-index", "psi", "psi", "known_two_bins"),
    ("debounce", "debounce", "debounce", "glitch_while_on"),
    ("softmax", "softmax", "softmax", "large_scores"),
    (
        "levenshtein-distance",
        "levenshtein",
        "levenshtein",
        "substitution_costs_one",
    ),
    ("pearson-correlation", "pearson", "pearson", "known"),
];

#[test]
fn the_registry_reads_and_every_file_is_carried() {
    let registry = registry();
    assert_eq!(registry.entries.len(), FILES.len());
    assert!(registry.entries.len() >= 8);
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../methods");
    let mut on_disk: Vec<String> = std::fs::read_dir(&root)
        .expect("methods/ reads")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".json"))
        .collect();
    on_disk.sort();
    let mut carried: Vec<String> = FILES.iter().map(|(n, _)| (*n).to_string()).collect();
    carried.sort();
    assert_eq!(on_disk, carried, "every methods/ file is in FILES");
    for (name, text) in FILES {
        let on_disk = std::fs::read_to_string(root.join(name)).expect("reads");
        assert_eq!(on_disk, text, "{name}");
    }
    for loaded in &registry.entries {
        let entry = &loaded.entry;
        assert!(!entry.provenance.why.is_empty(), "{}", entry.slug);
        assert!(!entry.citations.is_empty(), "{}", entry.slug);
        assert_eq!(loaded.digest.len(), 64);
    }
}

#[test]
fn a_bad_entry_is_refused() {
    let (_, text) = FILES[0];
    let mut value: Value = serde_json::from_str(text).expect("json");
    value["properties"][0]["same_as"] = json!([[1, 2], [3, 4]]);
    let bad = serde_json::to_string(&value).expect("text");
    let error = Registry::from_texts([(FILES[0].0, bad.as_str())]).expect_err("refused");
    assert!(error.contains("exactly one check"), "{error}");
    let error = Registry::from_texts([("wrong-name.json", FILES[0].1)]).expect_err("refused");
    assert!(error.contains("named for its slug"), "{error}");
}

#[test]
fn the_question_set_templates_one_choice_per_function() {
    let set = question_set();
    assert_eq!(set.id, "openagents.method-conformance.v1");
    assert!(set.instructions.contains(FINDING));
    assert!(!set.instructions.to_lowercase().contains("correct?"));
    let asked = serde_json::to_value(question(2)).expect("serializes");
    let text = asked.to_string();
    assert!(text.contains("functions[2]"));
    assert!(text.contains("\"none\""));
    for loaded in &registry().entries {
        assert!(text.contains(&loaded.entry.slug), "{}", loaded.entry.slug);
    }
}

#[test]
fn parameters_leave_out_self_and_star_arguments() {
    assert_eq!(
        parameters("def f(self, a, b: list[int] = [1, 2], *args, c=3, **kw):"),
        ["a", "b", "c"]
    );
    assert_eq!(parameters("def g(x, /, *, y):"), ["x", "y"]);
    assert!(parameters("def h():").is_empty());
}

#[test]
fn base64_matches_the_standard_alphabet() {
    assert_eq!(base64(b""), "");
    assert_eq!(base64(b"f"), "Zg==");
    assert_eq!(base64(b"fo"), "Zm8=");
    assert_eq!(base64(b"foo"), "Zm9v");
    assert_eq!(base64(b"hello world"), "aGVsbG8gd29ybGQ=");
}

#[test]
fn code_finds_functions_and_methods_but_not_constructors() {
    let found = candidates(&workspace());
    let names: Vec<String> = found
        .iter()
        .map(|c| format!("{}:{}", c.file, c.qualname))
        .collect();
    assert!(names.contains(&"metrics/good.py:cosine_similarity".to_string()));
    assert!(names.contains(&"metrics/good.py:Debouncer.update".to_string()));
    assert!(names.contains(&"metrics/bad.py:mmd2_biased".to_string()));
    assert!(!names.iter().any(|n| n.ends_with("__init__")));
    assert!(found.iter().all(|c| c.digest.len() == 64));
}

async fn fixture_check(file: &str, qualname: &str, slug: &str) -> Checked {
    let root = workspace();
    let candidate = candidates(&root)
        .into_iter()
        .find(|c| c.file == file && c.qualname == qualname)
        .unwrap_or_else(|| panic!("{file} {qualname} is a candidate"));
    let host = Contained {
        workdir: root.clone(),
    };
    let entry = &registry().get(slug).expect("an entry").entry;
    check(&host, &root.display().to_string(), &candidate, entry).await
}

#[tokio::test]
async fn every_reference_passes_and_every_departure_fails_its_property() {
    if !have_python() {
        return;
    }
    for (slug, good, bad, property) in PAIRS {
        let passed = fixture_check("metrics/good.py", good, slug).await;
        assert_eq!(passed.status, Status::Ran, "{slug}: {passed:?}");
        assert_eq!(passed.verdict(), "pass", "{slug}: {passed:?}");
        let failed = fixture_check("metrics/bad.py", bad, slug).await;
        assert_eq!(failed.verdict(), "fail", "{slug}: {failed:?}");
        assert!(
            failed.failed().any(|p| p.id == property),
            "{slug} fails {property}: {failed:?}"
        );
    }
}

#[tokio::test]
async fn a_debouncer_class_is_checked_one_sample_at_a_time() {
    if !have_python() {
        return;
    }
    let checked = fixture_check("metrics/good.py", "Debouncer.update", "debounce").await;
    assert_eq!(checked.form, Some(Form::Stepwise), "{checked:?}");
    assert_eq!(checked.verdict(), "pass", "{checked:?}");
}

#[tokio::test]
async fn a_function_of_the_wrong_shape_could_not_be_called() {
    if !have_python() {
        return;
    }
    // `Debouncer.update` takes one sample; the cosine entry passes two
    // vectors, and has no one-sample-at-a-time form.
    let checked = fixture_check("metrics/good.py", "Debouncer.update", "cosine-similarity").await;
    assert_eq!(checked.status, Status::CouldNotCall, "{checked:?}");
    assert_eq!(checked.verdict(), "unknown");
    assert!(checked.error.is_some());
}

/// Recorded answers that tie each fixture function to its entry.
fn recorded_for(found: &[Candidate]) -> Recorded {
    let mut recorded = Recorded::empty();
    for request in requests(found) {
        let mut answers = serde_json::Map::new();
        for (j, id) in request.ids.iter().enumerate() {
            let c = &found[request.offset + j];
            let slug = PAIRS
                .iter()
                .find(|(_, good, bad, _)| {
                    let name = c.qualname.as_str();
                    (c.file.ends_with("good.py") && name == *good)
                        || (c.file.ends_with("bad.py") && name == *bad)
                })
                .map_or("none", |(slug, ..)| slug);
            answers.insert(id.clone(), json!({ "choice": slug }));
        }
        recorded.entries.insert(
            request_key(&request.state, &request.questions),
            RecordedAnswer {
                name: "jev_method_conformance".to_string(),
                model: "test".to_string(),
                answers: Value::Object(answers),
                input_tokens: Some(100),
                output_tokens: None,
                milliseconds: None,
                source: "test".to_string(),
            },
        );
    }
    recorded
}

#[tokio::test]
async fn a_run_turns_failed_properties_into_typed_evidence() {
    if !have_python() {
        return;
    }
    let root = workspace();
    let found = candidates(&root);
    let jev = JevMode::Recorded(recorded_for(&found));
    let host = Contained {
        workdir: root.clone(),
    };
    let report = run(
        &jev,
        &Recorder::default(),
        &Context {
            component: COMPONENT,
            id: "test".to_string(),
            deadline: None,
        },
        &root,
        &host,
        &root.display().to_string(),
    )
    .await;
    assert_eq!(report.candidates, found.len());
    assert_eq!(report.answered, found.len());
    // Recorded answers cost nothing.
    assert!(report.jev_usd.abs() < f64::EPSILON);
    for (slug, _, _, property) in PAIRS {
        assert!(
            report.failures.iter().any(|f| f.method == slug
                && f.property == property
                && f.function.contains("bad.py")),
            "{slug} {property}"
        );
    }
    assert!(
        report
            .failures
            .iter()
            .all(|f| f.function.contains("bad.py")),
        "{:?}",
        report.failures
    );
    let mmd = report
        .failures
        .iter()
        .find(|f| f.method == "mmd-squared-unbiased")
        .expect("the biased estimate fails");
    assert!(mmd.expected.contains("below"), "{mmd:?}");
    assert_eq!(
        mmd.method_digest,
        registry().get("mmd-squared-unbiased").expect("x").digest
    );
    // Checking leaves no bytecode in the workspace it observed.
    assert!(!root.join("metrics/__pycache__").exists());
    let evidence = evidence(&report).expect("a briefing section");
    assert_eq!(evidence.label, LABEL);
    assert!(evidence.text.contains("identical_below_zero"));
}

#[tokio::test]
async fn with_jev_off_nothing_is_checked() {
    let root = workspace();
    let host = Contained {
        workdir: root.clone(),
    };
    let report = run(
        &JevMode::Off,
        &Recorder::default(),
        &Context {
            component: COMPONENT,
            id: "test".to_string(),
            deadline: None,
        },
        &root,
        &host,
        &root.display().to_string(),
    )
    .await;
    assert!(report.candidates > 0);
    assert_eq!(report.answered, 0);
    assert!(report.rows.is_empty());
    assert!(evidence(&report).is_none());
}
