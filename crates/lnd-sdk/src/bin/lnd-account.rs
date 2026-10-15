//! Create a capped worker account once; never reset its balance on deployment.
use lnd_sdk::{
    Error,
    liquidity::Journal,
    macaroon::{Macaroon, Role},
    payout::Accounts,
    transport::Config,
};
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    endpoint: String,
    certificate_sha256: String,
    operator_macaroon: PathBuf,
    mainnet_approved: bool,
    label: String,
    limit_sats: u64,
    expires_at: i64,
    credential_output: PathBuf,
    journal: PathBuf,
}
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("usage: lnd-account SETTINGS.json")?;
    let s: Settings = serde_json::from_slice(&std::fs::read(path)?)?;
    if !s.mainnet_approved || s.limit_sats == 0 {
        return Err(Error::PermissionDenied.into());
    }
    if s.credential_output.exists() {
        return Err(Error::Invalid("existing payout credential").into());
    }
    let pin = openagents_wallet::parse_hash32(&s.certificate_sha256)?;
    let accounts = Accounts::connect(
        &Config::new(s.endpoint, pin),
        Macaroon::from_file(Role::Ops, 0, &s.operator_macaroon)?,
    )
    .await?;
    let mut journal = Journal::open(&s.journal)?;
    journal.begin(
        "account",
        json!({"label": s.label, "limit_sats": s.limit_sats, "expires_at": s.expires_at}),
    )?;
    let account = accounts.create(s.label, s.limit_sats, s.expires_at).await?;
    account.macaroon.write_new(&s.credential_output)?;
    let record = json!({"id": account.id, "limit_sats": account.limit_sats, "credential": s.credential_output, "state": "created"});
    journal.save("account", record.clone())?;
    println!("{record}");
    Ok(())
}
#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
