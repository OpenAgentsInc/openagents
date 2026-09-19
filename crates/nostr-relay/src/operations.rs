use std::io::Write as _;

use nostr_relay::gateway::GatewayError;
use nostr_relay::{bulk_import::import_jsonl, store::Store};

pub async fn import_signed_events() -> Result<(), GatewayError> {
    let database_url = std::env::var("DATABASE_URL").map_err(|_| {
        GatewayError::Config("import-jsonl requires DATABASE_URL in the environment".to_owned())
    })?;
    if database_url.trim().is_empty() {
        return Err(GatewayError::Config(
            "import-jsonl requires a non-empty DATABASE_URL".to_owned(),
        ));
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| {
            GatewayError::Internal(format!("system clock is before Unix time: {error}"))
        })?
        .as_secs();
    let mut store = Store::connect(&database_url).await?;
    let report = import_jsonl(
        std::io::BufReader::new(std::io::stdin().lock()),
        &mut store,
        now,
    )
    .await
    .map_err(|error| GatewayError::Config(error.to_string()))?;
    serde_json::to_writer(std::io::stdout().lock(), &report).map_err(|error| {
        GatewayError::Internal(format!("could not serialize import report: {error}"))
    })?;
    std::io::stdout().lock().write_all(b"\n")?;
    Ok(())
}
