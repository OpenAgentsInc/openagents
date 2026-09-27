//! `openagents wallet`: the Lightning node this machine holds alone, for
//! both x402 roles. Every command opens the node, syncs, acts, and stops;
//! `serve` keeps it running so payers can reach it.

use std::time::Duration;

use openagents_wallet::ldk::LdkWallet;
use openagents_wallet::{
    LightningWallet, Network, WalletConfig, WalletError, config, parse_hash32,
};
use serde_json::{Value, json};

use crate::{Args, Output};

const USAGE: &str = "usage: openagents wallet COMMAND [OPTIONS]
  init [--network NET] [--esplora URL] [--listen HOST:PORT]
       [--lsp NODE_ID@HOST:PORT [--lsp-token TOKEN]] [--trust NODE_ID]...
                          Write config.json and a seed. NET is bitcoin,
                          testnet, signet, or regtest (default signet).
                          --trust lets that peer open anchor channels here
                          without an on-chain reserve (your own nodes).
                          Running init again keeps the seed.
  info                    Node id (the x402 payTo), network, balances, paths.
  status                  Chain sync state and queued node events.
  fund                    Print a fresh on-chain funding address.
  channel open NODE_ID@HOST:PORT --sats N [--announce]
                          Open a channel funded from the on-chain balance.
  channel list            List channels with capacity and readiness.
  invoice --msat N --request-hash HEX64 [--expiry SECONDS]
                          Issue an exact-amount BOLT11 whose description
                          hash is the request hash (x402 receiver).
  pay BOLT11 --max-fee-msat N [--wait SECONDS]
                          Pay within the fee cap and print the preimage
                          (x402 payer). Paying twice returns the same proof.
  lookup PAYMENT_HASH     The store's record of a payment, either direction.
  serve [--seconds N]     Keep the node online and print events as JSON lines.
Files live in ~/.openagents/wallet (OPENAGENTS_WALLET_HOME overrides). The
seed is never printed. Add --json before `wallet` for one JSON document.";

const SWITCHES: &[&str] = &["announce"];

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        return output.usage("wallet", "a command is required", USAGE);
    };
    let args = match Args::parse(rest, SWITCHES) {
        Ok(args) => args,
        Err(message) => return output.usage("wallet", &message, USAGE),
    };
    let home = config::home();
    let result = match command.as_str() {
        "--help" | "-h" | "help" => {
            println!("{USAGE}");
            return 0;
        }
        "init" => init(&args).map(|value| (value, render_info as fn(&Value) -> String)),
        "info" => with_node(info).map(|v| (v, render_info as _)),
        "status" => with_node(|wallet| {
            let mut events = Vec::new();
            while let Some(event) = wallet.next_event()? {
                events.push(event);
            }
            Ok(json!({ "node_id": wallet.node_id(), "status": wallet.status(), "events": events }))
        })
        .map(|v| (v, render_json as _)),
        "fund" => with_node(|wallet| {
            Ok(json!({ "address": wallet.funding_address()?, "network": network_name(&home)? }))
        })
        .map(|v| {
            (
                v,
                (|v: &Value| v["address"].as_str().unwrap_or("").to_owned()) as _,
            )
        }),
        "channel" => channel(&args).map(|v| (v, render_json as _)),
        "invoice" => invoice(&args).map(|v| {
            (
                v,
                (|v: &Value| v["bolt11"].as_str().unwrap_or("").to_owned()) as _,
            )
        }),
        "pay" => pay(&args).map(|v| (v, render_json as _)),
        "lookup" => lookup(&args).map(|v| (v, render_json as _)),
        "serve" => return serve(output, &args),
        other => return output.usage("wallet", &format!("unknown command `{other}`"), USAGE),
    };
    match result {
        Ok((value, render)) => {
            output.emit(&value, render);
            0
        }
        Err(Failure::Usage(message)) => output.usage("wallet", &message, USAGE),
        Err(Failure::Wallet(WalletError::Pending {
            payment_hash,
            waited_secs,
        })) => {
            if output.json() {
                println!(
                    "{}",
                    json!({
                        "error": format!("payment {payment_hash} is still pending after {waited_secs}s"),
                        "payment_hash": payment_hash,
                        "state": "pending",
                    })
                );
            }
            eprintln!(
                "openagents wallet: payment {payment_hash} is still pending after {waited_secs}s; run `openagents wallet lookup {payment_hash}` before paying again"
            );
            crate::EXIT_FAILURE
        }
        Err(Failure::Wallet(error)) => output.fail("wallet", &error.to_string()),
    }
}

