use super::seed::{self, KeyAdd, KeyRead, KeyStore, MemoryStore, SeedError, WalletWords};
use super::*;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

/// BIP-39's published test vectors for all-zero entropy.
const TWELVE_ZERO: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const TWENTY_FOUR_ZERO: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art";

/// An opener that records each mnemonic it is given and opens the offline
/// fixture.
fn recording(seen: Arc<Mutex<Vec<String>>>) -> Opener {
    Arc::new(move |_home, mnemonic| {
        seen.lock().unwrap().push(mnemonic.to_owned());
        Ok(Arc::new(fixture::Fixture::default()) as Arc<dyn Node>)
    })
}

struct Harness {
    wallet: Wallet,
    store: Arc<MemoryStore>,
    seen: Arc<Mutex<Vec<String>>>,
    _home: tempfile::TempDir,
}

fn harness(store: MemoryStore) -> Harness {
    let home = tempfile::tempdir().unwrap();
    let store = Arc::new(store);
    let seen = Arc::new(Mutex::new(vec![]));
    let wallet = Wallet::new(
        home.path().join("spark"),
        store.clone(),
        recording(seen.clone()),
        Arc::new(|| {}),
    );
    Harness {
        wallet,
        store,
        seen,
        _home: home,
    }
}

fn settle(wallet: &Wallet) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        {
            let state = wallet.lock();
            let quiet = !state.starting
                && !state.refreshing
                && !state.invoice_busy
                && !matches!(state.send, Sending::Quoting | Sending::Paying(_));
            if quiet && (state.node.is_some() || state.seed.is_none() || state.error.is_some()) {
                return;
            }
        }
        assert!(Instant::now() < deadline, "the wallet didn't settle");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn keys(wallet: &Wallet) -> Vec<String> {
    fn walk(node: &ViewNode<Action>, out: &mut Vec<String>) {
        out.push(node.key.clone());
        if let Element::Stack { children, .. } = &node.element {
            for child in children {
                walk(child, out);
            }
        }
    }
    let mut out = vec![];
    walk(&wallet.view().root, &mut out);
    out
}

fn texts(wallet: &Wallet) -> Vec<String> {
    fn walk(node: &ViewNode<Action>, out: &mut Vec<String>) {
        match &node.element {
            Element::Stack { children, .. } => children.iter().for_each(|c| walk(c, out)),
            Element::Text { value, .. } => out.push(value.clone()),
            Element::Button { label, .. } => out.push(label.clone()),
            Element::Field {
                label, placeholder, ..
            } => {
                out.push(label.clone());
                out.push(placeholder.clone());
            }
            _ => {}
        }
    }
    let mut out = vec![];
    walk(&wallet.view().root, &mut out);
    out
}

fn tap(wallet: &Wallet, key: &str) -> Effect {
    let view = wallet.view();
    wallet
        .activate(&Activation {
            instance: view.instance,
            revision: view.revision,
            node: key.into(),
        })
        .unwrap_or_else(|error| panic!("{key}: {error}"))
}

fn type_in(wallet: &Wallet, key: &str, value: &str) {
    let view = wallet.view();
    wallet
        .change_field(&FieldChange {
            activation: Activation {
                instance: view.instance,
                revision: view.revision,
                node: key.into(),
            },
            value: value.into(),
        })
        .unwrap_or_else(|error| panic!("{key}: {error}"));
}

fn mnemonic(entropy: &[u8]) -> String {
    Seed::from_entropy(entropy.to_vec()).unwrap().mnemonic
}

#[test]
fn an_existing_seed_is_used_unchanged_and_never_written() {
    for entropy in [vec![0x5a_u8; 16], (0..32).collect::<Vec<u8>>()] {
        let h = harness(MemoryStore::holding(entropy.clone()));
        h.wallet.open();
        settle(&h.wallet);
        assert_eq!(*h.seen.lock().unwrap(), vec![mnemonic(&entropy)]);
        assert_eq!(h.store.stored(), Some(entropy.clone()));
        assert!(h.store.adds.lock().unwrap().is_empty(), "nothing was added");
        let keys = keys(&h.wallet);
        assert!(keys.contains(&"balance".to_owned()), "{keys:?}");
        for refused in ["create", "restore-open", "restore"] {
            assert!(!keys.contains(&refused.to_owned()), "{refused} offered");
        }
        assert_eq!(
            h.wallet.words().unwrap().len(),
            if entropy.len() == 16 { 12 } else { 24 }
        );
        // Opening again neither reads a new seed nor starts a second wallet.
        h.wallet.open();
        h.wallet.refresh();
        settle(&h.wallet);
        assert_eq!(h.seen.lock().unwrap().len(), 1);
        assert_eq!(h.store.stored(), Some(entropy));
    }
}

