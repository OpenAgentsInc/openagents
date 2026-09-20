//! The numeric contract a typed answer holds to, and the request-aware check
//! that a response answered the questions it was asked. Each malformed door
//! here sends a body that reads as JSON of the right shape, so the only
//! thing between it and a typed answer is the contract.

use indexmap::IndexMap;
use jev::{Choice, Error, Noul, Questions, RawResponse, Score, SystemOneResponse};
use serde_json::{Value, json};

fn decode(answer: Value) -> jev::Result<SystemOneResponse> {
    SystemOneResponse::decode(RawResponse {
        status: 200,
        headers: Default::default(),
        bytes: serde_json::to_vec(&json!({"model":"fixture","answers":{"q":answer}})).unwrap(),
    })
}

/// The dotted path a body's first fault is named by.
fn fault(answer: Value) -> String {
    match decode(answer) {
        Err(Error::ResponseValidation { field_path, .. }) => field_path,
        Err(other) => panic!("another error: {other}"),
        Ok(_) => panic!("the answer was read"),
    }
}

fn choice(choice: &str, confidence: f64, probabilities: Value) -> Value {
    json!({"type":"choice","choice":choice,"confidence":confidence,"probabilities":probabilities})
}

fn score(score: f64, confidence: f64, probabilities: Value) -> Value {
    json!({"type":"score","score":score,"confidence":confidence,
        "legend":{"0":"calm","1":"annoyed","2":"angry"},"probabilities":probabilities})
}

#[test]
fn a_noul_is_a_probability() {
    for bad in [-2.0, 1.01, -0.0001, f64::MAX] {
        assert_eq!(fault(json!({"type":"noul","noul":bad})), "answers.q.noul");
    }
    for edge in [0.0, 1.0, 0.5] {
        assert!(decode(json!({"type":"noul","noul":edge})).is_ok());
    }
}

#[test]
fn a_choice_confidence_is_a_probability() {
    let spread = json!({"a":0.5,"b":0.5});
    assert_eq!(
        fault(choice("a", 1.5, spread.clone())),
        "answers.q.confidence"
    );
    assert_eq!(
        fault(choice("a", -0.1, spread.clone())),
        "answers.q.confidence"
    );
    assert!(decode(choice("a", 0.0, spread)).is_ok());
}

#[test]
fn a_choice_distribution_is_one_of_probabilities_that_sum_to_one() {
    assert_eq!(
        fault(choice("a", 0.5, json!({"a":-0.5,"b":1.5}))),
        "answers.q.probabilities.a"
    );
    assert_eq!(
        fault(choice("a", 0.5, json!({"a":0.9,"b":0.9}))),
        "answers.q.probabilities"
    );
    assert_eq!(
        fault(choice("a", 0.5, json!({"a":0.1,"b":0.1}))),
        "answers.q.probabilities"
    );
    assert_eq!(
        fault(choice("a", 0.5, json!({}))),
        "answers.q.probabilities"
    );
}

#[test]
fn a_rounded_distribution_is_within_tolerance() {
    // Three entries at two decimals may miss 1 by up to a cent and a half.
    assert!(decode(choice("a", 0.5, json!({"a":0.33,"b":0.33,"c":0.33}))).is_ok());
    assert!(decode(choice("a", 0.5, json!({"a":0.34,"b":0.34,"c":0.33}))).is_ok());
    assert_eq!(
        fault(choice("a", 0.5, json!({"a":0.34,"b":0.34,"c":0.34}))),
        "answers.q.probabilities"
    );
}

#[test]
fn a_choice_names_an_option_the_distribution_carries() {
    assert_eq!(
        fault(choice("c", 0.5, json!({"a":0.5,"b":0.5}))),
        "answers.q.choice"
    );
    assert_eq!(
        fault(choice("A", 0.5, json!({"a":0.5,"b":0.5}))),
        "answers.q.choice"
    );
}

#[test]
fn a_score_lies_within_its_legend() {
    let spread = json!({"0":0.2,"1":0.3,"2":0.5});
    assert_eq!(fault(score(-0.1, 0.5, spread.clone())), "answers.q.score");
    assert_eq!(fault(score(2.1, 0.5, spread.clone())), "answers.q.score");
    assert_eq!(
        fault(score(f64::NAN, 0.5, spread.clone())),
        "answers.q.score"
    );
    assert!(decode(score(0.0, 0.5, spread.clone())).is_ok());
    assert!(decode(score(2.0, 0.5, spread.clone())).is_ok());
    assert!(decode(score(1.3, 0.5, spread)).is_ok());
}

#[test]
fn a_score_confidence_is_a_probability() {
    let spread = json!({"0":0.2,"1":0.3,"2":0.5});
    assert_eq!(fault(score(1.0, 1.2, spread)), "answers.q.confidence");
}

