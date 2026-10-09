//! T1 personalization: a cheap, fast model finishes a prepared stem.
//!
//! When the chat router picks a stem (a bank line that is true for every
//! message on its route, such as "Working on"), the stem is
//! shown at once and a small model writes only the rest of the sentence
//! from the user's own words: "… finding where the relay's retry timeout
//! is set and making it configurable." This module is the router's
//! [`seams::Personalize`] implementation, after the design's
//! "Personalization with a cheap model"
//! (`docs/coder/design/2026-09-28-chat-router.md`).
//!
//! The router owns the invariants around the seam: it decides when the seam
//! is called, builds the [`Ask`] (route, stem, and the latest message
//! already redacted by [`super::redact`] and cut to
//! [`seams::MESSAGE_CHARS`]), bounds the call with
//! [`seams::PERSONALIZE_BUDGET`], validates what comes back with
//! [`super::validate_continuation`], and closes the stem with its generic
//! ending on any failure. This module adds:
//!
//! - the prompt: [`INSTRUCTIONS`] and [`prompt_text`], which carries the
//!   [`Ask`]'s route, stem, and message and nothing else;
//! - two measured providers, streamed: an OpenRouter model
//!   ([`OpenRouterLane`], the default, which the shipped chat worker runs)
//!   and the gateway door's `glm` lane ([`GatewayLane`]), chosen by
//!   [`PROVIDER_VAR`]. Either is only the personalization model; the chat
//!   itself answers on the worker's own primary and lane;
//! - [`check`], which tidies what the model wrote (quotes, a repeated
//!   stem, a missing period) and refuses what the instructions forbid and
//!   the router's validator does not look for ("we", a button, a time,
//!   price, or guarantee the user did not write, a reply cut off by the
//!   token bound), before handing the text to the router's validator.
//!
//! The provider's stream is read to its end before anything is shown,
//! because a continuation is validated whole: streaming only ends the call
//! sooner. An error this seam reports names a class of failure, never
//! message or model text, because the worker logs it.
//!
//! Nothing here routes. The route and the stem are chosen by the router's
//! typed judgment; [`check`] is deterministic parsing of a bounded field
//! after that choice, which `AGENTS.md` allows.

use std::env;
use std::sync::Arc;
use std::time::Duration;

use futures_util::future::BoxFuture;
use serde_json::{Map, Value, json};

use super::seams::{self, Ask, Continuation, NoPersonalize, SeamError};
use crate::generate::{
    DEFAULT_DOOR_URL, Generate, Lane as ModelLane, Message, ProviderPrivacy, ResponsesDoor, Role,
};

/// The personalization contract's identity, for evidence.
pub const SET: &str = "chat-personalize-v1";

/// The most tokens a provider may write: 20 words is about 30 tokens, and
/// a continuation longer than [`super::MAX_CONTINUATION_CHARS`] is refused
/// anyway.
pub const MAX_TOKENS: u32 = 60;

/// The variable that turns personalization on and picks its provider:
/// `openrouter` (the [`DEFAULT_OPENROUTER_MODEL`]), `openrouter:<model>`,
/// `gateway` (the door's `glm` lane), `gateway:<lane or model>`, or `off`.
/// Unset is `off`.
pub const PROVIDER_VAR: &str = "CODER_PERSONALIZE";

/// The OpenRouter model personalization uses when the variable names none:
/// the fastest to pass validation in the 2026-09-28 measurement.
pub const DEFAULT_OPENROUTER_MODEL: &str = "google/gemini-2.5-flash-lite";

