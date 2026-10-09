//! Pylon providers as upstreams (`docs/inference/gateway.md`, sections 4
//! and 8, P2).
//!
//! A Pylon provider runs `psionic-serve` and takes NIP-CJ conversation
//! jobs over Nostr. To serve gateway traffic it registers with the gateway
//! ([`Registration`]): its key, its pylon, the models it offers with its
//! own price per million tokens, and its stated data policy. Each
//! registration becomes one upstream, `pylon:<pylon>`:
//!
//! - **Price.** The rate card row is the provider's price plus the
//!   margin, like every other row. When an answer comes back, the adapter
//!   records an [`Earning`] for the provider of exactly their price for
//!   the tokens used; the caller's price is that plus the margin. The
//!   gateway's [`Earnings`] puts it in the split ledger (`pay-ledger`),
//!   and the payout worker pays it out over the existing rails. An answer
//!   whose earning cannot be recorded is not served (the attempt fails
//!   before its first token and the router falls back), so no provider
//!   work is used unpaid.
//! - **Privacy.** A provider whose stated policy is no training and no
//!   retention is eligible under `strict`; any other provider, including
//!   one that stated nothing, serves `standard` requests only.
//! - **Wire.** A job is the conversation as text turns: `system` and
//!   `developer` messages become the job's instructions, the last user
//!   message its task, earlier user and assistant messages its transcript
//!   (at most 32 turns and 16 KiB of text, NIP-CJ's bounds). No tools,
//!   images, or JSON schema. The job's result arrives whole and is
//!   replayed as Open Responses events ([`super::whole`]).
//! - **Transport.** [`Jobs`] sends a job and waits for its result. The
//!   gateway's sends it over a relay with the `pylon` crate; tests use a
//!   stub.
//!
//! An answer that arrives after the router gave up on the attempt (past
//! its first-token deadline) is not used, so it earns nothing.

use serde::{Deserialize, Serialize};

use super::whole::{Whole, events};
use super::{
    Account, AttemptError, AttemptMeter, BoxFuture, Capabilities, CostBasis, ErrorClass,
    EventStream, ModelRow, Price, PrivacyTerms, Sent, Upstream, check,
};
use crate::item::{ContentPart, Item, MessageContent, Role};
use crate::meter::Tokens;
use crate::request::CreateResponse;
use crate::response::{ResponseStatus, Usage};

/// The most turns a job carries (NIP-CJ).
pub const MAX_TURNS: usize = 32;
/// The most text a job carries, in bytes (NIP-CJ).
pub const MAX_TEXT: usize = 16 * 1024;

/// What a provider says it does with a request.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataPolicy {
    /// Whether the provider trains on requests; absent is "did not say".
    #[serde(default)]
    pub trains: Option<bool>,
    /// Whether the provider keeps requests after answering.
    #[serde(default)]
    pub retains: Option<bool>,
    /// The policy in the provider's words, or where it is published.
    #[serde(default)]
    pub statement: String,
}

impl DataPolicy {
    /// The privacy terms the router reads.
    #[must_use]
    pub fn terms(&self) -> PrivacyTerms {
        PrivacyTerms {
            trains: self.trains,
            retains: self.retains,
            source: if self.trains == Some(false) && self.retains == Some(false) {
                "the provider's stated data policy at registration: no training, no retention"
            } else {
                "the provider's stated data policy at registration"
            },
        }
    }
}

/// A model a provider offers, at its own price.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Offered {
    /// The public model id, `publisher/model`.
    pub id: String,
    /// The provider's model name (its `psionic-serve` id).
    pub model: String,
    pub context: u64,
    pub max_output: u64,
    /// The provider's price in micro-US-dollars per million tokens.
    pub input_micros: u64,
    pub output_micros: u64,
}

/// One provider's registration as a gateway upstream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registration {
    /// The provider's Nostr key (hex), which signs its beacon and results.
    pub provider: String,
    /// The pylon's slug (its beacon's `d` tag); the upstream is
    /// `pylon:<pylon>`.
    pub pylon: String,
    #[serde(default)]
    pub label: String,
    /// Who is paid, when not the provider key itself (the pylon's owner).
    #[serde(default)]
    pub owner: Option<String>,
    pub models: Vec<Offered>,
    #[serde(default)]
    pub policy: DataPolicy,
}

