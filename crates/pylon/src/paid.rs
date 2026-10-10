//! Paid pylon jobs (P3 of `docs/compute/verse-compute.md`): the direct
//! per-job Lightning payment between a buyer and a pylon, under NIP-X402's
//! native binding `nostr:openagents:1`.
//!
//! The buyer buys one job before it sends it. It seals a NIP-X402
//! `request` record (a private kind 3188 artifact) to the pylon whose
//! input is the digest of the NIP-CJ request it will send. The pylon
//! answers with a `challenge` carrying an x402 `exact` Lightning invoice
//! bound to that request, the buyer pays under its own ceiling and seals a
//! `claim` with the preimage, and the pylon's [`Seller`] settles it
//! through the embedded x402 facilitator and its replay store
//! (`openagents_x402::native`). Only then does the pylon run the CJ job
//! whose plaintext matches the admitted purchase's input; its status
//! records follow the job to `completed` or `failed`. The buyer's `3201`
//! receipt carries the payment hash and the preimage.
//!
//! x402's `exact` Lightning method names `bitcoin` and `testnet` only, so
//! a priced pylon runs on one of those. Test networks first:
//! `TestLightning` (feature `fixture`) is an in-memory Lightning network
//! of worthless testnet sats that signs real BOLT11 invoices for both
//! sides in fixtures, and it refuses `bitcoin`. On `bitcoin` a pylon or a
//! buyer needs a real wallet ([`Wallet`]) and the owner's standing grant
//! ([`Grant`]), and every buyer payment stays under the grant's ceilings
//! ([`Granted`]).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use nostr::contracts::{digest_bytes, jcs, parse_strict};
use nostr::domain::Event;
use nostr::private_artifact;
use nostr::pylon::{Payment, sha256_hex};
use openagents_x402::FileReplayStore;
use openagents_x402::facilitator::Facilitator;
use openagents_x402::native::{
    self, NATIVE_ONLY, Offer, Phase, PurchaseStore, RECORD_SCHEMA, Record, RecordType, Signed,
    parse_record,
};
pub use openagents_x402::server::Receiver;
use secp256k1::XOnlyPublicKey;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::identity::Identity;

/// The settlement profile a priced pylon advertises and its receipts name.
pub const PROFILE: &str = "x402-exact";
/// The one operation a pylon sells: a text generation job.
pub const OPERATION: &str = "generate";
/// The schema of a purchase's input: the NIP-CJ request plaintext.
pub const INPUT_SCHEMA: &str = "openagents.pylon.cj-request.v1";
/// The schema of a completed purchase's output: the NIP-CJ result plaintext.
pub const OUTPUT_SCHEMA: &str = "openagents.pylon.cj-result.v1";
const DEFINITION_SCHEMA: &str = "openagents.capability-definition.v1";
/// How long both sides keep purchase records on the relay.
const RETAIN: u64 = 7 * 24 * 3_600;
/// How long a challenge's invoice stays payable.
const INVOICE_SECS: u32 = 300;
const SKEW: u64 = 60;

/// A Lightning network.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Network {
    Bitcoin,
    Testnet,
    Signet,
    Regtest,
}

impl Network {
    /// The receipt's `network` name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bitcoin => "bitcoin",
            Self::Testnet => "testnet",
            Self::Signet => "signet",
            Self::Regtest => "regtest",
        }
    }

    /// Parse a receipt's `network` name.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "bitcoin" => Self::Bitcoin,
            "testnet" => Self::Testnet,
            "signet" => Self::Signet,
            "regtest" => Self::Regtest,
            _ => return None,
        })
    }

    /// Worthless sats: everything but `bitcoin`. The world marks these
    /// amounts **TEST** and never sums them with mainnet.
    #[must_use]
    pub fn is_test(self) -> bool {
        self != Self::Bitcoin
    }

    /// The x402 network identifier; `None` for `signet` and `regtest`,
    /// which the `exact` Lightning method does not name.
    #[must_use]
    pub fn x402(self) -> Option<&'static str> {
        openagents_x402::network_id(self.as_str())
    }

    /// The network an x402 identifier names.
    #[must_use]
    pub fn from_x402(id: &str) -> Option<Self> {
        [Self::Bitcoin, Self::Testnet]
            .into_iter()
            .find(|n| n.x402() == Some(id))
    }
}

