//! The router (`docs/inference/gateway.md`, section 5): a request's model id
//! or task class becomes an ordered list of at most three (model, upstream)
//! attempts.
//!
//! [`plan`] is a pure function of the request and a [`Context`]: the
//! catalog of what each upstream serves ([`Offering`]), the class table,
//! the rate card and credit ledger from [`crate::meter`], live rates, Gym
//! quality scores, and which upstreams are benched. The attempt loop that
//! sends to adapters uses [`fall_back`] and [`Bench::observe`] to decide
//! what happens when an attempt fails.
//!
//! Steps:
//!
//! 1. **Candidates.** A model id expands to every upstream that serves it;
//!    a task class to its table entries. `openagents/auto` takes the class
//!    a [`ClassJudge`] gives (a typed judgment, never keyword matching),
//!    `chat` without one. The request's `openagents.fallbacks` models follow.
//! 2. **Hard filters.** Capabilities the request's structure needs (tools,
//!    JSON schema, images, files, reasoning, context length), privacy
//!    (`strict` unless the request says `standard`), payer (`pay: "mine"`
//!    takes only the caller's own accounts, never ours), the caller's
//!    `max_price` and their own limits, benched upstreams, exhausted credit,
//!    and (for our accounts) a rate-card row to charge from.
//! 3. **Quality floor.** Candidates must meet the class's floor with both
//!    Gym scores and verified paid-outcome rates. Without outcomes, Gym
//!    scores stand; without either input, the class table's order stands.
//! 4. **Rank.** Credit we hold first (free capacity and prepaid balances,
//!    sooner expiry first), then estimated price, then measured time to
//!    first token. Without class scores, the table's model order comes
//!    before this ranking and the ranking orders each model's upstreams.
//!    `route.order` puts the named upstreams first; `route.sort` replaces
//!    the ranking.
//! 5. **Attempts.** The first three survive. Fallback happens only before
//!    the first output token, within the class's first-token deadline.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::{ApiError, ErrorType};
use crate::item::{ContentPart, Item, MessageContent, ToolOutput};
use crate::meter::{Basis, ErrorClass, Ledger, Rate, RateCard, RateRow};
use crate::openagents::{Payer, Privacy, Sort};
use crate::request::{CreateResponse, Input, TextFormat};

/// At most this many attempts per request.
pub const MAX_ATTEMPTS: usize = 3;
/// How long a 401 or 402 benches an upstream, ms.
pub const BENCH_MS: u64 = 5 * 60_000;
/// The router's own model id prefix.
pub const ROUTER_PREFIX: &str = "openagents/";
/// Requests over this many input tokens need the `long` class.
pub const LONG_CONTEXT: u64 = 200_000;

/// A task class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskClass {
    Classify,
    Fast,
    Chat,
    Code,
    Long,
    Reason,
}

impl TaskClass {
    pub const ALL: [Self; 6] = [
        Self::Classify,
        Self::Fast,
        Self::Chat,
        Self::Code,
        Self::Long,
        Self::Reason,
    ];

    /// The wire name (`classify`, `fast`, ...).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Classify => "classify",
            Self::Fast => "fast",
            Self::Chat => "chat",
            Self::Code => "code",
            Self::Long => "long",
            Self::Reason => "reason",
        }
    }

    /// The class a router model id names (`openagents/fast`), if any.
    #[must_use]
    pub fn from_model_id(model: &str) -> Option<Self> {
        let name = model.strip_prefix(ROUTER_PREFIX)?;
        Self::ALL.into_iter().find(|class| class.as_str() == name)
    }
}

/// What a model on an upstream can do.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    #[serde(default)]
    pub tools: bool,
    #[serde(default)]
    pub json_schema: bool,
    #[serde(default)]
    pub images: bool,
    #[serde(default)]
    pub files: bool,
    #[serde(default)]
    pub reasoning: bool,
    /// Context window, tokens.
    pub context: u64,
    /// Most output tokens per request; 0 for no stated limit.
    #[serde(default)]
    pub max_output: u64,
}