impl Registration {
    /// The upstream's name.
    #[must_use]
    pub fn upstream(&self) -> String {
        format!("pylon:{}", self.pylon)
    }

    /// The ledger party the provider is paid as: the owner when set,
    /// otherwise the provider key (as the broker names it).
    #[must_use]
    pub fn party(&self) -> String {
        self.owner.clone().unwrap_or_else(|| self.provider.clone())
    }

    /// Whether the registration is usable: a key, a pylon slug of plain
    /// characters, and at least one model.
    ///
    /// # Errors
    ///
    /// A sentence naming what is wrong.
    pub fn check(&self) -> Result<(), String> {
        if self.provider.len() != 64 || !self.provider.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(format!(
                "pylon {}: the provider key is not 64 hex characters",
                self.pylon
            ));
        }
        if self.pylon.is_empty()
            || !self
                .pylon
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        {
            return Err("a pylon slug is letters, digits, `-`, `_`, and `.`".into());
        }
        if self.models.is_empty() {
            return Err(format!("pylon {} offers no model", self.pylon));
        }
        Ok(())
    }
}

/// One text turn of a job.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Turn {
    /// `user` or `assistant`.
    pub role: String,
    pub content: String,
}

/// A job for a provider.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Job {
    /// The provider's model name.
    pub model: String,
    pub instructions: Option<String>,
    /// The turns before the task.
    pub history: Vec<Turn>,
    /// The last user message.
    pub task: String,
}

/// A provider's finished job.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Done {
    pub text: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// The job request's event id.
    pub request: String,
    /// The NIP-PYLON receipt's event id, when one was published.
    pub receipt: Option<String>,
}

/// Sends jobs to providers.
pub trait Jobs: Send + Sync {
    /// Runs `job` on `provider` and waits for its result.
    fn run<'a>(
        &'a self,
        provider: &'a Registration,
        job: Job,
    ) -> BoxFuture<'a, Result<Done, AttemptError>>;
}

/// What a provider earned from one answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Earning {
    /// The ledger party paid.
    pub party: String,
    pub provider: String,
    pub pylon: String,
    /// The public model id.
    pub model: String,
    /// The job request's event id: one earning per job.
    pub request: String,
    pub receipt: Option<String>,
    pub tokens: Tokens,
    /// The provider's price for those tokens, micro-dollars.
    pub provider_micros: u64,
    /// What the caller is priced: the provider's price plus the margin.
    pub price_micros: u64,
}

/// Records provider earnings.
pub trait Earnings: Send + Sync {
    /// Records `earning`.
    ///
    /// # Errors
    ///
    /// A sentence when it could not be recorded.
    fn earned(&self, earning: &Earning) -> Result<(), String>;
}

/// The job for `request` to `model` (the provider's name).
///
/// # Errors
///
/// [`ErrorClass::Unsupported`] for anything a NIP-CJ job cannot carry.
pub fn job(request: &CreateResponse, model: &str) -> Result<Job, AttemptError> {
    let unsupported = |why: &str| AttemptError::new(ErrorClass::Unsupported, why.to_owned());
    let mut instructions: Vec<String> = request.instructions.iter().cloned().collect();
    let mut turns: Vec<Turn> = Vec::new();
    for item in request.input_items() {
        match item {
            Item::Message(message) => {
                let text = match &message.content {
                    MessageContent::Text(text) => text.clone(),
                    MessageContent::Parts(parts) => {
                        let mut text = Vec::new();
                        for part in parts {
                            match part {
                                ContentPart::InputText(_)
                                | ContentPart::OutputText(_)
                                | ContentPart::Text(_) => {
                                    text.push(part.text().unwrap_or_default().to_owned());
                                }
                                ContentPart::Refusal(_) => {}
                                _ => return Err(unsupported("a Pylon job takes text only")),
                            }
                        }
                        text.join("\n")
                    }
                };
                match message.role {
                    Role::System | Role::Developer => instructions.push(text),
                    Role::User => turns.push(Turn {
                        role: "user".into(),
                        content: text,
                    }),
                    Role::Assistant => turns.push(Turn {
                        role: "assistant".into(),
                        content: text,
                    }),
                }
            }
            Item::Reasoning(_) => {}
            other => {
                return Err(unsupported(&format!(
                    "a Pylon job takes no `{}` items",
                    other.type_name()
                )));
            }
        }
    }
    let task = match turns.pop() {
        Some(turn) if turn.role == "user" => turn.content,
        _ => return Err(unsupported("a Pylon job needs a user message last")),
    };
    let instructions = (!instructions.is_empty()).then(|| instructions.join("\n\n"));
    let size = task.len()
        + instructions.as_ref().map_or(0, String::len)
        + turns.iter().map(|turn| turn.content.len()).sum::<usize>();
    if turns.len() + 1 > MAX_TURNS || size > MAX_TEXT {
        return Err(unsupported(
            "the conversation is longer than a Pylon job carries (32 turns, 16 KiB)",
        ));
    }
    Ok(Job {
        model: model.to_owned(),
        instructions,
        history: turns,
        task,
    })
}

