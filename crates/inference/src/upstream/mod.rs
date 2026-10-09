//! Upstream adapters: one per model account we hold (spec section 4).
//!
//! An [`Upstream`] speaks one provider's wire format and turns it into Open
//! Responses events. It says what it can do ([`Capabilities`] on each
//! [`ModelRow`]), what its endpoint does with a request ([`PrivacyTerms`]),
//! what each model costs ([`Price`]), and which [`Account`] pays. `send`
//! returns a [`Sent`]: the event stream and an [`AttemptMeter`] that measures the
//! attempt as the stream is read.
//!
//! | Adapter | Wire | Account |
//! | --- | --- | --- |
//! | [`vertex::Vertex`] | Vertex AI `streamGenerateContent` (Gemini) | Prepaid Google credit |
//! | [`zai::Zai`] | Z.ai Chat Completions | Prepaid Z.ai credit |
//! | [`pro::Pro`] | The Pro door's upstream, Chat Completions | Free capacity |
//! | [`openrouter::OpenRouter`] | OpenRouter Open Responses | OpenRouter credits |
//! | [`vercel::Vercel`] | Vercel AI Gateway Open Responses | Vercel credits |
//! | [`psionic::LocalPsionic`] | `psionic-serve` `/v1/responses` on this machine | None (`local`) |
//! | [`pylon::PylonUpstream`] | NIP-CJ jobs to a Pylon provider's `psionic-serve` | The provider, paid through the split ledger |
//!
//! Every adapter streams from its upstream, whether or not the caller asked
//! for a stream: the gateway collects a stream into one response for a
//! caller who did not. An adapter refuses, before any network call, a
//! request its upstream cannot honor: a capability the model lacks, or a
//! privacy level the endpoint's terms do not meet ([`check`]). Keys are
//! read through [`secret`] and never appear in an error, a measurement, or
//! a log line; adapters log nothing.
//!
//! A failure before the first output token is an `Err` the router may
//! fall back on (an HTTP error from `send`, or an `Err` item first on the
//! stream). After the first token, a failure is the caller's
//! `response.failed` event.

use std::fmt;
use std::future::Future;
use std::pin::Pin;

use futures_util::Stream;

use crate::event::Event;
use crate::item::{ContentPart, Item, MessageContent};
use crate::meter::RateRow;
use crate::openagents::Privacy;
use crate::request::{CreateResponse, TextFormat, Tool, ToolChoice, ToolChoiceMode};

pub mod chat;
pub mod emit;
pub mod gate;
pub mod google;
pub mod http;
pub mod measure;
pub mod openrouter;
pub mod pro;
pub mod psionic;
pub mod pylon;
pub mod responses;
pub mod secret;
pub mod vercel;
pub mod vertex;
pub mod whole;
pub mod zai;

pub use measure::{AttemptMeter, Measure, Stage};

/// A boxed future, so [`Upstream`] stays object safe.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// What an adapter's stream yields: Open Responses events, or a failure
/// before the first output token.
pub type EventStream = Pin<Box<dyn Stream<Item = Result<Event, AttemptError>> + Send>>;

/// One upstream account and the wire that reaches it.
pub trait Upstream: Send + Sync {
    /// The upstream's name in routes, records, and the rate card:
    /// `vertex`, `zai`, `pro`, `openrouter`, `vercel`, `local`,
    /// `pylon:<pylon>`.
    fn name(&self) -> &str;

    /// The account each call through this adapter is billed to.
    fn account(&self) -> &Account;

    /// What the endpoint does with a request, and how sure we are.
    fn privacy(&self) -> &PrivacyTerms;

    /// The models this adapter serves, with capabilities and price rows.
    fn models(&self) -> &[ModelRow];

    /// Whether a key (or token source) is configured. An adapter without
    /// one stays in the catalog and refuses every request with
    /// [`ErrorClass::Unconfigured`], so it switches on when the key lands.
    fn configured(&self) -> bool;

