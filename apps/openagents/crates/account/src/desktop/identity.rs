//! The desktop's account identity (#11997): the first-launch welcome, the
//! sidebar identity row, and Back up your account.
//!
//! Nobody signs in. On first launch the desktop offers three ways to start:
//!
//! - **Connect your phone**: the phone scans this computer's code, and a
//!   phone that has an account links this computer to it (#11998, #12000).
//! - **Start here**: make new 12 words on this computer.
//! - **Restore from 12 words**: type the words saved from another device.
//!
//! After that the identity's key signs in silently on every launch, through
//! [`crate::Controller::restore`]. The words, the NIP-06 key and the silent
//! sign-in itself belong to the shared identity (#11995); this module is the
//! desktop's side of it: [`DesktopIdentity`] is what the desktop asks of
//! that identity, and the rest is the welcome's state and its words.
use crate::{
    Controller, Phase,
    identity::{Identities, Identity, WordAnswer},
};
use std::{
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::{Arc, Mutex},
};

/// Work the identity does off the UI thread. A failure is a sentence the
/// person can read.
pub type Pending<T> = Pin<Box<dyn Future<Output = Result<T, String>> + Send + 'static>>;

/// Account work whose outcome shows in the controller's phase.
pub type Work = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

/// The person's identity on this computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdentityView {
    /// The public key as `npub1…`.
    pub npub: String,
    /// Whether the person said they wrote the 12 words down. A computer that
    /// joined through a phone has no words of its own and reports `true`.
    pub words_saved: bool,
    /// Whether this computer holds the 12 words, so Back up can show them.
    pub has_words: bool,
}

/// One key that signs in to the account, oldest first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedDevice {
    pub npub: String,
    /// The key this computer signs in with.
    pub this_device: bool,
}

/// What the desktop asks of the account's identity (#11995).
///
/// A fresh computer signs in with no key until the person picks Start here
/// or Restore from 12 words: a key that has made its own account can't be
/// linked to another one later (the server answers `identity_taken`). After
/// Connect your phone the computer waits for the phone's link (#12000), and
/// only then does [`DesktopIdentity::identity`] answer and the key sign in.
pub trait DesktopIdentity: Send + Sync {
    /// On launch, and again after a session is lost: sign in silently with
    /// this computer's key. A computer with no identity only restores a
    /// saved session, and makes no key, unless it was signed in before the
    /// identity existed: then its key joins that same account.
    fn start(&self) -> Work;
    /// While signed in: sign in again with the key once the session is
    /// close to its end.
    fn renew(&self) -> Work;
    /// This computer's identity, or `None` before the welcome is answered.
    fn identity(&self) -> Option<IdentityView>;
    /// Start here: make new 12 words, keep them on this computer, and sign
    /// in with their key.
    fn start_new(&self) -> Pending<IdentityView>;
    /// Restore from 12 words: keep these words and sign in with their key.
    fn restore(&self, words: String) -> Pending<IdentityView>;
    /// Connect your phone on a fresh computer: make this computer's key
    /// without signing in, offer it to a phone that scans the code
    /// ([`link`]), and sign in with it once the phone has linked it to its
    /// account. Fails quietly when the person picks another way instead.
    fn connect_phone(&self) -> Pending<IdentityView>;
    /// The 12 words, for Back up your account. Shown only after the person
    /// confirms.
    fn words(&self) -> Result<Vec<String>, String>;
    /// The person wrote the words down.
    fn confirm_saved(&self) -> Result<(), String>;
    /// The keys that sign in to the account: this computer, phones, and other
    /// computers.
    fn linked_devices(&self) -> Pending<Vec<LinkedDevice>>;
}

/// [`DesktopIdentity`] over the shared identity (#11995): the words in
/// [`Identities`] and the key sign-in in [`Controller`].
pub struct KeyIdentity {
    data: PathBuf,
    identities: Arc<Identities>,
    controller: Arc<Controller>,
    computer: String,
    /// This computer's identity once it has one. Opened once, on launch.
    current: Arc<Mutex<Option<Arc<Identity>>>>,
    /// A session saved before the identity existed, whose account the new
    /// key joins.
    signed_in_before: bool,
}

impl KeyIdentity {
    /// `signed_in_before`: a sign-in from an earlier build is saved here.
    /// Reads the Keychain once.
    pub fn new(
        data: &Path,
        identities: Identities,
        controller: Arc<Controller>,
        computer: String,
        signed_in_before: bool,
    ) -> Self {
        // Never makes words: a locked Keychain reads as no identity yet,
        // and the next launch tries again.
        let current = identities.load().ok().flatten().map(Arc::new);
        Self {
            data: data.to_owned(),
            identities: Arc::new(identities),
            controller,
            computer,
            current: Arc::new(Mutex::new(current)),
            signed_in_before,
        }
    }

