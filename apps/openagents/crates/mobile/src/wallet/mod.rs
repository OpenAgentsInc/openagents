//! The Wallet screen: the person's Bitcoin wallet on mainnet through Breez's
//! Spark SDK, run by the wallet code computers share (`crates/spark-wallet`).
//! Rust decides every state and line of text and builds the screen as a
//! Rust Native view; the app shell mounts it, collects typed values, and
//! keeps the seed in its key store ([`seed::KeyStore`]).
//!
//! The network is fixed in this file. No request, setting, or stored value
//! switches the phone's wallet to another network. The seed and its words
//! stay in memory: they are never written by this module, logged, or put in
//! the view or an error. The words leave Rust only through
//! [`CoreWallet::recovery_words`], which the shell calls after the person
//! passed Face ID, and through [`seed::wallet_seed_words`] for the identity.

// Simulator builds and tests only: a phone build has no fixture code.
#[cfg(any(test, debug_assertions, target_abi = "sim"))]
pub mod fixture;
pub mod seed;
#[cfg(test)]
mod tests;

use crate::client_ffi::{CoreError, CoreResult};
use bitcoin_amount::Format;
use openagents_spark::model::{Destination, Node, PaymentRow, Quote, QuoteFailure, SendRequest};
use openagents_spark::seed::Seed;
use rust_native::{Activation, Axis, Element, FieldChange, Node as ViewNode, TextRole, View};
use seed::{KeyStore, SeedError, SeedRead};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

/// The only network the phone's wallet runs on.
pub const NETWORK: openagents_spark::Network = openagents_spark::Network::Mainnet;
/// The description a new Lightning invoice carries.
const INVOICE_DESCRIPTION: &str = "OpenAgents";
/// How many payments the screen lists.
const HISTORY_LIMIT: u32 = 50;
/// Above this balance the screen reminds the person that it's a phone wallet.
pub const BALANCE_WARNING_SATS: u64 = 1_000_000;
/// How long copied recovery words stay on the pasteboard, in seconds.
pub const WORDS_PASTEBOARD_SECS: u64 = 60;

/// Files under the wallet home. None holds a secret. The earlier app used
/// the same names in the same folder, so what it saved still shows.
const BALANCE_FILE: &str = "last-balance";
const TRUST_FILE: &str = "trust-acknowledged";
/// A fingerprint of the seed whose words the person saved (never the seed).
const WORDS_SAVED_FILE: &str = "words-saved";

/// The trust note: what Spark is and who the person relies on.
pub const TRUST_TITLE: &str = "About this wallet";
pub const TRUST_SUMMARY: &str = "Your bitcoin is kept by this phone, and only your recovery words can bring it back. Payments are instant because a few companies help move them; if they ever stop, you can still take your bitcoin out yourself, slowly. Keep only what you'd carry in your pocket.";
pub const TRUST_LINES: [&str; 5] = [
    "This wallet runs on Spark, not on a Lightning node of your own. Your keys stay on this phone.",
    "Three companies run Spark: Lightspark, Breez, and Flashnet. Two of them must work together for payments off the Bitcoin chain, and your safety depends on at least one of them having deleted old keys, which no one can check.",
    "If they stop, you can still take your bitcoin out on the Bitcoin chain yourself, but it can take days and needs a separate payment for fees.",
    "Lightning payments go through Lightspark.",
    "Keep amounts you'd be comfortable carrying in a phone wallet, and write down your recovery words.",
];

/// Opens the wallet under a home with a mnemonic. Blocking.
pub type Opener = Arc<dyn Fn(&Path, &str) -> Result<Arc<dyn Node>, String> + Send + Sync>;

/// The real opener: Breez's SDK on mainnet, its records in SQLite under
/// `home/breez` (one database per wallet).
pub fn spark_opener() -> Opener {
    Arc::new(|home, mnemonic| {
        let storage =
            breez_sdk_spark::default_storage(home.join("breez").to_string_lossy().into_owned());
        openagents_spark::spark::SparkNode::open(storage, NETWORK, mnemonic)
            .map(|node| Arc::new(node) as Arc<dyn Node>)
    })
}

/// A view intent: the key of the node that raised it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Action(pub String);

/// What the shell does after an activation.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Effect {
    #[default]
    None,
    /// Put `text` on the pasteboard.
    Copy { text: String },
    /// Ask for Face ID, then show the recovery words
    /// ([`CoreWallet::recovery_words`]).
    Backup,
}