/// One model on one upstream: what an adapter advertises.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Offering {
    /// The upstream adapter's id (`vertex`, `zai`, `pro`, `openrouter`,
    /// `vercel`).
    pub upstream: String,
    /// Our model id (`google/gemini-3.8-flash`).
    pub model: String,
    pub capabilities: Capabilities,
    /// The upstream keeps no prompt or completion data (zero data
    /// retention), so it may serve `privacy: "strict"`.
    #[serde(default)]
    pub zero_retention: bool,
    /// Whose account pays.
    #[serde(default = "ours")]
    pub payer: Payer,
    /// The ledger account billed, when it is one we track.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
}

fn ours() -> Payer {
    Payer::Ours
}

/// One entry in a class's list: a model, optionally pinned to one upstream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassModel {
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream: Option<String>,
}

impl ClassModel {
    fn on(model: &str, upstream: &str) -> Self {
        Self {
            model: model.to_owned(),
            upstream: Some(upstream.to_owned()),
        }
    }
}

/// One class's row in the class table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClassEntry {
    pub models: Vec<ClassModel>,
    /// Fall back when no output token arrives within this, ms.
    pub first_token_ms: u64,
    /// How long the last planned attempt may take to its first token, ms:
    /// there is nothing left to fall back to, so a slow machine is waited
    /// out rather than failed. Never below `first_token_ms`; 0 means
    /// `first_token_ms`.
    #[serde(default)]
    pub last_ms: u64,
    /// The Gym score a candidate needs, when scores exist for the class.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floor: Option<f64>,
}

/// The class table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClassTable {
    pub classes: BTreeMap<TaskClass, ClassEntry>,
    /// First-token deadline for a request naming a model id, ms.
    pub model_first_token_ms: u64,
    /// The last attempt's ceiling for a request naming a model id, ms.
    #[serde(default)]
    pub model_last_ms: u64,
}

impl Default for ClassTable {
    /// The spec's starting table. Model ids and upstream names are the
    /// ones the P0 adapters use; the gateway's config replaces it.
    ///
    /// Gemini on Vertex AI comes first in every class it can serve
    /// (`classify`, `fast`, `chat`, `long`, and so `openagents/auto` when
    /// it judges one of them): it bills the prepaid Google credit (owner
    /// direction, 2026-10-10, #11222). The other upstreams follow in their
    /// earlier order. `code` and `reason` keep the Pro door first: Vertex
    /// serves no Gemini Pro model id here yet.
    fn default() -> Self {
        let flash = "google/gemini-3.8-flash";
        let glm = "zai/glm-5.3-flash";
        let entry = |models: Vec<ClassModel>, first_token_ms: u64, last_ms: u64| ClassEntry {
            models,
            first_token_ms,
            last_ms,
            floor: None,
        };
        let classes = BTreeMap::from([
            (
                TaskClass::Classify,
                entry(
                    vec![
                        ClassModel::on("google/gemini-2.5-flash-lite", "vertex"),
                        ClassModel::on("openai/gpt-5.6-luna", "pro"),
                        ClassModel::on(glm, "zai"),
                    ],
                    4_000,
                    10_000,
                ),
            ),
            (
                TaskClass::Fast,
                entry(
                    vec![
                        ClassModel::on(flash, "vertex"),
                        ClassModel::on(glm, "zai"),
                        ClassModel::on(flash, "openrouter"),
                    ],
                    4_000,
                    15_000,
                ),
            ),
            (
                TaskClass::Chat,
                entry(
                    vec![
                        ClassModel::on(flash, "vertex"),
                        ClassModel::on("openai/gpt-5.6-terra", "pro"),
                        ClassModel::on(flash, "openrouter"),
                    ],
                    8_000,
                    30_000,
                ),
            ),
            (
                TaskClass::Code,
                entry(
                    vec![
                        ClassModel::on("openai/gpt-5.6-sol", "pro"),
                        ClassModel::on("google/gemini-3.8-pro", "vertex"),
                        ClassModel::on(glm, "zai"),
                    ],
                    15_000,
                    60_000,
                ),
            ),
            (
                TaskClass::Long,
                entry(
                    vec![ClassModel::on(flash, "vertex"), ClassModel::on(glm, "zai")],
                    15_000,
                    60_000,
                ),
            ),
            (
                TaskClass::Reason,
                entry(
                    vec![
                        ClassModel::on("openai/gpt-5.6-sol", "pro"),
                        ClassModel::on("google/gemini-3.8-pro", "vertex"),
                        ClassModel::on("google/gemini-3.8-pro", "openrouter"),
                    ],
                    30_000,
                    90_000,
                ),
            ),
        ]);
        Self {
            classes,
            model_first_token_ms: 8_000,
            model_last_ms: 30_000,
        }
    }
}

