//! `openagents wallet`: the Lightning node this machine holds alone, for
//! both x402 roles. `serve` keeps the node online as the resident and
//! answers other wallet commands over `control.sock`; a command that finds
//! no resident opens the node, acts, and stops it.

use std::time::Duration;

use openagents_wallet::open::Opened;
use openagents_wallet::resident::Server;
use openagents_wallet::{
    LightningWallet, Network, WalletConfig, WalletError, config, parse_hash32,
};
use serde_json::{Value, json};

use crate::{Args, Output};

const USAGE: &str = "usage: openagents wallet COMMAND [OPTIONS]
  init [--network NET] [--esplora URL] [--listen HOST:PORT]
       [--lsp NODE_ID@HOST:PORT|olympus|mdk [--lsp-protocol lsps1|lsps2|lsps4]
        [--lsp-token TOKEN] [--lsp-min-msat N]] [--trust NODE_ID]...
       [--mnemonic -]
                          Write config.json and a seed. NET is bitcoin,
                          testnet, signet, or regtest (default signet).
                          --lsp olympus picks the Olympus (ZEUS) LSPS1
                          peer for the network; --lsp mdk picks the
                          MoneyDevKit LSPS4 peer (bitcoin, or Mutinynet
                          on signet, which also sets --esplora), whose
                          just-in-time channel needs no funding first. A
                          peer given by hand is LSPS2 unless
                          --lsp-protocol says otherwise. This node signs
                          its invoices on every protocol, as x402 requires
                          of payTo.
                          --trust lets that peer open anchor channels here
                          without an on-chain reserve (your own nodes).
                          Running init again keeps the seed. --mnemonic -
                          reads a BIP39 mnemonic from stdin to restore a
                          seed into a home that has none.
  info                    Node id (the x402 payTo), network, balances, paths,
                          and the last backup.
  export --reveal         Print the seed mnemonic. Refuses without --reveal.
  backup DIR              Copy the seed, config.json, and a consistent
                          snapshot of the node store into a new directory
                          DIR with a digest manifest; safe beside a resident.
  restore DIR             Verify DIR's manifest and copy it into an empty
                          wallet home. A restore from the seed alone leaves
                          channels behind; see `channel close --force`.
  status                  Chain sync state and queued node events.
  fund                    Print a fresh on-chain funding address.
  channel open NODE_ID@HOST:PORT --sats N [--announce] [--wait SECONDS]
                          Open a channel funded from the on-chain balance
                          and stay online until the funding transaction is
                          broadcast (state `pending`), up to --wait
                          (default 60).
  channel list            List channels with capacity and readiness.
  channel close USER_CHANNEL_ID COUNTERPARTY [--force]
                          Close a channel cooperatively (peer online), or
                          with --force broadcast the latest commitment.
                          After a restore without the store, ask the
                          counterparty to force-close and let the node sweep.
  channel buy --lsp-sats N [--our-sats N] [--expiry-blocks N] [--announce]
              [--pay lightning|onchain]
                          Order an inbound channel from the LSPS1 provider
                          set by init. Prints the order with its fee and
                          the BOLT11 and on-chain ways to pay it; --pay
                          settles from this wallet right away. Default
                          --expiry-blocks 13000 (about 90 days).
  channel order ORDER_ID  The LSP's current state of an order; `channel`
                          is set once the LSP has funded it.
  send ADDRESS --sats N   Send on-chain from the wallet's balance.
  invoice --msat N --request-hash HEX64 [--expiry SECONDS]
                          Issue an exact-amount BOLT11 whose description
                          hash is the request hash (x402 receiver).
  pay BOLT11 --max-fee-msat N [--wait SECONDS]
                          Pay within the fee cap and print the preimage
                          (x402 payer). Paying twice returns the same proof.
  lookup PAYMENT_HASH     The store's record of a payment, either direction.
  serve [--seconds N]     Keep the node online as the resident: print events
                          as JSON lines and answer other wallet and x402
                          commands over control.sock in the wallet home.
  service install|uninstall|status [--binary PATH]
                          Run `wallet serve` as a launchd agent or systemd
                          user unit, started at login.