/// What the provider is told. The prompt's user message is
/// [`prompt_text`].
pub const INSTRUCTIONS: &str = "You finish one sentence for OpenAgents, a chat assistant. \
You receive a route, the start of a sentence (the stem), and the user's latest message. \
Write only the words that come after the stem, so that the stem followed by your words is \
one grammatical sentence saying what the user asked for: at most 20 words, ending with a \
period. When the stem ends with \"to\", begin with the verb the user asked for (fix, add, \
find, explain, review). When the stem ends with \"on\", begin with that verb's -ing form \
(fixing, adding, finding, explaining, reviewing). The stem starts with a verb ending in -ing; \
any further verbs you list after it, joined by commas or \"and\", take the -ing form too. \
Begin with a lowercase word and do not repeat the stem. Where the \
user wrote \"my\" or \"our\", write \"your\"; otherwise keep their names as written. \
Never write \"I\", \"me\", \"my\", \"we\", \"us\", or \"our\", and never name who does \
the work. Name only things the \
user named. Do not say the work is done, promise a time or a price, add a link, or mention a \
button. Reply with plain words alone, with no quotes or formatting.\n\n\
Example: stem \"Working on\", message \"can you add dark mode to my settings \
page\", reply \"adding dark mode to your settings page.\"\n\
Example: stem \"Looking through\", message \"how does our deploy script pick \
the binary?\", reply \"your deploy script to find how it picks the binary.\"\n\
Example: stem \"Picking up\", message \"review PR 412 on my repo and comment on it\", \
reply \"PR 412 on your repo, reviewing it, and commenting on it.\"";

/// The prompt's user message: the [`Ask`]'s route, stem, and message, and
/// nothing else. The message is cut to [`seams::MESSAGE_CHARS`] again here,
/// so a caller that skipped the router's bound still cannot send more.
#[must_use]
pub fn prompt_text(ask: &Ask) -> String {
    let message: String = ask.message.chars().take(seams::MESSAGE_CHARS).collect();
    format!(
        "Route: {}\nStem: {}\nMessage: {}",
        ask.route.word(),
        ask.stem.trim(),
        message
    )
}

/// Why [`check`] refused a continuation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The provider stopped at [`MAX_TOKENS`].
    CutOff,
    /// More than one line.
    NotOneLine,
    /// A question, where the stem makes a statement.
    Question,
    /// "we", "us", or "our": a promise the stem does not make.
    SpeaksForUs,
    /// Hands the work to someone by name ("have Coder", "Coder will",
    /// "dispatch"): a reply that starts work says what starts, never who
    /// is sent to do it.
    NamesWorker,
    /// Opens with a lone letter that is no word ("e a background process"),
    /// the end of a word whose start was lost (#10178).
    BrokenWord,
    /// A button or a tap, which the app shows itself.
    Button,
    /// A time, price, or guarantee, or a claim that work is already
    /// settled, that the user did not write.
    Promise,
    /// After a stem ending "on", the first word is not an -ing verb, so
    /// the sentence reads "Working on calc.add and … to fix calc.add" (#10348).
    NotVerbFirst,
    /// A rule of the router's own validator.
    Router(super::Invalid),
}

impl Refusal {
    /// The word a log line carries.
    #[must_use]
    pub fn word(&self) -> String {
        match self {
            Refusal::CutOff => "cut_off".to_string(),
            Refusal::NotOneLine => "not_one_line".to_string(),
            Refusal::Question => "question".to_string(),
            Refusal::SpeaksForUs => "speaks_for_us".to_string(),
            Refusal::NamesWorker => "names_worker".to_string(),
            Refusal::BrokenWord => "broken_word".to_string(),
            Refusal::Button => "button".to_string(),
            Refusal::Promise => "promise".to_string(),
            Refusal::NotVerbFirst => "not_verb_first".to_string(),
            Refusal::Router(invalid) => format!("{invalid:?}"),
        }
    }
}

/// Words a continuation may use only when the user's message has them.
/// The router's validator refuses the completion claims ("done", "fixed")
/// outright; these are the rest of what the instructions forbid.
const PROMISE_WORDS: &[&str] = &[
    "already",
    "resolved",
    "free",
    "guarantee",
    "guaranteed",
    "instantly",
    "immediately",
    "seconds",
    "minutes",
    "hours",
    "today",
    "tonight",
    "tomorrow",
];

const PLURAL_US: &[&str] = &[
    "we",
    "we'll",
    "we're",
    "we've",
    "we'd",
    "us",
    "our",
    "ours",
    "ourselves",
];

/// Phrases that hand the work to someone by name, where a reply that
/// starts work says only what starts ([`Refusal::NamesWorker`]).
pub const HANDOFF_PHRASES: &[&str] = &[
    "have coder",
    "having coder",
    "coder will",
    "coder to",
    "dispatch",
    "dispatching",
    "dispatched",
    "we'll have",
];