/// Picks the class for `openagents/auto` with a typed judgment.
pub trait ClassJudge {
    /// The class for this request, or `None` when the judgment is not
    /// available (the router then uses `chat`).
    fn judge(&self, request: &CreateResponse) -> Option<TaskClass>;
}

/// Upstreams benched until a time, after a 401 or 402.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bench {
    until: BTreeMap<String, u64>,
}

impl Bench {
    /// Benches `upstream` for [`BENCH_MS`] when `error` is an auth or
    /// payment refusal; returns whether it did.
    pub fn observe(&mut self, upstream: &str, error: ErrorClass, now_ms: u64) -> bool {
        if matches!(error, ErrorClass::Auth | ErrorClass::Payment) {
            self.until
                .insert(upstream.to_owned(), now_ms.saturating_add(BENCH_MS));
            return true;
        }
        false
    }

    /// Whether `upstream` is benched at `now_ms`.
    #[must_use]
    pub fn is_benched(&self, upstream: &str, now_ms: u64) -> bool {
        self.until
            .get(upstream)
            .is_some_and(|until| now_ms < *until)
    }
}

/// Quality scores by class and model. The gateway combines Gym and paid outcomes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Scores {
    pub by_class: BTreeMap<TaskClass, BTreeMap<String, f64>>,
}

impl Scores {
    #[must_use]
    pub fn get(&self, class: TaskClass, model: &str) -> Option<f64> {
        self.by_class.get(&class)?.get(model).copied()
    }
}

/// Price limits in micros of US dollars per million tokens (what the
/// caller pays, margin included).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriceLimit {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<u64>,
}

impl PriceLimit {
    /// The tighter of two limits.
    #[must_use]
    pub fn min(self, other: Self) -> Self {
        let pick = |a: Option<u64>, b: Option<u64>| match (a, b) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        Self {
            input: pick(self.input, other.input),
            output: pick(self.output, other.output),
        }
    }
}

/// Everything [`plan`] reads besides the request.
pub struct Context<'a> {
    pub offerings: &'a [Offering],
    pub classes: &'a ClassTable,
    pub card: &'a RateCard,
    pub ledger: &'a Ledger,
    /// Live rates; the shortest window available is best.
    pub rates: &'a [Rate],
    pub scores: &'a Scores,
    pub bench: &'a Bench,
    /// Limits the key's owner set.
    pub limits: PriceLimit,
    pub judge: Option<&'a dyn ClassJudge>,
    pub now_ms: u64,
}

/// One planned attempt.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub model: String,
    pub upstream: String,
    pub payer: Payer,
    pub account: Option<String>,
    /// The caller's estimated price, micros of `currency`, when priced.
    pub estimate: Option<u64>,
    pub currency: Option<String>,
}

/// Why a candidate was dropped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Dropped {
    Capability(&'static str),
    ContextTooSmall,
    Privacy,
    Payer,
    OverPrice,
    Benched,
    CreditExhausted,
    NoPrice,
    BelowFloor,
    /// Left out by the caller's `route.only` or `route.ignore`.
    Excluded,
    BeyondAttemptLimit,
}

/// The router's answer.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    /// The class used, when the request named or was given one.
    pub class: Option<TaskClass>,
    /// At most [`MAX_ATTEMPTS`], best first.
    pub attempts: Vec<Candidate>,
    /// Fall back when no output token arrives within this, ms.
    pub first_token_ms: u64,
    /// The last attempt's ceiling for its first token, ms: never below
    /// `first_token_ms`. Nothing is left to fall back to, so a slow
    /// machine is waited out rather than failed.
    pub last_ms: u64,
    /// Candidates considered and dropped, with why.
    pub dropped: Vec<(Candidate, Dropped)>,
}

/// What the request's structure needs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Needs {
    pub tools: bool,
    pub json_schema: bool,
    pub images: bool,
    pub files: bool,
    pub reasoning: bool,
    /// Estimated input tokens (characters / 4).
    pub input_tokens: u64,
    pub max_output: Option<u64>,
}

