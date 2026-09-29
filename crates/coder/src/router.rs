//! The chat router: how OpenAgents answers a message, with Jev choosing the
//! route (`docs/coder/design/2026-09-28-chat-router.md`).
//!
//! For a turn that asks (`"router": "chat-router-v1"`), the chat worker
//! asks one System One (Jev) request, the `chat-router-v1` question set,
//! beside the model call and never in front of it. Code, not the judge,
//! maps the answers to a [`Tier`] through the policy table in
//! [`decide`]:
//!
//! | Tier | What the user sees | The model call |
//! | --- | --- | --- |
//! | T0 [`Tier::CannedFinal`], [`Tier::Refuse`] | a reviewed bank answer, whole | cancelled |
//! | T1 [`Tier::CannedStem`] | a bank stem, then a validated continuation | cancelled |
//! | T2 [`Tier::Grounded`] | the model, answering from retrieved passages | restarted with them |
//! | T3 [`Tier::Model`] | an optional lead line, then the model | kept |
//! | T4 [`Tier::Offer`] | a bank sentence and an offer card | cancelled |
//!
//! Text the user sees comes from three places only: the answer bank
//! ([`bank`], reviewed and versioned), a continuation ([`seams::Personalize`],
//! bounded and validated here), or the model. Jev writes no text.
//!
//! Nothing here is keyword matching: every routing reading is a Choice
//! answer's argmax or a Noul's probability over options this module lists.
//! Deterministic parsing appears only after the route is chosen and only
//! for bounded shapes: redacting secrets from what a seam may see
//! ([`redact`]) and validating a continuation ([`validate_continuation`]).
//!
//! The seams in [`seams`] are how other modules plug in: personalization
//! (T1), the product and codebase knowledge bases (T2), and the
//! `openagents` command tree (T4 CLI). Each has a no-op default.

pub mod personalize;
pub mod seams;

use serde_json::{Value, json};

/// The question set's identity, for evidence and for the wire.
pub const SET: &str = "chat-router-v1";

/// The least Jev relevance at which a retrieved passage is used.
pub const RELEVANCE_FLOOR: f64 = 0.5;

/// The least relevance at which a product entry's own reviewed answer is
/// served whole (T0), when the message needs no specifics.
pub const KB_ANSWER_CONFIDENCE: f64 = 0.8;

/// A route: the kind of reply a message calls for. The wire word is
/// [`RouteId::word`]; the description Jev reads is
/// [`RouteId::description`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RouteId {
    /// Questions about us: model, identity, capabilities, limits, pricing,
    /// privacy.
    Meta,
    /// Greetings, thanks, checks that the chat works.
    Smalltalk,
    /// General knowledge and help answerable in a chat reply.
    General,
    /// How to do something in the OpenAgents app or with its services.
    ProductKb,
    /// How the OpenAgents software is built.
    CodebaseKb,
    /// Work on code, a repository, files, or a machine.
    WorkDispatch,
    /// Something an `openagents` command does.
    Cli,
    /// The wallet, bitcoin amounts, receiving, sending, backup.
    Wallet,
    /// Settings, keys, computers, playtests, reporting a problem.
    Account,
    /// Too ambiguous to act on or answer well.
    Clarify,
    /// The user is done.
    End,
    /// Something we must not help with, or a message holding a secret.
    Refuse,
    /// The judge chose `none`, or did not answer.
    Unknown,
}

impl RouteId {
    /// Every route the `route` question offers, in order (`Unknown` is its
    /// `none`).
    pub const ALL: [RouteId; 12] = [
        RouteId::Meta,
        RouteId::Smalltalk,
        RouteId::General,
        RouteId::ProductKb,
        RouteId::CodebaseKb,
        RouteId::WorkDispatch,
        RouteId::Cli,
        RouteId::Wallet,
        RouteId::Account,
        RouteId::Clarify,
        RouteId::End,
        RouteId::Refuse,
    ];