/// A pylon's posted price per job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Price {
    pub msat: u64,
    pub network: Network,
}

/// The owner's standing grant for mainnet: nothing pays or sells on
/// `bitcoin` without one (`INVARIANTS.md`, Agent spending), and every
/// payment stays under both ceilings. The owner writes it by hand to
/// [`Grant::FILE`] in the pylon home; no command writes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    pub per_payment_msat: u64,
    pub daily_msat: u64,
}

impl Grant {
    /// The grant's file in the pylon home.
    pub const FILE: &'static str = "grant.json";

    /// The owner's grant in `home`, if one is there.
    ///
    /// # Errors
    ///
    /// A grant file that does not parse, or whose ceilings are zero or out
    /// of order.
    pub fn load(home: &Path) -> Result<Option<Self>, String> {
        let text = match std::fs::read_to_string(home.join(Self::FILE)) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("{}: {e}", Self::FILE)),
        };
        let grant: Self =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", Self::FILE))?;
        if grant.per_payment_msat == 0 || grant.per_payment_msat > grant.daily_msat {
            return Err(format!(
                "{}: both ceilings must be positive and the per-payment one at most the daily one",
                Self::FILE
            ));
        }
        Ok(Some(grant))
    }
}

/// An invoice one side issued.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Invoice {
    pub bolt11: String,
    /// 64 lowercase hex digits.
    pub payment_hash: String,
    pub amount_msat: u64,
}

/// The buyer's side: pay an invoice and return its preimage.
pub trait Payer: Send + Sync {
    fn network(&self) -> Network;
    /// Pay `invoice` and return the 64-hex preimage.
    ///
    /// # Errors
    ///
    /// When the payment did not settle; nothing is owed then.
    fn pay(&self, invoice: &Invoice) -> Result<String, String>;
}

/// A real wallet behind `openagents pylon serve` and `ask`: the receiver a
/// priced pylon issues invoices from, and the payer a buyer pays with. The
/// `openagents` binary supplies one over its Lightning node and wallet
/// (`openagents x402`); the standalone `pylon` binary has none.
pub trait Wallet: Send + Sync {
    /// The receiver for a priced pylon on `network`.
    ///
    /// # Errors
    ///
    /// No wallet, or a wallet on another network.
    fn receiver(&self, network: Network) -> Result<Arc<dyn Receiver>, String>;
    /// A payer on `network` that pays at most `max_msat` a job.
    ///
    /// # Errors
    ///
    /// No wallet, or a wallet on another network.
    fn payer(&self, network: Network, max_msat: u64) -> Result<Arc<dyn Payer>, String>;
}

/// No wallet: priced serving and paid asking are refused.
pub struct NoWallet;

impl Wallet for NoWallet {
    fn receiver(&self, _: Network) -> Result<Arc<dyn Receiver>, String> {
        Err("this binary has no wallet; run `openagents pylon serve`".into())
    }
    fn payer(&self, _: Network, _: u64) -> Result<Arc<dyn Payer>, String> {
        Err("this binary has no wallet; run `openagents pylon ask`".into())
    }
}

/// A payer under the owner's grant. A test network passes through. On
/// `bitcoin` it refuses without a grant, refuses a payment over the
/// per-payment ceiling or past the day's, and journals each payment
/// before it is attempted, so a payment whose outcome is unknown still
/// counts against the day.
pub struct Granted {
    inner: Arc<dyn Payer>,
    grant: Option<Grant>,
    journal: PathBuf,
    lock: Mutex<()>,
}

impl Granted {
    /// `inner` under `grant`, journaling mainnet payments in `home`.
    #[must_use]
    pub fn new(inner: Arc<dyn Payer>, grant: Option<Grant>, home: &Path) -> Self {
        Self {
            inner,
            grant,
            journal: home.join("mainnet-payments.jsonl"),
            lock: Mutex::new(()),
        }
    }

