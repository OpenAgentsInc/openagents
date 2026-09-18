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
use jev::{Choice, Client, ListOptions, Noul, Questions, Score, SystemOneRequest};

type Outcome = Result<(), Box<dyn std::error::Error>>;

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
