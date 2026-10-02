//! [`Access`]: which doors a call uses, and the fixed order among the
//! person's keys.
//!
//! - **Jev decisions:** TypeSafe, then the Vercel AI Gateway
//!   (`typesafe-ai/jev`), then OpenRouter (`typesafe/jev-1.13`).
//! - **Everything else** (chat, personalization, embeddings, Microcoder,
//!   judges): OpenRouter, then the Vercel AI Gateway.
//!
//! Under [`Mode::Mine`] the doors hold only the person's keys, so failing
//! over only ever means another key of theirs.

use crate::{ApiKey, Failure, KeyPrint, Keys, Mode, Paid, Payer, Provider};

/// The embedding model every provider serves with the same vectors.
pub const EMBEDDING_MODEL: &str = "openai/text-embedding-3-small";
/// The model Microcoder's cloud provider runs on the person's keys.
pub const MICROCODER_MODEL: &str = "openai/gpt-6.1-sol";
/// The model a plugin eval's `judge` grader asks.
pub const JUDGE_MODEL: &str = "google/gemini-3.8-flash";
/// The model that finishes a dispatch sentence.
pub const PERSONALIZE_MODEL: &str = "google/gemini-2.5-flash-lite";

/// What a call wants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Use<'a> {
    /// A chat reply: our current primary, and the fallback a turn takes
    /// when the primary misses.
    Chat { primary: &'a str, fallback: &'a str },
    /// One named model.
    Model(&'a str),
    /// Query and document embeddings ([`EMBEDDING_MODEL`]).
    Embeddings,
}

/// One door on the person's key: an OpenAI-compatible base URL, the model
/// to ask there, and the key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatDoor {
    pub provider: Provider,
    /// `https://openrouter.ai/api/v1` or `https://ai-gateway.vercel.sh/v1`.
    pub base_url: &'static str,
    pub model: String,
    pub key: ApiKey,
}

impl ChatDoor {
    /// The base an Open Responses client appends `/v1/responses` to:
    /// `https://openrouter.ai/api` or `https://ai-gateway.vercel.sh`.
    #[must_use]
    pub fn responses_base(&self) -> &'static str {
        self.base_url.strip_suffix("/v1").unwrap_or(self.base_url)
    }

    /// Who pays when this door answers.
    #[must_use]
    pub fn payer(&self) -> Payer {
        Payer::Theirs {
            provider: self.provider,
            fingerprint: self.key.fingerprint(),
        }
    }
}

/// The doors for a chat model or an embedding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Doors {
    /// Our doors: the call site keeps the door it has always used.
    Ours,
    /// The person's doors, asked in order; never empty.
    Theirs(Vec<ChatDoor>),
}

/// The doors for a Jev decision.
#[derive(Clone, Debug)]
pub enum Decisions {
    /// Our doors: the hosted decision service, or a local TypeSafe key as
    /// today (`jev_hosted::resolve`).
    Ours,
    /// The person's keys, asked in the fixed order: a client config whose
    /// exchange is a [`jev::doors::Failover`] over their doors only.
    Theirs {
        config: jev::Config,
        /// The providers asked, in order.
        order: Vec<Provider>,
        /// Who pays when the first answers.
        payer: Payer,
    },
}

/// Why a call under `mine` has no door: one plain line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoDoor(pub Failure);

impl std::fmt::Display for NoDoor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.line())
    }
}

impl std::error::Error for NoDoor {}

/// Who pays for this surface's calls: the mode and the person's keys.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Access {
    mode: Mode,
    keys: Keys,
}

/// Whether the gateway serves `model`. It does not serve OpenRouter's
/// stealth models (Space Bunny).
fn gateway_serves(model: &str) -> bool {
    !model.starts_with("stealth/") && !model.starts_with("openrouter/")
}

impl Access {
    /// Ours: today's behaviour.
    #[must_use]
    pub fn ours() -> Self {
        Self::default()
    }

    /// The access a surface builds at start: `once` keys (flags or the
    /// `OPENAGENTS_*_KEY` variables) mean `mine` for this invocation, over
    /// the stored keys; otherwise `mode` from the settings, with the
    /// stored keys.
    #[must_use]
    pub fn new(mode: Mode, stored: Keys, once: &Keys) -> Self {
        if once.is_empty() {
            Self { mode, keys: stored }
        } else {
            Self {
                mode: Mode::Mine,
                keys: stored.overlaid(once),
            }
        }
    }

    /// The person's keys, for exactly this call (an API caller's header,
    /// a job's payer envelope): always `mine`.
    #[must_use]
    pub fn theirs(keys: Keys) -> Self {
        Self {
            mode: Mode::Mine,
            keys,
        }
    }

    #[must_use]
    pub const fn mode(&self) -> Mode {
        self.mode
    }

    /// The keys this access holds (used only under `mine`).
    #[must_use]
    pub fn keys(&self) -> &Keys {
        &self.keys
    }

    /// Whether calls go on the person's keys.
    #[must_use]
    pub fn is_mine(&self) -> bool {
        self.mode == Mode::Mine
    }

