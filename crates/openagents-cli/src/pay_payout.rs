//! `openagents pay payouts`: the payout worker on the pay host. It drains
//! the ledger's accrued shares to each payee's registered destination
//! (`pay_ledger::payout`, the restart-safe state machine) over two rails:
//!
//! - **Lightning address:** LNURL-pay (LUD-06/16) resolves an invoice for
//!   the exact amount, whose description hash must be the address's
//!   metadata, and the receiver wallet (`crates/wallet`, through the
//!   resident) pays it within the routing-fee cap.
//! - **Spark address:** the payout Spark wallet (Breez SDK, its own seed)
//!   sends a Spark transfer whose id is the payout id. When it holds too
//!   little, it issues a BOLT11 invoice that the receiver wallet pays.
//!
//! Destinations come from `Ledger::resolve_payee`: the signed release the
//! shares were earned under, then the author's newest signed NIP-A3 and
//! kind-0 events, read from the relay.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use nostr::domain::Event;
use openagents_spark::model::{Node as _, PayFailure, SendRequest};
use openagents_spark::spark::SparkNode;
use openagents_wallet::config::Network;
use openagents_wallet::open::Opened;
use openagents_wallet::{LightningWallet, PaymentDirection, PaymentStatus, WalletError};
use pay_ledger::payout::{Invoice, Lookup, Outcome, Policy, Rails, Step};
use pay_ledger::{Ledger, PayoutState};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{Args, Output};

/// How long a Lightning payout waits for its outcome before it is `unknown`.
const PAY_WAIT: Duration = Duration::from_secs(60);
/// The smallest Spark wallet top-up, in sats, so payouts do not each pay a
/// top-up invoice.
const TOP_UP_MIN_SATS: u64 = 1_000;

/// The real rails: the receiver wallet and the payout Spark wallet.
pub(crate) struct LiveRails {
    wallet: Arc<Opened>,
    /// The BOLT11 currency prefix of the wallet's network (`bc`, `tb`, `bcrt`).
    currency: &'static str,
    http: reqwest::blocking::Client,
    spark: Option<SparkNode>,
    top_up_min_sats: u64,
}

fn currency(network: Network) -> &'static str {
    match network {
        Network::Bitcoin => "bc",
        Network::Testnet | Network::Signet => "tb",
        Network::Regtest => "bcrt",
    }
}

fn get_json(http: &reqwest::blocking::Client, url: &str) -> Result<Value, String> {
    let response = http.get(url).send().map_err(|e| e.to_string())?;
    let status = response.status();
    let body: Value = response.json().map_err(|e| format!("{status}: {e}"))?;
    if body["status"].as_str() == Some("ERROR") {
        return Err(body["reason"]
            .as_str()
            .unwrap_or("the service refused")
            .into());
    }
    if !status.is_success() {
        return Err(format!("the service answered {status}"));
    }
    Ok(body)
}

/// The LUD-16 well-known URL for `user@domain`.
pub(crate) fn lnurlp_url(address: &str) -> Result<String, String> {
    let (user, domain) = address.split_once('@').ok_or("not a Lightning address")?;
    let ok = |s: &str| {
        !s.is_empty()
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.+".contains(&b))
    };
    if !ok(user) || !ok(domain) || !domain.contains('.') {
        return Err("not a Lightning address".into());
    }
    Ok(format!(
        "https://{}/.well-known/lnurlp/{}",
        domain.to_ascii_lowercase(),
        user.to_ascii_lowercase()
    ))
}

/// Check an LNURL pay request's terms against `amount_msat` and return its
/// callback URL with the amount, and the metadata the invoice must commit to.
pub(crate) fn pay_request_terms(
    body: &Value,
    amount_msat: i64,
) -> Result<(String, String), String> {
    if body["tag"].as_str() != Some("payRequest") {
        return Err("the address is not an LNURL pay request".into());
    }
    let min = body["minSendable"].as_i64().ok_or("no minSendable")?;
    let max = body["maxSendable"].as_i64().ok_or("no maxSendable")?;
    if amount_msat < min || amount_msat > max {
        return Err(format!(
            "the address takes {min}..={max} msat, not {amount_msat}"
        ));
    }
    let metadata = body["metadata"].as_str().ok_or("no metadata")?.to_owned();
    let callback = body["callback"].as_str().ok_or("no callback")?;
    let mut url = reqwest::Url::parse(callback).map_err(|_| "the callback is not a URL")?;
    if url.scheme() != "https" {
        return Err("the callback is not https".into());
    }
    url.query_pairs_mut()
        .append_pair("amount", &amount_msat.to_string());
    Ok((url.to_string(), metadata))
}

