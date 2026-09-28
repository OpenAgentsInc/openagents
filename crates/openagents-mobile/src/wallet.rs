//! The Wallet tab: a Bitcoin wallet for test coins on Mutinynet, the public
//! signet that the repository's wallet (`crates/wallet`) uses for testing.
//! It runs that crate's `ldk-node` wallet, with its store in the app's
//! private state directory and its key from the platform key store.
//!
//! The network is fixed in this file. No request, configuration, or stored
//! value switches the phone's wallet to mainnet, and it never opens the
//! wallet directory a computer uses (`~/.openagents/wallet`). The 32-byte
//! wallet key arrives once per app lifetime with `wallet_open`; it and its
//! mnemonic stay in memory, and neither is written, logged, or put in a
//! packet.

use openagents_wallet::config::{MUTINYNET_ESPLORA, Network, WalletConfig};
use openagents_wallet::ldk::{LdkWallet, mnemonic_from_entropy};
use openagents_wallet::{Balance, LightningWallet, WalletError};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

/// The only network the phone's wallet runs on.
pub const NETWORK: Network = Network::Signet;
/// Mutinynet's Esplora server; Mutinynet is a signet with 30-second blocks.
pub const ESPLORA: &str = MUTINYNET_ESPLORA;
/// Where the owner gets free test coins.
pub const FAUCET: &str = "https://faucet.mutinynet.com";
/// The file under the wallet home that keeps the address the screen shows,
/// so it does not change on every launch. It holds no secret.
const ADDRESS_FILE: &str = "receive-address";
/// The file under the wallet home that keeps the last balance read, so the
/// screen shows it at once while the wallet starts and reads the chain
/// again. It holds no secret.
const BALANCE_FILE: &str = "last-balance";

/// The phone wallet's configuration: Mutinynet, no listening socket, no
/// liquidity provider, and no trusted peers.
pub fn config() -> Result<WalletConfig, String> {
    let config = WalletConfig::new(NETWORK, Some(ESPLORA)).map_err(|error| error.to_string())?;
    admit(&config)?;
    Ok(config)
}

/// Refuse any configuration but a test network. The phone never holds
/// mainnet coins.
fn admit(config: &WalletConfig) -> Result<(), String> {
    match config.network {
        Network::Signet | Network::Testnet | Network::Regtest => Ok(()),
        Network::Bitcoin => Err("The phone's wallet runs only on a test network.".into()),
    }
}

/// Whether `address` is a test-network bech32 address (`tb1` on testnet and
/// signet, `bcrt1` on regtest), never a mainnet one.
fn test_address(address: &str) -> bool {
    let lower = address.to_ascii_lowercase();
    lower.starts_with("tb1") || lower.starts_with("bcrt1")
}

/// What the screen needs from a running wallet. `LdkWallet` is the real one;
/// tests supply their own.
pub trait Node: Send + Sync {
    fn balance(&self) -> Result<Balance, WalletError>;
    fn new_address(&self) -> Result<String, WalletError>;
    fn sync(&self) -> Result<(), WalletError>;
    fn synced_at(&self) -> Option<u64>;
}

impl Node for LdkWallet {
    fn balance(&self) -> Result<Balance, WalletError> {
        LightningWallet::balance(self)
    }
    fn new_address(&self) -> Result<String, WalletError> {
        self.funding_address()
    }
    fn sync(&self) -> Result<(), WalletError> {
        LdkWallet::sync(self)
    }
    fn synced_at(&self) -> Option<u64> {
        self.onchain_synced_at()
    }
}

/// Opens the wallet under a home with a mnemonic. Blocking.
pub type Opener =
    Arc<dyn Fn(&Path, &WalletConfig, &str) -> Result<Arc<dyn Node>, String> + Send + Sync>;

/// The real opener: start `ldk-node` on the Mutinynet configuration.
pub fn ldk_opener() -> Opener {
    Arc::new(|home, config, mnemonic| {
        admit(config)?;
        LdkWallet::open(home, config, mnemonic)
            .map(|wallet| Arc::new(wallet) as Arc<dyn Node>)
            .map_err(|error| describe(&error))
    })
}

