//! The decision profile: the validated answer to "where do decisions go."
//!
//! A turn's questions have to land somewhere, and where is configuration:
//! a flag, an environment variable, or the default, in that order. This
//! module resolves that configuration into one [`Profile`], checked before
//! anything builds a client from it. Malformed configuration is a
//! [`Refusal`] naming the variable and what was wrong, never a silent
//! default laid over a typo.
//!
//! - [`Profile::HostedHttp`] is the hosted System One door: HTTP
//!   transport, a bearer credential, a named model.
//! - [`Profile::DirectLocal`] is a model endpoint on this machine or
//!   network: HTTP transport, a named model, and no credential — a local
//!   endpoint never invents one.
//! - [`Profile::OwnProvider`] is a door the caller runs: HTTP transport, a
//!   named model, and the credential only when the door takes one.
//! - [`Profile::Relay`] is a NIP-CJ decision job over a Nostr relay: a
//!   WebSocket URL and the worker's public key.
//!
//! `local` is a fact the resolver checks, not a claim the caller makes: a
//! loopback or private endpoint marks the profile `local: true`, and a
//! review or fallback policy can ask [`Profile::is_local`]. This module
//! resolves and validates only — no socket opens, no request flies.

use std::collections::BTreeMap;
use std::env;
use std::fmt;
use std::net::IpAddr;

use jev::ApiKey;
use secp256k1::XOnlyPublicKey;

/// The variable that names which profile a run resolves.
pub const PROFILE_VAR: &str = "CODER_DECISION_PROFILE";
/// The variable that names the endpoint an HTTP profile calls.
pub const URL_VAR: &str = "CODER_DECISION_URL";
/// The variable that names the model an HTTP profile asks.
pub const MODEL_VAR: &str = "CODER_DECISION_MODEL";
/// The variable that holds the bearer credential a keyed door sends.
pub const KEY_VAR: &str = "CODER_DECISION_KEY";
/// The variable that names the relay a relay profile publishes to.
pub const RELAY_VAR: &str = "CODER_DECISION_RELAY";
/// The variable that names the worker a relay profile asks.
pub const WORKER_VAR: &str = "CODER_DECISION_WORKER";

// Shared variables a profile falls back on: the caller's own decision
// credential and door, the TypeSafe SDK's, and the relay door's. They are
// read where they apply and never refused where they do not — another tool
// on the machine may own them.
const OPENAGENTS_KEY_VAR: &str = "OPENAGENTS_API_KEY";
const OPENAGENTS_URL_VAR: &str = "OPENAGENTS_BASE_URL";
const TYPESAFE_KEY_VAR: &str = "TYPESAFE_API_KEY";
const TYPESAFE_URL_VAR: &str = "TYPESAFE_BASE_URL";
const TYPESAFE_MODEL_VAR: &str = "TYPESAFE_DEFAULT_MODEL";
const SHARED_RELAY_VAR: &str = "CODER_RELAY";
const SHARED_WORKER_VAR: &str = "CODER_WORKER";

/// The `CODER_DECISION_*` variables a stray-value scan walks. A set
/// variable the resolved profile does not take is a mistake to name, not
/// to ignore.
const DECISION_VARS: &[&str] = &[URL_VAR, MODEL_VAR, KEY_VAR, RELAY_VAR, WORKER_VAR];

/// Every variable resolution reads, so [`Profiles::resolve_env`] can scan
/// them all for the failure a reader cannot express: bytes that are not
/// Unicode.
const ENV_VARS: &[&str] = &[
    PROFILE_VAR,
    URL_VAR,
    MODEL_VAR,
    KEY_VAR,
    RELAY_VAR,
    WORKER_VAR,
    OPENAGENTS_KEY_VAR,
    OPENAGENTS_URL_VAR,
    TYPESAFE_KEY_VAR,
    TYPESAFE_URL_VAR,
    TYPESAFE_MODEL_VAR,
    SHARED_RELAY_VAR,
    SHARED_WORKER_VAR,
];

/// Where a resolved setting came from. A flag beats the environment, and
/// the environment beats the default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// Supplied as a flag.
    Flag,
    /// Read from the named environment variable.
    Env(&'static str),
    /// The module's own default.
    Default,
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Flag => f.write_str("flag"),
            Self::Env(name) => f.write_str(name),
            Self::Default => f.write_str("default"),
        }
    }
}

/// A resolved setting and the source that won it, kept so an operator can
/// ask not only what a profile is but why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sourced<T> {
    /// The resolved value.
    pub value: T,
    /// Where the value came from.
    pub source: Source,
}