Files live in ~/.openagents/wallet (OPENAGENTS_WALLET_HOME overrides). The
seed is printed only by `export --reveal`. Add --json before `wallet` for one JSON document.
While a resident serves, every other command acts through it and `info`
reports `resident`; without one, each command opens and stops the node.";

const SWITCHES: &[&str] = &["announce", "reveal", "force"];

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
        "export" => export(&args).map(|v| {
            (
                v,
                (|v: &Value| v["mnemonic"].as_str().unwrap_or("").to_owned()) as _,
            )
        }),
        "backup" => backup(&args).map(|v| (v, render_json as _)),
        "restore" => restore(&args).map(|v| (v, render_json as _)),
        "status" => with_node(|wallet| {
            let mut events = Vec::new();
            while let Some(event) = wallet.next_event()? {
                events.push(event);
            }
            Ok(json!({ "node_id": wallet.node_id(), "status": wallet.status()?, "events": events }))
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
        "send" => send(&args).map(|v| (v, render_json as _)),
        "invoice" => invoice(&args).map(|v| {
            (
                v,
                (|v: &Value| v["bolt11"].as_str().unwrap_or("").to_owned()) as _,
            )
        }),
        "pay" => pay(&args).map(|v| (v, render_json as _)),
        "lookup" => lookup(&args).map(|v| (v, render_json as _)),
        "serve" => return serve(output, &args),
        "service" => return service::run(output, &args, USAGE),
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
    let lsp_protocol = args
        .option("lsp-protocol")
        .map(config::LspProtocol::parse)
        .transpose()?;
    let lsp = match args.option("lsp") {
        Some(text) => Some(config::Lsp::parse_or_preset(
            text,
            args.option("lsp-token"),
            lsp_protocol,
            network,
        )?),
        None => existing.as_ref().and_then(|c| c.lsp.clone()),
    };
    let esplora = args
        .option("esplora")
        .or_else(|| {
            lsp.as_ref()
                .filter(|_| args.option("lsp").is_some())
                .and_then(|lsp| lsp.esplora_override(network))
        })
        .or(existing.as_ref().map(|c| c.esplora_url.as_str()));
    let mut wallet_config = WalletConfig::new(network, esplora)?;
    wallet_config.listen = args
        .option("listen")
        .map(str::to_owned)
        .or(existing.as_ref().and_then(|c| c.listen.clone()));
    wallet_config.lsp = lsp;
    if let Some(text) = args.option("lsp-min-msat") {
        let min: u64 = text
            .parse()
            .map_err(|_| Failure::Usage(format!("--lsp-min-msat takes a number, not `{text}`")))?;
        match wallet_config.lsp.as_mut() {
            Some(lsp) => lsp.min_payment_msat = (min > 0).then_some(min),
            None => {
                return Err(Failure::Usage(
                    "--lsp-min-msat needs an LSP (--lsp)".to_string(),
                ));
            }
        }
    }
    let trusted = args.options("trust");
    wallet_config.trusted_peers = if trusted.is_empty() {
        existing.map(|c| c.trusted_peers).unwrap_or_default()
    } else {
        trusted
            .into_iter()
            .map(config::parse_node_id)
            .collect::<Result<_, _>>()?
    };
    let restored = match args.option("mnemonic") {
        None => None,
        Some("-") => {
            if home.join(config::SEED_FILE).exists() {
                return Err(Failure::Wallet(WalletError::Setup(format!(
                    "{} already holds a seed; --mnemonic restores only into an empty home",
                    home.display()
                ))));
            }
            let mut text = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut text)
                .map_err(|error| Failure::Usage(format!("stdin: {error}")))?;
            let words: Vec<&str> = text.split_whitespace().collect();
            if !matches!(words.len(), 12 | 15 | 18 | 21 | 24) {
                return Err(Failure::Usage(format!(
                    "--mnemonic - read {} words; a BIP39 mnemonic has 12, 15, 18, 21, or 24",
                    words.len()
                )));
            }
            Some(words.join(" "))
        }
        Some(other) => {
            return Err(Failure::Usage(format!(
                "--mnemonic takes `-` (read stdin), not `{other}`; the seed never goes on a command line"
            )));
        }
    };
    wallet_config.save(&home)?;
    let (_, created) = config::load_or_create_seed(&home, true, || {
        restored.unwrap_or_else(openagents_wallet::ldk::generate_mnemonic)
    })?;
    let wallet = open(&home, &wallet_config)?;
    let mut value = info(&wallet)?;
    wallet.stop()?;
    value["created"] = Value::Bool(created);
    Ok(value)
}