/// A mnemonic. It has no `Debug` so it cannot reach a log line.
struct Seed(String);

#[derive(Default)]
struct Shared {
    node: Option<Arc<dyn Node>>,
    address: Option<String>,
    starting: bool,
    refreshing: bool,
    /// The last start or sync failure, cleared by the next success.
    error: Option<String>,
    /// A sync finished since the node started.
    synced: bool,
    /// The last balance read, saved across launches.
    last: Option<LastBalance>,
}

/// A balance read by an earlier sync, shown until this launch reads again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LastBalance {
    total: u64,
    pending: u64,
    synced_at: Option<u64>,
}

impl LastBalance {
    fn read(home: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(home.join(BALANCE_FILE)).ok()?;
        let mut fields = text.split_whitespace();
        let total: u64 = fields.next()?.parse().ok()?;
        let pending: u64 = fields.next()?.parse().ok()?;
        let synced_at = fields.next().and_then(|value| value.parse().ok());
        Some(Self {
            total,
            pending: pending.min(total),
            synced_at,
        })
    }

    fn write(self, home: &Path) {
        let synced_at = self.synced_at.map(|at| at.to_string()).unwrap_or_default();
        // A lost cache only means the next launch shows no balance until the
        // wallet reads the chain, so a failed write is not an error.
        let _ = std::fs::write(
            home.join(BALANCE_FILE),
            format!("{} {} {synced_at}\n", self.total, self.pending),
        );
    }
}

pub struct Wallet {
    home: PathBuf,
    opener: Opener,
    seed: Option<Seed>,
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
    pub balance_sats: u64,
    /// "12,345 sats".
    pub balance: String,
    /// "0.00012345 tBTC".
    pub balance_btc: String,
    /// Received but not yet confirmed, in sats.
    pub pending_sats: u64,
    pub pending: Option<String>,
    /// No coins yet: the screen explains how to get some.
    pub empty: bool,
    pub address: String,
    /// The BIP21 URI the QR code carries.
    pub uri: String,
    pub qr: Option<crate::app::QrModules>,
    pub faucet: &'static str,
    /// When the last sync finished, in Unix seconds.
    pub synced_at: Option<u64>,
    pub refreshing: bool,
    /// A failed refresh; the balance shown is the last one read.
    pub error: Option<String>,
    /// No balance has been read on this phone yet: the balance fields are
    /// empty and the screen shows placeholders.
    pub balance_unknown: bool,
    /// What the wallet is doing while it starts or first reads the chain,
    /// shown beside a progress indicator; the rest of the screen stays.
    pub status: Option<String>,
}