    /// The msat journaled at or after `since`. A missing journal is none
    /// spent; an unreadable journal or a line that does not parse is an
    /// error, so the ceiling never fails open.
    fn spent_since(&self, since: u64) -> Result<u64, String> {
        let text = match std::fs::read_to_string(&self.journal) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(e) => return Err(format!("the payment journal: {e}; nothing was paid")),
        };
        let mut spent = 0u64;
        for (n, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let entry = serde_json::from_str::<Value>(line)
                .ok()
                .and_then(|v| Some((v["at"].as_u64()?, v["msat"].as_u64()?)))
                .ok_or_else(|| {
                    format!(
                        "the payment journal has an unreadable line {}; nothing was paid",
                        n + 1
                    )
                })?;
            if entry.0 >= since {
                spent = spent.saturating_add(entry.1);
            }
        }
        Ok(spent)
    }

    /// An exclusive lock on the journal's sibling `.lock` file, held from
    /// the read through the check to the append, so concurrent processes
    /// (two `pylon ask`s on one home) cannot both pass the ceiling.
    fn lock_journal(&self) -> Result<std::fs::File, String> {
        if let Some(dir) = self.journal.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("the payment journal: {e}; nothing was paid"))?;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(self.journal.with_extension("jsonl.lock"))
            .map_err(|e| format!("the payment journal lock: {e}; nothing was paid"))?;
        file.lock()
            .map_err(|e| format!("the payment journal lock: {e}; nothing was paid"))?;
        Ok(file)
    }
}

impl Payer for Granted {
    fn network(&self) -> Network {
        self.inner.network()
    }

    fn pay(&self, invoice: &Invoice) -> Result<String, String> {
        if self.inner.network() == Network::Bitcoin {
            let _held = self.lock.lock().unwrap_or_else(|e| e.into_inner());
            let grant = self
                .grant
                .ok_or("mainnet payments need the owner's standing grant; nothing was paid")?;
            if invoice.amount_msat > grant.per_payment_msat {
                return Err("over the owner's per-payment ceiling; nothing was paid".into());
            }
            // Released when dropped, after the append below.
            let _file_lock = self.lock_journal()?;
            let at = crate::now();
            if self
                .spent_since(at.saturating_sub(86_400))?
                .saturating_add(invoice.amount_msat)
                > grant.daily_msat
            {
                return Err("over the owner's daily ceiling; nothing was paid".into());
            }
            let line = json!({"at": at, "msat": invoice.amount_msat, "payment_hash": invoice.payment_hash});
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.journal)
                .and_then(|mut file| {
                    std::io::Write::write_all(&mut file, format!("{line}\n").as_bytes())
                })
                .map_err(|e| format!("the payment journal: {e}; nothing was paid"))?;
        }
        self.inner.pay(invoice)
    }
}

/// Check an invoice against the buyer's wallet and ceiling, pay it, and
/// return the receipt's `payment`.
///
/// # Errors
///
/// An invoice on another network, over the ceiling, or a payment that
/// failed or returned a preimage that does not hash to its payment hash.
pub fn pay(
    payer: &dyn Payer,
    invoice: &Invoice,
    network: Network,
    max_msat: u64,
) -> Result<Payment, String> {
    if network != payer.network() {
        return Err(format!(
            "the pylon asks for {} sats; this wallet is on {}",
            network.as_str(),
            payer.network().as_str()
        ));
    }
    if invoice.amount_msat > max_msat {
        return Err(format!(
            "the pylon asks {} msat, over this buyer's {max_msat} msat ceiling",
            invoice.amount_msat
        ));
    }
    let preimage = payer.pay(invoice)?;
    if !preimage_matches(&preimage, &invoice.payment_hash) {
        return Err("the wallet returned a preimage that does not match".into());
    }
    Ok(Payment {
        profile: PROFILE.into(),
        network: network.as_str().into(),
        amount_msat: invoice.amount_msat,
        payment_hash: invoice.payment_hash.clone(),
        preimage,
    })
}

