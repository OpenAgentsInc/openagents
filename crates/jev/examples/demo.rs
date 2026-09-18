//! The JavaScript SDK's `examples/demo.ts`, in Rust.
//!
//! Lists the account's models, asks four questions about a support ticket, and
//! prints the typed answers. Needs `TYPESAFE_API_KEY` in the environment:
//!
//! ```sh
//! cargo run -p jev --example demo
//! ```

use indexmap::IndexMap;
use jev::{Choice, Client, Entry, Error, ListOptions, Noul, Questions, Score, SystemOneRequest};

#[tokio::main]
async fn main() -> jev::Result<()> {
    let client = Client::from_env()?;

    let models = client.models().list(ListOptions::new()).await?;
    let names: Vec<&str> = models.iter().map(|model| model.name.as_str()).collect();
    println!("Available models: {}", names.join(", "));

    let ticket = serde_json::json!({
        "subject": "Charged twice this month",
        "body": "Hi, I see two charges of $49 on my card for August. I only \
                 have one account. Please fix this ASAP, I'm pretty frustrated.",
    });

    let questions = Questions::new()
        .with("isBilling", Noul::new("Is this ticket about billing?"))
        .with(
            "sentiment",
            Choice::new(
                "What is the customer's tone?",
                IndexMap::from([
                    ("calm".to_string(), None),
                    ("frustrated".to_string(), None),
                    ("angry".to_string(), None),
                ]),
            ),
        )
        .with(
            "urgency",
            Score::new(
                "How urgent is this ticket?",
                vec![
                    Some(Entry::from("can wait")),
                    Some(Entry::from("this week")),
                    Some(Entry::from("today")),
                    Some(Entry::from("right now")),
                ],
            ),
        )
        .with(
            "refundRisk",
            Score::new(
                "How likely is the customer to demand a refund?",
                vec![
                    Some(Entry::from("unlikely")),
                    Some(Entry::from("possible")),
                    Some(Entry::from("likely")),
                ],
            ),
        );

    match client
        .system_one(SystemOneRequest::new(ticket, questions))
        .await
    {
        Ok(response) => {
            let is_billing = response.noul("isBilling")?;
            println!("billing?     {:.2}", is_billing.noul);
            let sentiment = response.choice("sentiment")?;
            let picked = sentiment
                .probabilities
                .get(&sentiment.choice)
                .copied()
                .unwrap_or(0.0);
            println!("tone         {} ({picked:.2})", sentiment.choice);
            let urgency = response.score("urgency")?;
            println!(
                "urgency      {:.2} on a 0-3 scale: {:?}",
                urgency.score, urgency.legend
            );
            let refund_risk = response.score("refundRisk")?;
            println!(
                "refund risk  {:.2} ({:.2} confidence)",
                refund_risk.score, refund_risk.confidence
            );
            println!(
                "tokens       {:?} in / {:?} out",
                response.usage.input_tokens, response.usage.output_tokens
            );
            Ok(())
        }
        Err(Error::Api(error)) => {
            eprintln!(
                "API error {} (request {}): {:?}",
                error.status,
                error.request_id.as_deref().unwrap_or("unknown"),
                error.body
            );
            std::process::exit(1);
        }
        Err(error) => Err(error),
    }
}
