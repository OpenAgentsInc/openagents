//! Answers: what the documented response shapes decode to, what an unknown
//! answer type does, and which field a body that does not read names.

use jev::{Answer, Error, RawResponse, ResponseBody, SystemOneResponse};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::{Value, json};

type Outcome = Result<(), Box<dyn std::error::Error>>;

/// The recorded response of a real `POST /v1/systemone`, beside the request
/// that produced it.
const RECORDED_RESPONSE: &str = include_str!("fixtures/systemone-response.json");

/// One response with a request id, as the API sends it.
fn raw(body: &Value) -> Result<RawResponse, Box<dyn std::error::Error>> {
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("x-typesafe-request-id"),
        HeaderValue::from_static("req_01test"),
    );
    Ok(RawResponse {
        status: 200,
        headers,
        bytes: serde_json::to_vec(body)?,
    })
}

/// The response the documentation shows: one Choice, one Score, one Noul.
fn documented() -> Value {
    json!({
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
        "usage": {"input_tokens": 312, "output_tokens": 48},
    })
}

#[test]
fn the_documented_response_decodes() -> Outcome {
    let response = SystemOneResponse::decode(raw(&documented())?)?;
    assert_eq!(response.model, "jev-latest");
    assert_eq!(response.usage.input_tokens, Some(312));
    assert_eq!(response.usage.output_tokens, Some(48));
    assert_eq!(response.request_id(), Some("req_01test"));
    assert_eq!(response.raw().status, 200);

    let department = response.choice("department")?;
    assert_eq!(department.choice, "technical");
    assert!((department.confidence - 0.82).abs() < 1e-9);
    assert_eq!(department.probabilities.get("technical"), Some(&0.85));

    let frustration = response.score("frustration")?;
    assert!((frustration.score - 1.6).abs() < 1e-9);
    assert_eq!(frustration.probabilities.get(&2), Some(&0.65));
    assert_eq!(
        frustration.legend.get(&0).map(jev::Entry::to_value),
        Some(json!("Calm"))
    );

    assert!((response.noul("is_urgent")?.noul - 0.92).abs() < 1e-9);
    Ok(())
}

/// The answers and their probabilities keep the order the response sent them
/// in, so a caller that walks them reads them the way the API wrote them. The
/// body here is text rather than a built value, because building one sorts the
/// keys before they reach the wire.
#[test]
fn the_answers_keep_the_order_the_response_sent() -> Outcome {
    let body = br#"{
        "model": "jev-latest",
        "answers": {
            "zebra": {"type": "noul", "noul": 0.1},
            "apple": {
                "type": "choice",
                "choice": "second",
                "confidence": 0.5,
                "probabilities": {"third": 0.2, "first": 0.3, "second": 0.5}
            }
        },
        "usage": {}
    }"#;
    let response = SystemOneResponse::decode(RawResponse {
        status: 200,
        headers: HeaderMap::new(),
        bytes: body.to_vec(),
    })?;
    assert_eq!(
        response.answers.keys().collect::<Vec<_>>(),
        ["zebra", "apple"]
    );
    assert_eq!(
        response
            .answers
            .values()
            .map(Answer::kind)
            .collect::<Vec<_>>(),
        ["noul", "choice"]
    );
    assert_eq!(
        response
            .choice("apple")?
            .probabilities
            .keys()
            .collect::<Vec<_>>(),
        ["third", "first", "second"]
    );
    Ok(())
}

#[test]
fn the_filtered_views_hold_one_type_each() -> Outcome {
    let response = SystemOneResponse::decode(raw(&documented())?)?;
    assert_eq!(
        response.nouls().map(|(id, _)| id).collect::<Vec<_>>(),
        ["is_urgent"]
    );
    assert_eq!(
        response.choices().map(|(id, _)| id).collect::<Vec<_>>(),
        ["department"]
    );
    assert_eq!(
        response.scores().map(|(id, _)| id).collect::<Vec<_>>(),
        ["frustration"]
    );
    Ok(())
}

#[test]
fn a_typed_accessor_names_the_type_it_found_and_the_id_it_missed() -> Outcome {
    let response = SystemOneResponse::decode(raw(&documented())?)?;
    let Err(Error::AnswerType {
        id,
        expected,
        found,
    }) = response.noul("department")
    else {
        unreachable!("a Choice answer is not a Noul answer");
    };
    assert_eq!(
        (id.as_str(), expected, found),
        ("department", "noul", "choice")
    );

    let Err(Error::MissingAnswer { id }) = response.score("absent") else {
        unreachable!("an id the response does not carry is missing");
    };
    assert_eq!(id, "absent");
    Ok(())
}

/// A Score answer without `probabilities` reads, with an empty map, the way the
/// quick start's sample response shows it.
#[test]
fn a_score_without_probabilities_decodes() -> Outcome {
    let body = json!({
        "model": "jev-1.12",
        "answers": {
            "tone": {
                "type": "score",
                "score": 0.5,
                "confidence": 0.6,
                "legend": {"0": "Calm", "1": "Angry"},
            },
        },
        "usage": {},
    });
    let response = SystemOneResponse::decode(raw(&body)?)?;
    assert!(response.score("tone")?.probabilities.is_empty());
    assert_eq!(response.usage.input_tokens, None);
    Ok(())
}

/// A response without `usage` reads. The API leaves the counts out on some
/// answers, and both official SDKs treat them as optional.
#[test]
fn a_response_without_usage_decodes() -> Outcome {
    let body = json!({
        "model": "jev-latest",
        "answers": {"urgent": {"type": "noul", "noul": 0.1}},
    });
    let response = SystemOneResponse::decode(raw(&body)?)?;
    assert_eq!(response.usage.output_tokens, None);
    Ok(())
}

