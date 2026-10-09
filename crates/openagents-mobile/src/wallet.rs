//! The Wallet tab: a Bitcoin wallet on mainnet through Breez's Spark SDK
//! (`crate::spark`). Rust decides every state and line of text; the host lays
//! them out, collects typed values, and keeps the seed in its key store.
//!
//! The network is fixed in this file. No request, configuration, or stored
//! value switches the phone's wallet to another network, and it never opens
//! the x402 receiver's Lightning node on a computer (`~/.openagents/wallet`) or
//! any treasury wallet. The same wallet runs on computers the owner links
//! (`crates/spark-wallet`, `openagents wallet link`). The
//! seed arrives with `wallet_open` as BIP39 entropy (16 or 32 bytes); it and
//! its mnemonic stay in memory, and neither is written, logged, or put in the
//! app packet or an error; the entropy leaves the phone only sealed to a
//! computer the owner approves (`crate::wallet_link`). The recovery words leave Rust only in the direct
//! reply to [`crate::Request::WalletWords`], which the host sends after the
//! person confirms a warning.

use crate::amounts::Format;
use breez_sdk_spark::Network;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

/// The only network the phone's wallet runs on.
pub const NETWORK: Network = Network::Mainnet;
/// What the screen calls the network.
pub const NETWORK_LABEL: &str = "Bitcoin · Spark";
/// The description a new Lightning invoice carries.
const INVOICE_DESCRIPTION: &str = "OpenAgents";
/// Above this balance the screen reminds the person that it's a phone wallet.
pub const BALANCE_WARNING_SATS: u64 = 1_000_000;
/// How many payments the history shows.
const HISTORY_LIMIT: u32 = 50;
/// How many payments the main screen lists under Recent activity; the rest
/// are behind See all.
pub const RECENT_LIMIT: usize = 5;
/// A balance read longer ago than this says when it was read.
pub const STALE_SECS: u64 = 15 * 60;

/// Files under the wallet home. None holds a secret: the last balance read,
/// the receive addresses, recent payments, and whether the trust note was
/// acknowledged, so the screen shows at once while the wallet starts.
const BALANCE_FILE: &str = "last-balance";
const ADDRESSES_FILE: &str = "addresses.json";
const PAYMENTS_FILE: &str = "payments.json";
const TRUST_FILE: &str = "trust-acknowledged";
/// The Spark address this phone last published in its Nostr payment
/// targets, when the person turned publishing on.
const PUBLISHED_FILE: &str = "published-spark";
/// People paid by npub, newest first.
const PEOPLE_FILE: &str = "people.json";
/// The person confirmed they wrote down the recovery words: a fingerprint of
/// the seed they were shown (a hash, never the seed), so another wallet on
/// this phone asks again.
const WORDS_SAVED_FILE: &str = "words-saved";
/// The Advanced section is open, remembered on this phone.
const ADVANCED_FILE: &str = "advanced-open";
/// How many people paid by npub are kept.
const PEOPLE_LIMIT: usize = 20;

/// The trust note: what Spark is and who the person relies on.
pub const TRUST_TITLE: &str = "About this wallet";
/// The trust note in one plain paragraph, shown first.
pub const TRUST_SUMMARY: &str = "Your bitcoin is kept by this phone and any computer you link to it, and only your recovery words can bring it back. Payments are instant because a few companies help move them; if they ever stop, you can still take your bitcoin out yourself, slowly. Keep only what you'd carry in your pocket.";
pub const TRUST_LINES: [&str; 5] = [
    "This wallet runs on Spark, not on a Lightning node of your own. Your keys stay on this phone and any computer you link to it.",
    "Three companies run Spark's operators: Lightspark, Breez, and Flashnet. Two of them must cooperate for payments off the chain, and your safety depends on at least one of them having deleted old keys, which no one can check.",
    "If the operators stop, you can still withdraw on the Bitcoin chain yourself, but it can take days and needs a separate on-chain payment for fees.",
    "Lightning payments go through Lightspark.",
    "Keep amounts you'd be comfortable carrying in a phone wallet, and write down your recovery words.",
];

// The wallet's values and the `Node` trait are shared with `openagents
// wallet` on computers (`crates/spark-wallet`).
pub use openagents_spark::model::*;
use openagents_spark::seed::Seed;
pub use openagents_spark::seed::restore_entropy;

/// Someone paid by npub, kept so the Send screen can offer them again.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct PaidPerson {
    npub: String,
    name: Option<String>,
}

/// About how large a deposit refund is: one Taproot input and one output.
pub const REFUND_VBYTES: u64 = 111;

/// Keeps the unilateral-exit state outside the SDK's store, encrypted on
/// this phone ([`crate::app`] keys it with the device key).
pub trait Vault: Send + Sync {
    fn save(&self, saved: &SavedExit) -> Result<(), String>;
    fn load(&self) -> Option<SavedExit>;
    fn clear(&self);
}

/// The last unilateral-exit state saved, and when.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedExit {
    pub state: String,
    pub saved_at: u64,
}

/// Opens the wallet under a home with a mnemonic. Blocking.
pub type Opener = Arc<dyn Fn(&Path, &str) -> Result<Arc<dyn Node>, String> + Send + Sync>;

