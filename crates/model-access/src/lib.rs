//! Who pays for a model call (BYOK, `docs/byok/2026-10-02-byok-openrouter.md`).
//!
//! Every model call OpenAgents makes on a person's behalf asks this crate
//! one question: does it go on OpenAgents' keys, as it always has, or on the
//! person's own OpenRouter, Vercel AI Gateway, or TypeSafe key? The answer
//! is an [`Access`], built once at start from the person's settings
//! (`models.payer`), their stored keys ([`store`]), and any key given for
//! one invocation ([`Keys::once`]).
//!
//! - [`Mode::Ours`] is the default and today's behaviour: stored keys are
//!   kept but not used, and every call site keeps its own door.
//! - [`Mode::Mine`] sends every call to one of the person's keys in a fixed
//!   order ([`Access::chat`], [`Access::decisions`]) and never to one of
//!   ours. A call their keys cannot make fails with one plain line
//!   ([`Failure::line`]).
//!
//! An ambient `OPENROUTER_API_KEY`, `AI_GATEWAY_API_KEY`, or
//! `TYPESAFE_API_KEY` never switches anyone to their own keys (#7955): only
//! the `OPENAGENTS_*_KEY` variables ([`Provider::once_var`]), the
//! `--openrouter-key`-style flags, and a stored key under `mine` do.
//!
//! A key never appears in a `Debug` string, a log line, or a record:
//! records carry the provider and a [`fingerprint`] only.

pub mod access;
pub mod check;
pub mod store;

use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub use access::{Access, ChatDoor, Decisions, Doors, NoDoor, Use};

/// A provider the person can hold a key for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    /// OpenRouter: chat models, embeddings, and Jev through its Decisions API.
    #[serde(rename = "openrouter")]
    OpenRouter,
    /// The Vercel AI Gateway: chat models, embeddings, and Jev.
    Vercel,
    /// TypeSafe: Jev (System One) decisions only.
    #[serde(rename = "typesafe")]
    TypeSafe,
}

/// Every provider, in the order a person reads them.
pub const PROVIDERS: [Provider; 3] = [Provider::OpenRouter, Provider::Vercel, Provider::TypeSafe];

impl Provider {
    /// The word a command takes: `openrouter`, `vercel`, or `typesafe`.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Provider::OpenRouter => "openrouter",
            Provider::Vercel => "vercel",
            Provider::TypeSafe => "typesafe",
        }
    }

    /// The name a person reads.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Provider::OpenRouter => "OpenRouter",
            Provider::Vercel => "Vercel AI Gateway",
            Provider::TypeSafe => "TypeSafe",
        }
    }

    /// Read a command's word.
    ///
    /// # Errors
    /// A sentence naming the three providers.
    pub fn parse(word: &str) -> Result<Self, String> {
        match word.trim().to_ascii_lowercase().as_str() {
            "openrouter" | "open-router" => Ok(Provider::OpenRouter),
            "vercel" | "ai-gateway" | "gateway" => Ok(Provider::Vercel),
            "typesafe" | "jev" => Ok(Provider::TypeSafe),
            other => Err(format!(
                "`{other}` is not a provider: openrouter, vercel, or typesafe"
            )),
        }
    }

    /// Whether a key here can answer chat: OpenRouter and the gateway can;
    /// TypeSafe serves Jev only.
    #[must_use]
    pub const fn chat_capable(self) -> bool {
        !matches!(self, Provider::TypeSafe)
    }

    /// The variable that gives this provider's key for one invocation,
    /// never stored: `OPENAGENTS_OPENROUTER_KEY` and the rest. It is
    /// distinct from the provider's own variable on purpose (#7955).
    #[must_use]
    pub const fn once_var(self) -> &'static str {
        match self {
            Provider::OpenRouter => "OPENAGENTS_OPENROUTER_KEY",
            Provider::Vercel => "OPENAGENTS_VERCEL_KEY",
            Provider::TypeSafe => "OPENAGENTS_TYPESAFE_KEY",
        }
    }

    /// The global flag that gives this provider's key for one invocation.
    #[must_use]
    pub const fn flag(self) -> &'static str {
        match self {
            Provider::OpenRouter => "--openrouter-key",
            Provider::Vercel => "--vercel-key",
            Provider::TypeSafe => "--typesafe-key",
        }
    }

    /// The page where the person makes a key.
    #[must_use]
    pub const fn key_page(self) -> &'static str {
        match self {
            Provider::OpenRouter => "https://openrouter.ai/settings/keys",
            Provider::Vercel => "https://vercel.com/d?to=%2F%5Bteam%5D%2F%7E%2Fai%2Fapi-keys",
            Provider::TypeSafe => "https://typesafe.ai",
        }
    }

    /// The base URL of the provider's OpenAI-compatible API, for chat and
    /// embeddings; `None` for TypeSafe.
    #[must_use]
    pub const fn openai_base(self) -> Option<&'static str> {
        match self {
            Provider::OpenRouter => Some("https://openrouter.ai/api/v1"),
            Provider::Vercel => Some("https://ai-gateway.vercel.sh/v1"),
            Provider::TypeSafe => None,
        }
    }
}