/// Where the seed stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Key {
    /// Not read yet.
    Unread,
    Present,
    Absent,
    Unavailable,
    Unreadable,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Receive {
    #[default]
    Spark,
    Lightning,
    Bitcoin,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Sending {
    Idle,
    Quoting,
    Quoted(Quote),
    Paying(Quote),
    Sent(PaymentRow),
    Failed(String),
}

struct State {
    instance: String,
    revision: u64,
    key: Key,
    seed: Option<Seed>,
    node: Option<Arc<dyn Node>>,
    /// Bumped when the wallet starts again, so work begun for an earlier
    /// start changes nothing.
    generation: u64,
    starting: bool,
    refreshing: bool,
    error: Option<String>,
    balance: Option<u64>,
    synced_at: Option<u64>,
    spark: Option<String>,
    bitcoin: Option<String>,
    payments: Vec<PaymentRow>,
    receive: Receive,
    invoice_amount: String,
    invoice: Option<(String, Option<u64>)>,
    invoice_busy: bool,
    invoice_error: Option<String>,
    send_to: String,
    send_amount: String,
    send: Sending,
    restoring: bool,
    restore_words: String,
    restore_error: Option<String>,
    setup_busy: bool,
    trust_acknowledged: bool,
    trust_open: bool,
    words_saved: Option<String>,
}

impl State {
    fn bump(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }
}

/// The Wallet screen's controller.
pub struct Wallet {
    home: PathBuf,
    store: Arc<dyn KeyStore>,
    /// The identity's words, which a new wallet is made from (#11995).
    identity: Option<seed::IdentitySource>,
    opener: Opener,
    format: Format,
    shared: Arc<Mutex<State>>,
    notify: Arc<dyn Fn() + Send + Sync>,
}

fn lock(shared: &Mutex<State>) -> MutexGuard<'_, State> {
    shared.lock().unwrap_or_else(|poison| poison.into_inner())
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

impl Wallet {
    /// A wallet under `home` whose seed is in `store`. It reads nothing from
    /// the store until [`Wallet::open`].
    pub fn new(
        home: PathBuf,
        store: Arc<dyn KeyStore>,
        opener: Opener,
        notify: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        let last = std::fs::read_to_string(home.join(BALANCE_FILE))
            .ok()
            .and_then(|text| {
                let mut fields = text.split_whitespace();
                let total: u64 = fields.next()?.parse().ok()?;
                Some((total, fields.next().and_then(|value| value.parse().ok())))
            });
        let words_saved = std::fs::read_to_string(home.join(WORDS_SAVED_FILE))
            .ok()
            .map(|text| text.trim().to_owned());
        let state = State {
            instance: format!("wallet-{}", uuid::Uuid::new_v4()),
            revision: 1,
            key: Key::Unread,
            seed: None,
            node: None,
            generation: 0,
            starting: false,
            refreshing: false,
            error: None,
            balance: last.map(|(total, _)| total),
            synced_at: last.and_then(|(_, at)| at),
            spark: None,
            bitcoin: None,
            payments: vec![],
            receive: Receive::default(),
            invoice_amount: String::new(),
            invoice: None,
            invoice_busy: false,
            invoice_error: None,
            send_to: String::new(),
            send_amount: String::new(),
            send: Sending::Idle,
            restoring: false,
            restore_words: String::new(),
            restore_error: None,
            setup_busy: false,
            trust_acknowledged: home.join(TRUST_FILE).exists(),
            trust_open: false,
            words_saved,
        };
        Self {
            home,
            store,
            identity: None,
            opener,
            format: Format::default(),
            shared: Arc::new(Mutex::new(state)),
            notify,
        }
    }

    /// Make a new wallet from the identity's words (#11995).
    pub fn with_identity(mut self, identity: seed::IdentitySource) -> Self {
        self.identity = Some(identity);
        self
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        lock(&self.shared)
    }

    fn changed(&self) {
        self.lock().bump();
        (self.notify)();
    }

    /// Read the seed and start the wallet when there is one. A repeat only
    /// retries what failed: a seed already read is never read again, and a
    /// running wallet keeps running.
    pub fn open(&self) {
        {
            let state = self.lock();
            if state.seed.is_some() {
                drop(state);
                return self.start();
            }
        }
        let read = seed::read(self.store.as_ref());
        {
            let mut state = self.lock();
            match read {
                SeedRead::Present(seed) => {
                    state.key = Key::Present;
                    state.seed = Some(seed);
                }
                SeedRead::Absent => state.key = Key::Absent,
                SeedRead::Unavailable => state.key = Key::Unavailable,
                SeedRead::Unreadable => state.key = Key::Unreadable,
            }
        }
        self.start();
        self.changed();
    }

    /// The recovery words of the running wallet, for the backup sheet the
    /// shell shows after Face ID.
    pub fn words(&self) -> Option<Vec<String>> {
        self.lock()
            .seed
            .as_ref()
            .map(|seed| seed.words().into_iter().map(str::to_owned).collect())
    }

    /// The person saw their words and said they wrote them down.
    pub fn words_saved(&self) {
        let fingerprint = {
            let state = self.lock();
            let Some(seed) = &state.seed else { return };
            seed.fingerprint()
        };
        let _ = std::fs::create_dir_all(&self.home);
        let _ = std::fs::write(self.home.join(WORDS_SAVED_FILE), format!("{fingerprint}\n"));
        self.lock().words_saved = Some(fingerprint);
        self.changed();
    }

    /// Start the wallet in the background, unless it runs or is starting.
    fn start(&self) {
        let (mnemonic, generation) = {
            let mut state = self.lock();
            let Some(seed) = &state.seed else { return };
            if state.node.is_some() || state.starting {
                return;
            }
            let mnemonic = seed.mnemonic.clone();
            state.generation += 1;
            state.starting = true;
            state.error = None;
            (mnemonic, state.generation)
        };
        let (home, opener, shared, notify) = (
            self.home.clone(),
            self.opener.clone(),
            self.shared.clone(),
            self.notify.clone(),
        );
        std::thread::spawn(move || {
            let opened = std::fs::create_dir_all(&home)
                .map_err(|_| "Couldn't make the wallet's folder on this phone.".to_string())
                .and_then(|()| opener(&home, &mnemonic));
            drop(mnemonic);
            let node = match opened {
                Ok(node) => node,
                Err(message) => {
                    let mut state = lock(&shared);
                    if state.generation == generation {
                        state.starting = false;
                        state.error = Some(message);
                        state.bump();
                    }
                    drop(state);
                    notify();
                    return;
                }
            };
            let (spark, bitcoin) = (node.spark_address().ok(), node.bitcoin_address().ok());
            {
                let mut state = lock(&shared);
                if state.generation != generation {
                    return;
                }
                state.starting = false;
                state.refreshing = true;
                state.node = Some(node.clone());
                state.spark = spark;
                state.bitcoin = bitcoin;
                state.bump();
            }
            notify();
            let (later_shared, later_home, later_notify) =
                (shared.clone(), home.clone(), notify.clone());
            node.subscribe(Arc::new(move || {
                let (shared, home, notify) = (
                    later_shared.clone(),
                    later_home.clone(),
                    later_notify.clone(),
                );
                std::thread::spawn(move || read_node(&shared, &home, generation, false, &notify));
            }));
            read_node(&shared, &home, generation, true, &notify);
        });
    }

    /// Read Spark again, or start a wallet that failed to start.
    pub fn refresh(&self) {
        let generation = {
            let mut state = self.lock();
            if state.node.is_none() {
                drop(state);
                return self.open();
            }
            if state.refreshing {
                return;
            }
            state.refreshing = true;
            state.generation
        };
        self.changed();
        let (shared, home, notify) = (self.shared.clone(), self.home.clone(), self.notify.clone());
        std::thread::spawn(move || read_node(&shared, &home, generation, true, &notify));
    }

    /// The screen as a Rust Native view.
    pub fn view(&self) -> View<Action> {
        view(&self.lock(), self.format)
    }

    /// A typed value from a field.
    pub fn change_field(&self, event: &FieldChange) -> Result<(), String> {
        let action = self
            .view()
            .validate()
            .map_err(|_| "Couldn't open the wallet.".to_string())?
            .change_field(event)
            .map_err(|_| "This changed. Try again.".to_string())?
            .clone();
        let value = event.value.clone();
        {
            let mut state = self.lock();
            match action.0.as_str() {
                "restore-words" => state.restore_words = value,
                "invoice-amount" => state.invoice_amount = value,
                "send-to" => {
                    state.send_to = value;
                    if !matches!(state.send, Sending::Quoting | Sending::Paying(_)) {
                        state.send = Sending::Idle;
                    }
                }
                "send-amount" => {
                    state.send_amount = value;
                    if !matches!(state.send, Sending::Quoting | Sending::Paying(_)) {
                        state.send = Sending::Idle;
                    }
                }
                _ => return Err("This changed. Try again.".into()),
            }
        }
        self.changed();
        Ok(())
    }

    /// A tap on a button.
    pub fn activate(&self, event: &Activation) -> Result<Effect, String> {
        let action = self
            .view()
            .validate()
            .map_err(|_| "Couldn't open the wallet.".to_string())?
            .activate(event)
            .map_err(|_| "This changed. Try again.".to_string())?
            .clone();
        let effect = self.run(&action.0);
        self.changed();
        Ok(effect)
    }

    fn run(&self, key: &str) -> Effect {
        match key {
            "retry" => self.retry(),
            "refresh" => self.refresh(),
            "create" => self.create(),
            "restore-open" => {
                let mut state = self.lock();
                state.restoring = true;
                state.restore_error = None;
            }
            "restore-cancel" => {
                let mut state = self.lock();
                state.restoring = false;
                state.restore_words.clear();
                state.restore_error = None;
            }
            "restore" => self.restore(),
            "trust" => {
                let mut state = self.lock();
                state.trust_open = !state.trust_open;
            }
            "trust-ok" => {
                let _ = std::fs::create_dir_all(&self.home);
                let _ = std::fs::write(self.home.join(TRUST_FILE), b"1\n");
                let mut state = self.lock();
                state.trust_acknowledged = true;
                state.trust_open = false;
            }
            "backup" => return Effect::Backup,
            "receive-spark" => self.lock().receive = Receive::Spark,
            "receive-lightning" => self.lock().receive = Receive::Lightning,
            "receive-bitcoin" => self.lock().receive = Receive::Bitcoin,
            "invoice" => self.invoice(),
            "copy" => {
                let state = self.lock();
                let text = match state.receive {
                    Receive::Spark => state.spark.clone(),
                    Receive::Bitcoin => state.bitcoin.clone(),
                    Receive::Lightning => state.invoice.as_ref().map(|(text, _)| text.clone()),
                };
                if let Some(text) = text {
                    return Effect::Copy { text };
                }
            }
            "review" => self.quote(),
            "pay" => self.pay(),
            "send-cancel" => {
                let mut state = self.lock();
                if !matches!(state.send, Sending::Quoting | Sending::Paying(_)) {
                    state.send = Sending::Idle;
                }
            }
            "send-done" => {
                let mut state = self.lock();
                state.send = Sending::Idle;
                state.send_to.clear();
                state.send_amount.clear();
            }
            _ => {}
        }
        Effect::None
    }

    /// Read the key store again after it was locked, or start again after a
    /// failed start.
    fn retry(&self) {
        let key = self.lock().key;
        match key {
            Key::Present => self.start(),
            // A seed is never read twice; this only runs while none is held.
            _ => self.open(),
        }
    }

    /// Make a wallet, only when the key store says none exists.
    fn create(&self) {
        {
            let mut state = self.lock();
            if state.key != Key::Absent || state.setup_busy {
                return;
            }
            state.setup_busy = true;
        }
        let made = seed::create_with(self.store.as_ref(), self.identity.as_ref());
        self.settle_setup(made);
    }

    /// Restore from recovery words, only when the key store says no wallet
    /// exists.
    fn restore(&self) {
        let words = {
            let mut state = self.lock();
            if state.key != Key::Absent || state.setup_busy {
                return;
            }
            state.setup_busy = true;
            state.restore_error = None;
            state.restore_words.clone()
        };
        let restored = seed::restore(self.store.as_ref(), &words);
        let worked = restored.is_ok();
        self.settle_setup(restored);
        if worked {
            // The person typed the words, so they have them.
            self.words_saved();
        }
    }

    fn settle_setup(&self, result: Result<Seed, SeedError>) {
        {
            let mut state = self.lock();
            state.setup_busy = false;
            match result {
                Ok(seed) => {
                    state.key = Key::Present;
                    state.seed = Some(seed);
                    state.restoring = false;
                    state.restore_words.clear();
                    state.restore_error = None;
                    // A new wallet has nothing an earlier launch read.
                    state.balance = None;
                    state.synced_at = None;
                    let _ = std::fs::remove_file(self.home.join(BALANCE_FILE));
                }
                // Another seed appeared: read and use that one.
                Err(SeedError::Exists) => {
                    state.restoring = false;
                    state.restore_words.clear();
                    state.restore_error = None;
                    drop(state);
                    return self.open();
                }
                Err(SeedError::Unavailable) => state.key = Key::Unavailable,
                Err(SeedError::Unreadable) => state.key = Key::Unreadable,
                Err(error) => state.restore_error = Some(error.message()),
            }
        }
        self.start();
    }

    fn invoice(&self) {
        let amount = {
            let mut state = self.lock();
            match self.format.parse(&state.invoice_amount) {
                Ok(amount) => amount,
                Err(error) => {
                    state.invoice_error = Some(error.message(self.format));
                    return;
                }
            }
        };
        let (node, generation) = {
            let mut state = self.lock();
            let Some(node) = state.node.clone() else {
                state.invoice_error = Some("The wallet is still starting.".into());
                return;
            };
            if state.invoice_busy {
                return;
            }
            state.invoice_busy = true;
            state.invoice_error = None;
            (node, state.generation)
        };
        let (shared, notify) = (self.shared.clone(), self.notify.clone());
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
            state.bump();
            drop(state);
            notify();
        });
    }

    fn quote(&self) {
        let (request, node, generation) = {
            let mut state = self.lock();
            let input = state.send_to.trim().to_owned();
            if input.is_empty() {
                state.send = Sending::Failed(
                    "Paste an invoice, a Bitcoin address, or a Lightning address.".into(),
                );
                return;
            }
            let amount = match self.format.parse(&state.send_amount) {
                Ok(amount) => amount,
                Err(error) => {
                    state.send = Sending::Failed(error.message(self.format));
                    return;
                }
            };
            let Some(node) = state.node.clone() else {
                state.send = Sending::Failed("The wallet is still starting.".into());
                return;
            };
            if matches!(state.send, Sending::Quoting | Sending::Paying(_)) {
                return;
            }
            state.send = Sending::Quoting;
            (
                SendRequest {
                    input,
                    amount_sats: amount,
                    comment: None,
                    format: self.format,
                },
                node,
                state.generation,
            )
        };
        let (shared, notify) = (self.shared.clone(), self.notify.clone());
        std::thread::spawn(move || {
            let quoted = node.quote(&request);
            let mut state = lock(&shared);
            if state.generation != generation || state.send != Sending::Quoting {
                return;
            }
            state.send = match quoted {
                Ok(quote) => Sending::Quoted(quote),
                Err(QuoteFailure::NeedsAmount(ask)) => Sending::Failed(ask.message),
                Err(QuoteFailure::Refused(message)) => Sending::Failed(message),
            };
            state.bump();
            drop(state);
            notify();
        });
    }

    /// Pay the quote on screen, once.
    fn pay(&self) {
        let (node, quote, generation) = {
            let mut state = self.lock();
            let Sending::Quoted(quote) = &state.send else {
                return;
            };
            let quote = quote.clone();
            let Some(node) = state.node.clone() else {
                return;
            };
            state.send = Sending::Paying(quote.clone());
            (node, quote, state.generation)
        };
        let (shared, home, notify) = (self.shared.clone(), self.home.clone(), self.notify.clone());
        let key = uuid::Uuid::new_v4().to_string();
        std::thread::spawn(move || {
            let paid = node.pay(quote.id, &key);
            // Read the new balance and history before the screen leaves
            // "Sending", so they arrive together.
            read_node(&shared, &home, generation, false, &notify);
            {
                let mut state = lock(&shared);
                if state.generation != generation {
                    return;
                }
                state.send = match paid {
                    Ok(paid) => Sending::Sent(paid.row),
                    Err(failure) if failure.outcome_unknown() => Sending::Failed(format!(
                        "{} Check Activity before you try again.",
                        failure.message()
                    )),
                    Err(failure) => Sending::Failed(failure.message().to_owned()),
                };
                state.bump();
            }
            notify();
        });
    }
}