#[test]
fn the_existing_seeds_words_are_offered_to_the_identity() {
    let store = MemoryStore::holding(vec![0; 16]);
    match seed::words(&store) {
        WalletWords::Words { words } => assert_eq!(words.join(" "), TWELVE_ZERO),
        _ => panic!("expected the stored words"),
    }
    let store = MemoryStore::holding(vec![0; 32]);
    match seed::wallet_seed_words(Arc::new(MemoryStore::holding(vec![0; 32]))) {
        WalletWords::Words { words } => assert_eq!(words.join(" "), TWENTY_FOUR_ZERO),
        _ => panic!("expected the stored words"),
    }
    assert!(store.adds.lock().unwrap().is_empty());
    assert!(matches!(
        seed::words(&MemoryStore::default()),
        WalletWords::NoWallet
    ));
    let locked = MemoryStore::holding(vec![0; 16]);
    locked.locked.store(true, Ordering::SeqCst);
    assert!(matches!(seed::words(&locked), WalletWords::Unavailable));
    let odd = MemoryStore::holding(vec![1; 5]);
    assert!(matches!(seed::words(&odd), WalletWords::Unreadable));
    for store in [&locked, &odd] {
        assert!(store.adds.lock().unwrap().is_empty());
    }
}

#[test]
fn a_locked_key_store_says_so_and_never_makes_a_seed() {
    let entropy = vec![0x33; 16];
    let store = MemoryStore::holding(entropy.clone());
    store.locked.store(true, Ordering::SeqCst);
    let h = harness(store);
    h.wallet.open();
    let texts = texts(&h.wallet);
    assert!(texts.contains(&seed::UNAVAILABLE.to_owned()), "{texts:?}");
    let keys = keys(&h.wallet);
    assert!(keys.contains(&"retry".to_owned()));
    assert!(!keys.contains(&"create".to_owned()));
    assert!(!keys.contains(&"restore-open".to_owned()));
    assert!(h.seen.lock().unwrap().is_empty(), "no wallet started");
    // Locked and empty: making a seed is refused too.
    let empty = MemoryStore::default();
    empty.locked.store(true, Ordering::SeqCst);
    assert!(matches!(seed::create(&empty), Err(SeedError::Unavailable)));
    assert!(matches!(
        seed::restore(&empty, TWELVE_ZERO),
        Err(SeedError::Unavailable)
    ));
    assert!(empty.adds.lock().unwrap().is_empty());
    // Unlocked, Try again opens the same seed.
    h.store.locked.store(false, Ordering::SeqCst);
    tap(&h.wallet, "retry");
    settle(&h.wallet);
    assert_eq!(*h.seen.lock().unwrap(), vec![mnemonic(&entropy)]);
    assert_eq!(h.store.stored(), Some(entropy));
    assert!(h.store.adds.lock().unwrap().is_empty());
}

#[test]
fn an_unreadable_item_is_left_alone() {
    let h = harness(MemoryStore::holding(vec![9; 20]));
    h.wallet.open();
    assert!(texts(&h.wallet).contains(&seed::UNREADABLE.to_owned()));
    assert!(!keys(&h.wallet).contains(&"create".to_owned()));
    assert!(matches!(
        seed::create(h.store.as_ref()),
        Err(SeedError::Unreadable)
    ));
    assert!(matches!(
        seed::restore(h.store.as_ref(), TWELVE_ZERO),
        Err(SeedError::Unreadable)
    ));
    assert!(h.store.adds.lock().unwrap().is_empty());
    assert_eq!(h.store.stored(), Some(vec![9; 20]));
}