    fn view(&self, identity: &Identity) -> IdentityView {
        IdentityView {
            npub: identity.npub(),
            words_saved: self.identities.words_saved(identity),
            has_words: true,
        }
    }

    /// This computer's identity, unless its key still waits for a phone's
    /// link: that key must not sign in on its own first.
    fn current(&self) -> Option<Arc<Identity>> {
        self.current
            .lock()
            .unwrap()
            .clone()
            .filter(|identity| !link::waiting(&self.data, &identity.pubkey_hex()))
    }

    /// Open or make the words off the UI thread, keep them as this
    /// computer's identity, and sign in with them.
    fn sign_in_after(
        &self,
        open: impl FnOnce(&Identities) -> Result<Identity, crate::identity::IdentityError>
        + Send
        + 'static,
        replace_session: bool,
    ) -> Pending<IdentityView> {
        let identities = self.identities.clone();
        let current = self.current.clone();
        let controller = self.controller.clone();
        let computer = self.computer.clone();
        // The person picked another way in: stop offering the key to a phone.
        link::stop(&self.data);
        Box::pin(async move {
            let opened = tokio::task::spawn_blocking({
                let identities = identities.clone();
                move || open(&identities)
            })
            .await
            .map_err(|_| "Something went wrong. Try again.".to_owned())?
            .map_err(|error| error.message())?;
            let identity = Arc::new(opened);
            *current.lock().unwrap() = Some(identity.clone());
            let view = IdentityView {
                npub: identity.npub(),
                words_saved: identities.words_saved(&identity),
                has_words: true,
            };
            // The identity is made either way; a failed sign-in shows in
            // the row and is tried again from there.
            if replace_session {
                controller.sign_in_with_key(identity, computer).await;
            } else {
                controller.start_with_key(identity, computer).await;
            }
            Ok(view)
        })
    }
}

impl DesktopIdentity for KeyIdentity {
    fn start(&self) -> Work {
        let controller = self.controller.clone();
        let computer = self.computer.clone();
        if let Some(identity) = self.current() {
            return Box::pin(controller.start_with_key(identity, computer));
        }
        if !self.signed_in_before {
            // A fresh computer: nothing signs in until the welcome is
            // answered.
            return Box::pin(controller.restore());
        }
        let work = self.sign_in_after(|identities| identities.load_or_create(), false);
        Box::pin(async move {
            if let Err(message) = work.await {
                controller.identity_failed(message);
            }
        })
    }

    fn renew(&self) -> Work {
        match self.current() {
            Some(identity) if self.controller.renewal_due() => Box::pin(
                self.controller
                    .renew_with_key(identity, self.computer.clone()),
            ),
            _ => Box::pin(async {}),
        }
    }

    fn identity(&self) -> Option<IdentityView> {
        self.current().map(|identity| self.view(&identity))
    }

    fn start_new(&self) -> Pending<IdentityView> {
        // A saved session from an earlier build keeps its account: the
        // key joins it instead of making another.
        self.sign_in_after(|identities| identities.load_or_create(), false)
    }

    fn restore(&self, words: String) -> Pending<IdentityView> {
        self.sign_in_after(move |identities| identities.restore(&words, false), true)
    }

