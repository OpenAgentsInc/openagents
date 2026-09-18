//! Questions: what a builder makes, what it serializes to, and what the checks
//! refuse before any request.

use indexmap::IndexMap;
use jev::{Choice, Entry, Error, Noul, NoulCriteria, Question, Questions, Score};
use serde::Serialize;
use serde_json::{Value, json};

type Outcome = Result<(), Box<dyn std::error::Error>>;

#[test]
fn an_entry_takes_the_four_shapes_the_api_reads() -> Outcome {
    assert_eq!(Entry::from("text").to_value(), json!("text"));
    assert_eq!(
        Entry::from(json!({"field": 1})).to_value(),
        json!({"field": 1})
    );
    assert_eq!(Entry::from(json!(["one"])).to_value(), json!(["one"]));
    assert!(Entry::from(Value::Null).is_null());
    assert_eq!(Entry::default(), Entry::Null);
    Ok(())
}

/// The API reads text, an object, an array, or nothing. A number or a boolean
/// reaches it as text rather than as a shape it does not read.
#[test]
fn a_bare_scalar_becomes_text() -> Outcome {
    assert_eq!(Entry::from(json!(7)).to_value(), json!("7"));
    assert_eq!(Entry::from(json!(true)).to_value(), json!("true"));
    Ok(())
}

#[test]
fn an_entry_takes_anything_that_serializes() -> Outcome {
    #[derive(Serialize)]
    struct Ticket {
        id: u32,
        subject: String,
    }

    let entry = Entry::json(&Ticket {
        id: 7,
        subject: "Double charge".to_string(),
    })?;
    assert_eq!(
        entry.to_value(),
        json!({"id": 7, "subject": "Double charge"})
    );
    Ok(())
}

#[test]
fn a_noul_serializes_to_the_wire_shape() -> Outcome {
    let question = Noul::new("Does the customer ask for money back?");
    assert_eq!(
        serde_json::to_value(&question)?,
        json!({
            "type": "noul",
            "instructions": "Does the customer ask for money back?",
        })
    );

    let described = Noul::with_criteria(
        "Does the customer ask for money back?",
        NoulCriteria::new()
            .when_true("An explicit request for a refund")
            .when_false(Entry::Null),
    );
    assert_eq!(
        serde_json::to_value(&described)?,
        json!({
            "type": "noul",
            "instructions": "Does the customer ask for money back?",
            "criteria": {
                "true": "An explicit request for a refund",
                "false": null,
            },
        })
    );
    Ok(())
}

#[test]
fn a_choice_keeps_the_order_its_options_were_added_in() -> Outcome {
    let question = Choice::new("Which team handles this?", IndexMap::new())
        .option("billing", "Charges, invoices, and refunds")
        .option("technical", json!(["Bugs", "Outages"]))
        .bare_option("other");
    let wire = serde_json::to_string(&question)?;
    let billing = wire.find("billing").ok_or("the options reach the wire")?;
    let technical = wire.find("technical").ok_or("the options reach the wire")?;
    let other = wire.find("other").ok_or("the options reach the wire")?;
    assert!(billing < technical && technical < other, "{wire}");
    assert_eq!(
        serde_json::to_value(&question)?,
        json!({
            "type": "choice",
            "instructions": "Which team handles this?",
            "criteria": {
                "billing": "Charges, invoices, and refunds",
                "technical": ["Bugs", "Outages"],
                "other": null,
            },
        })
    );
    Ok(())
}

#[test]
fn a_score_serializes_its_levels_in_order() -> Outcome {
    let question = Score::new(json!(["How severe is the issue?"]), Vec::new())
        .level(json!({"meaning": "Cosmetic"}))
        .level("Blocking");
    assert_eq!(
        serde_json::to_value(&question)?,
        json!({
            "type": "score",
            "instructions": ["How severe is the issue?"],
            "criteria": [{"meaning": "Cosmetic"}, "Blocking"],
        })
    );

    let with_gap = Score::new("How severe?", vec![None, Some(Entry::from("Blocking"))]);
    assert_eq!(
        serde_json::to_value(&with_gap)?,
        json!({
            "type": "score",
            "instructions": "How severe?",
            "criteria": [null, "Blocking"],
        })
    );
    Ok(())
}

#[test]
fn a_question_written_out_as_json_is_sent_as_it_stands() -> Outcome {
    let raw = Question::Raw(json!({"type": "noul", "instructions": "Is it urgent?", "future": 1}));
    assert_eq!(raw.kind(), Some("noul"));
    assert_eq!(
        serde_json::to_value(&raw)?,
        json!({"type": "noul", "instructions": "Is it urgent?", "future": 1})
    );
    Ok(())
}

#[test]
fn a_question_set_keeps_its_order_and_names_its_types() -> Outcome {
    let mut questions = Questions::new()
        .with("refund", Noul::new("Money back?"))
        .with(
            "urgency",
            Score::new("How urgent?", Vec::new())
                .level("Calm")
                .level("Urgent"),
        );
    assert_eq!(questions.len(), 2);
    assert!(!questions.is_empty());
    assert_eq!(
        questions.iter().map(|(id, _)| id).collect::<Vec<_>>(),
        ["refund", "urgency"]
    );
    assert_eq!(
        questions.get("refund").and_then(Question::kind),
        Some("noul")
    );
    let replaced = questions.insert("refund", Noul::new("Refund asked for?"));
    assert!(matches!(replaced, Some(Question::Noul(_))));
    assert_eq!(questions.len(), 2);
    assert_eq!(
        serde_json::to_value(&questions)?["refund"]["instructions"],
        json!("Refund asked for?")
    );
    Ok(())
}