    /// The word the wire and the bank carry.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            RouteId::Meta => "meta",
            RouteId::Smalltalk => "smalltalk",
            RouteId::General => "general",
            RouteId::ProductKb => "product.kb",
            RouteId::CodebaseKb => "codebase.kb",
            RouteId::WorkDispatch => "work.dispatch",
            RouteId::Cli => "cli",
            RouteId::Wallet => "wallet",
            RouteId::Account => "account",
            RouteId::Clarify => "clarify",
            RouteId::End => "end",
            RouteId::Refuse => "refuse",
            RouteId::Unknown => "none",
        }
    }

    /// The route a wire or bank word names; `Unknown` for anything else.
    #[must_use]
    pub fn parse(word: &str) -> Self {
        RouteId::ALL
            .into_iter()
            .find(|route| route.word() == word)
            .unwrap_or(RouteId::Unknown)
    }

    /// What Jev reads for this route: a semantic description, not words to
    /// look for, with what it does not cover where a neighbor is close.
    #[must_use]
    pub fn description(self) -> &'static str {
        match self {
            RouteId::Meta => {
                "Questions about us, the assistant itself: what model or AI this is, who we are \
                 or who built us, what we can and cannot do, what it costs, message limits, \
                 privacy, whether we remember things, whether we are open source, or what \
                 Coder or Jev is"
            }
            RouteId::Smalltalk => {
                "A greeting, thanks, praise, or a check that the chat works, with no question or \
                 request in it"
            }
            RouteId::General => {
                "A question about the world or programming concepts, an explanation, writing \
                 help, or advice: anything answerable in a chat reply without OpenAgents product \
                 facts and without the user's own files or repositories"
            }
            RouteId::ProductKb => {
                "How to do something in the OpenAgents app or with OpenAgents services, or what \
                 an OpenAgents feature is (connecting a computer, the Grid, the Verse, XP, \
                 protocols such as NIP-CJ); not the wallet, and not account settings"
            }
            RouteId::CodebaseKb => {
                "How the OpenAgents software itself is built: where something lives in the \
                 OpenAgents repository, how one of its crates or protocols works, or why it was \
                 designed that way; not the user's own code"
            }
            RouteId::WorkDispatch => {
                "The user wants work done on code, a repository, files, or a machine: change, \
                 fix, build, test, refactor, review a pull request, look through a repository, \
                 find where something is in their code, run a command, or pick up a GitHub \
                 issue"
            }
            RouteId::Cli => {
                "The user wants something an `openagents` command does for their own account or \
                 devices: list or check their computers, look up their XP or quests, search the \
                 knowledge base, or read what is published on a relay"
            }
            RouteId::Wallet => {
                "Questions about the OpenAgents wallet: bitcoin amounts and units, receiving or \
                 getting paid, sending, backups and recovery words, or fees, including a request \
                 to send money"
            }
            RouteId::Account => {
                "Account and settings in the OpenAgents app: identity keys, adding or removing \
                 computers, turning on a playtest session, or reporting a problem or crash"
            }
            RouteId::Clarify => {
                "The latest message is too ambiguous to act on or answer well without asking \
                 what the user means"
            }
            RouteId::End => {
                "The user is done: a goodbye or sign-off with no new question or request"
            }
            RouteId::Refuse => {
                "A request we must not help with (harm to people, stealing, or getting into \
                 someone else's accounts or keys), or a message where the user pasted a private \
                 key, password, or recovery words"
            }
            RouteId::Unknown => "None of these fits the message",
        }
    }
}

/// What the `risk` question found, independent of the route.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Risk {
    /// Nothing to refuse or warn about.
    Ok,
    /// The user pasted a key, recovery words, or a password.
    SecretShared,
    /// The user asks for someone's secret, or for ours.
    AsksForSecret,
    /// A request we must not help with.
    Harmful,
    /// A request to move money.
    MoneyMovement,
    /// The judge chose `none`, or did not answer.
    Unknown,
}

impl Risk {
    /// The options the `risk` question offers, in order.
    pub const ALL: [Risk; 5] = [
        Risk::Ok,
        Risk::SecretShared,
        Risk::AsksForSecret,
        Risk::Harmful,
        Risk::MoneyMovement,
    ];

    /// The word the wire carries.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Risk::Ok => "ok",
            Risk::SecretShared => "secret_shared",
            Risk::AsksForSecret => "asks_for_secret",
            Risk::Harmful => "harmful",
            Risk::MoneyMovement => "money_movement",
            Risk::Unknown => "none",
        }
    }

    /// The risk a word names; `Unknown` for anything else.
    #[must_use]
    pub fn parse(word: &str) -> Self {
        Risk::ALL
            .into_iter()
            .find(|risk| risk.word() == word)
            .unwrap_or(Risk::Unknown)
    }
}

/// Where the chat is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    Phone,
    Desktop,
    Terminal,
}