/// Read the balance and recent payments, syncing first when `sync` is set.
/// It changes nothing if the wallet started again meanwhile.
fn read_node(
    shared: &Mutex<State>,
    home: &Path,
    generation: u64,
    sync: bool,
    notify: &Arc<dyn Fn() + Send + Sync>,
) {
    let Some(node) = lock(shared).node.clone() else {
        return;
    };
    let synced = if sync { node.sync() } else { Ok(()) };
    let balance = node.balance();
    let payments = node.payments(HISTORY_LIMIT);
    {
        let mut state = lock(shared);
        if state.generation != generation {
            return;
        }
        if sync {
            state.refreshing = false;
        }
        match (&synced, &balance) {
            (Ok(()), Ok(total)) => {
                let at = now();
                let _ = std::fs::write(home.join(BALANCE_FILE), format!("{total} {at}\n"));
                state.balance = Some(*total);
                state.synced_at = Some(at);
                state.error = None;
            }
            (Err(message), _) | (_, Err(message)) => state.error = Some(message.clone()),
        }
        if let Ok(payments) = payments {
            state.payments = payments;
        }
        state.bump();
    }
    notify();
}

fn node(key: impl Into<String>, element: Element<Action>) -> ViewNode<Action> {
    ViewNode {
        key: key.into(),
        style: Default::default(),
        element,
    }
}

