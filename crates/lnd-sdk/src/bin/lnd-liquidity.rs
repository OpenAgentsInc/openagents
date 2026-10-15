//! Operator-driven liquidity purchases; no background spending.
use lnd_sdk::{
    Client, Error,
    liquidity::{self, Budget, Journal, Provider},
    macaroon::{Macaroon, Role},
    service::LightningRail,
    transport::Config,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{path::PathBuf, time::Duration};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    endpoint: String,
    certificate_sha256: String,
    macaroon: PathBuf,
    node: String,
    network: String,
    provider: Provider,
    journal: PathBuf,
    budget: Budget,
    channel_peer: String,
    channel_address: String,
}
async fn run() -> Result<Value, Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let path = args
        .get(1)
        .ok_or("usage: lnd-liquidity SETTINGS info|plan|order|status|pay|open|costs")?;
    let action = args.get(2).ok_or("an action is required")?.as_str();
    let settings: Settings = serde_json::from_slice(&std::fs::read(path)?)?;
    if action == "info" {
        return Ok(liquidity::request(settings.provider, "get_info", None, None).await?);
    }
    if action == "plan" {
        let info = liquidity::request(settings.provider, "get_info", None, None).await?;
        return Ok(
            json!({"terms": settings.budget.terms(&info, &settings.node)?, "budget": settings.budget}),
        );
    }
    let pin = hex::decode(&settings.certificate_sha256)?
        .try_into()
        .map_err(|_| "invalid certificate pin")?;
    let client = Client::connect(
        &Config::new(settings.endpoint.clone(), pin),
        Macaroon::from_file(Role::Ops, 104, &settings.macaroon)?,
    )
    .await?;
    if client.node_id() != settings.node || client.network() != settings.network {
        return Err(Error::NodeMismatch.into());
    }
    if action == "guard" {
        settings.budget.authorize(&settings.network)?;
        client
            .guard_inbound_channel(settings.provider.peer().0, settings.budget.inbound_sats)
            .await?;
        return Err(Error::Unavailable.into());
    }
    let mut journal = Journal::open(&settings.journal)?;
    // Bind the single-use action record to this node and the exact approved budget.
    let binding = json!({"node": settings.node, "network": settings.network, "provider": settings.provider, "budget": settings.budget,
        "channel_peer": settings.channel_peer, "channel_address": settings.channel_address});
    if let Some(saved) = journal.entries.get("binding") {
        if saved != &binding {
            return Err(Error::Invalid("saved liquidity budget").into());
        }
    } else {
        journal.save("binding", binding)?;
    }
    match action {
        "order" => {
            settings.budget.authorize(&settings.network)?;
            let info = liquidity::request(settings.provider, "get_info", None, None).await?;
            let terms = settings.budget.terms(&info, &settings.node)?;
            journal.begin("purchase", terms.clone())?;
            let order =
                liquidity::request(settings.provider, "create_order", Some(&terms), None).await?;
            // Save even a malformed reply for operator recovery; never repeat creation.
            journal.save("purchase", json!({"terms": terms, "order": order}))?;
            liquidity::validate_order(
                &order,
                &terms,
                &settings.budget,
                &settings.network,
                settings.provider.peer().0,
            )?;
            Ok(order)
        }
        "status" => {
            let purchase = journal.entries.get("purchase").ok_or(Error::NotFound)?;
            let id = purchase["order"]["order_id"]
                .as_str()
                .ok_or(Error::Invalid(
                    "unknown order; ask the LSP before any new purchase",
                ))?;
            Ok(liquidity::request(settings.provider, "get_order", None, Some(id)).await?)
        }
        "pay" => {
            let purchase = journal.entries.get("purchase").ok_or(Error::NotFound)?;
            let text = purchase["order"]["payment"]["bolt11"]["invoice"]
                .as_str()
                .ok_or(Error::Invalid("saved LSP invoice"))?;
            let decoded = lnd_sdk::router::invoice(text)?;
            let hash = decoded.payment_hash().to_string();
            if journal.entries.contains_key("payment") {
                // Recover only; even NotFound does not authorize another dispatch.
                let payment = client
                    .track(
                        hex::decode(hash)?
                            .try_into()
                            .map_err(|_| Error::Invalid("payment hash"))?,
                    )
                    .await?;
                return Ok(
                    json!({"payment_hash": payment.payment_hash, "status": payment.status, "fee_msat": payment.fee_msat}),
                );
            }
            settings.budget.authorize(&settings.network)?;
            let invoice = liquidity::validate_order(
                &purchase["order"],
                &purchase["terms"],
                &settings.budget,
                &settings.network,
                settings.provider.peer().0,
            )?;
            journal.begin("payment", json!({"hash": hash}))?;
            let proof = client
                .send_payment(
                    &invoice,
                    settings.budget.maximum_routing_fee_msat,
                    Duration::from_secs(60),
                )
                .await?;
            let paid =
                json!({"hash": proof.payment_hash, "fee_msat": proof.fee_msat, "state": "paid"});
            journal.save("payment", paid.clone())?;
            Ok(paid)
        }
        "open" => {
            settings.budget.authorize(&settings.network)?;
            settings
                .budget
                .check_open(client.balance().await?.onchain_spendable_sats)?;
            journal.begin(
                "channel",
                json!({"peer": settings.channel_peer, "sats": settings.budget.channel_sats}),
            )?;
            let point = client
                .open_peer_channel_at_rate(
                    &settings.channel_peer,
                    &settings.channel_address,
                    settings.budget.channel_sats,
                    true,
                    settings.budget.opening_sat_per_vbyte,
                )
                .await?;
            let result = json!({"point": point, "state": "opened"});
            journal.save("channel", result.clone())?;
            Ok(result)
        }
        "costs" => {
            let point = journal
                .entries
                .get("channel")
                .and_then(|v| v["point"].as_str())
                .ok_or(Error::NotFound)?;
            let fee = client.channel_opening_fee(point).await?;
            let received = client
                .payments()
                .await?
                .iter()
                .filter(|p| {
                    p.direction == openagents_wallet::PaymentDirection::Inbound
                        && p.status == openagents_wallet::PaymentStatus::Succeeded
                })
                .try_fold(0u64, |a, p| a.checked_add(p.amount_msat.unwrap_or(0)))
                .ok_or(Error::Invalid("receipt sum"))?;
            let hash = journal
                .entries
                .get("payment")
                .and_then(|v| v["hash"].as_str().or(v["terms"]["hash"].as_str()))
                .ok_or(Error::NotFound)?;
            let payment = client
                .track(
                    hex::decode(hash)?
                        .try_into()
                        .map_err(|_| Error::Invalid("payment hash"))?,
                )
                .await?;
            if payment.status != lnd_sdk::proto::lnrpc::payment::PaymentStatus::Succeeded as i32 {
                return Err(Error::InFlight.into());
            }
            let costs = liquidity::Costs {
                lsp_fee_sats: payment.value_sat.try_into()?,
                opening_fee_sats: fee,
                routing_fee_msat: payment.fee_msat.try_into()?,
                received_msat: received,
            };
            let result = json!({"actual": costs, "comparison": costs.comparison()});
            journal.save("costs", result.clone())?;
            Ok(result)
        }
        _ => Err("unknown liquidity action".into()),
    }
}
#[tokio::main]
async fn main() {
    match run().await {
        Ok(v) => println!("{v}"),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
