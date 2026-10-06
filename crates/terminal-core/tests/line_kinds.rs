//! The structural fallback's record on the held-out line-kind cases
//! (`fixtures/line-kinds.json`, #10693). The floor below is the measured
//! result; a change to `route::classify` that drops under it is a
//! regression, and one that rises above it updates the floor and the
//! record in `docs/terminal/2026-10-06-optional-research.md`.

use std::collections::BTreeMap;

use terminal_core::route::{Route, Word, classify};

fn word(name: &str) -> Word {
    match name {
        "alias" => Word::Alias,
        "builtin" => Word::Builtin,
        "function" => Word::Function,
        "command" => Word::Command,
        "reserved" => Word::Reserved,
        "missing" => Word::Missing,
        _ => Word::Unknown,
    }
}

#[test]
fn the_structural_fallback_holds_its_measured_floor() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/line-kinds.json"
    ))
    .unwrap();
    let fixture: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(fixture["v"], "openagents.terminal-line-kinds.v1");
    // class -> (cases, correct, unsure, unsure and wrong)
    let mut classes: BTreeMap<String, (u32, u32, u32, u32)> = BTreeMap::new();
    for case in fixture["cases"].as_array().unwrap() {
        let line = case["line"].as_str().unwrap();
        let expected = match case["kind"].as_str().unwrap() {
            "shell" => Route::Shell,
            _ => Route::Ask,
        };
        let decision = classify(line, word(case["first"].as_str().unwrap()));
        let entry = classes
            .entry(case["class"].as_str().unwrap().to_owned())
            .or_default();
        let right = decision.route == expected;
        entry.0 += 1;
        entry.1 += u32::from(right);
        entry.2 += u32::from(!decision.sure);
        entry.3 += u32::from(!decision.sure && !right);
        if !right || !decision.sure {
            println!(
                "{}: {line:?} -> {}",
                if right { "unsure" } else { "miss" },
                decision.label()
            );
        }
    }
    let (mut cases, mut correct) = (0, 0);
    for (class, (n, right, unsure, unsure_wrong)) in &classes {
        println!("{class}: {right}/{n} right, {unsure} unsure ({unsure_wrong} of them wrong)");
        cases += n;
        correct += right;
    }
    println!("all: {correct}/{cases}");
    assert_eq!(cases, 68);
    assert!(correct >= FLOOR, "{correct} under the floor {FLOOR}");
}

/// Correct structural decisions measured on 2026-10-06.
const FLOOR: u32 = 57;