/// Authenticate the invoice an LNURL callback returned: the network, the
/// exact amount, and the description hash of the metadata (LUD-06).
pub(crate) fn check_invoice(
    bolt11: &str,
    currency: &str,
    amount_msat: i64,
    metadata: &str,
) -> Result<Invoice, String> {
    let invoice = nostr::x402::decode_invoice(bolt11)
        .map_err(|e| format!("the address returned an unreadable invoice: {e:?}"))?;
    if invoice.currency() != currency {
        return Err(format!(
            "the address returned an invoice for network {}",
            invoice.currency()
        ));
    }
    let hash: [u8; 32] = Sha256::digest(metadata.as_bytes()).into();
    if invoice.description_hash() != hash {
        return Err("the invoice does not commit to the address's metadata".into());
    }
    Ok(Invoice {
        bolt11: bolt11.to_owned(),
        payment_hash: invoice
            .payment_hash()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
        amount_msat: i64::try_from(invoice.amount_msat()).unwrap_or(i64::MAX),
    })
    .and_then(|found| {
        (found.amount_msat == amount_msat)
            .then_some(found.clone())
            .ok_or_else(|| {
                format!(
                    "the address returned an invoice for {} msat, not {amount_msat}",
                    found.amount_msat
                )
            })
    })
}

fn lookup_record(record: Option<openagents_wallet::PaymentRecord>) -> Result<Lookup, String> {
    let Some(record) = record else {
        return Ok(Lookup::Absent);
    };
    if record.direction != PaymentDirection::Outbound {
        return Err("the reference is an inbound payment".into());
    }
    Ok(match record.status {
        PaymentStatus::Succeeded => Lookup::Sent {
            fee_msat: record
                .fee_msat
                .and_then(|f| i64::try_from(f).ok())
                .unwrap_or(0),
        },
        PaymentStatus::Failed => Lookup::Failed("the payment failed".into()),
        PaymentStatus::Pending => Lookup::Pending,
    })
}

impl LiveRails {
    fn spark(&self) -> Result<&SparkNode, String> {
        self.spark
            .as_ref()
            .ok_or_else(|| "the payout Spark wallet is not set up on this host".into())
    }
}

impl Rails for LiveRails {
    fn spark_funding_invoice(&self, amount_sats: u64) -> Result<Option<Invoice>, String> {
        let spark = self.spark()?;
        spark.sync()?;
        let balance = spark.balance()?;
        if balance >= amount_sats {
            return Ok(None);
        }
        let top_up = (amount_sats - balance).max(self.top_up_min_sats);
        let bolt11 = spark.invoice(Some(top_up), "OpenAgents payout wallet top-up")?;
        let parsed: lightning_invoice::Bolt11Invoice =
            bolt11.parse().map_err(|_| "invalid Spark refill invoice")?;
        if parsed.amount_milli_satoshis() != top_up.checked_mul(1000) {
            return Err("incorrect Spark refill amount".into());
        }
        Ok(Some(Invoice {
            payment_hash: parsed.payment_hash().to_string(),
            bolt11,
            amount_msat: i64::try_from(top_up.checked_mul(1000).ok_or("invalid refill amount")?)
                .map_err(|_| "invalid refill amount")?,
        }))
    }
    fn await_spark_funding(&self, amount_sats: u64) -> Result<(), String> {
        let spark = self.spark()?;
        for _ in 0..30 {
            spark.sync()?;
            if spark.balance()? >= amount_sats {
                return Ok(());
            }
            std::thread::sleep(Duration::from_secs(2));
        }
        Err("the top-up has not reached the Spark wallet yet".into())
    }
    fn lightning_invoice(&self, address: &str, amount_msat: i64) -> Result<Invoice, String> {
        let terms = get_json(&self.http, &lnurlp_url(address)?)?;
        let (callback, metadata) = pay_request_terms(&terms, amount_msat)?;
        let answer = get_json(&self.http, &callback)?;
        let bolt11 = answer["pr"]
            .as_str()
            .ok_or("the callback returned no invoice")?;
        check_invoice(bolt11, self.currency, amount_msat, &metadata)
    }

