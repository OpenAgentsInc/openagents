//! The synchronous client, for a caller that runs no `tokio` runtime. The
//! Python SDK's `TypeSafeClient` works the same way.
//!
//! Needs the `blocking` feature and `TYPESAFE_API_KEY` in the environment:
//!
//! ```sh
//! cargo run -p jev --features blocking --example blocking
//! ```

use jev::{BlockingClient, ListOptions, Noul, Questions, SystemOneRequest};

fn main() -> jev::Result<()> {
    let client = BlockingClient::from_env()?;

    let models = client.list_models(ListOptions::new())?;
    for model in &models {
        println!("{} ({})", model.name, model.release_date);
    }

    let response = client.system_one(SystemOneRequest::new(
        "The same order was charged to my card twice, and I want the second charge back.",
        Questions::new().with("refund", Noul::new("Does the customer ask for money back?")),
    ))?;

    let refund = response.noul("refund")?;
    println!("refund probability: {:.2}", refund.noul);
    Ok(())
}