/// A validated decision profile: which transport, which destination, and
/// what each setting's source was. Each variant carries only the fields
/// its transport needs — a local endpoint holds no credential field at
/// all, because it never invents one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Profile {
    /// The hosted System One door: a base URL, a model, and the bearer
    /// credential the door requires.
    HostedHttp {
        /// The door's base URL.
        url: Sourced<String>,
        /// The model to ask.
        model: Sourced<String>,
        /// The bearer credential, redacted in every rendering.
        key: Sourced<ApiKey>,
        /// Where the profile choice itself came from.
        picked: Source,
    },
    /// A model endpoint on this machine or network: a base URL and a
    /// model, validated loopback or private at resolve time.
    DirectLocal {
        /// The endpoint's URL, already proven local.
        url: Sourced<String>,
        /// The model to ask.
        model: Sourced<String>,
        /// Where the profile choice itself came from.
        picked: Source,
    },
    /// A door the caller runs: a base URL, a model, and the credential
    /// only when the door takes one.
    OwnProvider {
        /// The door's base URL.
        url: Sourced<String>,
        /// The model to ask.
        model: Sourced<String>,
        /// The bearer credential, when the door takes one.
        key: Option<Sourced<ApiKey>>,
        /// Where the profile choice itself came from.
        picked: Source,
    },
    /// A NIP-CJ decision job over a Nostr relay: the relay's WebSocket URL
    /// and the worker job requests are addressed to. The `npub` is the
    /// account, so there is no bearer credential to hold.
    Relay {
        /// The relay's WebSocket URL.
        relay: Sourced<String>,
        /// The worker's public key, validated at resolve time.
        worker: Sourced<XOnlyPublicKey>,
        /// Where the profile choice itself came from.
        picked: Source,
    },
}

impl Profile {
    /// The name [`PROFILE_VAR`] takes for this profile.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::HostedHttp { .. } => "hosted_http",
            Self::DirectLocal { .. } => "direct_local",
            Self::OwnProvider { .. } => "own_provider",
            Self::Relay { .. } => "relay",
        }
    }

    /// The transport decisions travel over.
    #[must_use]
    pub fn transport(&self) -> &'static str {
        match self {
            Self::HostedHttp { .. } | Self::DirectLocal { .. } | Self::OwnProvider { .. } => "http",
            Self::Relay { .. } => "nostr relay",
        }
    }

    /// Where the profile choice itself came from.
    #[must_use]
    pub fn picked(&self) -> Source {
        match self {
            Self::HostedHttp { picked, .. }
            | Self::DirectLocal { picked, .. }
            | Self::OwnProvider { picked, .. }
            | Self::Relay { picked, .. } => *picked,
        }
    }

    /// Whether the resolved door keeps the turn's questions on this machine
    /// or network: `true` for a validated local endpoint and for the
    /// caller's own door when it names a loopback or private address,
    /// `false` for a hosted door and for a relay, which carries the job to
    /// a worker wherever that worker is. Review and fallback policy read
    /// this; it is a fact the resolver checked, not a claim.
    #[must_use]
    pub fn is_local(&self) -> bool {
        match self {
            Self::DirectLocal { .. } => true,
            Self::OwnProvider { url, .. } => local_address(&url.value),
            Self::HostedHttp { .. } | Self::Relay { .. } => false,
        }
    }

    /// The operator-facing description of what resolved: the transport,
    /// the destination, and which source each setting came from. A
    /// credential renders as `***`; no line ever carries its bytes.
    #[must_use]
    pub fn diagnostics(&self) -> Vec<String> {
        let mut lines = vec![
            format!("profile: {} (from {})", self.name(), self.picked()),
            format!("transport: {}", self.transport()),
        ];
        match self {
            Self::HostedHttp {
                url, model, key, ..
            } => {
                lines.push(format!("destination: {} (from {})", url.value, url.source));
                lines.push(format!("model: {} (from {})", model.value, model.source));
                lines.push(format!("credential: {} (from {})", key.value, key.source));
            }
            Self::DirectLocal { url, model, .. } => {
                lines.push(format!("destination: {} (from {})", url.value, url.source));
                lines.push(format!("model: {} (from {})", model.value, model.source));
                lines.push("credential: none (a local endpoint takes none)".to_string());
            }
            Self::OwnProvider {
                url, model, key, ..
            } => {
                lines.push(format!("destination: {} (from {})", url.value, url.source));
                lines.push(format!("model: {} (from {})", model.value, model.source));
                match key {
                    Some(key) => {
                        lines.push(format!("credential: {} (from {})", key.value, key.source));
                    }
                    None => lines.push("credential: none".to_string()),
                }
            }
            Self::Relay { relay, worker, .. } => {
                lines.push(format!(
                    "destination: {} (from {})",
                    relay.value, relay.source
                ));
                lines.push(format!("worker: {} (from {})", worker.value, worker.source));
            }
        }
        lines.push(format!("local: {}", self.is_local()));
        lines
    }

    /// The client this profile's calls travel over.
    ///
    /// A profile names where a judgment goes; the judgment itself does
    /// not change with the door, so every call site that asks builds
    /// through this one path rather than carrying a resolver of its
    /// own. Building the client is still not a call: no socket opens,
    /// no request flies.
    ///
    /// A local endpoint never invents a credential: `direct_local` and a
    /// keyless `own_provider` build on the SDK's credential-free
    /// transport, which takes a loopback IP literal and nothing wider.
    /// A keyless door beyond loopback is refused rather than lent the
    /// hosted credential another variable on this machine may hold.
    ///
    /// # Errors
    ///
    /// Returns [`Refusal::Unsupported`] when the profile names a door no
    /// System One client carries — a relay's decisions travel the relay
    /// itself — or one that cannot be built without inventing a
    /// credential. A resolved value the SDK cannot build from is
    /// [`Refusal::Malformed`] naming the setting that held it.
    pub fn client(&self) -> Result<jev::Client, Refusal> {
        let (url, config) = match self {
            Self::HostedHttp { url, model, key, .. } => (
                url,
                jev::Config::new()
                    .base_url(url.value.as_str())
                    .default_model(model.value.as_str())
                    .api_key(key.value.clone()),
            ),
            Self::DirectLocal { url, model, .. } => {
                if !loopback_ip(&url.value) {
                    return Err(Refusal::Unsupported {
                        profile: "direct_local",
                        reason: "the credential-free transport takes a loopback IP \
                                 literal, and this endpoint names another local address"
                            .to_string(),
                    });
                }
                (
                    url,
                    jev::Config::local(url.value.as_str(), model.value.as_str()),
                )
            }
            Self::OwnProvider { url, model, key, .. } => (
                url,
                match key {
                    Some(key) => jev::Config::new()
                        .base_url(url.value.as_str())
                        .default_model(model.value.as_str())
                        .api_key(key.value.clone()),
                    None if loopback_ip(&url.value) => {
                        jev::Config::local(url.value.as_str(), model.value.as_str())
                    }
                    None => {
                        return Err(Refusal::Unsupported {
                            profile: "own_provider",
                            reason: "names no credential, and a credential-free door \
                                     builds only on a loopback address"
                                .to_string(),
                        });
                    }
                },
            ),
            Self::Relay { .. } => {
                return Err(Refusal::Unsupported {
                    profile: "relay",
                    reason: "its decisions travel the relay, not the System One \
                             HTTP door"
                        .to_string(),
                });
            }
        };
        jev::Client::new(config).map_err(|error| Refusal::Malformed {
            variable: match url.source {
                Source::Env(name) => name,
                Source::Flag | Source::Default => URL_VAR,
            },
            reason: error.to_string(),
        })
    }
}