/// An answer type this crate does not model is skipped, and the rest of the
/// response still reads, so a newer API does not break an older client.
#[test]
fn an_unknown_answer_type_is_skipped() -> Outcome {
    let body = json!({
        "model": "jev-latest",
        "answers": {
            "kept": {"type": "noul", "noul": 0.4},
            "added_later": {"type": "constellation", "shape": "triangle"},
        },
        "usage": {"input_tokens": 1, "output_tokens": 2},
    });
    let response = SystemOneResponse::decode(raw(&body)?)?;
    assert_eq!(response.answers.keys().collect::<Vec<_>>(), ["kept"]);
    assert!(response.raw().text().contains("constellation"));
    Ok(())
}

/// A field this crate does not model is ignored, at the top level and inside an
/// answer.
#[test]
fn an_unknown_field_is_ignored() -> Outcome {
    let body = json!({
        "model": "jev-latest",
        "answers": {"urgent": {"type": "noul", "noul": 0.4, "added_later": 1}},
        "usage": {"input_tokens": 1, "output_tokens": 2, "billing_units": 3},
        "added_later": {"any": "shape"},
    });
    let response = SystemOneResponse::decode(raw(&body)?)?;
    assert_eq!(response.answers.len(), 1);
    assert_eq!(response.usage.input_tokens, Some(1));
    Ok(())
}

#[test]
fn a_field_that_does_not_read_is_named_by_a_dotted_path() -> Outcome {
    let cases = [
        (
            json!({"model": "m", "answers": {"tone": {"type": "score", "score": 1.0, "confidence": "high", "legend": {}}}}),
            "answers.tone.confidence",
        ),
        (
            json!({"model": "m", "answers": {"tone": {"type": "noul"}}}),
            "answers.tone.noul",
        ),
        (
            json!({"model": "m", "answers": {"tone": {"type": "choice", "choice": "a", "confidence": 1.0}}}),
            "answers.tone.probabilities",
        ),
        (
            json!({"model": "m", "answers": {"tone": {"noul": 0.5}}}),
            "answers.tone.type",
        ),
        (json!({"answers": {}}), "model"),
        (json!({"model": 7, "answers": {}}), "model"),
        (json!({"model": "m", "answers": 7}), "answers"),
        (json!(["not an object"]), "body"),
    ];
    for (body, expected) in cases {
        let Err(Error::ResponseValidation {
            status,
            field_path,
            request_id,
            ..
        }) = SystemOneResponse::decode(raw(&body)?)
        else {
            unreachable!("{expected} does not read");
        };
        assert_eq!(field_path, expected);
        assert_eq!(status, 200);
        assert_eq!(request_id.as_deref(), Some("req_01test"));
    }
    Ok(())
}

#[test]
fn a_body_that_is_not_json_names_the_body() -> Outcome {
    let response = RawResponse {
        status: 200,
        headers: HeaderMap::new(),
        bytes: b"<html>a proxy answered</html>".to_vec(),
    };
    assert!(matches!(
        response.body(),
        Some(ResponseBody::Text(ref text)) if text.contains("proxy")
    ));
    let Err(Error::ResponseValidation { field_path, .. }) = SystemOneResponse::decode(response)
    else {
        unreachable!("text is not a response");
    };
    assert_eq!(field_path, "body");
    Ok(())
}

/// The recorded exchange decodes field for field, so the crate reads what the
/// API actually sent.
#[test]
fn the_recorded_response_decodes() -> Outcome {
    let response = SystemOneResponse::decode(RawResponse {
        status: 200,
        headers: HeaderMap::new(),
        bytes: RECORDED_RESPONSE.as_bytes().to_vec(),
    })?;
    assert_eq!(response.model, "jev-1.13.0");
    assert_eq!(response.usage.input_tokens, Some(471));
    assert_eq!(response.usage.output_tokens, Some(71));
    assert_eq!(response.request_id(), None);

    let department = response.choice("department")?;
    assert_eq!(department.choice, "billing");
    assert_eq!(
        department.probabilities.keys().collect::<Vec<_>>(),
        ["technical", "other", "billing"],
        "the recorded order survives decoding"
    );
    assert_eq!(department.probabilities.get("billing"), Some(&1.0));

    let severity = response.score("severity")?;
    assert!((severity.score - 0.97).abs() < 1e-9);
    assert!((severity.confidence - 0.64).abs() < 1e-9);
    assert_eq!(
        severity.legend.get(&0).map(jev::Entry::to_value),
        Some(json!({"meaning": "Cosmetic; functionality works"}))
    );
    assert_eq!(severity.probabilities.get(&1), Some(&0.76));

    assert!((response.noul("requestsRefund")?.noul - 0.99).abs() < 1e-9);
    Ok(())
}

/// A distribution sums to one within the rounding the API applies, two decimal
/// places per value.
#[test]
fn the_recorded_distributions_sum_to_one() -> Outcome {
    let response = SystemOneResponse::decode(RawResponse {
        status: 200,
        headers: HeaderMap::new(),
        bytes: RECORDED_RESPONSE.as_bytes().to_vec(),
    })?;
    let choice: f64 = response.choice("department")?.probabilities.values().sum();
    let score: f64 = response.score("severity")?.probabilities.values().sum();
    for total in [choice, score] {
        assert!((total - 1.0).abs() <= 0.02, "{total}");
    }
    Ok(())
}
