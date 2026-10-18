use super::*;
use openagents_account::{
    identity::{Identities, Identity, IdentityVault, WalletRead, WalletSeed, WordAnswer},
    openagents::StorageError,
};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

#[derive(Default)]
struct Vault {
    entropy: Mutex<Option<Vec<u8>>>,
}
impl IdentityVault for Vault {
    fn load(&self) -> Result<Option<(Vec<u8>, bool)>, StorageError> {
        Ok(self.entropy.lock().unwrap().clone().map(|e| (e, false)))
    }
    fn store(&self, entropy: &[u8], _: bool) -> Result<(), StorageError> {
        *self.entropy.lock().unwrap() = Some(entropy.into());
        Ok(())
    }
}
struct Wallet(Vec<u8>);
impl WalletSeed for Wallet {
    fn read(&self) -> WalletRead {
        WalletRead::Found(self.0.clone())
    }
}
struct Fake {
    generation: AtomicU64,
    loaded: Mutex<Option<Identity>>,
    identities: Identities,
    vault: Arc<Vault>,
    locked: AtomicBool,
    switch_on_words: AtomicBool,
    word_reads: AtomicUsize,
    confirm_calls: AtomicUsize,
    start_calls: AtomicUsize,
    finished: AtomicBool,
    provider: String,
    _dir: tempfile::TempDir,
}
impl Fake {
    fn new() -> Arc<Self> {
        let dir = tempfile::tempdir().unwrap();
        let vault = Arc::new(Vault::default());
        let identities = Identities::new(vault.clone(), Arc::new(Wallet(vec![0; 16])), dir.path());
        let loaded = identities.load_or_create().unwrap();
        Arc::new(Self {
            generation: AtomicU64::new(5),
            loaded: Mutex::new(Some(loaded)),
            identities,
            vault,
            locked: AtomicBool::new(false),
            switch_on_words: AtomicBool::new(false),
            word_reads: AtomicUsize::new(0),
            confirm_calls: AtomicUsize::new(0),
            start_calls: AtomicUsize::new(0),
            finished: AtomicBool::new(false),
            provider: "synthetic-existing-provider".into(),
            _dir: dir,
        })
    }
    fn flow(self: &Arc<Self>) -> CoreAndroidRecovery {
        CoreAndroidRecovery {
            backend: self.clone(),
            state: Mutex::new(State::new(true)),
        }
    }
}
impl Backend for Fake {
    fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }
    fn identity(&self) -> Option<IdentityView> {
        self.loaded.lock().unwrap().as_ref().map(|i| IdentityView {
            npub: i.npub(),
            words_saved: self.identities.words_saved(i),
            icloud_copy: false,
        })
    }
    fn steps(&self) -> Vec<OnboardingStep> {
        super::super::onboarding::onboarding_steps()
    }
    fn should_show(&self) -> bool {
        !self.finished.load(Ordering::SeqCst)
    }
    fn finish(&self) {
        self.finished.store(true, Ordering::SeqCst);
    }
    fn start(&self, _: String) {
        self.start_calls.fetch_add(1, Ordering::SeqCst);
    }
    fn words(&self) -> Result<Vec<String>, CoreError> {
        self.word_reads.fetch_add(1, Ordering::SeqCst);
        if self.locked.load(Ordering::SeqCst) {
            return Err(CoreError::Closed);
        }
        if self.switch_on_words.load(Ordering::SeqCst) {
            self.generation.fetch_add(1, Ordering::SeqCst);
        }
        Ok(self.loaded.lock().unwrap().as_ref().unwrap().words())
    }
    fn positions(&self) -> Result<Vec<u32>, CoreError> {
        Ok(vec![2, 5, 11])
    }
    fn confirm(&self, answers: Vec<WordCheck>) -> Result<bool, CoreError> {
        self.confirm_calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.identities.confirm_backup(
            self.loaded.lock().unwrap().as_ref().unwrap(),
            &answers
                .into_iter()
                .map(|a| WordAnswer {
                    position: a.position,
                    word: a.word,
                })
                .collect::<Vec<_>>(),
        ))
    }
    fn restore(
        &self,
        words: String,
        generation: u64,
        npub: Option<String>,
    ) -> Result<IdentityView, CoreError> {
        if self.generation() != generation || self.identity().map(|i| i.npub) != npub {
            return Err(CoreError::Closed);
        }
        let identity = self
            .identities
            .restore(&words, false)
            .map_err(|_| problem("Couldn't restore your account."))?;
        *self.loaded.lock().unwrap() = Some(identity);
        Ok(self.identity().unwrap())
    }
}
fn mounted(flow: &CoreAndroidRecovery) -> u64 {
    let f = flow.render().unwrap();
    let j: serde_json::Value = serde_json::from_slice(&f.json).unwrap();
    let revision = j["revision"].as_u64().unwrap();
    flow.acknowledge("android-recovery".into(), revision)
        .unwrap();
    revision
}
fn press(flow: &CoreAndroidRecovery, key: &str) -> RecoveryFrame {
    let revision = mounted(flow);
    flow.activate("android-recovery".into(), revision, key.into())
        .unwrap()
}
fn home(flow: &CoreAndroidRecovery) {
    press(flow, "skip");
}
fn unlock(flow: &CoreAndroidRecovery, key: &str) -> RecoveryFrame {
    let frame = press(flow, key);
    assert!(frame.secure);
    flow.authenticated(frame.authentication.unwrap().token, true)
        .unwrap()
}
#[test]
fn tutorial_is_skippable_and_existing_identity_provider_and_seed_are_not_recreated() {
    let b = Fake::new();
    let f = b.flow();
    let before = b.vault.entropy.lock().unwrap().clone();
    f.start("fixture".into()).unwrap();
    home(&f);
    assert!(b.finished.load(Ordering::SeqCst));
    assert_eq!(b.start_calls.load(Ordering::SeqCst), 0);
    assert_eq!(before, *b.vault.entropy.lock().unwrap());
    assert_eq!(b.provider, "synthetic-existing-provider");
    assert!(f.render().unwrap().json.windows(7).any(|s| s == b"Your ID"));
}