/// Flag-supplied settings, the strongest source resolution consults.
///
/// A flag holds no credential field: credentials come from the
/// environment or a config file, never a command line.
#[derive(Clone, Debug, Default)]
pub struct Flags {
    /// The profile to run, spelled the way [`PROFILE_VAR`] spells it.
    pub profile: Option<String>,
    /// The endpoint an HTTP profile calls.
    pub url: Option<String>,
    /// The model an HTTP profile asks.
    pub model: Option<String>,
    /// The relay a relay profile publishes to.
    pub relay: Option<String>,
    /// The worker a relay profile asks.
    pub worker: Option<String>,
}

/// The profile resolver: flag-supplied settings over the environment over
/// the defaults.
#[derive(Clone, Debug, Default)]
pub struct Profiles {
    flags: Flags,
}

impl Profiles {
    /// A resolver with no flag overrides.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A resolver whose flag-supplied settings win over the environment.
    #[must_use]
    pub fn with_flags(flags: Flags) -> Self {
        Self { flags }
    }

    /// Build the active profile from `read`, which answers an environment
    /// variable's value or `None`. The reader is a parameter so a test
    /// decides what the environment says; [`Profiles::resolve_env`] reads
    /// the process's own.
    ///
    /// # Errors
    ///
    /// Returns a [`Refusal`] naming the variable or flag at fault: a
    /// required setting no source supplied, a value the profile cannot
    /// use, a profile name that does not exist, or a setting the resolved
    /// profile does not take.
    pub fn resolve(&self, read: impl Fn(&str) -> Option<String>) -> Result<Profile, Refusal> {
        // An unset variable and a blank one read the same, the way the SDK
        // and the doors already read them.
        let read = |name: &str| {
            read(name)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        };
        // The first non-blank value across flag, then each env fallback:
        // (value, source, name-for-refusals).
        let take = |flag: Option<(&'static str, &String)>, envs: &[&'static str]| {
            flag.and_then(|(flag_name, value)| {
                let value = value.trim();
                (!value.is_empty()).then(|| (value.to_string(), Source::Flag, flag_name))
            })
            .or_else(|| {
                envs.iter()
                    .find_map(|name| read(name).map(|value| (value, Source::Env(name), *name)))
            })
        };
        // A `CODER_DECISION_*` variable — or the flag that shadows one —
        // the resolved profile does not take is a mistake to name.
        let stray = |consumed: &[&'static str]| -> Option<&'static str> {
            for var in DECISION_VARS {
                if !consumed.contains(var) && read(var).is_some() {
                    return Some(var);
                }
            }
            let flags: [(&'static str, &'static str, &Option<String>); 4] = [
                ("--url", URL_VAR, &self.flags.url),
                ("--model", MODEL_VAR, &self.flags.model),
                ("--relay", RELAY_VAR, &self.flags.relay),
                ("--worker", WORKER_VAR, &self.flags.worker),
            ];
            for (flag_name, var, value) in flags {
                if !consumed.contains(&var)
                    && value
                        .as_deref()
                        .is_some_and(|value| !value.trim().is_empty())
                {
                    return Some(flag_name);
                }
            }
            None
        };

        let (name, picked, name_var) = match take(
            self.flags.profile.as_ref().map(|v| ("--profile", v)),
            &[PROFILE_VAR],
        ) {
            Some((name, source, var)) => (name, source, var),
            None => ("hosted_http".to_string(), Source::Default, PROFILE_VAR),
        };

        let profile = match name.as_str() {
            "hosted_http" => {
                let url = match take(
                    self.flags.url.as_ref().map(|v| ("--url", v)),
                    &[URL_VAR, OPENAGENTS_URL_VAR, TYPESAFE_URL_VAR],
                ) {
                    Some((text, source, var)) => Sourced {
                        value: http_url(&text, var)?,
                        source,
                    },
                    None => Sourced {
                        value: jev::defaults::BASE_URL.to_string(),
                        source: Source::Default,
                    },
                };
                let model = match take(
                    self.flags.model.as_ref().map(|v| ("--model", v)),
                    &[MODEL_VAR, TYPESAFE_MODEL_VAR],
                ) {
                    Some((text, source, _)) => Sourced {
                        value: text,
                        source,
                    },
                    None => Sourced {
                        value: jev::defaults::MODEL.to_string(),
                        source: Source::Default,
                    },
                };
                let key = match take(None, &[KEY_VAR, OPENAGENTS_KEY_VAR, TYPESAFE_KEY_VAR]) {
                    Some((text, source, _)) => Sourced {
                        value: ApiKey::new(text),
                        source,
                    },
                    None => {
                        return Err(Refusal::Missing {
                            variable: KEY_VAR,
                            hint: "a hosted door needs a bearer credential; set \
                                   CODER_DECISION_KEY, OPENAGENTS_API_KEY, or TYPESAFE_API_KEY",
                        });
                    }
                };
                if let Some(bad) = stray(&[URL_VAR, MODEL_VAR, KEY_VAR]) {
                    return Err(Refusal::Unexpected {
                        variable: bad,
                        profile: "hosted_http",
                    });
                }
                Profile::HostedHttp {
                    url,
                    model,
                    key,
                    picked,
                }
            }
            "direct_local" => {
                let url = match take(self.flags.url.as_ref().map(|v| ("--url", v)), &[URL_VAR]) {
                    Some((text, source, var)) => {
                        let url = http_url(&text, var)?;
                        if !local_address(&url) {
                            return Err(Refusal::Malformed {
                                variable: var,
                                reason: "names a public address; direct_local serves only \
                                         loopback and private endpoints"
                                    .to_string(),
                            });
                        }
                        Sourced { value: url, source }
                    }
                    None => {
                        return Err(Refusal::Missing {
                            variable: URL_VAR,
                            hint: "a local endpoint has no default; set CODER_DECISION_URL \
                                   or pass --url",
                        });
                    }
                };
                let model = match take(
                    self.flags.model.as_ref().map(|v| ("--model", v)),
                    &[MODEL_VAR],
                ) {
                    Some((text, source, _)) => Sourced {
                        value: text,
                        source,
                    },
                    None => {
                        return Err(Refusal::Missing {
                            variable: MODEL_VAR,
                            hint: "a local endpoint still serves a named model; set \
                                   CODER_DECISION_MODEL or pass --model",
                        });
                    }
                };
                if let Some(bad) = stray(&[URL_VAR, MODEL_VAR]) {
                    return Err(Refusal::Unexpected {
                        variable: bad,
                        profile: "direct_local",
                    });
                }
                Profile::DirectLocal { url, model, picked }
            }
            "own_provider" => {
                let url = match take(self.flags.url.as_ref().map(|v| ("--url", v)), &[URL_VAR]) {
                    Some((text, source, var)) => Sourced {
                        value: http_url(&text, var)?,
                        source,
                    },
                    None => {
                        return Err(Refusal::Missing {
                            variable: URL_VAR,
                            hint: "the caller's door has no default; set CODER_DECISION_URL \
                                   or pass --url",
                        });
                    }
                };
                let model = match take(
                    self.flags.model.as_ref().map(|v| ("--model", v)),
                    &[MODEL_VAR],
                ) {
                    Some((text, source, _)) => Sourced {
                        value: text,
                        source,
                    },
                    None => {
                        return Err(Refusal::Missing {
                            variable: MODEL_VAR,
                            hint: "the caller's door still serves a named model; set \
                                   CODER_DECISION_MODEL or pass --model",
                        });
                    }
                };
                let key =
                    take(None, &[KEY_VAR, OPENAGENTS_KEY_VAR]).map(|(text, source, _)| Sourced {
                        value: ApiKey::new(text),
                        source,
                    });
                if let Some(bad) = stray(&[URL_VAR, MODEL_VAR, KEY_VAR]) {
                    return Err(Refusal::Unexpected {
                        variable: bad,
                        profile: "own_provider",
                    });
                }
                Profile::OwnProvider {
                    url,
                    model,
                    key,
                    picked,
                }
            }
            "relay" => {
                let relay = match take(
                    self.flags.relay.as_ref().map(|v| ("--relay", v)),
                    &[RELAY_VAR, SHARED_RELAY_VAR],
                ) {
                    Some((text, source, var)) => Sourced {
                        value: relay_url(&text, var)?,
                        source,
                    },
                    None => Sourced {
                        value: crate::relay::DEFAULT_RELAY_URL.to_string(),
                        source: Source::Default,
                    },
                };
                let worker = match take(
                    self.flags.worker.as_ref().map(|v| ("--worker", v)),
                    &[WORKER_VAR, SHARED_WORKER_VAR],
                ) {
                    Some((text, source, var)) => {
                        let worker = crate::relay::parse_pubkey(&text).ok_or_else(|| {
                            Refusal::Malformed {
                                variable: var,
                                reason: "must be an npub or 64 lowercase hex".to_string(),
                            }
                        })?;
                        Sourced {
                            value: worker,
                            source,
                        }
                    }
                    None => {
                        return Err(Refusal::Missing {
                            variable: WORKER_VAR,
                            hint: "a relay profile asks a worker; set CODER_DECISION_WORKER \
                                   or pass --worker",
                        });
                    }
                };
                if let Some(bad) = stray(&[RELAY_VAR, WORKER_VAR]) {
                    return Err(Refusal::Unexpected {
                        variable: bad,
                        profile: "relay",
                    });
                }
                Profile::Relay {
                    relay,
                    worker,
                    picked,
                }
            }
            other => {
                return Err(Refusal::UnknownProfile {
                    variable: name_var,
                    value: other.to_string(),
                });
            }
        };
        Ok(profile)
    }

    /// [`Profiles::resolve`] against the process environment. A variable
    /// holding bytes that are not Unicode is malformed configuration, not
    /// an unset one.
    ///
    /// # Errors
    ///
    /// Returns the same [`Refusal`]s as [`Profiles::resolve`], plus
    /// [`Refusal::Malformed`] when a variable is not Unicode.
    pub fn resolve_env(&self) -> Result<Profile, Refusal> {
        let mut values = BTreeMap::new();
        for &name in ENV_VARS {
            match env::var(name) {
                Ok(value) => {
                    values.insert(name, value);
                }
                Err(env::VarError::NotPresent) => {}
                Err(env::VarError::NotUnicode(_)) => {
                    return Err(Refusal::Malformed {
                        variable: name,
                        reason: "must be valid Unicode".to_string(),
                    });
                }
            }
        }
        self.resolve(|name| values.get(name).cloned())
    }
}

/// Why a profile did not resolve. Every variant names the variable or
/// flag that caused it; no variant carries a value that could be a
/// secret.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The profile needs a value no source supplied.
    Missing {
        /// The canonical variable for the missing setting.
        variable: &'static str,
        /// What to set and where else the setting can come from.
        hint: &'static str,
    },
    /// A supplied value is not one the profile can use; `reason` says
    /// what was wrong with it, never what it was.
    Malformed {
        /// The variable or flag that held the value.
        variable: &'static str,
        /// What was wrong with it.
        reason: String,
    },
    /// The profile name is not one this module knows.
    UnknownProfile {
        /// Where the name was read from.
        variable: &'static str,
        /// The unrecognized name.
        value: String,
    },
    /// The resolved profile takes no such setting, so a set one is a
    /// mistake to name rather than to ignore.
    Unexpected {
        /// The variable or flag that was set.
        variable: &'static str,
        /// The profile that refuses it.
        profile: &'static str,
    },
    /// The profile resolved, but no System One client carries its
    /// calls: a relay's decisions travel the relay itself, and a
    /// credential-free door beyond a loopback address has no boundary
    /// to build under.
    Unsupported {
        /// The profile that cannot build the client.
        profile: &'static str,
        /// Why no client can carry it.
        reason: String,
    },
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { variable, hint } => write!(f, "{variable} is required: {hint}"),
            Self::Malformed { variable, reason } => write!(f, "{variable} {reason}"),
            Self::UnknownProfile { variable, value } => write!(
                f,
                "{variable} must be hosted_http, direct_local, own_provider, or relay; \
                 got {value:?}"
            ),
            Self::Unexpected { variable, profile } => {
                write!(f, "the {profile} profile does not take {variable}")
            }
            Self::Unsupported { profile, reason } => {
                write!(f, "the {profile} profile builds no System One client: {reason}")
            }
        }
    }
}