impl Wallet {
    pub fn new(home: PathBuf, opener: Opener) -> Self {
        // The saved address and last balance show before the wallet starts.
        let shared = Shared {
            address: saved_address(&home),
            last: LastBalance::read(&home),
            ..Shared::default()
        };
        Self {
            home,
            opener,
            seed: None,
            shared: Arc::new(Mutex::new(shared)),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Shared> {
        self.shared
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    /// Take the wallet key and start the wallet. A later call with the same
    /// key only retries a failed start; another key is refused, since one
    /// store belongs to one key.
    pub fn open(&mut self, entropy_hex: &str) {
        let mnemonic = decode(entropy_hex)
            .and_then(|entropy| mnemonic_from_entropy(&entropy).map_err(|error| describe(&error)));
        let mnemonic = match mnemonic {
            Ok(mnemonic) => mnemonic,
            Err(message) => {
                self.lock().error = Some(message);
                return;
            }
        };
        match &self.seed {
            Some(seed) if seed.0 != mnemonic => {
                self.lock().error = Some("This wallet already has a different key.".into());
                return;
            }
            Some(_) => {}
            None => self.seed = Some(Seed(mnemonic)),
        }
        self.start();
    }

    /// Start the wallet in the background, unless it runs or is starting.
    fn start(&mut self) {
        let Some(seed) = &self.seed else {
            return;
        };
        {
            let mut shared = self.lock();
            if shared.node.is_some() || shared.starting {
                return;
            }
            shared.starting = true;
            shared.error = None;
        }
        let (home, opener, shared) = (self.home.clone(), self.opener.clone(), self.shared.clone());
        let mnemonic = seed.0.clone();
        std::thread::spawn(move || {
            let opened = config().and_then(|config| {
                std::fs::create_dir_all(&home).map_err(|_| {
                    "The wallet's folder could not be created on this phone.".to_string()
                })?;
                let node = opener(&home, &config, &mnemonic)?;
                let address = receive_address(&home, node.as_ref())?;
                Ok((node, address))
            });
            drop(mnemonic);
            let node = {
                let mut state = shared.lock().unwrap_or_else(|poison| poison.into_inner());
                state.starting = false;
                match opened {
                    Ok((node, address)) => {
                        state.node = Some(node.clone());
                        state.address = Some(address);
                        state.refreshing = true;
                        node
                    }
                    Err(message) => {
                        state.error = Some(message);
                        return;
                    }
                }
            };
            let outcome = node.sync();
            finish_sync(&shared, &home, node.as_ref(), outcome);
        });
    }

    /// Read the chain again: sync a running wallet, or restart one that
    /// failed to start.
    pub fn refresh(&mut self) {
        let node = {
            let mut shared = self.lock();
            match shared.node.clone() {
                Some(node) if !shared.refreshing => {
                    shared.refreshing = true;
                    node
                }
                Some(_) => return,
                None => {
                    drop(shared);
                    self.start();
                    return;
                }
            }
        };
        let (shared, home) = (self.shared.clone(), self.home.clone());
        std::thread::spawn(move || {
            let outcome = node.sync();
            finish_sync(&shared, &home, node.as_ref(), outcome);
        });
    }

    /// Refresh a wallet that has its key; the app came to the foreground.
    pub fn refresh_if_open(&mut self) {
        if self.seed.is_some() {
            self.refresh();
        }
    }

    /// Whether a start or a sync is running in the background.
    pub fn loading(&self) -> bool {
        let shared = self.lock();
        shared.starting || shared.refreshing
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
            (None, _) => Some("Starting the wallet…"),
            (Some(_), _) if !shared.synced && shared.refreshing => Some("Reading Mutinynet…"),
            (Some(_), _) => None,
        };
        let address = shared.address.clone().unwrap_or_default();
        let mut error = shared.error.clone();
        // Until this launch reads the chain, show the last balance read.
        let live = match (&shared.node, status) {
            (Some(node), None) => match node.balance() {
                Ok(balance) => Some(LastBalance::from_node(&balance, node.synced_at())),
                Err(node_error) => {
                    error = Some(describe(&node_error));
                    None
                }
            },
            _ => None,
        };
        if status.is_none() && !shared.synced && error.is_none() {
            error = Some("The wallet has not read Mutinynet yet.".into());
        }
        let shown = live.or(shared.last);
        let (total, pending) = shown.map_or((0, 0), |last| (last.total, last.pending));
        let uri = if address.is_empty() {
            String::new()
        } else {
            format!("bitcoin:{address}")
        };
        Screen::Ready(Box::new(Summary {
            network: "Mutinynet signet",
            balance_sats: total,
            balance: shown.map(|_| sats(total)).unwrap_or_default(),
            balance_btc: shown
                .map(|_| format!("{} tBTC", btc(total)))
                .unwrap_or_default(),
            pending_sats: pending,
            pending: (pending > 0).then(|| format!("{} waiting for a confirmation", sats(pending))),
            empty: shown.is_some() && total == 0,
            // Upper case fits the QR code's compact alphanumeric mode;
            // BIP21 schemes and bech32 addresses are case-insensitive.
            qr: (!uri.is_empty())
                .then(|| qr(&uri.to_ascii_uppercase()))
                .flatten(),
            address,
            uri,
            faucet: FAUCET,
            synced_at: shown.and_then(|last| last.synced_at),
            refreshing: shared.refreshing || shared.starting,
            error,
            balance_unknown: shown.is_none(),
            status: status.map(str::to_owned),
        }))
    }
}

impl LastBalance {
    fn from_node(balance: &Balance, synced_at: Option<u64>) -> Self {
        let total = balance
            .onchain_total_sats
            .saturating_add(balance.lightning_total_sats);
        let settled = balance
            .onchain_spendable_sats
            .saturating_add(balance.anchor_reserve_sats)
            .saturating_add(balance.lightning_total_sats);
        Self {
            total,
            pending: total.saturating_sub(settled),
            synced_at,
        }
    }
}

fn finish_sync(
    shared: &Mutex<Shared>,
    home: &Path,
    node: &dyn Node,
    outcome: Result<(), WalletError>,
) {
    // Save what this sync read, for the next launch to show at once.
    let read = outcome
        .is_ok()
        .then(|| node.balance().ok())
        .flatten()
        .map(|balance| LastBalance::from_node(&balance, node.synced_at()));
    if let Some(read) = read {
        read.write(home);
    }
    let mut state = shared.lock().unwrap_or_else(|poison| poison.into_inner());
    state.refreshing = false;
    match outcome {
        Ok(()) => {
            state.synced = true;
            state.error = None;
            if read.is_some() {
                state.last = read;
            }
        }
        Err(error) => state.error = Some(describe(&error)),
    }
}

/// The saved receive address, if a test-network one was saved.
fn saved_address(home: &Path) -> Option<String> {
    let saved = std::fs::read_to_string(home.join(ADDRESS_FILE)).ok()?;
    let saved = saved.trim();
    test_address(saved).then(|| saved.to_owned())
}

/// The address the screen shows: the saved one, or a new one saved now.
fn receive_address(home: &Path, node: &dyn Node) -> Result<String, String> {
    if let Some(saved) = saved_address(home) {
        return Ok(saved);
    }
    let path = home.join(ADDRESS_FILE);
    let address = node.new_address().map_err(|error| describe(&error))?;
    if !test_address(&address) {
        return Err("The wallet produced an address that is not for a test network.".into());
    }
    std::fs::write(&path, &address)
        .map_err(|_| "The wallet could not save its address on this phone.".to_string())?;
    Ok(address)
}

fn decode(hex_text: &str) -> Result<Vec<u8>, String> {
    let bytes = (hex_text.len() == 64 && hex_text.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| {
            (0..32)
                .map(|i| u8::from_str_radix(&hex_text[i * 2..i * 2 + 2], 16).ok())
                .collect::<Option<Vec<u8>>>()
        })
        .flatten();
    bytes.ok_or_else(|| "The wallet key is unreadable.".to_string())
}

/// A wallet error for the screen. Node errors carry `ldk-node`'s own text,
/// which names no key material.
fn describe(error: &WalletError) -> String {
    match error {
        WalletError::Node(detail) => match detail.split_once(": ") {
            Some(("start", reason)) => {
                format!("The wallet could not start ({reason}). Check the connection and refresh.")
            }
            Some(("sync", reason)) => {
                format!("Mutinynet could not be read ({reason}). Refresh to try again.")
            }
            _ => detail.clone(),
        },
        other => other.to_string(),
    }
}

/// "12,345 sats", with thousands separators.
pub fn sats(value: u64) -> String {
    let digits = value.to_string();
    let mut grouped = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    if value == 1 {
        "1 sat".into()
    } else {
        format!("{grouped} sats")
    }
}

/// Bitcoin with eight decimals: "0.00012345".
pub fn btc(value: u64) -> String {
    format!("{}.{:08}", value / 100_000_000, value % 100_000_000)
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
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, Instant};

    const ENTROPY: &str = "5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a";

    struct Fake {
        total: AtomicU64,
        pending: AtomicU64,
        sync_fails: bool,
        addresses: AtomicU64,
    }

    impl Node for Fake {
        fn balance(&self) -> Result<Balance, WalletError> {
            let total = self.total.load(Ordering::SeqCst);
            Ok(Balance {
                onchain_total_sats: total,
                onchain_spendable_sats: total - self.pending.load(Ordering::SeqCst),
                lightning_total_sats: 0,
                anchor_reserve_sats: 0,
            })
        }
        fn new_address(&self) -> Result<String, WalletError> {
            let n = self.addresses.fetch_add(1, Ordering::SeqCst);
            Ok(format!("tb1qfake{n}"))
        }
        fn sync(&self) -> Result<(), WalletError> {
            if self.sync_fails {
                Err(WalletError::Node("sync: WalletOperationFailed".into()))
            } else {
                Ok(())
            }
        }
        fn synced_at(&self) -> Option<u64> {
            (!self.sync_fails).then_some(1_790_000_000)
        }
    }

    fn fake(total: u64, pending: u64, sync_fails: bool) -> Arc<Fake> {
        Arc::new(Fake {
            total: AtomicU64::new(total),
            pending: AtomicU64::new(pending),
            sync_fails,
            addresses: AtomicU64::new(0),
        })
    }

    fn opener(node: Arc<Fake>, seen: Arc<Mutex<Vec<(Network, String)>>>) -> Opener {
        Arc::new(move |_home, config, mnemonic| {
            seen.lock()
                .unwrap()
                .push((config.network, mnemonic.to_owned()));
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

    #[test]
    fn the_phone_wallet_is_signet_only_on_mutinynet() {
        let config = config().expect("config");
        assert_eq!(config.network, Network::Signet);
        assert_eq!(config.esplora_url, "https://mutinynet.com/api");
        assert_eq!(config.listen, None);
        assert_eq!(config.lsp, None);
        assert!(config.trusted_peers.is_empty());
        let mut mainnet = config.clone();
        mainnet.network = Network::Bitcoin;
        assert!(
            admit(&mainnet).is_err(),
            "a mainnet configuration is refused"
        );
        assert!(!test_address("bc1qexample"));
        assert!(test_address("tb1qexample"));

        // The real wallet, built without reaching the network, derives
        // signet addresses from a key.
        let home = tempfile::tempdir().expect("temp dir");
        let mnemonic = mnemonic_from_entropy(&[0x5a; 32]).expect("mnemonic");
        let (_, addresses) =
            LdkWallet::identity(home.path(), &config, &mnemonic, 2).expect("identity");
        for address in addresses {
            assert!(address.starts_with("tb1"), "{address} is a signet address");
        }
    }

    #[test]
    fn a_key_opens_the_wallet_and_a_funded_one_shows_its_balance() {
        let home = tempfile::tempdir().expect("temp dir");
        let node = fake(0, 0, false);
        let seen = Arc::new(Mutex::new(vec![]));
        let mut wallet = Wallet::new(
            home.path().join("wallet"),
            opener(node.clone(), seen.clone()),
        );
        // Before the key arrives the screen is already the wallet's, with
        // placeholders where nothing has been read yet.
        let Screen::Ready(opening) = wallet.screen() else {
            panic!("the wallet screen shows while it opens");
        };
        assert!(opening.balance_unknown && opening.address.is_empty() && opening.qr.is_none());
        assert_eq!(opening.status.as_deref(), Some("Opening the wallet…"));
        wallet.open(ENTROPY);
        settle(&wallet);
        let Screen::Ready(empty) = wallet.screen() else {
            panic!("ready: {:?}", wallet.screen());
        };
        assert!(empty.empty);
        assert_eq!(empty.balance, "0 sats");
        assert_eq!(empty.address, "tb1qfake0");
        assert_eq!(empty.uri, "bitcoin:tb1qfake0");
        assert!(empty.qr.is_some());
        assert_eq!(empty.faucet, FAUCET);
        assert_eq!(empty.error, None);
        let (network, mnemonic) = seen.lock().unwrap()[0].clone();
        assert_eq!(network, Network::Signet);
        assert_eq!(mnemonic.split_whitespace().count(), 24);

        node.total.store(123_456, Ordering::SeqCst);
        node.pending.store(10_000, Ordering::SeqCst);
        wallet.refresh();
        settle(&wallet);
        let Screen::Ready(funded) = wallet.screen() else {
            panic!("ready");
        };
        assert!(!funded.empty);
        assert_eq!(funded.balance, "123,456 sats");
        assert_eq!(funded.balance_btc, "0.00123456 tBTC");
        assert_eq!(
            funded.pending.as_deref(),
            Some("10,000 sats waiting for a confirmation")
        );
        assert_eq!(funded.address, "tb1qfake0", "the address stays put");

        // A new lifetime shows the saved address and the last balance read
        // before its wallet starts, then reuses that address.
        let mut again = Wallet::new(home.path().join("wallet"), opener(node.clone(), seen));
        let Screen::Ready(cached) = again.screen() else {
            panic!("the cached wallet shows at once");
        };
        assert_eq!(cached.balance, "123,456 sats");
        assert_eq!(cached.address, "tb1qfake0");
        assert!(cached.qr.is_some() && !cached.balance_unknown);
        assert_eq!(cached.status.as_deref(), Some("Opening the wallet…"));
        again.open(ENTROPY);
        settle(&again);
        let Screen::Ready(reopened) = again.screen() else {
            panic!("ready");
        };
        assert_eq!(reopened.address, "tb1qfake0");
        assert_eq!(node.addresses.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn failures_are_typed_and_never_carry_the_key() {
        let home = tempfile::tempdir().expect("temp dir");
        let mut wallet = Wallet::new(
            home.path().to_path_buf(),
            Arc::new(|_, _, _| Err("Mutinynet could not be reached.".to_string())),
        );
        wallet.open("zz");
        assert_eq!(
            wallet.screen(),
            Screen::Failed {
                message: "The wallet key is unreadable.".into()
            }
        );
        wallet.open(ENTROPY);
        settle(&wallet);
        assert_eq!(
            wallet.screen(),
            Screen::Failed {
                message: "Mutinynet could not be reached.".into()
            }
        );

        // A wallet that starts but cannot sync shows what it has, and why.
        let node = fake(5, 0, true);
        let mut syncless = Wallet::new(
            home.path().join("syncless"),
            opener(node, Arc::new(Mutex::new(vec![]))),
        );
        syncless.open(ENTROPY);
        settle(&syncless);
        let Screen::Ready(summary) = syncless.screen() else {
            panic!("ready: {:?}", syncless.screen());
        };
        assert_eq!(summary.balance, "5 sats");
        assert!(summary.error.as_deref().unwrap().contains("Mutinynet"));
        let other = ENTROPY.replace('5', "6");
        syncless.open(&other);
        assert!(
            matches!(syncless.screen(), Screen::Ready(summary) if summary.error.as_deref().is_some_and(|e| e.contains("different key")))
        );

        let mnemonic = mnemonic_from_entropy(&[0x5a; 32]).expect("mnemonic");
        let first_word = mnemonic.split_whitespace().next().unwrap().to_owned();
        for screen in [wallet.screen(), syncless.screen()] {
            let json = serde_json::to_string(&screen).unwrap();
            assert!(
                !json.contains(ENTROPY) && !json.contains(&mnemonic),
                "{json}"
            );
            assert!(!json.contains(&format!("\"{first_word}")), "{json}");
        }
    }

    #[test]
    fn amounts_read_plainly() {
        assert_eq!(sats(0), "0 sats");
        assert_eq!(sats(1), "1 sat");
        assert_eq!(sats(999), "999 sats");
        assert_eq!(sats(1_000), "1,000 sats");
        assert_eq!(sats(21_000_000), "21,000,000 sats");
        assert_eq!(btc(0), "0.00000000");
        assert_eq!(btc(150_000_000), "1.50000000");
    }
}