    fn pay_lightning(&self, invoice: &Invoice, max_fee_msat: i64) -> Outcome {
        let cap = u64::try_from(max_fee_msat).unwrap_or(0);
        match self.wallet.pay(&invoice.bolt11, cap, PAY_WAIT) {
            Ok(proof) => Outcome::Sent {
                fee_msat: i64::try_from(proof.fee_msat).unwrap_or(0),
            },
            // Refused before dispatch, or failed back: nothing went out.
            Err(WalletError::Invalid(why)) => Outcome::Failed(why),
            Err(WalletError::Failed { reason, .. }) => Outcome::Failed(reason),
            Err(other) => Outcome::Unknown(other.to_string()),
        }
    }

    fn lookup_lightning(&self, payment_hash: &str) -> Result<Lookup, String> {
        let hash = openagents_wallet::parse_hash32(payment_hash).map_err(|e| e.to_string())?;
        lookup_record(self.wallet.lookup(hash).map_err(|e| e.to_string())?)
    }

    fn fund_spark(&self, amount_sats: u64) -> Result<(), String> {
        self.fund_spark_shadow(amount_sats, &|_| {})
    }
    fn fund_spark_shadow(
        &self,
        amount_sats: u64,
        observe: &dyn Fn(&Invoice),
    ) -> Result<(), String> {
        let Some(invoice) = self.spark_funding_invoice(amount_sats)? else {
            return Ok(());
        };
        observe(&invoice);
        let cap = (invoice.amount_msat / 100).max(5_000);
        self.wallet
            .pay(&invoice.bolt11, cap as u64, PAY_WAIT)
            .map_err(|e| format!("paying the top-up: {e}"))?;
        self.await_spark_funding(amount_sats)
    }

    fn pay_spark(&self, address: &str, amount_sats: u64, key: &str) -> Outcome {
        let spark = match self.spark() {
            Ok(spark) => spark,
            Err(why) => return Outcome::Failed(why),
        };
        let request = SendRequest {
            input: address.to_owned(),
            amount_sats: Some(amount_sats),
            ..SendRequest::default()
        };
        let quote = match spark.quote(&request) {
            Ok(quote) => quote,
            Err(failure) => return Outcome::Failed(format!("{failure:?}")),
        };
        if quote.amount_sats != amount_sats || quote.fee_sats > amount_sats / 100 {
            return Outcome::Failed(format!(
                "the quote is {} sats with a {} sat fee",
                quote.amount_sats, quote.fee_sats
            ));
        }
        match spark.pay(quote.id, key) {
            Ok(paid) => match paid.row.status.as_str() {
                "completed" => Outcome::Sent {
                    fee_msat: i64::try_from(paid.row.fee_sats).unwrap_or(0) * 1000,
                },
                "failed" => Outcome::Failed("the transfer failed".into()),
                _ => Outcome::Unknown("the transfer is pending".into()),
            },
            // Only a definite refusal is a failure; anything that may have
            // left stays unknown so the same key is asked again.
            Err(PayFailure::NotSent(why)) => Outcome::Failed(why),
            Err(PayFailure::Unknown(why)) => Outcome::Unknown(why),
        }
    }

    fn lookup_spark(&self, key: &str) -> Result<Lookup, String> {
        let spark = self.spark()?;
        spark.sync()?;
        let row = spark.payment(key)?;
        if row.id != key || row.received {
            return Err("the payout transfer does not match".into());
        }
        let row = Some(row);
        Ok(match row {
            None => Lookup::Absent,
            Some(row) => match row.status.as_str() {
                "completed" => Lookup::Sent {
                    fee_msat: i64::try_from(row.fee_sats).unwrap_or(0) * 1000,
                },
                "failed" => Lookup::Failed("the transfer failed".into()),
                _ => Lookup::Pending,
            },
        })
    }
}