impl fmt::Display for Provider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.word())
    }
}

/// A provider key. It never prints: `Debug` and `Display` show the
/// provider-neutral `***`.
#[derive(Clone, PartialEq, Eq)]
pub struct ApiKey(String);

impl ApiKey {
    /// Hold a key, trimmed.
    #[must_use]
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into().trim().to_owned())
    }

    /// The key itself, for the one place it goes on the wire.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Whether the key is blank.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The last four characters, which a settings row may show.
    #[must_use]
    pub fn last_four(&self) -> String {
        let chars: Vec<char> = self.0.chars().collect();
        chars[chars.len().saturating_sub(4)..].iter().collect()
    }

    /// The key's [`fingerprint`].
    #[must_use]
    pub fn fingerprint(&self) -> String {
        fingerprint(&self.0)
    }
}

impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApiKey(***)")
    }
}

impl fmt::Display for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

impl Drop for ApiKey {
    fn drop(&mut self) {
        // Overwrite the bytes before the allocation is freed, so a key is
        // not left readable in freed memory after its job.
        // SAFETY: zero bytes keep the string valid UTF-8.
        unsafe {
            for byte in self.0.as_bytes_mut() {
                std::ptr::write_volatile(byte, 0);
            }
        }
    }
}

/// What a record names instead of a key: the first 8 hex characters of the
/// key's SHA-256 digest.
#[must_use]
pub fn fingerprint(key: &str) -> String {
    let digest = Sha256::digest(key.trim().as_bytes());
    digest.iter().take(4).map(|b| format!("{b:02x}")).collect()
}

/// Who pays for model calls: the setting `models.payer`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// OpenAgents' keys, today's behaviour. Stored keys are kept, unused.
    #[default]
    Ours,
    /// The person's keys for every call, never ours.
    Mine,
}

impl Mode {
    /// The setting's word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Mode::Ours => "ours",
            Mode::Mine => "mine",
        }
    }

    /// Read the setting's word.
    ///
    /// # Errors
    /// The word is not `ours` or `mine`.
    pub fn parse(word: &str) -> Result<Self, String> {
        match word.trim() {
            "ours" | "openagents" => Ok(Mode::Ours),
            "mine" | "theirs" => Ok(Mode::Mine),
            other => Err(format!("`{other}` is not ours or mine")),
        }
    }
}

/// Who paid for one call, as a record names it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "payer", rename_all = "lowercase")]
pub enum Payer {
    /// OpenAgents.
    Ours,
    /// The person, on the key with this fingerprint.
    Theirs {
        provider: Provider,
        fingerprint: String,
    },
}

impl Payer {
    /// The word a usage log or record carries: `ours` or `theirs`.
    #[must_use]
    pub const fn word(&self) -> &'static str {
        match self {
            Payer::Ours => "ours",
            Payer::Theirs { .. } => "theirs",
        }
    }
}

/// Why one of the person's keys could not make a call, read from the
/// error's code and never from the provider's words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// 401: a bad or revoked key.
    Refused(Provider),
    /// 402: no credits.
    NoCredits(Provider),
    /// 429: rate-limited.
    RateLimited(Provider),
    /// None of the person's keys can call this model.
    ModelUnavailable(String),
    /// No connection to the provider.
    NoConnection(Provider),
}

impl Failure {
    /// The failure an HTTP status from `provider` means, when it is one of
    /// the person's key's own reasons; `None` otherwise.
    #[must_use]
    pub fn of_status(provider: Provider, status: u16) -> Option<Self> {
        match status {
            401 | 403 => Some(Failure::Refused(provider)),
            402 => Some(Failure::NoCredits(provider)),
            429 => Some(Failure::RateLimited(provider)),
            502..=504 => Some(Failure::NoConnection(provider)),
            _ => None,
        }
    }

    /// Whether the call may move to the person's next key: their key failed
    /// for its own reasons (401, 402, 429, no connection).
    #[must_use]
    pub const fn fails_over(&self) -> bool {
        !matches!(self, Failure::ModelUnavailable(_))
    }