#[test]
fn completed_onboarding_start_and_reentry_stay_on_your_id_without_recreating_identity() {
    let b = Fake::new();
    b.finished.store(true, Ordering::SeqCst);
    let identity = b.identity().unwrap().npub;
    let seed = b.vault.entropy.lock().unwrap().clone();
    let f = b.flow();

    let startup = f.start("fixture".into()).unwrap();
    assert!(!startup.open_chat);
    let reentry = f.start("fixture".into()).unwrap();
    assert!(!reentry.open_chat);
    assert!(String::from_utf8(reentry.json).unwrap().contains("Your ID"));
    assert_eq!(b.start_calls.load(Ordering::SeqCst), 0);
    assert_eq!(b.identity().unwrap().npub, identity);
    assert_eq!(seed, *b.vault.entropy.lock().unwrap());
}

#[test]
fn denied_unavailable_and_cancelled_authentication_never_read_words() {
    for _ in 0..3 {
        let b = Fake::new();
        let f = b.flow();
        home(&f);
        let frame = press(&f, "backup");
        let frame = f
            .authenticated(frame.authentication.unwrap().token, false)
            .unwrap();
        assert!(!frame.secure);
        assert_eq!(b.word_reads.load(Ordering::SeqCst), 0);
        assert!(f.state.lock().unwrap().words.is_empty());
    }
}
#[test]
fn stale_callbacks_account_switch_and_expired_requests_cannot_reveal_words() {
    let b = Fake::new();
    let f = b.flow();
    home(&f);
    let token = press(&f, "backup").authentication.unwrap().token;
    f.redact();
    assert!(f.authenticated(token, true).is_err());
    let token = press(&f, "backup").authentication.unwrap().token;
    b.generation.fetch_add(1, Ordering::SeqCst);
    assert!(f.authenticated(token, true).is_err());
    let token = press(&f, "backup").authentication.unwrap().token;
    f.state.lock().unwrap().auth.as_mut().unwrap().1 = Instant::now() - Duration::from_secs(1);
    assert!(f.authenticated(token, true).is_err());
    assert_eq!(b.word_reads.load(Ordering::SeqCst), 0);
}
#[test]
fn locked_store_releases_references_without_replacing_seed() {
    let b = Fake::new();
    let before = b.vault.entropy.lock().unwrap().clone();
    b.locked.store(true, Ordering::SeqCst);
    let f = b.flow();
    home(&f);
    let frame = unlock(&f, "backup");
    assert!(!frame.secure);
    assert!(f.state.lock().unwrap().words.is_empty());
    assert_eq!(before, *b.vault.entropy.lock().unwrap());
}
#[test]
fn copy_is_explicit_once_and_has_a_sixty_second_limit() {
    let b = Fake::new();
    let f = b.flow();
    home(&f);
    assert!(unlock(&f, "backup").clipboard.is_none());
    let copy = press(&f, "copy").clipboard.unwrap();
    assert_eq!(copy.clear_after_ms, 60_000);
    assert_eq!(copy.text.split_whitespace().count(), 12);
    assert!(f.render().unwrap().clipboard.is_none());
    f.redact();
    assert!(f.state.lock().unwrap().pending.is_none());
    assert!(f.state.lock().unwrap().applied.is_none());
}
#[test]
fn all_requested_words_are_required_before_shared_confirmation() {
    let b = Fake::new();
    let f = b.flow();
    home(&f);
    unlock(&f, "backup");
    press(&f, "check");
    press(&f, "confirm");
    assert_eq!(b.confirm_calls.load(Ordering::SeqCst), 0);
    let words = b.loaded.lock().unwrap().as_ref().unwrap().words();
    let revision = mounted(&f);
    for p in [2, 5, 11] {
        f.change(
            "android-recovery".into(),
            revision,
            format!("answer-{p}"),
            words[p as usize - 1].clone(),
        )
        .unwrap();
    }
    press(&f, "confirm");
    assert!(b.identity().unwrap().words_saved);
    assert!(f.state.lock().unwrap().answers.is_empty());
}
#[test]
fn malformed_or_different_restore_words_do_not_replace_existing_wallet_seed() {
    let b = Fake::new();
    let f = b.flow();
    home(&f);
    let before = b.vault.entropy.lock().unwrap().clone();
    for words in [
        "malformed words".to_owned(),
        Identity::from_entropy(&[1; 16]).unwrap().words().join(" "),
    ] {
        unlock(&f, "restore");
        let revision = mounted(&f);
        f.change(
            "android-recovery".into(),
            revision,
            "restore-words".into(),
            words,
        )
        .unwrap();
        let frame = press(&f, "submit");
        assert!(!frame.secure);
        assert_eq!(before, *b.vault.entropy.lock().unwrap());
    }
}
#[test]
fn background_close_and_restart_keep_identity_but_never_secret_screen_state() {
    let b = Fake::new();
    let before = b.identity().unwrap().npub;
    let f = b.flow();
    home(&f);
    unlock(&f, "backup");
    f.redact();
    assert!(f.state.lock().unwrap().words.is_empty());
    let revision = mounted(&f);
    f.close();
    assert!(
        f.activate("android-recovery".into(), revision, "backup".into())
            .is_err()
    );
    let fresh = b.flow();
    assert!(!fresh.render().unwrap().secure);
    assert_eq!(b.identity().unwrap().npub, before);
}
#[test]
fn unacknowledged_or_previous_view_cannot_trigger_secret_actions() {
    let b = Fake::new();
    let f = b.flow();
    let rev = mounted(&f);
    home(&f);
    assert!(
        f.activate("android-recovery".into(), rev, "backup".into())
            .is_err()
    );
    let frame = f.render().unwrap();
    let j: serde_json::Value = serde_json::from_slice(&frame.json).unwrap();
    assert!(
        f.activate(
            "android-recovery".into(),
            j["revision"].as_u64().unwrap(),
            "backup".into()
        )
        .is_err()
    );
    assert_eq!(b.word_reads.load(Ordering::SeqCst), 0);
}

#[test]
fn account_change_during_secret_read_discards_the_result_before_rendering() {
    let b = Fake::new();
    let f = b.flow();
    home(&f);
    b.switch_on_words.store(true, Ordering::SeqCst);
    let frame = unlock(&f, "backup");
    assert!(!frame.secure);
    assert!(frame.clipboard.is_none());
    assert!(f.state.lock().unwrap().words.is_empty());
    assert!(!String::from_utf8(frame.json).unwrap().contains("abandon"));
}