/// Read what the relay knows about `pubkey`: the release the shares were
/// earned under, and the key's kind-0 and kind-10133 events.
fn relay_sources(relay: &str, pubkey: &str, release: Option<&str>) -> pay_ledger::payee::Sources {
    let mut sources = pay_ledger::payee::Sources {
        pubkey: Some(pubkey.to_owned()),
        ..Default::default()
    };
    let Ok(signer) = crate::relay::signer_for(None) else {
        return sources;
    };
    let mut client = crate::relay::Client::connect(relay, signer);
    let mut events: Vec<Event> = vec![];
    let mut filters =
        vec![json!({"authors": [pubkey], "kinds": [0, nostr::payto::PAYMENT_TARGETS_KIND]})];
    if let Some(release) = release {
        filters.push(json!({"ids": [release]}));
    }
    let _ = client.subscribe(filters, false, Duration::from_secs(10), |event| {
        events.push(event.clone());
    });
    client.close();
    sources.release = release.and_then(|id| events.iter().find(|e| e.id == id).cloned());
    sources.events = events;
    sources
}

fn is_pubkey(party: &str) -> bool {
    party.len() == 64
        && party
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn step_json(step: &Step) -> Value {
    match step {
        Step::Payout {
            id,
            party,
            rail,
            amount_msat,
            state,
            error,
        } => json!({"event": "payout", "id": id, "party": party, "rail": rail,
                    "amount_msat": amount_msat, "state": state.as_str(), "error": error}),
        Step::NoDestination { party, owed_msat } => {
            json!({"event": "no_destination", "party": party, "owed_msat": owed_msat})
        }
        Step::Unpayable { party, kind } => {
            json!({"event": "unpayable", "party": party, "kind": kind})
        }
        Step::BackingOff { party, until } => {
            json!({"event": "backing_off", "party": party, "until": until})
        }
    }
}

fn now() -> i64 {
    i64::try_from(crate::relay::unix_now()).unwrap_or(i64::MAX)
}

fn spark_home(args: &Args) -> PathBuf {
    args.option("spark-home")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("OPENAGENTS_PAY_SPARK_HOME").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("/var/lib/openagents-pay/spark"))
}

fn ledger_path(args: &Args) -> Option<PathBuf> {
    args.option("ledger")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("OPENAGENTS_PAY_LEDGER").map(PathBuf::from))
}

fn spark_network(network: Network) -> Option<openagents_spark::Network> {
    match network {
        Network::Bitcoin => Some(openagents_spark::Network::Mainnet),
        Network::Regtest => Some(openagents_spark::Network::Regtest),
        Network::Testnet | Network::Signet => None,
    }
}

fn open_spark(home: &Path, network: Network) -> Result<Option<SparkNode>, String> {
    let Some(seed) = openagents_spark::computer::load_seed(home)? else {
        return Ok(None);
    };
    let network = spark_network(network).ok_or("Spark has no network for this wallet")?;
    openagents_spark::computer::open_with(home, &seed, network).map(Some)
}