fn export(args: &Args) -> Result<Value, Failure> {
    if !args.switch("reveal") {
        return Err(Failure::Usage(
            "export prints the seed; add --reveal to confirm".to_string(),
        ));
    }
    let home = config::home();
    let (mnemonic, _) = config::load_or_create_seed(&home, false, String::new)?;
    Ok(json!({ "mnemonic": mnemonic, "home": home.display().to_string() }))
}

fn backup(args: &Args) -> Result<Value, Failure> {
    let dest = args
        .positional()
        .first()
        .cloned()
        .ok_or_else(|| "backup needs DIR".to_string())?;
    let home = config::home();
    let manifest = openagents_wallet::backup::write(&home, std::path::Path::new(&dest))?;
    let resident = openagents_wallet::resident::RemoteWallet::probe(&home).is_some();
    Ok(json!({
        "path": dest,
        "created_at": manifest.created_at,
        "store": manifest.store,
        "resident_running": resident,
        "files": manifest.files,
    }))
}

fn restore(args: &Args) -> Result<Value, Failure> {
    let source = args
        .positional()
        .first()
        .cloned()
        .ok_or_else(|| "restore needs DIR".to_string())?;
    let home = config::home();
    let manifest = openagents_wallet::backup::restore(std::path::Path::new(&source), &home)?;
    Ok(json!({
        "path": source,
        "home": home.display().to_string(),
        "created_at": manifest.created_at,
        "store": manifest.store,
        "files": manifest.files,
        "note": if manifest.store {
            "start the node once (`wallet info`) so it resyncs; do not run the old copy again"
        } else {
            "seed only: channels are not restored; ask each counterparty to force-close"
        },
    }))
}

fn open(home: &std::path::Path, wallet_config: &WalletConfig) -> Result<Opened, Failure> {
    let (mnemonic, _) = config::load_or_create_seed(home, false, String::new)?;
    Ok(Opened::open(home, wallet_config, &mnemonic)?)
}

fn with_node<T>(act: impl FnOnce(&Opened) -> Result<T, Failure>) -> Result<T, Failure> {
    let home = config::home();
    let wallet_config = WalletConfig::load(&home)?;
    let wallet = open(&home, &wallet_config)?;
    let result = act(&wallet);
    let stopped = wallet.stop();
    let value = result?;
    stopped?;
    Ok(value)
}

fn info(wallet: &Opened) -> Result<Value, Failure> {
    let home = config::home();
    let wallet_config = WalletConfig::load(&home)?;
    let balance = wallet.balance()?;
    let channels = wallet.channels()?;
    let resident = match wallet {
        Opened::Resident(remote) => {
            let status = remote.status()?;
            json!({ "pid": status.pid, "uptime_secs": status.uptime_secs, "socket": remote.path().display().to_string() })
        }
        Opened::Local(_) => Value::Null,
    };
    Ok(json!({
        "resident": resident,
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
        "last_backup": openagents_wallet::backup::last(&home),
    }))
}