fn text(key: impl Into<String>, value: impl Into<String>, role: TextRole) -> ViewNode<Action> {
    node(
        key,
        Element::Text {
            value: value.into(),
            role,
        },
    )
}

fn button(key: impl Into<String>, label: impl Into<String>, enabled: bool) -> ViewNode<Action> {
    let key = key.into();
    node(
        key.clone(),
        Element::Button {
            label: label.into(),
            enabled,
            icon: None,
            shortcut: None,
            intent: Action(key),
        },
    )
}

fn field(
    key: impl Into<String>,
    label: impl Into<String>,
    value: impl Into<String>,
    placeholder: impl Into<String>,
    multiline: bool,
    enabled: bool,
) -> ViewNode<Action> {
    let key = key.into();
    node(
        key.clone(),
        Element::Field {
            label: label.into(),
            value: value.into(),
            placeholder: placeholder.into(),
            secret: false,
            multiline,
            enabled,
            max_bytes: 4096,
            on_change: Action(key),
        },
    )
}

fn stack(key: impl Into<String>, children: Vec<ViewNode<Action>>) -> ViewNode<Action> {
    node(
        key,
        Element::Stack {
            axis: Axis::Vertical,
            children,
        },
    )
}

fn view(state: &State, format: Format) -> View<Action> {
    let children = match state.key {
        Key::Unread => vec![text("status", "Opening your wallet…", TextRole::Status)],
        Key::Unavailable => vec![
            text("error", seed::UNAVAILABLE, TextRole::Body),
            button("retry", "Try again", true),
        ],
        Key::Unreadable => vec![text("error", seed::UNREADABLE, TextRole::Body)],
        Key::Absent => setup(state),
        Key::Present => ready(state, format),
    };
    View::new_v3(&state.instance, state.revision, stack("wallet", children))
}