#[test]
fn a_missing_seed_is_made_only_when_asked_after_its_absence_is_certain() {
    let h = harness(MemoryStore::default());
    h.wallet.open();
    assert!(
        h.store.adds.lock().unwrap().is_empty(),
        "opening makes nothing"
    );
    assert!(h.seen.lock().unwrap().is_empty());
    let keys = keys(&h.wallet);
    assert!(keys.contains(&"create".to_owned()), "{keys:?}");
    assert!(keys.contains(&"restore-open".to_owned()));
    tap(&h.wallet, "create");
    settle(&h.wallet);
    let stored = h.store.stored().expect("a seed");
    assert_eq!(stored.len(), 16);
    assert_eq!(h.store.adds.lock().unwrap().len(), 1);
    assert_eq!(*h.seen.lock().unwrap(), vec![mnemonic(&stored)]);
    // A new wallet asks for a backup.
    assert!(self::keys(&h.wallet).contains(&"backup-card".to_owned()));
}

/// Reads say "not found", but an item appears before the add: the stored
/// item wins and is never replaced.
struct Racing {
    inner: MemoryStore,
    first: std::sync::atomic::AtomicBool,
}

impl KeyStore for Racing {
    fn read_seed(&self) -> KeyRead {
        if self.first.swap(false, Ordering::SeqCst) {
            return KeyRead::NotFound;
        }
        self.inner.read_seed()
    }
    fn add_seed(&self, entropy: Vec<u8>) -> KeyAdd {
        self.inner.add_seed(entropy)
    }
}

#[test]
fn making_a_seed_never_replaces_one_that_appeared() {
    let existing = vec![0x44; 32];
    let racing = Racing {
        inner: MemoryStore::holding(existing.clone()),
        first: true.into(),
    };
    assert!(matches!(seed::create(&racing), Err(SeedError::Exists)));
    assert_eq!(racing.inner.stored(), Some(existing.clone()));
    // Through the screen: the wallet then opens the stored seed.
    let home = tempfile::tempdir().unwrap();
    let seen = Arc::new(Mutex::new(vec![]));
    let store = Arc::new(Racing {
        inner: MemoryStore::holding(existing.clone()),
        first: true.into(),
    });
    let wallet = Wallet::new(
        home.path().to_path_buf(),
        store.clone(),
        recording(seen.clone()),
        Arc::new(|| {}),
    );
    wallet.open();
    tap(&wallet, "create");
    settle(&wallet);
    assert_eq!(*seen.lock().unwrap(), vec![mnemonic(&existing)]);
    assert_eq!(store.inner.stored(), Some(existing));
}

#[test]
fn recovery_words_restore_12_and_24_word_wallets() {
    for (words, entropy) in [(TWELVE_ZERO, vec![0; 16]), (TWENTY_FOUR_ZERO, vec![0; 32])] {
        let h = harness(MemoryStore::default());
        h.wallet.open();
        tap(&h.wallet, "restore-open");
        // Words are read in any case and spacing.
        type_in(
            &h.wallet,
            "restore-words",
            &format!("  {}\n", words.to_uppercase()),
        );
        tap(&h.wallet, "restore");
        settle(&h.wallet);
        assert_eq!(h.store.stored(), Some(entropy));
        assert_eq!(*h.seen.lock().unwrap(), vec![words.to_owned()]);
        // Typed words count as saved, so no backup card.
        assert!(!keys(&h.wallet).contains(&"backup-card".to_owned()));
    }
}

#[test]
fn bad_words_are_refused_without_repeating_them() {
    let h = harness(MemoryStore::default());
    h.wallet.open();
    tap(&h.wallet, "restore-open");
    let wrong = TWELVE_ZERO.replace("about", "abandon");
    type_in(&h.wallet, "restore-words", &wrong);
    tap(&h.wallet, "restore");
    let texts = texts(&h.wallet);
    let error = texts
        .iter()
        .find(|text| text.contains("valid recovery phrase"))
        .unwrap_or_else(|| panic!("{texts:?}"));
    assert!(!error.contains("abandon"));
    assert!(h.store.stored().is_none());
    assert!(h.store.adds.lock().unwrap().is_empty());
}

