//! One request against the live API, with one question of each type.
//!
//! Needs `TYPESAFE_API_KEY` in the environment:
//!
//! ```sh
//! set -a; . ~/work/.secrets/typesafe.env; set +a
//! cargo run -p jev --example chat_probe
//! ```

use indexmap::IndexMap;
use jev::{Choice, Client, Noul, Questions, Score, SystemOneRequest};

#[tokio::main]
async fn main() -> jev::Result<()> {
    let client = Client::from_env()?;
    let questions = Questions::new()
        .with("urgent", Noul::new("Is the sender under time pressure?"))
        .with(
            "intent",
            Choice::new("What does the sender want?", IndexMap::new())
                .bare_option("task")
                .bare_option("sales")
                .bare_option("support")
                .bare_option("other"),
        )
        .with(
            "politeness",
            Score::new("How polite is the message?", Vec::new())
                .level("rude")
                .level("neutral")
                .level("polite"),
        );
    let response = client
        .system_one(SystemOneRequest::new(
            "My laptop died an hour before the demo. I rebuilt the slides on \
             my phone and I need rush pricing on a replacement, please.",
            questions,
        ))
        .await?;

    println!("model:      {}", response.model);
    println!("request id: {}", response.request_id().unwrap_or("-"));
    let urgent = response.noul("urgent")?;
    println!("urgent:     noul {:.2}", urgent.noul);
    let intent = response.choice("intent")?;
    println!(
        "intent:     {:?} @ {:.2} confidence  {:?}",
        intent.choice, intent.confidence, intent.probabilities
    );
    let politeness = response.score("politeness")?;
    println!(
        "politeness: score {:.2} on 0-{} scale  {:?}",
        politeness.score,
        politeness.legend.len() - 1,
        politeness.probabilities
    );
    println!("raw status: {}", response.raw().status);
    Ok(())
}
