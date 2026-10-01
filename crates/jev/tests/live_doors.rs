//! Live checks of Jev's fallback doors (`jev::doors`), opt-in: TypeSafe is
//! made unreachable in the test itself (a closed loopback port stands in
//! for it), so the real door behind it must answer. Each test reads its own
//! key from the environment and says so and passes when the key is absent.
//!
//! ```sh
//! OPENROUTER_API_KEY=… cargo test -p jev --test live_doors -- --ignored --nocapture
//! AI_GATEWAY_API_KEY=… cargo test -p jev --test live_doors -- --ignored --nocapture
//! ```

use std::time::{Duration, Instant};

use jev::doors::{self, Door, Failover, Naming};
use jev::{ApiKey, Choice, Client, Config, Noul, Questions, RetryPolicy, SystemOneRequest};
use serde_json::json;

fn closed_port() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    url
}

fn asking() -> SystemOneRequest {
    SystemOneRequest::new(
        "I was charged twice for my subscription. Please refund the duplicate.",
        Questions::new()
            .with(
                "refund",
                Noul::new("Is the customer asking for money back?"),
            )
            .with(
                "department",
                Choice::new("Which team should handle this?", Default::default())
                    .option("billing", "Charges and refunds")
                    .option("technical", "Bugs and outages"),
            ),
    )
}

async fn through(fallback: &doors::Fallback) -> Option<()> {
    let Some(key) = std::env::var(fallback.key_var)
        .ok()
        .filter(|key| !key.trim().is_empty())
    else {
        eprintln!(
            "{} is not set: {} not checked",
            fallback.key_var, fallback.door
        );
        return None;
    };
    let failover = Failover::new(
        Door::new(
            doors::TYPESAFE_DOOR,
            closed_port(),
            Naming::Canonical,
            ApiKey::new("unused"),
        ),
        vec![Door::fallback(fallback, ApiKey::new(key.trim()))],
    );
    let client = Client::new(
        Config::new()
            .exchange(doors::exchange(failover))
            .base_url(doors::TYPESAFE_DOOR)
            .default_model("jev-1.13.0")
            .timeout(Duration::from_secs(30))
            .retry(RetryPolicy {
                max_retries: 0,
                ..RetryPolicy::default()
            }),
    )
    .unwrap();
    let started = Instant::now();
    let response = client.system_one(asking()).await.unwrap_or_else(|error| {
        panic!("{} did not answer: {error}", fallback.door);
    });
    assert_eq!(response.service(), Some(json!({"door": fallback.door})));
    let refund = response.noul("refund").expect("a refund noul");
    let department = response.choice("department").expect("a department choice");
    eprintln!(
        "{} answered as {} in {} ms: refund={:.3} department={} ({:.3}) input_tokens={:?} cost_usd={:?}",
        fallback.door,
        response.model,
        started.elapsed().as_millis(),
        refund.noul,
        department.choice,
        department.confidence,
        response.usage.input_tokens,
        response.usage.cost_usd(),
    );
    assert!(refund.noul > 0.5);
    assert_eq!(department.choice, "billing");
    Some(())
}

#[tokio::test]
#[ignore = "live: asks OpenRouter's Decisions API under OPENROUTER_API_KEY"]
async fn live_openrouter_door_answers_when_typesafe_cannot() {
    let _ = through(&doors::FALLBACKS[1]).await;
}

#[tokio::test]
#[ignore = "live: asks the Vercel AI Gateway under AI_GATEWAY_API_KEY"]
async fn live_gateway_door_answers_when_typesafe_cannot() {
    let _ = through(&doors::FALLBACKS[0]).await;
}
