//! Android presentation over the existing account/identity and onboarding owners.
//! No seed generation, wallet storage, account linking, or proof algorithm lives here.
use super::{CoreAccount, CoreError, CoreOnboarding, IdentityView, OnboardingStep, WordCheck};
use rust_native::{Activation, Axis, Element, FieldChange, Node, TextRole, ValidatedView, View};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

trait Backend: Send + Sync {
    fn generation(&self) -> u64;
    fn identity(&self) -> Option<IdentityView>;
    fn steps(&self) -> Vec<OnboardingStep>;
    fn should_show(&self) -> bool;
    fn finish(&self);
    fn start(&self, device: String);
    fn words(&self) -> Result<Vec<String>, CoreError>;
    fn positions(&self) -> Result<Vec<u32>, CoreError>;
    fn confirm(&self, answers: Vec<WordCheck>) -> Result<bool, CoreError>;
    fn restore(
        &self,
        words: String,
        generation: u64,
        npub: Option<String>,
    ) -> Result<IdentityView, CoreError>;
}
struct AccountBackend {
    account: Arc<CoreAccount>,
    onboarding: Arc<CoreOnboarding>,
}
impl Backend for AccountBackend {
    fn generation(&self) -> u64 {
        self.account.snapshot().generation
    }
    fn identity(&self) -> Option<IdentityView> {
        self.account.identity()
    }
    fn steps(&self) -> Vec<OnboardingStep> {
        self.onboarding.steps()
    }
    fn should_show(&self) -> bool {
        self.onboarding.should_show()
    }
    fn finish(&self) {
        self.onboarding.finish()
    }
    fn start(&self, device: String) {
        self.account.start_identity(device)
    }
    fn words(&self) -> Result<Vec<String>, CoreError> {
        self.account.backup_words()
    }
    fn positions(&self) -> Result<Vec<u32>, CoreError> {
        self.account.backup_check()
    }
    fn confirm(&self, answers: Vec<WordCheck>) -> Result<bool, CoreError> {
        self.account.confirm_backup(answers)
    }
    fn restore(
        &self,
        words: String,
        generation: u64,
        npub: Option<String>,
    ) -> Result<IdentityView, CoreError> {
        self.account
            .restore_words_if_current(words, generation, npub)
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
enum Intent {
    Next,
    Skip,
    Backup,
    Restore,
    Close,
    Copy,
    Check,
    Confirm,
    Submit,
    RestoreInput,
    Word(u32),
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Tutorial,
    Home,
    AwaitBackup,
    AwaitRestore,
    Words,
    Confirm,
    Restore,
}
#[derive(Clone, uniffi::Record)]
pub struct RecoveryAuthentication {
    pub token: u64,
    pub title: String,
    pub description: String,
    pub timeout_ms: u32,
}
/// Only an explicit Copy activation returns this value. It is never kept in a view or frame cache.
#[derive(uniffi::Record)]
pub struct RecoveryClipboard {
    pub text: String,
    pub clear_after_ms: u32,
}
#[derive(uniffi::Record)]
pub struct RecoveryFrame {
    pub json: Vec<u8>,
    pub secure: bool,
    pub authentication: Option<RecoveryAuthentication>,
    pub open_chat: bool,
    pub clipboard: Option<RecoveryClipboard>,
}
struct State {
    mode: Mode,
    page: usize,
    revision: u64,
    token: u64,
    auth: Option<(u64, Instant)>,
    fence: Option<(u64, Option<String>)>,
    until: Option<Instant>,
    words: Vec<String>,
    positions: Vec<u32>,
    answers: BTreeMap<u32, String>,
    input: String,
    notice: Option<String>,
    pending: Option<ValidatedView<Intent>>,
    applied: Option<ValidatedView<Intent>>,
    closed: bool,
    started: bool,
}
impl State {
    fn new(show: bool) -> Self {
        Self {
            mode: if show { Mode::Tutorial } else { Mode::Home },
            page: 0,
            revision: 0,
            token: 0,
            auth: None,
            fence: None,
            until: None,
            words: vec![],
            positions: vec![],
            answers: BTreeMap::new(),
            input: String::new(),
            notice: None,
            pending: None,
            applied: None,
            closed: false,
            started: false,
        }
    }
    fn redact(&mut self) {
        self.mode = Mode::Home;
        self.auth = None;
        self.fence = None;
        self.until = None;
        self.words.clear();
        self.positions.clear();
        self.answers.clear();
        self.input.clear();
        self.pending = None;
        self.applied = None;
    }
    fn secure(&self) -> bool {
        matches!(
            self.mode,
            Mode::AwaitBackup | Mode::AwaitRestore | Mode::Words | Mode::Confirm | Mode::Restore
        )
    }
}
#[derive(uniffi::Object)]
pub struct CoreAndroidRecovery {
    backend: Arc<dyn Backend>,
    state: Mutex<State>,
}
fn problem(message: &str) -> CoreError {
    CoreError::InvalidArgument {
        message: message.into(),
    }
}
fn node(key: impl Into<String>, element: Element<Intent>) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Default::default(),
        element,
    }
}
fn text(key: &str, value: impl Into<String>, role: TextRole) -> Node<Intent> {
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
fn field(
    key: impl Into<String>,
    label: String,
    max_bytes: usize,
    on_change: Intent,
) -> Node<Intent> {
    node(
        key,
        Element::Field {
            label,
            value: String::new(),
            placeholder: String::new(),
            secret: true,
            multiline: false,
            enabled: true,
            max_bytes,
            on_change,
        },
    )
}
impl CoreAndroidRecovery {
    fn valid(&self, s: &mut State) -> Result<(), CoreError> {
        if s.closed {
            return Err(CoreError::Closed);
        }
        let now = Instant::now();
        let changed = s.fence.as_ref().is_some_and(|f| {
            *f != (
                self.backend.generation(),
                self.backend.identity().map(|i| i.npub),
            )
        });
        let expired = s.until.is_some_and(|t| t <= now) || s.auth.is_some_and(|(_, t)| t <= now);
        if changed || expired {
            s.redact();
            s.notice = Some("Unlock your device to continue.".into());
        }
        Ok(())
    }
    fn frame(
        &self,
        s: &mut State,
        open_chat: bool,
        clipboard: Option<RecoveryClipboard>,
    ) -> Result<RecoveryFrame, CoreError> {
        self.valid(s)?;
        let clipboard = if s.mode == Mode::Words {
            clipboard
        } else {
            None
        };
        s.revision = s.revision.checked_add(1).ok_or(CoreError::Closed)?;
        let mut body = vec![];
        match s.mode {
            Mode::Tutorial => {
                let steps = self.backend.steps();
                if let Some(p) = steps.get(s.page) {
                    body.push(text("title", &p.title, TextRole::Heading));
                    body.push(text("body", &p.body, TextRole::Body));
                    body.push(button("next", &p.action, Intent::Next));
                    body.push(button("skip", "Skip", Intent::Skip));
                }
            }
            Mode::Home => {
                body.push(text("title", "Your ID", TextRole::Heading));
                if let Some(id) = self.backend.identity() {
                    body.push(text("id", id.npub, TextRole::Body));
                    body.push(button("backup", "Back up your account", Intent::Backup));
                } else {
                    body.push(text(
                        "starting",
                        "Your account is getting ready.",
                        TextRole::Status,
                    ));
                }
                body.push(button("restore", "Restore your account", Intent::Restore));
            }
            Mode::AwaitBackup | Mode::AwaitRestore => {
                body.push(text("title", "Unlock your device", TextRole::Heading));
            }
            Mode::Words => {
                body.push(text("title", "Write down your words", TextRole::Heading));
                for (i, word) in s.words.iter().enumerate() {
                    body.push(text(
                        &format!("word-{i}"),
                        format!("{}. {word}", i + 1),
                        TextRole::Body,
                    ));
                }
                body.push(button("copy", "Copy words", Intent::Copy));
                body.push(button("check", "I've written them down", Intent::Check));
            }
            Mode::Confirm => {
                body.push(text("title", "Check your backup", TextRole::Heading));
                for &position in &s.positions {
                    body.push(field(
                        format!("answer-{position}"),
                        format!("Word {position}"),
                        64,
                        Intent::Word(position),
                    ));
                }
                body.push(button("confirm", "Confirm", Intent::Confirm));
            }
            Mode::Restore => {
                body.push(text("title", "Restore your account", TextRole::Heading));
                body.push(field(
                    "restore-words",
                    "Recovery words".into(),
                    1024,
                    Intent::RestoreInput,
                ));
                body.push(button("submit", "Restore", Intent::Submit));
            }
        }
        if let Some(notice) = &s.notice {
            body.push(text("notice", notice, TextRole::Status));
        }
        body.push(button("close", "Close", Intent::Close));
        let view = View::new_v3(
            "android-recovery",
            s.revision,
            node(
                "root",
                Element::Stack {
                    axis: Axis::Vertical,
                    children: body,
                },
            ),
        )
        .validate()
        .map_err(|_| problem("Couldn't open this screen."))?;
        let json = view
            .to_json()
            .map_err(|_| problem("Couldn't open this screen."))?;
        s.pending = Some(view);
        let authentication = s.auth.map(|(token, _)| RecoveryAuthentication {
            token,
            title: "Unlock your device".into(),
            description: "Confirm it's you to open your recovery words.".into(),
            timeout_ms: 60_000,
        });
        Ok(RecoveryFrame {
            json,
            secure: s.secure(),
            authentication,
            open_chat,
            clipboard,
        })
    }
    fn request(&self, s: &mut State, backup: bool) -> Result<(), CoreError> {
        s.redact();
        s.token = s.token.checked_add(1).ok_or(CoreError::Closed)?;
        s.mode = if backup {
            Mode::AwaitBackup
        } else {
            Mode::AwaitRestore
        };
        s.fence = Some((
            self.backend.generation(),
            self.backend.identity().map(|i| i.npub),
        ));
        s.auth = Some((s.token, Instant::now() + Duration::from_secs(60)));
        Ok(())
    }
    fn action(&self, s: &mut State, intent: Intent) -> Result<RecoveryFrame, CoreError> {
        self.valid(s)?;
        s.notice = None;
        let mut open_chat = false;
        let mut clipboard = None;
        match intent {
            Intent::Next | Intent::Skip => {
                if s.mode != Mode::Tutorial {
                    return Err(CoreError::Closed);
                }
                s.page += 1;
                if intent == Intent::Skip || s.page >= self.backend.steps().len() {
                    self.backend.finish();
                    s.redact();
                    open_chat = true;
                }
            }
            Intent::Backup => self.request(s, true)?,
            Intent::Restore => self.request(s, false)?,
            Intent::Close => {
                s.redact();
                open_chat = true;
            }
            Intent::Copy if s.mode == Mode::Words => {
                clipboard = Some(RecoveryClipboard {
                    text: s.words.join(" "),
                    clear_after_ms: 60_000,
                })
            }
            Intent::Check if s.mode == Mode::Words => {
                let count = s.words.len() as u32;
                let positions = self.backend.positions()?;
                if positions.len() != 3
                    || positions.iter().any(|p| *p == 0 || *p > count)
                    || positions
                        .iter()
                        .collect::<std::collections::BTreeSet<_>>()
                        .len()
                        != 3
                {
                    return Err(problem("Couldn't check your backup. Try again."));
                }
                s.words.clear();
                s.positions = positions;
                s.answers.clear();
                s.mode = Mode::Confirm;
            }
            Intent::Confirm if s.mode == Mode::Confirm => {
                if s.positions
                    .iter()
                    .any(|p| !s.answers.get(p).is_some_and(|w| !w.trim().is_empty()))
                {
                    s.notice = Some("Enter each requested word.".into());
                } else {
                    let answers = s
                        .positions
                        .iter()
                        .map(|p| WordCheck {
                            position: *p,
                            word: s.answers.get(p).cloned().unwrap_or_default(),
                        })
                        .collect();
                    match self.backend.confirm(answers) {
                        Ok(true) => {
                            s.redact();
                            s.notice = Some("Your backup is saved.".into());
                        }
                        Ok(false) => s.notice = Some("Those words don't match. Try again.".into()),
                        Err(_) => {
                            s.redact();
                            s.notice = Some(
                                "Couldn't check your backup. Unlock your device and try again."
                                    .into(),
                            );
                        }
                    }
                }
            }
            Intent::Submit if s.mode == Mode::Restore => {
                let input = std::mem::take(&mut s.input);
                let (generation, npub) = s.fence.clone().ok_or(CoreError::Closed)?;
                match self.backend.restore(input, generation, npub) {
                    Ok(_) => {
                        s.redact();
                        s.notice = Some("Your account is being restored.".into());
                    }
                    Err(_) => {
                        s.redact();
                        s.notice=Some("Couldn't restore your account. Check your words and unlock your device to try again.".into());
                    }
                }
            }
            _ => return Err(CoreError::Closed),
        }
        self.frame(s, open_chat, clipboard)
    }
}
#[uniffi::export]
impl CoreAndroidRecovery {
    /// Construct after A3 supplies the shared account and identity stores. Does not create an identity.
    #[uniffi::constructor]
    pub fn new(account: Arc<CoreAccount>, onboarding: Arc<CoreOnboarding>) -> Arc<Self> {
        let backend: Arc<dyn Backend> = Arc::new(AccountBackend {
            account,
            onboarding,
        });
        let state = Mutex::new(State::new(backend.should_show()));
        Arc::new(Self { backend, state })
    }
    /// The shared Rust start path preserves a provider account and existing wallet seed.
    pub fn start(&self, device_name: String) -> Result<RecoveryFrame, CoreError> {
        let mut s = self.state.lock().unwrap();
        self.valid(&mut s)?;
        if !s.started {
            s.started = true;
            if self.backend.identity().is_none() {
                self.backend.start(device_name);
            }
        }
        let open_chat = s.mode == Mode::Home && !self.backend.should_show();
        self.frame(&mut s, open_chat, None)
    }
    pub fn render(&self) -> Result<RecoveryFrame, CoreError> {
        self.frame(&mut self.state.lock().unwrap(), false, None)
    }
    pub fn acknowledge(&self, instance: String, revision: u64) -> Result<(), CoreError> {
        let mut s = self.state.lock().unwrap();
        self.valid(&mut s)?;
        let view = s.pending.as_ref().ok_or(CoreError::Closed)?;
        if view.view().instance != instance || view.view().revision != revision {
            return Err(CoreError::Closed);
        }
        s.applied = s.pending.take();
        Ok(())
    }
    pub fn activate(
        &self,
        instance: String,
        revision: u64,
        key: String,
    ) -> Result<RecoveryFrame, CoreError> {
        let mut s = self.state.lock().unwrap();
        self.valid(&mut s)?;
        let intent = *s
            .applied
            .as_ref()
            .ok_or(CoreError::Closed)?
            .activate(&Activation {
                instance,
                revision,
                node: key,
            })
            .map_err(|_| CoreError::Closed)?;
        self.action(&mut s, intent)
    }
    pub fn change(
        &self,
        instance: String,
        revision: u64,
        key: String,
        value: String,
    ) -> Result<(), CoreError> {
        let mut s = self.state.lock().unwrap();
        self.valid(&mut s)?;
        let event = FieldChange {
            activation: Activation {
                instance,
                revision,
                node: key,
            },
            value,
        };
        let intent = *s
            .applied
            .as_ref()
            .ok_or(CoreError::Closed)?
            .change_field(&event)
            .map_err(|_| CoreError::Closed)?;
        match intent {
            Intent::RestoreInput if s.mode == Mode::Restore => s.input = event.value,
            Intent::Word(p) if s.mode == Mode::Confirm && s.positions.contains(&p) => {
                s.answers.insert(p, event.value);
            }
            _ => return Err(CoreError::Closed),
        }
        Ok(())
    }
    /// Only the platform authenticator's matching, live callback may answer this request.
    pub fn authenticated(&self, token: u64, accepted: bool) -> Result<RecoveryFrame, CoreError> {
        let mut s = self.state.lock().unwrap();
        self.valid(&mut s)?;
        if s.auth.as_ref().map(|a| a.0) != Some(token) {
            return Err(CoreError::Closed);
        }
        s.auth = None;
        if !accepted {
            s.redact();
            s.notice = Some("Unlock your device to continue.".into());
        } else {
            s.until = Some(Instant::now() + Duration::from_secs(60));
            match s.mode {
                Mode::AwaitBackup => match self.backend.words() {
                    Ok(words) if matches!(words.len(), 12 | 24) => {
                        s.words = words;
                        s.mode = Mode::Words;
                    }
                    _ => {
                        s.redact();
                        s.notice = Some(
                            "Couldn't open your words. Unlock your device and try again.".into(),
                        );
                    }
                },
                Mode::AwaitRestore => s.mode = Mode::Restore,
                _ => return Err(CoreError::Closed),
            }
        }
        self.frame(&mut s, false, None)
    }
    pub fn redact(&self) {
        let mut s = self.state.lock().unwrap();
        s.redact();
    }
    pub fn close(&self) {
        let mut s = self.state.lock().unwrap();
        s.redact();
        s.closed = true;
    }
}
#[cfg(test)]
mod tests;

#[uniffi::export]
impl CoreAccount {
    /// Restore only for the account/identity that passed this screen's device authentication.
    pub fn restore_words_if_current(
        &self,
        words: String,
        generation: u64,
        npub: Option<String>,
    ) -> Result<IdentityView, CoreError> {
        let computer = self.device_name.lock().unwrap().clone();
        let (identity, task) = self.controller().restore_identity_if_current(
            generation,
            || {
                let mut loaded = self.identity.lock().unwrap();
                if loaded.as_ref().map(|i| i.npub()) != npub {
                    return Err(CoreError::Closed);
                }
                let identity = Arc::new(self.identities.restore(&words, false).map_err(|_| {
                    problem("Couldn't restore your account. Check your words and try again.")
                })?);
                *loaded = Some(identity.clone());
                Ok(identity)
            },
            CoreError::Closed,
            computer,
        )?;
        let view = IdentityView {
            npub: identity.npub(),
            words_saved: self.identities.words_saved(&identity),
            icloud_copy: self.identities.icloud_copy(),
        };
        // The existing generation watcher retires old clients. Do not close a newer account's clients here.
        openagents_client::runtime::shared().spawn(task);
        Ok(view)
    }
}