fn render_info(value: &Value) -> String {
    let balance = &value["balance"];
    format!(
        "node {}\nnetwork {}  esplora {}\nonchain {} sats ({} spendable)  lightning {} sats\nchannels {} ({} usable)  inbound {} msat  outbound {} msat\nhome {}{}{}",
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
        },
        match value["resident"]["pid"].as_u64() {
            Some(pid) => format!(
                "\nresident pid {pid} up {}s",
                value["resident"]["uptime_secs"]
            ),
            None => "\nresident none (this command opened the node)".to_owned(),
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
            let wait: u64 = args.number("wait", 60)?;
            with_node(|wallet| {
                let user_channel_id = wallet.open_channel(node_id, address, sats, announce)?;
                let deadline = std::time::Instant::now() + Duration::from_secs(wait);
                let mut state = "negotiating";
                let mut events = Vec::new();
                while std::time::Instant::now() < deadline {
                    if wallet.is_resident() {
                        let listed = wallet
                            .channels()?
                            .into_iter()
                            .any(|c| c.user_channel_id == user_channel_id);
                        if listed {
                            state = "pending";
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(500));
                        continue;
                    }
                    match wallet.next_event()? {
                        Some(event) => {
                            let kind = event["event"].as_str().unwrap_or("");
                            let done = match kind {
                                "channel_pending" => Some("pending"),
                                "channel_closed" => Some("closed"),
                                _ => None,
                            };
                            events.push(event);
                            if let Some(done) = done {
                                state = done;
                                break;
                            }
                        }
                        None => std::thread::sleep(Duration::from_millis(200)),
                    }
                }
                Ok(json!({
                    "user_channel_id": user_channel_id,
                    "counterparty": node_id,
                    "address": address,
                    "sats": sats,
                    "announced": announce,
                    "state": state,
                    "events": events,
                }))
            })
        }
        Some("buy") => {
            let lsp_sats: u64 = args.number("lsp-sats", 0)?;
            if lsp_sats == 0 {
                return Err("channel buy needs --lsp-sats N".to_string().into());
            }
            let our_sats: u64 = args.number("our-sats", 0)?;
            let expiry_blocks: u32 = args.number("expiry-blocks", 13_000)?;
            let announce = args.switch("announce");
            let pay_with = match args.option("pay") {
                None => None,
                Some("lightning") => Some("lightning"),
                Some("onchain") => Some("onchain"),
                Some(other) => {
                    return Err(format!("--pay must be lightning or onchain, not `{other}`").into());
                }
            };
            with_node(|wallet| {
                let mut order = wallet.buy_channel(lsp_sats, our_sats, expiry_blocks, announce)?;
                order["paid"] = match pay_with {
                    None => Value::Null,
                    Some("lightning") => {
                        let bolt11 = order["bolt11"]["invoice"]
                            .as_str()
                            .ok_or_else(|| {
                                WalletError::Node("the order has no BOLT11 to pay".to_string())
                            })?
                            .to_owned();
                        let fee_total_sat = order["bolt11"]["fee_total_sat"].as_u64().unwrap_or(0);
                        let proof = wallet.pay(
                            &bolt11,
                            fee_total_sat.saturating_mul(10).max(1_000),
                            Duration::from_secs(60),
                        )?;
                        json!({ "via": "lightning", "proof": proof })
                    }
                    Some(_) => {
                        let address = order["onchain"]["address"]
                            .as_str()
                            .ok_or_else(|| {
                                WalletError::Node("the order has no on-chain address".to_string())
                            })?
                            .to_owned();
                        let total =
                            order["onchain"]["order_total_sat"]
                                .as_u64()
                                .ok_or_else(|| {
                                    WalletError::Node("the order has no on-chain total".to_string())
                                })?;
                        let txid = wallet.send_onchain(&address, total)?;
                        json!({ "via": "onchain", "txid": txid, "sats": total })
                    }
                };
                Ok(order)
            })
        }
        Some("close") => {
            let user_channel_id = positional
                .get(1)
                .ok_or_else(|| "channel close needs USER_CHANNEL_ID COUNTERPARTY".to_string())?;
            let counterparty = positional
                .get(2)
                .ok_or_else(|| "channel close needs USER_CHANNEL_ID COUNTERPARTY".to_string())?;
            let force = args.switch("force");
            with_node(|wallet| {
                wallet.close_channel(user_channel_id, counterparty, force)?;
                Ok(json!({
                    "user_channel_id": user_channel_id,
                    "counterparty": counterparty,
                    "force": force,
                    "state": if force { "force_closing" } else { "closing" },
                }))
            })
        }
        Some("order") => {
            let order_id = positional
                .get(1)
                .ok_or_else(|| "channel order needs ORDER_ID".to_string())?;
            with_node(|wallet| Ok(wallet.channel_order(order_id)?))
        }
        _ => Err("channel needs `open`, `list`, `close`, `buy`, or `order`"
            .to_string()
            .into()),
    }
}