#[test]
fn a_set_built_from_an_iterator_reads_the_same() -> Outcome {
    let questions: Questions = [("refund", Noul::new("Money back?"))].into_iter().collect();
    assert_eq!(questions.len(), 1);
    questions.validate()?;
    Ok(())
}

#[test]
fn a_request_asks_at_least_one_question() {
    let Err(Error::Question { id, message }) = Questions::new().validate() else {
        unreachable!("an empty set fails its checks");
    };
    assert_eq!(id, "");
    assert!(message.contains("at least one question"), "{message}");
}

#[test]
fn a_score_names_two_to_ten_levels() {
    let one = Questions::new().with(
        "urgency",
        Score::new("How urgent?", Vec::new()).level("Calm"),
    );
    let Err(Error::Question { id, message }) = one.validate() else {
        unreachable!("one level fails its checks");
    };
    assert_eq!(id, "urgency");
    assert!(message.contains("at least 2 levels"), "{message}");

    let mut eleven = Score::new("How urgent?", Vec::new());
    for level in 0..11 {
        eleven = eleven.level(format!("level {level}"));
    }
    let Err(Error::Question { id, message }) = Questions::new().with("urgency", eleven).validate()
    else {
        unreachable!("eleven levels fail their checks");
    };
    assert_eq!(id, "urgency");
    assert!(message.contains("at most 10 levels"), "{message}");
}

#[test]
fn a_choice_names_at_most_255_options() {
    let mut question = Choice::new("Which line?", IndexMap::new());
    for option in 0..256 {
        question = question.bare_option(format!("option-{option}"));
    }
    let Err(Error::Question { id, message }) = Questions::new().with("line", question).validate()
    else {
        unreachable!("256 options fail their checks");
    };
    assert_eq!(id, "line");
    assert!(message.contains("at most 255 options"), "{message}");
}

#[test]
fn a_question_written_out_as_json_names_a_type_and_its_criteria() {
    let untyped = Questions::new().with("odd", json!({"instructions": "What?"}));
    assert!(matches!(untyped.validate(), Err(Error::Question { .. })));

    let bare = Questions::new().with("odd", json!({"type": "score", "instructions": "What?"}));
    let Err(Error::Question { message, .. }) = bare.validate() else {
        unreachable!("a score written out as JSON names its criteria");
    };
    assert!(message.contains("criteria"), "{message}");

    let short = Questions::new().with("odd", json!({"type": "score", "criteria": ["only"]}));
    assert!(matches!(short.validate(), Err(Error::Question { .. })));

    let sound = Questions::new().with("odd", json!({"type": "score", "criteria": ["a", "b"]}));
    assert!(sound.validate().is_ok());
}

/// The Python SDK's list of question dictionaries it refuses before a
/// request: no type, an empty type, a type that is not a string, and a Choice
/// or Score without criteria.
#[test]
fn a_question_written_out_as_json_fails_the_checks_the_python_sdk_runs() {
    let invalid = [
        json!({}),
        json!({"instructions": "?"}),
        json!({"type": ""}),
        json!({"type": null}),
        json!({"type": 1}),
        json!({"type": ["future"]}),
        json!("noul"),
        json!({"type": "choice"}),
        json!({"type": "score"}),
        json!({"type": "score", "criteria": []}),
        json!({"type": "score", "criteria": {"a": 1, "b": 2}}),
    ];
    for body in invalid {
        let set = Questions::new().with("odd", Question::Raw(body.clone()));
        assert!(
            matches!(set.validate(), Err(Error::Question { .. })),
            "{body} fails its checks"
        );
    }
}

/// What the checks do not know is left to the API: a type the SDK does not
/// name, extra fields, and a criteria shape only the API judges.
#[test]
fn a_question_the_checks_do_not_know_is_left_to_the_api() -> Outcome {
    let passthrough = [
        json!({"type": "noul", "instructions": null, "weight": 3, "nested": {"k": null}}),
        json!({"type": "choice", "criteria": {"a": null}, "weight": 2}),
        json!({"type": "future-type", "instructions": "?"}),
        json!({"type": "choice", "criteria": ["a"]}),
    ];
    for body in passthrough {
        let set = Questions::new().with("odd", Question::Raw(body.clone()));
        set.validate()?;
        assert_eq!(
            serde_json::to_value(&set)?["odd"],
            body,
            "sent as it stands"
        );
    }
    Ok(())
}

/// A Noul with criteria but no descriptions sends an empty criteria object.
#[test]
fn an_empty_noul_criteria_sends_an_empty_object() -> Outcome {
    let question = Noul::with_criteria("Is it urgent?", NoulCriteria::new());
    assert_eq!(
        serde_json::to_value(&question)?,
        json!({"type": "noul", "instructions": "Is it urgent?", "criteria": {}})
    );
    Ok(())
}

/// The error names the question and never the whole set, so a log line says
/// which question to fix.
#[test]
fn a_question_failure_reads_with_its_id() {
    let error = Error::Question {
        id: "urgency".to_string(),
        message: "a Score question names at least 2 levels".to_string(),
    };
    assert_eq!(
        error.to_string(),
        "question \"urgency\": a Score question names at least 2 levels"
    );
    let whole = Error::Question {
        id: String::new(),
        message: "a request asks at least one question".to_string(),
    };
    assert_eq!(whole.to_string(), "a request asks at least one question");
}