    /// The doors for `want`, in the fixed order. Under `mine` this never
    /// returns one of ours: a call no key of theirs can make is
    /// [`NoDoor`].
    ///
    /// # Errors
    /// Under `mine`, no key of the person's serves `want`.
    pub fn chat(&self, want: Use<'_>) -> Result<Doors, NoDoor> {
        if self.mode == Mode::Ours {
            return Ok(Doors::Ours);
        }
        let order = [Provider::OpenRouter, Provider::Vercel];
        let mut doors: Vec<ChatDoor> = Vec::new();
        let mut push = |provider: Provider, model: &str| {
            let Some(key) = self.keys.get(provider) else {
                return;
            };
            let Some(base_url) = provider.openai_base() else {
                return;
            };
            if provider == Provider::Vercel && !gateway_serves(model) {
                return;
            }
            if doors
                .iter()
                .any(|door| door.provider == provider && door.model == model)
            {
                return;
            }
            doors.push(ChatDoor {
                provider,
                base_url,
                model: model.to_owned(),
                key: key.clone(),
            });
        };
        let named = match want {
            Use::Chat { primary, fallback } => {
                for provider in order {
                    push(provider, primary);
                }
                for provider in order {
                    push(provider, fallback);
                }
                primary
            }
            Use::Model(model) => {
                for provider in order {
                    push(provider, model);
                }
                model
            }
            Use::Embeddings => {
                for provider in order {
                    push(provider, EMBEDDING_MODEL);
                }
                EMBEDDING_MODEL
            }
        };
        if doors.is_empty() {
            return Err(NoDoor(Failure::ModelUnavailable(named.to_owned())));
        }
        Ok(Doors::Theirs(doors))
    }

    /// The doors for a Jev decision. Under `mine` the hosted decision
    /// service is skipped: the computer asks Jev directly on the person's
    /// keys, TypeSafe first, then the gateway, then OpenRouter.
    ///
    /// # Errors
    /// Under `mine`, the person holds no key that serves Jev.
    pub fn decisions(&self) -> Result<Decisions, NoDoor> {
        use jev::doors::{self, Door, Naming};
        if self.mode == Mode::Ours {
            return Ok(Decisions::Ours);
        }
        let mut list: Vec<(Provider, Door)> = Vec::new();
        for provider in [Provider::TypeSafe, Provider::Vercel, Provider::OpenRouter] {
            let Some(key) = self.keys.get(provider) else {
                continue;
            };
            let key = jev::ApiKey::new(key.expose());
            let door = match provider {
                Provider::TypeSafe => Door::new(
                    doors::TYPESAFE_DOOR,
                    doors::TYPESAFE_DOOR,
                    Naming::Canonical,
                    key,
                ),
                Provider::Vercel => Door::new(
                    doors::GATEWAY_DOOR,
                    doors::GATEWAY_URL,
                    Naming::Gateway,
                    key,
                ),
                Provider::OpenRouter => Door::new(
                    doors::OPENROUTER_DOOR,
                    doors::OPENROUTER_URL,
                    Naming::OpenRouter,
                    key,
                ),
            };
            list.push((provider, door));
        }
        if list.is_empty() {
            return Err(NoDoor(Failure::ModelUnavailable("Jev".into())));
        }
        let order: Vec<Provider> = list.iter().map(|(p, _)| *p).collect();
        let first = order[0];
        let mut doors = list.into_iter().map(|(_, door)| door);
        let primary = doors.next().expect("one door at least");
        let failover = doors::Failover::new(primary, doors.collect());
        let config = jev::Config::new()
            .timeout(std::time::Duration::from_secs(30))
            .exchange(doors::exchange(failover))
            .base_url(doors::TYPESAFE_DOOR);
        let payer = Payer::Theirs {
            provider: first,
            fingerprint: self
                .keys
                .get(first)
                .map(ApiKey::fingerprint)
                .unwrap_or_default(),
        };
        Ok(Decisions::Theirs {
            config,
            order,
            payer,
        })
    }

    /// Who pays for the calls this access makes, as a run record keeps
    /// it: [`Paid::Ours`] with no keys, or [`Paid::Theirs`] with each of
    /// the person's keys that may pay (provider and fingerprint, in
    /// [`crate::PROVIDERS`] order), never a key.
    #[must_use]
    pub fn paid(&self) -> (Paid, Vec<KeyPrint>) {
        if !self.is_mine() {
            return (Paid::Ours, Vec::new());
        }
        let prints = self
            .keys
            .providers()
            .into_iter()
            .filter_map(|provider| {
                self.keys.get(provider).map(|key| KeyPrint {
                    provider,
                    fingerprint: key.fingerprint(),
                })
            })
            .collect();
        (Paid::Theirs, prints)
    }