    fn connect_phone(&self) -> Pending<IdentityView> {
        let identities = self.identities.clone();
        let current = self.current.clone();
        let controller = self.controller.clone();
        let computer = self.computer.clone();
        let data = self.data.clone();
        Box::pin(async move {
            let made = tokio::task::spawn_blocking({
                let identities = identities.clone();
                move || identities.load_or_create()
            })
            .await
            .map_err(|_| "Something went wrong. Try again.".to_owned())?
            .map_err(|error| error.message())?;
            let identity = Arc::new(made);
            let pubkey = identity.pubkey_hex();
            *current.lock().unwrap() = Some(identity.clone());
            let mut state = link::offer(&data, identity.clone());
            let mut signed_at: Option<std::time::Instant> = None;
            let mut refreshed = std::time::Instant::now();
            loop {
                // The engine's device connection may run in its own process:
                // read what it wrote beside the offer.
                link::follow_disk(&data);
                let now = state.borrow_and_update().clone();
                match now {
                    link::LinkState::Linked(key) if key == pubkey => break,
                    link::LinkState::Signed(key) if key == pubkey => {
                        // The phone sends the signed proof to our server and
                        // says when it landed (#12000). Without that word the
                        // key may not be on the phone's account, and signing
                        // in would make it its own: offer it again.
                        let at = *signed_at.get_or_insert_with(std::time::Instant::now);
                        if at.elapsed() >= link::CONFIRM {
                            state = link::offer(&data, identity.clone());
                            signed_at = None;
                            continue;
                        }
                    }
                    link::LinkState::Idle => return Err(String::new()),
                    _ => {}
                }
                if refreshed.elapsed() >= link::REFRESH {
                    link::refresh_offer(&data, &pubkey);
                    refreshed = std::time::Instant::now();
                }
                tokio::select! {
                    changed = state.changed() => {
                        if changed.is_err() {
                            return Err(String::new());
                        }
                    }
                    () = tokio::time::sleep(link::POLL) => {}
                }
            }
            link::finish(&data, &pubkey);
            let view = IdentityView {
                npub: identity.npub(),
                words_saved: identities.words_saved(&identity),
                has_words: true,
            };
            controller.sign_in_with_key(identity, computer).await;
            Ok(view)
        })
    }

    fn words(&self) -> Result<Vec<String>, String> {
        self.current()
            .map(|identity| identity.words())
            .ok_or_else(|| copy::NO_WORDS.to_owned())
    }

    fn confirm_saved(&self) -> Result<(), String> {
        let identity = self.current().ok_or_else(|| copy::NO_WORDS.to_owned())?;
        let answers: Vec<WordAnswer> = identity
            .words()
            .into_iter()
            .enumerate()
            .map(|(ix, word)| WordAnswer {
                position: ix as u32 + 1,
                word,
            })
            .collect();
        if self.identities.confirm_backup(&identity, &answers) {
            Ok(())
        } else {
            Err("Couldn't save that. Try again.".into())
        }
    }

    fn linked_devices(&self) -> Pending<Vec<LinkedDevice>> {
        let lease = self.controller.lease();
        let mine = self.current().map(|identity| identity.pubkey_hex());
        Box::pin(async move {
            let lease = lease.ok_or_else(|| "Not connected yet.".to_owned())?;
            let keys = lease
                .client()
                .account_keys(lease.session())
                .await
                .map_err(|error| error.to_string())?;
            Ok(keys
                .into_iter()
                .map(|key| LinkedDevice {
                    this_device: mine.as_deref() == Some(key.pubkey.as_str()),
                    npub: key.npub,
                })
                .collect())
        })
    }
}

/// The key a fresh computer offers the phone that scans its Connect your
/// phone code (#12000), and whether the phone has asked for it.
///
/// The engine's device connection watches [`watch`]: while it says
/// [`LinkState::Offered`], it answers a scanning phone's `LinkAccount` call
/// with that key (`Hosted::set_account_signer`), and after signing a proof
/// it calls [`signed`]. The key stays marked as waiting on disk, so a
/// relaunch doesn't sign in with it before the link: an unlinked key that
/// signs in makes its own account, and then it can't join the phone's.
pub mod link {
    use crate::identity::Identity;
    use std::{
        collections::HashMap,
        path::{Path, PathBuf},
        sync::{Arc, Mutex, OnceLock},
        time::Duration,
    };
    use tokio::sync::watch;

    /// How long after the computer signs a proof it waits for the phone to
    /// say the link landed, before it offers the key again.
    pub const CONFIRM: Duration = Duration::from_secs(60);
    /// How often each side reads the other's file.
    pub const POLL: Duration = Duration::from_secs(1);
    /// How often the window rewrites its offer, so a closed app's offer
    /// goes stale.
    pub const REFRESH: Duration = Duration::from_secs(60);
    /// An offer older than this, in seconds, offers nothing.
    pub const OFFER_STALE: u64 = 300;
    const WAITING: &str = "openagents-account/identity/link-waiting";
    /// The window's offer, for an engine in another process: the key and
    /// the Keychain item it lives in, never the secret.
    const OFFER: &str = "openagents-account/identity/link-offer.json";
    /// What the engine did with the offer: signed a proof, or heard the
    /// link landed.
    const STATE: &str = "openagents-account/identity/link-state.json";

    #[derive(serde::Serialize, serde::Deserialize)]
    struct OfferFile {
        v: u32,
        pubkey: String,
        keychain: String,
        at: u64,
    }

    #[derive(serde::Serialize, serde::Deserialize, PartialEq, Eq)]
    struct StateFile {
        v: u32,
        state: String,
        pubkey: String,
    }

