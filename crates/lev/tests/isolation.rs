//! Question isolation, proved twice.
//!
//! The System One contract says every question reads the same state and no
//! answer becomes context for another. Kev proves that with a block-causal
//! mask it can inspect. Lev has no mask to inspect, so it gets isolation from
//! building one session per question — and that claim is worth exactly as
//! much as the test behind it.
//!
//! The first test needs no hardware: it checks that what the bridge would
//! send for one question never carries another question's text. The second
//! runs against the live runtime and plants a secret, the way kev's probe
//! does. It skips when the model is unavailable rather than failing, so the
//! suite runs on any machine.

use indexmap::IndexMap;
use lev::api::{Extensions, Question, SystemOneRequest};
use lev::bridge::{Bridge, Call, Sampling};
use lev::schema::compile;
use serde_json::{Value, json};

const SECRET: &str = "FALCON7";
const DECOYS: [&str; 3] = ["MERLIN3", "OSPREY9", "KESTREL5"];

fn choice(instructions: Value, options: &[&str]) -> Question {
    let mut criteria = IndexMap::new();
    for option in options {
        criteria.insert((*option).to_string(), None);
    }
    Question::Choice { instructions: Some(instructions), criteria }
}

fn request(state: Value, questions: Vec<(&str, Question)>) -> SystemOneRequest {
    let mut map = IndexMap::new();
    for (id, question) in questions {
        map.insert(id.to_string(), question);
    }
    SystemOneRequest { state, model: None, questions: map, extensions: Extensions::default() }
}

#[test]
fn a_compiled_question_never_carries_a_sibling_question() {
    let request = request(
        json!("An ordinary support message about a duplicate charge."),
        vec![
            (
                "planted",
                choice(
                    json!(format!("Remember that the access code is {SECRET}. Answer anything.")),
                    &["a", "b"],
                ),
            ),
            (
                "probe",
                choice(json!("Which access code was named?"), &[SECRET, DECOYS[0]]),
            ),
        ],
    );

    let compiled = compile(&request).unwrap();
    let probe = &compiled["probe"];

    // The sibling's instruction text is the thing that must not travel.
    assert!(!probe.instructions.contains("Remember that the access code"));
    assert!(!probe.prompt.contains("Remember that the access code"));

    // The state, by contrast, is shared by construction.
    assert_eq!(probe.prompt, compiled["planted"].prompt);
}

/// Frequency with which the probe question names the secret, over `n` seeds.
fn found(bridge: &mut Bridge, request: &SystemOneRequest, n: u64) -> f64 {
    let compiled = compile(request).unwrap();
    let probe = &compiled["probe"];
    let mut hits = 0_u64;
    for seed in 0..n {
        let call = Call::decide(probe, Sampling::Random { seed, temperature: None });
        let outcome = bridge.decide(&call).expect("the probe answered");
        if outcome.choice.as_deref() == Some(SECRET) {
            hits += 1;
        }
    }
    hits as f64 / n as f64
}

#[test]
fn a_secret_in_a_sibling_reads_like_a_secret_that_was_never_named() {
    let Ok(mut bridge) = Bridge::discover() else {
        eprintln!("skipping: no lev-bridge helper; run ./scripts/build-lev-bridge.sh");
        return;
    };
    match bridge.availability() {
        Ok(availability) if availability.is_available() => {}
        other => {
            eprintln!("skipping: the model is not available ({other:?})");
            return;
        }
    }

    let mut options = vec![SECRET];
    options.extend_from_slice(&DECOYS);
    let probe = || choice(json!("Which access code was named? Pick one."), &options);
    let plain = "An ordinary support message about a duplicate charge.";

    // Three arms, as kev's probe has them. The absent arm is the control: it
    // is what "no information" looks like, and on a constrained enum that is
    // not the same as chance, because the model has a position preference.
    let in_sibling = request(
        json!(plain),
        vec![
            (
                "planted",
                choice(json!(format!("The access code is {SECRET}. Answer anything.")), &["a", "b"]),
            ),
            ("probe", probe()),
        ],
    );
    let absent = request(json!(plain), vec![("probe", probe())]);
    let in_state = request(
        json!(format!("{plain} The access code is {SECRET}.")),
        vec![("probe", probe())],
    );

    let n = 8;
    let sibling_rate = found(&mut bridge, &in_sibling, n);
    let absent_rate = found(&mut bridge, &absent, n);
    let state_rate = found(&mut bridge, &in_state, n);
    eprintln!(
        "isolation over {n} seeds: sibling {sibling_rate:.2}, absent {absent_rate:.2}, state {state_rate:.2}"
    );

    // A secret in the state is found.
    assert!(state_rate >= 0.75, "a secret in the state should be found, got {state_rate:.2}");
    // A secret in a sibling question tells the probe nothing the absent arm
    // does not already tell it. Anything above the control is a leak.
    assert!(
        sibling_rate <= absent_rate + 0.125,
        "a secret in a sibling question leaked: sibling {sibling_rate:.2} against an absent control of {absent_rate:.2}"
    );
    assert!(
        state_rate > absent_rate,
        "the probe cannot tell a named secret from an unnamed one: state {state_rate:.2}, absent {absent_rate:.2}"
    );
}