/// `pay payouts`: the worker loop.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct LndPayoutSettings {
    endpoint: String,
    certificate_sha256: String,
    node: String,
    network: String,
    account_macaroon: PathBuf,
    policy: PathBuf,
    observations: PathBuf,
    funding_journal: PathBuf,
}
fn routed_rails(
    path: &Path,
    old: Arc<LiveRails>,
    network: Network,
) -> Result<lnd_sdk::payout::Router<lnd_sdk::service::DeferredClient, LiveRails>, String> {
    let mut s: LndPayoutSettings = serde_json::from_slice(
        &std::fs::read(path).map_err(|_| "Could not read payout settings.")?,
    )
    .map_err(|_| "The payout settings are invalid.")?;
    let base = path.parent().unwrap_or(Path::new("."));
    for file in [
        &mut s.account_macaroon,
        &mut s.policy,
        &mut s.observations,
        &mut s.funding_journal,
    ] {
        if file.is_relative() {
            *file = base.join(&*file);
        }
    }
    let expected = match network {
        Network::Bitcoin => "mainnet",
        Network::Testnet => "testnet",
        Network::Signet => "signet",
        Network::Regtest => "regtest",
    };
    if s.network != expected {
        return Err("The payout wallets use different Bitcoin networks.".into());
    }
    let _: secp256k1::PublicKey = s
        .node
        .parse()
        .map_err(|_| "The payout node identity is invalid.")?;
    let pin = openagents_wallet::parse_hash32(&s.certificate_sha256)
        .map_err(|_| "The payout certificate fingerprint is invalid.")?;
    lnd_sdk::payout::Policy::load(&s.policy).map_err(|e| e.to_string())?;
    let rail = lnd_sdk::service::DeferredClient {
        config: lnd_sdk::transport::Config::new(s.endpoint, pin),
        macaroon: s.account_macaroon,
        node: s.node,
        network: s.network,
        connected: tokio::sync::Mutex::new(None),
    };
    Ok(lnd_sdk::payout::Router {
        old,
        wallet: lnd_sdk::service::Wallet::new(Arc::new(rail)).map_err(|e| e.to_string())?,
        policy: s.policy,
        observations: s.observations,
        funding_journal: s.funding_journal,
    })
}
pub(crate) fn payouts(output: &Output, words: &[String], usage: &str) -> u8 {
    let args = match Args::parse(words, &["once", "check"]) {
        Ok(args) => args,
        Err(message) => return output.usage("pay", &message, usage),
    };
    let Some(path) = ledger_path(&args) else {
        return output.usage("pay", "payouts needs --ledger FILE", usage);
    };
    if args.option("require-sent-after").is_some() && !args.switch("once") && !args.switch("check")
    {
        return output.usage("pay", "The restart payout check needs --once.", usage);
    }
    if args.switch("check") && args.option("require-sent-after").is_none() {
        return output.usage(
            "pay",
            "The payout check needs --require-sent-after TIME.",
            usage,
        );
    }
    let required_since = match args.option("require-sent-after") {
        Some(value) => match value.parse::<i64>() {
            Ok(time) if time > 0 && time <= now() => Some(time),
            _ => return output.usage("pay", "The payout check needs a valid restart time.", usage),
        },
        None => None,
    };
    let interval: u64 = match args.number("interval", 60) {
        Ok(n) => n,
        Err(message) => return output.usage("pay", &message, usage),
    };
    let relay = crate::relay::relay_url(args.option("relay"));
    let mut ledger = match Ledger::open(&path) {
        Ok(ledger) => ledger,
        Err(e) => return output.fail("pay", &format!("{}: {e}", path.display())),
    };
    let opened = if args.switch("check") {
        let home = openagents_wallet::config::home();
        openagents_wallet::WalletConfig::load(&home).and_then(|config| {
            openagents_wallet::resident::RemoteWallet::probe(&home)
                .map(|remote| (Opened::Resident(remote), config))
                .ok_or_else(|| WalletError::Node("No running receiving wallet answered.".into()))
        })
    } else {
        crate::x402::open_wallet()
    };
    let (wallet, config) = match opened {
        Ok(opened) => opened,
        Err(error) => return output.fail("pay", &error.to_string()),
    };
    let spark = match if args.switch("check") {
        Ok(None)
    } else {
        open_spark(&spark_home(&args), config.network)
    } {
        Ok(spark) => spark,
        Err(why) => {
            let _ = wallet.stop();
            return output.fail("pay", &format!("the payout Spark wallet: {why}"));
        }
    };
    let http = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::limited(3))
        .build()
    {
        Ok(http) => http,
        Err(e) => return output.fail("pay", &e.to_string()),
    };
    let old = Arc::new(LiveRails {
        wallet: Arc::new(wallet),
        currency: currency(config.network),
        http,
        spark,
        top_up_min_sats: TOP_UP_MIN_SATS,
    });
    let routed = match args.option("lnd") {
        Some(path) => match routed_rails(Path::new(path), old.clone(), config.network) {
            Ok(router) => Some(router),
            Err(why) => return output.fail("pay", &why),
        },
        None => None,
    };
    let rails: &dyn Rails = routed
        .as_ref()
        .map_or(old.as_ref() as &dyn Rails, |router| router as &dyn Rails);
    if !args.switch("check") {
        output.line(
            &json!({"event": "payouts", "ledger": path.display().to_string(), "relay": relay,
                "spark": old.spark.is_some(), "network": config.network.as_str(),
                "interval_secs": interval}),
            |v| {
                format!(
                    "paying out from {} every {}s (Spark rail {})",
                    v["ledger"].as_str().unwrap_or(""),
                    v["interval_secs"],
                    if v["spark"].as_bool() == Some(true) {
                        "on"
                    } else {
                        "off"
                    }
                )
            },
        );
    }
    let policy = Policy::default();
    loop {
        if args.switch("check") {
            break;
        }
        let mut resolve = |ledger: &mut Ledger, party: &str, at: i64| {
            let release = ledger.latest_release(party)?;
            ledger.resolve_payee(party, at, || {
                if is_pubkey(party) {
                    relay_sources(&relay, party, release.as_deref())
                } else {
                    pay_ledger::payee::Sources::default()
                }
            })
        };
        let mut new_id = || uuid::Uuid::new_v4().to_string();
        match pay_ledger::payout::tick(
            &mut ledger,
            rails,
            &policy,
            now(),
            &mut resolve,
            &mut new_id,
        ) {
            Ok(steps) => {
                for step in &steps {
                    output.line(&step_json(step), |v| v.to_string());
                }
            }
            Err(e) => {
                output.line(&json!({"event": "error", "error": e.to_string()}), |v| {
                    v.to_string()
                });
            }
        }
        if let Ok(rows) = ledger.payouts(Some(&[PayoutState::Unknown])) {
            let overdue = rows
                .iter()
                .filter(|row| now().saturating_sub(row.created_at) > 3600)
                .count();
            if overdue > 0 {
                output.line(
                    &json!({"event": "payouts_need_attention", "over_one_hour": overdue}),
                    |_| format!("{overdue} payouts need payment confirmation."),
                );
            }
        }
        if args.switch("once") {
            break;
        }
        std::thread::sleep(Duration::from_secs(interval.max(1)));
    }
    if let Some(since) = required_since {
        let rows = match ledger.payouts(None) {
            Ok(rows) => rows,
            Err(_) => {
                let _ = old.wallet.stop();
                return output.fail("pay", "Could not read payouts for the restart check.");
            }
        };
        let overdue = rows.iter().any(|row| {
            row.state == PayoutState::Unknown && now().saturating_sub(row.created_at) > 3600
        });
        if overdue {
            let _ = old.wallet.stop();
            return output.fail(
                "pay",
                "A payout has needed payment confirmation for over an hour.",
            );
        }
        let confirmed = rows.iter().any(|row| {
            row.state == PayoutState::Sent
                && row.created_at >= since
                && row.updated_at >= since
                && row.rail == "lightning"
                && row
                    .wallet_reference
                    .as_deref()
                    .is_some_and(|r| matches!(rails.lookup_lightning(r), Ok(Lookup::Sent { .. })))
        });
        if !confirmed {
            let _ = old.wallet.stop();
            return output.fail(
                "pay",
                "No completed payout has been confirmed since the restart.",
            );
        }
        output.line(
            &json!({"event": "payout_check", "confirmed": true, "since": since}),
            |_| "Payout confirmed after restart.".into(),
        );
    }
    match old.wallet.stop() {
        Ok(()) => 0,
        Err(e) => output.fail("pay", &e.to_string()),
    }
}