    /// Sends `request` to the model whose public id is `model`.
    ///
    /// The returned stream ends with a terminal event
    /// (`response.completed`, `response.incomplete`, or `response.failed`)
    /// or an `Err` before the first output token.
    fn send<'a>(
        &'a self,
        request: &'a CreateResponse,
        model: &'a str,
    ) -> BoxFuture<'a, Result<Sent, AttemptError>>;

    /// The row for the public model id `model`.
    fn model(&self, model: &str) -> Option<&ModelRow> {
        self.models().iter().find(|row| row.id == model)
    }

    /// What this adapter advertises to the router: one offering per
    /// model, `zero_retention` when its terms meet `strict`.
    fn offerings(&self) -> Vec<crate::router::Offering> {
        let zero_retention = self.privacy().allows(&Privacy::Strict);
        self.models()
            .iter()
            .map(|row| {
                let caps = row.capabilities;
                crate::router::Offering {
                    upstream: self.name().to_owned(),
                    model: row.id.clone(),
                    capabilities: crate::router::Capabilities {
                        tools: caps.tools,
                        json_schema: caps.json_schema,
                        images: caps.images,
                        files: false,
                        reasoning: caps.reasoning,
                        context: caps.context,
                        max_output: caps.max_output,
                    },
                    zero_retention,
                    payer: crate::openagents::Payer::Ours,
                    account: Some(self.account().id.clone()),
                }
            })
            .collect()
    }

    /// This adapter's rows for the meter's rate card.
    fn rate_rows(&self) -> Vec<RateRow> {
        self.models()
            .iter()
            .map(|row| row.price.rate_row(self.name(), &row.id))
            .collect()
    }
}

/// A started attempt: the event stream and the meter measuring it.
pub struct Sent {
    pub events: EventStream,
    pub meter: AttemptMeter,
}

impl fmt::Debug for Sent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sent").field("meter", &self.meter).finish()
    }
}

/// How an account's spend is paid for, which the router ranks on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CostBasis {
    /// A prepaid balance we already bought (Google, Z.ai).
    PrepaidCredit,
    /// Capacity that costs us nothing per call (the Pro door).
    FreeCapacity,
    /// Billed per call from a cash balance (OpenRouter, Vercel).
    PayAsYouGo,
}

impl CostBasis {
    /// The basis's wire word.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PrepaidCredit => "prepaid_credit",
            Self::FreeCapacity => "free_capacity",
            Self::PayAsYouGo => "pay_as_you_go",
        }
    }
}

/// The account an adapter bills.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    /// Stable id for the ledger and attempt records, such as
    /// `google-credit` or `zai-credit`.
    pub id: String,
    pub basis: CostBasis,
}

/// What an endpoint does with a request. `None` is "not verified".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrivacyTerms {
    /// Whether the provider trains on requests.
    pub trains: Option<bool>,
    /// Whether the endpoint keeps requests (for abuse review or caching)
    /// when the adapter asks it not to.
    pub retains: Option<bool>,
    /// Where the claim comes from: a terms page, a doc, or "unverified".
    pub source: &'static str,
}

impl PrivacyTerms {
    /// Terms nobody has checked: eligible for `standard`, never `strict`.
    #[must_use]
    pub fn unverified(source: &'static str) -> Self {
        Self {
            trains: None,
            retains: None,
            source,
        }
    }

    /// Terms we have checked: no training and zero retention when the
    /// adapter asks for it.
    #[must_use]
    pub fn zero_retention(source: &'static str) -> Self {
        Self {
            trains: Some(false),
            retains: Some(false),
            source,
        }
    }

    /// Whether a request at `level` may go to this endpoint. `strict`
    /// needs verified no-training and zero retention; `standard` needs
    /// only that the provider is not known to train on requests.
    #[must_use]
    pub fn allows(&self, level: &Privacy) -> bool {
        match level {
            Privacy::Standard => self.trains != Some(true),
            // Strict, and any level we do not know, is the strict rule.
            _ => self.trains == Some(false) && self.retains == Some(false),
        }
    }
}

/// List prices in micro-US-dollars per million tokens, so `150_000` is
/// $0.15 per million: the units of the meter's rate card
/// ([`crate::meter::RateRow`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Price {
    pub input: u64,
    pub cached_input: u64,
    /// Writing a prompt cache, where the upstream charges for it.
    pub cache_write: Option<u64>,
    pub output: u64,
}

impl Price {
    /// A price from dollars-per-million figures written as micro-dollars.
    #[must_use]
    pub const fn micro(input: u64, cached_input: u64, output: u64) -> Self {
        Self {
            input,
            cached_input,
            cache_write: None,
            output,
        }
    }

    /// This price as a rate card row for `model` through `upstream`, with
    /// the recommended margin ([`DEFAULT_MARGIN_BPS`]).
    #[must_use]
    pub fn rate_row(&self, upstream: &str, model: &str) -> RateRow {
        RateRow {
            upstream: upstream.to_owned(),
            model: model.to_owned(),
            currency: "USD".to_owned(),
            input: self.input,
            cached_input: Some(self.cached_input),
            cache_write: self.cache_write,
            output: self.output,
            margin_bps: DEFAULT_MARGIN_BPS,
            promotion: None,
        }
    }
}