fn setup(state: &State) -> Vec<ViewNode<Action>> {
    let enabled = !state.setup_busy;
    let mut nodes = vec![text("heading", "Wallet", TextRole::Heading)];
    if state.restoring {
        nodes.push(text(
            "restore-detail",
            "Type the 12 or 24 recovery words of your wallet, in order.",
            TextRole::Body,
        ));
        nodes.push(field(
            "restore-words",
            "Recovery words",
            state.restore_words.clone(),
            "",
            true,
            enabled,
        ));
        if let Some(error) = &state.restore_error {
            nodes.push(text("restore-error", error, TextRole::Status));
        }
        nodes.push(button("restore", "Restore", enabled));
        nodes.push(button("restore-cancel", "Cancel", enabled));
    } else {
        nodes.push(text(
            "setup-detail",
            "Make a new Bitcoin wallet on this phone, or bring back one you had with its recovery words.",
            TextRole::Body,
        ));
        if let Some(error) = &state.restore_error {
            nodes.push(text("setup-error", error, TextRole::Status));
        }
        nodes.push(button("create", "Make a wallet", enabled));
        nodes.push(button("restore-open", "Restore from words", enabled));
    }
    nodes
}

fn ready(state: &State, format: Format) -> Vec<ViewNode<Action>> {
    let running = state.node.is_some();
    let mut nodes = vec![text(
        "balance",
        state
            .balance
            .map_or_else(|| "–".to_owned(), |total| format.show(total)),
        TextRole::Heading,
    )];
    let status = if state.starting {
        Some("Connecting…".to_owned())
    } else if state.refreshing {
        Some("Updating…".to_owned())
    } else {
        state.synced_at.and_then(|at| {
            (now().saturating_sub(at) > 15 * 60).then(|| format!("Updated {}", day(at)))
        })
    };
    if let Some(status) = status {
        nodes.push(text("status", status, TextRole::Status));
    }
    if let Some(error) = &state.error {
        nodes.push(text("error", error, TextRole::Status));
        if !running && !state.starting {
            nodes.push(button("retry", "Try again", true));
        }
    }
    if state
        .balance
        .is_some_and(|total| total > BALANCE_WARNING_SATS)
    {
        nodes.push(text(
            "warning",
            format!(
                "This phone wallet holds more than {}. Keep only what you'd carry, and make sure your recovery words are written down.",
                format.show(BALANCE_WARNING_SATS)
            ),
            TextRole::Body,
        ));
    }
    let fingerprint = state.seed.as_ref().map(Seed::fingerprint);
    if fingerprint.is_some() && state.words_saved != fingerprint {
        nodes.push(stack(
            "backup-card",
            vec![
                text("backup-title", "Back up your wallet", TextRole::Heading),
                text(
                    "backup-detail",
                    "If you lose this phone, your recovery words are the only way to get your bitcoin back.",
                    TextRole::Body,
                ),
            ],
        ));
    }
    if !state.trust_acknowledged || state.trust_open {
        let mut trust = vec![
            text("trust-title", TRUST_TITLE, TextRole::Heading),
            text("trust-summary", TRUST_SUMMARY, TextRole::Body),
        ];
        if state.trust_open {
            for (index, line) in TRUST_LINES.iter().enumerate() {
                trust.push(text(format!("trust-line-{index}"), *line, TextRole::Body));
            }
        } else {
            trust.push(button("trust", "Read more", true));
        }
        trust.push(button("trust-ok", "Got it", true));
        nodes.push(stack("trust-card", trust));
    }
    nodes.push(receive(state, format, running));
    nodes.push(send(state, format, running));
    nodes.push(history(state, format));
    let mut more = vec![
        button("backup", "Back up recovery words", true),
        button("refresh", "Refresh", running && !state.refreshing),
    ];
    if state.trust_acknowledged && !state.trust_open {
        more.push(button("trust", TRUST_TITLE, true));
    }
    nodes.push(stack("more", more));
    nodes
}