#[test]
fn restoring_is_refused_while_a_wallet_exists() {
    let existing = vec![0x21; 16];
    let store = MemoryStore::holding(existing.clone());
    assert!(matches!(
        seed::restore(&store, TWELVE_ZERO),
        Err(SeedError::Exists)
    ));
    assert!(matches!(seed::create(&store), Err(SeedError::Exists)));
    assert!(store.adds.lock().unwrap().is_empty());
    assert_eq!(store.stored(), Some(existing));
}

#[test]
fn the_wallet_opens_on_a_fixture_seed() {
    let home = tempfile::tempdir().unwrap();
    let wallet = Wallet::new(
        home.path().to_path_buf(),
        Arc::new(MemoryStore::holding(fixture::ENTROPY.to_vec())),
        fixture::opener(),
        Arc::new(|| {}),
    );
    wallet.open();
    settle(&wallet);
    let texts = texts(&wallet);
    for expected in ["₿250,000", fixture::SPARK, "Receive", "Send", "Activity"] {
        assert!(texts.iter().any(|t| t == expected), "{expected}: {texts:?}");
    }
    assert!(
        texts.iter().any(|t| t.starts_with("Received +₿262,003")),
        "{texts:?}"
    );
    assert_eq!(
        tap(&wallet, "copy"),
        Effect::Copy {
            text: fixture::SPARK.into()
        }
    );
    tap(&wallet, "receive-bitcoin");
    assert!(self::texts(&wallet).contains(&fixture::BITCOIN.to_owned()));
    assert_eq!(tap(&wallet, "backup"), Effect::Backup);
    // The balance survives a relaunch before the wallet reads Spark again.
    let again = Wallet::new(
        home.path().to_path_buf(),
        Arc::new(MemoryStore::default()),
        fixture::opener(),
        Arc::new(|| {}),
    );
    assert_eq!(again.lock().balance, Some(250_000));
}

#[test]
fn receive_makes_invoices_and_send_pays_the_confirmed_quote_once() {
    let h = harness(MemoryStore::holding(fixture::ENTROPY.to_vec()));
    h.wallet.open();
    settle(&h.wallet);
    tap(&h.wallet, "receive-lightning");
    type_in(&h.wallet, "invoice-amount", "2100");
    tap(&h.wallet, "invoice");
    settle(&h.wallet);
    let texts = texts(&h.wallet);
    assert!(
        texts.contains(&"Request for ₿2,100".to_owned()),
        "{texts:?}"
    );
    assert!(texts.iter().any(|t| t.starts_with("lnbc2100n1")));

    type_in(&h.wallet, "send-to", "lnbc1000n1pfixture");
    tap(&h.wallet, "review");
    settle(&h.wallet);
    let texts = self::texts(&h.wallet);
    assert!(
        texts.iter().any(|t| t.contains("Fee ₿2. Total ₿1,002.")),
        "{texts:?}"
    );
    tap(&h.wallet, "pay");
    settle(&h.wallet);
    assert!(self::texts(&h.wallet).contains(&"Sent ₿1,000.".to_owned()));
    // The quote is gone; paying again does nothing.
    assert!(!keys(&h.wallet).contains(&"pay".to_owned()));
    tap(&h.wallet, "send-done");
    assert_eq!(h.wallet.lock().balance, Some(250_000 - 1_002));
}

#[test]
fn the_wallet_screen_passes_the_copy_guard() {
    let mut seen = vec![];
    // Locked, unreadable, empty, restoring, and ready with the trust note
    // and every receive and send state.
    let locked = MemoryStore::holding(vec![1; 16]);
    locked.locked.store(true, Ordering::SeqCst);
    for store in [locked, MemoryStore::holding(vec![1; 3])] {
        let h = harness(store);
        h.wallet.open();
        seen.extend(texts(&h.wallet));
    }
    let h = harness(MemoryStore::default());
    h.wallet.open();
    seen.extend(texts(&h.wallet));
    tap(&h.wallet, "restore-open");
    seen.extend(texts(&h.wallet));
    let h = harness(MemoryStore::holding(fixture::ENTROPY.to_vec()));
    h.wallet.open();
    settle(&h.wallet);
    seen.extend(texts(&h.wallet));
    tap(&h.wallet, "trust");
    seen.extend(texts(&h.wallet));
    for key in ["receive-lightning", "receive-bitcoin"] {
        tap(&h.wallet, key);
        seen.extend(texts(&h.wallet));
    }
    type_in(&h.wallet, "send-to", "bc1qexample");
    tap(&h.wallet, "review");
    settle(&h.wallet);
    seen.extend(texts(&h.wallet));
    seen.extend(TRUST_LINES.iter().map(|line| (*line).to_owned()));
    for error in [
        SeedError::Exists,
        SeedError::Unavailable,
        SeedError::Unreadable,
        SeedError::NotSaved,
    ] {
        seen.push(error.message());
    }
    for text in seen {
        assert!(oa_copy::violations(&text, &[]).is_empty(), "{text}");
    }
}