impl Surface {
    /// The word the wire carries.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Surface::Phone => "phone",
            Surface::Desktop => "desktop",
            Surface::Terminal => "terminal",
        }
    }

    fn parse(word: &str) -> Option<Self> {
        [Surface::Phone, Surface::Desktop, Surface::Terminal]
            .into_iter()
            .find(|surface| surface.word() == word)
    }
}

/// The longest `context.app_build`, in bytes.
pub const MAX_BUILD_BYTES: usize = 64;

/// The bounded context a request may carry. It holds no credential, key,
/// host name, or amount, and it never reaches a seam.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Context {
    /// Where the chat is; the phone when unsaid.
    pub surface: Option<Surface>,
    /// Whether the device has a ready computer; unknown when unsaid.
    pub computer_ready: Option<bool>,
    /// The app's build, for the bank's version-specific entries.
    pub app_build: Option<String>,
}

impl Context {
    /// Reads `context` from a request payload. Unknown fields, wrong
    /// types, and an overlong build are ignored rather than refused: the
    /// context only ever narrows what is offered.
    #[must_use]
    pub fn of(value: &Value) -> Self {
        Self {
            surface: value["surface"].as_str().and_then(Surface::parse),
            computer_ready: value["computer_ready"].as_bool(),
            app_build: value["app_build"]
                .as_str()
                .filter(|build| {
                    build.len() <= MAX_BUILD_BYTES && !build.chars().any(char::is_control)
                })
                .map(str::to_string),
        }
    }

    /// The surface, the phone when unsaid.
    #[must_use]
    pub fn surface(&self) -> Surface {
        self.surface.unwrap_or(Surface::Phone)
    }
}

/// A command's effect class, declared next to its usage string.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Reads only.
    ReadOnly,
    /// Writes this device's stores.
    LocalWrite,
    /// Signs and sends an event.
    Publishes,
    /// Changes who may do what.
    Grants,
    /// Moves money.
    Spends,
    /// Shows or exports a secret.
    Secret,
    /// Runs until stopped.
    LongRunning,
}

impl Effect {
    /// The word the wire carries.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Effect::ReadOnly => "read_only",
            Effect::LocalWrite => "local_write",
            Effect::Publishes => "publishes",
            Effect::Grants => "grants",
            Effect::Spends => "spends",
            Effect::Secret => "secret",
            Effect::LongRunning => "long_running",
        }
    }
}

/// Where a command can run for a surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunsOn {
    /// The device's own client.
    ThisDevice,
    /// Through a connected computer.
    ConnectedComputer,
    /// A screen on the device does it better.
    Screen,
}

impl RunsOn {
    /// The word the wire carries.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            RunsOn::ThisDevice => "this_device",
            RunsOn::ConnectedComputer => "connected_computer",
            RunsOn::Screen => "screen",
        }
    }
}

/// A screen an `open_screen` offer opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    AccountComputers,
    AccountKeys,
    AccountPlaytest,
    AccountReportProblem,
    Wallet,
}

impl Screen {
    /// Every screen, in order.
    pub const ALL: [Screen; 5] = [
        Screen::AccountComputers,
        Screen::AccountKeys,
        Screen::AccountPlaytest,
        Screen::AccountReportProblem,
        Screen::Wallet,
    ];

    /// The word the wire and the bank carry.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Screen::AccountComputers => "account.computers",
            Screen::AccountKeys => "account.keys",
            Screen::AccountPlaytest => "account.playtest",
            Screen::AccountReportProblem => "account.report_problem",
            Screen::Wallet => "wallet",
        }
    }

    /// The screen a word names.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        Screen::ALL.into_iter().find(|screen| screen.word() == word)
    }
}

/// An action the reply puts one tap in front of. An offer is an
/// observation, never permission: the router never dispatches, runs, or
/// opens anything itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Offer {
    /// Dispatch Coder to the connected computer with this conversation as
    /// its task. The target comes from the screen's controls, never from
    /// the message.
    RunCoder { label: String },
    /// Open a screen of the app.
    OpenScreen { screen: Screen, label: String },
    /// Run an `openagents` command, after a confirm.
    Cli {
        argv: Vec<String>,
        effect: Effect,
        runs_on: RunsOn,
    },
}