impl Needs {
    /// Reads what `request` needs from its structure.
    #[must_use]
    pub fn of(request: &CreateResponse) -> Self {
        let mut needs = Self {
            tools: request
                .tools
                .as_ref()
                .is_some_and(|tools| !tools.is_empty()),
            json_schema: request
                .text
                .as_ref()
                .and_then(|text| text.format.as_ref())
                .is_some_and(|format| matches!(format, TextFormat::JsonSchema(_))),
            reasoning: request
                .reasoning
                .as_ref()
                .and_then(|reasoning| reasoning.effort.as_ref())
                .is_some_and(|effort| effort.as_str() != "none"),
            max_output: request.max_output_tokens,
            ..Self::default()
        };
        let mut chars = request.instructions.as_ref().map_or(0, String::len);
        if let Some(tools) = &request.tools {
            chars += serde_json::to_string(tools).map_or(0, |text| text.len());
        }
        let mut part = |part: &ContentPart, chars: &mut usize| match part {
            ContentPart::InputImage(_) => needs.images = true,
            ContentPart::InputFile(file) => {
                needs.files = true;
                *chars += file.file_data.as_ref().map_or(0, String::len) / 2;
            }
            other => *chars += other.text().map_or(0, str::len),
        };
        let items = match &request.input {
            None => Vec::new(),
            Some(Input::Text(text)) => {
                chars += text.len();
                Vec::new()
            }
            Some(Input::Items(items)) => items.clone(),
        };
        for item in &items {
            match item {
                Item::Message(message) => match &message.content {
                    MessageContent::Text(text) => chars += text.len(),
                    MessageContent::Parts(parts) => {
                        for each in parts {
                            part(each, &mut chars);
                        }
                    }
                },
                Item::FunctionCall(call) => chars += call.arguments.len() + call.name.len(),
                Item::FunctionCallOutput(output) => match &output.output {
                    ToolOutput::Text(text) => chars += text.len(),
                    ToolOutput::Parts(parts) => {
                        for each in parts {
                            part(each, &mut chars);
                        }
                    }
                },
                Item::Reasoning(reasoning) => {
                    chars += reasoning.summary_text().len()
                        + reasoning.encrypted_content.as_ref().map_or(0, String::len) / 2;
                }
                _ => {}
            }
        }
        needs.input_tokens = (chars as u64).div_ceil(4);
        needs
    }
}

/// Parses a decimal dollar string (`"0.25"`) to micros.
#[must_use]
pub fn usd_micros(text: &str) -> Option<u64> {
    let text = text.trim();
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    if whole.is_empty() && fraction.is_empty() {
        return None;
    }
    if !whole.chars().all(|c| c.is_ascii_digit()) || !fraction.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let whole: u64 = if whole.is_empty() {
        0
    } else {
        whole.parse().ok()?
    };
    let mut micros = whole.checked_mul(1_000_000)?;
    let digits: String = fraction
        .chars()
        .chain(std::iter::repeat('0'))
        .take(6)
        .collect();
    micros = micros.checked_add(digits.parse::<u64>().ok()?)?;
    // Round half up on the seventh digit.
    if fraction.chars().nth(6).is_some_and(|c| c >= '5') {
        micros = micros.checked_add(1)?;
    }
    Some(micros)
}

/// Formats micros as a decimal dollar string, trailing zeros trimmed.
#[must_use]
pub fn micros_usd(micros: u64) -> String {
    let whole = micros / 1_000_000;
    let fraction = micros % 1_000_000;
    if fraction == 0 {
        return whole.to_string();
    }
    let fraction = format!("{fraction:06}");
    format!("{whole}.{}", fraction.trim_end_matches('0'))
}

/// The caller's price per million tokens on a row, margin included.
#[must_use]
pub fn caller_rate(per_million: u64, row: &RateRow) -> u64 {
    let margin = (u128::from(per_million) * u128::from(row.margin_bps)).div_ceil(10_000);
    per_million.saturating_add(u64::try_from(margin).unwrap_or(u64::MAX))
}

/// The output tokens assumed when estimating a price with no limit set.
const ASSUMED_OUTPUT: u64 = 1_000;