fn receive(state: &State, format: Format, running: bool) -> ViewNode<Action> {
    let mut nodes = vec![
        text("receive-heading", "Receive", TextRole::Heading),
        button(
            "receive-spark",
            selected("Spark address", state.receive == Receive::Spark),
            true,
        ),
        button(
            "receive-lightning",
            selected("Lightning invoice", state.receive == Receive::Lightning),
            true,
        ),
        button(
            "receive-bitcoin",
            selected("Bitcoin address", state.receive == Receive::Bitcoin),
            true,
        ),
    ];
    let code = match state.receive {
        Receive::Spark => {
            nodes.push(text(
                "receive-detail",
                "Free and instant from other Spark wallets.",
                TextRole::Body,
            ));
            state.spark.clone()
        }
        Receive::Bitcoin => {
            nodes.push(text(
                "receive-detail",
                "Arrives after 3 confirmations on the Bitcoin chain.",
                TextRole::Body,
            ));
            state.bitcoin.clone()
        }
        Receive::Lightning => {
            nodes.push(field(
                "invoice-amount",
                format!("Amount in {}", format.unit()),
                state.invoice_amount.clone(),
                "Any amount",
                false,
                running && !state.invoice_busy,
            ));
            if let Some(error) = &state.invoice_error {
                nodes.push(text("invoice-error", error, TextRole::Status));
            }
            nodes.push(button(
                "invoice",
                if state.invoice_busy {
                    "Making invoice…"
                } else {
                    "Make invoice"
                },
                running && !state.invoice_busy,
            ));
            if let Some((_, amount)) = &state.invoice {
                nodes.push(text(
                    "receive-detail",
                    match amount {
                        Some(amount) => format!("Request for {}", format.show(*amount)),
                        None => "Request for any amount".into(),
                    },
                    TextRole::Body,
                ));
            }
            state.invoice.as_ref().map(|(invoice, _)| invoice.clone())
        }
    };
    match code {
        Some(code) => {
            nodes.push(text("qr", code, TextRole::Code));
            nodes.push(button("copy", "Copy", true));
        }
        None if state.receive != Receive::Lightning => {
            nodes.push(text(
                "receive-waiting",
                "Shows once the wallet connects.",
                TextRole::Status,
            ));
        }
        None => {}
    }
    stack("receive", nodes)
}

