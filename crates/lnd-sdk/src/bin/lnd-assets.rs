//! Operator-only asset trial; mainnet needs explicit asset and custody approvals.
use lnd_sdk::{
    Client, Error,
    assets::{Approval, Assets},
    liquidity::Journal,
    macaroon::{Macaroon, Role},
    service::LightningRail,
    transport::Config,
};
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rpc {
    endpoint: String,
    certificate_sha256: String,
    macaroon: PathBuf,
}
impl Rpc {
    fn config(&self) -> Result<Config, Box<dyn std::error::Error>> {
        Ok(Config::new(
            self.endpoint.clone(),
            openagents_wallet::parse_hash32(&self.certificate_sha256)?,
        ))
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    lnd: Rpc,
    tapd: Rpc,
    node: String,
    network: String,
    asset_id: String,
    edge: String,
    approval: Approval,
    journal: PathBuf,
    ledger: PathBuf,
}
fn hash(text: &str) -> Result<[u8; 32], Box<dyn std::error::Error>> {
    Ok(openagents_wallet::parse_hash32(text)?)
}
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let s: Settings = serde_json::from_slice(&std::fs::read(args.get(1).ok_or(
        "usage: lnd-assets SETTINGS receive ID ACCOUNT UNITS|settle HASH|payout QUOTE.json|payout-status HASH|report",
    )?)?)?;
    let action = args.get(2).ok_or("an action is required")?;
    if action == "report" {
        let l = pay_ledger::Ledger::open_read_only(&s.ledger)?;
        println!("{}", serde_json::to_string_pretty(&l.asset_receipts()?)?);
        return Ok(());
    }
    let units = if action == "receive" {
        args.get(5).ok_or("asset units required")?.parse()?
    } else {
        1
    };
    if action == "receive" {
        s.approval.receive(&s.network, &s.asset_id, units)?;
    }
    let payout = if action == "payout" {
        let q: pay_ledger::assets::Quote =
            serde_json::from_slice(&std::fs::read(args.get(3).ok_or("quote file required")?)?)?;
        s.approval.payout(&s.network, &q.asset_id, q.units)?;
        Some(q)
    } else {
        None
    };
    if !["receive", "settle", "payout", "payout-status"].contains(&action.as_str()) {
        return Err(Error::Invalid("asset action").into());
    }
    let lnd = Client::connect(
        &s.lnd.config()?,
        Macaroon::from_file(Role::ReadOnly, 0, &s.lnd.macaroon)?,
    )
    .await?;
    if lnd.node_id() != s.node || lnd.network() != s.network {
        return Err(Error::NodeMismatch.into());
    }
    if action == "payout-status" {
        let h = hash(args.get(3).ok_or("payment hash required")?)?;
        let p = lnd.track(h).await?;
        println!(
            "{}",
            json!({"payment_hash":p.payment_hash,"status":p.status,"amount_msat":p.value_msat,"fee_msat":p.fee_msat})
        );
        return Ok(());
    }
    let assets = Assets::connect(
        &s.tapd.config()?,
        Macaroon::from_file(Role::Ops, 0, &s.tapd.macaroon)?,
        lnd,
    )
    .await?;
    let mut j = Journal::open(&s.journal)?;
    let binding = json!({"node":s.node,"network":s.network,"tapd_pin":s.tapd.certificate_sha256});
    if let Some(old) = j.entries.get("binding") {
        if old != &binding {
            return Err(Error::NodeMismatch.into());
        }
    } else {
        j.save("binding", binding)?;
    }
    let mut ledger = pay_ledger::Ledger::open(&s.ledger)?;
    match action.as_str() {
        "receive" => {
            let edge: [u8; 33] = hex::decode(&s.edge)?
                .try_into()
                .map_err(|_| Error::Invalid("asset edge"))?;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() as i64;
            let q = assets
                .receive(
                    &mut j,
                    args.get(3).ok_or("receive ID required")?,
                    args.get(4).ok_or("account required")?,
                    hash(&s.asset_id)?,
                    edge,
                    units,
                    &s.approval,
                    now,
                )
                .await?;
            ledger.open_asset_quote(&q)?;
            println!("{}", serde_json::to_string_pretty(&q)?);
        }
        "settle" => {
            let q = ledger
                .asset_quote(&s.node, args.get(3).ok_or("payment hash required")?)?
                .ok_or(Error::NotFound)?;
            let r = assets.settled(&q).await?;
            let credited = ledger.credit_asset(&q.node, &q.payment_hash, &r)?;
            println!(
                "{}",
                json!({"payment_hash":q.payment_hash,"units":r.units,"credited_msat":credited,"settle_index":r.settle_index})
            );
        }
        "payout" => {
            let edge: [u8; 33] = hex::decode(&s.edge)?
                .try_into()
                .map_err(|_| Error::Invalid("asset edge"))?;
            println!(
                "{}",
                assets
                    .pay(
                        &mut j,
                        &payout.ok_or(Error::Invalid("asset payout quote"))?,
                        edge,
                        &s.approval
                    )
                    .await?
            );
        }
        _ => unreachable!(),
    }
    Ok(())
}
#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("{e}");
        std::process::exit(1)
    }
}