fn send(args: &Args) -> Result<Value, Failure> {
    let address = args
        .positional()
        .first()
        .cloned()
        .ok_or_else(|| "send needs ADDRESS".to_string())?;
    let sats: u64 = args.number("sats", 0)?;
    if sats == 0 {
        return Err("send needs --sats N".to_string().into());
    }
    with_node(|wallet| {
        let txid = wallet.send_onchain(&address, sats)?;
        Ok(json!({ "txid": txid, "address": address, "sats": sats }))
    })
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

static SHUTDOWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

extern "C" fn on_signal(_: libc::c_int) {
    SHUTDOWN.store(true, std::sync::atomic::Ordering::Relaxed);
}

fn serve(output: &Output, args: &Args) -> u8 {
    use std::sync::Arc;
    use std::sync::atomic::Ordering;

    let seconds: u64 = match args.number("seconds", 0) {
        Ok(seconds) => seconds,
        Err(message) => return output.usage("wallet", &message, USAGE),
    };
    let home = config::home();
    let server = match Server::bind(&home) {
        Ok(server) => server,
        Err(error) => return output.fail("wallet", &error.to_string()),
    };
    let wallet = match WalletConfig::load(&home)
        .map_err(Failure::from)
        .and_then(|c| {
            let (mnemonic, _) = config::load_or_create_seed(&home, false, String::new)?;
            Ok(openagents_wallet::ldk::LdkWallet::open(
                &home, &c, &mnemonic,
            )?)
        }) {
        Ok(wallet) => Arc::new(wallet),
        Err(Failure::Usage(message)) => return output.usage("wallet", &message, USAGE),
        Err(Failure::Wallet(error)) => return output.fail("wallet", &error.to_string()),
    };
    // SAFETY: the handler only stores to an atomic.
    unsafe {
        libc::signal(libc::SIGINT, on_signal as *const () as usize);
        libc::signal(libc::SIGTERM, on_signal as *const () as usize);
    }
    output.line(
        &json!({
            "event": "serving",
            "node_id": wallet.node_id(),
            "pid": std::process::id(),
            "socket": server.path().display().to_string(),
            "status": wallet.status(),
        }),
        |value| {
            format!(
                "serving {} (pid {}, socket {})",
                value["node_id"].as_str().unwrap_or(""),
                value["pid"],
                value["socket"].as_str().unwrap_or("")
            )
        },
    );
    let stop = server.stop_flag();
    let served = Arc::clone(&wallet);
    let accepting = std::thread::spawn(move || server.run(served));
    let deadline = (seconds > 0).then(|| std::time::Instant::now() + Duration::from_secs(seconds));
    let mut failed = None;
    let mut next_dial = std::time::Instant::now();
    loop {
        if std::time::Instant::now() >= next_dial {
            let dialed = wallet.dial_stored_peers();
            if dialed > 0 {
                output.line(
                    &json!({ "event": "peers_dialed", "count": dialed }),
                    render_json,
                );
            }
            next_dial =
                std::time::Instant::now() + openagents_wallet::ldk::LdkWallet::peer_dial_interval();
        }
        match wallet.next_event() {
            Ok(Some(event)) => output.line(&event, render_json),
            Ok(None) => std::thread::sleep(Duration::from_millis(250)),
            Err(error) => {
                failed = Some(error);
                break;
            }
        }
        if SHUTDOWN.load(Ordering::Relaxed)
            || deadline.is_some_and(|deadline| std::time::Instant::now() >= deadline)
        {
            break;
        }
    }
    stop.store(true, Ordering::Relaxed);
    let _ = accepting.join();
    output.line(&json!({ "event": "stopping" }), |_| "stopping".to_owned());
    match (wallet.stop(), failed) {
        (Ok(()), None) => 0,
        (_, Some(error)) | (Err(error), None) => output.fail("wallet", &error.to_string()),
    }
}

mod service {
    //! `wallet service`: a launchd agent or systemd user unit that runs
    //! `openagents wallet serve` from login on, so the resident is there
    //! whenever a payer or another command needs it.

    use std::path::PathBuf;
    use std::process::Command;

    use coder_service::service::Platform;
    use serde_json::{Value, json};

    use crate::{Args, Output};

    const LABEL: &str = "com.openagents.wallet";

    pub fn run(output: &Output, args: &Args, usage: &str) -> u8 {
        let Some(platform) = Platform::current() else {
            return output.fail("wallet", "wallet service needs macOS or Linux");
        };
        let home_dir = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let path = platform
            .default_registration_dir(&home_dir)
            .join(match platform {
                Platform::Macos => format!("{LABEL}.plist"),
                Platform::Linux => format!("{LABEL}.service"),
            });
        let result = match args.positional().first().map(String::as_str) {
            Some("install") => install(platform, &path, args.option("binary")),
            Some("uninstall") => uninstall(platform, &path),
            Some("status") => status(platform, &path),
            _ => {
                return output.usage(
                    "wallet",
                    "service needs install, uninstall, or status",
                    usage,
                );
            }
        };
        match result {
            Ok(value) => {
                output.emit(&value, |v| {
                    format!(
                        "{} {} ({})",
                        v["service"].as_str().unwrap_or(""),
                        v["state"].as_str().unwrap_or(""),
                        v["path"].as_str().unwrap_or("")
                    )
                });
                0
            }
            Err(message) => output.fail("wallet", &message),
        }
    }

    fn binary(flag: Option<&str>) -> Result<PathBuf, String> {
        match flag {
            Some(path) => Ok(PathBuf::from(path)),
            None => std::env::current_exe().map_err(|error| format!("current binary: {error}")),
        }
    }

    fn render(
        platform: Platform,
        binary: &std::path::Path,
        wallet_home: &std::path::Path,
    ) -> String {
        let binary = binary.display();
        let wallet_home = wallet_home.display();
        match platform {
            Platform::Macos => format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>{LABEL}</string>
  <key>ProgramArguments</key>
  <array><string>{binary}</string><string>--json</string><string>wallet</string><string>serve</string></array>
  <key>EnvironmentVariables</key>
  <dict><key>OPENAGENTS_WALLET_HOME</key><string>{wallet_home}</string></dict>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
  <key>StandardOutPath</key><string>{wallet_home}/serve.log</string>
  <key>StandardErrorPath</key><string>{wallet_home}/serve.log</string>
</dict>
</plist>
"#
            ),
            Platform::Linux => format!(
                "[Unit]\nDescription=openagents wallet resident node\nAfter=network-online.target\n\n[Service]\nEnvironment=OPENAGENTS_WALLET_HOME={wallet_home}\nExecStart={binary} --json wallet serve\nRestart=always\nRestartSec=5\n\n[Install]\nWantedBy=default.target\n"
            ),
        }
    }

    fn sh(program: &str, args: &[&str]) -> Result<String, String> {
        let done = Command::new(program)
            .args(args)
            .output()
            .map_err(|error| format!("{program}: {error}"))?;
        let text = String::from_utf8_lossy(&done.stdout).into_owned()
            + &String::from_utf8_lossy(&done.stderr);
        if done.status.success() {
            Ok(text)
        } else {
            Err(format!("{program} {}: {}", args.join(" "), text.trim()))
        }
    }

    fn uid() -> String {
        // SAFETY: getuid has no preconditions.
        unsafe { libc::getuid() }.to_string()
    }

    fn install(
        platform: Platform,
        path: &std::path::Path,
        flag: Option<&str>,
    ) -> Result<Value, String> {
        let binary = binary(flag)?;
        let wallet_home = openagents_wallet::config::home();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("{}: {error}", parent.display()))?;
        }
        std::fs::write(path, render(platform, &binary, &wallet_home))
            .map_err(|error| format!("{}: {error}", path.display()))?;
        match platform {
            Platform::Macos => {
                let domain = format!("gui/{}", uid());
                let _ = sh("launchctl", &["bootout", &format!("{domain}/{LABEL}")]);
                sh(
                    "launchctl",
                    &["bootstrap", &domain, &path.display().to_string()],
                )?;
            }
            Platform::Linux => {
                sh("systemctl", &["--user", "daemon-reload"])?;
                sh(
                    "systemctl",
                    &["--user", "enable", "--now", &format!("{LABEL}.service")],
                )?;
            }
        }
        Ok(json!({
            "service": LABEL,
            "state": "installed",
            "path": path.display().to_string(),
            "binary": binary.display().to_string(),
            "wallet_home": wallet_home.display().to_string(),
        }))
    }

    fn uninstall(platform: Platform, path: &std::path::Path) -> Result<Value, String> {
        match platform {
            Platform::Macos => {
                let _ = sh("launchctl", &["bootout", &format!("gui/{}/{LABEL}", uid())]);
            }
            Platform::Linux => {
                let _ = sh(
                    "systemctl",
                    &["--user", "disable", "--now", &format!("{LABEL}.service")],
                );
            }
        }
        let existed = path.exists();
        if existed {
            std::fs::remove_file(path).map_err(|error| format!("{}: {error}", path.display()))?;
        }
        if platform == Platform::Linux {
            let _ = sh("systemctl", &["--user", "daemon-reload"]);
        }
        Ok(json!({
            "service": LABEL,
            "state": if existed { "removed" } else { "absent" },
            "path": path.display().to_string(),
        }))
    }

    fn status(platform: Platform, path: &std::path::Path) -> Result<Value, String> {
        let registered = path.exists();
        let (active, detail) = match platform {
            Platform::Macos => match sh("launchctl", &["print", &format!("gui/{}/{LABEL}", uid())])
            {
                Ok(text) => (text.contains("state = running"), text),
                Err(text) => (false, text),
            },
            Platform::Linux => match sh(
                "systemctl",
                &["--user", "is-active", &format!("{LABEL}.service")],
            ) {
                Ok(text) => (text.trim() == "active", text),
                Err(text) => (false, text),
            },
        };
        let wallet_home = openagents_wallet::config::home();
        let resident = openagents_wallet::resident::RemoteWallet::probe(&wallet_home)
            .and_then(|remote| remote.status().ok())
            .map(|status| json!({ "pid": status.pid, "uptime_secs": status.uptime_secs }));
        Ok(json!({
            "service": LABEL,
            "state": if !registered { "absent" } else if active { "running" } else { "stopped" },
            "path": path.display().to_string(),
            "resident": resident,
            "detail": detail.trim(),
        }))
    }
}