fn selected(label: &str, chosen: bool) -> String {
    if chosen {
        format!("✓ {label}")
    } else {
        label.to_owned()
    }
}

fn send(state: &State, format: Format, running: bool) -> ViewNode<Action> {
    let editable = running && !matches!(state.send, Sending::Quoting | Sending::Paying(_));
    let mut nodes = vec![
        text("send-heading", "Send", TextRole::Heading),
        field(
            "send-to",
            "Pay to",
            state.send_to.clone(),
            "Invoice, Bitcoin address, or Lightning address",
            true,
            editable,
        ),
        field(
            "send-amount",
            format!("Amount in {}", format.unit()),
            state.send_amount.clone(),
            "Only if the request has none",
            false,
            editable,
        ),
    ];
    match &state.send {
        Sending::Idle => nodes.push(button("review", "Review", editable)),
        Sending::Quoting => nodes.push(button("review", "Checking…", false)),
        Sending::Failed(message) => {
            nodes.push(text("send-error", message, TextRole::Status));
            nodes.push(button("review", "Review", editable));
        }
        Sending::Quoted(quote) | Sending::Paying(quote) => {
            let paying = matches!(state.send, Sending::Paying(_));
            nodes.push(text("quote", quote_text(quote, format), TextRole::Body));
            if let Some(note) = &quote.note {
                nodes.push(text("quote-note", note, TextRole::Body));
            }
            nodes.push(button(
                "pay",
                if paying {
                    "Sending…".to_owned()
                } else {
                    format!(
                        "Send {}",
                        format.show(quote.amount_sats.saturating_add(quote.fee_sats))
                    )
                },
                !paying,
            ));
            nodes.push(button("send-cancel", "Cancel", !paying));
        }
        Sending::Sent(row) => {
            nodes.push(text(
                "sent",
                match row.status.as_str() {
                    "pending" => format!("{} is on its way.", format.show(row.amount_sats)),
                    "failed" => "The payment didn't go through.".to_owned(),
                    _ => format!("Sent {}.", format.show(row.amount_sats)),
                },
                TextRole::Body,
            ));
            nodes.push(button("send-done", "Done", true));
        }
    }
    stack("send", nodes)
}

fn quote_text(quote: &Quote, format: Format) -> String {
    let to = match &quote.destination {
        Destination::Lightning(invoice) => format!("Lightning invoice {}", shorten(invoice)),
        Destination::LightningAddress(address) => address.clone(),
        Destination::Spark(address) => format!("Spark address {}", shorten(address)),
        Destination::Bitcoin(address) => format!("Bitcoin address {}", shorten(address)),
    };
    format!(
        "{} to {to}. Fee {}. Total {}.",
        format.show(quote.amount_sats),
        format.show(quote.fee_sats),
        format.show(quote.amount_sats.saturating_add(quote.fee_sats))
    )
}

fn history(state: &State, format: Format) -> ViewNode<Action> {
    let mut nodes = vec![text("history-heading", "Activity", TextRole::Heading)];
    if state.payments.is_empty() {
        nodes.push(text("history-empty", "No payments yet.", TextRole::Status));
    }
    for (index, row) in state.payments.iter().enumerate() {
        let mut line = format!(
            "{} {} · {}",
            if row.received { "Received" } else { "Sent" },
            format.show_signed(row.amount_sats, row.received),
            day(row.at)
        );
        match row.status.as_str() {
            "pending" => line.push_str(" · Pending"),
            "failed" => line.push_str(" · Failed"),
            _ => {}
        }
        if !row.received && row.fee_sats > 0 {
            line.push_str(&format!(" · {} fee", format.show(row.fee_sats)));
        }
        nodes.push(text(format!("payment-{index}"), line, TextRole::Body));
    }
    stack("history", nodes)
}