const BUTTON_WORDS: &[&str] = &[
    "tap", "taps", "tapping", "click", "clicking", "button", "buttons",
];

/// The words of `text`, lowercased, with curly apostrophes made straight.
fn words(text: &str) -> Vec<String> {
    text.replace(['\u{2019}', '\u{2018}'], "'")
        .split(|c: char| !(c.is_alphanumeric() || c == '\''))
        .map(|word| word.trim_matches('\'').to_lowercase())
        .filter(|word| !word.is_empty())
        .collect()
}

/// The continuation as it may be shown after `stem`, starting with the
/// space that joins them, or why not.
///
/// Before the checks: surrounding whitespace and quotes are removed, a
/// repeated stem is dropped, runs of spaces become one, and a period is
/// added when the sentence has no ending mark. After this module's own
/// rules, the text must pass [`super::validate_continuation`], which the
/// router applies again before showing it.
///
/// # Errors
///
/// The first [`Refusal`] the continuation meets.
pub fn check(written: &str, stem: &str, message: &str, cut_off: bool) -> Result<String, Refusal> {
    if cut_off {
        return Err(Refusal::CutOff);
    }
    let raw = written.trim();
    if raw.contains(['\n', '\r']) {
        return Err(Refusal::NotOneLine);
    }
    let mut text = raw
        .trim_matches(|c: char| matches!(c, '"' | '\'' | '\u{201c}' | '\u{201d}'))
        .trim()
        .to_string();
    let stem = stem.trim();
    if !stem.is_empty()
        && text.len() >= stem.len()
        && text.is_char_boundary(stem.len())
        && text[..stem.len()].eq_ignore_ascii_case(stem)
    {
        text = text[stem.len()..].trim().to_string();
    }
    let mut text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    match text.chars().last() {
        Some('?') => return Err(Refusal::Question),
        Some('.' | '!') | None => {}
        Some(_) => text.push('.'),
    }
    let said = words(&text);
    let asked = words(message);
    // A first word of one letter is "a", or the tail of a word cut short.
    if text.split_whitespace().next().is_some_and(|first| {
        first.chars().count() == 1
            && first.chars().all(char::is_alphabetic)
            && !first.eq_ignore_ascii_case("a")
    }) {
        return Err(Refusal::BrokenWord);
    }
    if said.iter().any(|word| PLURAL_US.contains(&word.as_str())) {
        return Err(Refusal::SpeaksForUs);
    }
    let joined = said.join(" ");
    if HANDOFF_PHRASES.iter().any(|phrase| {
        format!(" {joined} ").contains(&format!(" {phrase} "))
            && !format!(" {} ", asked.join(" ")).contains(&format!(" {phrase} "))
    }) {
        return Err(Refusal::NamesWorker);
    }
    if said
        .iter()
        .any(|word| BUTTON_WORDS.contains(&word.as_str()))
        || text.to_lowercase().contains("run coder")
    {
        return Err(Refusal::Button);
    }
    if said
        .iter()
        .any(|word| PROMISE_WORDS.contains(&word.as_str()) && !asked.contains(word))
        || text
            .chars()
            .any(|c| matches!(c, '$' | '€' | '£' | '₿') && !message.contains(c))
    {
        return Err(Refusal::Promise);
    }
    let text = super::validate_continuation(&text, message).map_err(Refusal::Router)?;
    // "Working on" takes an -ing verb next ("fixing calc.add"); anything
    // else repeats itself as "Working on X to fix X" (#10348).
    if stem
        .rsplit(' ')
        .next()
        .is_some_and(|last| last.eq_ignore_ascii_case("on"))
        && !text.split_whitespace().next().is_some_and(|first| {
            first
                .to_lowercase()
                .trim_end_matches([',', '.'])
                .ends_with("ing")
        })
    {
        return Err(Refusal::NotVerbFirst);
    }
    Ok(text)
}

/// What a provider wrote.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Written {
    /// The text, every streamed delta joined.
    pub text: String,
    /// Whether the provider stopped at its token limit.
    pub cut_off: bool,
}

/// A model that writes a continuation from [`INSTRUCTIONS`] and a
/// [`prompt_text`]. `sink` receives each delta as it streams; nothing a
/// provider writes is shown before [`check`] and the router's validator.
pub trait Provider: Send + Sync {
    /// The model that writes, as the result's `model` names it.
    fn model(&self) -> String;