impl std::error::Error for Refusal {}

/// `text` as an HTTP door URL: the `http` or `https` scheme, a host, and
/// nothing a base URL should not carry.
fn http_url(text: &str, variable: &'static str) -> Result<String, Refusal> {
    door_url(text, variable, &["http", "https"], "http or https")
}

/// `text` as a relay URL: the `ws` or `wss` scheme, a host, and nothing a
/// base URL should not carry.
fn relay_url(text: &str, variable: &'static str) -> Result<String, Refusal> {
    door_url(text, variable, &["ws", "wss"], "ws or wss")
}

/// `text` as a door's base URL under `schemes`. Credentials, a query, and
/// a fragment are all refused: a credential does not travel inside a URL
/// here, and a base URL is a root, not a request.
fn door_url(
    text: &str,
    variable: &'static str,
    schemes: &[&str],
    named: &str,
) -> Result<String, Refusal> {
    let parsed = reqwest::Url::parse(text).map_err(|error| Refusal::Malformed {
        variable,
        reason: format!("is not a URL: {error}"),
    })?;
    if !schemes.contains(&parsed.scheme()) {
        return Err(Refusal::Malformed {
            variable,
            reason: format!(
                "uses the {} scheme; this door speaks {named}",
                parsed.scheme()
            ),
        });
    }
    if parsed.host_str().is_none() {
        return Err(Refusal::Malformed {
            variable,
            reason: "has no host".to_string(),
        });
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(Refusal::Malformed {
            variable,
            reason: "must not carry a user or password".to_string(),
        });
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err(Refusal::Malformed {
            variable,
            reason: "must not carry a query or fragment".to_string(),
        });
    }
    Ok(parsed.as_str().trim_end_matches('/').to_string())
}