/// The margin the spec recommends (5%, decision 1); the owner decides it,
/// and a deployment's rate card config can set another.
pub const DEFAULT_MARGIN_BPS: u32 = 500;

/// What a model can do through one adapter. The router reads these as hard
/// filters; [`check`] refuses a request that needs something missing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capabilities {
    pub tools: bool,
    pub reasoning: bool,
    /// The model reasons on every request and cannot be told not to
    /// (Z.ai's GLM). Its reasoning streams as reasoning items.
    pub reasoning_always_on: bool,
    pub json_schema: bool,
    pub images: bool,
    /// Context window, in tokens.
    pub context: u64,
    pub max_output: u64,
}

/// One model through one adapter: a rate card row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelRow {
    /// The public id, `publisher/model`.
    pub id: String,
    /// The id the upstream's API takes.
    pub upstream_model: String,
    pub capabilities: Capabilities,
    pub price: Price,
    /// Where the price comes from, and when it was read.
    pub price_source: &'static str,
}

/// Why an attempt failed. Measurements carry this class and the upstream
/// status, never the upstream's message.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ErrorClass {
    /// No key or token source is configured for this adapter.
    Unconfigured,
    /// The adapter refused before sending: the request's privacy level is
    /// one this endpoint's terms do not meet.
    PrivacyRefused,
    /// The adapter refused before sending: the model lacks a capability
    /// the request needs, or the model is not one this adapter serves.
    Unsupported,
    /// 401 or 403: the key is wrong or revoked. The router benches the
    /// upstream.
    Auth,
    /// 402: the account is out of credit. The router benches the upstream.
    Payment,
    /// 429.
    RateLimited,
    /// 400, 404, 413, 422: the upstream refused the request as sent.
    BadRequest,
    /// A 5xx, or a failure the upstream reported in the stream.
    Upstream,
    /// The request or a stream read timed out.
    Timeout,
    /// The connection failed or broke.
    Connection,
    /// The upstream answered in a shape we could not read.
    Decode,
    /// The stream finished with no output at all.
    Empty,
}

impl ErrorClass {
    /// The class's word in attempt records.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unconfigured => "unconfigured",
            Self::PrivacyRefused => "privacy_refused",
            Self::Unsupported => "unsupported",
            Self::Auth => "auth",
            Self::Payment => "payment",
            Self::RateLimited => "rate_limited",
            Self::BadRequest => "bad_request",
            Self::Upstream => "upstream",
            Self::Timeout => "timeout",
            Self::Connection => "connection",
            Self::Decode => "decode",
            Self::Empty => "empty",
        }
    }

    /// The class of an HTTP error status.
    #[must_use]
    pub fn of_status(status: u16) -> Self {
        match status {
            401 | 403 => Self::Auth,
            402 => Self::Payment,
            408 => Self::Timeout,
            429 => Self::RateLimited,
            400..=499 => Self::BadRequest,
            _ => Self::Upstream,
        }
    }

    /// Whether the router should try the next candidate. Everything but a
    /// request the next upstream would refuse the same way.
    #[must_use]
    pub fn falls_back(self) -> bool {
        !matches!(self, Self::BadRequest)
    }

    /// The attempt record's class for this failure. `status` is the
    /// upstream's HTTP status; `after_first_token` says whether output had
    /// begun (a failure in the stream rather than before it).
    #[must_use]
    pub fn record(self, status: Option<u16>, after_first_token: bool) -> crate::meter::ErrorClass {
        use crate::meter::ErrorClass as Record;
        match self {
            Self::Auth => Record::Auth,
            Self::Payment => Record::Payment,
            Self::RateLimited => Record::RateLimited,
            Self::BadRequest => Record::BadRequest,
            Self::Upstream if status.is_some_and(|status| status >= 500) => Record::Server,
            Self::Upstream if after_first_token => Record::StreamFailed,
            Self::Upstream => Record::Server,
            Self::Timeout => Record::Timeout,
            Self::Connection if after_first_token => Record::StreamFailed,
            Self::Connection => Record::Network,
            Self::Empty => Record::EmptyStream,
            Self::Unconfigured | Self::PrivacyRefused | Self::Unsupported | Self::Decode => {
                Record::Other
            }
        }
    }

    /// Whether the router should bench the upstream for a while (the
    /// key or the balance is the problem, not this request).
    #[must_use]
    pub fn benches(self) -> bool {
        matches!(self, Self::Auth | Self::Payment | Self::Unconfigured)
    }
}

/// A failed attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttemptError {
    pub class: ErrorClass,
    /// The upstream's HTTP status, when it sent one.
    pub status: Option<u16>,
    /// `Retry-After`, in seconds, when the upstream sent a number.
    pub retry_after: Option<u64>,
    /// A short reason for operators: the upstream's error message with any
    /// key-shaped text removed, truncated. Never prompt or completion text
    /// of ours, and never stored in a measurement.
    pub message: String,
}