    /// Where a message goes, named for a person, for the privacy answer.
    fn host(&self) -> String;

    /// Writes the continuation for `prompt`.
    ///
    /// # Errors
    ///
    /// A class of failure, never message or model text.
    fn write<'a>(
        &'a self,
        prompt: &'a str,
        sink: &'a mut (dyn FnMut(&str) + Send),
    ) -> impl std::future::Future<Output = Result<Written, String>> + Send + 'a;
}

/// The class of an OpenRouter failure, without its message or body, which
/// can quote the prompt.
fn openrouter_class(error: &openrouter::Error) -> String {
    match error {
        openrouter::Error::NoKey => "no OpenRouter key".to_string(),
        openrouter::Error::Client(_) => "the HTTP client could not start".to_string(),
        openrouter::Error::Api { status, .. } => format!("OpenRouter returned HTTP {status}"),
        openrouter::Error::Connection(_) => "could not reach OpenRouter".to_string(),
        openrouter::Error::Timeout => "OpenRouter timed out".to_string(),
        openrouter::Error::Decode { .. } | openrouter::Error::Schema { .. } => {
            "OpenRouter's stream could not be read".to_string()
        }
    }
}

/// An OpenRouter model, streamed through `crates/openrouter`.
#[derive(Clone, Debug)]
pub struct OpenRouterLane {
    client: openrouter::Client,
    model: String,
    /// What the request asks OpenRouter's providers about keeping and
    /// training on it (#11040).
    privacy: ProviderPrivacy,
}

impl OpenRouterLane {
    /// A lane for `model` through `client`.
    #[must_use]
    pub fn new(client: openrouter::Client, model: &str) -> Self {
        Self {
            client,
            model: model.to_string(),
            privacy: ProviderPrivacy::from_env(),
        }
    }

    /// The same lane asking for `privacy` instead of the environment's.
    #[must_use]
    pub fn with_privacy(mut self, privacy: ProviderPrivacy) -> Self {
        self.privacy = privacy;
        self
    }

    /// A lane for `model`, with the key from `OPENROUTER_API_KEY` (or
    /// `~/.openagents/openrouter.json`) and a client that makes one attempt,
    /// bounded a little past [`seams::PERSONALIZE_BUDGET`].
    ///
    /// # Errors
    ///
    /// A sentence when there is no key or the client cannot start.
    pub fn from_env(model: &str) -> Result<Self, String> {
        let mut config = openrouter::Config::from_env().map_err(|error| error.to_string())?;
        config.timeout = seams::PERSONALIZE_BUDGET + Duration::from_millis(300);
        config.retries = 0;
        let client = openrouter::Client::new(config).map_err(|error| error.to_string())?;
        Ok(Self::new(client, model))
    }

    fn request(&self, prompt: &str) -> openrouter::ChatRequest {
        let request = openrouter::ChatRequest::new(
            &self.model,
            vec![
                openrouter::Message::system(INSTRUCTIONS),
                openrouter::Message::user(prompt),
            ],
        )
        .max_tokens(MAX_TOKENS)
        .temperature(0.2);
        match self.privacy {
            ProviderPrivacy::Strict => request.no_retention(true),
            ProviderPrivacy::NoTraining => request.no_retention(false),
            ProviderPrivacy::Off => request,
        }
    }
}

impl Provider for OpenRouterLane {
    fn model(&self) -> String {
        self.model.clone()
    }

    fn host(&self) -> String {
        "OpenRouter".to_string()
    }

    async fn write<'a>(
        &'a self,
        prompt: &'a str,
        sink: &'a mut (dyn FnMut(&str) + Send),
    ) -> Result<Written, String> {
        let reply = self
            .client
            .stream(&self.request(prompt), sink)
            .await
            .map_err(|error| openrouter_class(&error))?;
        Ok(Written {
            cut_off: reply.finish_reason.as_deref() == Some("length"),
            text: reply.text,
        })
    }
}

