//! One request against the live API.
//!
//! The test runs only with the `live` feature, because it spends the account's
//! tokens and needs the network:
//!
//! ```sh
//! set -a; . ~/work/.secrets/typesafe.env; set +a
//! cargo test -p jev --features live -- --nocapture
//! ```
//!
//! The key comes from `TYPESAFE_API_KEY` and reaches nothing the test prints.
//! The request id does reach it, so you can quote the id when you report a
//! failure to TypeSafe.

#![cfg(feature = "live")]

use indexmap::IndexMap;
use jev::{
    ApiErrorKind, Choice, Client, Config, Entry, Error, ListOptions, Noul, NoulCriteria, Question,
    Questions, RetryPolicy, Score, SystemOneRequest,
};
use serde_json::json;

type Outcome = Result<(), Box<dyn std::error::Error>>;

/// A client with a policy that sends each request once.
fn once_client(configure: impl FnOnce(Config) -> Config) -> Result<Client, jev::Error> {
    Client::new(configure(Config::new()).retry(RetryPolicy {
        max_retries: 0,
        ..RetryPolicy::default()
    }))
}

#[tokio::test]
async fn one_request_reaches_the_api_and_its_answers_read() -> Outcome {
    if std::env::var("TYPESAFE_API_KEY").is_err() {
        return Err("set TYPESAFE_API_KEY before you run the live test".into());
    }
    let client = Client::from_env()?;
    let questions = Questions::new()
        .with("refund", Noul::new("Does the customer ask for money back?"))
        .with(
            "department",
            Choice::new("Which team should handle this?", IndexMap::new())
                .option("billing", "Charges, invoices, and refunds")
                .option("technical", "Bugs and outages")
                .bare_option("other"),
        )
        .with(
            "severity",
            Score::new("How severe is the issue?", Vec::new())
                .level("Cosmetic; the product works")
                .level("Impaired; a workaround exists")
                .level("Blocking; no workaround"),
        );
    let response = client
        .system_one(SystemOneRequest::new(
            "The same order was charged to my card twice, and I want the second charge back.",
            questions,
        ))
        .await?;

    println!("model: {}", response.model);
    println!("request id: {}", response.request_id().unwrap_or("-"));
    println!(
        "usage: {:?} input, {:?} output",
        response.usage.input_tokens, response.usage.output_tokens
    );
    for (id, answer) in &response.answers {
        println!("{id}: {answer:?}");
    }

    let refund = response.noul("refund")?;
    assert!((0.0..=1.0).contains(&refund.noul));
    let department = response.choice("department")?;
    assert!(
        ["billing", "technical", "other"].contains(&department.choice.as_str()),
        "{}",
        department.choice
    );
    let total: f64 = department.probabilities.values().sum();
    assert!((total - 1.0).abs() <= 0.02, "{total}");
    let severity = response.score("severity")?;
    assert!((0.0..=2.0).contains(&severity.score), "{}", severity.score);
    assert_eq!(severity.legend.len(), 3);
    Ok(())
}

#[tokio::test]
async fn the_live_account_lists_its_models() -> Outcome {
    if std::env::var("TYPESAFE_API_KEY").is_err() {
        return Err("set TYPESAFE_API_KEY before you run the live test".into());
    }
    let models = Client::from_env()?
        .models()
        .list(ListOptions::new())
        .await?;
    for model in &models {
        println!("{} ({})", model.name, model.release_date);
    }
    assert!(!models.is_empty());
    Ok(())
}

/// A key the API does not accept is an authentication error, and the error
/// carries the request id the server gave it.
#[tokio::test]
async fn a_bad_key_is_an_authentication_error() -> Outcome {
    if std::env::var("TYPESAFE_API_KEY").is_err() {
        return Err("set TYPESAFE_API_KEY before you run the live test".into());
    }
    let client = once_client(|config| config.api_key("ts-not-a-real-key"))?;
    let Err(Error::Api(error)) = client.models().list(ListOptions::new()).await else {
        unreachable!("a bad key is an API error");
    };
    println!("bad key: {error}");
    assert_eq!(error.status, 401);
    assert_eq!(error.kind, ApiErrorKind::Authentication);
    Ok(())
}

