//! The public surface, used from outside the crate.
//!
//! The crate documentation carries the same list as a doctest, which names every
//! public item. This test uses them, so a removed item fails `cargo test` as
//! well, and the push gate runs no doctests.

use std::time::Duration;

use indexmap::IndexMap;
use jev::{
    Answer, ApiError, ApiErrorKind, ApiKey, Choice, ChoiceAnswer, Client, Config, Entry, Error,
    ListOptions, ModelCard, Noul, NoulAnswer, NoulCriteria, Question, Questions, RawResponse,
    ResponseBody, Result, RetryPolicy, RetryPredicate, Score, ScoreAnswer, SystemOneRequest,
    SystemOneResponse, Usage, defaults, env, parse_retry_after, parse_retry_after_at,
};
use reqwest::header::HeaderMap;
use serde_json::json;

type Outcome = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn the_defaults_and_the_variable_names_are_the_official_ones() {
    assert_eq!(defaults::BASE_URL, "https://api.typesafe.ai");
    assert_eq!(defaults::MODEL, "jev-latest");
    assert_eq!(defaults::TIMEOUT, Duration::from_secs(10));
    assert_eq!(env::API_KEY, "TYPESAFE_API_KEY");
    assert_eq!(env::BASE_URL, "TYPESAFE_BASE_URL");
    assert_eq!(env::DEFAULT_MODEL, "TYPESAFE_DEFAULT_MODEL");
    assert_eq!(env::LOG_LEVEL, "TYPESAFE_LOG_LEVEL");
    assert_eq!(jev::VERSION, env!("CARGO_PKG_VERSION"));
}

#[test]
fn a_caller_builds_every_question_type_and_reads_every_answer_type() -> Outcome {
    let questions = Questions::new()
        .with("refund", Noul::new("Money back?"))
        .with(
            "described",
            Noul::with_criteria(
                "Money back?",
                NoulCriteria::new()
                    .when_true("asked")
                    .when_false("not asked"),
            ),
        )
        .with(
            "department",
            Choice::new("Which team?", IndexMap::new()).option("billing", "Charges"),
        )
        .with(
            "severity",
            Score::new("How severe?", vec![Some(Entry::from("Calm"))]).level("Urgent"),
        )
        .with("raw", json!({"type": "noul", "instructions": "Urgent?"}));
    questions.validate()?;
    assert_eq!(questions.get("raw").and_then(Question::kind), Some("noul"));

    let request = SystemOneRequest::new(Entry::json(&json!({"field": 1}))?, questions)
        .model(defaults::MODEL)
        .timeout(Duration::from_secs(2))
        .headers(HeaderMap::new())
        .extra_body(serde_json::Map::new())
        .retry(RetryPolicy::default());
    assert_eq!(request.model.as_deref(), Some("jev-latest"));

    let body = json!({
        "model": "jev-latest",
        "answers": {
            "refund": {"type": "noul", "noul": 0.9},
            "department": {
                "type": "choice",
                "choice": "billing",
                "confidence": 0.8,
                "probabilities": {"billing": 1.0},
            },
            "severity": {
                "type": "score",
                "score": 1.0,
                "confidence": 0.7,
                "legend": {"0": "Calm", "1": "Urgent"},
                "probabilities": {"0": 0.0, "1": 1.0},
            },
        },
        "usage": {"input_tokens": 12, "output_tokens": 3},
    });
    let response = SystemOneResponse::decode(RawResponse {
        status: 200,
        headers: HeaderMap::new(),
        bytes: serde_json::to_vec(&body)?,
    })?;
    let _: &NoulAnswer = response.noul("refund")?;
    let _: &ChoiceAnswer = response.choice("department")?;
    let _: &ScoreAnswer = response.score("severity")?;
    let _: Usage = response.usage;
    let kind: fn(&Answer) -> &'static str = Answer::kind;
    assert_eq!(response.answers.len(), 3);
    assert_eq!(response.answers.values().map(kind).count(), 3);
    // The reading keeps the order the response sent, which
    // `tests/answers.rs` tests against body text; this comparison sorts
    // the pairs so it reads the set alone.
    let mut kinds = response
        .answers
        .iter()
        .map(|(id, answer)| (id.as_str(), answer.kind()))
        .collect::<Vec<_>>();
    kinds.sort();
    assert_eq!(
        kinds,
        [
            ("department", "choice"),
            ("refund", "noul"),
            ("severity", "score"),
        ]
    );
    assert_eq!(response.nouls().count(), 1);
    assert_eq!(response.choices().count(), 1);
    assert_eq!(response.scores().count(), 1);
    assert_eq!(response.raw().status, 200);
    assert_eq!(response.raw().request_id(), None);
    assert!(response.raw().text().contains("jev-latest"));
    assert!(matches!(response.raw().body(), Some(ResponseBody::Json(_))));
    Ok(())
}