impl Offer {
    /// The word the wire carries in `offer`.
    #[must_use]
    pub fn word(&self) -> &'static str {
        match self {
            Offer::RunCoder { .. } => "run_coder",
            Offer::OpenScreen { .. } => "open_screen",
            Offer::Cli { .. } => "cli",
        }
    }

    /// The `27000` `offer` feedback body at payload `version`.
    #[must_use]
    pub fn feedback(&self, version: u64) -> Value {
        let mut body = json!({
            "v": version,
            "requires": [],
            "type": "offer",
            "offer": self.word(),
        });
        match self {
            Offer::RunCoder { label } => {
                body["target"] = json!("connected_computer");
                body["label"] = json!(label);
            }
            Offer::OpenScreen { screen, label } => {
                body["screen"] = json!(screen.word());
                body["label"] = json!(label);
            }
            Offer::Cli {
                argv,
                effect,
                runs_on,
            } => {
                body["argv"] = json!(argv);
                body["effect"] = json!(effect.word());
                body["runs_on"] = json!(runs_on.word());
                body["confirm"] = json!(true);
            }
        }
        body
    }
}

/// What a CLI proposal becomes on a surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CliGate {
    /// Offer the command, with a confirm.
    Offer,
    /// Do not propose the command; open this screen instead.
    Screen(Screen),
    /// Do not propose anything for it.
    Withhold,
}

/// The gate a command's effect meets on a surface. Money and secrets are
/// never proposed from chat; on the phone only read-only commands are
/// offered, and grants open the computers screen instead.
#[must_use]
pub fn gate(effect: Effect, surface: Surface) -> CliGate {
    match (effect, surface) {
        (Effect::Spends | Effect::Secret, Surface::Phone) => CliGate::Screen(Screen::Wallet),
        (Effect::Spends | Effect::Secret, _) => CliGate::Withhold,
        (Effect::ReadOnly, _) => CliGate::Offer,
        (Effect::Grants, Surface::Phone) => CliGate::Screen(Screen::AccountComputers),
        (_, Surface::Phone) => CliGate::Withhold,
        (Effect::LocalWrite | Effect::Publishes | Effect::Grants | Effect::LongRunning, _) => {
            CliGate::Offer
        }
    }
}

/// The corpus a grounded reply reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corpus {
    Product,
    Codebase,
}

impl Corpus {
    /// The word the wire carries.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Corpus::Product => "product",
            Corpus::Codebase => "codebase",
        }
    }
}

// ---------------------------------------------------------------------------
// Bounded parsing after the route is chosen
// ---------------------------------------------------------------------------

/// What a redacted secret shape becomes.
pub const REDACTED: &str = "[redacted]";

/// The latest message as a seam may see it: bounded secret shapes replaced
/// by [`REDACTED`] (a Nostr `nsec`, a 64-hex key, a BOLT11 invoice, an
/// LNURL, a Lightning address, an on-chain address), then cut to
/// [`seams::MESSAGE_CHARS`] characters. This parses bounded fields after
/// the route is chosen, which `AGENTS.md` allows; it routes nothing.
#[must_use]
pub fn redact(message: &str) -> String {
    let mut out = String::with_capacity(message.len());
    let mut token = String::new();
    let flush = |token: &mut String, out: &mut String| {
        if !token.is_empty() {
            let core = token.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '@');
            if secret_shape(core) {
                let start = token.find(core).unwrap_or(0);
                out.push_str(&token[..start]);
                out.push_str(REDACTED);
                out.push_str(&token[start + core.len()..]);
            } else {
                out.push_str(token);
            }
            token.clear();
        }
    };
    for c in message.chars() {
        if c.is_whitespace() {
            flush(&mut token, &mut out);
            out.push(c);
        } else {
            token.push(c);
        }
    }
    flush(&mut token, &mut out);
    out.chars().take(seams::MESSAGE_CHARS).collect()
}

fn secret_shape(token: &str) -> bool {
    let lower = token.to_ascii_lowercase();
    let alnum = |s: &str| s.chars().all(|c| c.is_ascii_alphanumeric());
    let hex64 = token.len() == 64 && token.chars().all(|c| c.is_ascii_hexdigit());
    let bech =
        |prefix: &str, min: usize| lower.starts_with(prefix) && lower.len() >= min && alnum(&lower);
    let base58 = (26..=35).contains(&token.len())
        && (token.starts_with('1') || token.starts_with('3'))
        && token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() && !matches!(c, '0' | 'O' | 'I' | 'l'))
        && token.chars().any(|c| c.is_ascii_uppercase())
        && token.chars().any(|c| c.is_ascii_digit());
    let lightning_address = token.split_once('@').is_some_and(|(user, domain)| {
        !user.is_empty()
            && domain.contains('.')
            && !domain.starts_with('.')
            && !domain.ends_with('.')
            && user
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '+'))
            && domain
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
    });
    hex64
        || bech("nsec1", 50)
        || bech("lnbc", 20)
        || bech("lntb", 20)
        || bech("lntbs", 20)
        || bech("lnbcrt", 20)
        || bech("lnurl", 20)
        || bech("bc1", 26)
        || bech("tb1", 26)
        || bech("bcrt1", 26)
        || base58
        || lightning_address
}