/// Plans the attempts for `request`.
pub fn plan(request: &CreateResponse, context: &Context<'_>) -> Result<Plan, ApiError> {
    let model = request
        .model
        .as_deref()
        .filter(|model| !model.is_empty())
        .ok_or_else(|| ApiError::invalid_request("model", "Name a model or a task class."))?;
    let needs = Needs::of(request);
    let options = request.openagents.clone().unwrap_or_default();
    let strict = !matches!(options.privacy, Some(Privacy::Standard));
    let payer = options.pay.clone().unwrap_or(Payer::Ours);
    let mut limit = context.limits;
    if let Some(max) = &options.max_price {
        let parse = |field: &Option<String>, name: &str| -> Result<Option<u64>, ApiError> {
            field
                .as_deref()
                .map(|text| {
                    usd_micros(text).ok_or_else(|| {
                        ApiError::invalid_request(
                            format!("openagents.max_price.{name}"),
                            "Give a price in dollars per million tokens, such as \"0.50\".",
                        )
                    })
                })
                .transpose()
        };
        limit = limit.min(PriceLimit {
            input: parse(&max.input, "input")?,
            output: parse(&max.output, "output")?,
        });
    }

    // 1. Candidates.
    let class = if model == "openagents/auto" {
        Some(
            context
                .judge
                .and_then(|judge| judge.judge(request))
                .unwrap_or(TaskClass::Chat),
        )
    } else if model.starts_with(ROUTER_PREFIX) {
        Some(TaskClass::from_model_id(model).ok_or_else(|| {
            ApiError::new(
                ErrorType::NotFound,
                format!("There is no model called {model}."),
            )
            .with_code("model_not_found")
        })?)
    } else {
        None
    };
    let entry = class.and_then(|class| context.classes.classes.get(&class));
    if class.is_some() && entry.is_none() {
        return Err(ApiError::new(
            ErrorType::NoRoute,
            format!("No models are set up for {model} yet."),
        ));
    }
    // (offering, model position in the class table or request order)
    let mut considered: Vec<(&Offering, usize)> = Vec::new();
    match entry {
        Some(entry) => {
            for (position, wanted) in entry.models.iter().enumerate() {
                for offering in context.offerings {
                    if offering.model == wanted.model
                        && wanted
                            .upstream
                            .as_ref()
                            .is_none_or(|upstream| *upstream == offering.upstream)
                    {
                        considered.push((offering, position));
                    }
                }
            }
        }
        None => {
            for offering in context.offerings {
                if offering.model == model {
                    considered.push((offering, 0));
                }
            }
            if considered.is_empty() {
                return Err(ApiError::new(
                    ErrorType::NotFound,
                    format!("There is no model called {model}."),
                )
                .with_code("model_not_found"));
            }
        }
    }
    let base = considered
        .iter()
        .map(|(_, position)| position + 1)
        .max()
        .unwrap_or(0);
    for (index, fallback) in options.fallbacks.iter().enumerate() {
        for offering in context.offerings {
            if offering.model == *fallback {
                considered.push((offering, base + index));
            }
        }
    }
    // The same (model, upstream, payer) once, at its first position: our
    // OpenRouter and the caller's own OpenRouter are different offerings.
    let mut seen = std::collections::BTreeSet::new();
    considered.retain(|(offering, _)| {
        seen.insert((
            offering.model.clone(),
            offering.upstream.clone(),
            offering.payer.as_str().to_owned(),
        ))
    });

    // 2. Hard filters.
    let mut dropped = Vec::new();
    let mut kept: Vec<(Candidate, usize, Option<Rank>)> = Vec::new();
    for (offering, position) in considered {
        let row = context.card.get(&offering.upstream, &offering.model);
        let estimate = row.map(|row| {
            let output = needs.max_output.unwrap_or(ASSUMED_OUTPUT);
            let input = u128::from(needs.input_tokens) * u128::from(caller_rate(row.input, row));
            let output = u128::from(output) * u128::from(caller_rate(row.output, row));
            u64::try_from((input + output).div_ceil(1_000_000)).unwrap_or(u64::MAX)
        });
        let candidate = Candidate {
            model: offering.model.clone(),
            upstream: offering.upstream.clone(),
            payer: offering.payer.clone(),
            account: offering.account.clone(),
            estimate,
            currency: row.map(|row| row.currency.clone()),
        };
        let caps = &offering.capabilities;
        let reason = if needs.tools && !caps.tools {
            Some(Dropped::Capability("tools"))
        } else if needs.json_schema && !caps.json_schema {
            Some(Dropped::Capability("json_schema"))
        } else if needs.images && !caps.images {
            Some(Dropped::Capability("images"))
        } else if needs.files && !caps.files {
            Some(Dropped::Capability("files"))
        } else if needs.reasoning && !caps.reasoning {
            Some(Dropped::Capability("reasoning"))
        } else if caps.context > 0
            && needs.input_tokens + needs.max_output.unwrap_or(0) > caps.context
        {
            Some(Dropped::ContextTooSmall)
        } else if strict && !offering.zero_retention {
            Some(Dropped::Privacy)
        } else if offering.payer != payer {
            Some(Dropped::Payer)
        } else if context.bench.is_benched(&offering.upstream, context.now_ms) {
            Some(Dropped::Benched)
        } else if payer == Payer::Ours && row.is_none() {
            Some(Dropped::NoPrice)
        } else if row.is_some_and(|row| {
            limit
                .input
                .is_some_and(|max| caller_rate(row.input, row) > max)
                || limit
                    .output
                    .is_some_and(|max| caller_rate(row.output, row) > max)
        }) {
            Some(Dropped::OverPrice)
        } else if exhausted(offering, context) {
            Some(Dropped::CreditExhausted)
        } else {
            None
        };
        match reason {
            Some(reason) => dropped.push((candidate, reason)),
            None => {
                let rank = Rank::of(offering, &candidate, context);
                kept.push((candidate, position, Some(rank)));
            }
        }
    }

    // 3. Quality floor.
    let scored = class.is_some_and(|class| {
        context
            .scores
            .by_class
            .get(&class)
            .is_some_and(|scores| !scores.is_empty())
    });
    if let (Some(class), Some(floor), true) = (class, entry.and_then(|entry| entry.floor), scored) {
        let (pass, fail): (Vec<_>, Vec<_>) = kept.into_iter().partition(|(candidate, _, _)| {
            context
                .scores
                .get(class, &candidate.model)
                .is_some_and(|score| score >= floor)
        });
        dropped.extend(
            fail.into_iter()
                .map(|(candidate, _, _)| (candidate, Dropped::BelowFloor)),
        );
        kept = pass;
    }

    // 4. Rank.
    let keep_table_order = class.is_some() && !scored;
    let sort = options.route.as_ref().and_then(|route| route.sort.clone());
    kept.sort_by(|(a, a_position, a_rank), (b, b_position, b_rank)| {
        let (a_rank, b_rank) = (a_rank.as_ref(), b_rank.as_ref());
        let by_rank = match (&sort, a_rank, b_rank) {
            (Some(sort), Some(a_rank), Some(b_rank)) => a_rank.by(sort, b_rank, a, b, context),
            (None, Some(a_rank), Some(b_rank)) => a_rank.cmp(b_rank),
            _ => Ordering::Equal,
        };
        if keep_table_order && sort.is_none() {
            a_position.cmp(b_position).then(by_rank)
        } else {
            by_rank.then(a_position.cmp(b_position))
        }
    });
    if let Some(order) = options
        .route
        .as_ref()
        .map(|route| &route.order)
        .filter(|order| !order.is_empty())
    {
        let place = |candidate: &Candidate| {
            order
                .iter()
                .position(|upstream| *upstream == candidate.upstream)
                .unwrap_or(order.len())
        };
        kept.sort_by_key(|(candidate, _, _)| place(candidate));
    }
    if let Some(route) = &options.route {
        let only = &route.only;
        let ignore = &route.ignore;
        kept.retain(|(candidate, _, _)| {
            let allowed = (only.is_empty() || only.contains(&candidate.upstream))
                && !ignore.contains(&candidate.upstream);
            if !allowed {
                dropped.push((candidate.clone(), Dropped::Excluded));
            }
            allowed
        });
    }

    // 5. Attempts.
    let mut attempts: Vec<Candidate> = kept
        .into_iter()
        .map(|(candidate, _, _)| candidate)
        .collect();
    for extra in attempts.split_off(attempts.len().min(MAX_ATTEMPTS)) {
        dropped.push((extra, Dropped::BeyondAttemptLimit));
    }
    if attempts.is_empty() {
        return Err(no_route(model, &dropped));
    }
    Ok(Plan {
        class,
        attempts,
        first_token_ms: entry.map_or(context.classes.model_first_token_ms, |entry| {
            entry.first_token_ms
        }),
        last_ms: entry
            .map_or(context.classes.model_last_ms, |entry| entry.last_ms)
            .max(entry.map_or(context.classes.model_first_token_ms, |entry| {
                entry.first_token_ms
            })),
        dropped,
    })
}