    fn now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs())
    }

    /// Write `bytes` at `path` readable by this user only, in one step.
    fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
        use std::io::Write as _;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temporary, path)
    }

    fn write_offer(data: &Path, pubkey: &str) {
        let offer = OfferFile {
            v: 1,
            pubkey: pubkey.to_owned(),
            keychain: super::super::identity_service(),
            at: now(),
        };
        if let Ok(bytes) = serde_json::to_vec(&offer) {
            let _ = write_private(&data.join(OFFER), &bytes);
        }
    }

    fn write_state(data: &Path, state: &str, pubkey: &str) {
        let record = StateFile {
            v: 1,
            state: state.to_owned(),
            pubkey: pubkey.to_owned(),
        };
        if let Ok(bytes) = serde_json::to_vec(&record) {
            let _ = write_private(&data.join(STATE), &bytes);
        }
    }

    fn read_state(data: &Path) -> Option<StateFile> {
        let bytes = std::fs::read(data.join(STATE)).ok()?;
        (bytes.len() <= 4096)
            .then(|| serde_json::from_slice(&bytes).ok())
            .flatten()
    }

    fn clear_files(data: &Path) {
        let _ = std::fs::remove_file(data.join(OFFER));
        let _ = std::fs::remove_file(data.join(STATE));
    }

    /// The window still offers `pubkey`: keep its offer fresh.
    pub fn refresh_offer(data: &Path, pubkey: &str) {
        write_offer(data, pubkey);
    }

    /// The key the window offers, from its file, when the offer is fresh at
    /// `at` (Unix seconds) and names this computer's identity item.
    pub fn offered_key(data: &Path, at: u64) -> Option<String> {
        let bytes = std::fs::read(data.join(OFFER)).ok()?;
        if bytes.len() > 4096 {
            return None;
        }
        let offer: OfferFile = serde_json::from_slice(&bytes).ok()?;
        (offer.v == 1
            && offer.keychain == super::super::identity_service()
            && at <= offer.at.saturating_add(OFFER_STALE)
            && offer.at <= at.saturating_add(60))
        .then_some(offer.pubkey)
    }

    /// The identity the window offers, read from the Keychain item its file
    /// names, when the stored identity is that key. Blocking: the engine
    /// calls it off its runtime threads.
    pub fn load_offered(data: &Path, at: u64) -> Option<Arc<Identity>> {
        let key = offered_key(data, at)?;
        let identity = super::super::identities(data).load().ok()??;
        (identity.pubkey_hex() == key).then(|| Arc::new(identity))
    }

    /// The device connection for `data` signed a link proof with `pubkey`:
    /// tell this process's window and one in another process.
    pub fn signed_in(data: &Path, pubkey: &str) {
        signed(pubkey);
        if offered_key(data, now()).as_deref() == Some(pubkey) {
            write_state(data, "signed", pubkey);
        }
    }

    /// The phone said the link landed for `pubkey`: tell this process's
    /// window and one in another process.
    pub fn linked_in(data: &Path, pubkey: &str) {
        linked(pubkey);
        if offered_key(data, now()).as_deref() == Some(pubkey) {
            write_state(data, "linked", pubkey);
        }
    }

    /// The window's side: take what an engine in another process wrote
    /// for `data` into this process's state.
    pub fn follow_disk(data: &Path) {
        let Some(record) = read_state(data) else {
            return;
        };
        if record.v != 1 {
            return;
        }
        match record.state.as_str() {
            "signed" => signed(&record.pubkey),
            "linked" => {
                // A link can land before this side read "signed".
                signed(&record.pubkey);
                linked(&record.pubkey);
            }
            _ => {}
        }
    }

    #[derive(Clone)]
    pub enum LinkState {
        /// Nothing to offer: this computer signs in on its own.
        Idle,
        /// A phone that scans the code may link this key to its account.
        Offered(Arc<Identity>),
        /// The computer signed the phone's link proof for this key (hex).
        Signed(String),
        /// The phone said our server added this key (hex) to its account:
        /// the key may sign in now.
        Linked(String),
    }

    impl std::fmt::Debug for LinkState {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            // The key's secret stays out of debug output.
            match self {
                Self::Idle => f.write_str("Idle"),
                Self::Offered(identity) => write!(f, "Offered({})", identity.pubkey_hex()),
                Self::Signed(key) => write!(f, "Signed({key})"),
                Self::Linked(key) => write!(f, "Linked({key})"),
            }
        }
    }

    fn channels() -> &'static Mutex<HashMap<PathBuf, watch::Sender<LinkState>>> {
        static CHANNELS: OnceLock<Mutex<HashMap<PathBuf, watch::Sender<LinkState>>>> =
            OnceLock::new();
        CHANNELS.get_or_init(Mutex::default)
    }

    fn sender(data: &Path) -> watch::Sender<LinkState> {
        channels()
            .lock()
            .unwrap()
            .entry(data.to_owned())
            .or_insert_with(|| watch::channel(LinkState::Idle).0)
            .clone()
    }

    /// What the device connection for `data` should offer now.
    pub fn watch(data: &Path) -> watch::Receiver<LinkState> {
        sender(data).subscribe()
    }

    /// Offer `identity` to a scanning phone, and remember on disk that its
    /// key waits for that link.
    pub fn offer(data: &Path, identity: Arc<Identity>) -> watch::Receiver<LinkState> {
        let path = data.join(WAITING);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, identity.pubkey_hex());
        // A fresh offer: nothing from an earlier one carries over.
        let _ = std::fs::remove_file(data.join(STATE));
        write_offer(data, &identity.pubkey_hex());
        let sender = sender(data);
        let receiver = sender.subscribe();
        sender.send_replace(LinkState::Offered(identity));
        receiver
    }

    /// The device connection signed a link proof with the key `pubkey`
    /// (hex). Called for every data directory offering that key.
    pub fn signed(pubkey: &str) {
        for sender in channels().lock().unwrap().values() {
            let offered = matches!(
                &*sender.borrow(),
                LinkState::Offered(identity) if identity.pubkey_hex() == pubkey
            );
            if offered {
                sender.send_replace(LinkState::Signed(pubkey.to_owned()));
            }
        }
    }

    /// A paired phone said our server added the key `pubkey` (hex) to its
    /// account (`AccountLinked`). Called for every data directory offering
    /// or having signed for that key.
    pub fn linked(pubkey: &str) {
        for sender in channels().lock().unwrap().values() {
            let waiting = match &*sender.borrow() {
                LinkState::Offered(identity) => identity.pubkey_hex() == pubkey,
                LinkState::Signed(key) => key == pubkey,
                _ => false,
            };
            if waiting {
                sender.send_replace(LinkState::Linked(pubkey.to_owned()));
            }
        }
    }

    /// The person picked Start here or Restore instead: offer nothing.
    pub fn stop(data: &Path) {
        let _ = std::fs::remove_file(data.join(WAITING));
        clear_files(data);
        sender(data).send_replace(LinkState::Idle);
    }

    /// The link landed: the key may sign in now.
    pub fn finish(data: &Path, pubkey: &str) {
        if waiting(data, pubkey) {
            let _ = std::fs::remove_file(data.join(WAITING));
        }
        clear_files(data);
        sender(data).send_replace(LinkState::Idle);
    }

    /// Whether the key `pubkey` still waits for a phone's link.
    pub fn waiting(data: &Path, pubkey: &str) -> bool {
        std::fs::read_to_string(data.join(WAITING)).is_ok_and(|text| text.trim() == pubkey)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn identity() -> Arc<Identity> {
            Arc::new(
                Identity::from_words(
                    "abandon abandon abandon abandon abandon abandon abandon abandon abandon \
                     abandon abandon about",
                )
                .unwrap(),
            )
        }

        #[test]
        fn an_engine_in_another_process_reads_the_offer_and_answers_on_disk() {
            let dir = tempfile::tempdir().unwrap();
            let data = dir.path();
            let identity = identity();
            let key = identity.pubkey_hex();
            assert_eq!(offered_key(data, now()), None);
            let mut state = offer(data, identity.clone());
            // The offer names the key and its Keychain item, never the secret.
            let written = std::fs::read_to_string(data.join(OFFER)).unwrap();
            assert!(written.contains(&key));
            assert!(written.contains(&super::super::super::identity_service()));
            assert!(!written.contains(&identity.nsec()));
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = std::fs::metadata(data.join(OFFER))
                    .unwrap()
                    .permissions()
                    .mode();
                assert_eq!(mode & 0o777, 0o600);
            }
            assert_eq!(offered_key(data, now()), Some(key.clone()));
            // A stale offer offers nothing.
            assert_eq!(offered_key(data, now() + OFFER_STALE + 1), None);

            // The engine's process writes "linked"; the window reads it.
            write_state(data, "linked", &key);
            follow_disk(data);
            assert!(matches!(&*state.borrow_and_update(), LinkState::Linked(k) if *k == key));

            // Finishing clears both files.
            finish(data, &key);
            assert!(!data.join(OFFER).exists());
            assert!(!data.join(STATE).exists());
            assert_eq!(offered_key(data, now()), None);
        }

        #[test]
        fn the_engine_writes_its_answer_only_for_the_offered_key() {
            let dir = tempfile::tempdir().unwrap();
            let data = dir.path();
            let identity = Arc::new(Identity::generate().unwrap());
            let key = identity.pubkey_hex();
            signed_in(data, &key);
            assert!(
                read_state(data).is_none(),
                "nothing offered, nothing written"
            );
            let _state = offer(data, identity);
            signed_in(data, &"ab".repeat(32));
            assert!(read_state(data).is_none());
            signed_in(data, &key);
            let record = read_state(data).unwrap();
            assert_eq!(
                (record.state.as_str(), record.pubkey.as_str()),
                ("signed", key.as_str())
            );
            linked_in(data, &key);
            assert_eq!(read_state(data).unwrap().state, "linked");
            stop(data);
            assert!(read_state(data).is_none());
        }
    }
}