#[test]
fn a_key_never_reaches_a_log_line_or_a_failure() -> Outcome {
    let key = ApiKey::new("ts-secret-value-1234");
    assert_eq!(format!("{key:?}"), "***");
    assert_eq!(key.to_string(), "***");
    assert_eq!(key.expose(), "ts-secret-value-1234");

    let client = Client::new(
        Config::new()
            .api_key(key)
            .base_url("https://api.typesafe.ai")
            .default_model(defaults::MODEL)
            .timeout(defaults::TIMEOUT)
            .retry(RetryPolicy::default())
            .default_headers(HeaderMap::new())
            .http_client(reqwest::Client::new()),
    )?;
    assert!(!format!("{client:?}").contains("ts-secret-value"));
    assert!(client.default_headers().is_empty());
    let _ = client.models();
    Ok(())
}

#[test]
fn every_failure_reads_without_its_key_and_names_its_request() {
    let error = ApiError {
        status: 429,
        headers: HeaderMap::new(),
        body: Some(ResponseBody::Text("slow down".to_string())),
        request_id: Some("req_01test".to_string()),
        endpoint: "POST https://api.typesafe.ai/v1/systemone".to_string(),
        kind: ApiErrorKind::RateLimit { retry_after: None },
    };
    assert_eq!(error.message(), "slow down");
    assert_eq!(error.retry_after(), None);
    let wrapped = Error::from(error);
    assert_eq!(wrapped.request_id(), Some("req_01test"));
    assert!(wrapped.to_string().contains("429 slow down"));

    let failures: Vec<Error> = vec![
        Error::Config("no key".to_string()),
        Error::Question {
            id: "tone".to_string(),
            message: "a Score question names at least 2 levels".to_string(),
        },
        Error::Connection {
            message: "connection error: reset".to_string(),
            source: None,
        },
        Error::Timeout {
            timeout: Duration::from_secs(10),
        },
        Error::ResponseValidation {
            status: 200,
            field_path: "answers.tone.confidence".to_string(),
            body: None,
            request_id: None,
        },
        Error::AnswerType {
            id: "tone".to_string(),
            expected: "noul",
            found: "score",
        },
        Error::MissingAnswer {
            id: "tone".to_string(),
        },
    ];
    for failure in failures {
        assert!(!failure.to_string().is_empty());
    }
}

#[test]
fn the_retry_surface_reads_from_outside() {
    let predicate: RetryPredicate = std::sync::Arc::new(|error| matches!(error, Error::Config(_)));
    let policy = RetryPolicy {
        predicate: Some(predicate),
        ..RetryPolicy::default()
    };
    assert!(policy.validate().is_ok());
    assert!(policy.retries_status(503));
    assert!(policy.retries_error(&Error::Config("no key".to_string())));
    assert!(policy.delay(0, None) <= Duration::from_millis(500));
    assert_eq!(policy.delay_with(0, None, 0.0), Duration::from_millis(500));
    assert_eq!(parse_retry_after(&HeaderMap::new()), None);
    assert_eq!(
        parse_retry_after_at(&HeaderMap::new(), std::time::SystemTime::UNIX_EPOCH),
        None
    );
    assert_eq!(
        ApiErrorKind::of(500, &HeaderMap::new()),
        ApiErrorKind::InternalServer
    );
}

#[test]
fn the_models_surface_reads_from_outside() {
    let _ = ListOptions::new()
        .retry(RetryPolicy::default())
        .timeout(Duration::from_secs(1))
        .headers(HeaderMap::new());
    let card = ModelCard {
        name: "jev-latest".to_string(),
        description: "The current model".to_string(),
        release_date: "2026-09-01".to_string(),
    };
    assert_eq!(card.name, "jev-latest");
}

/// A call returns this crate's own `Result`, so a caller can name it.
#[test]
fn the_result_alias_names_this_crate_s_failure() {
    let outcome: Result<u8> = Err(Error::Config("no key".to_string()));
    assert!(outcome.is_err());
}

/// The blocking client is built the same way, and it owns the runtime it runs
/// each call on.
#[cfg(feature = "blocking")]
#[test]
fn the_blocking_client_builds_from_the_same_settings() -> Outcome {
    let client = jev::BlockingClient::new(Config::new().api_key("ts-secret-value-1234"))?;
    assert_eq!(client.client().default_model(), defaults::MODEL);
    Ok(())
}