    /// The payer a record names for a call through `door`, or ours.
    #[must_use]
    pub fn payer(&self, door: Option<&ChatDoor>) -> Payer {
        match door {
            Some(door) if self.is_mine() => door.payer(),
            _ => Payer::Ours,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRIMARY: &str = "stealth/space-bunny-alpha";
    const FALLBACK: &str = "google/gemini-3.8-flash";

    fn keys(providers: &[Provider]) -> Keys {
        let mut keys = Keys::none();
        for p in providers {
            keys.insert(*p, ApiKey::new(format!("key-{}", p.word())));
        }
        keys
    }

    fn theirs(doors: Doors) -> Vec<(Provider, String)> {
        match doors {
            Doors::Theirs(doors) => doors.into_iter().map(|d| (d.provider, d.model)).collect(),
            Doors::Ours => panic!("ours under mine"),
        }
    }

    #[test]
    fn a_record_names_who_paid_and_each_key_by_fingerprint_only() {
        let stored = keys(&[Provider::TypeSafe, Provider::OpenRouter]);
        assert_eq!(
            Access::new(Mode::Ours, stored.clone(), &Keys::none()).paid(),
            (Paid::Ours, Vec::new())
        );
        let (paid, prints) = Access::new(Mode::Mine, stored, &Keys::none()).paid();
        assert_eq!(paid, Paid::Theirs);
        assert_eq!(
            prints,
            vec![
                KeyPrint {
                    provider: Provider::OpenRouter,
                    fingerprint: crate::fingerprint("key-openrouter"),
                },
                KeyPrint {
                    provider: Provider::TypeSafe,
                    fingerprint: crate::fingerprint("key-typesafe"),
                },
            ]
        );
        let json = serde_json::to_string(&(paid, prints)).unwrap();
        assert!(json.contains("theirs") && !json.contains("key-openrouter"));
    }

    #[test]
    fn ours_keeps_our_doors_even_with_stored_keys() {
        let access = Access::new(Mode::Ours, keys(&[Provider::OpenRouter]), &Keys::none());
        assert_eq!(access.chat(Use::Embeddings).unwrap(), Doors::Ours);
        assert!(matches!(access.decisions().unwrap(), Decisions::Ours));
    }

    #[test]
    fn a_once_key_means_mine_for_the_invocation() {
        let once = keys(&[Provider::Vercel]);
        let access = Access::new(Mode::Ours, Keys::none(), &once);
        assert!(access.is_mine());
    }

    #[test]
    fn chat_order_is_openrouter_then_vercel_and_vercel_alone_gets_the_fallback() {
        let both = Access::new(
            Mode::Mine,
            keys(&[Provider::Vercel, Provider::OpenRouter, Provider::TypeSafe]),
            &Keys::none(),
        );
        let want = Use::Chat {
            primary: PRIMARY,
            fallback: FALLBACK,
        };
        assert_eq!(
            theirs(both.chat(want).unwrap()),
            vec![
                (Provider::OpenRouter, PRIMARY.into()),
                (Provider::OpenRouter, FALLBACK.into()),
                (Provider::Vercel, FALLBACK.into()),
            ]
        );
        let vercel = Access::theirs(keys(&[Provider::Vercel]));
        assert_eq!(
            theirs(vercel.chat(want).unwrap()),
            vec![(Provider::Vercel, FALLBACK.into())]
        );
        assert_eq!(
            theirs(both.chat(Use::Embeddings).unwrap()),
            vec![
                (Provider::OpenRouter, EMBEDDING_MODEL.into()),
                (Provider::Vercel, EMBEDDING_MODEL.into()),
            ]
        );
    }

    #[test]
    fn mine_without_a_serving_key_fails_plainly_and_never_returns_ours() {
        let typesafe = Access::theirs(keys(&[Provider::TypeSafe]));
        let error = typesafe.chat(Use::Model(MICROCODER_MODEL)).unwrap_err();
        assert_eq!(error.to_string(), "Your keys can't use openai/gpt-6.1-sol.");
        let vercel = Access::theirs(keys(&[Provider::Vercel]));
        assert!(vercel.chat(Use::Model(PRIMARY)).is_err());
        let none = Access::theirs(Keys::none());
        assert_eq!(
            none.decisions().unwrap_err().to_string(),
            "Your keys can't use Jev."
        );
    }

    #[test]
    fn decisions_order_is_typesafe_then_vercel_then_openrouter() {
        let all = Access::theirs(keys(&[
            Provider::OpenRouter,
            Provider::Vercel,
            Provider::TypeSafe,
        ]));
        let Decisions::Theirs { order, payer, .. } = all.decisions().unwrap() else {
            panic!("ours under mine");
        };
        assert_eq!(
            order,
            vec![Provider::TypeSafe, Provider::Vercel, Provider::OpenRouter]
        );
        assert_eq!(payer.word(), "theirs");
        let two = Access::theirs(keys(&[Provider::OpenRouter, Provider::Vercel]));
        let Decisions::Theirs { order, .. } = two.decisions().unwrap() else {
            panic!("ours under mine");
        };
        assert_eq!(order, vec![Provider::Vercel, Provider::OpenRouter]);
    }
}