fn exhausted(offering: &Offering, context: &Context<'_>) -> bool {
    let Some(account) = offering
        .account
        .as_deref()
        .and_then(|id| context.ledger.account(id))
    else {
        return false;
    };
    match account.basis {
        Basis::Prepaid => {
            account.balance == 0
                || account
                    .expires_at_ms
                    .is_some_and(|expires| expires <= context.now_ms)
        }
        Basis::FreeCapacity | Basis::PayAsYouGo => false,
    }
}

/// How a candidate ranks: credit we hold first (sooner expiry first), then
/// price, then time to first token.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Rank {
    /// 0 for prepaid credit or free capacity, 1 for cash.
    tier: u8,
    /// Expiry of the credit; `u64::MAX` when it does not expire.
    expires: u64,
    /// Estimated price, micros; `u64::MAX` when unpriced.
    price: u64,
    /// Measured p50 time to first token, ms; `u64::MAX` when unmeasured.
    ttft: u64,
}

impl Rank {
    fn of(offering: &Offering, candidate: &Candidate, context: &Context<'_>) -> Self {
        let account = offering
            .account
            .as_deref()
            .and_then(|id| context.ledger.account(id));
        let (tier, expires) = match account {
            Some(account) if account.basis == Basis::FreeCapacity => (0, u64::MAX),
            Some(account) if account.basis == Basis::Prepaid => {
                (0, account.expires_at_ms.unwrap_or(u64::MAX))
            }
            _ => (1, u64::MAX),
        };
        let ttft = context
            .rates
            .iter()
            .find(|rate| rate.upstream == offering.upstream && rate.model == offering.model)
            .and_then(|rate| rate.ttft_p50_ms)
            .unwrap_or(u64::MAX);
        Self {
            tier,
            expires,
            price: candidate.estimate.unwrap_or(u64::MAX),
            ttft,
        }
    }