#[test]
fn a_score_distribution_names_only_legend_levels_and_sums_to_one() {
    assert_eq!(
        fault(score(1.0, 0.5, json!({"0":0.2,"1":0.3,"3":0.5}))),
        "answers.q.probabilities.3"
    );
    assert_eq!(
        fault(score(1.0, 0.5, json!({"0":0.2,"1":1.3,"2":0.5}))),
        "answers.q.probabilities.1"
    );
    assert_eq!(
        fault(score(1.0, 0.5, json!({"0":0.2,"1":0.3,"2":0.2}))),
        "answers.q.probabilities"
    );
    // The API leaves the distribution off some Score answers.
    assert!(decode(score(1.0, 0.5, json!({}))).is_ok());
}

#[test]
fn a_score_needs_levels_from_its_legend_or_its_distribution() {
    assert_eq!(
        fault(json!({"type":"score","score":0.0,"confidence":0.5,"legend":{}})),
        "answers.q.legend"
    );
    // A door asked with level indices alone sends no legend; the
    // distribution names the levels, and the score is held to them.
    let indexed = |score: f64| {
        json!({"type":"score","score":score,"confidence":0.5,"legend":{},
            "probabilities":{"0":0.5,"1":0.375,"2":0.125}})
    };
    assert!(decode(indexed(0.62)).is_ok());
    assert_eq!(fault(indexed(2.5)), "answers.q.score");
}

#[test]
fn a_faulty_answer_leaves_the_rest_of_the_body_unread() {
    // The bytes are the caller's to inspect; the typed answers are not built.
    let Err(Error::ResponseValidation { body, .. }) = decode(json!({"type":"noul","noul":-2.0}))
    else {
        panic!("the answer was read");
    };
    assert!(body.is_some());
}

/// The questions the documented response answers, with the options the
/// documented response carries.
fn asked() -> Questions {
    let mut options = IndexMap::new();
    for option in ["billing", "technical", "sales"] {
        options.insert(option.to_string(), None);
    }
    Questions::new()
        .with("department", Choice::new("Which team?", options))
        .with(
            "frustration",
            Score::new("How upset?", vec![None, None, None]),
        )
        .with("is_urgent", Noul::new("Urgent?"))
}

fn documented() -> SystemOneResponse {
    SystemOneResponse::decode(RawResponse {
        status: 200,
        headers: Default::default(),
        bytes: serde_json::to_vec(&json!({
            "model": "jev-latest",
            "answers": {
                "department": {
                    "type": "choice",
                    "choice": "technical",
                    "probabilities": {"billing": 0.08, "technical": 0.85, "sales": 0.07},
                    "confidence": 0.82,
                },
                "frustration": {
                    "type": "score",
                    "score": 1.6,
                    "legend": {"0": "Calm", "1": "Frustrated", "2": "Very angry"},
                    "probabilities": {"0": 0.05, "1": 0.3, "2": 0.65},
                    "confidence": 0.78,
                },
                "is_urgent": {"type": "noul", "noul": 0.92},
            },
        }))
        .unwrap(),
    })
    .unwrap()
}

#[test]
fn the_documented_response_answers_the_documented_questions() {
    documented().check_against(&asked()).unwrap();
}

#[test]
fn a_question_without_an_answer_is_missing() {
    let questions = asked().with("also", Noul::new("And this?"));
    let error = documented().check_against(&questions).unwrap_err();
    assert!(matches!(error, Error::MissingAnswer { id } if id == "also"));
}

#[test]
fn a_question_answered_in_another_type_is_a_type_error() {
    let mut options = IndexMap::new();
    options.insert("yes".to_string(), None);
    let questions = Questions::new().with("is_urgent", Choice::new("Urgent?", options));
    let error = documented().check_against(&questions).unwrap_err();
    assert!(matches!(
        error,
        Error::AnswerType { id, expected: "choice", found: "noul" } if id == "is_urgent"
    ));
}

#[test]
fn a_choice_answered_over_other_options_names_the_stranger() {
    let mut options = IndexMap::new();
    for option in ["billing", "technical"] {
        options.insert(option.to_string(), None);
    }
    let questions = Questions::new().with("department", Choice::new("Which team?", options));
    let error = documented().check_against(&questions).unwrap_err();
    assert!(matches!(
        error,
        Error::ResponseValidation { field_path, .. }
            if field_path == "answers.department.probabilities.sales"
    ));
}

#[test]
fn a_choice_answered_over_fewer_options_is_a_fault_too() {
    let mut options = IndexMap::new();
    for option in ["billing", "technical", "sales", "legal"] {
        options.insert(option.to_string(), None);
    }
    let questions = Questions::new().with("department", Choice::new("Which team?", options));
    let error = documented().check_against(&questions).unwrap_err();
    assert!(matches!(
        error,
        Error::ResponseValidation { field_path, .. }
            if field_path == "answers.department.probabilities"
    ));
}

#[test]
fn a_score_answered_over_other_levels_is_a_fault() {
    let questions =
        Questions::new().with("frustration", Score::new("How upset?", vec![None, None]));
    let error = documented().check_against(&questions).unwrap_err();
    assert!(matches!(
        error,
        Error::ResponseValidation { field_path, .. } if field_path == "answers.frustration.legend"
    ));
}