/// `pay payout-list`: the payouts in the ledger.
pub(crate) fn list(output: &Output, words: &[String], usage: &str) -> u8 {
    let args = match Args::parse(words, &["open"]) {
        Ok(args) => args,
        Err(message) => return output.usage("pay", &message, usage),
    };
    let Some(path) = ledger_path(&args) else {
        return output.usage("pay", "payout-list needs --ledger FILE", usage);
    };
    if !path.is_file() {
        return output.fail("pay", &format!("no ledger at {}", path.display()));
    }
    let ledger = match Ledger::open(&path) {
        Ok(ledger) => ledger,
        Err(e) => return output.fail("pay", &format!("{}: {e}", path.display())),
    };
    let open = [
        PayoutState::Planned,
        PayoutState::Sending,
        PayoutState::Unknown,
    ];
    let payouts = match ledger.payouts(args.switch("open").then_some(&open[..])) {
        Ok(payouts) => payouts,
        Err(e) => return output.fail("pay", &e.to_string()),
    };
    if payouts.is_empty() && !output.json() {
        println!("No payouts.");
    }
    for p in payouts {
        output.line(
            &json!({"id": p.id, "party": p.party, "rail": p.rail, "state": p.state.as_str(),
                    "amount_msat": p.amount_msat, "sent_msat": p.sent_msat, "fee_msat": p.fee_msat,
                    "wallet_reference": p.wallet_reference, "attempts": p.attempts,
                    "created_at": p.created_at, "updated_at": p.updated_at, "error": p.error}),
            |v| {
                format!(
                    "{} {} {} {} msat {}",
                    v["id"].as_str().unwrap_or(""),
                    v["rail"].as_str().unwrap_or(""),
                    v["state"].as_str().unwrap_or(""),
                    v["amount_msat"],
                    v["error"].as_str().unwrap_or("")
                )
            },
        );
    }
    0
}

