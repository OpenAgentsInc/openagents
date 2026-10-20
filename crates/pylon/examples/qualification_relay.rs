//! Run the existing in-memory loopback relay for bounded CLI qualification.

use std::{io::Write, time::Duration};

#[tokio::main]
async fn main() -> Result<(), String> {
    let seconds: u64 = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "900".into())
        .parse()
        .map_err(|_| "pass a maximum relay lifetime in seconds")?;
    let (url, _hub) = pylon::fixture::relay().await?;
    println!("{url}");
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = tokio::time::sleep(Duration::from_secs(seconds)) => {}
    }
    Ok(())
}