/// One Pylon provider as an upstream.
pub struct PylonUpstream {
    name: String,
    registration: Registration,
    account: Account,
    privacy: PrivacyTerms,
    models: Vec<ModelRow>,
    jobs: std::sync::Arc<dyn Jobs>,
    earnings: std::sync::Arc<dyn Earnings>,
}

impl PylonUpstream {
    /// The upstream for `registration`, sending jobs through `jobs` and
    /// recording earnings into `earnings`.
    ///
    /// # Errors
    ///
    /// The registration's problem ([`Registration::check`]).
    pub fn new(
        registration: Registration,
        jobs: std::sync::Arc<dyn Jobs>,
        earnings: std::sync::Arc<dyn Earnings>,
    ) -> Result<Self, String> {
        registration.check()?;
        let name = registration.upstream();
        let models = registration
            .models
            .iter()
            .map(|offered| ModelRow {
                id: offered.id.clone(),
                upstream_model: offered.model.clone(),
                capabilities: Capabilities {
                    tools: false,
                    reasoning: false,
                    reasoning_always_on: false,
                    json_schema: false,
                    images: false,
                    context: offered.context,
                    max_output: offered.max_output,
                },
                price: Price::micro(
                    offered.input_micros,
                    offered.input_micros,
                    offered.output_micros,
                ),
                price_source: "the provider's own price at registration",
            })
            .collect();
        Ok(Self {
            account: Account {
                id: name.clone(),
                basis: CostBasis::PayAsYouGo,
            },
            privacy: registration.policy.terms(),
            name,
            registration,
            models,
            jobs,
            earnings,
        })
    }

    /// The registration.
    #[must_use]
    pub fn registration(&self) -> &Registration {
        &self.registration
    }
}

impl Upstream for PylonUpstream {
    fn name(&self) -> &str {
        &self.name
    }

    fn account(&self) -> &Account {
        &self.account
    }

    fn privacy(&self) -> &PrivacyTerms {
        &self.privacy
    }

    fn models(&self) -> &[ModelRow] {
        &self.models
    }

    fn configured(&self) -> bool {
        true
    }