/// The words the desktop shows. Tests run the `oa-copy` guard over them.
pub mod copy {
    pub const WELCOME_TITLE: &str = "Welcome to OpenAgents";
    pub const WELCOME_BODY: &str = "There's nothing to sign in to. Pick how you want to start.";
    pub const CONNECT_PHONE: &str = "Connect your phone";
    pub const CONNECT_PHONE_HINT: &str =
        "Use OpenAgents on your phone? Scan a code to use the same account here.";
    pub const START_HERE: &str = "Start here";
    pub const START_HERE_HINT: &str = "Make a new account on this computer.";
    pub const RESTORE: &str = "Restore from 12 words";
    pub const RESTORE_HINT: &str = "Bring your account back with the 12 words you wrote down.";
    pub const NOT_NOW: &str = "Not now";
    pub const STARTING: &str = "Setting up your account…";

    pub const RESTORE_BODY: &str = "Type your 12 words in order, with a space between each one.";
    pub const RESTORE_PLACEHOLDER: &str = "word1 word2 word3 …";
    pub const RESTORE_BUTTON: &str = "Restore";
    pub const BACK: &str = "Back";

    pub const SET_UP: &str = "Set up your account";
    pub const YOUR_ID: &str = "Your ID";
    pub const COPY_ID: &str = "Copy your ID";
    pub const COPIED: &str = "Copied";
    pub const LINKED_DEVICES: &str = "Linked devices";
    pub const THIS_COMPUTER: &str = "This computer";
    pub const NO_DEVICES: &str = "Only this computer so far.";
    pub const BACK_UP: &str = "Back up your account";