/// A model the API does not have is a bad request that names the model, and
/// the error carries the request id the server gave it.
#[tokio::test]
async fn an_unknown_model_is_a_bad_request() -> Outcome {
    if std::env::var("TYPESAFE_API_KEY").is_err() {
        return Err("set TYPESAFE_API_KEY before you run the live test".into());
    }
    let key = std::env::var("TYPESAFE_API_KEY")?;
    let client = once_client(|config| config.api_key(key))?;
    let Err(Error::Api(error)) = client
        .system_one(
            SystemOneRequest::new(
                "hello",
                Questions::new().with("q", Noul::new("Is this a greeting?")),
            )
            .model("no-such-model"),
        )
        .await
    else {
        unreachable!("an unknown model is an API error");
    };
    println!("unknown model: {error}");
    assert_eq!(error.status, 400);
    assert_eq!(error.kind, ApiErrorKind::BadRequest);
    assert!(
        error.message().contains("Unknown model: no-such-model"),
        "{}",
        error.message()
    );
    assert!(
        error
            .request_id
            .as_deref()
            .is_some_and(|id| id.starts_with("req_")),
        "{:?}",
        error.request_id
    );
    Ok(())
}

/// A shape the SDK does not judge but the API rejects reads as a 422 that
/// names the field, the way the official SDKs render one.
#[tokio::test]
async fn server_side_validation_names_the_field() -> Outcome {
    if std::env::var("TYPESAFE_API_KEY").is_err() {
        return Err("set TYPESAFE_API_KEY before you run the live test".into());
    }
    let key = std::env::var("TYPESAFE_API_KEY")?;
    let client = once_client(|config| config.api_key(key))?;
    // A criteria entry the SDK sends as it stands and the API refuses.
    let malformed = Questions::new().with(
        "q",
        Question::Raw(json!({"type": "score", "instructions": "?", "criteria": [123, "ok"]})),
    );
    let Err(Error::Api(error)) = client
        .system_one(SystemOneRequest::new("x", malformed))
        .await
    else {
        unreachable!("a malformed criteria entry is an API error");
    };
    println!("server validation: {error}");
    assert_eq!(error.status, 422);
    assert_eq!(error.kind, ApiErrorKind::UnprocessableEntity);
    assert!(
        error.message().contains("questions.q.score.criteria.0"),
        "{}",
        error.message()
    );
    Ok(())
}

/// Rich descriptions and one-sided criteria ride to the API and back: an
/// object state, a Noul with only its yes side described, and a Choice whose
/// options mix objects and nulls.
#[tokio::test]
async fn rich_descriptions_and_one_sided_criteria_reach_the_api() -> Outcome {
    if std::env::var("TYPESAFE_API_KEY").is_err() {
        return Err("set TYPESAFE_API_KEY before you run the live test".into());
    }
    let key = std::env::var("TYPESAFE_API_KEY")?;
    let client = once_client(|config| config.api_key(key))?;
    let mut options = IndexMap::new();
    options.insert(
        "calm".to_string(),
        Some(Entry::from(json!({
            "summary": "measured",
            "examples": ["please look into this"],
        }))),
    );
    options.insert("upset".to_string(), None);
    let questions = Questions::new()
        .with(
            "duplicate",
            Noul::with_criteria(
                "Is the customer reporting a duplicate charge?",
                NoulCriteria::new().when_true(json!({
                    "meaning": "the same amount charged more than once",
                    "examples": ["billed twice"],
                })),
            ),
        )
        .with("tone", Choice::new("Tone?", options));
    let response = client
        .system_one(SystemOneRequest::new(
            json!({
                "subject": "Charged twice",
                "body": "Please look into this; the same amount was charged more than once.",
            }),
            questions,
        ))
        .await?;
    println!("rich descriptions: {:?}", response.answers);
    let duplicate = response.noul("duplicate")?;
    assert!((0.0..=1.0).contains(&duplicate.noul));
    let tone = response.choice("tone")?;
    let mut names: Vec<&str> = tone.probabilities.keys().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(names, ["calm", "upset"]);
    Ok(())
}