/// `pay payout-spark-init`: a fresh seed for the payout Spark wallet, kept
/// in the Spark home. Prints the wallet's Spark address, never the seed.
pub(crate) fn spark_init(output: &Output, words: &[String], usage: &str) -> u8 {
    let args = match Args::parse(words, &[]) {
        Ok(args) => args,
        Err(message) => return output.usage("pay", &message, usage),
    };
    let home = spark_home(&args);
    let made = (|| -> Result<String, String> {
        if openagents_spark::computer::has_seed(&home) {
            return Err(format!("{} already has a seed", home.display()));
        }
        let words = openagents_wallet::ldk::generate_mnemonic();
        let text = openagents_spark::seed::restore_entropy(&words)?;
        let entropy = (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(text.get(i..i + 2).unwrap_or("zz"), 16))
            .collect::<Result<Vec<u8>, _>>()
            .map_err(|_| "the new seed is unreadable".to_string())?;
        let seed = openagents_spark::seed::Seed::from_entropy(entropy)?;
        openagents_spark::computer::save_seed(&home, &seed, false)?;
        let node = openagents_spark::computer::open(&home)?;
        node.spark_address()
    })();
    match made {
        Ok(address) => {
            output.line(
                &json!({"spark_home": home.display().to_string(), "spark_address": address}),
                |v| {
                    format!(
                        "payout Spark wallet {}",
                        v["spark_address"].as_str().unwrap_or("")
                    )
                },
            );
            0
        }
        Err(why) => output.fail("pay", &why),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lightning_address_maps_to_its_well_known_url() {
        assert_eq!(
            lnurlp_url("Alice@GetAlby.com").unwrap(),
            "https://getalby.com/.well-known/lnurlp/alice"
        );
        assert!(lnurlp_url("alice").is_err());
        assert!(lnurlp_url("a/b@example.com").is_err());
        assert!(lnurlp_url("alice@localhost").is_err());
    }

    #[test]
    fn pay_request_terms_bound_the_amount_and_keep_the_callback_query() {
        let body = json!({"tag": "payRequest", "minSendable": 1000, "maxSendable": 5_000_000,
                          "metadata": "[[\"text/plain\",\"hi\"]]",
                          "callback": "https://example.com/cb?user=alice"});
        let (url, metadata) = pay_request_terms(&body, 2_000_000).unwrap();
        assert_eq!(url, "https://example.com/cb?user=alice&amount=2000000");
        assert_eq!(metadata, "[[\"text/plain\",\"hi\"]]");
        assert!(pay_request_terms(&body, 500).is_err());
        assert!(pay_request_terms(&body, 6_000_000).is_err());
        let mut plain = body.clone();
        plain["callback"] = json!("http://example.com/cb");
        assert!(pay_request_terms(&plain, 2_000_000).is_err());
        let mut withdraw = body;
        withdraw["tag"] = json!("withdrawRequest");
        assert!(pay_request_terms(&withdraw, 2_000_000).is_err());
    }

    #[test]
    fn an_invoice_must_match_network_amount_and_metadata() {
        let metadata = "[[\"text/plain\",\"hi\"]]";
        let hash: [u8; 32] = Sha256::digest(metadata.as_bytes()).into();
        use nostr::x402::test_invoice::{number, signed, tag, words};
        let mut fields = tag(1, &words(&[7; 32]));
        fields.extend(tag(16, &words(&[2; 32])));
        fields.extend(tag(23, &words(&hash)));
        fields.extend(tag(6, &number(600)));
        let bolt11 = signed("lnbc20000n", fields, false, false);
        let found = check_invoice(&bolt11, "bc", 2_000_000, metadata).unwrap();
        assert_eq!(found.amount_msat, 2_000_000);
        assert_eq!(found.payment_hash.len(), 64);
        assert!(check_invoice(&bolt11, "bc", 1_000_000, metadata).is_err());
        assert!(check_invoice(&bolt11, "tb", 2_000_000, metadata).is_err());
        assert!(check_invoice(&bolt11, "bc", 2_000_000, "other").is_err());
    }
}