    /// The one plain line the person sees.
    #[must_use]
    pub fn line(&self) -> String {
        match self {
            Failure::Refused(p) => {
                format!("Your {} key was refused. Update it in Settings.", p.name())
            }
            Failure::NoCredits(p) => format!(
                "Your {} account is out of credits. Add credits there, or switch to OpenAgents in Settings.",
                p.name()
            ),
            Failure::RateLimited(p) => {
                format!("{} is rate-limiting your key; try again shortly.", p.name())
            }
            Failure::ModelUnavailable(model) => format!("Your keys can't use {model}."),
            Failure::NoConnection(p) => format!("Couldn't reach {}; try again.", p.name()),
        }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.line())
    }
}

/// The person's keys, at most one per provider.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Keys(Vec<(Provider, ApiKey)>);

impl Keys {
    /// No keys.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// Add or replace `provider`'s key; a blank key is ignored.
    pub fn insert(&mut self, provider: Provider, key: ApiKey) {
        if key.is_empty() {
            return;
        }
        self.0.retain(|(p, _)| *p != provider);
        self.0.push((provider, key));
    }

    /// `provider`'s key.
    #[must_use]
    pub fn get(&self, provider: Provider) -> Option<&ApiKey> {
        self.0.iter().find(|(p, _)| *p == provider).map(|(_, k)| k)
    }

    /// Whether there are no keys.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The providers with a key, in [`PROVIDERS`] order.
    #[must_use]
    pub fn providers(&self) -> Vec<Provider> {
        PROVIDERS
            .into_iter()
            .filter(|p| self.get(*p).is_some())
            .collect()
    }

    /// Whether some key can answer chat.
    #[must_use]
    pub fn chat_capable(&self) -> bool {
        self.0.iter().any(|(p, _)| p.chat_capable())
    }

    /// `self`, with `other`'s keys in place of any for the same provider.
    #[must_use]
    pub fn overlaid(mut self, other: &Keys) -> Self {
        for (provider, key) in &other.0 {
            self.insert(*provider, key.clone());
        }
        self
    }

    /// The keys given for one invocation: each provider's
    /// [`Provider::once_var`] from `env`, then the `--openrouter-key`-style
    /// flags in `args`, which are removed from `args` (both `--flag KEY` and
    /// `--flag=KEY`). Never the ambient provider variables.
    ///
    /// # Errors
    /// A flag with no key after it.
    pub fn once(
        env: &dyn Fn(&str) -> Option<String>,
        args: &mut Vec<String>,
    ) -> Result<Keys, String> {
        let mut keys = Keys::none();
        for provider in PROVIDERS {
            if let Some(key) = env(provider.once_var()) {
                keys.insert(provider, ApiKey::new(key));
            }
        }
        let mut kept = Vec::with_capacity(args.len());
        let mut words = std::mem::take(args).into_iter();
        while let Some(word) = words.next() {
            let flagged = PROVIDERS.into_iter().find_map(|p| {
                if word == p.flag() {
                    Some((p, None))
                } else {
                    word.strip_prefix(p.flag())
                        .and_then(|rest| rest.strip_prefix('='))
                        .map(|key| (p, Some(key.to_owned())))
                }
            });
            match flagged {
                Some((provider, Some(key))) => keys.insert(provider, ApiKey::new(key)),
                Some((provider, None)) => match words.next() {
                    Some(key) => keys.insert(provider, ApiKey::new(key)),
                    None => {
                        *args = kept;
                        return Err(format!("{} needs a key after it", provider.flag()));
                    }
                },
                None => kept.push(word),
            }
        }
        *args = kept;
        Ok(keys)
    }

    /// Each provider and its key.
    pub fn iter(&self) -> impl Iterator<Item = &(Provider, ApiKey)> {
        self.0.iter()
    }
}

static ONCE: std::sync::OnceLock<Keys> = std::sync::OnceLock::new();

/// Remember the keys given for this invocation ([`Keys::once`]), so every
/// surface in the process builds its [`Access`] with them. The first call
/// wins; they are never stored.
pub fn remember_once(keys: Keys) {
    let _ = ONCE.set(keys);
}

/// The keys given for this invocation, if any.
#[must_use]
pub fn once_keys() -> Keys {
    ONCE.get().cloned().unwrap_or_default()
}

/// The status line every surface shows.
#[must_use]
pub fn status_line(mode: Mode, keys: &Keys, last: Option<&Failure>) -> String {
    match (mode, last) {
        (Mode::Ours, _) => "Running on OpenAgents.".into(),
        (Mode::Mine, Some(failure)) => {
            format!("{} Nothing is running on ours.", failure.line())
        }
        (Mode::Mine, None) if !keys.chat_capable() => {
            "Add an OpenRouter or Vercel AI Gateway key to run on your keys.".into()
        }
        (Mode::Mine, None) => "Running on your keys.".into(),
    }
}

