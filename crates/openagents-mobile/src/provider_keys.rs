//! Account > **Your keys** (BYOK, #10176,
//! `docs/byok/2026-10-02-byok-openrouter.md`): the person's own
//! OpenRouter, Vercel AI Gateway, and TypeSafe keys on the phone.
//!
//! The platform host keeps each key in its protected store (a
//! this-device-only Keychain item on iOS, an Android Keystore-encrypted file
//! with no backup on Android) and hands the keys to Rust at start
//! ([`ProviderKeys::load`]). Rust holds them in memory only, tests a new key
//! with the provider's cheapest call before it counts
//! ([`model_access::check`]), and installs who pays
//! ([`model_access::install`]), so the hosted chat seals them into each
//! job's `payer.keys` envelope under `mine`
//! (`openagents_chat::basic_coder::seal_payer`).
//!
//! A key never reaches the app packet, a log line, or a `Debug` string: a
//! row carries the provider, the key's last four characters, and the last
//! test's word and line. The host learns whether to keep a key it collected
//! from [`Done`], which names the provider only.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard};

use model_access::check::{self, State};
use model_access::{
    Access, ApiKey, Failure, Keys, MAX_KEY_BYTES, Mode, PROVIDERS, Provider, TYPESAFE_ONLY,
};
use serde::{Deserialize, Serialize};

/// What the host collects a key with: a secure, never-echoed field. The
/// purpose tells the host to keep the value out of autofill suggestions,
/// the pasteboard history, and screenshots of the field.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct KeyInput {
    /// Always `provider_key`.
    pub purpose: &'static str,
    /// Always true: the field never shows what was typed or pasted.
    pub secret: bool,
    pub label: String,
    pub prompt: String,
    /// The longest key the field takes, in bytes.
    pub max_bytes: usize,
}

/// One provider's row. Never the key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Row {
    /// `openrouter`, `vercel`, or `typesafe`.
    pub provider: &'static str,
    /// The name a person reads.
    pub name: &'static str,
    /// The key's last four characters, when one is added.
    pub last_four: Option<String>,
    /// The last test's word: `works`, `no credits`, `refused`, or
    /// `unchecked`.
    pub state: Option<&'static str>,
    /// The last test's line ("Your OpenRouter key works.").
    pub line: Option<String>,
    /// A test of this provider's key is running.
    pub checking: bool,
    /// Where the person makes a key.
    pub page: &'static str,
    /// How the host asks for this provider's key.
    pub input: KeyInput,
}

/// A key the host collected was tested: keep it in the protected store
/// (`kept`) or drop it. `ask_mine` asks "Use your keys for everything?",
/// once, after a chat-capable key is added while OpenAgents pays.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Done {
    pub provider: &'static str,
    pub kept: bool,
    pub ask_mine: bool,
}

/// The section as the Account screen shows it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct View {
    pub rows: Vec<Row>,
    /// Every model call runs on the person's keys. The host saves it (it
    /// is not a secret) and sends it back with the keys at start.
    pub mine: bool,
    /// Why "Use my keys for everything" can't be turned on now.
    pub mine_blocked: Option<String>,
    /// Who pays for model calls, such as "Running on your keys."
    pub status: String,
    /// The last thing a tap said, such as the TypeSafe-only refusal.
    pub notice: Option<String>,
    /// Keys the host collected whose test finished since the last packet.
    pub done: Vec<Done>,
}

/// One key the host read from its protected store.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stored {
    pub provider: String,
    pub key: String,
}

impl std::fmt::Debug for Stored {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stored")
            .field("provider", &self.provider)
            .field("key", &"***")
            .finish()
    }
}

/// The future a key test returns.
pub type Checking = Pin<Box<dyn Future<Output = State> + Send>>;

/// Tests a key with the provider's cheapest call.
pub trait Check: Send + Sync {
    fn check(&self, provider: Provider, key: ApiKey) -> Checking;
}

/// The real test, over HTTPS.
pub struct Https;

impl Check for Https {
    fn check(&self, provider: Provider, key: ApiKey) -> Checking {
        Box::pin(async move {
            let request = check::request(provider);
            let Ok(client) = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(20))
                .build()
            else {
                return State::Unknown(Failure::NoConnection(provider));
            };
            let builder = match request.method {
                "POST" => client.post(request.url),
                _ => client.get(request.url),
            };
            let mut builder = builder.bearer_auth(key.expose());
            if let Some(body) = &request.body {
                builder = builder.json(body);
            }
            match builder.send().await {
                Ok(response) => {
                    let status = response.status().as_u16();
                    let body = response
                        .bytes()
                        .await
                        .map(|b| b.to_vec())
                        .unwrap_or_default();
                    check::read(provider, Some(status), &body)
                }
                Err(_) => check::read(provider, None, &[]),
            }
        })
    }
}