/// Whether `url` names a loopback IP literal — `127.*` or `[::1]` — the
/// only destination the SDK's credential-free transport takes. A
/// hostname or a private address may be local, but it is not this.
fn loopback_ip(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };
    let Some(host) = parsed.host_str() else {
        return false;
    };
    host.parse::<IpAddr>()
        .ok()
        .or_else(|| {
            host.strip_prefix('[')
                .and_then(|inner| inner.strip_suffix(']'))
                .and_then(|inner| inner.parse::<IpAddr>().ok())
        })
        .is_some_and(|ip| ip.is_loopback())
}

/// Whether `url` names an endpoint on this machine or network: a loopback
/// or private IP, or `localhost`. A name this check cannot verify — a
/// public DNS name, a `.local` guess — is not local.
fn local_address(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };
    let Some(host) = parsed.host_str() else {
        return false;
    };
    let ip = host.parse::<IpAddr>().ok().or_else(|| {
        host.strip_prefix('[')
            .and_then(|inner| inner.strip_suffix(']'))
            .and_then(|inner| inner.parse::<IpAddr>().ok())
    });
    match ip {
        Some(IpAddr::V4(ip)) => ip.is_loopback() || ip.is_private() || ip.is_link_local(),
        Some(IpAddr::V6(ip)) => {
            ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local()
        }
        None => {
            let host = host.to_ascii_lowercase();
            host == "localhost" || host.ends_with(".localhost")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An env reader backed by a fixed map, so a test decides what the
    /// environment says.
    fn env<'a>(pairs: &'a [(&'static str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_string())
        }
    }

    /// A valid worker public key: the secp256k1 generator's x coordinate.
    const WORKER: &str = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";

    #[test]
    fn each_variant_resolves_from_a_supplied_env_map() {
        let hosted = Profiles::new()
            .resolve(env(&[
                (PROFILE_VAR, "hosted_http"),
                (KEY_VAR, "oak_test.secret"),
            ]))
            .unwrap();
        assert!(matches!(hosted, Profile::HostedHttp { .. }));
        assert_eq!(hosted.name(), "hosted_http");
        assert_eq!(hosted.transport(), "http");
        assert!(!hosted.is_local());

        let local = Profiles::new()
            .resolve(env(&[
                (PROFILE_VAR, "direct_local"),
                (URL_VAR, "http://127.0.0.1:8080"),
                (MODEL_VAR, "local-kev"),
            ]))
            .unwrap();
        assert!(matches!(local, Profile::DirectLocal { .. }));
        assert!(local.is_local());

        let own = Profiles::new()
            .resolve(env(&[
                (PROFILE_VAR, "own_provider"),
                (URL_VAR, "https://decisions.example.com"),
                (MODEL_VAR, "shared-kev"),
                (KEY_VAR, "oak_test.secret"),
            ]))
            .unwrap();
        match &own {
            Profile::OwnProvider { key, .. } => {
                assert_eq!(key.as_ref().unwrap().value.expose(), "oak_test.secret");
            }
            other => panic!("expected OwnProvider, got {other:?}"),
        }
        assert!(!own.is_local());

        let relay = Profiles::new()
            .resolve(env(&[(PROFILE_VAR, "relay"), (WORKER_VAR, WORKER)]))
            .unwrap();
        match &relay {
            Profile::Relay {
                relay: url, worker, ..
            } => {
                assert_eq!(url.value, crate::relay::DEFAULT_RELAY_URL);
                assert_eq!(url.source, Source::Default);
                assert_eq!(worker.value.to_string(), WORKER);
                assert_eq!(worker.source, Source::Env(WORKER_VAR));
            }
            other => panic!("expected Relay, got {other:?}"),
        }
        assert_eq!(relay.transport(), "nostr relay");
        assert!(!relay.is_local());
    }

    #[test]
    fn a_missing_required_setting_refuses_with_the_variable_named() {
        for (pairs, variable) in [
            (&[(PROFILE_VAR, "direct_local")][..], URL_VAR),
            (
                &[
                    (PROFILE_VAR, "direct_local"),
                    (URL_VAR, "http://127.0.0.1:1"),
                ][..],
                MODEL_VAR,
            ),
            (&[(PROFILE_VAR, "hosted_http")][..], KEY_VAR),
            (&[(PROFILE_VAR, "own_provider")][..], URL_VAR),
            (&[(PROFILE_VAR, "relay")][..], WORKER_VAR),
        ] {
            match Profiles::new().resolve(env(pairs)).unwrap_err() {
                Refusal::Missing { variable: got, .. } => assert_eq!(got, variable),
                other => panic!("expected Missing({variable}), got {other:?}"),
            }
        }
    }

    #[test]
    fn an_unknown_profile_name_refuses() {
        match Profiles::new()
            .resolve(env(&[(PROFILE_VAR, "hosted-ish")]))
            .unwrap_err()
        {
            Refusal::UnknownProfile { variable, value } => {
                assert_eq!(variable, PROFILE_VAR);
                assert_eq!(value, "hosted-ish");
            }
            other => panic!("expected UnknownProfile, got {other:?}"),
        }
    }

    #[test]
    fn precedence_is_flag_then_env_then_default() {
        // A flag beats the environment.
        let flags = Flags {
            url: Some("https://flag.example.com".to_string()),
            ..Flags::default()
        };
        let profile = Profiles::with_flags(flags)
            .resolve(env(&[
                (PROFILE_VAR, "hosted_http"),
                (URL_VAR, "https://env.example.com"),
                (KEY_VAR, "oak_test.secret"),
            ]))
            .unwrap();
        match &profile {
            Profile::HostedHttp { url, .. } => {
                assert_eq!(url.value, "https://flag.example.com");
                assert_eq!(url.source, Source::Flag);
            }
            other => panic!("expected HostedHttp, got {other:?}"),
        }

        // The environment beats the default.
        let profile = Profiles::new()
            .resolve(env(&[
                (PROFILE_VAR, "hosted_http"),
                (URL_VAR, "https://env.example.com"),
                (KEY_VAR, "oak_test.secret"),
            ]))
            .unwrap();
        match &profile {
            Profile::HostedHttp { url, .. } => {
                assert_eq!(url.value, "https://env.example.com");
                assert_eq!(url.source, Source::Env(URL_VAR));
            }
            other => panic!("expected HostedHttp, got {other:?}"),
        }

        // Neither leaves the default.
        let profile = Profiles::new()
            .resolve(env(&[
                (PROFILE_VAR, "hosted_http"),
                (KEY_VAR, "oak_test.secret"),
            ]))
            .unwrap();
        match &profile {
            Profile::HostedHttp { url, .. } => {
                assert_eq!(url.value, jev::defaults::BASE_URL);
                assert_eq!(url.source, Source::Default);
            }
            other => panic!("expected HostedHttp, got {other:?}"),
        }

        // And a flag profile beats an env profile.
        let flags = Flags {
            profile: Some("direct_local".to_string()),
            url: Some("http://127.0.0.1:9".to_string()),
            model: Some("local-kev".to_string()),
            ..Flags::default()
        };
        let profile = Profiles::with_flags(flags)
            .resolve(env(&[(PROFILE_VAR, "hosted_http")]))
            .unwrap();
        assert_eq!(profile.name(), "direct_local");
        assert_eq!(profile.picked(), Source::Flag);
    }

    #[test]
    fn a_local_endpoint_needs_no_credential_and_reports_local() {
        for url in [
            "http://127.0.0.1:8080",
            "http://[::1]:9000",
            "http://localhost:11434",
            "http://192.168.1.20:8080",
            "http://10.0.0.4",
        ] {
            let profile = Profiles::new()
                .resolve(env(&[
                    (PROFILE_VAR, "direct_local"),
                    (URL_VAR, url),
                    (MODEL_VAR, "local-kev"),
                ]))
                .unwrap();
            assert!(profile.is_local(), "{url}");
        }
    }

    #[test]
    fn local_is_checked_not_claimed() {
        // direct_local refuses a public address outright.
        match Profiles::new()
            .resolve(env(&[
                (PROFILE_VAR, "direct_local"),
                (URL_VAR, "https://example.invalid"),
                (MODEL_VAR, "local-kev"),
            ]))
            .unwrap_err()
        {
            Refusal::Malformed { variable, .. } => assert_eq!(variable, URL_VAR),
            other => panic!("expected Malformed, got {other:?}"),
        }

        // own_provider marks the same check on the caller's door.
        for (url, local) in [
            ("http://192.168.1.20:8080", true),
            ("http://10.0.0.4", true),
            ("https://decisions.example.com", false),
        ] {
            let profile = Profiles::new()
                .resolve(env(&[
                    (PROFILE_VAR, "own_provider"),
                    (URL_VAR, url),
                    (MODEL_VAR, "shared-kev"),
                ]))
                .unwrap();
            assert_eq!(profile.is_local(), local, "{url}");
        }
    }

    #[test]
    fn a_setting_the_profile_does_not_take_refuses() {
        match Profiles::new()
            .resolve(env(&[
                (PROFILE_VAR, "direct_local"),
                (URL_VAR, "http://127.0.0.1:1"),
                (MODEL_VAR, "local-kev"),
                (KEY_VAR, "oak_test.secret"),
            ]))
            .unwrap_err()
        {
            Refusal::Unexpected { variable, .. } => assert_eq!(variable, KEY_VAR),
            other => panic!("expected Unexpected, got {other:?}"),
        }
    }

    #[test]
    fn malformed_values_name_their_variable() {
        match Profiles::new()
            .resolve(env(&[
                (PROFILE_VAR, "own_provider"),
                (URL_VAR, "ftp://files.example.com"),
                (MODEL_VAR, "shared-kev"),
            ]))
            .unwrap_err()
        {
            Refusal::Malformed { variable, reason } => {
                assert_eq!(variable, URL_VAR);
                assert!(reason.contains("scheme"), "{reason}");
            }
            other => panic!("expected Malformed, got {other:?}"),
        }

        match Profiles::new()
            .resolve(env(&[(PROFILE_VAR, "relay"), (WORKER_VAR, "not-a-key")]))
            .unwrap_err()
        {
            Refusal::Malformed { variable, .. } => assert_eq!(variable, WORKER_VAR),
            other => panic!("expected Malformed, got {other:?}"),
        }

        // A malformed value in a fallback variable is named too, not
        // skipped for the next source.
        match Profiles::new()
            .resolve(env(&[
                (PROFILE_VAR, "hosted_http"),
                (TYPESAFE_URL_VAR, "not a url"),
                (KEY_VAR, "oak_test.secret"),
            ]))
            .unwrap_err()
        {
            Refusal::Malformed { variable, .. } => assert_eq!(variable, TYPESAFE_URL_VAR),
            other => panic!("expected Malformed, got {other:?}"),
        }
    }

    #[test]
    fn shared_variables_feed_the_profile_that_reads_them() {
        // The default profile is hosted_http, and the SDK's own variables
        // supply it.
        let profile = Profiles::new()
            .resolve(env(&[(TYPESAFE_KEY_VAR, "ts-secret")]))
            .unwrap();
        match &profile {
            Profile::HostedHttp { key, picked, .. } => {
                assert_eq!(key.value.expose(), "ts-secret");
                assert_eq!(key.source, Source::Env(TYPESAFE_KEY_VAR));
                assert_eq!(*picked, Source::Default);
            }
            other => panic!("expected HostedHttp, got {other:?}"),
        }

        // The relay profile falls back to the relay door's own variables.
        let profile = Profiles::new()
            .resolve(env(&[(PROFILE_VAR, "relay"), (SHARED_WORKER_VAR, WORKER)]))
            .unwrap();
        match &profile {
            Profile::Relay { worker, .. } => {
                assert_eq!(worker.source, Source::Env(SHARED_WORKER_VAR));
            }
            other => panic!("expected Relay, got {other:?}"),
        }
    }

    #[test]
    fn diagnostics_describe_and_redact() {
        let secret = "oak_d34db33f.sup3rs3cr3t";
        let profile = Profiles::new()
            .resolve(env(&[(PROFILE_VAR, "hosted_http"), (KEY_VAR, secret)]))
            .unwrap();
        let joined = profile.diagnostics().join("\n");
        assert!(joined.contains("profile: hosted_http"), "{joined}");
        assert!(joined.contains("transport: http"), "{joined}");
        assert!(joined.contains("destination:"), "{joined}");
        assert!(joined.contains("credential: ***"), "{joined}");
        assert!(joined.contains("local: false"), "{joined}");
        assert!(!joined.contains(secret), "{joined}");
    }
}