    fn send<'a>(
        &'a self,
        request: &'a CreateResponse,
        model: &'a str,
    ) -> BoxFuture<'a, Result<Sent, AttemptError>> {
        Box::pin(async move {
            let row = check(self, request, model)?;
            let job = job(request, &row.upstream_model)?;
            let meter = AttemptMeter::start(self, row);
            let done = match self.jobs.run(&self.registration, job).await {
                Ok(done) => done,
                Err(error) => {
                    meter.fail(&error);
                    return Err(error);
                }
            };
            if done.text.is_empty() {
                let error = AttemptError::new(ErrorClass::Empty, "the provider sent no text");
                meter.fail(&error);
                return Err(error);
            }
            let tokens = Tokens {
                input: done.input_tokens,
                output: done.output_tokens,
                ..Tokens::default()
            };
            let priced = row.price.rate_row(&self.name, &row.id).price(&tokens);
            let earning = Earning {
                party: self.registration.party(),
                provider: self.registration.provider.clone(),
                pylon: self.registration.pylon.clone(),
                model: row.id.clone(),
                request: done.request.clone(),
                receipt: done.receipt.clone(),
                tokens,
                provider_micros: priced.cost,
                price_micros: priced.cost.saturating_add(priced.margin),
            };
            if let Err(why) = self.earnings.earned(&earning) {
                let error = AttemptError::new(
                    ErrorClass::Upstream,
                    format!("the provider's earning could not be recorded: {why}"),
                );
                meter.fail(&error);
                return Err(error);
            }
            meter.status(200);
            let whole = Whole {
                model: row.id.clone(),
                text: done.text,
                reasoning: None,
                calls: Vec::new(),
                usage: Usage::new(done.input_tokens, 0, done.output_tokens, 0),
                status: ResponseStatus::Completed,
            };
            let events: EventStream = Box::pin(futures_util::stream::iter(
                events(request, &whole).into_iter().map(Ok),
            ));
            Ok(Sent {
                events: meter.wrap(events),
                meter,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn conversations_become_jobs_within_nip_cj_bounds() {
        let request: CreateResponse = serde_json::from_value(json!({
            "instructions": "Be brief.",
            "input": [
                {"type": "message", "role": "developer", "content": "No lists."},
                {"type": "message", "role": "user", "content": "hi"},
                {"type": "message", "role": "assistant", "content": "hello"},
                {"type": "message", "role": "user",
                 "content": [{"type": "input_text", "text": "how are you?"}]}
            ]
        }))
        .unwrap();
        let job = job(&request, "qwen").unwrap();
        assert_eq!(job.instructions.as_deref(), Some("Be brief.\n\nNo lists."));
        assert_eq!(job.task, "how are you?");
        assert_eq!(job.history.len(), 2);
        assert_eq!(job.history[1].role, "assistant");

        let long: CreateResponse =
            serde_json::from_value(json!({"input": "x".repeat(MAX_TEXT + 1)})).unwrap();
        assert!(super::job(&long, "qwen").is_err());
        let ends_assistant: CreateResponse = serde_json::from_value(json!({"input": [
            {"type": "message", "role": "assistant", "content": "x"}]}))
        .unwrap();
        assert!(super::job(&ends_assistant, "qwen").is_err());
    }

    #[test]
    fn only_a_stated_zero_retention_policy_is_strict() {
        let strict = crate::openagents::Privacy::Strict;
        let standard = crate::openagents::Privacy::Standard;
        let silent = DataPolicy::default().terms();
        assert!(!silent.allows(&strict) && silent.allows(&standard));
        let keeps = DataPolicy {
            trains: Some(false),
            retains: Some(true),
            statement: String::new(),
        }
        .terms();
        assert!(!keeps.allows(&strict));
        let zero = DataPolicy {
            trains: Some(false),
            retains: Some(false),
            statement: "We keep nothing.".into(),
        }
        .terms();
        assert!(zero.allows(&strict));
        let trains = DataPolicy {
            trains: Some(true),
            ..DataPolicy::default()
        }
        .terms();
        assert!(!trains.allows(&standard));
    }

    #[test]
    fn registrations_are_checked() {
        let mut registration = Registration {
            provider: "a".repeat(64),
            pylon: "box-1".into(),
            label: String::new(),
            owner: None,
            models: vec![Offered {
                id: "qwen/qwen3.5-9b".into(),
                model: "qwen3.5-9b".into(),
                context: 32_768,
                max_output: 8_192,
                input_micros: 50_000,
                output_micros: 100_000,
            }],
            policy: DataPolicy::default(),
        };
        assert!(registration.check().is_ok());
        assert_eq!(registration.upstream(), "pylon:box-1");
        assert_eq!(registration.party(), "a".repeat(64));
        registration.owner = Some("owner-key".into());
        assert_eq!(registration.party(), "owner-key");
        registration.pylon = "../x".into();
        assert!(registration.check().is_err());
    }
}