/// What turning on `mine` with only these keys says, when it is refused.
pub const TYPESAFE_ONLY: &str = "A TypeSafe key covers decisions only. Add an OpenRouter or Vercel AI Gateway key to run everything on your keys.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_never_prints() {
        let key = ApiKey::new("sk-or-v1-secretsecret");
        assert_eq!(format!("{key:?}"), "ApiKey(***)");
        assert_eq!(format!("{key}"), "***");
        let mut keys = Keys::none();
        keys.insert(Provider::OpenRouter, key.clone());
        assert!(!format!("{keys:?}").contains("secret"));
        assert_eq!(key.last_four(), "cret");
        assert_eq!(key.fingerprint().len(), 8);
        assert_eq!(key.fingerprint(), fingerprint(" sk-or-v1-secretsecret "));
    }

    #[test]
    fn once_reads_its_own_variables_and_flags_but_never_ambient_ones() {
        let env = |name: &str| match name {
            "OPENROUTER_API_KEY" | "AI_GATEWAY_API_KEY" | "TYPESAFE_API_KEY" => {
                Some("ambient".to_owned())
            }
            "OPENAGENTS_TYPESAFE_KEY" => Some("ts-once".to_owned()),
            _ => None,
        };
        let mut args: Vec<String> = [
            "chat",
            "--openrouter-key",
            "or-flag",
            "--vercel-key=vk",
            "hi",
        ]
        .map(String::from)
        .to_vec();
        let keys = Keys::once(&env, &mut args).unwrap();
        assert_eq!(args, vec!["chat", "hi"]);
        assert_eq!(keys.get(Provider::OpenRouter).unwrap().expose(), "or-flag");
        assert_eq!(keys.get(Provider::Vercel).unwrap().expose(), "vk");
        assert_eq!(keys.get(Provider::TypeSafe).unwrap().expose(), "ts-once");

        let mut args = vec!["chat".to_owned()];
        let ambient_only = |name: &str| match name {
            "OPENROUTER_API_KEY" | "AI_GATEWAY_API_KEY" | "TYPESAFE_API_KEY" => {
                Some("ambient".to_owned())
            }
            _ => None,
        };
        assert!(Keys::once(&ambient_only, &mut args).unwrap().is_empty());
        let mut args = vec!["--typesafe-key".to_owned()];
        assert!(Keys::once(&|_| None, &mut args).is_err());
    }

    #[test]
    fn each_failure_has_its_line() {
        let p = Provider::OpenRouter;
        assert_eq!(
            Failure::of_status(p, 401).unwrap().line(),
            "Your OpenRouter key was refused. Update it in Settings."
        );
        assert_eq!(
            Failure::of_status(Provider::Vercel, 402).unwrap().line(),
            "Your Vercel AI Gateway account is out of credits. Add credits there, or switch to OpenAgents in Settings."
        );
        assert_eq!(
            Failure::of_status(Provider::TypeSafe, 429).unwrap().line(),
            "TypeSafe is rate-limiting your key; try again shortly."
        );
        assert_eq!(
            Failure::ModelUnavailable("openai/gpt-6.1-sol".into()).line(),
            "Your keys can't use openai/gpt-6.1-sol."
        );
        assert_eq!(
            Failure::NoConnection(p).line(),
            "Couldn't reach OpenRouter; try again."
        );
        assert!(Failure::of_status(p, 400).is_none());
        assert!(!Failure::ModelUnavailable("m".into()).fails_over());
    }

    #[test]
    fn status_lines() {
        let mut keys = Keys::none();
        assert_eq!(
            status_line(Mode::Ours, &keys, None),
            "Running on OpenAgents."
        );
        keys.insert(Provider::OpenRouter, ApiKey::new("k"));
        assert_eq!(
            status_line(Mode::Mine, &keys, None),
            "Running on your keys."
        );
        assert_eq!(
            status_line(
                Mode::Mine,
                &keys,
                Some(&Failure::Refused(Provider::OpenRouter))
            ),
            "Your OpenRouter key was refused. Update it in Settings. Nothing is running on ours."
        );
    }

    #[test]
    fn modes_and_payers_read_and_write() {
        assert_eq!(Mode::parse("mine").unwrap(), Mode::Mine);
        assert!(Mode::parse("mine_then_ours").is_err());
        let payer = Payer::Theirs {
            provider: Provider::Vercel,
            fingerprint: "abcd1234".into(),
        };
        let json = serde_json::to_value(&payer).unwrap();
        assert_eq!(json["payer"], "theirs");
        assert_eq!(json["provider"], "vercel");
        assert_eq!(Provider::parse("TypeSafe").unwrap(), Provider::TypeSafe);
    }
}