/// A gateway door lane (the chat door's `CODER_DOOR_KEY` or
/// `CODER_AI_GATEWAY_KEY`), streamed through [`ResponsesDoor`], with the
/// model's reasoning turned off and its reply bounded to [`MAX_TOKENS`].
pub struct GatewayLane {
    door: ResponsesDoor,
}

impl GatewayLane {
    /// The request fields that make a lane answer without reasoning first:
    /// the Open Responses `reasoning` effort `none`, Z.ai's own `thinking`
    /// switch through the gateway's provider options, and the token bound.
    #[must_use]
    pub fn options() -> Map<String, Value> {
        let value = json!({
            "reasoning": { "effort": "none" },
            "providerOptions": { "zai": { "thinking": { "type": "disabled" } } },
            "max_output_tokens": MAX_TOKENS,
        });
        value.as_object().cloned().unwrap_or_default()
    }

    /// A lane on `door`, which keeps its URL and key and runs `model` (a
    /// lane name such as `glm`, or a model id).
    #[must_use]
    pub fn new(door: ResponsesDoor, model: &str) -> Self {
        Self {
            door: door.serving(model).with_options(Self::options()),
        }
    }

    /// A lane running `model` on the door the environment configures.
    ///
    /// # Errors
    ///
    /// A sentence when neither `CODER_DOOR_KEY` nor `CODER_AI_GATEWAY_KEY`
    /// is set.
    pub fn from_env(model: &str) -> Result<Self, String> {
        let door = ResponsesDoor::from_env()
            .ok_or_else(|| "no gateway door key: set CODER_DOOR_KEY".to_string())?;
        Ok(Self::new(door, model))
    }
}

impl Provider for GatewayLane {
    fn model(&self) -> String {
        self.door.model.clone()
    }

    fn host(&self) -> String {
        if self.door.url == DEFAULT_DOOR_URL {
            "the Vercel AI Gateway".to_string()
        } else {
            "the configured gateway door".to_string()
        }
    }

    async fn write<'a>(
        &'a self,
        prompt: &'a str,
        sink: &'a mut (dyn FnMut(&str) + Send),
    ) -> Result<Written, String> {
        let input = [Message {
            role: Role::User,
            text: prompt.to_string(),
        }];
        let mut meta = |_| {};
        match self
            .door
            .generate(INSTRUCTIONS, &input, sink, &mut meta)
            .await
        {
            Ok((text, _)) => Ok(Written {
                text,
                cut_off: false,
            }),
            // A reply the token bound cut short ends as `incomplete`.
            Err(error) if error.to_string().contains("max_output_tokens") => Ok(Written {
                text: String::new(),
                cut_off: true,
            }),
            Err(error) => Err(format!("the gateway lane failed: {}", error.cause())),
        }
    }
}

/// A provider for tests: a fixed line after a fixed wait, or a failure.
#[derive(Clone, Debug)]
pub struct StubLane {
    /// What it writes, or the failure it answers when `Err`.
    pub line: Result<String, String>,
    /// How long it takes.
    pub wait: Duration,
}

impl Provider for StubLane {
    fn model(&self) -> String {
        "stub".to_string()
    }

    fn host(&self) -> String {
        "a test stub".to_string()
    }

    async fn write<'a>(
        &'a self,
        _prompt: &'a str,
        sink: &'a mut (dyn FnMut(&str) + Send),
    ) -> Result<Written, String> {
        tokio::time::sleep(self.wait).await;
        let text = self.line.clone()?;
        sink(&text);
        Ok(Written {
            text,
            cut_off: false,
        })
    }
}

/// The configured provider, chosen by [`PROVIDER_VAR`]; the router holds it
/// as its [`seams::Personalize`].
pub enum Personalizer {
    /// An OpenRouter model.
    OpenRouter(OpenRouterLane),
    /// A gateway door lane.
    Gateway(GatewayLane),
    /// A fixed line, for tests.
    Stub(StubLane),
}

impl Personalizer {
    /// The provider [`PROVIDER_VAR`] names, `None` when it is unset or
    /// `off`.
    ///
    /// # Errors
    ///
    /// A sentence when the variable names an unknown provider, or the
    /// provider it names has no key: a worker asked to personalize should
    /// not start without the means to.
    pub fn from_env() -> Result<Option<Self>, String> {
        let asked = env::var(PROVIDER_VAR).unwrap_or_default();
        Self::named(asked.trim())
    }

