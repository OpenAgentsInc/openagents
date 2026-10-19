//! Rust Native wallet backup over the existing wallet, without exporting raw word lists to the host.
use super::*;
use crate::wallet::CoreWallet;
use rust_native::{Activation, Axis, Element, Node, TextRole, ValidatedView, View};
use serde::Serialize;
use std::time::{Duration, Instant};

trait Backend: Send + Sync {
    fn generation(&self) -> u64;
    fn open(&self);
    fn snapshot(&self) -> CoreResult<String>;
    fn activate(&self, instance: String, revision: u64, key: String) -> CoreResult<String>;
    fn field(&self, instance: String, revision: u64, key: String, value: String) -> CoreResult<()>;
    fn words(&self) -> CoreResult<Vec<String>>;
    fn saved(&self);
}
struct Real {
    account: Arc<CoreAccount>,
    wallet: Arc<CoreWallet>,
}
impl Backend for Real {
    fn generation(&self) -> u64 {
        self.account.snapshot().generation
    }
    fn open(&self) {
        self.wallet.open()
    }
    fn snapshot(&self) -> CoreResult<String> {
        self.wallet.snapshot()
    }
    fn activate(&self, instance: String, revision: u64, key: String) -> CoreResult<String> {
        self.wallet.activate(instance, revision, key)
    }
    fn field(&self, instance: String, revision: u64, key: String, value: String) -> CoreResult<()> {
        self.wallet.change_field(instance, revision, key, value)
    }
    fn words(&self) -> CoreResult<Vec<String>> {
        self.wallet.recovery_words()
    }
    fn saved(&self) {
        self.wallet.words_saved()
    }
}
#[derive(Clone, Copy, Serialize)]
enum Intent {
    Copy,
    Saved,
    Close,
}
enum Mode {
    Wallet,
    Authenticate { token: u64, until: Instant },
    Words { words: Vec<String>, until: Instant },
}
struct Backup {
    mode: Mode,
    revision: u64,
    next_token: u64,
    pending: Option<ValidatedView<Intent>>,
    applied: Option<ValidatedView<Intent>>,
}
impl Backup {
    fn new() -> Self {
        Self {
            mode: Mode::Wallet,
            revision: 0,
            next_token: 0,
            pending: None,
            applied: None,
        }
    }
    fn redact(&mut self) {
        self.mode = Mode::Wallet;
        self.pending = None;
        self.applied = None;
    }
    fn expire(&mut self) {
        let expired = match &self.mode {
            Mode::Wallet => false,
            Mode::Authenticate { until, .. } | Mode::Words { until, .. } => {
                *until <= Instant::now()
            }
        };
        if expired {
            self.redact()
        }
    }
    fn begin(&mut self) -> CoreResult<()> {
        self.next_token = self.next_token.checked_add(1).ok_or(CoreError::Closed)?;
        self.mode = Mode::Authenticate {
            token: self.next_token,
            until: Instant::now() + Duration::from_secs(60),
        };
        self.pending = None;
        self.applied = None;
        Ok(())
    }
}
#[derive(uniffi::Record)]
pub struct AndroidWalletFrame {
    pub json: Vec<u8>,
    pub secure: bool,
    pub authentication: Option<RecoveryAuthentication>,
    pub clipboard: Option<RecoveryClipboard>,
    pub redact_after_ms: Option<u32>,
}
#[derive(uniffi::Object)]
pub struct CoreAndroidWallet {
    backend: Arc<dyn Backend>,
    generation: u64,
    state: Mutex<Backup>,
}
impl CoreAndroidWallet {
    pub(super) fn new(account: Arc<CoreAccount>, wallet: Arc<CoreWallet>) -> Arc<Self> {
        let backend = Arc::new(Real { account, wallet });
        Arc::new(Self {
            generation: backend.generation(),
            backend,
            state: Mutex::new(Backup::new()),
        })
    }
    fn current(&self, state: &mut Backup) -> CoreResult<()> {
        if self.backend.generation() != self.generation {
            state.redact();
            return Err(CoreError::Closed);
        }
        state.expire();
        Ok(())
    }
    fn frame(
        &self,
        state: &mut Backup,
        clipboard: Option<RecoveryClipboard>,
    ) -> CoreResult<AndroidWalletFrame> {
        self.current(state)?;
        if matches!(state.mode, Mode::Wallet) {
            return Ok(AndroidWalletFrame {
                json: self.backend.snapshot()?.into_bytes(),
                secure: false,
                authentication: None,
                clipboard,
                redact_after_ms: None,
            });
        }
        let mut children = vec![];
        let (authentication, until) = match &state.mode {
            Mode::Authenticate { token, until } => {
                children.push(text("heading", "Unlock your device", TextRole::Heading));
                (
                    Some(RecoveryAuthentication {
                        token: *token,
                        title: "Unlock your device".into(),
                        description: "Confirm it's you to open your wallet recovery words.".into(),
                        timeout_ms: 60_000,
                    }),
                    *until,
                )
            }
            Mode::Words { words, until } => {
                children.push(text(
                    "heading",
                    "Write down your wallet words",
                    TextRole::Heading,
                ));
                for (i, word) in words.iter().enumerate() {
                    children.push(text(
                        &format!("word-{i}"),
                        &format!("{}. {word}", i + 1),
                        TextRole::Body,
                    ));
                }
                children.push(button("copy", "Copy words", Intent::Copy));
                children.push(button("saved", "I've written them down", Intent::Saved));
                (None, *until)
            }
            Mode::Wallet => unreachable!(),
        };
        children.push(button("close", "Close", Intent::Close));
        state.revision = state.revision.checked_add(1).ok_or(CoreError::Closed)?;
        let view = View::new_v3(
            format!("android-wallet-backup-{}", self.generation),
            state.revision,
            node(
                "root",
                Element::Stack {
                    axis: Axis::Vertical,
                    children,
                },
            ),
        )
        .validate()
        .map_err(|_| CoreError::Closed)?;
        let json = view.to_json().map_err(|_| CoreError::Closed)?;
        state.pending = Some(view);
        Ok(AndroidWalletFrame {
            json,
            secure: true,
            authentication,
            clipboard,
            redact_after_ms: Some(
                until
                    .saturating_duration_since(Instant::now())
                    .as_millis()
                    .min(60_000) as u32,
            ),
        })
    }
}
fn node(key: &str, element: Element<Intent>) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Default::default(),
        element,
    }
}
fn text(key: &str, value: &str, role: TextRole) -> Node<Intent> {
    node(
        key,
        Element::Text {
            value: value.into(),
            role,
        },
    )
}
fn button(key: &str, label: &str, intent: Intent) -> Node<Intent> {
    node(
        key,
        Element::Button {
            label: label.into(),
            enabled: true,
            icon: None,
            shortcut: None,
            intent,
        },
    )
}
#[uniffi::export]
impl CoreAndroidWallet {
    pub fn open(&self) -> CoreResult<AndroidWalletFrame> {
        let mut state = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.current(&mut state)?;
        self.backend.open();
        self.frame(&mut state, None)
    }
    pub fn snapshot(&self) -> CoreResult<AndroidWalletFrame> {
        let mut state = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.frame(&mut state, None)
    }
    pub fn acknowledge(&self, instance: String, revision: u64) -> CoreResult<()> {
        let mut state = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.current(&mut state)?;
        if matches!(state.mode, Mode::Wallet) {
            return Ok(());
        }
        let pending = state.pending.as_ref().ok_or(CoreError::Closed)?;
        if pending.view().instance != instance || pending.view().revision != revision {
            return Err(CoreError::Closed);
        }
        state.applied = state.pending.take();
        Ok(())
    }
    pub fn change_field(
        &self,
        instance: String,
        revision: u64,
        key: String,
        value: String,
    ) -> CoreResult<()> {
        let mut state = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.current(&mut state)?;
        if !matches!(state.mode, Mode::Wallet) {
            return Err(CoreError::Closed);
        }
        self.backend.field(instance, revision, key, value)
    }
    pub fn activate(
        &self,
        instance: String,
        revision: u64,
        key: String,
    ) -> CoreResult<AndroidWalletFrame> {
        let mut state = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.current(&mut state)?;
        let mut clipboard = None;
        if matches!(state.mode, Mode::Wallet) {
            let effect: serde_json::Value =
                serde_json::from_str(&self.backend.activate(instance, revision, key)?)
                    .map_err(|_| CoreError::Closed)?;
            match effect["kind"].as_str() {
                Some("backup") => state.begin()?,
                Some("copy") => {
                    let text = effect["text"]
                        .as_str()
                        .filter(|text| text.len() <= 64 * 1024)
                        .ok_or(CoreError::Closed)?;
                    clipboard = Some(RecoveryClipboard {
                        text: text.into(),
                        clear_after_ms: 60_000,
                    });
                }
                Some("none") => (),
                _ => return Err(CoreError::Closed),
            }
        } else {
            let intent = *state
                .applied
                .as_ref()
                .ok_or(CoreError::Closed)?
                .activate(&Activation {
                    instance,
                    revision,
                    node: key,
                })
                .map_err(|_| CoreError::Closed)?;
            match intent {
                Intent::Close => state.redact(),
                Intent::Saved => {
                    if !matches!(state.mode, Mode::Words { .. }) {
                        return Err(CoreError::Closed);
                    }
                    self.backend.saved();
                    state.redact();
                }
                Intent::Copy => {
                    let Mode::Words { words, .. } = &state.mode else {
                        return Err(CoreError::Closed);
                    };
                    clipboard = Some(RecoveryClipboard {
                        text: words.join(" "),
                        clear_after_ms: 30_000,
                    });
                }
            }
        }
        self.frame(&mut state, clipboard)
    }
    pub fn authenticated(&self, token: u64, success: bool) -> CoreResult<AndroidWalletFrame> {
        let mut state = self.state.lock().map_err(|_| CoreError::Closed)?;
        self.current(&mut state)?;
        if !matches!(state.mode, Mode::Authenticate { token: expected, .. } if expected == token) {
            return Err(CoreError::Closed);
        }
        state.redact();
        if success {
            let words = self.backend.words()?;
            self.current(&mut state)?;
            if !matches!(words.len(), 12 | 24)
                || words.iter().any(|word| {
                    word.is_empty()
                        || word.len() > 32
                        || !word.bytes().all(|c| c.is_ascii_lowercase())
                })
            {
                return Err(CoreError::Closed);
            }
            state.mode = Mode::Words {
                words,
                until: Instant::now() + Duration::from_secs(60),
            };
        }
        self.frame(&mut state, None)
    }
    pub fn redact(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.redact();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, AtomicUsize};
    struct Fake {
        generation: AtomicU64,
        reads: AtomicUsize,
        switch_during_read: AtomicBool,
    }
    impl Backend for Fake {
        fn generation(&self) -> u64 {
            self.generation.load(Ordering::SeqCst)
        }
        fn open(&self) {}
        fn snapshot(&self) -> CoreResult<String> {
            let view = View::new_v3(
                "fixture-wallet",
                1,
                text("title", "Wallet", TextRole::Heading),
            )
            .validate()
            .unwrap()
            .to_json()
            .unwrap();
            Ok(String::from_utf8(view).unwrap())
        }
        fn activate(&self, _: String, _: u64, _: String) -> CoreResult<String> {
            Ok("{\"kind\":\"backup\"}".into())
        }
        fn field(&self, _: String, _: u64, _: String, _: String) -> CoreResult<()> {
            Ok(())
        }
        fn words(&self) -> CoreResult<Vec<String>> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            if self.switch_during_read.load(Ordering::SeqCst) {
                self.generation.fetch_add(1, Ordering::SeqCst);
            }
            Ok(vec!["maple".into(); 12])
        }
        fn saved(&self) {}
    }
    fn fixture() -> (CoreAndroidWallet, Arc<Fake>, u64) {
        let fake = Arc::new(Fake {
            generation: AtomicU64::new(7),
            reads: AtomicUsize::new(0),
            switch_during_read: AtomicBool::new(false),
        });
        let mut state = Backup::new();
        state.begin().unwrap();
        let token = state.next_token;
        (
            CoreAndroidWallet {
                backend: fake.clone(),
                generation: 7,
                state: Mutex::new(state),
            },
            fake,
            token,
        )
    }
    #[test]
    fn denied_or_stale_device_authentication_never_reads_words() {
        let (wallet, fake, token) = fixture();
        assert!(!wallet.authenticated(token, false).unwrap().secure);
        assert_eq!(fake.reads.load(Ordering::SeqCst), 0);
        assert!(wallet.authenticated(token, true).is_err());
        let (wallet, fake, token) = fixture();
        fake.generation.store(8, Ordering::SeqCst);
        assert!(wallet.authenticated(token, true).is_err());
        assert_eq!(fake.reads.load(Ordering::SeqCst), 0);
    }
    #[test]
    fn account_switch_during_word_read_never_publishes_the_old_words() {
        let (wallet, fake, token) = fixture();
        fake.switch_during_read.store(true, Ordering::SeqCst);
        assert!(wallet.authenticated(token, true).is_err());
        assert_eq!(fake.reads.load(Ordering::SeqCst), 1);
        assert!(matches!(wallet.state.lock().unwrap().mode, Mode::Wallet));
    }
    #[test]
    fn copy_requires_the_current_acknowledged_word_view_and_redaction_clears_it() {
        let (wallet, _, token) = fixture();
        let frame = wallet.authenticated(token, true).unwrap();
        assert!(frame.secure);
        let view: serde_json::Value = serde_json::from_slice(&frame.json).unwrap();
        let id = view["instance"].as_str().unwrap().to_owned();
        let revision = view["revision"].as_u64().unwrap();
        assert!(
            wallet
                .activate(id.clone(), revision, "copy".into())
                .is_err()
        );
        wallet.acknowledge(id.clone(), revision).unwrap();
        let copied = wallet
            .activate(id.clone(), revision, "copy".into())
            .unwrap();
        assert_eq!(copied.clipboard.unwrap().text, vec!["maple"; 12].join(" "));
        wallet.redact();
        assert!(wallet.activate(id, revision, "copy".into()).is_err());
        let frame = wallet.snapshot().unwrap();
        assert!(!frame.secure);
        assert!(!String::from_utf8(frame.json).unwrap().contains("maple"));
    }
    #[test]
    fn expired_authentication_does_not_read_a_seed() {
        let (wallet, fake, token) = fixture();
        wallet.state.lock().unwrap().mode = Mode::Authenticate {
            token,
            until: Instant::now() - Duration::from_secs(1),
        };
        assert!(wallet.authenticated(token, true).is_err());
        assert_eq!(fake.reads.load(Ordering::SeqCst), 0);
    }
}
