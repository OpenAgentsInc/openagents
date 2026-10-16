//! Scheduled liquidity observations and explicitly approved Loop swaps.
use lnd_sdk::{
    Client, Error,
    automation::{self, Balances, Loop, Plan, Policy},
    liquidity::Journal,
    macaroon::{Macaroon, Role},
    service::LightningRail,
    transport::Config,
};
use serde::Deserialize;
use serde_json::json;
use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
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
    r#loop: Option<Rpc>,
    node: String,
    network: String,
    policy: PathBuf,
    journal: PathBuf,
    output: PathBuf,
}
async fn run() -> Result<bool, Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("usage: lnd-autoloop SETTINGS.json")?;
    let s: Settings = serde_json::from_slice(&std::fs::read(path)?)?;
    let p = Policy::parse(&std::fs::read_to_string(&s.policy)?)?;
    let c = Client::connect(
        &s.lnd.config()?,
        Macaroon::from_file(Role::ReadOnly, 0, &s.lnd.macaroon)?,
    )
    .await?;
    if c.node_id() != s.node || c.network() != s.network {
        return Err(Error::NodeMismatch.into());
    }
    let h = c.health().await?;
    let b = c.balance().await?;
    let balances = Balances {
        hot_sats: b
            .onchain_total_sats
            .checked_add(b.lightning_total_sats)
            .ok_or(Error::Invalid("hot balance"))?,
        inbound_sats: h.inbound_msat / 1000,
        outbound_sats: h.outbound_msat / 1000,
        confirmed_sats: b.onchain_spendable_sats,
    };
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let mut j = Journal::open(&s.journal)?;
    let channels =
        automation::channel_review(&mut j, &p.channels, &c.channel_details().await?, now)?;
    let mut plan = p.plan(&s.network, balances)?;
    if let Some(r) = &s.r#loop {
        automation::bind(&mut j, &s.node, &s.network, &r.certificate_sha256)?;
        let rail = Loop::connect(
            &r.config()?,
            Macaroon::from_file(Role::Ops, 0, &r.macaroon)?,
            &s.network,
        )
        .await?;
        if !h.synced_to_chain || h.pending_force_closes > 0 {
            automation::reconcile(&mut j, &rail).await?;
            plan = Plan::Alert {
                reason: "The node needs attention before another swap.".into(),
            };
        } else {
            plan = automation::tick(&mut j, &rail, &p, &s.network, balances, now).await?;
        }
    } else if !matches!(plan, Plan::Alert { .. })
        || j.entries.keys().any(|k| k.starts_with("swap:"))
    {
        return Err(Error::Invalid("Loop connection required").into());
    }
    let snapshot = json!({"observed_at":now,"node":s.node,"policy_revision":p.revision,"balances":balances,"plan":plan,"channel_review":channels,"swaps":j.entries.iter().filter(|(k,_)|k.starts_with("observed:")||k.starts_with("swap:")).collect::<std::collections::BTreeMap<_,_>>()});
    let temporary = s.output.with_extension("new");
    std::fs::write(&temporary, serde_json::to_vec_pretty(&snapshot)?)?;
    std::fs::rename(temporary, s.output)?;
    println!("{snapshot}");
    Ok(!matches!(plan, Plan::Alert { .. }))
}
#[tokio::main]
async fn main() {
    match run().await {
        Ok(true) => {}
        Ok(false) => std::process::exit(1),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2)
        }
    }
}
