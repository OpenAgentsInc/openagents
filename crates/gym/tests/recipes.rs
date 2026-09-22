//! The recipe library's contract: every `recipes/*.recipe.json` is a
//! versioned `openagents.recipe.v1` document whose question definitions
//! are typed, whose composition references only its own questions, and
//! whose fixtures the composition actually produces. Composition is
//! deterministic — the model supplies answers; these rules never do.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::Value;

const AREAS: &[&str] = &[
    "filtering-and-retrieval",
    "support-and-trust",
    "operations",
    "agent-composition",
    "business-and-knowledge",
    "personal-workflows",
];

const LABELS: &[&str] = &["measured", "assessed", "unmeasured"];

fn recipes_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../recipes")
}

fn load(path: &Path) -> Value {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|trouble| panic!("{}: {trouble}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|trouble| panic!("{}: invalid JSON: {trouble}", path.display()))
}

/// The question names a compose expression may touch.
fn compose_questions(compose: &Value, out: &mut BTreeSet<String>) {
    match compose {
        Value::Object(map) => {
            if let Some(question) = map.get("answer").and_then(Value::as_str) {
                out.insert(question.to_string());
            }
            if let Some(question) = map.get("value").and_then(Value::as_str) {
                out.insert(question.to_string());
            }
            for value in map.values() {
                compose_questions(value, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                compose_questions(item, out);
            }
        }
        _ => {}
    }
}

/// Is a compose result boolean, for `all`/`any`/`if` positions?
fn truthy(value: &Value) -> Result<bool, String> {
    value
        .as_bool()
        .ok_or_else(|| format!("expected a boolean position, got {value}"))
}

/// Evaluate a compose expression over a fixture's recorded answers.
fn evaluate(compose: &Value, answers: &Value) -> Result<Value, String> {
    let Value::Object(map) = compose else {
        return Ok(compose.clone());
    };
    if let Some(question) = map.get("answer").and_then(Value::as_str) {
        return answers
            .get(question)
            .and_then(|answer| answer.get("answer"))
            .cloned()
            .ok_or_else(|| format!("no recorded answer for {question}"));
    }
    if let Some(items) = map.get("all").and_then(Value::as_array) {
        for item in items {
            if !truthy(&evaluate(item, answers)?)? {
                return Ok(Value::Bool(false));
            }
        }
        return Ok(Value::Bool(true));
    }
    if let Some(items) = map.get("any").and_then(Value::as_array) {
        for item in items {
            if truthy(&evaluate(item, answers)?)? {
                return Ok(Value::Bool(true));
            }
        }
        return Ok(Value::Bool(false));
    }
    if let Some(item) = map.get("not") {
        return Ok(Value::Bool(!truthy(&evaluate(item, answers)?)?));
    }
    if let Some(pair) = map.get("gte").and_then(Value::as_array) {
        let [reference, bound] = pair.as_slice() else {
            return Err("gte takes [value-ref, bound]".into());
        };
        let question = reference["value"]
            .as_str()
            .ok_or("gte's first element is {value: name}")?;
        let reported = answers
            .get(question)
            .and_then(|answer| answer.get("answer"))
            .and_then(Value::as_f64)
            .ok_or_else(|| format!("no recorded score for {question}"))?;
        let bound = bound.as_f64().ok_or("gte's bound is not a number")?;
        return Ok(Value::Bool(reported >= bound));
    }
    if let Some(branch) = map.get("if") {
        let cond = evaluate(&branch["cond"], answers)?;
        let picked = if truthy(&cond)? {
            &branch["then"]
        } else {
            &branch["else"]
        };
        return evaluate(picked, answers);
    }
    Err(format!("unknown compose form: {compose}"))
}

/// Validate one recipe document; failures panic with the file's name.
fn check(path: &Path) {
    let recipe = load(path);
    let name = path.file_name().unwrap().to_string_lossy().to_string();
    let require = |field: &str| -> &Value {
        recipe
            .get(field)
            .unwrap_or_else(|| panic!("{name}: missing `{field}`"))
    };

    assert_eq!(recipe["v"], "openagents.recipe.v1", "{name}: schema tag");
    assert!(
        recipe["id"]
            .as_str()
            .unwrap()
            .starts_with("openagents.recipe."),
        "{name}: id is not an openagents.recipe.* name"
    );
    assert!(
        recipe["version"].as_u64().unwrap_or(0) >= 1,
        "{name}: version must be a positive integer"
    );
    assert!(
        AREAS.contains(&recipe["area"].as_str().unwrap_or_default()),
        "{name}: area {:?} is not a declared recipe area",
        recipe["area"]
    );
    for field in ["name", "summary", "license"] {
        assert!(
            require(field).as_str().is_some_and(|s| !s.is_empty()),
            "{name}: `{field}` is required"
        );
    }
    assert!(
        require("source")
            .get("author")
            .and_then(Value::as_str)
            .is_some(),
        "{name}: source.author is required"
    );

    // Questions: typed, instructed, with options where a choice needs them.
    let questions = require("questions")
        .as_object()
        .unwrap_or_else(|| panic!("{name}: questions is not an object"));
    assert!(!questions.is_empty(), "{name}: no questions defined");
    for (question, definition) in questions {
        let kind = definition["type"].as_str().unwrap_or_default();
        assert!(
            ["noul", "choice", "score"].contains(&kind),
            "{name}: question {question} has unknown type {kind:?}"
        );
        assert!(
            definition["instructions"]
                .as_str()
                .is_some_and(|s| !s.is_empty()),
            "{name}: question {question} carries no instructions"
        );
        if kind == "choice" {
            assert!(
                definition.get("options").is_some(),
                "{name}: choice question {question} names no options"
            );
        }
    }

    // Compose: present, and every question it names is defined.
    let mut touched = BTreeSet::new();
    compose_questions(require("compose"), &mut touched);
    for question in &touched {
        assert!(
            questions.contains_key(question),
            "{name}: compose names undefined question {question}"
        );
    }

    // Fixtures: at least two (a positive and a negative case), each
    // carrying recorded answers the composition consumes and the
    // expected output it must produce.
    let fixtures = require("fixtures")
        .as_array()
        .unwrap_or_else(|| panic!("{name}: fixtures is not an array"));
    assert!(
        fixtures.len() >= 2,
        "{name}: fewer than two fixtures — a single case proves nothing"
    );
    for fixture in fixtures {
        let fixture_name = fixture["name"].as_str().unwrap_or("?");
        for question in &touched {
            assert!(
                fixture["answers"].get(question).is_some(),
                "{name}: fixture {fixture_name} lacks a recorded answer for {question}"
            );
        }
        let produced = evaluate(require("compose"), &fixture["answers"])
            .unwrap_or_else(|trouble| panic!("{name}: fixture {fixture_name}: {trouble}"));
        assert_eq!(
            produced, fixture["expected"],
            "{name}: fixture {fixture_name} expected {} but composition produced {produced}",
            fixture["expected"]
        );
    }

    // Evidence: a declared label; `measured` must pin a report that exists.
    let label = require("evidence")["label"].as_str().unwrap_or_default();
    assert!(
        LABELS.contains(&label),
        "{name}: unknown evidence label {label:?}"
    );
    if label == "measured" {
        let report = recipe["evidence"]["report"]
            .as_str()
            .unwrap_or_else(|| panic!("{name}: measured evidence names no report"));
        assert!(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../")
                .join(report)
                .exists(),
            "{name}: measured evidence pins {report} which does not exist"
        );
    }

    // The remaining required disclosures.
    for field in [
        "input",
        "outputs",
        "cost",
        "uncertainty",
        "limits",
        "examples",
    ] {
        require(field);
    }
    assert!(
        require("cost")
            .get("questions_per_item")
            .and_then(Value::as_u64)
            .is_some_and(|n| n >= 1),
        "{name}: cost.questions_per_item must be a positive integer"
    );
    assert!(
        require("uncertainty")
            .get("abstain")
            .and_then(Value::as_str)
            .is_some()
            && require("uncertainty")
                .get("refusal")
                .and_then(Value::as_str)
                .is_some(),
        "{name}: uncertainty must state abstain and refusal behavior"
    );
    assert!(
        require("limits").as_array().is_some_and(|l| !l.is_empty()),
        "{name}: limits must name at least one honest limit"
    );
}

#[test]
fn every_recipe_is_versioned_typed_and_fixture_verified() {
    let dir = recipes_dir();
    let mut count = 0usize;
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|trouble| panic!("{}: {trouble}", dir.display()))
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    entries.sort();
    for path in &entries {
        check(path);
        count += 1;
    }
    // The library covers every declared area.
    assert!(count >= 18, "only {count} recipes — the library shrank");
    let covered: BTreeSet<String> = entries
        .iter()
        .map(|path| load(path)["area"].as_str().unwrap_or_default().to_string())
        .collect();
    for area in AREAS {
        assert!(covered.contains(*area), "no recipe covers area {area}");
    }
}