    pub const BACKUP_WARNING: &str = "Your 12 words are the only way back into your account and \
        your wallet if you lose this computer. Anyone who has them can use both. Write them down \
        and keep them somewhere safe.";
    pub const SHOW_WORDS: &str = "Show my words";
    pub const CANCEL: &str = "Cancel";
    pub const WROTE_THEM_DOWN: &str = "I wrote them down";
    pub const BACKUP_DONE: &str = "You've backed up your account.";
    pub const NO_WORDS: &str = "This computer joined through your phone, so it has no words of \
        its own. Back up from your phone.";
    pub const CLOSE: &str = "Close";

    /// Every string above, for the copy guard.
    pub const ALL: &[&str] = &[
        WELCOME_TITLE,
        WELCOME_BODY,
        CONNECT_PHONE,
        CONNECT_PHONE_HINT,
        START_HERE,
        START_HERE_HINT,
        RESTORE,
        RESTORE_HINT,
        NOT_NOW,
        STARTING,
        RESTORE_BODY,
        RESTORE_PLACEHOLDER,
        RESTORE_BUTTON,
        BACK,
        SET_UP,
        YOUR_ID,
        COPY_ID,
        COPIED,
        LINKED_DEVICES,
        THIS_COMPUTER,
        NO_DEVICES,
        BACK_UP,
        BACKUP_WARNING,
        SHOW_WORDS,
        CANCEL,
        WROTE_THEM_DOWN,
        BACKUP_DONE,
        NO_WORDS,
        CLOSE,
    ];
}

/// Written when the person picks Not now, so the welcome doesn't return on
/// every launch. The sidebar row still offers Set up your account.
const WELCOME_DISMISSED: &str = "openagents-account/welcome-dismissed";

pub fn welcome_dismissed(data: &Path) -> bool {
    data.join(WELCOME_DISMISSED).exists()
}