enum Failure {
    Usage(String),
    Wallet(WalletError),
}

impl From<WalletError> for Failure {
    fn from(error: WalletError) -> Self {
        match error {
            WalletError::Invalid(message) => Self::Usage(message),
            other => Self::Wallet(other),
        }
    }
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Self::Usage(message)
    }
}

fn network_name(home: &std::path::Path) -> Result<&'static str, Failure> {
    Ok(WalletConfig::load(home)?.network.as_str())
}

fn init(args: &Args) -> Result<Value, Failure> {
    let home = config::home();
    let existing = WalletConfig::load(&home).ok();
    let network = match args.option("network") {
        Some(text) => Network::parse(text)?,
        None => existing.as_ref().map_or(Network::Signet, |c| c.network),
    };
    if let Some(existing) = &existing
        && existing.network != network
    {
        return Err(Failure::Wallet(WalletError::Setup(format!(
            "wallet at {} is on {}; a node cannot change network",
            home.display(),
            existing.network.as_str()
        ))));
    }
    let mut wallet_config = WalletConfig::new(
        network,
        args.option("esplora")
            .or(existing.as_ref().map(|c| c.esplora_url.as_str())),
    )?;
    wallet_config.listen = args
        .option("listen")
        .map(str::to_owned)
        .or(existing.as_ref().and_then(|c| c.listen.clone()));
    wallet_config.lsp = match args.option("lsp") {
        Some(text) => Some(config::Lsp::parse(text, args.option("lsp-token"))?),
        None => existing.as_ref().and_then(|c| c.lsp.clone()),
    };
    let trusted = args.options("trust");
    wallet_config.trusted_peers = if trusted.is_empty() {
        existing.map(|c| c.trusted_peers).unwrap_or_default()
    } else {
        trusted
            .into_iter()
            .map(config::parse_node_id)
            .collect::<Result<_, _>>()?
    };
    wallet_config.save(&home)?;
    let (_, created) =
        config::load_or_create_seed(&home, true, openagents_wallet::ldk::generate_mnemonic)?;
    let wallet = open(&home, &wallet_config)?;
    let mut value = info(&wallet)?;
    wallet.stop()?;
    value["created"] = Value::Bool(created);
    Ok(value)
}

fn open(home: &std::path::Path, wallet_config: &WalletConfig) -> Result<LdkWallet, Failure> {
    let (mnemonic, _) = config::load_or_create_seed(home, false, String::new)?;
    Ok(LdkWallet::open(home, wallet_config, &mnemonic)?)
}

fn with_node<T>(act: impl FnOnce(&LdkWallet) -> Result<T, Failure>) -> Result<T, Failure> {
    let home = config::home();
    let wallet_config = WalletConfig::load(&home)?;
    let wallet = open(&home, &wallet_config)?;
    let result = act(&wallet);
    let stopped = wallet.stop();
    let value = result?;
    stopped?;
    Ok(value)
}

fn info(wallet: &LdkWallet) -> Result<Value, Failure> {
    let home = config::home();
    let wallet_config = WalletConfig::load(&home)?;
    let balance = wallet.balance()?;
    let channels = wallet.channels()?;
    Ok(json!({
        "node_id": wallet.node_id(),
        "network": wallet_config.network.as_str(),
        "esplora_url": wallet_config.esplora_url,
        "listen": wallet_config.listen,
        "lsp": wallet_config.lsp,
        "trusted_peers": wallet_config.trusted_peers,
        "balance": balance,
        "channels": channels.len(),
        "usable_channels": channels.iter().filter(|c| c.usable).count(),
        "inbound_msat": channels.iter().map(|c| c.inbound_msat).sum::<u64>(),
        "outbound_msat": channels.iter().map(|c| c.outbound_msat).sum::<u64>(),
        "home": home.display().to_string(),
    }))
}

fn render_info(value: &Value) -> String {
    let balance = &value["balance"];
    format!(
        "node {}\nnetwork {}  esplora {}\nonchain {} sats ({} spendable)  lightning {} sats\nchannels {} ({} usable)  inbound {} msat  outbound {} msat\nhome {}{}",
        value["node_id"].as_str().unwrap_or(""),
        value["network"].as_str().unwrap_or(""),
        value["esplora_url"].as_str().unwrap_or(""),
        balance["onchain_total_sats"],
        balance["onchain_spendable_sats"],
        balance["lightning_total_sats"],
        value["channels"],
        value["usable_channels"],
        value["inbound_msat"],
        value["outbound_msat"],
        value["home"].as_str().unwrap_or(""),
        if value["created"].as_bool().unwrap_or(false) {
            " (new seed)"
        } else {
            ""
        }
    )
}

