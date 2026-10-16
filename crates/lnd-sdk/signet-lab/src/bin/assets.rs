//! Exact, restart-safe RFQ payments with a synthetic asset on public signet.
use lnd_sdk::{
    Client,
    assets::{Approval, Assets},
    liquidity::Journal,
    macaroon::{Macaroon, Role},
    service::LightningRail,
    transport::Config,
};
use pay_ledger::Ledger;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, io::Write, path::Path, time::SystemTime};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
fn save(path: &Path, value: &Value) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(&serde_json::to_vec_pretty(value)?)?;
    f.sync_all()?;
    fs::File::open(path.parent().ok_or("Missing research directory.")?)?.sync_all()?;
    Ok(())
}
fn read(path: &Path) -> Result<Value> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}
async fn connect(root: &Path, node: &str) -> Result<(Client, Assets)> {
    use std::os::unix::fs::PermissionsExt;
    let port = match node {
        "a" => 10089,
        "b" => 10099,
        _ => return Err("Unknown research node.".into()),
    };
    let directory = root.join(node);
    let cert = rustls_pemfile::certs(&mut std::io::Cursor::new(fs::read(
        directory.join("lnd/tls.cert"),
    )?))
    .next()
    .ok_or("Missing research certificate.")??;
    let config = Config::new(
        format!("https://127.0.0.1:{port}"),
        Sha256::digest(cert.as_ref()).into(),
    );
    let macaroon = directory.join("lnd/data/chain/bitcoin/signet/admin.macaroon");
    fs::set_permissions(&macaroon, fs::Permissions::from_mode(0o600))?;
    let lnd = Client::connect(&config, Macaroon::from_file(Role::Ops, 0, &macaroon)?).await?;
    let identities = read(&root.join("identities.json"))?;
    if lnd.network() != "signet" || identities["nodes"][node] != lnd.node_id() {
        return Err("The research node identity or network changed.".into());
    }
    let macaroon = directory.join("tap/data/signet/admin.macaroon");
    fs::set_permissions(&macaroon, fs::Permissions::from_mode(0o600))?;
    let assets = Assets::connect(
        &config,
        Macaroon::from_file(Role::Ops, 0, &macaroon)?,
        lnd.clone(),
    )
    .await?;
    Ok((lnd, assets))
}
async fn fund_channel(root: &Path, a: &(Client, Assets), b: &(Client, Assets)) -> Result<Value> {
    let mint = read(&root.join("asset.json"))?;
    if mint["synthetic_only"] != true || mint["name"] != "openagents-signet-test-11863" {
        return Err("Only the lab's confirmed synthetic asset can fund this channel.".into());
    }
    if !a.0.channel_details().await?.is_empty() || !b.0.channel_details().await?.is_empty() {
        return Err(
            "The research nodes already have a channel; inspect its original funding.".into(),
        );
    }
    let asset = hex::decode(
        mint["asset_id"]
            .as_str()
            .ok_or("Missing research asset ID.")?,
    )?
    .try_into()
    .map_err(|_| "Invalid research asset ID.")?;
    let peer = hex::decode(b.0.node_id())?
        .try_into()
        .map_err(|_| "Invalid research peer.")?;
    if a.0.balance().await?.onchain_spendable_sats < 105000
        || b.0.balance().await?.onchain_spendable_sats < 10000
    {
        return Err(
            "Wait for the original scratch recovery to fund the asset channel and fee reserves."
                .into(),
        );
    }
    save(
        &root.join("channel-funding-started.json"),
        &json!({"asset_id":mint["asset_id"],
        "asset_units":500000,"btc_channel_sat":100000,"fee_rate_sat_per_vbyte":2,
        "push_sat":1062,"peer":b.0.node_id()}),
    )?;
    let funded =
        a.1.fund_signet_research_channel(asset, peer, 500000)
            .await?;
    let result = json!({"txid":funded.txid,"output_index":funded.output_index});
    save(&root.join("channel-funding-result.json"), &result)?;
    Ok(result)
}
async fn exercise(root: &Path, a: &(Client, Assets), b: &(Client, Assets)) -> Result<Value> {
    use std::os::unix::fs::PermissionsExt;
    let mint = read(&root.join("asset.json"))?;
    if mint["synthetic_only"] != true || mint["name"] != "openagents-signet-test-11863" {
        return Err("Only the lab's synthetic test asset is admitted.".into());
    }
    let id = mint["asset_id"]
        .as_str()
        .ok_or("Missing confirmed test asset.")?;
    let asset: [u8; 32] = hex::decode(id)?
        .try_into()
        .map_err(|_| "Invalid test asset ID.")?;
    let approval = Approval {
        mainnet_receive_approved: false,
        custody_review_approved: false,
        mainnet_payout_approved: false,
        asset_ids: vec![id.to_owned()],
        maximum_units: 20000,
        maximum_credit_msat: 20_000_000,
        maximum_fee_msat: 5000,
    };
    let private = root.join("private");
    fs::create_dir_all(&private)?;
    fs::set_permissions(&private, fs::Permissions::from_mode(0o700))?;
    let mut journal = Journal::open(&private.join("rfq-actions.json"))?;
    let binding = json!({"network":"signet","asset_id":id,"a":a.0.node_id(),"b":b.0.node_id()});
    if let Some(saved) = journal.entries.get("binding") {
        if saved != &binding {
            return Err("The original asset lab binding changed.".into());
        }
    } else {
        journal.save("binding", binding)?;
    }
    let mut results = Vec::new();
    for (label, payer, receiver, units) in [("a-to-b", a, b, 10000u64), ("b-to-a", b, a, 5000u64)] {
        let path = private.join(format!("{label}.db"));
        let now = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs() as i64;
        let mut ledger = Ledger::open(&path)?;
        ledger.create_compute_account(label, now)?;
        let edge = hex::decode(payer.0.node_id())?
            .try_into()
            .map_err(|_| "Invalid research peer.")?;
        let quote = receiver
            .1
            .receive(
                &mut journal,
                label,
                label,
                asset,
                edge,
                units,
                &approval,
                now,
            )
            .await?;
        ledger.open_asset_quote(&quote)?;
        let payer_edge = hex::decode(receiver.0.node_id())?
            .try_into()
            .map_err(|_| "Invalid research edge.")?;
        let paid = payer
            .1
            .pay(&mut journal, &quote, payer_edge, &approval)
            .await?;
        if paid["status"] != 2 || paid["payment_hash"] != quote.payment_hash {
            return Err(
                "The original asset payment has not succeeded; no replacement was sent.".into(),
            );
        }
        let receipt = receiver.1.settled(&quote).await?;
        if receipt.units != units {
            return Err("The receiver settled a different asset amount.".into());
        }
        ledger.credit_asset(&quote.node, &quote.payment_hash, &receipt)?;
        drop(ledger);
        let mut ledger = Ledger::open(&path)?;
        ledger.credit_asset(&quote.node, &quote.payment_hash, &receipt)?;
        let recovered = payer
            .1
            .pay(&mut journal, &quote, payer_edge, &approval)
            .await?;
        if recovered != paid
            || ledger.credited(label)? != quote.credit_msat
            || ledger.asset_receipts()?.len() != 1
        {
            return Err("The original asset payment or credit changed after recovery.".into());
        }
        let invoice = receiver
            .0
            .lookup_invoice(openagents_wallet::parse_hash32(&quote.payment_hash)?)
            .await?;
        let received = invoice
            .htlcs
            .iter()
            .map(|h| {
                json!({"channel":h.chan_id,"amount_msat":h.amt_msat,
            "custom_channel_data":hex::encode(&h.custom_channel_data)})
            })
            .collect::<Vec<_>>();
        results.push(
            json!({"case":label,"asset_id":id,"units":units,"payment_hash":quote.payment_hash,
            "rfq_id":quote.rfq_id,"rate":quote.rate,"credited_msat":quote.credit_msat,
            "payment":paid,"received_asset_htlcs":received,"receipt":receipt,
            "original_hash_recovered":true,"ledger_restart_deduplicated":true}),
        );
    }
    let value = json!({"network":"signet","synthetic_only":true,"mainnet_approvals":false,"payments":results});
    let path = root.join("rfq-public-result.json");
    if !path.exists() {
        save(&path, &value)?;
    }
    if read(&path)? != value {
        return Err("The original asset results changed.".into());
    }
    Ok(value)
}
#[tokio::main]
async fn main() {
    let result: Result<Value> = async {
        let args: Vec<_> = std::env::args().skip(1).collect();
        if args.len() != 2 {
            return Err("Use ROOT inspect|fund-channel|exercise.".into());
        }
        let root = fs::canonicalize(&args[0])?;
        let expected =
            std::path::PathBuf::from(std::env::var_os("HOME").ok_or("Missing home directory.")?)
                .join("lnd-assets-signet-11863");
        if root != expected {
            return Err("Use the isolated signet asset research directory.".into());
        }
        let a = connect(&root, "a").await?;
        let b = connect(&root, "b").await?;
        match args[1].as_str() {
            "inspect" => Ok(
                json!({"network":"signet","a":a.0.node_id(),"b":b.0.node_id(),"tapd_ready":true}),
            ),
            "fund-channel" => fund_channel(&root, &a, &b).await,
            "exercise" => exercise(&root, &a, &b).await,
            _ => Err("Unknown research command.".into()),
        }
    }
    .await;
    match result {
        Ok(value) => println!("{value}"),
        Err(_) => {
            eprintln!(
                "The signet asset check stopped; inspect the original saved action before retrying."
            );
            std::process::exit(1);
        }
    }
}