#[derive(Default)]
struct Inner {
    keys: Keys,
    mine: bool,
    /// The last test of each provider's key: its word and line.
    tested: Vec<(Provider, &'static str, String)>,
    checking: Vec<Provider>,
    /// A stored key's last test failed for its own reason: the status line
    /// says so while `mine`.
    last: Option<Failure>,
    notice: Option<String>,
    done: Vec<Done>,
}

impl Inner {
    fn tested(&mut self, provider: Provider, state: &State) {
        self.tested.retain(|(p, _, _)| *p != provider);
        self.tested
            .push((provider, state.word(), state.line(provider)));
    }

    fn remove(&mut self, provider: Provider) {
        let mut kept = Keys::none();
        for (p, key) in self.keys.iter() {
            if *p != provider {
                kept.insert(*p, key.clone());
            }
        }
        self.keys = kept;
        self.tested.retain(|(p, _, _)| *p != provider);
        if matches!(self.last, Some(Failure::Refused(p) | Failure::NoCredits(p)) if p == provider) {
            self.last = None;
        }
        // Removing the last key that can answer chat returns to ours.
        if !self.keys.chat_capable() {
            self.mine = false;
        }
    }

    fn access(&self) -> Access {
        let mode = if self.mine { Mode::Mine } else { Mode::Ours };
        Access::new(mode, self.keys.clone(), &Keys::none())
    }
}

/// The phone's own keys and who pays.
#[derive(Clone)]
pub struct ProviderKeys {
    inner: Arc<Mutex<Inner>>,
    /// Install each change process-wide; off in tests, which share it.
    install: bool,
    /// Ring the host after a test finishes in the background.
    wake: fn(),
}

impl Default for ProviderKeys {
    fn default() -> Self {
        Self::new(false, || {})
    }
}

fn provider_of(word: &str) -> Option<Provider> {
    Provider::parse(word).ok()
}

impl ProviderKeys {
    #[must_use]
    pub fn new(install: bool, wake: fn()) -> Self {
        Self {
            inner: Arc::default(),
            install,
            wake,
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn changed(&self, inner: &Inner) {
        if self.install {
            model_access::install(inner.access());
        }
    }

    /// Who pays now.
    #[must_use]
    pub fn access(&self) -> Access {
        self.lock().access()
    }

    /// The keys the host read from its protected store at start, and the
    /// saved switch. Earlier keys are replaced. `mine` without a key that
    /// can answer chat stays off.
    pub fn load(&self, stored: Vec<Stored>, mine: bool) {
        let mut inner = self.lock();
        let mut keys = Keys::none();
        for Stored { provider, key } in stored {
            if let Some(provider) = provider_of(&provider)
                && key.len() <= MAX_KEY_BYTES
            {
                keys.insert(provider, ApiKey::new(key));
            }
        }
        inner.mine = mine && keys.chat_capable();
        inner.keys = keys;
        self.changed(&inner);
    }

    /// Test a key the person entered; it counts only once the provider
    /// accepts it. The host keeps it when [`Done::kept`] says so.
    pub fn add(
        &self,
        provider: &str,
        key: String,
        check: &Arc<dyn Check>,
        handle: &tokio::runtime::Handle,
    ) {
        let Some(provider) = provider_of(provider) else {
            return;
        };
        let key = ApiKey::new(key);
        let mut inner = self.lock();
        if key.is_empty() || key.expose().len() > MAX_KEY_BYTES {
            inner.notice = Some(format!("{} didn't accept that key.", provider.name()));
            inner.done.push(Done {
                provider: provider.word(),
                kept: false,
                ask_mine: false,
            });
            return;
        }
        inner.notice = None;
        if !inner.checking.contains(&provider) {
            inner.checking.push(provider);
        }
        drop(inner);
        let this = self.clone();
        let test = check.check(provider, key.clone());
        handle.spawn(async move {
            let state = test.await;
            this.added(provider, key, &state);
            (this.wake)();
        });
    }

    /// A new key's test finished.
    pub fn added(&self, provider: Provider, key: ApiKey, state: &State) {
        let mut inner = self.lock();
        inner.checking.retain(|p| *p != provider);
        inner.tested(provider, state);
        let kept = state.storable();
        let ask_mine = kept && !inner.mine && provider.chat_capable();
        if kept {
            inner.keys.insert(provider, key);
            if matches!(inner.last, Some(Failure::Refused(p) | Failure::NoCredits(p)) if p == provider)
            {
                inner.last = None;
            }
        }
        inner.notice = Some(state.line(provider));
        inner.done.push(Done {
            provider: provider.word(),
            kept,
            ask_mine,
        });
        self.changed(&inner);
    }

    /// Test the stored key again.
    pub fn test(&self, provider: &str, check: &Arc<dyn Check>, handle: &tokio::runtime::Handle) {
        let Some(provider) = provider_of(provider) else {
            return;
        };
        let mut inner = self.lock();
        let Some(key) = inner.keys.get(provider).cloned() else {
            return;
        };
        if !inner.checking.contains(&provider) {
            inner.checking.push(provider);
        }
        drop(inner);
        let this = self.clone();
        let test = check.check(provider, key);
        handle.spawn(async move {
            let state = test.await;
            this.retested(provider, &state);
            (this.wake)();
        });
    }

    /// A stored key's test finished. The key stays stored whatever it
    /// says; a refused or empty key shows on the status line.
    pub fn retested(&self, provider: Provider, state: &State) {
        let mut inner = self.lock();
        inner.checking.retain(|p| *p != provider);
        inner.tested(provider, state);
        inner.last = match state {
            State::Refused => Some(Failure::Refused(provider)),
            State::NoCredits => Some(Failure::NoCredits(provider)),
            _ => inner.last.take().filter(|failure| {
                !matches!(failure, Failure::Refused(p) | Failure::NoCredits(p) if *p == provider)
            }),
        };
        inner.notice = Some(state.line(provider));
    }

    /// Remove the key; the host deletes it from its protected store first.
    pub fn remove(&self, provider: &str) {
        let Some(provider) = provider_of(provider) else {
            return;
        };
        let mut inner = self.lock();
        inner.remove(provider);
        inner.notice = Some(format!("Removed your {} key.", provider.name()));
        self.changed(&inner);
    }

    /// "Use my keys for everything", on or off. On needs a key that can
    /// answer chat; otherwise it stays off and says why.
    pub fn set_mine(&self, on: bool) {
        let mut inner = self.lock();
        if on && !inner.keys.chat_capable() {
            inner.notice = Some(TYPESAFE_ONLY.to_owned());
            inner.mine = false;
        } else {
            inner.notice = None;
            inner.mine = on;
        }
        self.changed(&inner);
    }

    /// The section for the packet. Finished tests leave [`View::done`]
    /// once they are read.
    pub fn view(&self) -> View {
        let mut inner = self.lock();
        let rows = PROVIDERS
            .into_iter()
            .map(|provider| {
                let tested = inner.tested.iter().find(|(p, _, _)| *p == provider);
                Row {
                    provider: provider.word(),
                    name: provider.name(),
                    last_four: inner.keys.get(provider).map(ApiKey::last_four),
                    state: tested.map(|(_, word, _)| *word),
                    line: tested.map(|(_, _, line)| line.clone()),
                    checking: inner.checking.contains(&provider),
                    page: provider.key_page(),
                    input: KeyInput {
                        purpose: "provider_key",
                        secret: true,
                        label: format!("{} key", provider.name()),
                        prompt: format!(
                            "Paste your {} API key. It stays on this phone and goes only to {} with your chats.",
                            provider.name(),
                            provider.name()
                        ),
                        max_bytes: MAX_KEY_BYTES,
                    },
                }
            })
            .collect();
        let mode = if inner.mine { Mode::Mine } else { Mode::Ours };
        let mine_blocked = (!inner.keys.chat_capable()).then(|| {
            if inner.keys.is_empty() {
                "Add an OpenRouter or Vercel AI Gateway key first.".to_owned()
            } else {
                TYPESAFE_ONLY.to_owned()
            }
        });
        View {
            rows,
            mine: inner.mine,
            mine_blocked,
            // The shared line names the computer's setting; the phone has
            // its own switch on this screen.
            status: match mode {
                Mode::Ours => OURS_ON_PHONE.to_owned(),
                Mode::Mine => model_access::status_line(mode, &inner.keys, inner.last.as_ref()),
            },
            notice: inner.notice.clone(),
            done: std::mem::take(&mut inner.done),
        }
    }
}

/// The status line while OpenAgents pays for model calls.
pub const OURS_ON_PHONE: &str = "OpenAgents pays for your model calls. To use your own, add a key and turn on Use my keys for everything.";

#[cfg(test)]
mod tests {
    use super::*;

    /// The status line while OpenAgents pays for model calls.
    const OURS: &str = OURS_ON_PHONE;

    const KEY: &str = "sk-or-v1-0123456789abcdefWXYZ";

    fn works() -> State {
        State::Works {
            label: None,
            remaining_usd: Some(5.0),
            spent_usd: Some(1.0),
        }
    }

    struct Fake(State);

    impl Check for Fake {
        fn check(&self, _: Provider, _: ApiKey) -> Checking {
            let state = self.0.clone();
            Box::pin(async move { state })
        }
    }

    fn json(view: &View) -> String {
        serde_json::to_string(view).unwrap()
    }

    /// A key the provider accepts is kept, shows only its last four
    /// characters, and the host is told to keep it and asked about mine.
    #[test]
    fn an_accepted_key_is_kept_and_never_shown() {
        let keys = ProviderKeys::default();
        keys.added(Provider::OpenRouter, ApiKey::new(KEY), &works());
        let view = keys.view();
        let text = json(&view);
        assert!(!text.contains("0123456789"), "{text}");
        assert!(!format!("{view:?}").contains("0123456789"));
        assert_eq!(view.rows[0].last_four.as_deref(), Some("WXYZ"));
        assert_eq!(view.rows[0].state, Some("works"));
        assert_eq!(
            view.done,
            vec![Done {
                provider: "openrouter",
                kept: true,
                ask_mine: true
            }]
        );
        assert!(
            view.rows
                .iter()
                .all(|row| row.input.secret && row.input.purpose == "provider_key")
        );
        assert_eq!(view.status, OURS);
        // Done is read once.
        assert!(keys.view().done.is_empty());
        assert!(!keys.access().is_mine());
    }

    /// A refused key is not kept, and the host drops it.
    #[test]
    fn a_refused_key_is_dropped() {
        let keys = ProviderKeys::default();
        keys.added(Provider::Vercel, ApiKey::new("vck_bad"), &State::Refused);
        let view = keys.view();
        assert_eq!(view.rows[1].last_four, None);
        assert_eq!(
            view.notice.as_deref(),
            Some("Vercel AI Gateway didn't accept that key.")
        );
        assert_eq!(view.done[0].kept, false);
        assert!(view.mine_blocked.is_some());
    }

    /// With only a TypeSafe key, the switch stays off with the one line.
    #[test]
    fn typesafe_alone_cannot_turn_on_mine() {
        let keys = ProviderKeys::default();
        keys.load(
            vec![Stored {
                provider: "typesafe".into(),
                key: "ts-key-1234".into(),
            }],
            true,
        );
        assert!(
            !keys.view().mine,
            "a saved mine without a chat key stays off"
        );
        keys.set_mine(true);
        let view = keys.view();
        assert!(!view.mine);
        assert_eq!(view.notice.as_deref(), Some(TYPESAFE_ONLY));
        assert_eq!(view.mine_blocked.as_deref(), Some(TYPESAFE_ONLY));
        assert!(!keys.access().is_mine());
    }

    /// Mine sends every call to the person's keys; removing the last chat
    /// key returns to ours.
    #[test]
    fn mine_runs_on_their_keys_until_the_last_chat_key_goes() {
        let keys = ProviderKeys::default();
        keys.load(
            vec![Stored {
                provider: "openrouter".into(),
                key: KEY.into(),
            }],
            true,
        );
        let view = keys.view();
        assert!(view.mine);
        assert_eq!(view.status, "Running on your keys.");
        let access = keys.access();
        assert!(access.is_mine());
        assert!(access.keys().get(Provider::OpenRouter).is_some());
        keys.remove("openrouter");
        let view = keys.view();
        assert!(!view.mine);
        assert_eq!(view.status, OURS);
        assert!(!keys.access().is_mine());
    }

    /// A stored key the provider now refuses stays stored and shows on the
    /// status line; a passing test clears it.
    #[test]
    fn a_refused_stored_key_shows_on_the_status_line() {
        let keys = ProviderKeys::default();
        keys.load(
            vec![Stored {
                provider: "openrouter".into(),
                key: KEY.into(),
            }],
            true,
        );
        keys.retested(Provider::OpenRouter, &State::Refused);
        let view = keys.view();
        assert_eq!(view.rows[0].last_four.as_deref(), Some("WXYZ"));
        assert_eq!(
            view.status,
            "Your OpenRouter key was refused. Update it in Settings. Nothing is running on ours."
        );
        keys.retested(Provider::OpenRouter, &works());
        assert_eq!(keys.view().status, "Running on your keys.");
    }

    /// The test runs in the background; the row says it is checking until
    /// it finishes.
    #[test]
    fn a_key_is_tested_off_the_calling_thread() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();
        let keys = ProviderKeys::default();
        let check: Arc<dyn Check> = Arc::new(Fake(works()));
        keys.add("openrouter", KEY.into(), &check, runtime.handle());
        for _ in 0..200 {
            if keys.view().rows[0].last_four.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let view = keys.view();
        assert!(!view.rows[0].checking);
        assert_eq!(view.rows[0].last_four.as_deref(), Some("WXYZ"));
    }

    /// A key read from the host never prints.
    #[test]
    fn a_stored_key_never_prints() {
        let stored = Stored {
            provider: "openrouter".into(),
            key: KEY.into(),
        };
        assert!(!format!("{stored:?}").contains("0123456789"));
    }
}