pub fn dismiss_welcome(data: &Path) {
    let path = data.join(WELCOME_DISMISSED);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, b"1\n");
}

/// Whether the first-launch welcome opens: no identity on this computer, no
/// saved session either (an older sign-in gets its key added silently), and
/// the person hasn't put it off. It waits while a saved session restores.
pub fn shows_welcome(identity: Option<&IdentityView>, phase: &Phase, dismissed: bool) -> bool {
    identity.is_none() && !dismissed && matches!(phase, Phase::SignedOut { .. })
}

/// The npub as the sidebar shows it: `npub1` and its first and last few
/// characters. The full npub is in the menu and on the clipboard.
pub fn short_npub(npub: &str) -> String {
    let chars: Vec<char> = npub.chars().collect();
    if chars.len() <= 18 {
        return npub.to_owned();
    }
    let head: String = chars[..10].iter().collect();
    let tail: String = chars[chars.len() - 6..].iter().collect();
    format!("{head}…{tail}")
}

/// The typed words, lowercased and single-spaced, once there are 12. The
/// identity checks the words themselves.
pub fn typed_words(input: &str) -> Result<String, String> {
    let words: Vec<String> = input
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|word| !word.is_empty())
        .map(|word| {
            word.trim_matches(|c: char| c.is_ascii_digit() || c == '.')
                .to_lowercase()
        })
        .filter(|word| !word.is_empty())
        .collect();
    match words.len() {
        0 => Err("Type your 12 words.".into()),
        12 => Ok(words.join(" ")),
        1 => Err("That's 1 word. Your backup has 12.".into()),
        n => Err(format!("That's {n} words. Your backup has 12.")),
    }
}

/// What the welcome shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WelcomeStep {
    /// Connect your phone, Start here, or Restore from 12 words.
    Choose { error: Option<String> },
    /// The words field.
    Restore { error: Option<String> },
    /// Making or restoring the identity and signing in.
    Working,
}

/// What Back up your account shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BackupStep {
    /// The warning, before any word is on screen.
    Confirm,
    /// The words, numbered.
    Words(Vec<String>),
    /// This computer joined through a phone and has no words.
    NoWords,
    /// The person confirmed.
    Done,
}

impl BackupStep {
    /// The first step for this identity.
    pub fn open(identity: Option<&IdentityView>) -> Self {
        match identity {
            Some(identity) if !identity.has_words => Self::NoWords,
            _ => Self::Confirm,
        }
    }
}