/// Why a continuation was not shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Invalid {
    Empty,
    TooLong,
    MoreThanOneSentence,
    Url,
    FirstPersonSingular,
    ClaimsCompletion,
    NewNumber,
    Markup,
}

/// The longest continuation shown, in characters.
pub const MAX_CONTINUATION_CHARS: usize = 160;

/// A continuation as it may be shown after its stem, or why not: at most
/// [`MAX_CONTINUATION_CHARS`] characters, one sentence, no URL, no
/// first-person singular pronoun, no claim of completion, no digits that
/// are not in the user's message, and no markup or control characters.
/// The result starts with the space that joins it to the stem, unless it
/// starts with punctuation.
///
/// # Errors
///
/// The first rule the continuation breaks.
pub fn validate_continuation(continuation: &str, message: &str) -> Result<String, Invalid> {
    let text = continuation.trim_end();
    let trimmed = text.trim_start();
    if trimmed.is_empty() {
        return Err(Invalid::Empty);
    }
    if trimmed.chars().count() > MAX_CONTINUATION_CHARS {
        return Err(Invalid::TooLong);
    }
    if trimmed
        .chars()
        .any(|c| c.is_control() || matches!(c, '`' | '*' | '#' | '<' | '>' | '[' | ']' | '{' | '}'))
    {
        return Err(Invalid::Markup);
    }
    let lower = trimmed.to_lowercase();
    if lower.contains("http://") || lower.contains("https://") || lower.contains("www.") {
        return Err(Invalid::Url);
    }
    // One sentence: a terminator may only end the text.
    let body = trimmed.trim_end_matches(['.', '!', '?']);
    if body
        .char_indices()
        .any(|(i, c)| matches!(c, '.' | '!' | '?') && body[i + 1..].starts_with(' '))
    {
        return Err(Invalid::MoreThanOneSentence);
    }
    let words: Vec<String> = lower
        .split(|c: char| !(c.is_alphanumeric() || c == '\'' || c == '’'))
        .filter(|word| !word.is_empty())
        .map(|word| word.replace('’', "'"))
        .collect();
    if words.iter().any(|word| {
        matches!(
            word.as_str(),
            "i" | "i'll" | "i'm" | "i've" | "i'd" | "me" | "my" | "mine" | "myself"
        )
    }) {
        return Err(Invalid::FirstPersonSingular);
    }
    if words.iter().any(|word| {
        matches!(
            word.as_str(),
            "done" | "fixed" | "finished" | "completed" | "merged" | "deployed" | "shipped"
        )
    }) {
        return Err(Invalid::ClaimsCompletion);
    }
    let numbers = |text: &str| -> Vec<String> {
        text.split(|c: char| !c.is_ascii_digit())
            .filter(|run| !run.is_empty())
            .map(str::to_string)
            .collect()
    };
    let known = numbers(message);
    if numbers(trimmed).iter().any(|run| !known.contains(run)) {
        return Err(Invalid::NewNumber);
    }
    Ok(if trimmed.starts_with([',', '.', ';', ':', '!', '?']) {
        trimmed.to_string()
    } else {
        format!(" {trimmed}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_words_round_trip_and_describe_themselves() {
        for route in RouteId::ALL {
            assert_eq!(RouteId::parse(route.word()), route);
            assert!(!route.description().is_empty());
        }
        assert_eq!(RouteId::parse("none"), RouteId::Unknown);
        assert_eq!(RouteId::parse("weather"), RouteId::Unknown);
        for risk in Risk::ALL {
            assert_eq!(Risk::parse(risk.word()), risk);
        }
        for screen in Screen::ALL {
            assert_eq!(Screen::parse(screen.word()), Some(screen));
        }
    }

    #[test]
    fn context_is_bounded_and_optional() {
        assert_eq!(Context::of(&Value::Null), Context::default());
        assert_eq!(Context::default().surface(), Surface::Phone);
        let context = Context::of(&json!({
            "surface": "desktop", "computer_ready": true, "app_build": "1.0.0 (19)",
            "host": "secret-box"
        }));
        assert_eq!(context.surface, Some(Surface::Desktop));
        assert_eq!(context.computer_ready, Some(true));
        assert_eq!(context.app_build.as_deref(), Some("1.0.0 (19)"));
        let odd = Context::of(&json!({
            "surface": "fridge", "computer_ready": "yes", "app_build": "x".repeat(65)
        }));
        assert_eq!(odd, Context::default());
    }

    /// Money and secrets are never proposed from chat, and the phone gets
    /// read-only commands only; grants open the computers screen.
    #[test]
    fn the_cli_gate_follows_the_effect_table() {
        use CliGate::{Offer as O, Screen as S, Withhold as W};
        let table = [
            (Effect::ReadOnly, [O, O, O]),
            (Effect::LocalWrite, [W, O, O]),
            (Effect::Publishes, [W, O, O]),
            (Effect::Grants, [S(Screen::AccountComputers), O, O]),
            (Effect::Spends, [S(Screen::Wallet), W, W]),
            (Effect::Secret, [S(Screen::Wallet), W, W]),
            (Effect::LongRunning, [W, O, O]),
        ];
        for (effect, expected) in table {
            for (surface, want) in [Surface::Phone, Surface::Desktop, Surface::Terminal]
                .into_iter()
                .zip(expected)
            {
                assert_eq!(gate(effect, surface), want, "{effect:?} on {surface:?}");
            }
        }
    }

    #[test]
    fn offers_serialize_as_nip_cj_offer_feedback() {
        let run = Offer::RunCoder {
            label: "Run Coder".into(),
        }
        .feedback(2);
        assert_eq!(run["type"], "offer");
        assert_eq!(run["offer"], "run_coder");
        assert_eq!(run["target"], "connected_computer");
        let screen = Offer::OpenScreen {
            screen: Screen::AccountComputers,
            label: "Connect a computer".into(),
        }
        .feedback(2);
        assert_eq!(screen["screen"], "account.computers");
        let cli = Offer::Cli {
            argv: vec!["computer".into(), "list".into()],
            effect: Effect::ReadOnly,
            runs_on: RunsOn::ThisDevice,
        }
        .feedback(2);
        assert_eq!(cli["argv"], json!(["computer", "list"]));
        assert_eq!(cli["effect"], "read_only");
        assert_eq!(cli["runs_on"], "this_device");
        assert_eq!(cli["confirm"], true);
    }

    #[test]
    fn redaction_replaces_bounded_secret_shapes_and_bounds_length() {
        let key = "a".repeat(64);
        let nsec = format!("nsec1{}", "q".repeat(58));
        let message = format!(
            "my key is {key}, and {nsec}. pay lnbc1500n1pj9x7ypp5qqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqf \
             to bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq or 1BoatSLRHtKNngkdXEeobR76b53LETtpyT \
             or alice@getalby.com, then fix crates/coder/src/router.rs line 42"
        );
        let redacted = redact(&message);
        assert!(!redacted.contains(&key));
        assert!(!redacted.contains("nsec1"));
        assert!(!redacted.contains("lnbc"));
        assert!(!redacted.contains("bc1q"));
        assert!(!redacted.contains("1BoatSLR"));
        assert!(!redacted.contains("alice@"));
        assert!(redacted.contains("[redacted],"), "{redacted}");
        assert!(redacted.contains("crates/coder/src/router.rs line 42"));
        assert_eq!(
            redact(&"word ".repeat(300)).chars().count(),
            seams::MESSAGE_CHARS
        );
    }

    #[test]
    fn continuations_are_validated_before_they_are_shown() {
        let message = "fix the flaky test in crates/coder and bump to 0.4";
        assert_eq!(
            validate_continuation("fix the flaky test in crates/coder.", message),
            Ok(" fix the flaky test in crates/coder.".to_string())
        );
        assert_eq!(
            validate_continuation(", then bump the version to 0.4.", message),
            Ok(", then bump the version to 0.4.".to_string())
        );
        let bad = [
            ("", Invalid::Empty),
            (&"x ".repeat(90), Invalid::TooLong),
            ("fix it. Then open a PR.", Invalid::MoreThanOneSentence),
            ("read https://example.com", Invalid::Url),
            ("fix what I broke", Invalid::FirstPersonSingular),
            ("confirm the test is fixed", Invalid::ClaimsCompletion),
            ("bump to 0.5", Invalid::NewNumber),
            ("run `cargo test`", Invalid::Markup),
        ];
        for (text, why) in bad {
            assert_eq!(validate_continuation(text, message), Err(why), "{text}");
        }
    }
}