fn render_json(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_default()
}

fn channel(args: &Args) -> Result<Value, Failure> {
    let positional = args.positional();
    match positional.first().map(String::as_str) {
        Some("list") => with_node(|wallet| Ok(json!({ "channels": wallet.channels()? }))),
        Some("open") => {
            let target = positional
                .get(1)
                .ok_or_else(|| "channel open needs NODE_ID@HOST:PORT".to_string())?;
            let (node_id, address) = target
                .split_once('@')
                .ok_or_else(|| "channel open needs NODE_ID@HOST:PORT".to_string())?;
            let sats: u64 = args.number("sats", 0)?;
            if sats == 0 {
                return Err("channel open needs --sats N".to_string().into());
            }
            let announce = args.switch("announce");
            with_node(|wallet| {
                let user_channel_id = wallet.open_channel(node_id, address, sats, announce)?;
                Ok(json!({
                    "user_channel_id": user_channel_id,
                    "counterparty": node_id,
                    "address": address,
                    "sats": sats,
                    "announced": announce,
                    "state": "pending",
                }))
            })
        }
        _ => Err("channel needs `open` or `list`".to_string().into()),
    }
}

fn invoice(args: &Args) -> Result<Value, Failure> {
    let msat: u64 = args.number("msat", 0)?;
    if msat == 0 {
        return Err("invoice needs --msat N (positive)".to_string().into());
    }
    let request_hash = parse_hash32(
        args.option("request-hash")
            .ok_or_else(|| "invoice needs --request-hash HEX64".to_string())?,
    )?;
    let expiry: u32 = args.number("expiry", 600)?;
    with_node(|wallet| {
        Ok(
            serde_json::to_value(wallet.receive_exact(msat, request_hash, expiry)?)
                .unwrap_or_default(),
        )
    })
}

fn pay(args: &Args) -> Result<Value, Failure> {
    let bolt11 = args
        .positional()
        .first()
        .ok_or_else(|| "pay needs a BOLT11 invoice".to_string())?;
    let max_fee: u64 = args
        .option("max-fee-msat")
        .ok_or_else(|| "pay needs --max-fee-msat N".to_string())?
        .parse()
        .map_err(|_| "--max-fee-msat takes a number".to_string())?;
    let wait: u64 = args.number("wait", 60)?;
    with_node(|wallet| {
        Ok(
            serde_json::to_value(wallet.pay(bolt11, max_fee, Duration::from_secs(wait))?)
                .unwrap_or_default(),
        )
    })
}

fn lookup(args: &Args) -> Result<Value, Failure> {
    let hash = parse_hash32(
        args.positional()
            .first()
            .ok_or_else(|| "lookup needs a payment hash".to_string())?,
    )?;
    with_node(|wallet| {
        let record = wallet.lookup(hash)?;
        Ok(json!({
            "payment_hash": hex_lower(&hash),
            "known": record.is_some(),
            "payment": record,
        }))
    })
}

fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn serve(output: &Output, args: &Args) -> u8 {
    let seconds: u64 = match args.number("seconds", 0) {
        Ok(seconds) => seconds,
        Err(message) => return output.usage("wallet", &message, USAGE),
    };
    let home = config::home();
    let wallet = match WalletConfig::load(&home)
        .map_err(Failure::from)
        .and_then(|c| open(&home, &c))
    {
        Ok(wallet) => wallet,
        Err(Failure::Usage(message)) => return output.usage("wallet", &message, USAGE),
        Err(Failure::Wallet(error)) => return output.fail("wallet", &error.to_string()),
    };
    output.line(
        &json!({ "event": "serving", "node_id": wallet.node_id(), "status": wallet.status() }),
        |value| format!("serving {}", value["node_id"].as_str().unwrap_or("")),
    );
    let deadline = (seconds > 0).then(|| std::time::Instant::now() + Duration::from_secs(seconds));
    loop {
        match wallet.next_event() {
            Ok(Some(event)) => output.line(&event, render_json),
            Ok(None) => std::thread::sleep(Duration::from_millis(250)),
            Err(error) => {
                let _ = wallet.stop();
                return output.fail("wallet", &error.to_string());
            }
        }
        if deadline.is_some_and(|deadline| std::time::Instant::now() >= deadline) {
            break;
        }
    }
    match wallet.stop() {
        Ok(()) => 0,
        Err(error) => output.fail("wallet", &error.to_string()),
    }
}