/// Whether `preimage` (64 hex) hashes to `payment_hash`.
#[must_use]
pub fn preimage_matches(preimage: &str, payment_hash: &str) -> bool {
    hex32(preimage).is_some_and(|raw| sha256_hex(&raw) == payment_hash)
}

fn hex32(text: &str) -> Option<[u8; 32]> {
    if !is_hex64(text) {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(out)
}

#[cfg(feature = "fixture")]
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn is_hex64(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// The pylon's capability as a CAP DefinitionRef: the qualified ID its
/// beacon names, over a fixed definition of the one operation it sells.
#[must_use]
pub fn capability(provider: &str) -> Value {
    let id = crate::provider::Config::capability(provider);
    let definition = json!({
        "v": DEFINITION_SCHEMA,
        "id": id,
        "operation": OPERATION,
        "lane": "cj-conversation",
    });
    let bytes = jcs(&definition).unwrap_or_default();
    json!({
        "id": id,
        "artifact": {
            "digest": digest_bytes(&bytes),
            "size": bytes.len(),
            "media_type": "application/json",
            "schema": DEFINITION_SCHEMA,
        },
    })
}

/// The ArtifactRef of a CJ plaintext under `schema`: what a purchase's
/// input and output name.
#[must_use]
pub fn plaintext_ref(plaintext: &str, schema: &str) -> Value {
    json!({
        "digest": digest_bytes(plaintext.as_bytes()),
        "size": plaintext.len(),
        "media_type": "application/json",
        "schema": schema,
    })
}

/// Seal one NIP-X402 record to `to` as a private kind 3188 artifact whose
/// mailbox is the purchase nonce.
///
/// # Errors
///
/// A malformed recipient or record.
pub fn seal_record(
    me: &Identity,
    to: &str,
    purchase: &str,
    record: &Value,
    now: u64,
) -> Result<Event, String> {
    let bytes = jcs(record).map_err(|e| e.to_string())?;
    let body = json!({
        "v": "openagents.artifact-envelope.v1",
        "requires": [],
        "artifact": {
            "digest": digest_bytes(&bytes),
            "size": bytes.len(),
            "media_type": "application/json",
            "schema": RECORD_SCHEMA,
        },
        "inline": record,
        "issued_at": now,
        "retain_until": now + RETAIN,
    });
    let recipient = to
        .parse::<XOnlyPublicKey>()
        .map_err(|_| format!("{to} is not an x-only public key"))?;
    private_artifact::seal(
        &body,
        me.secret(),
        &recipient,
        purchase,
        now,
        secp256k1::rand::random(),
    )
    .map_err(|e| e.to_string())
}

/// Open a NIP-X402 record sealed to `me`: the parsed record and its
/// signed bytes with the event's provenance. `None` for anything else.
#[must_use]
pub fn open_record(me: &Identity, event: &Event) -> Option<(Record, Signed)> {
    let opened = private_artifact::open(event, me.secret()).ok()?;
    if opened.artifact().schema.as_deref() != Some(RECORD_SCHEMA) {
        return None;
    }
    let bytes = opened.inline_bytes()?.to_vec();
    let record = parse_record(&bytes, &event.pubkey).ok()?;
    let value = parse_strict(&bytes).ok()?;
    let signed = Signed::new(&value)
        .ok()?
        .with_event(&event.id, &event.pubkey);
    Some((record, signed))
}

/// The relay filter for NIP-X402 records sealed to `pubkey`.
#[must_use]
pub fn inbox(pubkey: &str, since: u64) -> Value {
    json!({
        "kinds": [nostr::contracts::ARTIFACT_ENVELOPE_KIND],
        "#p": [pubkey],
        "#t": [nostr::contracts::ARTIFACT_MARKER],
        "since": since,
    })
}

/// A priced pylon's side of NIP-X402: it answers requests with
/// challenges, settles claims through the embedded facilitator and its
/// replay store, and admits exactly one CJ job per settled purchase. The
/// purchase ledger and the replay store live under one directory, so a
/// restarted pylon neither settles a proof twice nor runs a paid job twice.
pub struct Seller {
    pubkey: String,
    price: Price,
    per_buyer_hourly: Option<u32>,
    receiver: Arc<dyn Receiver>,
    facilitator: Facilitator<FileReplayStore>,
    store: PurchaseStore,
    /// One purchase step at a time: each is a short ledger transition.
    lock: Mutex<()>,
}

impl Seller {
    /// A seller for the pylon `pubkey` at `price`, issuing invoices from
    /// `receiver`, with its ledger in `dir`.
    ///
    /// # Errors
    ///
    /// A network x402 does not name, or a ledger that cannot be opened.
    pub fn open(
        dir: &Path,
        pubkey: &str,
        price: Price,
        receiver: Arc<dyn Receiver>,
        per_buyer_hourly: Option<u32>,
    ) -> Result<Self, String> {
        if price.network.x402().is_none() {
            return Err(format!(
                "x402 exact Lightning names bitcoin and testnet only, not {}",
                price.network.as_str()
            ));
        }
        if price.msat == 0 {
            return Err("a priced pylon needs a positive price".into());
        }
        Ok(Self {
            pubkey: pubkey.into(),
            price,
            per_buyer_hourly,
            receiver,
            facilitator: Facilitator::with_profiles(
                FileReplayStore::open(&dir.join("replay")).map_err(|e| e.to_string())?,
                SKEW,
                NATIVE_ONLY,
            ),
            store: PurchaseStore::open(&dir.join("purchases")).map_err(|e| e.to_string())?,
            lock: Mutex::new(()),
        })
    }

    #[must_use]
    pub fn price(&self) -> Price {
        self.price
    }

    fn native(&self) -> native::Provider<'_, FileReplayStore> {
        native::Provider {
            pubkey: self.pubkey.clone(),
            offer: Offer {
                capability_id: crate::provider::Config::capability(&self.pubkey),
                operation: OPERATION.into(),
                network: self.price.network.x402().unwrap_or_default().into(),
                amount_msat: self.price.msat,
                timeout_secs: INVOICE_SECS,
                description: "one pylon text generation job".into(),
                per_buyer_hourly: self.per_buyer_hourly,
            },
            receiver: self.receiver.as_ref(),
            facilitator: &self.facilitator,
            store: &self.store,
            skew: SKEW,
        }
    }

    /// Answer one record a buyer sealed to this pylon: a request gets a
    /// challenge and the `offered` status (or a refusal), a claim is
    /// settled and admitted (or rejected), and a status query gets every
    /// status so far. Returns the records to seal back to the buyer.
    #[must_use]
    pub fn handle(&self, record: &Record, signed: &Signed, now: u64) -> Vec<Value> {
        let _held = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let provider = self.native();
        match record.kind {
            RecordType::Request => match provider.offer(signed, now) {
                Ok(emit) => emit.records,
                Err(refusal) => refusal.emit.records,
            },
            RecordType::Claim => match provider.claim(signed, now) {
                Ok(admitted) => admitted.emit.records,
                Err(refusal) => refusal.emit.records,
            },
            RecordType::StatusQuery => provider
                .statuses(signed)
                .map(|emit| emit.records)
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    /// Start the CJ job `request` (its event ID) from `buyer` whose
    /// plaintext is the input of one of the buyer's admitted purchases:
    /// the purchase moves to `running`. Returns the purchase nonce and
    /// the status to send.
    ///
    /// # Errors
    ///
    /// No admitted purchase has this input: the job was not paid for, or
    /// its purchase already ran.
    pub fn start(
        &self,
        buyer: &str,
        plaintext: &str,
        relay: &str,
        request: &str,
        now: u64,
    ) -> Result<(String, Vec<Value>), String> {
        let _held = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let digest = digest_bytes(plaintext.as_bytes());
        let purchases = self.store.list().map_err(|e| e.to_string())?;
        let purchase = purchases
            .into_iter()
            .filter(|p| p.buyer == buyer && p.phase().is_ok_and(|ph| ph == Phase::Admitted))
            .find(|p| {
                p.request
                    .value()
                    .ok()
                    .is_some_and(|v| v["body"]["input"]["digest"].as_str() == Some(digest.as_str()))
            })
            .ok_or("no paid purchase admits this job; buy it first under NIP-X402")?;
        let job = json!({"relay": relay, "worker": self.pubkey, "request": request});
        let emit = self
            .native()
            .start_job(buyer, &purchase.purchase, Some(job), now)?;
        Ok((purchase.purchase, emit.records))
    }

    /// Finish a running purchase: `Ok` with the CJ result plaintext, or a
    /// stable failure cause. Returns the status to send.
    #[must_use]
    pub fn finish(
        &self,
        buyer: &str,
        purchase: &str,
        outcome: Result<&str, &'static str>,
        now: u64,
    ) -> Vec<Value> {
        let _held = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let outcome = outcome.map(|plain| plaintext_ref(plain, OUTPUT_SCHEMA));
        match self.native().finish(buyer, purchase, outcome, now) {
            Ok(emit) => emit.records,
            Err(e) => {
                eprintln!("pylon: finishing purchase {}: {e}", &purchase[..12]);
                Vec::new()
            }
        }
    }
}

#[cfg(feature = "fixture")]
pub use test_lightning::TestLightning;

#[cfg(feature = "fixture")]
mod test_lightning {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use nostr::pylon::sha256_hex;
    use nostr::x402::test_invoice::{number, payee_of, signed_by, tag, words};

    use super::{Invoice, Network, Payer, Receiver, hex, hex32};

    struct Pending {
        preimage: String,
        amount_msat: u64,
        paid: bool,
    }

    /// An in-memory Lightning network of worthless testnet sats for
    /// fixtures: it issues real BOLT11 invoices signed by a test node key
    /// (an x402 [`Receiver`]) and pays them (the buyer's [`Payer`]), so a
    /// fixture's proofs pass the x402 facilitator and its receipts carry
    /// real preimages. It never touches a wallet, and it runs on `testnet`
    /// only: it refuses `bitcoin`, and x402 names no other network.
    pub struct TestLightning {
        network: Network,
        node: [u8; 32],
        invoices: Mutex<HashMap<String, Pending>>,
        /// The payment hash, by bolt11.
        by_bolt11: Mutex<HashMap<String, String>>,
    }

    impl TestLightning {
        /// A test network.
        ///
        /// # Errors
        ///
        /// For anything but `testnet`.
        pub fn new(network: Network) -> Result<Self, String> {
            if !network.is_test() {
                return Err("TestLightning never runs on bitcoin".into());
            }
            if network != Network::Testnet {
                return Err("x402 exact Lightning names testnet as its only test network".into());
            }
            let mut node: [u8; 32] = secp256k1::rand::random();
            node[0] = node[0].clamp(1, 0x7f);
            Ok(Self {
                network,
                node,
                invoices: Mutex::new(HashMap::new()),
                by_bolt11: Mutex::new(HashMap::new()),
            })
        }

        /// How many invoices were paid, and their total.
        #[must_use]
        pub fn paid(&self) -> (usize, u64) {
            let invoices = self.invoices.lock().unwrap_or_else(|e| e.into_inner());
            invoices
                .values()
                .filter(|p| p.paid)
                .fold((0, 0), |(n, sum), p| (n + 1, sum + p.amount_msat))
        }

        /// Whether the invoice with this hash has been paid.
        #[must_use]
        pub fn settled(&self, payment_hash: &str) -> bool {
            self.invoices
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(payment_hash)
                .is_some_and(|p| p.paid)
        }

        /// The invoice behind `bolt11`, as the network knows it.
        #[must_use]
        pub fn lookup(&self, bolt11: &str) -> Option<Invoice> {
            let hash = self
                .by_bolt11
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(bolt11)
                .cloned()?;
            let amount_msat = self
                .invoices
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(&hash)?
                .amount_msat;
            Some(Invoice {
                bolt11: bolt11.into(),
                payment_hash: hash,
                amount_msat,
            })
        }
    }

    impl Receiver for TestLightning {
        fn pay_to(&self) -> String {
            hex(&payee_of(self.node))
        }

        fn invoice(
            &self,
            amount_msat: u64,
            request_hash: [u8; 32],
            expiry_secs: u32,
        ) -> Result<String, String> {
            if amount_msat == 0 {
                return Err("a zero invoice".into());
            }
            let preimage: [u8; 32] = secp256k1::rand::random();
            let secret: [u8; 32] = secp256k1::rand::random();
            let payment_hash = sha256_hex(&preimage);
            let hash = hex32(&payment_hash).ok_or("hash")?;
            let mut fields = tag(1, &words(&hash));
            fields.extend(tag(16, &words(&secret)));
            fields.extend(tag(23, &words(&request_hash)));
            fields.extend(tag(6, &number(u64::from(expiry_secs))));
            let bolt11 = signed_by(
                self.node,
                &format!("lntb{}p", u128::from(amount_msat) * 10),
                fields,
                false,
                false,
                crate::now(),
            );
            self.invoices
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(
                    payment_hash.clone(),
                    Pending {
                        preimage: hex(&preimage),
                        amount_msat,
                        paid: false,
                    },
                );
            self.by_bolt11
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(bolt11.clone(), payment_hash);
            Ok(bolt11)
        }

        fn received_msat(&self, payment_hash: [u8; 32]) -> Result<Option<u64>, String> {
            let invoices = self.invoices.lock().unwrap_or_else(|e| e.into_inner());
            Ok(invoices
                .get(&hex(&payment_hash))
                .filter(|p| p.paid)
                .map(|p| p.amount_msat))
        }
    }

    impl Payer for TestLightning {
        fn network(&self) -> Network {
            self.network
        }

        fn pay(&self, invoice: &Invoice) -> Result<String, String> {
            let hash = self
                .by_bolt11
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(&invoice.bolt11)
                .cloned()
                .ok_or("no route: the invoice is not on this test network")?;
            let mut invoices = self.invoices.lock().unwrap_or_else(|e| e.into_inner());
            let pending = invoices.get_mut(&hash).ok_or("unknown invoice")?;
            if pending.amount_msat != invoice.amount_msat || hash != invoice.payment_hash {
                return Err("the invoice's terms differ from the network's".into());
            }
            pending.paid = true;
            Ok(pending.preimage.clone())
        }
    }
}

#[cfg(all(test, feature = "fixture"))]
mod tests {
    use super::*;

    #[test]
    fn test_lightning_signs_x402_invoices_pays_them_and_refuses_mainnet() {
        assert!(TestLightning::new(Network::Bitcoin).is_err());
        assert!(TestLightning::new(Network::Regtest).is_err());
        let net = TestLightning::new(Network::Testnet).unwrap();
        let bolt11 = Receiver::invoice(&net, 2_000, [3; 32], 300).unwrap();
        let decoded = nostr::x402::decode_invoice(&bolt11).unwrap();
        assert_eq!(decoded.amount_msat(), 2_000);
        assert_eq!(hex(&decoded.payee()), net.pay_to());
        let invoice = net.lookup(&bolt11).unwrap();
        assert!(pay(&net, &invoice, Network::Testnet, 1_999).is_err());
        assert!(pay(&net, &invoice, Network::Bitcoin, 5_000).is_err());
        let payment = pay(&net, &invoice, Network::Testnet, 2_000).unwrap();
        assert!(net.settled(&invoice.payment_hash));
        assert!(preimage_matches(&payment.preimage, &payment.payment_hash));
        assert_eq!(net.paid(), (1, 2_000));
        assert!(!preimage_matches(&"0".repeat(64), &payment.payment_hash));
    }

    struct Mainnet(Mutex<u32>);
    impl Payer for Mainnet {
        fn network(&self) -> Network {
            Network::Bitcoin
        }
        fn pay(&self, _: &Invoice) -> Result<String, String> {
            *self.0.lock().unwrap() += 1;
            Ok("00".repeat(32))
        }
    }

    #[test]
    fn a_mainnet_payment_needs_the_grant_and_stays_under_its_ceilings() {
        let home = tempfile::tempdir().unwrap();
        let invoice = |msat| Invoice {
            bolt11: "lnbc".into(),
            payment_hash: "11".repeat(32),
            amount_msat: msat,
        };
        let inner = Arc::new(Mainnet(Mutex::new(0)));
        let none = Granted::new(inner.clone(), None, home.path());
        assert!(none.pay(&invoice(1_000)).unwrap_err().contains("grant"));
        let grant = Grant {
            per_payment_msat: 5_000,
            daily_msat: 8_000,
        };
        let granted = Granted::new(inner.clone(), Some(grant), home.path());
        assert!(
            granted
                .pay(&invoice(6_000))
                .unwrap_err()
                .contains("per-payment")
        );
        granted.pay(&invoice(5_000)).unwrap();
        assert!(granted.pay(&invoice(4_000)).unwrap_err().contains("daily"));
        granted.pay(&invoice(3_000)).unwrap();
        assert_eq!(*inner.0.lock().unwrap(), 2);
        // The grant file: missing is none; zero or inverted ceilings refuse.
        assert_eq!(Grant::load(home.path()).unwrap(), None);
        std::fs::write(
            home.path().join(Grant::FILE),
            r#"{"per_payment_msat": 9000, "daily_msat": 1000}"#,
        )
        .unwrap();
        assert!(Grant::load(home.path()).is_err());
        std::fs::write(
            home.path().join(Grant::FILE),
            r#"{"per_payment_msat": 1000, "daily_msat": 9000}"#,
        )
        .unwrap();
        assert_eq!(
            Grant::load(home.path()).unwrap(),
            Some(Grant {
                per_payment_msat: 1_000,
                daily_msat: 9_000
            })
        );
    }

    fn mainnet_invoice(msat: u64) -> Invoice {
        Invoice {
            bolt11: "lnbc".into(),
            payment_hash: "11".repeat(32),
            amount_msat: msat,
        }
    }

    const GRANT: Grant = Grant {
        per_payment_msat: 5_000,
        daily_msat: 8_000,
    };

    #[test]
    fn a_corrupt_mainnet_journal_refuses_instead_of_counting_zero() {
        let home = tempfile::tempdir().unwrap();
        let inner = Arc::new(Mainnet(Mutex::new(0)));
        let granted = Granted::new(inner.clone(), Some(GRANT), home.path());
        std::fs::write(home.path().join("mainnet-payments.jsonl"), "not json\n").unwrap();
        assert!(
            granted
                .pay(&mainnet_invoice(1_000))
                .unwrap_err()
                .contains("unreadable line 1")
        );
        // A line that parses but lacks its amount refuses too.
        std::fs::write(
            home.path().join("mainnet-payments.jsonl"),
            format!("{{\"at\": {}}}\n", crate::now()),
        )
        .unwrap();
        assert!(granted.pay(&mainnet_invoice(1_000)).is_err());
        // A journal that is not a readable file refuses.
        std::fs::remove_file(home.path().join("mainnet-payments.jsonl")).unwrap();
        std::fs::create_dir(home.path().join("mainnet-payments.jsonl")).unwrap();
        assert!(granted.pay(&mainnet_invoice(1_000)).is_err());
        assert_eq!(*inner.0.lock().unwrap(), 0);
    }

    #[test]
    fn separate_payers_on_one_home_share_the_daily_ceiling() {
        let home = tempfile::tempdir().unwrap();
        let inner = Arc::new(Mainnet(Mutex::new(0)));
        // As separate `pylon ask` processes would: each its own Granted.
        let payers: Vec<_> = (0..8)
            .map(|_| Arc::new(Granted::new(inner.clone(), Some(GRANT), home.path())))
            .collect();
        let threads: Vec<_> = payers
            .iter()
            .cloned()
            .map(|p| std::thread::spawn(move || p.pay(&mainnet_invoice(3_000)).is_ok()))
            .collect();
        let paid = threads
            .into_iter()
            .map(|t| t.join().unwrap())
            .filter(|ok| *ok)
            .count();
        // 8_000 / 3_000: two fit, a third would cross the ceiling.
        assert_eq!(paid, 2);
        assert_eq!(*inner.0.lock().unwrap(), 2);
    }
}
