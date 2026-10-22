//! Save verified head and stage beacons for a bounded qualification check.

use pylon::{client, identity::Identity, relay};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), String> {
    let url = std::env::args()
        .nth(1)
        .ok_or("pass the qualification relay URL")?;
    let reader = Identity::generate();
    let mut connection = relay::connect(&url, &reader, relay::LIFETIME).await?;
    // This reader includes stage beacons. Ordinary routing excludes them.
    let book = client::all_beacons(&mut connection, None).await?;
    let events: Vec<_> = book.iter().map(|(event, _)| event.clone()).collect();
    connection.close().await?;
    println!(
        "{}",
        json!({
            "schema": "openagents.pylon.qualification_beacons.v1",
            "observed_at": pylon::now(),
            "relay": url,
            "events": events,
        })
    );
    Ok(())
}