    fn by(
        &self,
        sort: &Sort,
        other: &Self,
        a: &Candidate,
        b: &Candidate,
        context: &Context<'_>,
    ) -> Ordering {
        match sort {
            Sort::Price => self
                .price
                .cmp(&other.price)
                .then(self.ttft.cmp(&other.ttft)),
            Sort::Latency => self
                .ttft
                .cmp(&other.ttft)
                .then(self.price.cmp(&other.price)),
            Sort::Quality => {
                let score = |candidate: &Candidate| {
                    context
                        .scores
                        .by_class
                        .values()
                        .filter_map(|scores| scores.get(&candidate.model).copied())
                        .fold(f64::NEG_INFINITY, f64::max)
                };
                score(b)
                    .partial_cmp(&score(a))
                    .unwrap_or(Ordering::Equal)
                    .then(self.cmp(other))
            }
            Sort::Other(_) => self.cmp(other),
        }
    }
}

fn no_route(model: &str, dropped: &[(Candidate, Dropped)]) -> ApiError {
    let over_price = !dropped.is_empty()
        && dropped
            .iter()
            .all(|(_, reason)| matches!(reason, Dropped::OverPrice));
    if over_price {
        return ApiError {
            param: Some("openagents.max_price".into()),
            ..ApiError::new(
                ErrorType::LimitReached,
                format!("Every way to run {model} costs more than the price limit."),
            )
        };
    }
    ApiError::new(
        ErrorType::NoRoute,
        format!("No model can take this request right now ({model})."),
    )
}

/// When an attempt fails, whether the router tries the next candidate:
/// only before the first output token, and never for a request the
/// upstream refused as malformed (the next one would refuse it too).
#[must_use]
pub fn fall_back(first_token_sent: bool, error: ErrorClass) -> bool {
    !first_token_sent && error != ErrorClass::BadRequest
}