/// The start and end of a long code.
fn shorten(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= 28 {
        return text.to_owned();
    }
    let head: String = chars[..14].iter().collect();
    let tail: String = chars[chars.len() - 10..].iter().collect();
    format!("{head}…{tail}")
}

/// A Unix time as a UTC date, `2026-10-17`.
fn day(at: u64) -> String {
    let days = (at / 86_400) as i64;
    // Howard Hinnant's civil-from-days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

// ── UniFFI surface ───────────────────────────────────────────────────────

/// Called whenever the Wallet screen changes; the shell takes a new snapshot.
#[uniffi::export(with_foreign)]
pub trait WalletListener: Send + Sync {
    fn on_wallet_changed(&self);
}

/// The Wallet screen for the app shell.
#[derive(uniffi::Object)]
pub struct CoreWallet {
    wallet: Wallet,
}

fn failed(message: impl Into<String>) -> CoreError {
    CoreError::InvalidArgument {
        message: message.into(),
    }
}

#[uniffi::export]
impl CoreWallet {
    /// The person's wallet. `home` is the wallet's folder (the earlier app
    /// used `Application Support/openagents-v1/spark`); `store` holds the
    /// seed. Nothing is read until [`CoreWallet::open`].
    #[uniffi::constructor]
    pub fn new(
        home: String,
        store: Arc<dyn KeyStore>,
        listener: Arc<dyn WalletListener>,
    ) -> Arc<Self> {
        Arc::new(Self {
            wallet: Wallet::new(
                PathBuf::from(home),
                store,
                spark_opener(),
                Arc::new(move || listener.on_wallet_changed()),
            )
            .with_identity(seed::phone_identity()),
        })
    }

    /// Read the seed and start the wallet; a repeat retries what failed.
    pub fn open(&self) {
        self.wallet.open();
    }

    pub fn refresh(&self) {
        self.wallet.refresh();
    }

    /// The screen as Rust Native view JSON.
    pub fn snapshot(&self) -> CoreResult<String> {
        let view = self
            .wallet
            .view()
            .validate()
            .map_err(|_| failed("Couldn't open the wallet."))?;
        serde_json::to_string(view.view()).map_err(|_| failed("Couldn't open the wallet."))
    }

    pub fn change_field(
        &self,
        instance: String,
        revision: u64,
        key: String,
        value: String,
    ) -> CoreResult<()> {
        self.wallet
            .change_field(&FieldChange {
                activation: Activation {
                    instance,
                    revision,
                    node: key,
                },
                value,
            })
            .map_err(failed)
    }

    /// A button tap; the JSON effect for the shell (`none`, `copy` with
    /// `text`, or `backup`).
    pub fn activate(&self, instance: String, revision: u64, key: String) -> CoreResult<String> {
        let effect = self
            .wallet
            .activate(&Activation {
                instance,
                revision,
                node: key,
            })
            .map_err(failed)?;
        serde_json::to_string(&effect).map_err(|_| failed("Couldn't do that."))
    }

    /// The recovery words, for the backup sheet. Call it only after the
    /// person passed Face ID or the passcode, and keep the words no longer
    /// than the sheet shows them.
    pub fn recovery_words(&self) -> CoreResult<Vec<String>> {
        self.wallet
            .words()
            .ok_or_else(|| failed("This phone has no wallet yet."))
    }

    /// The person said they wrote the words down.
    pub fn words_saved(&self) {
        self.wallet.words_saved();
    }

    /// How long copied recovery words stay on the pasteboard, in seconds.
    pub fn words_pasteboard_seconds(&self) -> u64 {
        WORDS_PASTEBOARD_SECS
    }
}

/// The offline fixture, in simulator builds and tests only: a build for a
/// phone has no fixture code.
#[cfg(any(test, debug_assertions, target_abi = "sim"))]
#[uniffi::export]
impl CoreWallet {
    /// An offline wallet for simulator captures: a fixed balance and
    /// history, a seed held only in memory, and no network. It never touches
    /// the Keychain and holds no money.
    #[uniffi::constructor]
    pub fn fixture(home: String, listener: Arc<dyn WalletListener>) -> Arc<Self> {
        Arc::new(Self {
            wallet: Wallet::new(
                PathBuf::from(home),
                Arc::new(seed::MemoryStore::holding(fixture::ENTROPY.to_vec())),
                fixture::opener(),
                Arc::new(move || listener.on_wallet_changed()),
            ),
        })
    }
}