#[test]
fn the_keychain_item_is_the_one_the_earlier_app_wrote() {
    assert_eq!(seed::KEYCHAIN_SERVICE, "com.openagents.app.spark");
    assert_eq!(seed::KEYCHAIN_ACCOUNT, "seed-v1");
    assert!(matches!(NETWORK, openagents_spark::Network::Mainnet));
}

fn identity_source(answer: seed::IdentityEntropy) -> (seed::IdentitySource, Arc<Mutex<usize>>) {
    let reads = Arc::new(Mutex::new(0));
    let counted = reads.clone();
    let source: seed::IdentitySource = Arc::new(move || {
        *counted.lock().unwrap() += 1;
        answer.clone()
    });
    (source, reads)
}

/// "Make a wallet" uses the identity's words, so one backup covers both
/// (#11995).
#[test]
fn a_new_wallet_is_made_from_the_identity_words() {
    let (source, _) = identity_source(seed::IdentityEntropy::Found(vec![0; 16]));
    let home = tempfile::tempdir().unwrap();
    let store = Arc::new(MemoryStore::default());
    let seen = Arc::new(Mutex::new(vec![]));
    let wallet = Wallet::new(
        home.path().join("spark"),
        store.clone(),
        recording(seen.clone()),
        Arc::new(|| {}),
    )
    .with_identity(source);
    wallet.open();
    tap(&wallet, "create");
    settle(&wallet);
    assert_eq!(store.stored(), Some(vec![0; 16]));
    assert_eq!(store.adds.lock().unwrap().len(), 1);
    assert_eq!(*seen.lock().unwrap(), vec![mnemonic(&[0; 16])]);
    assert_eq!(wallet.words().unwrap().join(" "), TWELVE_ZERO);
}

#[test]
fn an_existing_wallet_seed_is_kept_and_the_identity_is_not_used() {
    let (source, reads) = identity_source(seed::IdentityEntropy::Found(vec![0; 16]));
    let store = MemoryStore::holding(vec![7; 32]);
    assert!(matches!(
        seed::create_with(&store, Some(&source)),
        Err(SeedError::Exists)
    ));
    assert_eq!(*reads.lock().unwrap(), 0);
    assert!(store.adds.lock().unwrap().is_empty());
    assert_eq!(store.stored(), Some(vec![7; 32]));
}

#[test]
fn a_locked_identity_makes_no_wallet_and_no_identity_makes_new_words() {
    let (locked, _) = identity_source(seed::IdentityEntropy::Unavailable);
    let store = MemoryStore::default();
    assert!(matches!(
        seed::create_with(&store, Some(&locked)),
        Err(SeedError::Unavailable)
    ));
    assert!(store.adds.lock().unwrap().is_empty());

    // No identity yet: new words, which the identity adopts when it is made.
    let (absent, reads) = identity_source(seed::IdentityEntropy::Absent);
    let made = seed::create_with(&store, Some(&absent)).unwrap();
    assert_eq!(*reads.lock().unwrap(), 1);
    assert_eq!(made.entropy.len(), 16);
    assert_eq!(store.stored(), Some(made.entropy.clone()));
    // And a second make never replaces it.
    let (other, _) = identity_source(seed::IdentityEntropy::Found(vec![0; 16]));
    assert!(matches!(
        seed::create_with(&store, Some(&other)),
        Err(SeedError::Exists)
    ));
    assert_eq!(store.stored(), Some(made.entropy));
}