    /// The provider `asked` names, in [`PROVIDER_VAR`]'s syntax.
    ///
    /// # Errors
    ///
    /// As [`Personalizer::from_env`].
    pub fn named(asked: &str) -> Result<Option<Self>, String> {
        let (kind, model) = asked.split_once(':').unwrap_or((asked, ""));
        let model = model.trim();
        match kind.trim() {
            "" | "off" => Ok(None),
            "openrouter" => {
                let model = if model.is_empty() {
                    DEFAULT_OPENROUTER_MODEL
                } else {
                    model
                };
                OpenRouterLane::from_env(model).map(|lane| Some(Self::OpenRouter(lane)))
            }
            "gateway" => {
                let model = if model.is_empty() {
                    ModelLane::Glm.name()
                } else {
                    model
                };
                GatewayLane::from_env(model).map(|lane| Some(Self::Gateway(lane)))
            }
            other => Err(format!(
                "{PROVIDER_VAR}={other} names no provider: use openrouter, gateway, or off"
            )),
        }
    }
}

impl Personalizer {
    /// The personalizer on a caller's own keys (BYOK, a `payer.keys`
    /// job): [`DEFAULT_OPENROUTER_MODEL`] on their OpenRouter key, else on
    /// their Vercel AI Gateway key. `None` when no key of theirs serves
    /// it, and the dispatch sentence then stays the bank's own words: it
    /// never falls back to our key.
    #[must_use]
    pub fn theirs(access: &model_access::Access) -> Option<Self> {
        let Ok(model_access::Doors::Theirs(doors)) =
            access.chat(model_access::Use::Model(DEFAULT_OPENROUTER_MODEL))
        else {
            return None;
        };
        let door = doors.into_iter().next()?;
        match door.provider {
            model_access::Provider::Vercel => Some(Self::Gateway(GatewayLane::new(
                ResponsesDoor::new(door.responses_base(), door.model.clone(), door.key.expose()),
                &door.model,
            ))),
            _ => {
                let config = openrouter::Config::new(openrouter::ApiKey::new(door.key.expose()))
                    .base_url(door.base_url);
                let client = openrouter::Client::new(config).ok()?;
                Some(Self::OpenRouter(OpenRouterLane::new(client, &door.model)))
            }
        }
    }
}

/// The seam the worker holds: the provider [`PROVIDER_VAR`] names, or
/// [`NoPersonalize`] when it names none.
///
/// # Errors
///
/// As [`Personalizer::from_env`].
pub fn seam_from_env() -> Result<Arc<dyn seams::Personalize>, String> {
    Ok(match Personalizer::from_env()? {
        Some(personalizer) => Arc::new(personalizer),
        None => Arc::new(NoPersonalize),
    })
}

impl Provider for Personalizer {
    fn model(&self) -> String {
        match self {
            Personalizer::OpenRouter(lane) => lane.model(),
            Personalizer::Gateway(lane) => lane.model(),
            Personalizer::Stub(lane) => lane.model(),
        }
    }

    fn host(&self) -> String {
        match self {
            Personalizer::OpenRouter(lane) => lane.host(),
            Personalizer::Gateway(lane) => lane.host(),
            Personalizer::Stub(lane) => lane.host(),
        }
    }

    async fn write<'a>(
        &'a self,
        prompt: &'a str,
        sink: &'a mut (dyn FnMut(&str) + Send),
    ) -> Result<Written, String> {
        match self {
            Personalizer::OpenRouter(lane) => lane.write(prompt, sink).await,
            Personalizer::Gateway(lane) => lane.write(prompt, sink).await,
            Personalizer::Stub(lane) => lane.write(prompt, sink).await,
        }
    }
}

impl seams::Personalize for Personalizer {
    fn available(&self) -> bool {
        true
    }

    fn recipients(&self) -> Vec<String> {
        vec![Provider::host(self)]
    }