/// The linked devices as rows: this computer first, then the rest oldest
/// first, each with its label.
pub fn device_rows(devices: &[LinkedDevice]) -> Vec<(String, String)> {
    let mut rows: Vec<(String, String)> = devices
        .iter()
        .filter(|device| device.this_device)
        .map(|device| (copy::THIS_COMPUTER.to_owned(), short_npub(&device.npub)))
        .collect();
    rows.extend(
        devices
            .iter()
            .filter(|device| !device.this_device)
            .map(|device| (short_npub(&device.npub), String::new())),
    );
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    const NPUB: &str = "npub1sg6plzptd64u62a878hep2kev88swjh3tw00gjsfl8f237lmu63q0uf63m";

    fn identity(has_words: bool) -> IdentityView {
        IdentityView {
            npub: NPUB.into(),
            words_saved: false,
            has_words,
        }
    }

    #[test]
    fn the_welcome_opens_only_on_a_fresh_signed_out_computer() {
        let out = Phase::SignedOut { notice: None };
        assert!(shows_welcome(None, &out, false));
        assert!(!shows_welcome(None, &out, true), "Not now is remembered");
        assert!(!shows_welcome(Some(&identity(true)), &out, false));
        assert!(!shows_welcome(None, &Phase::Restoring, false));
        assert!(!shows_welcome(None, &Phase::Starting, false));
        let failed = Phase::Failed {
            message: "offline".into(),
            can_restore: true,
        };
        assert!(
            !shows_welcome(None, &failed, false),
            "a saved sign-in is still there"
        );
    }

    #[test]
    fn not_now_is_remembered_per_data_directory() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!welcome_dismissed(dir.path()));
        dismiss_welcome(dir.path());
        assert!(welcome_dismissed(dir.path()));
        let other = tempfile::tempdir().unwrap();
        assert!(!welcome_dismissed(other.path()));
    }

    #[test]
    fn a_key_offered_to_a_phone_waits_until_the_link_is_signed() {
        let dir = tempfile::tempdir().unwrap();
        let identity = Arc::new(
            Identity::from_words(
                "leader monkey parrot ring guide accident before fence cannon height naive bean",
            )
            .unwrap(),
        );
        let key = identity.pubkey_hex();
        let mut state = link::offer(dir.path(), identity.clone());
        assert!(
            link::waiting(dir.path(), &key),
            "a relaunch won't sign in with it"
        );
        assert!(matches!(
            &*state.borrow_and_update(),
            link::LinkState::Offered(_)
        ));
        link::signed("someone-else");
        assert!(matches!(&*state.borrow(), link::LinkState::Offered(_)));
        link::signed(&key);
        assert!(matches!(&*state.borrow(), link::LinkState::Signed(k) if *k == key));
        link::linked("someone-else");
        assert!(matches!(&*state.borrow(), link::LinkState::Signed(_)));
        link::linked(&key);
        assert!(matches!(&*state.borrow(), link::LinkState::Linked(k) if *k == key));
        link::finish(dir.path(), &key);
        assert!(!link::waiting(dir.path(), &key));
        assert!(matches!(
            &*link::watch(dir.path()).borrow(),
            link::LinkState::Idle
        ));

        link::offer(dir.path(), identity);
        link::stop(dir.path());
        assert!(
            !link::waiting(dir.path(), &key),
            "Start here stops the offer"
        );
    }

    #[test]
    fn the_npub_is_shortened_for_the_row() {
        assert_eq!(short_npub(NPUB), "npub1sg6pl…0uf63m");
        assert_eq!(short_npub("npub1short"), "npub1short");
    }

    #[test]
    fn typed_words_take_any_spacing_numbering_and_case() {
        let words = "Abandon abandon abandon abandon abandon abandon abandon abandon abandon \
                     abandon abandon about";
        let expected = words.to_lowercase();
        assert_eq!(typed_words(words).unwrap(), expected);
        let numbered = "1. abandon 2. abandon 3. abandon 4. abandon 5. abandon 6. abandon\n\
                        7. abandon 8. abandon 9. abandon 10. abandon 11. abandon 12. about";
        assert_eq!(typed_words(numbered).unwrap(), expected);
        let commas = "abandon,abandon, abandon,  abandon\tabandon abandon abandon abandon \
                      abandon abandon abandon about ";
        assert_eq!(typed_words(commas).unwrap(), expected);
    }

    #[test]
    fn the_wrong_number_of_words_says_how_many() {
        assert_eq!(typed_words("  ").unwrap_err(), "Type your 12 words.");
        assert_eq!(
            typed_words("abandon").unwrap_err(),
            "That's 1 word. Your backup has 12."
        );
        assert_eq!(
            typed_words(&"abandon ".repeat(11)).unwrap_err(),
            "That's 11 words. Your backup has 12."
        );
        assert_eq!(
            typed_words(&"abandon ".repeat(24)).unwrap_err(),
            "That's 24 words. Your backup has 12."
        );
    }

    #[test]
    fn backup_shows_the_warning_first_and_no_words_for_a_linked_computer() {
        assert_eq!(BackupStep::open(Some(&identity(true))), BackupStep::Confirm);
        assert_eq!(
            BackupStep::open(Some(&identity(false))),
            BackupStep::NoWords
        );
        assert_eq!(BackupStep::open(None), BackupStep::Confirm);
    }

    #[test]
    fn this_computer_is_listed_first() {
        let devices = vec![
            LinkedDevice {
                npub: "npub1phone0000000000000000000000000000000000000000000000000001".into(),
                this_device: false,
            },
            LinkedDevice {
                npub: NPUB.into(),
                this_device: true,
            },
        ];
        let rows = device_rows(&devices);
        assert_eq!(
            rows[0],
            ("This computer".into(), "npub1sg6pl…0uf63m".into())
        );
        assert_eq!(rows[1], ("npub1phone…000001".into(), String::new()));
    }

    #[test]
    fn the_desktop_words_read_plainly_and_never_ask_to_sign_in() {
        let mut texts: Vec<String> = copy::ALL.iter().map(|s| s.to_string()).collect();
        texts.push(typed_words("one").unwrap_err());
        texts.push(typed_words("").unwrap_err());
        for text in &texts {
            assert!(oa_copy::violations(text, &[]).is_empty(), "{text}");
            let lower = text.to_lowercase();
            assert!(
                !lower.contains("sign in to openagents") && !lower.contains("log in"),
                "{text}"
            );
            assert!(
                !lower.contains("nostr") && !lower.contains("nip-"),
                "{text}"
            );
        }
    }
}
