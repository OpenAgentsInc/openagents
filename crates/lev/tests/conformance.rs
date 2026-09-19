//! One contract, three implementations.
//!
//! `crates/jev` is the client for TypeSafe's hosted Jev. If it reaches a Lev
//! door with nothing changed but its `base_url`, the contract holds. This
//! test is the only place that claim is checked rather than asserted.
//!
//! It skips when the on-device model is unavailable, so the suite runs on any
//! machine.

#![cfg(feature = "serve")]

use std::sync::Arc;

use jev::{Choice, Client, Config, Noul, Questions, Score, SystemOneRequest};
use lev::bridge::{Bridge, Pool};
use lev::serve::Door;

/// Starts a door on an ephemeral port, or says why it could not.
async fn door() -> Option<String> {
    let mut bridge = match Bridge::discover() {
        Ok(bridge) => bridge,
        Err(refusal) => {
            eprintln!("skipping: {refusal}");
            return None;
        }
    };
    match bridge.availability() {
        Ok(availability) if availability.is_available() => {}
        other => {
            eprintln!("skipping: the model is not available ({other:?})");
            return None;
        }
    }

    drop(bridge);
    let pool = Pool::discover(2).ok()?;
    let door = Arc::new(Door::new(pool, "lev-base", 4));
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.ok()?;
    let port = listener.local_addr().ok()?.port();
    tokio::spawn(async move {
        let _ = axum::serve(listener, door.router()).await;
    });
    Some(format!("http://127.0.0.1:{port}"))
}

fn client(base: &str) -> Client {
    Client::new(
        Config::new()
            .api_key("unused-by-an-on-device-door")
            .base_url(base)
            .default_model("lev-base"),
    )
    .expect("the client builds")
}

#[tokio::test]
async fn an_unmodified_jev_client_round_trips_all_three_question_types() {
    let Some(base) = door().await else { return };
    let client = client(&base);

    let questions = Questions::new()
        .with("refund", Noul::new("Does the customer ask for money back?"))
        .with(
            "department",
            Choice::default()
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
            "I was charged twice for the same order last week and I want one of them refunded.",
            questions,
        ))
        .await
        .expect("the door answered");

    assert_eq!(response.model, "lev-base");

    let refund = response.noul("refund").expect("a Noul answer");
    assert!((0.0..=1.0).contains(&refund.noul), "a Noul is a probability: {}", refund.noul);

    let department = response.choice("department").expect("a Choice answer");
    assert!(
        ["billing", "technical", "other"].contains(&department.choice.as_str()),
        "the door returned an option outside the admitted set: {}",
        department.choice
    );
    assert_eq!(department.probabilities.len(), 3, "every option carries a probability");
    let total: f64 = department.probabilities.values().sum();
    assert!((total - 1.0).abs() < 1e-9, "a Choice distribution sums to one, got {total}");
    assert!(
        department.probabilities[&department.choice]
            >= department.probabilities.values().copied().fold(0.0, f64::max) - 1e-12,
        "the selected option holds the maximal probability"
    );

    let severity = response.score("severity").expect("a Score answer");
    assert!((0.0..=2.0).contains(&severity.score), "a Score falls inside its levels");
    assert_eq!(severity.legend.len(), 3, "the rubric comes back with the answer");

    eprintln!(
        "jev -> lev: refund {:.2}, department {} at {:.2}, severity {:.2}",
        refund.noul, department.choice, department.confidence, severity.score
    );
}

#[tokio::test]
async fn the_door_refuses_rather_than_inventing_a_calibrated_number() {
    let Some(base) = door().await else { return };
    let client = client(&base);

    let questions = Questions::new().with("refund", Noul::new("Does the customer want a refund?"));
    let mut request = SystemOneRequest::new("They asked for their money back.", questions);
    request = request.extra_body(
        serde_json::json!({"extensions": {"require_calibration": true}})
            .as_object()
            .expect("an object")
            .clone(),
    );

    let error = client.system_one(request).await.expect_err("the door refuses");
    let jev::Error::Api(api) = &error else {
        panic!("expected an API error, got {error}");
    };
    assert_eq!(api.status, 409, "an uncalibrated refusal is a conflict");
    let body = format!("{:?}", api.body);
    assert!(body.contains("uncalibrated"), "the refusal names its code: {body}");
}

#[tokio::test]
async fn a_question_outside_the_bounds_is_refused_before_any_call() {
    let Some(base) = door().await else { return };
    let client = client(&base);

    let mut choice = Choice::default();
    for index in 0..256 {
        choice = choice.bare_option(format!("option{index}"));
    }
    let questions = Questions::new().with("too_many", choice);
    // The client validates this itself, which is the contract working twice.
    let error = client
        .system_one(SystemOneRequest::new("a state", questions))
        .await
        .expect_err("a 256-option Choice is refused");
    assert!(matches!(error, jev::Error::Question { .. }), "got {error}");
}

#[tokio::test]
async fn a_jev_client_lists_the_door_s_models() {
    let Some(base) = door().await else { return };
    let cards = client(&base).models().list(jev::ListOptions::default()).await.expect("the door lists models");
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].name, "lev-base");
    assert!(!cards[0].description.is_empty());
    assert!(!cards[0].release_date.is_empty());
}