    fn continuation<'a>(&'a self, ask: &'a Ask) -> BoxFuture<'a, Result<Continuation, SeamError>> {
        Box::pin(async move {
            let prompt = prompt_text(ask);
            let mut sink = |_: &str| {};
            let written = self
                .write(&prompt, &mut sink)
                .await
                .map_err(SeamError::Failed)?;
            let text = check(&written.text, &ask.stem, &ask.message, written.cut_off).map_err(
                |refusal| {
                    SeamError::Failed(format!("the continuation was refused: {}", refusal.word()))
                },
            )?;
            Ok(Continuation {
                text,
                model: Provider::model(self),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::seams::Personalize as _;
    use super::super::{Invalid, RouteId, redact};
    use super::*;

    const STEM: &str = "Working on";

    fn ask(message: &str) -> Ask {
        Ask {
            route: RouteId::WorkDispatch,
            answer: "dispatch.stem".to_string(),
            stem: STEM.to_string(),
            message: redact(message),
        }
    }

    #[test]
    fn the_prompt_carries_only_the_route_the_stem_and_the_message() {
        let key = "a".repeat(64);
        let ask = ask(&format!("rotate {key} and fix the build"));
        assert_eq!(
            prompt_text(&ask),
            "Route: work.dispatch\nStem: Working on\nMessage: rotate [redacted] \
             and fix the build"
        );
        let mut long = ask.clone();
        long.message = "fix it ".repeat(200);
        let text = prompt_text(&long);
        let message = text.split("Message: ").nth(1).unwrap();
        assert_eq!(message.chars().count(), seams::MESSAGE_CHARS);
    }

    #[test]
    fn a_good_continuation_is_tidied_and_joined_by_one_space() {
        let message = "fix the flaky retry test in crates/coder and open a PR";
        assert_eq!(
            check(
                "fixing the flaky retry test in crates/coder and opening a PR",
                STEM,
                message,
                false
            ),
            Ok(" fixing the flaky retry test in crates/coder and opening a PR.".to_string())
        );
        assert_eq!(
            check(
                "\"Working on fixing the flaky retry test.\"",
                STEM,
                message,
                false
            ),
            Ok(" fixing the flaky retry test.".to_string())
        );
        assert!(check("closing issue 9920.", STEM, "close issue 9920", false).is_ok());
        // A promise word the user wrote is theirs to repeat.
        assert!(check("shipping it today", STEM, "ship it today", false).is_ok());
    }

    #[test]
    fn after_on_the_continuation_starts_with_its_verb() {
        let message = "fix calc.add and run the test";
        assert_eq!(
            check(
                "calc.add and test_calc.py to fix calc.add and run the test.",
                STEM,
                message,
                false
            ),
            Err(Refusal::NotVerbFirst)
        );
        assert!(
            check(
                "fixing calc.add and running the test.",
                STEM,
                message,
                false
            )
            .is_ok()
        );
        // A stem that does not end in "on" takes its object first.
        assert!(
            check(
                "your repo to find the test.",
                "Looking through",
                "find the test in my repo",
                false
            )
            .is_ok()
        );
    }

    #[test]
    fn every_rule_refuses_its_own_failure() {
        let message = "look through my rails repo and tell me how auth works";
        let cases = [
            ("look through it\nand fix it", Refusal::NotOneLine),
            ("look through your repo?", Refusal::Question),
            (
                "look through your repo, and we'll open a PR",
                Refusal::SpeaksForUs,
            ),
            (
                "look through your repo after you tap Run Coder",
                Refusal::Button,
            ),
            ("having Coder look through your repo", Refusal::NamesWorker),
            ("e a background process", Refusal::BrokenWord),
            (
                "your repo, and Coder will report back",
                Refusal::NamesWorker,
            ),
            ("dispatching a run on your repo", Refusal::NamesWorker),
            (
                "look through your repo, which is already fine",
                Refusal::Promise,
            ),
            ("look through your repo in five minutes", Refusal::Promise),
            ("look through your repo for $5", Refusal::Promise),
            ("", Refusal::Router(Invalid::Empty)),
            (
                &"look through the repo ".repeat(10),
                Refusal::Router(Invalid::TooLong),
            ),
            (
                "look through it. Then fix it.",
                Refusal::Router(Invalid::MoreThanOneSentence),
            ),
            ("read https://example.com", Refusal::Router(Invalid::Url)),
            (
                "fix the test in **coder**",
                Refusal::Router(Invalid::Markup),
            ),
            (
                "look through my rails repo",
                Refusal::Router(Invalid::FirstPersonSingular),
            ),
            (
                "look through your repo until auth is fixed",
                Refusal::Router(Invalid::ClaimsCompletion),
            ),
            (
                "look through the 3 auth modules",
                Refusal::Router(Invalid::NewNumber),
            ),
        ];
        for (written, expected) in cases {
            assert_eq!(
                check(written, STEM, message, false),
                Err(expected),
                "{written}"
            );
        }
        assert_eq!(
            check("look through your repo", STEM, message, true),
            Err(Refusal::CutOff)
        );
    }

    fn stub(line: Result<&str, &str>) -> Personalizer {
        Personalizer::Stub(StubLane {
            line: line.map(str::to_string).map_err(str::to_string),
            wait: Duration::ZERO,
        })
    }

    #[tokio::test]
    async fn the_seam_answers_a_checked_continuation_and_names_its_recipient() {
        let seam = stub(Ok("fixing your login bug"));
        assert!(seam.available());
        assert_eq!(seam.recipients(), vec!["a test stub".to_string()]);
        let continuation = seam.continuation(&ask("fix my login bug")).await.unwrap();
        assert_eq!(continuation.text, " fixing your login bug.");
        assert_eq!(continuation.model, "stub");
    }

    #[tokio::test]
    async fn a_refused_or_failed_continuation_is_an_error_without_its_text() {
        let refused = stub(Ok("fix my login bug"))
            .continuation(&ask("fix my login bug"))
            .await
            .unwrap_err();
        assert_eq!(
            refused,
            SeamError::Failed("the continuation was refused: FirstPersonSingular".to_string())
        );
        let failed = stub(Err("OpenRouter returned HTTP 503"))
            .continuation(&ask("fix my login bug"))
            .await
            .unwrap_err();
        assert_eq!(
            failed,
            SeamError::Failed("OpenRouter returned HTTP 503".to_string())
        );
    }

    #[test]
    fn an_openrouter_failure_is_named_by_its_class_only() {
        let error = openrouter::Error::Decode {
            detail: "bad".to_string(),
            excerpt: "fix my secret project".to_string(),
        };
        assert!(!openrouter_class(&error).contains("secret"));
    }

    #[test]
    fn the_provider_variable_is_off_unless_it_names_one() {
        assert!(matches!(Personalizer::named(""), Ok(None)));
        assert!(matches!(Personalizer::named("off"), Ok(None)));
        assert!(Personalizer::named("carrier-pigeon").is_err());
    }

    #[test]
    fn the_gateway_lane_turns_reasoning_off_and_bounds_its_reply() {
        let options = GatewayLane::options();
        assert_eq!(options["reasoning"]["effort"], "none");
        assert_eq!(
            options["providerOptions"]["zai"]["thinking"]["type"],
            "disabled"
        );
        assert_eq!(options["max_output_tokens"], MAX_TOKENS);
        let lane = GatewayLane::new(ResponsesDoor::new(DEFAULT_DOOR_URL, "x", "k"), "glm");
        assert_eq!(lane.model(), ModelLane::Glm.model());
        assert_eq!(lane.host(), "the Vercel AI Gateway");
    }

    /// #11040: the OpenRouter lane asks its providers not to keep or train
    /// on the request.
    #[test]
    fn the_openrouter_lane_asks_providers_not_to_keep_the_request() {
        let client =
            openrouter::Client::new(openrouter::Config::new(openrouter::ApiKey::new("k"))).unwrap();
        let lane = OpenRouterLane::new(client, "m").with_privacy(ProviderPrivacy::Strict);
        let sent = serde_json::to_value(lane.request("p")).unwrap();
        assert_eq!(sent["provider"]["data_collection"], "deny");
        assert_eq!(sent["provider"]["zdr"], true);
        let sent =
            serde_json::to_value(lane.with_privacy(ProviderPrivacy::Off).request("p")).unwrap();
        assert_eq!(sent.get("provider"), None);
    }

    #[test]
    fn the_instructions_forbid_what_the_checks_refuse() {
        for word in ["\"I\"", "\"we\"", "button", "link", "done", "20 words"] {
            assert!(INSTRUCTIONS.contains(word), "{word}");
        }
    }
}
