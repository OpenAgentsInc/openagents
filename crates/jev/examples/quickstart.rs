//! The Python SDK's README quickstart, in Rust.
//!
//! Set `TYPESAFE_API_KEY` in the environment, then:
//!
//! ```sh
//! cargo run -p jev --example quickstart
//! ```

use indexmap::IndexMap;
use jev::{Choice, Client, Questions, SystemOneRequest};

#[tokio::main]
async fn main() -> jev::Result<()> {
    let client = Client::from_env()?;
    let response = client
        .system_one(SystemOneRequest::new(
            jev::Entry::from(serde_json::json!({
                "document": "I was charged twice. Please fix this ASAP."
            })),
            Questions::new().with(
                "category",
                Choice::new(
                    "What is this ticket about?",
                    IndexMap::from([
                        ("billing".to_string(), None),
                        ("technical".to_string(), None),
                        ("other".to_string(), None),
                    ]),
                ),
            ),
        ))
        .await?;

    println!("{}", response.choice("category")?.choice);
    Ok(())
}