impl AttemptError {
    #[must_use]
    pub fn new(class: ErrorClass, message: impl Into<String>) -> Self {
        Self {
            class,
            status: None,
            retry_after: None,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn status(mut self, status: u16) -> Self {
        self.status = Some(status);
        self
    }
}

impl fmt::Display for AttemptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.status {
            Some(status) => write!(f, "{} ({status}): {}", self.class.as_str(), self.message),
            None => write!(f, "{}: {}", self.class.as_str(), self.message),
        }
    }
}

impl std::error::Error for AttemptError {}

/// The privacy level a request asks for: `strict` unless it says
/// `standard`.
#[must_use]
pub fn privacy_level(request: &CreateResponse) -> Privacy {
    request
        .openagents
        .as_ref()
        .and_then(|options| options.privacy.clone())
        .unwrap_or(Privacy::Strict)
}

/// Whether the request declares or forces function tools.
#[must_use]
pub fn uses_tools(request: &CreateResponse) -> bool {
    let declared = request
        .tools
        .as_deref()
        .is_some_and(|tools| !tools.is_empty());
    let none = matches!(
        request.tool_choice,
        Some(ToolChoice::Mode(ToolChoiceMode::None))
    );
    let history = request
        .input_items()
        .iter()
        .any(|item| matches!(item, Item::FunctionCall(_) | Item::FunctionCallOutput(_)));
    (declared && !none) || history
}

/// Whether any input message carries an image.
#[must_use]
pub fn uses_images(request: &CreateResponse) -> bool {
    request.input_items().iter().any(|item| match item {
        Item::Message(message) => match &message.content {
            MessageContent::Parts(parts) => parts
                .iter()
                .any(|part| matches!(part, ContentPart::InputImage(_))),
            MessageContent::Text(_) => false,
        },
        _ => false,
    })
}

/// The checks every adapter makes before it sends: the model is one it
/// serves, the request's privacy level is one the endpoint's terms meet,
/// and the model has every capability the request needs.
///
/// # Errors
///
/// [`ErrorClass::Unsupported`] or [`ErrorClass::PrivacyRefused`], with a
/// sentence naming what is missing.
pub fn check<'a, U: Upstream + ?Sized>(
    upstream: &'a U,
    request: &CreateResponse,
    model: &str,
) -> Result<&'a ModelRow, AttemptError> {
    let name = upstream.name();
    let row = upstream.model(model).ok_or_else(|| {
        AttemptError::new(
            ErrorClass::Unsupported,
            format!("{name} does not serve {model}"),
        )
    })?;
    let level = privacy_level(request);
    if !upstream.privacy().allows(&level) {
        return Err(AttemptError::new(
            ErrorClass::PrivacyRefused,
            format!(
                "{name} is not eligible for `{level}` privacy ({})",
                upstream.privacy().source
            ),
        ));
    }
    let caps = row.capabilities;
    if uses_tools(request) && !caps.tools {
        return Err(AttemptError::new(
            ErrorClass::Unsupported,
            format!("{model} through {name} takes no tools"),
        ));
    }
    if uses_images(request) && !caps.images {
        return Err(AttemptError::new(
            ErrorClass::Unsupported,
            format!("{model} through {name} takes no images"),
        ));
    }
    let schema = request
        .text
        .as_ref()
        .and_then(|text| text.format.as_ref())
        .is_some_and(|format| matches!(format, TextFormat::JsonSchema(_)));
    if schema && !caps.json_schema {
        return Err(AttemptError::new(
            ErrorClass::Unsupported,
            format!("{model} through {name} takes no JSON schema"),
        ));
    }
    if let Some(max) = request.max_output_tokens
        && max > caps.max_output
    {
        return Err(AttemptError::new(
            ErrorClass::Unsupported,
            format!(
                "{model} through {name} writes at most {} tokens",
                caps.max_output
            ),
        ));
    }
    if request
        .tools
        .as_deref()
        .unwrap_or_default()
        .iter()
        .any(|tool| matches!(tool, Tool::Unknown(_)))
    {
        return Err(AttemptError::new(
            ErrorClass::Unsupported,
            format!("{name} takes function tools only"),
        ));
    }
    Ok(row)
}

/// A refusal for an adapter with no key.
#[must_use]
pub fn unconfigured(name: &str, what: &str) -> AttemptError {
    AttemptError::new(
        ErrorClass::Unconfigured,
        format!("{name} has no {what} configured"),
    )
}