/// The real opener: Breez's SDK on mainnet.
pub fn spark_opener() -> Opener {
    Arc::new(|home, mnemonic| {
        crate::spark::open(home, NETWORK, mnemonic).map(|node| Arc::new(node) as Arc<dyn Node>)
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Sending {
    Idle,
    Quoting,
    NeedsAmount(Ask),
    Quoted(Quote),
    Paying(Quote),
    Sent(Paid),
    Failed(String),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Addresses {
    spark: Option<String>,
    bitcoin: Option<String>,
}

/// A deposit refund the person started: fee rates, then a reviewed
/// address and speed, then its outcome.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Refund {
    txid: String,
    vout: u32,
    rates: Option<FeeRates>,
    review: Option<RefundReview>,
    busy: bool,
    message: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RefundReview {
    address: String,
    speed: Speed,
    rate: u64,
    fee_sats: u64,
    amount_sats: u64,
}

/// A deposit claim the person started: its quote, then its outcome.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Claim {
    txid: String,
    vout: u32,
    quote: Option<ClaimQuote>,
    busy: bool,
    message: Option<String>,
}

struct Shared {
    node: Option<Arc<dyn Node>>,
    /// Bumped when the wallet is replaced, so a start or read that began
    /// for the old one changes nothing.
    generation: u64,
    starting: bool,
    refreshing: bool,
    /// The last start or sync failure, cleared by the next success.
    error: Option<String>,
    /// A sync finished since the node started.
    synced: bool,
    /// The last balance read, saved across launches.
    last: Option<LastBalance>,
    addresses: Addresses,
    payments: Vec<PaymentRow>,
    invoice: Option<(String, Option<u64>)>,
    invoice_busy: bool,
    invoice_error: Option<String>,
    send: Sending,
    trust_acknowledged: bool,
    deposits: Vec<DepositRow>,
    claim: Option<Claim>,
    refund: Option<Refund>,
    /// When the exit state was last saved, and why the last save failed.
    exit_saved_at: Option<u64>,
    exit_error: Option<String>,
    vault: Option<Arc<dyn Vault>>,
    /// Reads Nostr profiles and publishes payment targets.
    directory: Option<Arc<dyn crate::payees::Directory>>,
    /// This device's npub, which others pay through what it publishes.
    npub: Option<String>,
    /// The person an npub in the Send field resolved to.
    person: Option<crate::payees::Resolved>,
    /// The Spark address published in this device's payment targets.
    published: Option<String>,
    publish_busy: bool,
    publish_message: Option<String>,
    contact_busy: bool,
    contacts: Vec<Contact>,
    people: Vec<PaidPerson>,
    /// A Lightning address just paid that isn't a contact yet.
    save_suggestion: Option<String>,
    buy_busy: bool,
    buy_error: Option<String>,
    /// A purchase page for the host to open once.
    open_url: Option<String>,
    /// How amounts are shown and read; the app's choice.
    format: Format,
    /// The Advanced section is open.
    advanced_open: bool,
}

/// A balance read by an earlier sync, shown until this launch reads again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LastBalance {
    total: u64,
    synced_at: Option<u64>,
}

impl LastBalance {
    fn read(home: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(home.join(BALANCE_FILE)).ok()?;
        let mut fields = text.split_whitespace();
        let total: u64 = fields.next()?.parse().ok()?;
        let synced_at = fields.next().and_then(|value| value.parse().ok());
        Some(Self { total, synced_at })
    }

    fn write(self, home: &Path) {
        let synced_at = self.synced_at.map(|at| at.to_string()).unwrap_or_default();
        // A lost cache only means the next launch shows no balance until the
        // wallet reads Spark, so a failed write is not an error.
        let _ = std::fs::write(
            home.join(BALANCE_FILE),
            format!("{} {synced_at}\n", self.total),
        );
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(home: &Path, file: &str) -> Option<T> {
    serde_json::from_slice(&std::fs::read(home.join(file)).ok()?).ok()
}

fn write_json<T: Serialize>(home: &Path, file: &str, value: &T) {
    if let Ok(bytes) = serde_json::to_vec(value) {
        let _ = std::fs::write(home.join(file), bytes);
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

pub struct Wallet {
    home: PathBuf,
    opener: Opener,
    seed: Option<Seed>,
    /// The fingerprint of the seed whose words the person wrote down.
    words_saved: Option<String>,
    shared: Arc<Mutex<Shared>>,
}

/// The Wallet screen's state, drawn by the host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Screen {
    /// The wallet could not start. `retry` restarts it.
    Failed {
        message: String,
    },
    Ready(Box<Summary>),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Summary {
    pub network: &'static str,
    /// The balance in base units (machine-readable; the name is kept).
    pub balance_sats: u64,
    /// In the chosen format: "₿12,345" or "0.00012345 BTC".
    pub balance: String,
    /// In the other format, for the transitional dual display.
    pub balance_alternate: String,
    /// For a screen reader: "12,345 bitcoin" or "0.00012345 BTC".
    pub balance_spoken: String,
    /// No bitcoin yet: the screen explains how to receive some.
    pub empty: bool,
    /// When the last sync finished, in Unix seconds.
    pub synced_at: Option<u64>,
    pub refreshing: bool,
    /// A failed refresh; the balance shown is the last one read.
    pub error: Option<String>,
    /// No balance has been read on this phone yet: the balance fields are
    /// empty and the screen shows placeholders.
    pub balance_unknown: bool,
    /// What the wallet is doing while it starts or first reads Spark,
    /// shown beside a progress indicator; the rest of the screen stays.
    pub status: Option<String>,
    /// A reminder when the balance is large for a phone wallet.
    pub warning: Option<String>,
    pub trust: Trust,
    pub receive: Receive,
    pub send: SendView,
    pub payments: Vec<PaymentView>,
    /// The seed is in memory, so the recovery words can be shown.
    pub can_show_words: bool,
    pub buy: BuyView,
    /// On-chain deposits waiting to be claimed.
    pub deposits: Vec<DepositView>,
    /// A claim the person started.
    pub claim: Option<ClaimView>,
    /// A refund the person started.
    pub refund: Option<RefundView>,
    /// The unilateral-exit backup.
    pub backup: BackupView,
    /// Contacts and people paid by npub, for the Send screen.
    pub people: Vec<PersonView>,
    /// The balance was read long ago (or never): the main screen says when,
    /// quietly. A fresh balance shows no time.
    pub stale: bool,
    /// The newest payments for the main screen's Recent activity, at most
    /// [`RECENT_LIMIT`]; `payments` has them all for See all.
    pub recent: Vec<PaymentView>,
    /// There are more payments than `recent` shows.
    pub more_payments: bool,
    /// Shown on the main screen only until the person has written down this
    /// wallet's recovery words.
    pub backup_card: Option<BackupCard>,
    /// The Advanced section at the bottom of the main screen.
    pub advanced: AdvancedView,
}

/// The one card that asks the person to back up the wallet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct BackupCard {
    pub title: &'static str,
    pub detail: &'static str,
    /// The button that shows the words, after the warning.
    pub action: &'static str,
}

/// Everything that is not Receive, Send, recent activity, or the backup
/// card: other ways to receive, buying, deposits, recovery, the exit
/// backup, people, agent payments, and the amount setting.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AdvancedView {
    /// Open on this phone; closed by default.
    pub open: bool,
    /// Something inside wants attention while the section is closed:
    /// "1 deposit waiting".
    pub note: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RefundView {
    pub txid: String,
    pub vout: u32,
    pub busy: bool,
    /// The speeds to choose from, once the fee rates are read.
    pub speeds: Vec<SpeedView>,
    /// "Refund 48,890 sats to bc1q…; about 110 sats fee at 1 sat/vB."
    pub review: Option<String>,
    pub message: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SpeedView {
    /// Send this back as `speed`.
    pub id: &'static str,
    pub label: &'static str,
    /// "1,200 sats", or "about 1,110 sats at 10 sat/vB" for a refund.
    pub fee: String,
    pub chosen: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct BackupView {
    pub title: &'static str,
    pub detail: &'static str,
    /// When the exit state was last saved on this phone.
    pub saved_at: Option<u64>,
    /// There is a saved state to export.
    pub can_export: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct BuyView {
    pub busy: bool,
    pub error: Option<String>,
    pub providers: Vec<ProviderView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ProviderView {
    /// Send this back with `wallet_buy`.
    pub id: &'static str,
    pub label: &'static str,
    pub detail: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DepositView {
    pub txid: String,
    pub vout: u32,
    pub amount: String,
    /// Where the deposit stands, in words.
    pub status: String,
    /// It can still be claimed or refunded; false once a refund is out.
    pub actionable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ClaimView {
    pub txid: String,
    pub vout: u32,
    pub busy: bool,
    /// "Claim now for a ₿1,200 fee; ₿48,800 reaches your balance."
    pub quote: Option<String>,
    pub message: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Trust {
    /// The person has read the note; the screen shows a link to it instead.
    pub acknowledged: bool,
    pub title: &'static str,
    /// The note in one plain paragraph, shown before the details.
    pub summary: &'static str,
    pub lines: Vec<&'static str>,
}

/// One way to receive: the text to share, the URI its QR code carries, and
/// the code.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Code {
    pub text: String,
    pub uri: String,
    pub qr: Option<crate::app::QrModules>,
    /// "Request for ₿1,000", or what the code is for.
    pub caption: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Receive {
    pub lightning: Option<Code>,
    /// A new invoice is being made.
    pub lightning_busy: bool,
    pub lightning_error: Option<String>,
    pub spark: Option<Code>,
    pub bitcoin: Option<Code>,
    /// This device's npub, which other OpenAgents users can pay once its
    /// Spark address is published.
    pub nostr: Option<Code>,
    pub publish: Option<PublishView>,
}

/// The setting that publishes the Spark address in this device's Nostr
/// profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PublishView {
    /// The Spark address is published.
    pub on: bool,
    pub busy: bool,
    pub detail: &'static str,
    pub message: Option<String>,
}

/// Someone to pay: a saved contact or a person paid by npub before.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PersonView {
    pub name: String,
    /// A Lightning address or a short npub.
    pub detail: String,
    /// What to put in the Send field.
    pub input: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SendView {
    /// `idle`, `quoting`, `needs_amount`, `quoted`, `paying`, `sent`, or
    /// `failed`.
    pub state: &'static str,
    pub message: Option<String>,
    pub quote: Option<QuoteView>,
    pub result: Option<PaymentView>,
    /// While an amount is needed: who is paid, as the request names them.
    pub recipient: Option<String>,
    /// While an amount is needed: the recipient's description.
    pub description: Option<String>,
    /// While an amount is needed: the longest comment the recipient takes.
    /// The screen shows a comment field only when this is set.
    pub comment_max: Option<u16>,
    /// After a payment: what the recipient said, as plain text.
    pub recipient_message: Option<String>,
    /// The person an npub resolved to: "Alice (npub1abc…wxyz)".
    pub person: Option<String>,
    /// Where their address came from, naming it.
    pub person_source: Option<String>,
    /// After paying a Lightning address that isn't a contact: offer to
    /// save it.
    pub save_suggestion: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct QuoteView {
    /// Send this back with `wallet_pay`.
    pub id: u64,
    /// "Lightning invoice", "Lightning address", "Spark address", or
    /// "Bitcoin address", for Advanced detail.
    pub kind: &'static str,
    /// What is paid, in plain words for the confirm screen: "Payment
    /// request", "Address", "Wallet address", or "Bitcoin address".
    pub to: &'static str,
    /// The destination, shortened for the screen.
    pub destination: String,
    pub amount: String,
    pub fee: String,
    pub total: String,
    pub note: Option<String>,
    /// The comment sent to the recipient.
    pub comment: Option<String>,
    /// For an on-chain withdrawal, the speeds and their fees.
    pub speeds: Vec<SpeedView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PaymentView {
    pub id: String,
    /// "Received" or "Sent".
    pub title: &'static str,
    /// "+₿1,000" or "-₿1,000" (or legacy BTC).
    pub amount: String,
    pub fee: Option<String>,
    pub method: String,
    /// `completed`, `pending`, or `failed`.
    pub status: String,
    pub at: u64,
}

impl Wallet {
    pub fn new(home: PathBuf, opener: Opener) -> Self {
        // What earlier launches read shows before the wallet starts.
        let shared = Shared {
            node: None,
            generation: 0,
            starting: false,
            refreshing: false,
            error: None,
            synced: false,
            last: LastBalance::read(&home),
            addresses: read_json(&home, ADDRESSES_FILE).unwrap_or_default(),
            payments: read_json(&home, PAYMENTS_FILE).unwrap_or_default(),
            invoice: None,
            invoice_busy: false,
            invoice_error: None,
            send: Sending::Idle,
            trust_acknowledged: home.join(TRUST_FILE).exists(),
            deposits: vec![],
            claim: None,
            refund: None,
            exit_saved_at: None,
            exit_error: None,
            vault: None,
            directory: None,
            npub: None,
            person: None,
            published: std::fs::read_to_string(home.join(PUBLISHED_FILE))
                .ok()
                .map(|text| text.trim().to_owned())
                .filter(|text| !text.is_empty()),
            publish_busy: false,
            publish_message: None,
            contact_busy: false,
            contacts: vec![],
            people: read_json(&home, PEOPLE_FILE).unwrap_or_default(),
            save_suggestion: None,
            buy_busy: false,
            buy_error: None,
            open_url: None,
            format: Format::default(),
            advanced_open: home.join(ADVANCED_FILE).exists(),
        };
        let words_saved = std::fs::read_to_string(home.join(WORDS_SAVED_FILE))
            .ok()
            .map(|text| text.trim().to_owned());
        Self {
            home,
            opener,
            seed: None,
            words_saved,
            shared: Arc::new(Mutex::new(shared)),
        }
    }

    /// Resolve npubs and publish payment targets through `directory`, as
    /// the device key whose npub is `npub`.
    pub fn with_directory(
        self,
        directory: Arc<dyn crate::payees::Directory>,
        npub: String,
    ) -> Self {
        {
            let mut shared = self.lock();
            shared.directory = Some(directory);
            shared.npub = Some(npub);
        }
        self
    }

    /// Keep the unilateral-exit state in `vault` after each sync.
    pub fn with_vault(self, vault: Arc<dyn Vault>) -> Self {
        {
            let mut shared = self.lock();
            shared.exit_saved_at = vault.load().map(|saved| saved.saved_at);
            shared.vault = Some(vault);
        }
        self
    }

    fn lock(&self) -> MutexGuard<'_, Shared> {
        lock(&self.shared)
    }

    /// The running wallet, for an agent payment the owner approved.
    pub fn node(&self) -> Option<Arc<dyn Node>> {
        self.lock().node.clone()
    }

    /// Show and read amounts in `format` from now on.
    pub fn set_format(&mut self, format: Format) {
        self.lock().format = format;
    }

    /// Take the wallet key and start the wallet. A later call with the same
    /// key only retries a failed start. Another key is refused unless
    /// `replace` is set, which the host sends after a restore: that stops the
    /// running wallet and forgets what the screen cached for it.
    pub fn open(&mut self, entropy_hex: &str, replace: bool) {
        let seed = match decode(entropy_hex).and_then(Seed::from_entropy) {
            Ok(seed) => seed,
            Err(message) => {
                self.lock().error = Some(message);
                return;
            }
        };
        match &self.seed {
            Some(held) if held.entropy == seed.entropy => {}
            Some(_) if !replace => {
                self.lock().error = Some("This wallet already has a different key.".into());
                return;
            }
            Some(_) => {
                self.forget();
                self.seed = Some(seed);
            }
            None => {
                if replace {
                    self.forget();
                }
                self.seed = Some(seed);
            }
        }
        // A restore was typed from the recovery words, so the person has
        // them.
        if replace {
            self.words_saved();
        }
        self.start();
    }

    /// The person confirmed they wrote down the recovery words of the
    /// running wallet; the Back up card goes away.
    pub fn words_saved(&mut self) {
        let Some(seed) = &self.seed else {
            return;
        };
        let fingerprint = seed.fingerprint();
        let _ = std::fs::create_dir_all(&self.home);
        let _ = std::fs::write(self.home.join(WORDS_SAVED_FILE), format!("{fingerprint}\n"));
        self.words_saved = Some(fingerprint);
    }

    /// Open or close the Advanced section; this phone remembers it.
    pub fn set_advanced(&mut self, open: bool) {
        let file = self.home.join(ADVANCED_FILE);
        if open {
            let _ = std::fs::create_dir_all(&self.home);
            let _ = std::fs::write(file, b"1\n");
        } else {
            let _ = std::fs::remove_file(file);
        }
        self.lock().advanced_open = open;
    }

    /// Stop the running wallet and clear what the screen shows of it.
    fn forget(&mut self) {
        let old = {
            let mut shared = self.lock();
            shared.generation += 1;
            shared.starting = false;
            shared.refreshing = false;
            shared.error = None;
            shared.synced = false;
            shared.last = None;
            shared.addresses = Addresses::default();
            shared.payments.clear();
            shared.invoice = None;
            shared.invoice_busy = false;
            shared.invoice_error = None;
            shared.send = Sending::Idle;
            shared.deposits.clear();
            shared.claim = None;
            shared.refund = None;
            shared.exit_saved_at = None;
            shared.exit_error = None;
            shared.person = None;
            shared.contacts.clear();
            shared.people.clear();
            shared.save_suggestion = None;
            shared.publish_message = None;
            shared.buy_busy = false;
            shared.buy_error = None;
            shared.open_url = None;
            shared.node.take()
        };
        // A published address now names the old wallet; it stays published
        // until the person turns publishing off or on again, and the screen
        // says so.
        if self.lock().published.is_some() {
            self.lock().publish_message = Some(
                "Your Nostr profile still names the previous wallet's Spark address. Publish again to update it."
                    .into(),
            );
        }
        for file in [BALANCE_FILE, ADDRESSES_FILE, PAYMENTS_FILE, PEOPLE_FILE] {
            let _ = std::fs::remove_file(self.home.join(file));
        }
        // The old wallet's exit state is no use without its words.
        if let Some(vault) = self.lock().vault.clone() {
            vault.clear();
        }
        // Disconnecting reaches the network; do it off the app thread.
        if let Some(old) = old {
            std::thread::spawn(move || drop(old));
        }
    }

    /// The recovery words, for the direct reply to an explicit request.
    /// The wallet seed sealed to a computer's one-time link key
    /// (`openagents wallet link`), or `None` while this phone has no
    /// wallet. The seed never leaves here otherwise.
    pub(crate) fn seal_for(
        &self,
        key: &str,
    ) -> Option<Result<openagents_spark::link::Sealed, String>> {
        self.seed
            .as_ref()
            .map(|seed| openagents_spark::link::seal(seed, key))
    }

    pub fn words(&self) -> Option<Vec<String>> {
        self.seed.as_ref().map(|seed| {
            seed.mnemonic
                .split_whitespace()
                .map(str::to_owned)
                .collect()
        })
    }

    /// Start the wallet in the background, unless it runs or is starting.
    fn start(&mut self) {
        let Some(seed) = &self.seed else {
            return;
        };
        let generation = {
            let mut shared = self.lock();
            if shared.node.is_some() || shared.starting {
                return;
            }
            shared.starting = true;
            shared.error = None;
            shared.generation
        };
        let (home, opener, shared) = (self.home.clone(), self.opener.clone(), self.shared.clone());
        let mnemonic = seed.mnemonic.clone();
        std::thread::spawn(move || {
            let opened = std::fs::create_dir_all(&home)
                .map_err(|_| "The wallet's folder could not be created on this phone.".to_string())
                .and_then(|()| opener(&home, &mnemonic));
            drop(mnemonic);
            let node = match opened {
                Ok(node) => node,
                Err(message) => {
                    let mut state = lock(&shared);
                    if state.generation == generation {
                        state.starting = false;
                        state.error = Some(message);
                    }
                    return;
                }
            };
            let addresses = Addresses {
                spark: node.spark_address().ok(),
                bitcoin: node.bitcoin_address().ok(),
            };
            {
                let mut state = lock(&shared);
                if state.generation != generation {
                    return;
                }
                state.starting = false;
                state.node = Some(node.clone());
                state.refreshing = true;
                if addresses.spark.is_some() || addresses.bitcoin.is_some() {
                    write_json(&home, ADDRESSES_FILE, &addresses);
                    state.addresses = addresses;
                }
            }
            let (notify_shared, notify_home) = (shared.clone(), home.clone());
            node.subscribe(Arc::new(move || {
                let (shared, home) = (notify_shared.clone(), notify_home.clone());
                std::thread::spawn(move || read(&shared, &home, generation, false));
            }));
            read(&shared, &home, generation, true);
        });
    }

    /// Read Spark again: sync a running wallet, or restart one that failed
    /// to start.
    pub fn refresh(&mut self) {
        let generation = {
            let mut shared = self.lock();
            match shared.node.is_some() {
                true if !shared.refreshing => {
                    shared.refreshing = true;
                    shared.generation
                }
                true => return,
                false => {
                    drop(shared);
                    self.start();
                    return;
                }
            }
        };
        let (shared, home) = (self.shared.clone(), self.home.clone());
        std::thread::spawn(move || read(&shared, &home, generation, true));
    }

    /// Refresh a wallet that has its key; the app came to the foreground.
    pub fn refresh_if_open(&mut self) {
        if self.seed.is_some() {
            self.refresh();
        }
    }

    /// Make a Lightning invoice. `amount` is what the person typed; empty
    /// means any amount.
    pub fn invoice(&mut self, amount: &str) {
        let format = self.lock().format;
        let amount = match parse_amount(amount, format) {
            Ok(amount) => amount,
            Err(message) => {
                self.lock().invoice_error = Some(message);
                return;
            }
        };
        let (node, generation) = {
            let mut shared = self.lock();
            let Some(node) = shared.node.clone() else {
                shared.invoice_error = Some("The wallet is still starting.".into());
                return;
            };
            if shared.invoice_busy {
                return;
            }
            shared.invoice_busy = true;
            shared.invoice_error = None;
            (node, shared.generation)
        };
        let shared = self.shared.clone();
        std::thread::spawn(move || {
            let made = node.invoice(amount, INVOICE_DESCRIPTION);
            let mut state = lock(&shared);
            if state.generation != generation {
                return;
            }
            state.invoice_busy = false;
            match made {
                Ok(invoice) => state.invoice = Some((invoice, amount)),
                Err(message) => state.invoice_error = Some(message),
            }
        });
    }

    /// Quote a payment to what the person pasted or scanned. `comment` goes
    /// to an LNURL recipient that takes one.
    pub fn quote(&mut self, input: &str, amount: &str, comment: &str) {
        let input = input.trim().to_owned();
        if input.is_empty() {
            self.lock().send = Sending::Idle;
            return;
        }
        let format = self.lock().format;
        let amount = match parse_amount(amount, format) {
            Ok(amount) => amount,
            Err(message) => {
                let mut shared = self.lock();
                // Keep what the screen knew about the recipient.
                shared.send = Sending::NeedsAmount(match &shared.send {
                    Sending::NeedsAmount(ask) => Ask {
                        message,
                        ..ask.clone()
                    },
                    _ => Ask::amount(message),
                });
                return;
            }
        };
        let request = SendRequest {
            input,
            amount_sats: amount,
            comment: Some(comment.trim().to_owned()).filter(|comment| !comment.is_empty()),
            format,
        };
        let (node, directory, generation) = {
            let mut shared = self.lock();
            let Some(node) = shared.node.clone() else {
                shared.send = Sending::Failed("The wallet is still starting.".into());
                return;
            };
            if matches!(shared.send, Sending::Quoting | Sending::Paying(_)) {
                return;
            }
            shared.send = Sending::Quoting;
            shared.person = None;
            shared.save_suggestion = None;
            (node, shared.directory.clone(), shared.generation)
        };
        let shared = self.shared.clone();
        std::thread::spawn(move || {
            // An npub is paid through what its owner published.
            let person = match crate::payees::classify(&request.input) {
                crate::payees::Payload::Person { pubkey, npub } => Some(
                    directory
                        .ok_or_else(|| "Paying a Nostr profile isn't available here.".to_string())
                        .and_then(|directory| directory.profile(&pubkey))
                        .and_then(|profile| crate::payees::resolve(&npub, &profile)),
                ),
                _ => None,
            };
            let quoted = match &person {
                Some(Ok(resolved)) => node.quote(&SendRequest {
                    input: resolved.pay_to.clone(),
                    ..request
                }),
                Some(Err(message)) => Err(QuoteFailure::Refused(message.clone())),
                None => node.quote(&request),
            };
            let person = person.and_then(Result::ok);
            let mut state = lock(&shared);
            if state.generation != generation || state.send != Sending::Quoting {
                return;
            }
            state.send = match quoted {
                Ok(quote) => Sending::Quoted(quote),
                Err(QuoteFailure::NeedsAmount(mut ask)) => {
                    // Name the person and where their address came from.
                    if let Some(person) = &person {
                        ask.recipient = Some(person.label());
                        ask.description = Some(person.source());
                    }
                    Sending::NeedsAmount(ask)
                }
                Err(QuoteFailure::Refused(message)) => Sending::Failed(message),
            };
            state.person = person;
        });
    }

    /// Publish this wallet's Spark address in this device's Nostr payment
    /// targets (NIP-A3), or take it out. Only the person's explicit setting
    /// calls this.
    pub fn publish(&mut self, on: bool) {
        let (directory, spark) = {
            let mut shared = self.lock();
            let Some(directory) = shared.directory.clone() else {
                return;
            };
            if shared.publish_busy {
                return;
            }
            let spark = if on {
                match shared
                    .addresses
                    .spark
                    .clone()
                    .filter(|_| shared.node.is_some())
                {
                    Some(spark) => Some(spark),
                    None => {
                        shared.publish_message = Some("The wallet is still starting.".into());
                        return;
                    }
                }
            } else {
                None
            };
            shared.publish_busy = true;
            shared.publish_message = None;
            (directory, spark)
        };
        let (shared, home) = (self.shared.clone(), self.home.clone());
        std::thread::spawn(move || {
            let published = directory.publish(spark.as_deref());
            let mut state = lock(&shared);
            state.publish_busy = false;
            match published {
                Ok(relays) => {
                    match &spark {
                        Some(spark) => {
                            let _ = std::fs::create_dir_all(&home);
                            let _ = std::fs::write(home.join(PUBLISHED_FILE), spark);
                        }
                        None => {
                            let _ = std::fs::remove_file(home.join(PUBLISHED_FILE));
                        }
                    }
                    state.published = spark.clone();
                    state.publish_message = Some(format!(
                        "{} on {relays} relay{}.",
                        if spark.is_some() {
                            "Published"
                        } else {
                            "Removed"
                        },
                        if relays == 1 { "" } else { "s" }
                    ));
                }
                Err(message) => state.publish_message = Some(message),
            }
        });
    }

    /// Save a Lightning address as a contact.
    pub fn save_contact(&mut self, name: &str, address: &str) {
        let name = name.trim();
        let Some(address) = crate::payees::lightning_address(address) else {
            return;
        };
        if name.is_empty() || name.chars().count() > 100 {
            return;
        }
        let (node, generation) = {
            let mut shared = self.lock();
            let Some(node) = shared.node.clone() else {
                return;
            };
            shared.save_suggestion = None;
            shared.contact_busy = true;
            (node, shared.generation)
        };
        let (shared, name) = (self.shared.clone(), name.to_owned());
        std::thread::spawn(move || {
            let saved = node.add_contact(&name, &address);
            let contacts = node.contacts();
            let mut state = lock(&shared);
            state.contact_busy = false;
            if state.generation != generation {
                return;
            }
            if let Ok(contacts) = contacts {
                state.contacts = contacts;
            }
            if let Err(message) = saved {
                state.send = Sending::Failed(message);
            }
        });
    }

    /// Pay the quote the person confirmed. Only the quote on screen pays,
    /// and only once.
    pub fn pay(&mut self, quote_id: u64) {
        let (node, quote, generation) = {
            let mut shared = self.lock();
            let quote = match &shared.send {
                Sending::Quoted(quote) if quote.id == quote_id => quote.clone(),
                _ => return,
            };
            let Some(node) = shared.node.clone() else {
                return;
            };
            shared.send = Sending::Paying(quote.clone());
            (node, quote, shared.generation)
        };
        let (shared, home) = (self.shared.clone(), self.home.clone());
        let key = uuid::Uuid::new_v4().to_string();
        std::thread::spawn(move || {
            let paid = node.pay(quote.id, &key);
            {
                let mut state = lock(&shared);
                if state.generation != generation {
                    return;
                }
                let succeeded = matches!(&paid, Ok(paid) if paid.row.status != "failed");
                state.send = match paid {
                    Ok(paid) => Sending::Sent(paid),
                    Err(message) => Sending::Failed(message),
                };
                if succeeded {
                    remember(&mut state, &home, &quote);
                }
            }
            read(&shared, &home, generation, false);
        });
    }

    /// Start buying bitcoin with dollars from `provider` (`moonpay` or
    /// `cashapp`). The page to open arrives in the next packet.
    pub fn buy(&mut self, provider: &str, amount: &str) {
        let Some(provider) = Provider::parse(provider) else {
            self.lock().buy_error = Some("Choose MoonPay or Cash App.".into());
            return;
        };
        let format = self.lock().format;
        let amount = match parse_amount(amount, format) {
            Ok(Some(amount)) => amount,
            Ok(None) => {
                self.lock().buy_error = Some("Enter how much bitcoin to buy.".into());
                return;
            }
            Err(message) => {
                self.lock().buy_error = Some(message);
                return;
            }
        };
        let (node, generation) = {
            let mut shared = self.lock();
            let Some(node) = shared.node.clone() else {
                shared.buy_error = Some("The wallet is still starting.".into());
                return;
            };
            if shared.buy_busy {
                return;
            }
            shared.buy_busy = true;
            shared.buy_error = None;
            (node, shared.generation)
        };
        let shared = self.shared.clone();
        std::thread::spawn(move || {
            let bought = node.buy(provider, amount).and_then(|link| {
                // Only a web page opens; a provider's other schemes do not.
                match url::Url::parse(&link) {
                    Ok(url) if url.scheme() == "https" => Ok(link),
                    _ => Err("The provider returned a link this app won't open.".to_string()),
                }
            });
            let mut state = lock(&shared);
            if state.generation != generation {
                return;
            }
            state.buy_busy = false;
            match bought {
                Ok(link) => state.open_url = Some(link),
                Err(message) => state.buy_error = Some(message),
            }
        });
    }

    /// The purchase page to open, once.
    pub fn take_open_url(&mut self) -> Option<String> {
        self.lock().open_url.take()
    }

    /// Quote claiming a waiting deposit now.
    pub fn claim_quote(&mut self, txid: &str, vout: u32) {
        let (node, generation) = {
            let mut shared = self.lock();
            let known = shared
                .deposits
                .iter()
                .any(|deposit| deposit.txid == txid && deposit.vout == vout);
            let Some(node) = shared.node.clone().filter(|_| known) else {
                return;
            };
            if shared.claim.as_ref().is_some_and(|claim| claim.busy) {
                return;
            }
            shared.claim = Some(Claim {
                txid: txid.to_owned(),
                vout,
                quote: None,
                busy: true,
                message: None,
            });
            (node, shared.generation)
        };
        let (shared, txid) = (self.shared.clone(), txid.to_owned());
        std::thread::spawn(move || {
            let quoted = node.claim_quote(&txid, vout);
            let mut state = lock(&shared);
            if state.generation != generation {
                return;
            }
            if let Some(claim) = state.claim.as_mut().filter(|claim| claim.txid == txid) {
                claim.busy = false;
                match quoted {
                    Ok(quote) => claim.quote = Some(quote),
                    Err(message) => claim.message = Some(message),
                }
            }
        });
    }

    /// Claim the quoted deposit, with its quoted fee as the ceiling.
    pub fn claim(&mut self, txid: &str, vout: u32) {
        let (node, fee, generation) = {
            let mut shared = self.lock();
            let Some(node) = shared.node.clone() else {
                return;
            };
            let generation = shared.generation;
            let Some(claim) = shared.claim.as_mut() else {
                return;
            };
            let Some(quote) = claim
                .quote
                .filter(|_| claim.txid == txid && claim.vout == vout && !claim.busy)
            else {
                return;
            };
            claim.busy = true;
            claim.message = None;
            (node, quote.fee_sats, generation)
        };
        let (shared, home, txid) = (self.shared.clone(), self.home.clone(), txid.to_owned());
        std::thread::spawn(move || {
            let claimed = node.claim(&txid, vout, fee);
            {
                let mut state = lock(&shared);
                if state.generation != generation {
                    return;
                }
                if let Some(claim) = state.claim.as_mut().filter(|claim| claim.txid == txid) {
                    claim.busy = false;
                    claim.quote = None;
                    claim.message = Some(claimed.unwrap_or_else(|message| message));
                }
            }
            read(&shared, &home, generation, false);
        });
    }

    /// Choose how fast the quoted on-chain withdrawal confirms (`slow`,
    /// `medium`, or `fast`); the quote's fee follows.
    pub fn speed(&mut self, quote_id: u64, speed: &str) {
        let Some(speed) = Speed::parse(speed) else {
            return;
        };
        let mut shared = self.lock();
        let Some(node) = shared.node.clone() else {
            return;
        };
        let Sending::Quoted(quote) = &mut shared.send else {
            return;
        };
        if quote.id != quote_id || !quote.speeds.iter().any(|(offered, _)| *offered == speed) {
            return;
        }
        // Only the node's stored quote changes; nothing reaches the network.
        match node.set_speed(quote_id, speed) {
            Ok(fee) => {
                quote.fee_sats = fee;
                quote.speed = Some(speed);
            }
            Err(message) => shared.send = Sending::Failed(message),
        }
    }

    /// Start refunding a waiting deposit: read the fee rates.
    pub fn refund_start(&mut self, txid: &str, vout: u32) {
        let (node, generation) = {
            let mut shared = self.lock();
            let open = shared.deposits.iter().any(|deposit| {
                deposit.txid == txid && deposit.vout == vout && deposit.refund_txid.is_none()
            });
            let Some(node) = shared.node.clone().filter(|_| open) else {
                return;
            };
            if shared.refund.as_ref().is_some_and(|refund| refund.busy) {
                return;
            }
            shared.refund = Some(Refund {
                txid: txid.to_owned(),
                vout,
                rates: None,
                review: None,
                busy: true,
                message: None,
            });
            (node, shared.generation)
        };
        let (shared, txid) = (self.shared.clone(), txid.to_owned());
        std::thread::spawn(move || {
            let rates = node.fee_rates();
            let mut state = lock(&shared);
            if state.generation != generation {
                return;
            }
            if let Some(refund) = state.refund.as_mut().filter(|refund| refund.txid == txid) {
                refund.busy = false;
                match rates {
                    Ok(rates) => refund.rates = Some(rates),
                    Err(message) => refund.message = Some(message),
                }
            }
        });
    }

    /// Review a refund to `address` at `speed`, from the rates read. Nothing
    /// is sent until [`Wallet::refund`].
    pub fn refund_review(&mut self, txid: &str, vout: u32, address: &str, speed: &str) {
        let mut shared = self.lock();
        let amount = shared
            .deposits
            .iter()
            .find(|deposit| deposit.txid == txid && deposit.vout == vout)
            .map(|deposit| deposit.amount_sats);
        let own = shared.addresses.bitcoin.clone();
        let Some(refund) = shared
            .refund
            .as_mut()
            .filter(|refund| refund.txid == txid && refund.vout == vout && !refund.busy)
        else {
            return;
        };
        let (Some(rates), Some(amount)) = (refund.rates, amount) else {
            return;
        };
        refund.review = None;
        let address = address.trim();
        let Some(speed) = Speed::parse(speed) else {
            refund.message = Some("Choose how fast the refund should confirm.".into());
            return;
        };
        if !bitcoin_address_shape(address) {
            refund.message = Some("Enter a Bitcoin address to refund to.".into());
            return;
        }
        if own
            .as_deref()
            .is_some_and(|own| own.eq_ignore_ascii_case(address))
        {
            refund.message =
                Some("That is this wallet's own deposit address. Refund to another wallet.".into());
            return;
        }
        let rate = rates.rate(speed);
        let fee_sats = rate.saturating_mul(REFUND_VBYTES);
        if fee_sats >= amount {
            refund.message = Some(format!(
                "At {rate} sat/vB the fee would take the whole deposit. Choose a slower speed or claim it instead."
            ));
            return;
        }
        refund.message = None;
        refund.review = Some(RefundReview {
            address: address.to_owned(),
            speed,
            rate,
            fee_sats,
            amount_sats: amount,
        });
    }

    /// Send the reviewed refund, once.
    pub fn refund(&mut self, txid: &str, vout: u32) {
        let (node, review, generation) = {
            let mut shared = self.lock();
            let Some(node) = shared.node.clone() else {
                return;
            };
            let generation = shared.generation;
            let Some(refund) = shared.refund.as_mut() else {
                return;
            };
            let Some(review) = refund
                .review
                .take()
                .filter(|_| refund.txid == txid && refund.vout == vout && !refund.busy)
            else {
                return;
            };
            refund.busy = true;
            refund.message = None;
            (node, review, generation)
        };
        let (shared, home, txid) = (self.shared.clone(), self.home.clone(), txid.to_owned());
        std::thread::spawn(move || {
            let sent = node.refund(&txid, vout, &review.address, review.rate);
            {
                let mut state = lock(&shared);
                if state.generation != generation {
                    return;
                }
                if let Some(refund) = state.refund.as_mut().filter(|refund| refund.txid == txid) {
                    refund.busy = false;
                    refund.message = Some(match sent {
                        Ok(refund_txid) => format!(
                            "Refund sent to {} in transaction {}. It arrives once it confirms.",
                            shorten(&review.address),
                            shorten(&refund_txid)
                        ),
                        Err(message) => message,
                    });
                }
            }
            read(&shared, &home, generation, false);
        });
    }

    /// Close the refund step.
    pub fn refund_reset(&mut self) {
        let mut shared = self.lock();
        if !shared.refund.as_ref().is_some_and(|refund| refund.busy) {
            shared.refund = None;
        }
    }

    /// The saved unilateral-exit state as a file for the person to keep:
    /// its name and contents. Only the direct reply to an explicit export
    /// carries it.
    pub fn exit_export(&self) -> Result<(String, String), String> {
        let vault = self.lock().vault.clone();
        let saved = vault.and_then(|vault| vault.load()).ok_or_else(|| {
            "No exit backup has been saved yet. Refresh the wallet first.".to_string()
        })?;
        Ok((
            format!("openagents-spark-exit-{}.json", saved.saved_at),
            saved.state,
        ))
    }

    /// Close the claim step.
    pub fn claim_reset(&mut self) {
        let mut shared = self.lock();
        if !shared.claim.as_ref().is_some_and(|claim| claim.busy) {
            shared.claim = None;
        }
    }

    /// Close the send review or its result.
    pub fn reset_send(&mut self) {
        let mut shared = self.lock();
        if !matches!(shared.send, Sending::Paying(_) | Sending::Quoting) {
            shared.send = Sending::Idle;
            shared.person = None;
            shared.save_suggestion = None;
        }
    }

    /// The person read the trust note.
    pub fn acknowledge(&mut self) {
        let _ = std::fs::create_dir_all(&self.home);
        let _ = std::fs::write(self.home.join(TRUST_FILE), b"1\n");
        self.lock().trust_acknowledged = true;
    }

    /// Whether a start, a sync, a quote, or a payment runs in the background.
    pub fn loading(&self) -> bool {
        let shared = self.lock();
        shared.starting
            || shared.refreshing
            || shared.invoice_busy
            || shared.buy_busy
            || shared.claim.as_ref().is_some_and(|claim| claim.busy)
            || shared.refund.as_ref().is_some_and(|refund| refund.busy)
            || shared.publish_busy
            || shared.contact_busy
            || matches!(shared.send, Sending::Quoting | Sending::Paying(_))
    }

    pub fn screen(&self) -> Screen {
        let shared = self.lock();
        let status = match (&shared.node, &self.seed) {
            (None, None) if shared.error.is_none() => Some("Opening the wallet…"),
            (None, _) if shared.error.is_some() && !shared.starting => {
                return Screen::Failed {
                    message: shared.error.clone().unwrap_or_default(),
                };
            }
            (None, _) => Some("Connecting to Spark…"),
            (Some(_), _) if !shared.synced && shared.refreshing => Some("Reading Spark…"),
            (Some(_), _) => None,
        };
        let mut error = shared.error.clone();
        if status.is_none() && !shared.synced && error.is_none() {
            error = Some("The wallet has not read Spark yet.".into());
        }
        let format = shared.format;
        let shown = shared.last;
        let total = shown.map_or(0, |last| last.total);
        let addresses = &shared.addresses;
        let stale = shown
            .and_then(|last| last.synced_at)
            .is_none_or(|at| now().saturating_sub(at) > STALE_SECS);
        let payments: Vec<PaymentView> = shared
            .payments
            .iter()
            .map(|row| payment_view(row, format))
            .collect();
        let open = shared
            .deposits
            .iter()
            .filter(|row| row.refund_txid.is_none());
        let stuck = open.clone().filter(|row| row.problem.is_some()).count();
        let arriving = open.count() - stuck;
        let backup_card = self
            .seed
            .as_ref()
            .filter(|seed| self.words_saved.as_deref() != Some(seed.fingerprint().as_str()))
            .map(|_| BackupCard {
                title: "Back up your wallet",
                detail: "If you lose this phone, your recovery words are the only way to get your bitcoin back.",
                action: "Show my recovery words",
            });
        Screen::Ready(Box::new(Summary {
            stale,
            recent: payments.iter().take(RECENT_LIMIT).cloned().collect(),
            more_payments: payments.len() > RECENT_LIMIT,
            backup_card,
            advanced: AdvancedView {
                open: shared.advanced_open,
                note: match (stuck, arriving) {
                    (0, 0) => None,
                    (0, 1) => Some("A deposit is on its way".into()),
                    (0, count) => Some(format!("{count} deposits are on their way")),
                    (1, _) => Some("A deposit needs you".into()),
                    (count, _) => Some(format!("{count} deposits need you")),
                },
            },
            network: NETWORK_LABEL,
            balance_sats: total,
            balance: shown.map(|_| format.show(total)).unwrap_or_default(),
            balance_alternate: shown
                .map(|_| format.other().show(total))
                .unwrap_or_default(),
            balance_spoken: shown.map(|_| format.spoken(total)).unwrap_or_default(),
            empty: shown.is_some() && total == 0,
            synced_at: shown.and_then(|last| last.synced_at),
            refreshing: shared.refreshing || shared.starting,
            error,
            balance_unknown: shown.is_none(),
            status: status.map(str::to_owned),
            warning: (total > BALANCE_WARNING_SATS).then(|| {
                format!(
                    "This phone wallet holds more than {}. Keep only what you'd carry, and make sure your recovery words are written down.",
                    format.show(BALANCE_WARNING_SATS)
                )
            }),
            trust: Trust {
                acknowledged: shared.trust_acknowledged,
                title: TRUST_TITLE,
                summary: TRUST_SUMMARY,
                lines: TRUST_LINES.to_vec(),
            },
            receive: Receive {
                lightning: shared.invoice.as_ref().map(|(invoice, amount)| {
                    code(
                        invoice,
                        &format!("lightning:{invoice}"),
                        match amount {
                            Some(amount) => {
                                format!("Request for {}", format.show(*amount))
                            }
                            None => "Request for any amount".into(),
                        },
                    )
                }),
                lightning_busy: shared.invoice_busy,
                lightning_error: shared.invoice_error.clone(),
                spark: addresses.spark.as_ref().map(|address| {
                    code(
                        address,
                        address,
                        "Spark address: free and instant from other Spark wallets".into(),
                    )
                }),
                bitcoin: addresses.bitcoin.as_ref().map(|address| {
                    code(
                        address,
                        &format!("bitcoin:{address}"),
                        "Bitcoin address: credited after 3 confirmations".into(),
                    )
                }),
                nostr: shared.npub.as_ref().map(|npub| {
                    code(
                        npub,
                        &format!("nostr:{npub}"),
                        if shared.published.is_some() {
                            "Your npub: OpenAgents users pay it to your published Spark address".into()
                        } else {
                            "Your npub: publish your Spark address below so others can pay it".into()
                        },
                    )
                }),
                publish: shared.directory.as_ref().map(|_| PublishView {
                    on: shared.published.is_some(),
                    busy: shared.publish_busy,
                    detail: "Publish this wallet's Spark address in your Nostr profile so people can pay your npub. Anyone can read it and link it to your npub.",
                    message: shared.publish_message.clone(),
                }),
            },
            send: {
                let mut view = send_view(&shared.send, format);
                if matches!(shared.send, Sending::Quoted(_) | Sending::Paying(_) | Sending::Sent(_))
                    && let Some(person) = &shared.person
                {
                    view.person = Some(person.label());
                    view.person_source = Some(person.source());
                }
                view.save_suggestion = shared.save_suggestion.clone();
                view
            },
            payments,
            can_show_words: self.seed.is_some(),
            buy: BuyView {
                busy: shared.buy_busy,
                error: shared.buy_error.clone(),
                providers: vec![
                    ProviderView {
                        id: "moonpay",
                        label: "Card or Apple Pay",
                        detail: "Through MoonPay. Arrives on-chain and is credited after 3 confirmations.",
                    },
                    ProviderView {
                        id: "cashapp",
                        label: "Cash App",
                        detail: "Pays a Lightning invoice from your Cash App balance or debit card. Credited at once.",
                    },
                ],
            },
            deposits: shared
                .deposits
                .iter()
                .map(|row| deposit_view(row, format))
                .collect(),
            claim: shared.claim.as_ref().map(|claim| claim_view(claim, format)),
            refund: shared.refund.as_ref().map(|refund| refund_view(refund, format)),
            people: shared
                .contacts
                .iter()
                .map(|contact| PersonView {
                    name: contact.name.clone(),
                    detail: contact.address.clone(),
                    input: contact.address.clone(),
                })
                .chain(shared.people.iter().map(|person| PersonView {
                    name: person
                        .name
                        .clone()
                        .unwrap_or_else(|| crate::payees::short_npub(&person.npub)),
                    detail: crate::payees::short_npub(&person.npub),
                    input: person.npub.clone(),
                }))
                .collect(),
            backup: BackupView {
                title: "Exit backup",
                detail: "If Spark's operators ever stop, this file and your recovery words let you take your bitcoin out on the Bitcoin chain yourself. The wallet keeps it up to date on this phone. It holds no keys, but it shows your balance, so keep the exported file private.",
                saved_at: shared.exit_saved_at,
                can_export: shared.exit_saved_at.is_some(),
                error: shared.exit_error.clone(),
            },
        }))
    }
}

/// After a payment: keep a person paid by npub for the Send screen, and
/// offer to save a Lightning address that isn't a contact yet.
fn remember(state: &mut Shared, home: &Path, quote: &Quote) {
    if let Some(person) = state.person.clone() {
        state.people.retain(|known| known.npub != person.npub);
        state.people.insert(
            0,
            PaidPerson {
                npub: person.npub,
                name: person.name,
            },
        );
        state.people.truncate(PEOPLE_LIMIT);
        write_json(home, PEOPLE_FILE, &state.people);
    } else if let Destination::LightningAddress(address) = &quote.destination
        && crate::payees::lightning_address(address).is_some()
        && !state
            .contacts
            .iter()
            .any(|contact| contact.address.eq_ignore_ascii_case(address))
    {
        state.save_suggestion = Some(address.clone());
    }
}

fn lock(shared: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(|poison| poison.into_inner())
}

/// Read the balance and recent payments into the screen's state, syncing
/// first when `sync` is set. It changes nothing if the wallet was replaced.
fn read(shared: &Mutex<Shared>, home: &Path, generation: u64, sync: bool) {
    let Some(node) = lock(shared).node.clone() else {
        return;
    };
    let synced = if sync { node.sync() } else { Ok(()) };
    let vault = lock(shared).vault.clone();
    let backup = match (&synced, vault) {
        (Ok(()), Some(vault)) => Some(back_up(node.as_ref(), vault.as_ref())),
        _ => None,
    };
    let balance = node.balance();
    let payments = node.payments(HISTORY_LIMIT);
    let deposits = node.deposits();
    let contacts = node.contacts();
    let mut state = lock(shared);
    if state.generation != generation {
        return;
    }
    if sync {
        state.refreshing = false;
    }
    match (&synced, &balance) {
        (Ok(()), Ok(total)) => {
            let read = LastBalance {
                total: *total,
                synced_at: Some(now()),
            };
            read.write(home);
            state.last = Some(read);
            state.synced = true;
            state.error = None;
        }
        (Err(message), _) | (_, Err(message)) => state.error = Some(message.clone()),
    }
    if let Ok(payments) = payments {
        write_json(home, PAYMENTS_FILE, &payments);
        state.payments = payments;
    }
    if let Ok(deposits) = deposits {
        state.deposits = deposits;
    }
    if let Ok(contacts) = contacts {
        state.contacts = contacts;
    }
    match backup {
        Some(Ok(saved_at)) => {
            state.exit_saved_at = Some(saved_at);
            state.exit_error = None;
        }
        Some(Err(message)) => state.exit_error = Some(message),
        None => {}
    }
}

/// Save the exit state when it changed; when the saved one dates from.
fn back_up(node: &dyn Node, vault: &dyn Vault) -> Result<u64, String> {
    let state = node
        .exit_state()
        .map_err(|_| "The exit backup could not be read from the wallet.".to_string())?;
    if let Some(saved) = vault.load().filter(|saved| saved.state == state) {
        return Ok(saved.saved_at);
    }
    let saved = SavedExit {
        state,
        saved_at: now(),
    };
    vault
        .save(&saved)
        .map_err(|_| "The exit backup could not be saved on this phone.".to_string())?;
    Ok(saved.saved_at)
}

fn code(text: &str, uri: &str, caption: String) -> Code {
    Code {
        text: text.to_owned(),
        uri: uri.to_owned(),
        // Upper case fits the QR code's compact alphanumeric mode; these
        // schemes and bech32 payloads are case-insensitive.
        qr: qr(&uri.to_ascii_uppercase()),
        caption,
    }
}

fn send_view(send: &Sending, format: Format) -> SendView {
    let mut view = SendView {
        state: "idle",
        message: None,
        quote: None,
        result: None,
        recipient: None,
        description: None,
        comment_max: None,
        recipient_message: None,
        person: None,
        person_source: None,
        save_suggestion: None,
    };
    match send {
        Sending::Idle => {}
        Sending::Quoting => {
            view.state = "quoting";
            view.message = Some("Preparing the payment…".into());
        }
        Sending::NeedsAmount(ask) => {
            view.state = "needs_amount";
            view.message = Some(ask.message.clone());
            view.recipient = ask.recipient.clone();
            view.description = ask.description.clone();
            view.comment_max = (ask.comment_max > 0).then_some(ask.comment_max);
        }
        Sending::Quoted(quote) => {
            view.state = "quoted";
            view.quote = Some(quote_view(quote, format));
        }
        Sending::Paying(quote) => {
            view.state = "paying";
            view.message = Some("Sending…".into());
            view.quote = Some(quote_view(quote, format));
        }
        Sending::Sent(paid) => {
            view.state = "sent";
            view.message = Some(match paid.row.status.as_str() {
                "pending" => "Sent. The payment is still settling.".to_string(),
                "failed" => "The payment failed.".to_string(),
                _ => "Sent.".to_string(),
            });
            view.result = Some(payment_view(&paid.row, format));
            view.recipient_message = paid.message.clone();
        }
        Sending::Failed(message) => {
            view.state = "failed";
            view.message = Some(message.clone());
        }
    }
    view
}

fn deposit_view(row: &DepositRow, format: Format) -> DepositView {
    DepositView {
        txid: row.txid.clone(),
        vout: row.vout,
        amount: format.show(row.amount_sats),
        status: match (&row.refund_txid, &row.problem, row.mature) {
            (Some(refund), _, _) => format!(
                "Refund sent in transaction {}. It leaves the wallet once it confirms.",
                shorten(refund)
            ),
            (None, Some(problem), _) => {
                let problem = match problem {
                    DepositProblem::FeeAboveLimit(fee) => format!(
                        "Claiming it costs {}, above the automatic limit.",
                        format.show(*fee)
                    ),
                    DepositProblem::Missing => "The deposit wasn't found on the chain.".into(),
                    DepositProblem::Failed(message) => {
                        format!("The last claim failed ({message}).")
                    }
                };
                format!("{problem} Claim it at a quoted fee, or refund it on-chain.")
            }
            (None, None, false) => "Waiting for 3 confirmations.".into(),
            (None, None, true) => "Confirmed; the wallet is claiming it.".into(),
        },
        actionable: row.refund_txid.is_none(),
    }
}

fn refund_view(refund: &Refund, format: Format) -> RefundView {
    RefundView {
        txid: refund.txid.clone(),
        vout: refund.vout,
        busy: refund.busy,
        speeds: refund.rates.map_or_else(Vec::new, |rates| {
            Speed::ALL
                .into_iter()
                .map(|speed| {
                    let rate = rates.rate(speed);
                    SpeedView {
                        id: speed.id(),
                        label: speed.label(),
                        fee: format!(
                            "about {} at {rate} sat/vB",
                            format.show(rate.saturating_mul(REFUND_VBYTES))
                        ),
                        chosen: refund
                            .review
                            .as_ref()
                            .is_some_and(|review| review.speed == speed),
                    }
                })
                .collect()
        }),
        review: refund.review.as_ref().map(|review| {
            format!(
                "Refund {} to {}; about {} fee at {} sat/vB.",
                format.show(review.amount_sats.saturating_sub(review.fee_sats)),
                shorten(&review.address),
                format.show(review.fee_sats),
                review.rate
            )
        }),
        message: refund.message.clone(),
    }
}

fn claim_view(claim: &Claim, format: Format) -> ClaimView {
    ClaimView {
        txid: claim.txid.clone(),
        vout: claim.vout,
        busy: claim.busy,
        quote: claim.quote.map(|quote| {
            let when = if quote.early {
                "Claim now"
            } else if quote.confirmations >= quote.confirmations_required {
                "Claim"
            } else {
                "Claim at 3 confirmations"
            };
            format!(
                "{when} for a {} fee; {} reaches your balance.",
                format.show(quote.fee_sats),
                format.show(quote.credit_sats)
            )
        }),
        message: claim.message.clone(),
    }
}

fn quote_view(quote: &Quote, format: Format) -> QuoteView {
    let (kind, to, destination) = match &quote.destination {
        Destination::Lightning(invoice) => {
            ("Lightning invoice", "Payment request", shorten(invoice))
        }
        Destination::LightningAddress(address) => ("Lightning address", "Address", address.clone()),
        Destination::Spark(address) => ("Spark address", "Wallet address", shorten(address)),
        Destination::Bitcoin(address) => ("Bitcoin address", "Bitcoin address", shorten(address)),
    };
    QuoteView {
        id: quote.id,
        kind,
        to,
        destination,
        amount: format.show(quote.amount_sats),
        fee: format.show(quote.fee_sats),
        total: format.show(quote.amount_sats.saturating_add(quote.fee_sats)),
        note: quote.note.clone(),
        comment: quote.comment.clone(),
        speeds: quote
            .speeds
            .iter()
            .map(|(speed, fee)| SpeedView {
                id: speed.id(),
                label: speed.label(),
                fee: format.show(*fee),
                chosen: quote.speed == Some(*speed),
            })
            .collect(),
    }
}

fn payment_view(row: &PaymentRow, format: Format) -> PaymentView {
    PaymentView {
        id: row.id.clone(),
        title: if row.received { "Received" } else { "Sent" },
        amount: format.show_signed(row.amount_sats, row.received),
        fee: (!row.received && row.fee_sats > 0)
            .then(|| format!("{} fee", format.show(row.fee_sats))),
        method: row.method.clone(),
        status: row.status.clone(),
        at: row.at,
    }
}

/// A mainnet Bitcoin address by its shape: bech32 `bc1…`, or base58 `1…`
/// or `3…`. The SDK checks it fully when it builds the transaction.
fn bitcoin_address_shape(address: &str) -> bool {
    let lower = address.to_ascii_lowercase();
    let bech32 = lower.starts_with("bc1")
        && (42..=62).contains(&lower.len())
        && (address == lower || address == address.to_ascii_uppercase())
        && lower[3..]
            .chars()
            .all(|c| "qpzry9x8gf2tvdw0s3jn54khce6mua7l".contains(c));
    let base58 = (address.starts_with('1') || address.starts_with('3'))
        && (26..=35).contains(&address.len())
        && address
            .chars()
            .all(|c| c.is_ascii_alphanumeric() && !matches!(c, '0' | 'O' | 'I' | 'l'));
    bech32 || base58
}

/// The start and end of a long code, for the confirm screen.
fn shorten(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= 28 {
        return text.to_owned();
    }
    let head: String = chars[..14].iter().collect();
    let tail: String = chars[chars.len() - 10..].iter().collect();
    format!("{head}…{tail}")
}

/// An amount the person typed, as base units, in the app's format: whole
/// base units (BIP 177) or decimal BTC (legacy). Empty means none.
fn parse_amount(text: &str, format: Format) -> Result<Option<u64>, String> {
    format.parse(text).map_err(|error| error.message(format))
}

fn decode(hex_text: &str) -> Result<Vec<u8>, String> {
    let bytes = (matches!(hex_text.len(), 32 | 64)
        && hex_text.bytes().all(|b| b.is_ascii_hexdigit()))
    .then(|| {
        (0..hex_text.len() / 2)
            .map(|i| u8::from_str_radix(&hex_text[i * 2..i * 2 + 2], 16).ok())
            .collect::<Option<Vec<u8>>>()
    })
    .flatten();
    bytes.ok_or_else(|| "The wallet key is unreadable.".to_string())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn qr(text: &str) -> Option<crate::app::QrModules> {
    let code = qrcodegen::QrCode::encode_text(text, qrcodegen::QrCodeEcc::Medium).ok()?;
    let side = code.size() + 8;
    Some(crate::app::QrModules {
        size: usize::try_from(side).ok()?,
        rows: (0..side)
            .map(|y| {
                (0..side)
                    .map(|x| {
                        if code.get_module(x - 4, y - 4) {
                            '1'
                        } else {
                            '0'
                        }
                    })
                    .collect()
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::time::{Duration, Instant};

    /// 16 bytes: a 12-word wallet, as the host creates.
    const ENTROPY: &str = "5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a";
    const INVOICE: &str = "lnbc10u1pfakeinvoice0000000000000000000000000000000000000000";

    #[derive(Default)]
    struct Fake {
        balance: AtomicU64,
        sync_fails: AtomicBool,
        invoices: AtomicU64,
        paid: Mutex<Vec<(u64, String)>>,
        payments: Mutex<Vec<PaymentRow>>,
        deposits: Mutex<Vec<DepositRow>>,
        claims: Mutex<Vec<(String, u32, u64)>>,
        refunds: Mutex<Vec<(String, u32, String, u64)>>,
        speeds: Mutex<Vec<(u64, Speed)>>,
        exit: Mutex<String>,
        contacts: Mutex<Vec<Contact>>,
        notify: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
    }

    impl Node for Fake {
        fn balance(&self) -> Result<u64, String> {
            Ok(self.balance.load(Ordering::SeqCst))
        }
        fn sync(&self) -> Result<(), String> {
            if self.sync_fails.load(Ordering::SeqCst) {
                Err("Spark could not be read (timeout). Refresh to try again.".into())
            } else {
                Ok(())
            }
        }
        fn spark_address(&self) -> Result<String, String> {
            Ok("spark1fakeaddress".into())
        }
        fn bitcoin_address(&self) -> Result<String, String> {
            Ok("bc1qfakedeposit".into())
        }
        fn invoice(&self, amount: Option<u64>, description: &str) -> Result<String, String> {
            assert_eq!(description, "OpenAgents");
            self.invoices.fetch_add(1, Ordering::SeqCst);
            Ok(format!("{INVOICE}{}", amount.unwrap_or(0)))
        }
        fn quote(&self, request: &SendRequest) -> Result<Quote, QuoteFailure> {
            match (request.input.as_str(), request.amount_sats) {
                ("lnbc-with-amount", _) => Ok(Quote {
                    id: 7,
                    destination: Destination::Lightning("lnbc-with-amount".into()),
                    amount_sats: 1_000,
                    fee_sats: 3,
                    note: Some("Coffee".into()),
                    comment: None,
                    speeds: vec![],
                    speed: None,
                }),
                ("bc1qfriend", Some(amount)) => Ok(Quote {
                    id: 10,
                    destination: Destination::Bitcoin("bc1qfriend".into()),
                    amount_sats: amount,
                    fee_sats: 400,
                    note: None,
                    comment: None,
                    speeds: vec![(Speed::Slow, 250), (Speed::Medium, 400), (Speed::Fast, 900)],
                    speed: Some(Speed::Medium),
                }),
                ("spark1friend", None) => Err(QuoteFailure::NeedsAmount(Ask::amount(
                    "Enter the amount to send to this address.",
                ))),
                ("spark1friend", Some(amount)) => Ok(Quote {
                    id: 8,
                    destination: Destination::Spark("spark1friend".into()),
                    amount_sats: amount,
                    fee_sats: 0,
                    note: None,
                    comment: None,
                    speeds: vec![],
                    speed: None,
                }),
                // A Lightning address and an LNURL code resolve to a pay
                // request, whose terms decide what is asked.
                ("alice@example.com" | "lnurl1fixedprice", amount) => {
                    let terms = if request.input.starts_with("alice") {
                        LnurlTerms::of(
                            "alice@example.com".into(),
                            1_000,
                            5_000_000,
                            20,
                            r#"[["text/identifier","alice@example.com"],["text/plain","Sats for Alice"]]"#,
                        )
                    } else {
                        LnurlTerms::of("shop.example".into(), 21_000_000, 21_000_000, 0, "[]")
                    };
                    let (amount, comment) =
                        terms.check(amount, request.comment.as_deref(), request.format)?;
                    Ok(Quote {
                        id: 9,
                        destination: Destination::LightningAddress(terms.recipient.clone()),
                        amount_sats: amount,
                        fee_sats: 2,
                        note: terms.description,
                        comment,
                        speeds: vec![],
                        speed: None,
                    })
                }
                _ => Err(QuoteFailure::Refused(
                    "That isn't a payment request.".into(),
                )),
            }
        }
        fn pay(&self, quote: u64, key: &str) -> Result<Paid, String> {
            self.paid.lock().unwrap().push((quote, key.to_owned()));
            let row = PaymentRow {
                id: format!("pay-{quote}"),
                received: false,
                amount_sats: 1_000,
                fee_sats: 3,
                method: "Lightning".into(),
                status: "completed".into(),
                at: 1_790_000_000,
            };
            self.payments.lock().unwrap().insert(0, row.clone());
            self.balance.fetch_sub(1_003, Ordering::SeqCst);
            Ok(Paid {
                row,
                message: (quote == 9).then(|| "Thanks for the sats!".to_owned()),
            })
        }
        fn payments(&self, _limit: u32) -> Result<Vec<PaymentRow>, String> {
            Ok(self.payments.lock().unwrap().clone())
        }
        fn buy(&self, provider: Provider, amount: u64) -> Result<String, String> {
            match provider {
                Provider::Moonpay => Ok(format!("https://buy.moonpay.com/?amount={amount}")),
                Provider::CashApp if amount == 666 => Ok("cashapp://pay".into()),
                Provider::CashApp => Ok(format!("https://cash.app/launch/lightning/lnbc{amount}")),
            }
        }
        fn deposits(&self) -> Result<Vec<DepositRow>, String> {
            Ok(self.deposits.lock().unwrap().clone())
        }
        fn claim_quote(&self, _txid: &str, _vout: u32) -> Result<ClaimQuote, String> {
            Ok(ClaimQuote {
                fee_sats: 1_200,
                credit_sats: 48_800,
                early: false,
                confirmations: 3,
                confirmations_required: 3,
            })
        }
        fn claim(&self, txid: &str, vout: u32, max_fee: u64) -> Result<String, String> {
            self.claims
                .lock()
                .unwrap()
                .push((txid.to_owned(), vout, max_fee));
            self.deposits.lock().unwrap().clear();
            self.balance.fetch_add(48_800, Ordering::SeqCst);
            Ok("Claimed. It's in your balance.".into())
        }
        fn fee_rates(&self) -> Result<FeeRates, String> {
            Ok(FeeRates {
                fastest: 20,
                half_hour: 8,
                hour: 0,
            })
        }
        fn refund(
            &self,
            txid: &str,
            vout: u32,
            address: &str,
            rate: u64,
        ) -> Result<String, String> {
            self.refunds
                .lock()
                .unwrap()
                .push((txid.to_owned(), vout, address.to_owned(), rate));
            for deposit in self.deposits.lock().unwrap().iter_mut() {
                if deposit.txid == txid {
                    deposit.refund_txid = Some("cd".repeat(32));
                }
            }
            Ok("cd".repeat(32))
        }
        fn set_speed(&self, quote: u64, speed: Speed) -> Result<u64, String> {
            self.speeds.lock().unwrap().push((quote, speed));
            Ok(match speed {
                Speed::Slow => 250,
                Speed::Medium => 400,
                Speed::Fast => 900,
            })
        }
        fn exit_state(&self) -> Result<String, String> {
            Ok(self.exit.lock().unwrap().clone())
        }
        fn contacts(&self) -> Result<Vec<Contact>, String> {
            Ok(self.contacts.lock().unwrap().clone())
        }
        fn add_contact(&self, name: &str, address: &str) -> Result<(), String> {
            self.contacts.lock().unwrap().push(Contact {
                name: name.to_owned(),
                address: address.to_owned(),
            });
            Ok(())
        }
        fn subscribe(&self, notify: Arc<dyn Fn() + Send + Sync>) {
            *self.notify.lock().unwrap() = Some(notify);
        }
    }

    #[derive(Default)]
    struct MemoryVault {
        saved: Mutex<Option<SavedExit>>,
        saves: AtomicU64,
    }

    impl Vault for MemoryVault {
        fn save(&self, saved: &SavedExit) -> Result<(), String> {
            self.saves.fetch_add(1, Ordering::SeqCst);
            *self.saved.lock().unwrap() = Some(saved.clone());
            Ok(())
        }
        fn load(&self) -> Option<SavedExit> {
            self.saved.lock().unwrap().clone()
        }
        fn clear(&self) {
            *self.saved.lock().unwrap() = None;
        }
    }

    fn deposit(
        txid: &str,
        mature: bool,
        problem: Option<DepositProblem>,
        refund: Option<&str>,
    ) -> DepositRow {
        DepositRow {
            txid: txid.into(),
            vout: 0,
            amount_sats: 40_000,
            mature,
            problem,
            refund_txid: refund.map(str::to_owned),
        }
    }

    /// Profiles by hex key, and what was published.
    #[derive(Default)]
    struct Profiles {
        known: Mutex<std::collections::HashMap<String, crate::payees::Profile>>,
        published: Mutex<Vec<Option<String>>>,
    }

    impl crate::payees::Directory for Profiles {
        fn profile(&self, pubkey: &str) -> Result<crate::payees::Profile, String> {
            Ok(self
                .known
                .lock()
                .unwrap()
                .get(pubkey)
                .cloned()
                .unwrap_or_default())
        }
        fn publish(&self, spark: Option<&str>) -> Result<usize, String> {
            self.published
                .lock()
                .unwrap()
                .push(spark.map(str::to_owned));
            Ok(3)
        }
    }

    fn npub_of(byte: u8) -> (String, String) {
        let secret = SecretKeyFor::from_byte_array([byte; 32]).unwrap();
        let (key, _) = secret.x_only_public_key(&secp256k1::Secp256k1::new());
        (key.to_string(), nostr::nip19::encode_npub(&key.serialize()))
    }
    use secp256k1::SecretKey as SecretKeyFor;

    #[test]
    fn an_npub_is_paid_at_its_published_address_after_the_person_confirms() {
        let home = tempfile::tempdir().expect("temp dir");
        let node = Arc::new(Fake::default());
        node.balance.store(90_000, Ordering::SeqCst);
        let profiles = Arc::new(Profiles::default());
        let (alice_hex, alice) = npub_of(7);
        let (bob_hex, bob) = npub_of(8);
        let (_, carol) = npub_of(9);
        profiles.known.lock().unwrap().insert(
            alice_hex,
            crate::payees::Profile {
                name: Some("Alice".into()),
                spark: Some("spark1friend".into()),
                lightning_address: Some("alice@example.com".into()),
            },
        );
        profiles.known.lock().unwrap().insert(
            bob_hex,
            crate::payees::Profile {
                name: None,
                spark: None,
                lightning_address: Some("alice@example.com".into()),
            },
        );
        let mut wallet = Wallet::new(
            home.path().to_path_buf(),
            opener(node.clone(), Arc::new(Mutex::new(vec![]))),
        )
        .with_directory(profiles.clone(), "npub1me".into());
        wallet.open(ENTROPY, false);
        settle(&wallet);

        // A scanned `nostr:` code resolves to the Spark address first; the
        // screen names the person and the address before any amount.
        wallet.quote(&format!("nostr:{alice}"), "", "");
        settle(&wallet);
        let asked = ready(&wallet).send;
        assert_eq!(asked.state, "needs_amount");
        let label = asked.recipient.expect("person");
        assert!(label.starts_with("Alice (npub1"), "{label}");
        assert_eq!(
            asked.description.as_deref(),
            Some("Their published Spark address, spark1friend")
        );
        wallet.quote(&alice, "1,500", "");
        settle(&wallet);
        let quoted = ready(&wallet).send;
        let quote = quoted.quote.expect("quote");
        assert_eq!(
            (quote.kind, quote.destination.as_str()),
            ("Spark address", "spark1friend")
        );
        assert!(quoted.person.expect("person").starts_with("Alice"));
        assert!(quoted.person_source.unwrap().contains("spark1friend"));
        assert!(
            node.paid.lock().unwrap().is_empty(),
            "nothing pays before the tap"
        );
        wallet.pay(quote.id);
        settle(&wallet);
        let paid = ready(&wallet);
        assert_eq!(paid.send.state, "sent");
        assert_eq!(paid.people.len(), 1);
        assert_eq!(paid.people[0].name, "Alice");
        assert_eq!(paid.people[0].input, alice);
        assert_eq!(
            paid.send.save_suggestion, None,
            "a person isn't a contact suggestion"
        );
        wallet.reset_send();

        // Without a Spark address, the profile's Lightning address.
        wallet.quote(&bob, "2000", "");
        settle(&wallet);
        let quoted = ready(&wallet).send;
        assert_eq!(quoted.quote.expect("quote").kind, "Lightning address");
        assert!(
            quoted
                .person_source
                .unwrap()
                .contains("Lightning address, alice@example.com")
        );
        wallet.reset_send();

        // Neither: refused, and nothing is quoted.
        wallet.quote(&carol, "2000", "");
        settle(&wallet);
        let refused = ready(&wallet).send;
        assert_eq!(refused.state, "failed");
        assert!(
            refused
                .message
                .unwrap()
                .contains("hasn't published a way to be paid")
        );

        // People paid by npub are remembered across launches.
        let again = Wallet::new(home.path().to_path_buf(), spark_opener());
        assert_eq!(ready(&again).people.len(), 1);
    }

    #[test]
    fn a_lightning_address_paid_can_be_saved_as_a_contact() {
        let home = tempfile::tempdir().expect("temp dir");
        let node = Arc::new(Fake::default());
        node.balance.store(90_000, Ordering::SeqCst);
        let mut wallet = Wallet::new(
            home.path().to_path_buf(),
            opener(node.clone(), Arc::new(Mutex::new(vec![]))),
        );
        wallet.open(ENTROPY, false);
        settle(&wallet);
        wallet.quote("alice@example.com", "2000", "");
        settle(&wallet);
        let quote = ready(&wallet).send.quote.expect("quote");
        wallet.pay(quote.id);
        settle(&wallet);
        assert_eq!(
            ready(&wallet).send.save_suggestion.as_deref(),
            Some("alice@example.com")
        );
        wallet.save_contact("", "alice@example.com");
        wallet.save_contact("Alice", "not an address");
        settle(&wallet);
        assert!(node.contacts.lock().unwrap().is_empty());
        wallet.save_contact(" Alice ", "alice@example.com");
        settle(&wallet);
        let saved = ready(&wallet);
        assert_eq!(saved.send.save_suggestion, None);
        assert_eq!(saved.people.len(), 1);
        assert_eq!(
            (
                saved.people[0].name.as_str(),
                saved.people[0].input.as_str()
            ),
            ("Alice", "alice@example.com")
        );
        // Paying a contact again suggests nothing.
        wallet.reset_send();
        wallet.quote("alice@example.com", "2000", "");
        settle(&wallet);
        let quote = ready(&wallet).send.quote.expect("quote");
        wallet.pay(quote.id);
        settle(&wallet);
        assert_eq!(ready(&wallet).send.save_suggestion, None);
    }

    #[test]
    fn the_spark_address_is_published_only_by_the_setting_and_can_be_removed() {
        let home = tempfile::tempdir().expect("temp dir");
        let node = Arc::new(Fake::default());
        let profiles = Arc::new(Profiles::default());
        let mut wallet = Wallet::new(
            home.path().to_path_buf(),
            opener(node.clone(), Arc::new(Mutex::new(vec![]))),
        )
        .with_directory(profiles.clone(), "npub1me".into());
        let before = ready(&wallet).receive;
        assert_eq!(
            before.nostr.as_ref().map(|code| code.uri.as_str()),
            Some("nostr:npub1me")
        );
        assert!(!before.publish.as_ref().unwrap().on);
        // Nothing is published before the wallet runs, or without the tap.
        wallet.publish(true);
        assert!(profiles.published.lock().unwrap().is_empty());
        wallet.open(ENTROPY, false);
        settle(&wallet);
        assert!(profiles.published.lock().unwrap().is_empty());
        wallet.publish(true);
        settle(&wallet);
        let on = ready(&wallet).receive;
        assert!(on.publish.as_ref().unwrap().on);
        assert_eq!(
            on.publish.unwrap().message.as_deref(),
            Some("Published on 3 relays.")
        );
        assert!(
            on.nostr
                .unwrap()
                .caption
                .contains("published Spark address")
        );
        // It stays on across launches.
        let again = Wallet::new(home.path().to_path_buf(), spark_opener())
            .with_directory(profiles.clone(), "npub1me".into());
        assert!(ready(&again).receive.publish.unwrap().on);
        wallet.publish(false);
        settle(&wallet);
        assert!(!ready(&wallet).receive.publish.unwrap().on);
        assert_eq!(
            *profiles.published.lock().unwrap(),
            vec![Some("spark1fakeaddress".to_string()), None]
        );
    }

    #[test]
    fn each_deposit_state_reads_plainly_and_only_open_ones_can_be_acted_on() {
        let home = tempfile::tempdir().expect("temp dir");
        let node = Arc::new(Fake::default());
        *node.deposits.lock().unwrap() = vec![
            deposit("aa", false, None, None),
            deposit("bb", true, None, None),
            deposit("cc", true, Some(DepositProblem::FeeAboveLimit(1_200)), None),
            deposit("dd", true, Some(DepositProblem::Missing), None),
            deposit(
                "ee",
                true,
                Some(DepositProblem::Failed("timeout".into())),
                None,
            ),
            deposit("ff", true, None, Some(&"12".repeat(32))),
        ];
        let mut wallet = Wallet::new(
            home.path().to_path_buf(),
            opener(node, Arc::new(Mutex::new(vec![]))),
        );
        wallet.open(ENTROPY, false);
        settle(&wallet);
        let rows: Vec<(String, bool)> = ready(&wallet)
            .deposits
            .into_iter()
            .map(|row| (row.status, row.actionable))
            .collect();
        assert_eq!(
            rows,
            vec![
                ("Waiting for 3 confirmations.".into(), true),
                ("Confirmed; the wallet is claiming it.".into(), true),
                (
                    "Claiming it costs ₿1,200, above the automatic limit. Claim it at a quoted fee, or refund it on-chain.".into(),
                    true
                ),
                (
                    "The deposit wasn't found on the chain. Claim it at a quoted fee, or refund it on-chain.".into(),
                    true
                ),
                (
                    "The last claim failed (timeout). Claim it at a quoted fee, or refund it on-chain.".into(),
                    true
                ),
                (
                    "Refund sent in transaction 12121212121212…1212121212. It leaves the wallet once it confirms.".into(),
                    false
                ),
            ]
        );
        // A deposit already refunded can't start another refund.
        wallet.refund_start("ff", 0);
        assert!(ready(&wallet).refund.is_none());
    }

    #[test]
    fn a_refund_is_sent_once_to_a_reviewed_address_at_a_chosen_speed() {
        let home = tempfile::tempdir().expect("temp dir");
        let node = Arc::new(Fake::default());
        *node.deposits.lock().unwrap() = vec![deposit(
            "cc",
            true,
            Some(DepositProblem::Failed("too costly".into())),
            None,
        )];
        let mut wallet = Wallet::new(
            home.path().to_path_buf(),
            opener(node.clone(), Arc::new(Mutex::new(vec![]))),
        );
        wallet.open(ENTROPY, false);
        settle(&wallet);
        // Nothing to review before the rates are read.
        wallet.refund_review(
            "cc",
            0,
            "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq",
            "slow",
        );
        assert!(ready(&wallet).refund.is_none());
        wallet.refund_start("cc", 0);
        settle(&wallet);
        let started = ready(&wallet).refund.expect("refund");
        let speeds: Vec<(&str, String)> = started
            .speeds
            .iter()
            .map(|speed| (speed.id, speed.fee.clone()))
            .collect();
        // A zero rate is raised to 1 sat/vB.
        assert_eq!(
            speeds,
            vec![
                ("slow", "about ₿111 at 1 sat/vB".to_string()),
                ("medium", "about ₿888 at 8 sat/vB".to_string()),
                ("fast", "about ₿2,220 at 20 sat/vB".to_string()),
            ]
        );
        for (address, expected) in [
            ("not an address", "Enter a Bitcoin address to refund to."),
            (
                "bitcoin:bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq",
                "Enter a Bitcoin address to refund to.",
            ),
            ("bc1qfakedeposit", "Enter a Bitcoin address to refund to."),
        ] {
            wallet.refund_review("cc", 0, address, "fast");
            let refund = ready(&wallet).refund.expect("refund");
            assert_eq!(refund.message.as_deref(), Some(expected), "{address}");
            assert_eq!(refund.review, None);
        }
        wallet.refund_review(
            "cc",
            0,
            "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq",
            "warp",
        );
        assert!(
            ready(&wallet)
                .refund
                .unwrap()
                .message
                .unwrap()
                .contains("how fast")
        );
        // Refunding without a review sends nothing.
        wallet.refund("cc", 0);
        assert!(node.refunds.lock().unwrap().is_empty());
        wallet.refund_review(
            "cc",
            0,
            " bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq ",
            "medium",
        );
        let reviewed = ready(&wallet).refund.expect("refund");
        assert_eq!(
            reviewed.review.as_deref(),
            Some("Refund ₿39,112 to bc1qar0srrr7xf…gtzzwf5mdq; about ₿888 fee at 8 sat/vB.")
        );
        assert!(
            reviewed
                .speeds
                .iter()
                .any(|speed| speed.id == "medium" && speed.chosen)
        );
        wallet.refund("cc", 0);
        wallet.refund("cc", 0);
        settle(&wallet);
        assert_eq!(
            *node.refunds.lock().unwrap(),
            vec![(
                "cc".to_string(),
                0,
                "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq".to_string(),
                8
            )]
        );
        let done = ready(&wallet);
        assert!(
            done.refund
                .unwrap()
                .message
                .unwrap()
                .starts_with("Refund sent to bc1q")
        );
        assert!(!done.deposits[0].actionable);
        wallet.refund_reset();
        assert!(ready(&wallet).refund.is_none());

        // A fee that would take the whole deposit is refused.
        *node.deposits.lock().unwrap() = vec![DepositRow {
            amount_sats: 500,
            ..deposit("small", true, None, None)
        }];
        wallet.refresh();
        settle(&wallet);
        wallet.refund_start("small", 0);
        settle(&wallet);
        wallet.refund_review(
            "small",
            0,
            "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq",
            "fast",
        );
        assert!(
            ready(&wallet)
                .refund
                .unwrap()
                .message
                .unwrap()
                .contains("take the whole deposit")
        );
    }

    #[test]
    fn an_onchain_withdrawal_shows_each_speed_and_pays_the_chosen_one() {
        let home = tempfile::tempdir().expect("temp dir");
        let node = Arc::new(Fake::default());
        node.balance.store(90_000, Ordering::SeqCst);
        let mut wallet = Wallet::new(
            home.path().to_path_buf(),
            opener(node.clone(), Arc::new(Mutex::new(vec![]))),
        );
        wallet.open(ENTROPY, false);
        settle(&wallet);
        wallet.quote("bc1qfriend", "50000", "");
        settle(&wallet);
        let quote = ready(&wallet).send.quote.expect("quote");
        let speeds: Vec<(&str, &str, bool)> = quote
            .speeds
            .iter()
            .map(|speed| (speed.id, speed.fee.as_str(), speed.chosen))
            .collect();
        assert_eq!(
            speeds,
            vec![
                ("slow", "₿250", false),
                ("medium", "₿400", true),
                ("fast", "₿900", false)
            ]
        );
        wallet.speed(quote.id, "fast");
        wallet.speed(quote.id, "warp");
        wallet.speed(999, "slow");
        let fast = ready(&wallet).send.quote.expect("quote");
        assert_eq!(
            (fast.fee.as_str(), fast.total.as_str()),
            ("₿900", "₿50,900")
        );
        assert_eq!(*node.speeds.lock().unwrap(), vec![(10, Speed::Fast)]);
        wallet.pay(fast.id);
        settle(&wallet);
        assert_eq!(node.paid.lock().unwrap()[0].0, 10);
        // A Lightning quote has no speeds.
        wallet.reset_send();
        wallet.quote("lnbc-with-amount", "", "");
        settle(&wallet);
        assert!(ready(&wallet).send.quote.unwrap().speeds.is_empty());
    }

    #[test]
    fn the_exit_state_is_saved_after_each_sync_and_exported_on_request() {
        let home = tempfile::tempdir().expect("temp dir");
        let node = Arc::new(Fake::default());
        *node.exit.lock().unwrap() = r#"{"version":2,"pedigrees":["a"]}"#.into();
        let vault = Arc::new(MemoryVault::default());
        let mut wallet = Wallet::new(
            home.path().to_path_buf(),
            opener(node.clone(), Arc::new(Mutex::new(vec![]))),
        )
        .with_vault(vault.clone());
        assert!(!ready(&wallet).backup.can_export);
        assert!(wallet.exit_export().is_err());
        wallet.open(ENTROPY, false);
        settle(&wallet);
        let backup = ready(&wallet).backup;
        assert!(backup.can_export && backup.saved_at.is_some() && backup.error.is_none());
        let (name, text) = wallet.exit_export().expect("export");
        assert!(name.starts_with("openagents-spark-exit-") && name.ends_with(".json"));
        assert_eq!(text, r#"{"version":2,"pedigrees":["a"]}"#);
        // An unchanged state is not written again; a changed one is.
        wallet.refresh();
        settle(&wallet);
        assert_eq!(vault.saves.load(Ordering::SeqCst), 1);
        *node.exit.lock().unwrap() = r#"{"version":2,"pedigrees":["a","b"]}"#.into();
        wallet.refresh();
        settle(&wallet);
        assert_eq!(vault.saves.load(Ordering::SeqCst), 2);
        assert!(wallet.exit_export().unwrap().1.contains("\"b\""));
        // The state never reaches the app packet.
        let json = serde_json::to_string(&wallet.screen()).unwrap();
        assert!(!json.contains("pedigrees"), "{json}");
        // A new lifetime shows when it was saved before the wallet starts.
        let again =
            Wallet::new(home.path().to_path_buf(), spark_opener()).with_vault(vault.clone());
        assert!(ready(&again).backup.can_export);
        // Replacing the wallet forgets the old one's exit state.
        wallet.open(&hex(&[7; 32]), true);
        assert!(vault.load().is_none());
        assert!(!ready(&wallet).backup.can_export);
    }

    fn opener(node: Arc<Fake>, seen: Arc<Mutex<Vec<String>>>) -> Opener {
        Arc::new(move |_home, mnemonic| {
            seen.lock().unwrap().push(mnemonic.to_owned());
            Ok(node.clone() as Arc<dyn Node>)
        })
    }

    fn settle(wallet: &Wallet) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while wallet.loading() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!wallet.loading(), "the wallet settled");
    }

    fn ready(wallet: &Wallet) -> Summary {
        match wallet.screen() {
            Screen::Ready(summary) => *summary,
            other => panic!("ready: {other:?}"),
        }
    }

    #[test]
    fn the_phone_wallet_runs_on_mainnet_with_the_committed_key() {
        assert_eq!(NETWORK, Network::Mainnet);
        let config = crate::spark::sdk_config(NETWORK);
        assert_eq!(config.network, Network::Mainnet);
        assert_eq!(config.api_key.as_deref(), Some(crate::spark::BREEZ_API_KEY));
        // The key is Breez's validation certificate, not a spending secret.
        assert!(crate::spark::BREEZ_API_KEY.starts_with("MIIB"));
        assert_eq!(config.real_time_sync_server_url, None);
        assert!(config.cross_chain_config.is_none());
        assert!(config.stable_balance_config.is_none());
    }

    #[test]
    fn a_key_opens_the_wallet_and_the_screen_shows_what_it_read() {
        let home = tempfile::tempdir().expect("temp dir");
        let node = Arc::new(Fake::default());
        let seen = Arc::new(Mutex::new(vec![]));
        let mut wallet = Wallet::new(
            home.path().join("spark"),
            opener(node.clone(), seen.clone()),
        );
        // Before the key arrives the screen is already the wallet's, with
        // placeholders where nothing has been read yet.
        let opening = ready(&wallet);
        assert!(opening.balance_unknown && opening.receive.spark.is_none());
        assert_eq!(opening.status.as_deref(), Some("Opening the wallet…"));
        assert!(!opening.trust.acknowledged && opening.trust.lines.len() == 5);
        assert!(!opening.can_show_words);
        wallet.open(ENTROPY, false);
        settle(&wallet);
        let empty = ready(&wallet);
        assert!(empty.empty);
        assert_eq!(empty.balance, "₿0");
        assert_eq!(empty.balance_alternate, "0.00000000 BTC");
        assert_eq!(empty.network, "Bitcoin · Spark");
        let spark = empty.receive.spark.expect("spark address");
        assert_eq!(spark.text, "spark1fakeaddress");
        assert!(spark.qr.is_some());
        let bitcoin = empty.receive.bitcoin.expect("bitcoin address");
        assert_eq!(bitcoin.uri, "bitcoin:bc1qfakedeposit");
        assert_eq!(empty.error, None);
        assert!(empty.can_show_words);
        let mnemonic = seen.lock().unwrap()[0].clone();
        assert_eq!(mnemonic.split_whitespace().count(), 12);

        // A received payment reaches the screen through the SDK's event.
        node.balance.store(123_456, Ordering::SeqCst);
        node.payments.lock().unwrap().push(PaymentRow {
            id: "in-1".into(),
            received: true,
            amount_sats: 123_456,
            fee_sats: 0,
            method: "Lightning".into(),
            status: "completed".into(),
            at: 1_790_000_000,
        });
        (node.notify.lock().unwrap().clone().expect("subscribed"))();
        let deadline = Instant::now() + Duration::from_secs(10);
        while ready(&wallet).balance_sats != 123_456 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        let funded = ready(&wallet);
        assert_eq!(funded.balance, "₿123,456");
        assert_eq!(funded.balance_alternate, "0.00123456 BTC");
        assert_eq!(funded.balance_spoken, "123,456 bitcoin");
        assert_eq!(funded.payments.len(), 1);
        assert_eq!(funded.payments[0].amount, "+₿123,456");
        assert_eq!(funded.payments[0].title, "Received");
        assert_eq!(funded.warning, None);

        // A new lifetime shows the cached balance, addresses, and history
        // before its wallet starts.
        let again = Wallet::new(home.path().join("spark"), opener(node, seen));
        let cached = ready(&again);
        assert_eq!(cached.balance, "₿123,456");
        assert!(!cached.balance_unknown);
        assert_eq!(
            cached.receive.spark.map(|code| code.text).as_deref(),
            Some("spark1fakeaddress")
        );
        assert_eq!(cached.payments.len(), 1);
        assert_eq!(cached.status.as_deref(), Some("Opening the wallet…"));
    }

    #[test]
    fn receive_makes_lightning_invoices_with_and_without_an_amount() {
        let home = tempfile::tempdir().expect("temp dir");
        let node = Arc::new(Fake::default());
        let mut wallet = Wallet::new(
            home.path().to_path_buf(),
            opener(node.clone(), Arc::new(Mutex::new(vec![]))),
        );
        wallet.invoice("");
        assert_eq!(
            ready(&wallet).receive.lightning_error.as_deref(),
            Some("The wallet is still starting.")
        );
        wallet.open(ENTROPY, false);
        settle(&wallet);
        wallet.invoice("");
        settle(&wallet);
        let any = ready(&wallet).receive.lightning.expect("invoice");
        assert_eq!(any.caption, "Request for any amount");
        assert!(any.uri.starts_with("lightning:lnbc"));
        assert!(any.qr.is_some());
        wallet.invoice("2,500");
        settle(&wallet);
        let fixed = ready(&wallet).receive.lightning.expect("invoice");
        assert_eq!(fixed.caption, "Request for ₿2,500");
        assert!(fixed.text.ends_with("2500"));
        wallet.invoice("a lot");
        assert!(ready(&wallet).receive.lightning_error.is_some());
        assert_eq!(node.invoices.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn send_quotes_the_fee_and_pays_only_the_confirmed_quote_once() {
        let home = tempfile::tempdir().expect("temp dir");
        let node = Arc::new(Fake::default());
        node.balance.store(50_000, Ordering::SeqCst);
        let mut wallet = Wallet::new(
            home.path().to_path_buf(),
            opener(node.clone(), Arc::new(Mutex::new(vec![]))),
        );
        wallet.open(ENTROPY, false);
        settle(&wallet);

        wallet.quote("not a request", "", "");
        settle(&wallet);
        let refused = ready(&wallet).send;
        assert_eq!(refused.state, "failed");
        assert_eq!(
            refused.message.as_deref(),
            Some("That isn't a payment request.")
        );

        // An address needs an amount; the screen asks for it.
        wallet.quote("spark1friend", "", "");
        settle(&wallet);
        assert_eq!(ready(&wallet).send.state, "needs_amount");
        wallet.quote("spark1friend", "0", "");
        assert_eq!(
            ready(&wallet).send.message.as_deref(),
            Some("Enter an amount above zero.")
        );
        wallet.quote("spark1friend", "1,500", "");
        settle(&wallet);
        let spark = ready(&wallet).send.quote.expect("quote");
        assert_eq!(
            (spark.kind, spark.amount.as_str(), spark.fee.as_str()),
            ("Spark address", "₿1,500", "₿0")
        );

        wallet.quote("lnbc-with-amount", "", "");
        settle(&wallet);
        let quoted = ready(&wallet).send;
        assert_eq!(quoted.state, "quoted");
        let quote = quoted.quote.expect("quote");
        assert_eq!(quote.kind, "Lightning invoice");
        assert_eq!(quote.to, "Payment request");
        assert_eq!(
            (
                quote.amount.as_str(),
                quote.fee.as_str(),
                quote.total.as_str()
            ),
            ("₿1,000", "₿3", "₿1,003")
        );
        assert_eq!(quote.note.as_deref(), Some("Coffee"));

        // A stale quote ID pays nothing; the one on screen pays once.
        wallet.pay(8);
        assert!(node.paid.lock().unwrap().is_empty());
        wallet.pay(quote.id);
        wallet.pay(quote.id);
        settle(&wallet);
        let paid = node.paid.lock().unwrap().clone();
        assert_eq!(paid.len(), 1);
        assert_eq!(paid[0].0, 7);
        assert!(uuid::Uuid::parse_str(&paid[0].1).is_ok(), "a UUID key");
        let sent = ready(&wallet);
        assert_eq!(sent.send.state, "sent");
        assert_eq!(sent.send.message.as_deref(), Some("Sent."));
        assert_eq!(
            sent.send.result.map(|row| row.amount).as_deref(),
            Some("-₿1,000")
        );
        assert_eq!(sent.balance, "₿48,997");
        assert_eq!(sent.payments[0].fee.as_deref(), Some("₿3 fee"));
        wallet.reset_send();
        assert_eq!(ready(&wallet).send.state, "idle");
    }

    #[test]
    fn lightning_addresses_and_lnurl_codes_quote_within_the_recipients_terms() {
        let home = tempfile::tempdir().expect("temp dir");
        let node = Arc::new(Fake::default());
        node.balance.store(50_000, Ordering::SeqCst);
        let mut wallet = Wallet::new(
            home.path().to_path_buf(),
            opener(node.clone(), Arc::new(Mutex::new(vec![]))),
        );
        wallet.open(ENTROPY, false);
        settle(&wallet);

        // Without an amount the screen shows the recipient, its range, its
        // description, and a comment field sized to what it takes.
        wallet.quote("alice@example.com", "", "");
        settle(&wallet);
        let asked = ready(&wallet).send;
        assert_eq!(asked.state, "needs_amount");
        assert_eq!(
            asked.message.as_deref(),
            Some("Enter an amount. alice@example.com takes from ₿1 to ₿5,000.")
        );
        assert_eq!(asked.recipient.as_deref(), Some("alice@example.com"));
        assert_eq!(asked.description.as_deref(), Some("Sats for Alice"));
        assert_eq!(asked.comment_max, Some(20));

        // Out of range, or too long a comment, asks again.
        wallet.quote("alice@example.com", "6000", "");
        settle(&wallet);
        assert_eq!(
            ready(&wallet).send.message.as_deref(),
            Some("alice@example.com takes from ₿1 to ₿5,000.")
        );
        wallet.quote("alice@example.com", "2000", &"x".repeat(21));
        settle(&wallet);
        assert_eq!(
            ready(&wallet).send.message.as_deref(),
            Some("alice@example.com takes a comment of up to 20 characters.")
        );
        // An unreadable amount keeps what the screen knew of the recipient.
        wallet.quote("alice@example.com", "lots", "");
        let kept = ready(&wallet).send;
        assert_eq!(kept.recipient.as_deref(), Some("alice@example.com"));
        assert_eq!(kept.comment_max, Some(20));

        wallet.quote("alice@example.com", "2,000", "  for lunch ");
        settle(&wallet);
        let quote = ready(&wallet).send.quote.expect("quote");
        assert_eq!(quote.kind, "Lightning address");
        assert_eq!(quote.to, "Address");
        assert_eq!(quote.destination, "alice@example.com");
        assert_eq!(
            (
                quote.amount.as_str(),
                quote.fee.as_str(),
                quote.total.as_str()
            ),
            ("₿2,000", "₿2", "₿2,002")
        );
        assert_eq!(quote.note.as_deref(), Some("Sats for Alice"));
        assert_eq!(quote.comment.as_deref(), Some("for lunch"));
        wallet.pay(quote.id);
        settle(&wallet);
        let sent = ready(&wallet).send;
        assert_eq!(sent.state, "sent");
        assert_eq!(
            sent.recipient_message.as_deref(),
            Some("Thanks for the sats!")
        );
        assert_eq!(node.paid.lock().unwrap().len(), 1);
        wallet.reset_send();

        // A code that takes one amount is quoted for it without asking, and
        // a comment it doesn't take is not sent.
        wallet.quote("lnurl1fixedprice", "", "hello");
        settle(&wallet);
        let fixed = ready(&wallet).send.quote.expect("quote");
        assert_eq!(fixed.amount, "₿21,000");
        assert_eq!(fixed.destination, "shop.example");
        assert_eq!(fixed.comment, None);
    }

    #[test]
    fn lnurl_terms_read_ranges_comments_and_metadata() {
        let terms = LnurlTerms::of("bob@example.com".into(), 1_500, 2_999, 0, "not json");
        // Millisats round inward: at least ₿2, at most ₿2.
        assert_eq!((terms.min_sats, terms.max_sats), (2, 2));
        assert_eq!(terms.description, None);
        assert_eq!(terms.check(None, None, Format::Bip177), Ok((2, None)));
        // What it asks is worded in the person's format.
        let range = LnurlTerms::of("bob@example.com".into(), 1_000, 100_000_000_000, 0, "[]");
        let asked = |format| match range.check(None, None, format) {
            Err(QuoteFailure::NeedsAmount(ask)) => ask.message,
            other => panic!("{other:?}"),
        };
        assert_eq!(
            asked(Format::Bip177),
            "Enter an amount. bob@example.com takes from ₿1 to ₿100,000,000."
        );
        assert_eq!(
            asked(Format::LegacyBtc),
            "Enter an amount. bob@example.com takes from 0.00000001 BTC to 1.00000000 BTC."
        );
        let closed = LnurlTerms::of("bob@example.com".into(), 1_000, 0, 0, "[]");
        assert_eq!(
            closed.check(Some(10), None, Format::Bip177),
            Err(QuoteFailure::Refused(
                "bob@example.com isn't taking payments right now.".into()
            ))
        );
        let inverted = LnurlTerms::of("bob@example.com".into(), 9_000, 1_000, 0, "[]");
        assert!(matches!(
            inverted.check(Some(5), None, Format::Bip177),
            Err(QuoteFailure::Refused(_))
        ));
        assert_eq!(
            lnurl_description(
                r#"[["text/plain","Pay\n  Bob\u0007 here"],["image/png;base64","AA"]]"#
            )
            .as_deref(),
            Some("Pay Bob here")
        );
        assert_eq!(lnurl_description(r#"[["text/plain","   "]]"#), None);
        assert_eq!(plain_text(&"ab".repeat(10), 5).as_deref(), Some("ababa…"));
        // Comments count characters, not bytes.
        let terms = LnurlTerms::of("c@example.com".into(), 1_000, 10_000, 3, "[]");
        assert_eq!(
            terms.check(Some(5), Some("éèê"), Format::Bip177),
            Ok((5, Some("éèê".into())))
        );
    }

    #[test]
    fn failures_are_typed_and_never_carry_the_key() {
        let home = tempfile::tempdir().expect("temp dir");
        let mut wallet = Wallet::new(
            home.path().to_path_buf(),
            Arc::new(|_, _| Err("The wallet could not reach Spark (timeout).".to_string())),
        );
        wallet.open("zz", false);
        assert_eq!(
            wallet.screen(),
            Screen::Failed {
                message: "The wallet key is unreadable.".into()
            }
        );
        // 20 bytes is not a BIP39 wallet this app makes.
        wallet.open(&"ab".repeat(20), false);
        assert!(matches!(wallet.screen(), Screen::Failed { .. }));
        wallet.open(ENTROPY, false);
        settle(&wallet);
        assert_eq!(
            wallet.screen(),
            Screen::Failed {
                message: "The wallet could not reach Spark (timeout).".into()
            }
        );

        // A wallet that starts but cannot sync shows what it has, and why.
        let node = Arc::new(Fake::default());
        node.balance.store(5, Ordering::SeqCst);
        node.sync_fails.store(true, Ordering::SeqCst);
        let mut syncless = Wallet::new(
            home.path().join("syncless"),
            opener(node, Arc::new(Mutex::new(vec![]))),
        );
        syncless.open(ENTROPY, false);
        settle(&syncless);
        let summary = ready(&syncless);
        assert!(summary.balance_unknown);
        assert!(summary.error.as_deref().unwrap().contains("Spark"));
        let other = ENTROPY.replace('5', "6");
        syncless.open(&other, false);
        assert!(
            ready(&syncless)
                .error
                .is_some_and(|error| error.contains("different key"))
        );

        let entropy = decode(ENTROPY).unwrap();
        let mnemonic = bip39::Mnemonic::from_entropy(&entropy).unwrap().to_string();
        let first_word = mnemonic.split_whitespace().next().unwrap().to_owned();
        for screen in [wallet.screen(), syncless.screen()] {
            let json = serde_json::to_string(&screen).unwrap();
            assert!(
                !json.contains(ENTROPY) && !json.contains(&mnemonic),
                "{json}"
            );
            assert!(!json.contains(&format!("\"{first_word}")), "{json}");
        }
        // Only the explicit request returns the words.
        assert_eq!(syncless.words().unwrap().join(" "), mnemonic);
    }

    #[test]
    fn recovery_words_restore_and_replace_the_wallet() {
        let entropy = decode(ENTROPY).unwrap();
        let mnemonic = bip39::Mnemonic::from_entropy(&entropy).unwrap().to_string();
        assert_eq!(restore_entropy(&mnemonic).unwrap(), ENTROPY);
        // Case and spacing don't matter.
        let shouted = format!("  {}  ", mnemonic.to_uppercase().replace(' ', "   "));
        assert_eq!(restore_entropy(&shouted).unwrap(), ENTROPY);
        let long = hex(&[7; 32]);
        let long_words = bip39::Mnemonic::from_entropy(&[7; 32]).unwrap().to_string();
        assert_eq!(restore_entropy(&long_words).unwrap(), long);

        let words: Vec<&str> = mnemonic.split_whitespace().collect();
        let short = words[..11].join(" ");
        assert_eq!(
            restore_entropy(&short).unwrap_err(),
            "Enter 12 or 24 recovery words; that was 11."
        );
        let mut misspelled = words.clone();
        misspelled[3] = "notaword";
        let error = restore_entropy(&misspelled.join(" ")).unwrap_err();
        assert_eq!(error, "Word 4 isn't a recovery word. Check its spelling.");
        let mut swapped = words.clone();
        swapped.swap(0, 11);
        if swapped != words {
            let error = restore_entropy(&swapped.join(" ")).unwrap_err();
            assert!(!error.contains(words[0]), "{error}");
        }

        // Replacing stops the old wallet and forgets its cached state.
        let home = tempfile::tempdir().expect("temp dir");
        let node = Arc::new(Fake::default());
        node.balance.store(900, Ordering::SeqCst);
        let seen = Arc::new(Mutex::new(vec![]));
        let mut wallet = Wallet::new(home.path().to_path_buf(), opener(node, seen.clone()));
        wallet.open(ENTROPY, false);
        settle(&wallet);
        assert_eq!(ready(&wallet).balance, "₿900");
        wallet.open(&long, true);
        assert!(ready(&wallet).balance_unknown, "the old balance is gone");
        assert!(!home.path().join(BALANCE_FILE).exists());
        settle(&wallet);
        assert_eq!(seen.lock().unwrap().len(), 2);
        assert_eq!(seen.lock().unwrap()[1].split_whitespace().count(), 24);
        assert_eq!(wallet.words().unwrap().len(), 24);
    }

    #[test]
    fn the_main_screen_is_plain_and_the_rest_waits_under_advanced() {
        let home = tempfile::tempdir().expect("temp dir");
        let node = Arc::new(Fake::default());
        for index in 0..7 {
            node.payments.lock().unwrap().push(PaymentRow {
                id: format!("in-{index}"),
                received: true,
                amount_sats: 1_000,
                fee_sats: 0,
                method: "Lightning".into(),
                status: "completed".into(),
                at: 1_790_000_000 + index,
            });
        }
        let mut wallet = Wallet::new(
            home.path().to_path_buf(),
            opener(node.clone(), Arc::new(Mutex::new(vec![]))),
        );
        // Before the key arrives there is nothing to back up yet, and the
        // Advanced section starts closed.
        let opening = ready(&wallet);
        assert!(opening.backup_card.is_none());
        assert!(opening.stale);
        assert_eq!(
            opening.advanced,
            AdvancedView {
                open: false,
                note: None
            }
        );
        wallet.open(ENTROPY, false);
        settle(&wallet);
        let shown = ready(&wallet);
        // A fresh balance shows no time; recent activity is the newest few.
        assert!(!shown.stale);
        assert_eq!(shown.recent.len(), RECENT_LIMIT);
        assert_eq!(shown.payments.len(), 7);
        assert!(shown.more_payments);
        assert_eq!(shown.recent[0], shown.payments[0]);
        // Receiving is a plain request, and its caption has no jargon.
        wallet.invoice("");
        settle(&wallet);
        let request = ready(&wallet).receive.lightning.expect("request");
        for word in ["Lightning", "invoice", "Spark", "sat", "on-chain"] {
            assert!(!request.caption.contains(word), "{}", request.caption);
        }
        assert!(!TRUST_SUMMARY.contains("Spark") && !TRUST_SUMMARY.contains("Lightning"));

        // The backup card shows until the person saves the words of this
        // wallet, and stays gone across launches.
        let card = shown.backup_card.expect("backup card");
        assert_eq!(card.title, "Back up your wallet");
        wallet.words_saved();
        assert!(ready(&wallet).backup_card.is_none());
        let mut again = Wallet::new(
            home.path().to_path_buf(),
            opener(node.clone(), Arc::new(Mutex::new(vec![]))),
        );
        again.open(ENTROPY, false);
        assert!(ready(&again).backup_card.is_none());
        // The marker holds a fingerprint, never the seed or its words.
        let marker = std::fs::read_to_string(home.path().join(WORDS_SAVED_FILE)).expect("marker");
        assert!(!marker.contains(ENTROPY));
        let words = again.words().expect("words");
        assert!(!words.iter().any(|word| marker.contains(word.as_str())));
        // Another wallet on this phone asks again; restoring one from its
        // words counts as saved.
        let other = Wallet::new(
            home.path().to_path_buf(),
            opener(node.clone(), Arc::new(Mutex::new(vec![]))),
        );
        let mut other = other;
        other.open(&hex(&[9; 16]), false);
        assert!(ready(&other).backup_card.is_some());
        other.open(&hex(&[8; 16]), true);
        assert!(ready(&other).backup_card.is_none());

        // Deposits wait under Advanced, with a note while it is closed.
        node.deposits
            .lock()
            .unwrap()
            .push(deposit("arriving", false, None, None));
        again.refresh();
        settle(&again);
        assert_eq!(
            ready(&again).advanced.note.as_deref(),
            Some("A deposit is on its way")
        );
        node.deposits.lock().unwrap().push(deposit(
            "stuck",
            true,
            Some(DepositProblem::Missing),
            None,
        ));
        again.refresh();
        settle(&again);
        assert_eq!(
            ready(&again).advanced.note.as_deref(),
            Some("A deposit needs you")
        );
        // Opening Advanced is remembered on this phone.
        again.set_advanced(true);
        assert!(ready(&again).advanced.open);
        let relaunched = Wallet::new(home.path().to_path_buf(), spark_opener());
        assert!(ready(&relaunched).advanced.open);
        again.set_advanced(false);
        assert!(
            !ready(&Wallet::new(home.path().to_path_buf(), spark_opener()))
                .advanced
                .open
        );
    }

    #[test]
    fn the_trust_note_shows_until_read_and_large_balances_warn() {
        let home = tempfile::tempdir().expect("temp dir");
        let node = Arc::new(Fake::default());
        node.balance
            .store(BALANCE_WARNING_SATS + 1, Ordering::SeqCst);
        let mut wallet = Wallet::new(
            home.path().to_path_buf(),
            opener(node, Arc::new(Mutex::new(vec![]))),
        );
        assert!(!ready(&wallet).trust.acknowledged);
        wallet.acknowledge();
        assert!(ready(&wallet).trust.acknowledged);
        // It stays read across launches.
        let relaunched = Wallet::new(home.path().to_path_buf(), spark_opener());
        assert!(ready(&relaunched).trust.acknowledged);
        wallet.open(ENTROPY, false);
        settle(&wallet);
        let warned = ready(&wallet);
        assert!(
            warned
                .warning
                .is_some_and(|warning| warning.contains("₿1,000,000"))
        );
    }

    #[test]
    fn buying_opens_the_provider_page_once_and_deposits_are_claimed_at_the_quoted_fee() {
        let home = tempfile::tempdir().expect("temp dir");
        let node = Arc::new(Fake::default());
        node.deposits.lock().unwrap().push(DepositRow {
            txid: "ab".repeat(32),
            vout: 1,
            amount_sats: 50_000,
            mature: true,
            problem: Some(DepositProblem::FeeAboveLimit(1_200)),
            refund_txid: None,
        });
        let mut wallet = Wallet::new(
            home.path().to_path_buf(),
            opener(node.clone(), Arc::new(Mutex::new(vec![]))),
        );
        wallet.open(ENTROPY, false);
        settle(&wallet);
        let summary = ready(&wallet);
        assert_eq!(summary.buy.providers.len(), 2);
        assert_eq!(summary.deposits.len(), 1);
        assert_eq!(summary.deposits[0].amount, "₿50,000");
        assert_eq!(
            summary.deposits[0].status,
            "Claiming it costs ₿1,200, above the automatic limit. Claim it at a quoted fee, or refund it on-chain."
        );

        wallet.buy("moonpay", "");
        assert_eq!(
            ready(&wallet).buy.error.as_deref(),
            Some("Enter how much bitcoin to buy.")
        );
        wallet.buy("paypal", "1000");
        assert!(ready(&wallet).buy.error.is_some());
        wallet.buy("moonpay", "50,000");
        settle(&wallet);
        assert_eq!(
            wallet.take_open_url().as_deref(),
            Some("https://buy.moonpay.com/?amount=50000")
        );
        assert_eq!(wallet.take_open_url(), None, "it opens once");
        wallet.buy("cashapp", "20000");
        settle(&wallet);
        assert!(
            wallet
                .take_open_url()
                .is_some_and(|url| url.starts_with("https://cash.app/"))
        );
        // A link that isn't a web page never reaches the host.
        wallet.buy("cashapp", "666");
        settle(&wallet);
        assert_eq!(wallet.take_open_url(), None);
        assert!(ready(&wallet).buy.error.is_some());

        // A claim is sent only after its quote, with the quoted fee as the
        // ceiling.
        let txid = "ab".repeat(32);
        wallet.claim(&txid, 1);
        assert!(node.claims.lock().unwrap().is_empty());
        wallet.claim_quote("unknown", 0);
        assert!(ready(&wallet).claim.is_none());
        wallet.claim_quote(&txid, 1);
        settle(&wallet);
        assert_eq!(
            ready(&wallet)
                .claim
                .and_then(|claim| claim.quote)
                .as_deref(),
            Some("Claim for a ₿1,200 fee; ₿48,800 reaches your balance.")
        );
        wallet.claim(&txid, 1);
        settle(&wallet);
        assert_eq!(*node.claims.lock().unwrap(), vec![(txid, 1, 1_200)]);
        let claimed = ready(&wallet);
        assert_eq!(
            claimed.claim.and_then(|claim| claim.message).as_deref(),
            Some("Claimed. It's in your balance.")
        );
        assert!(claimed.deposits.is_empty());
        assert_eq!(claimed.balance, "₿48,800");
        wallet.claim_reset();
        assert!(ready(&wallet).claim.is_none());
    }

    #[test]
    fn the_legacy_btc_format_shows_and_reads_decimal_btc() {
        let home = tempfile::tempdir().expect("temp dir");
        let node = Arc::new(Fake::default());
        node.balance.store(123_456, Ordering::SeqCst);
        node.deposits.lock().unwrap().push(DepositRow {
            txid: "cd".repeat(32),
            vout: 0,
            amount_sats: 50_000,
            mature: false,
            problem: None,
            refund_txid: None,
        });
        let mut wallet = Wallet::new(
            home.path().to_path_buf(),
            opener(node.clone(), Arc::new(Mutex::new(vec![]))),
        );
        wallet.set_format(Format::LegacyBtc);
        wallet.open(ENTROPY, false);
        settle(&wallet);
        let summary = ready(&wallet);
        assert_eq!(summary.balance, "0.00123456 BTC");
        assert_eq!(summary.balance_alternate, "₿123,456");
        assert_eq!(summary.balance_spoken, "0.00123456 BTC");
        assert_eq!(summary.balance_sats, 123_456, "stored as base units");
        assert_eq!(summary.deposits[0].amount, "0.00050000 BTC");
        // Amounts are typed as decimal BTC and quoted as base units.
        wallet.quote("spark1friend", "0.00002", "");
        settle(&wallet);
        let quote = ready(&wallet).send.quote.expect("quote");
        assert_eq!(
            (
                quote.amount.as_str(),
                quote.fee.as_str(),
                quote.total.as_str()
            ),
            ("0.00002000 BTC", "0.00000000 BTC", "0.00002000 BTC")
        );
        wallet.reset_send();
        wallet.quote("spark1friend", "2000", "");
        settle(&wallet);
        assert_eq!(
            ready(&wallet).send.quote.expect("quote").amount,
            "2,000.00000000 BTC"
        );
        wallet.reset_send();
        wallet.quote("spark1friend", "0.000000001", "");
        assert_eq!(
            ready(&wallet).send.message.as_deref(),
            Some("BTC amounts have at most eight decimals.")
        );
        wallet.invoice("0.0001");
        settle(&wallet);
        assert_eq!(
            ready(&wallet).receive.lightning.expect("invoice").caption,
            "Request for 0.00010000 BTC"
        );
        // Switching back changes every amount on the next screen.
        wallet.set_format(Format::Bip177);
        let summary = ready(&wallet);
        assert_eq!(summary.balance, "₿123,456");
        assert_eq!(
            summary.receive.lightning.expect("invoice").caption,
            "Request for ₿10,000"
        );
        wallet.invoice("0.0001");
        assert_eq!(
            ready(&wallet).receive.lightning_error.as_deref(),
            Some("Enter the amount in whole bitcoin base units, such as ₿1,000.")
        );
    }

    #[test]
    fn amounts_read_plainly() {
        let (bip177, legacy) = (Format::Bip177, Format::LegacyBtc);
        assert_eq!(parse_amount("", bip177), Ok(None));
        assert_eq!(parse_amount(" ₿1,000", bip177), Ok(Some(1_000)));
        assert_eq!(
            parse_amount("1.5", bip177),
            Err("Enter the amount in whole bitcoin base units, such as ₿1,000.".into())
        );
        assert_eq!(parse_amount("1.5", legacy), Ok(Some(150_000_000)));
        assert_eq!(parse_amount("0.00001 BTC", legacy), Ok(Some(1_000)));
        assert!(parse_amount("21000001", legacy).is_err());
        assert_eq!(shorten("short"), "short");
        assert_eq!(
            shorten("lnbc1234567890abcdefghijklmnopqrstuvwxyz"),
            "lnbc1234567890…qrstuvwxyz"
        );
    }

    /// A real Spark wallet on Lightspark's hosted regtest, which needs no
    /// API key or funds: it connects, reads its zero balance, and makes a
    /// Spark address. Run with `--ignored`; it reaches the network.
    #[test]
    #[ignore = "reaches Lightspark's hosted regtest"]
    fn a_regtest_wallet_connects_and_reads_its_balance() {
        let home = tempfile::tempdir().expect("temp dir");
        let entropy: [u8; 16] = rand_entropy();
        let mnemonic = bip39::Mnemonic::from_entropy(&entropy).unwrap().to_string();
        let node =
            crate::spark::open(home.path(), Network::Regtest, &mnemonic).expect("regtest wallet");
        node.sync().expect("sync");
        assert_eq!(node.balance().expect("balance"), 0);
        assert!(node.spark_address().expect("address").starts_with("spark"));
        assert!(node.payments(10).expect("payments").is_empty());
        assert!(matches!(
            node.quote(&SendRequest {
                input: "spark1nonsense".into(),
                ..SendRequest::default()
            }),
            Err(QuoteFailure::Refused(_) | QuoteFailure::NeedsAmount(_))
        ));
    }

    /// The deposit and exit calls against Lightspark's hosted regtest with a
    /// fresh wallet: fee rates read, no deposits waiting, and an exit state
    /// that exports and names its network. Funding a deposit there to claim
    /// and refund it needs the faucet's credentials (`FAUCET_USERNAME` and
    /// `FAUCET_PASSWORD` in Breez's `spark-itest`), which this repository
    /// does not hold. Run with `--ignored`; it reaches the network.
    #[test]
    #[ignore = "reaches Lightspark's hosted regtest"]
    fn a_regtest_wallet_reads_fee_rates_deposits_and_its_exit_state() {
        let home = tempfile::tempdir().expect("temp dir");
        let mnemonic = bip39::Mnemonic::from_entropy(&rand_entropy())
            .unwrap()
            .to_string();
        let node =
            crate::spark::open(home.path(), Network::Regtest, &mnemonic).expect("regtest wallet");
        node.sync().expect("sync");
        let rates = node.fee_rates().expect("fee rates");
        assert!(rates.fastest >= rates.hour, "{rates:?}");
        assert!(node.deposits().expect("deposits").is_empty());
        let exit = node.exit_state().expect("exit state");
        let parsed: serde_json::Value = serde_json::from_str(&exit).expect("json");
        assert!(parsed.get("version").is_some(), "{exit}");
        eprintln!("rates {rates:?}; exit state {} bytes", exit.len());
    }

    /// A throwaway mainnet wallet with the committed API key: it connects,
    /// reads its zero balance, and makes a Spark address, a deposit address,
    /// and a Lightning invoice. It moves no funds; nothing should be sent to
    /// what it prints, since its seed is discarded. Run with `--ignored`.
    #[test]
    #[ignore = "reaches Breez and Spark on mainnet"]
    fn a_mainnet_wallet_connects_with_the_committed_key() {
        let home = tempfile::tempdir().expect("temp dir");
        let mnemonic = bip39::Mnemonic::from_entropy(&rand_entropy())
            .unwrap()
            .to_string();
        let node = crate::spark::open(home.path(), NETWORK, &mnemonic).expect("mainnet wallet");
        node.sync().expect("sync");
        assert_eq!(node.balance().expect("balance"), 0);
        assert!(node.spark_address().expect("spark").starts_with("spark1"));
        assert!(node.bitcoin_address().expect("deposit").starts_with("bc1"));
        let invoice = node.invoice(Some(1_000), "OpenAgents").expect("invoice");
        assert!(invoice.starts_with("lnbc"), "{invoice}");
        assert!(node.payments(10).expect("payments").is_empty());
        let buy = node.buy(Provider::CashApp, 1_000).expect("cash app link");
        assert!(buy.starts_with("https://cash.app/"));
        let moonpay = node.buy(Provider::Moonpay, 50_000).expect("moonpay link");
        assert!(
            moonpay.starts_with("https://buy.moonpay.io/?apiKey="),
            "{moonpay}"
        );
        assert!(moonpay.contains("walletAddress=bc1"), "{moonpay}");
    }

    /// A throwaway, empty mainnet wallet reads a real Lightning address's
    /// terms and has the recipient's server make an invoice, then stops at
    /// the quote: nothing is paid, and the empty wallet could not pay. Run
    /// with `--ignored`; it reaches Breez, Spark, and getalby.com.
    #[test]
    #[ignore = "reaches Breez, Spark, and a Lightning address server on mainnet"]
    fn a_mainnet_lightning_address_quotes_without_paying() {
        let home = tempfile::tempdir().expect("temp dir");
        let mnemonic = bip39::Mnemonic::from_entropy(&rand_entropy())
            .unwrap()
            .to_string();
        let node = crate::spark::open(home.path(), NETWORK, &mnemonic).expect("mainnet wallet");
        let ask = |amount, comment: Option<&str>| {
            node.quote(&SendRequest {
                input: "hello@getalby.com".into(),
                amount_sats: amount,
                comment: comment.map(str::to_owned),
                format: Format::Bip177,
            })
        };
        match ask(None, None) {
            Err(QuoteFailure::NeedsAmount(ask)) => {
                assert_eq!(ask.recipient.as_deref(), Some("hello@getalby.com"));
                assert!(ask.message.contains("takes from ₿1 "), "{}", ask.message);
                assert!(ask.comment_max > 0);
                eprintln!("asked: {ask:?}");
            }
            other => panic!("an amount is asked: {other:?}"),
        }
        match ask(Some(21), Some("OpenAgents LNURL check")) {
            Ok(quote) => {
                assert_eq!(
                    quote.destination,
                    Destination::LightningAddress("hello@getalby.com".into())
                );
                assert_eq!(quote.amount_sats, 21);
                assert_eq!(quote.comment.as_deref(), Some("OpenAgents LNURL check"));
                eprintln!("quoted, not paid: {quote:?}");
            }
            // An empty wallet may be refused at the quote; that is still a
            // read of the recipient's terms and invoice.
            Err(QuoteFailure::Refused(message)) => {
                assert!(message.contains("enough"), "{message}");
                eprintln!("refused as empty: {message}");
            }
            other => panic!("quote: {other:?}"),
        }
        assert!(node.payments(10).expect("payments").is_empty());
    }

    /// Breez's server stopped signing MoonPay URLs (#9865), so the phone
    /// builds the SDK's URL unsigned: the SDK's key and parameters, the
    /// wallet's deposit address, and the amount in BTC with the amount
    /// locked. Anything but a plain address is refused.
    #[test]
    fn moonpay_opens_unsigned_for_the_deposit_address() {
        let url = crate::spark::moonpay_url(" bc1qexampleaddress0 ", 50_000).expect("url");
        let parsed = url::Url::parse(&url).expect("parses");
        assert_eq!(parsed.scheme(), "https");
        assert_eq!(parsed.host_str(), Some("buy.moonpay.io"));
        let pairs: std::collections::HashMap<String, String> =
            parsed.query_pairs().into_owned().collect();
        assert_eq!(pairs["apiKey"], "pk_live_Mx5g6bpD6Etd7T0bupthv7smoTNn2Vr");
        assert_eq!(pairs["currencyCode"], "btc");
        assert_eq!(pairs["walletAddress"], "bc1qexampleaddress0");
        assert_eq!(pairs["quoteCurrencyAmount"], "0.00050000");
        assert_eq!(pairs["lockAmount"], "true");
        assert_eq!(pairs["colorCode"], "#055DEB");
        assert!(!pairs.contains_key("signature"));
        assert_eq!(
            crate::spark::moonpay_url(&"x".repeat(0), 1).unwrap_err(),
            "no Bitcoin deposit address"
        );
        assert!(crate::spark::moonpay_url("bc1q&apiKey=evil", 1).is_err());
        let whole = crate::spark::moonpay_url("bc1q", 123_456_789).expect("url");
        assert!(whole.contains("quoteCurrencyAmount=1.23456789"), "{whole}");
    }

    fn rand_entropy() -> [u8; 16] {
        let id = uuid::Uuid::new_v4();
        *id.as_bytes()
    }
}
