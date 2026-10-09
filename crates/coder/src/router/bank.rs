//! The answer bank: reviewed, versioned short answers in the OpenAgents
//! voice, and the openers that may lead a model's reply.
//!
//! The bank is a data file, `crates/coder/answers/chat-answers-v1.toml`,
//! compiled into the worker. Its identity on the wire is
//! `chat-answers-v1@<digest>`, where the digest is the first 12 hex digits
//! of the file's SHA-256, so a logged `meta.model@1` can always be traced
//! to the exact text that was shown.
//!
//! [`lint`] holds the file to the rules the design sets: plural voice, no
//! button named, fact slots declared and filled only from the worker's
//! configuration ([`Facts`]), sources cited and present, a length cap,
//! followups that exist and have a chip, and a stem with a generic end.
//! `coder-worker --check` runs it, and so do the tests.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::OnceLock;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::{RouteId, Screen};

/// The bank file, as compiled in.
pub const SOURCE: &str = include_str!("../../answers/chat-answers-v1.toml");

/// The repository path of [`SOURCE`], for the lint's messages.
pub const PATH: &str = "crates/coder/answers/chat-answers-v1.toml";

/// The longest text an entry shows, in characters.
pub const MAX_TEXT_CHARS: usize = 600;

/// The fact keys a slot may name. `worker.*` keys are filled by the worker
/// from its own configuration (see [`Facts::set`]); `gym.*` keys only by
/// [`super::gym::reply`], from a record the Gym seam verified, and
/// `capability.*` keys only by [`super::policy::decide`], from an entry
/// of the admitted-capability set, and `deck.*` keys only by
/// [`super::policy::decide`], from the decks the desktop app ships
/// ([`super::decks`]), and `engine.*` keys only by
/// [`super::policy::decide`], from the engine the typed `engine` reading
/// named (#10076), and `fanout.*` keys only by [`super::policy::decide`],
/// from the dispatch plan the typed `fanout` reading made (#10183), each
/// for an entry that sets [`Entry::records`].
/// `chat.*` keys are filled only by [`super::Context::facts`], from the
/// request's bounded `context`: the computer's name and the chat's project
/// folder, the person's own words and paths (#10077).
pub const FACT_KEYS: &[&str] = &[
    "worker.lane.display",
    "worker.door.display",
    "worker.recipients",
    "gym.tool",
    "gym.tests",
    "gym.plugins",
    "capability.name",
    "capability.line",
    "deck.title",
    "deck.list",
    "chat.computer",
    "chat.project",
    "chat.project_path",
    "engine.name",
    "fanout.doing",
    "fanout.engines",
];

/// The id suffix of an entry's variant for a chat on a computer: the entry
/// `meta.who` answers off a computer and `meta.who.here` on one
/// ([`Bank::placed`]).
pub const HERE_SUFFIX: &str = ".here";

/// The id suffix of an entry's variant for the desktop app: the entry
/// `meta.map` answers off it and `meta.map.desktop` in it, where its offer
/// can open the desktop's own screen (#10085).
pub const DESKTOP_SUFFIX: &str = ".desktop";

/// The id suffix of an entry's variant for the website's chat: the entry
/// `meta.who` answers in the apps and `meta.who.website` on openagents.com,
/// where nothing sends Coder to a computer and Coder is the terminal agent
/// a person installs. On the website a base with a `.website` variant is never
/// shown or offered; the variant is.
pub const WEB_SUFFIX: &str = ".website";

/// Where an entry may be shown (#10077).
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Place {
    /// On any surface.
    #[default]
    Any,
    /// Only in a chat on the computer Coder runs on (the request's
    /// `context.computer` is `here`): the desktop app, or `openagents chat`
    /// on a computer.
    Here,
    /// Only in a chat that is not on such a computer: a phone, or a client
    /// that does not say.
    Away,
    /// Only in the desktop app (the request's `context.surface` is
    /// `desktop`), for an offer only it can open.
    Desktop,
    /// Anywhere but the desktop app: the base of a `.desktop` variant.
    OffDesktop,
    /// Only in the website's chat (the request's `context.surface` is
    /// `web`): a `.website` variant ([`WEB_SUFFIX`]).
    Web,
}

impl Place {
    /// Whether an entry with this place may show in a chat that is, or is
    /// not, on a computer.
    #[must_use]
    pub fn admits(self, on_computer: bool) -> bool {
        self.admits_in(on_computer, false)
    }

    /// Whether an entry with this place may show in a chat that is, or is
    /// not, on a computer, and is, or is not, in the desktop app.
    #[must_use]
    pub fn admits_in(self, on_computer: bool, desktop: bool) -> bool {
        self.admits_on(on_computer, desktop, false)
    }

    /// Whether an entry with this place may show in a chat that is, or is
    /// not, on a computer, in the desktop app, and on the website.
    #[must_use]
    pub fn admits_on(self, on_computer: bool, desktop: bool, web: bool) -> bool {
        match self {
            Place::Any => true,
            Place::Here => on_computer,
            Place::Away => !on_computer,
            Place::Desktop => desktop,
            Place::OffDesktop => !desktop,
            Place::Web => web,
        }
    }
}

/// The routes whose entries must cite sources: every factual answer.
const SOURCED: &[RouteId] = &[
    RouteId::Meta,
    RouteId::Wallet,
    RouteId::Account,
    RouteId::ProductKb,
    RouteId::GymNews,
    RouteId::EvalRun,
    RouteId::EvalAuthor,
    RouteId::EvalCheck,
    RouteId::EvalResult,
    RouteId::EvalCredit,
];

/// An entry's offer, as the file writes it.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EntryOffer {
    /// Offer to dispatch Coder to the connected computer.
    #[serde(default)]
    pub run_coder: bool,
    /// Offer to open this screen.
    pub screen: Option<String>,
    /// The offer's button label: user-facing copy.
    pub label: String,
}

impl EntryOffer {
    /// The typed offer.
    #[must_use]
    pub fn offer(&self) -> Option<super::Offer> {
        match (
            self.run_coder,
            self.screen.as_deref().and_then(Screen::parse),
        ) {
            (true, None) => Some(super::Offer::RunCoder {
                label: self.label.clone(),
                engine: None,
                plan: super::DispatchPlan::default(),
            }),
            (false, Some(screen)) => Some(super::Offer::OpenScreen {
                screen,
                label: self.label.clone(),
            }),
            _ => None,
        }
    }
}

/// One prepared answer.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// The id; never reused.
    pub id: String,
    /// Bumped whenever the text, stem, or generic end changes.
    pub version: u32,
    /// The routes it answers, as route words.
    pub routes: Vec<String>,
    /// The messages it answers, written for Jev.
    pub when: String,
    /// What a neighboring entry answers instead, for Jev: the rubric's
    /// boundary.
    pub not_for: Option<String>,
    /// Messages it answers, from the labeled set's tune split only, for Jev.
    #[serde(default)]
    pub examples: Vec<String>,
    /// The whole answer, with `{slot}`s.
    pub text: Option<String>,
    /// A sentence start that a continuation or `generic_end` completes.
    pub stem: Option<String>,
    /// The stem's ending when personalization is off or fails.
    pub generic_end: Option<String>,
    /// The question a suggestion chip for this entry sends.
    pub chip: Option<String>,
    /// Each `{slot}` in the text, mapped to a [`FACT_KEYS`] key.
    #[serde(default)]
    pub facts: BTreeMap<String, String>,
    /// Where each factual claim comes from, as repository paths.
    #[serde(default)]
    pub sources: Vec<String>,
    /// Entries shown as suggestion chips under this answer.
    #[serde(default)]
    pub followups: Vec<String>,
    /// The action shown with the answer.
    pub offer: Option<EntryOffer>,
    /// The NIP-CJ verdict when this entry answers (`end_conversation`).
    pub verdict: Option<String>,
    /// The entry says something about what the Gym's records hold ("no
    /// result is waiting for a check"), so [`super::gym::reply`] picks it
    /// from the verified records, and the `answer` question never offers
    /// it.
    #[serde(default)]
    pub records: bool,
    /// The answer comes with the plugin catalog's cards
    /// (`docs/web/plugin-card.md`): its result carries the catalog's
    /// package slugs in `plugins`, and a surface that draws plugin cards
    /// draws them from its own copy of the catalog. The text then speaks
    /// of the cards' plugins without naming them all.
    #[serde(default)]
    pub plugins: bool,
    /// Where it may be shown: an entry whose words assume the chat is or
    /// is not on a computer says so, and its variant for the other place is
    /// `id.here` ([`HERE_SUFFIX`]).
    #[serde(default)]
    pub place: Place,
    /// Why the text may tell the reader to do something without an
    /// `https://` link, a `command`, or an offer: the rare exception to
    /// the rule that advice always carries the page or the one command to
    /// run ([`knowledge::product::unlinked_instruction`]).
    pub unlinked: Option<String>,
    /// The bank has this entry's `.website` variant, so the website shows that
    /// instead ([`Bank::parse`] sets it; never written in the file).
    #[serde(skip)]
    pub web_variant: bool,
}

impl Entry {
    /// `id@version`, as the wire names an answer.
    #[must_use]
    pub fn tag(&self) -> String {
        format!("{}@{}", self.id, self.version)
    }

    /// What the `answer` question reads for this entry: its `when` text
    /// alone, or a rubric `{what, not_for, examples}` when the entry draws
    /// a boundary or gives examples.
    #[must_use]
    pub fn criterion(&self) -> serde_json::Value {
        if self.not_for.is_none() && self.examples.is_empty() {
            return serde_json::Value::from(self.when.clone());
        }
        let examples: Vec<&str> = self.examples.iter().map(String::as_str).collect();
        super::rubric::option(&self.when, self.not_for.as_deref(), &examples)
    }

    /// Whether the entry answers `route`.
    #[must_use]
    pub fn answers(&self, route: RouteId) -> bool {
        route != RouteId::Unknown && self.routes.iter().any(|word| RouteId::parse(word) == route)
    }

    /// The whole text with its slots filled, or `None` when the entry has
    /// no text or a slot has no value.
    #[must_use]
    pub fn render(&self, facts: &Facts) -> Option<String> {
        fill(self.text.as_deref()?, &self.facts, facts)
    }

    /// The stem and its generic end, filled, or `None`.
    #[must_use]
    pub fn stem(&self, facts: &Facts) -> Option<(String, String)> {
        Some((
            fill(self.stem.as_deref()?, &self.facts, facts)?,
            fill(self.generic_end.as_deref()?, &self.facts, facts)?,
        ))
    }

    /// Whether the entry can be shown at all with these facts: in its
    /// place, with every slot filled.
    #[must_use]
    pub fn eligible(&self, facts: &Facts) -> bool {
        self.place
            .admits_on(facts.on_computer, facts.desktop, facts.web)
            && !(facts.web && self.web_variant)
            && (self.render(facts).is_some() || self.stem(facts).is_some())
    }

    /// Whether the `answer` question may offer it: eligible, and not an
    /// entry the Gym's records pick.
    #[must_use]
    pub fn selectable(&self, facts: &Facts) -> bool {
        !self.records && self.eligible(facts)
    }

    /// The typed offer, when the entry has one.
    #[must_use]
    pub fn offer(&self) -> Option<super::Offer> {
        self.offer.as_ref().and_then(EntryOffer::offer)
    }
}

fn fill(text: &str, slots: &BTreeMap<String, String>, facts: &Facts) -> Option<String> {
    let mut out = text.to_string();
    for (slot, key) in slots {
        let marker = format!("{{{slot}}}");
        if out.contains(&marker) {
            out = out.replace(&marker, facts.get(key)?);
        }
    }
    (!out.contains('{') && !out.contains('}')).then_some(out)
}

/// An opener: a first line above the model's reply.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Opener {
    pub id: String,
    pub text: String,
    pub when: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    bank: String,
    #[serde(default)]
    answer: Vec<Entry>,
    #[serde(default)]
    opener: Vec<Opener>,
}

/// A parsed bank.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bank {
    /// The bank's name (`chat-answers-v1`).
    pub name: String,
    /// The first 12 hex digits of the file's SHA-256.
    pub digest: String,
    pub answers: Vec<Entry>,
    pub openers: Vec<Opener>,
}

impl Bank {
    /// Parses a bank file.
    ///
    /// # Errors
    ///
    /// The TOML error, naming the line.
    pub fn parse(source: &str) -> Result<Self, String> {
        let file: File = toml::from_str(source).map_err(|error| format!("{PATH}: {error}"))?;
        let digest = Sha256::digest(source.as_bytes());
        let digest: String = digest.iter().take(6).map(|b| format!("{b:02x}")).collect();
        let mut answers = file.answer;
        let varied: Vec<String> = answers
            .iter()
            .filter_map(|entry| entry.id.strip_suffix(WEB_SUFFIX).map(str::to_string))
            .collect();
        for entry in &mut answers {
            entry.web_variant = varied.contains(&entry.id);
        }
        Ok(Self {
            name: file.bank,
            digest,
            answers,
            openers: file.opener,
        })
    }

    /// The compiled-in bank. It is linted by the tests and by
    /// `coder-worker --check`, so a bank that does not parse never ships.
    ///
    /// # Panics
    ///
    /// When [`SOURCE`] does not parse, which the tests rule out.
    #[must_use]
    pub fn builtin() -> &'static Bank {
        static BANK: OnceLock<Bank> = OnceLock::new();
        BANK.get_or_init(|| Bank::parse(SOURCE).expect("the compiled-in answer bank parses"))
    }

    /// `name@digest`, the bank's identity on the wire.
    #[must_use]
    pub fn id(&self) -> String {
        format!("{}@{}", self.name, self.digest)
    }

    /// The entry with `id`.
    #[must_use]
    pub fn entry(&self, id: &str) -> Option<&Entry> {
        self.answers.iter().find(|entry| entry.id == id)
    }

    /// The entry code picks by `id`, for the chat's place: its
    /// `id.here` variant in a chat on a computer, when it has one.
    #[must_use]
    pub fn placed(&self, id: &str, facts: &Facts) -> Option<&Entry> {
        facts
            .web
            .then(|| self.entry(&format!("{id}{WEB_SUFFIX}")))
            .flatten()
            .or_else(|| {
                facts
                    .desktop
                    .then(|| self.entry(&format!("{id}{DESKTOP_SUFFIX}")))
                    .flatten()
            })
            .or_else(|| {
                facts
                    .on_computer
                    .then(|| self.entry(&format!("{id}{HERE_SUFFIX}")))
                    .flatten()
            })
            .or_else(|| self.entry(id))
    }

    /// The opener with `id`.
    #[must_use]
    pub fn opener(&self, id: &str) -> Option<&Opener> {
        self.openers.iter().find(|opener| opener.id == id)
    }

    /// The chips to show under `entry`: `(id, chip)` for each followup that
    /// has one and is eligible with these facts.
    #[must_use]
    pub fn followups(&self, entry: &Entry, facts: &Facts) -> Vec<(String, String)> {
        entry
            .followups
            .iter()
            .filter_map(|id| self.placed(id, facts))
            .filter(|next| next.eligible(facts))
            .filter_map(|next| Some((next.id.clone(), next.chip.clone()?)))
            .collect()
    }
}

/// The values the bank's slots are filled from: the worker's own
/// configuration, never text someone typed once.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    values: BTreeMap<String, String>,
    /// The chat is on the computer Coder runs on, so entries placed
    /// `away` are not shown and `here` ones are.
    on_computer: bool,
    /// The chat is in the desktop app, so `desktop` entries are shown and
    /// `off_desktop` ones are not.
    desktop: bool,
    /// The chat is on the website, so `web` entries are shown and an entry
    /// with a `.website` variant is not.
    web: bool,
}

impl Facts {
    /// The same facts for a chat that is, or is not, on the computer Coder
    /// runs on.
    #[must_use]
    pub fn on_computer(mut self, here: bool) -> Self {
        self.on_computer = here;
        self
    }

    /// The same facts for a chat that is, or is not, in the desktop app.
    #[must_use]
    pub fn on_desktop(mut self, desktop: bool) -> Self {
        self.desktop = desktop;
        self
    }

    /// The same facts for a chat that is, or is not, on the website.
    #[must_use]
    pub fn on_web(mut self, web: bool) -> Self {
        self.web = web;
        self
    }

    /// Whether these facts are for a chat on the website.
    #[must_use]
    pub fn is_on_web(&self) -> bool {
        self.web
    }

    /// Whether these facts are for a chat in the desktop app.
    #[must_use]
    pub fn is_on_desktop(&self) -> bool {
        self.desktop
    }

    /// Whether these facts are for a chat on a computer.
    #[must_use]
    pub fn is_on_computer(&self) -> bool {
        self.on_computer
    }

    /// Sets `key` (one of [`FACT_KEYS`]) to `value`.
    ///
    /// # Panics
    ///
    /// When `key` is not a known fact key: a typo would otherwise leave
    /// every entry using it silently ineligible.
    #[must_use]
    pub fn set(mut self, key: &str, value: impl Into<String>) -> Self {
        assert!(FACT_KEYS.contains(&key), "unknown fact key {key}");
        self.values.insert(key.to_string(), value.into());
        self
    }

    /// The value of `key`, if the worker knows it.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }
}

fn singular(text: &str) -> Vec<String> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '\'' || c == '’'))
        .filter(|word| {
            let lower = word.to_lowercase().replace('’', "'");
            matches!(
                lower.as_str(),
                "i" | "i'll" | "i'm" | "i've" | "i'd" | "me" | "my" | "mine" | "myself"
            )
        })
        .map(str::to_string)
        .collect()
}

fn slots(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        let Some(end) = rest[start..].find('}') else {
            out.push(rest[start..].to_string());
            break;
        };
        out.push(rest[start + 1..start + end].to_string());
        rest = &rest[start + end + 1..];
    }
    out
}

/// Every rule `bank` breaks, as `id: problem` lines; empty when it is
/// clean. `root` is the repository root for the source check, or `None`
/// to skip it (a deployed worker has no checkout).
#[must_use]
pub fn lint(bank: &Bank, root: Option<&Path>) -> Vec<String> {
    let mut problems = Vec::new();
    let mut push = |id: &str, problem: String| problems.push(format!("{id}: {problem}"));
    let mut ids: Vec<&str> = Vec::new();
    for entry in &bank.answers {
        let id = entry.id.as_str();
        if ids.contains(&id) || bank.opener(id).is_some() {
            push(id, "the id is repeated".into());
        }
        ids.push(id);
        if entry.version == 0 {
            push(id, "version starts at 1".into());
        }
        if entry.when.trim().is_empty() {
            push(id, "when is empty".into());
        }
        let routes: Vec<RouteId> = entry
            .routes
            .iter()
            .map(|word| RouteId::parse(word))
            .collect();
        if routes.is_empty() || routes.contains(&RouteId::Unknown) {
            push(
                id,
                format!("routes {:?} name an unknown route", entry.routes),
            );
        }
        match (&entry.text, &entry.stem, &entry.generic_end) {
            (None, None, _) => push(id, "has neither text nor stem".into()),
            (_, Some(_), None) => push(id, "a stem needs a generic_end".into()),
            (_, None, Some(_)) => push(id, "a generic_end needs a stem".into()),
            _ => {}
        }
        let shown: Vec<&str> = [&entry.text, &entry.stem, &entry.generic_end]
            .into_iter()
            .flatten()
            .map(String::as_str)
            .collect();
        for text in &shown {
            let words = singular(text);
            if !words.is_empty() {
                push(id, format!("speaks in the singular ({})", words.join(", ")));
            }
            if text.chars().count() > MAX_TEXT_CHARS {
                push(id, format!("is longer than {MAX_TEXT_CHARS} characters"));
            }
            // The app shows the action itself, and may not show a control a
            // line names, so no line names one.
            let lower = text.to_lowercase();
            for control in ["run coder", "button", "click", "tap ", "tap."] {
                if lower.contains(control) {
                    push(id, format!("names a control (`{control}`)"));
                }
            }
            // Advice to do something carries the exact page or the one
            // command to run, or the offer that does it (2026-10-09).
            if entry.offer.is_none()
                && entry.unlinked.is_none()
                && let Some(step) = knowledge::product::unlinked_instruction(text)
            {
                push(
                    id,
                    format!(
                        "tells the reader to act (\"{step}\") with no https:// link, \
                         `command`, or offer; add the page or command, or say why in unlinked"
                    ),
                );
            }
            if let Some(offer) = &entry.offer
                && text.contains(offer.label.as_str())
            {
                push(id, format!("names its own offer `{}`", offer.label));
            }
            for slot in slots(text) {
                match entry.facts.get(&slot) {
                    None => push(id, format!("uses the undeclared slot {{{slot}}}")),
                    Some(key) if !FACT_KEYS.contains(&key.as_str()) => {
                        push(id, format!("maps {{{slot}}} to the unknown fact {key}"));
                    }
                    Some(_) => {}
                }
            }
        }
        // No machine talk in anything a person reads (#11031).
        let read = shown
            .iter()
            .copied()
            .chain(entry.chip.as_deref())
            .chain(entry.offer.as_ref().map(|offer| offer.label.as_str()));
        for text in read {
            for hit in oa_copy::violations(text, &[]) {
                push(
                    id,
                    format!("machine talk ({:?} in \"{}\")", hit.term, hit.context),
                );
            }
        }
        for slot in entry.facts.keys() {
            if !shown
                .iter()
                .any(|text| text.contains(&format!("{{{slot}}}")))
            {
                push(id, format!("declares the unused slot {slot}"));
            }
        }
        if routes.iter().any(|route| SOURCED.contains(route)) && entry.sources.is_empty() {
            push(id, "cites no sources".into());
        }
        if let Some(root) = root {
            for source in &entry.sources {
                if !root.join(source).exists() {
                    push(id, format!("cites {source}, which does not exist"));
                }
            }
        }
        for next in &entry.followups {
            match bank.entry(next) {
                None => push(id, format!("follows up with the unknown entry {next}")),
                Some(next) if next.chip.is_none() => {
                    push(
                        id,
                        format!("follows up with {}, which has no chip", next.id),
                    );
                }
                Some(_) => {}
            }
        }
        if let Some(offer) = &entry.offer {
            if offer.offer().is_none() {
                push(
                    id,
                    "the offer needs exactly one of run_coder or a known screen".into(),
                );
            }
            if offer.label.trim().is_empty() {
                push(id, "the offer has no label".into());
            }
        }
        if let Some(verdict) = &entry.verdict
            && verdict != "end_conversation"
        {
            push(id, format!("the verdict {verdict} is not end_conversation"));
        }
        // A records entry is picked by code: from the Gym's records on a
        // Gym or eval route, from the admitted-capability set on
        // `capability.missing` or a dispatch stem that names a capability
        // or the engine the person asked for, from the deck list on
        // `presentation.open`, or by surface on `standing.rule`.
        let capability_slot = entry.facts.values().any(|key| {
            key.starts_with("capability.")
                || key.starts_with("engine.")
                || key.starts_with("fanout.")
        });
        let picked_by_code = routes.iter().all(|route| {
            route.is_gym()
                || *route == RouteId::CapabilityMissing
                || *route == RouteId::PresentationOpen
                || *route == RouteId::StandingRule
                || (*route == RouteId::WorkDispatch && capability_slot)
        });
        // A `.here` variant is shown only on a computer, beside its base,
        // which is shown only away from one; `chat.*` slots fill only from
        // a computer or a paired phone's context, so they need a place.
        if let Some(base) = id.strip_suffix(HERE_SUFFIX) {
            if entry.place != Place::Here {
                push(id, "a .here variant needs place = \"here\"".into());
            }
            match bank.entry(base) {
                None => push(id, format!("varies the unknown entry {base}")),
                Some(base) if base.place != Place::Away => {
                    push(
                        id,
                        format!("varies {}, which needs place = \"away\"", base.id),
                    );
                }
                Some(_) => {}
            }
        }
        // A `.desktop` variant is shown only in the desktop app, beside its
        // base, which is shown everywhere else.
        if let Some(base) = id.strip_suffix(DESKTOP_SUFFIX) {
            if entry.place != Place::Desktop {
                push(id, "a .desktop variant needs place = \"desktop\"".into());
            }
            match bank.entry(base) {
                None => push(id, format!("varies the unknown entry {base}")),
                Some(base) if base.place != Place::OffDesktop => {
                    push(
                        id,
                        format!("varies {}, which needs place = \"off_desktop\"", base.id),
                    );
                }
                Some(_) => {}
            }
        }
        // A `.website` variant is shown only on the website, in place of its
        // base, which keeps its own place everywhere else.
        if let Some(base) = id.strip_suffix(WEB_SUFFIX) {
            if entry.place != Place::Web {
                push(id, "a .website variant needs place = \"web\"".into());
            }
            if bank.entry(base).is_none() {
                push(id, format!("varies the unknown entry {base}"));
            }
        } else if entry.place == Place::Web {
            push(id, "place = \"web\" is for a .website variant".into());
        }
        if entry.facts.values().any(|key| key.starts_with("chat.")) && entry.place == Place::Any {
            push(id, "a chat.* slot needs a place".into());
        }
        if entry.records && !picked_by_code {
            push(
                id,
                "a records entry answers Gym and eval routes, capability.missing, \
                 presentation.open, standing.rule, or a dispatch stem with a capability or \
                 engine slot only"
                    .into(),
            );
        }
        if !entry.records
            && (capability_slot
                || entry
                    .facts
                    .values()
                    .any(|key| key.starts_with("gym.") || key.starts_with("deck.")))
        {
            push(
                id,
                "only a records entry fills a slot from the Gym's records, the admitted \
                 capabilities, the engine reading, or the deck list"
                    .into(),
            );
        }
    }
    for opener in &bank.openers {
        let id = opener.id.as_str();
        if ids.contains(&id) {
            push(id, "the id is repeated".into());
        }
        ids.push(id);
        if !singular(&opener.text).is_empty() {
            push(id, "speaks in the singular".into());
        }
        for hit in oa_copy::violations(&opener.text, &[]) {
            push(
                id,
                format!("machine talk ({:?} in \"{}\")", hit.term, hit.context),
            );
        }
        // An opener says something true and useful; a bare
        // acknowledgement is filler.
        if opener.text.split_whitespace().count() < 3 {
            push(id, "is filler".into());
        }
        if opener.id == "none" {
            push(id, "`none` is reserved".into());
        }
    }
    if bank.answers.iter().any(|entry| entry.id == "none") {
        problems.push("none: `none` is reserved".into());
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    /// The shipped bank is clean: plural, no control named, slots declared
    /// and known, sources present, followups with chips.
    #[test]
    fn the_shipped_bank_passes_its_lint() {
        let bank = Bank::builtin();
        assert_eq!(bank.name, "chat-answers-v1");
        assert_eq!(bank.digest.len(), 12);
        let problems = lint(bank, Some(&root()));
        assert!(problems.is_empty(), "{}", problems.join("\n"));
        // The chat-answers-v1 entries that shipped in coder::first are
        // here, at the versions that shipped, so logged tags still resolve.
        for (id, version) in [
            ("meta.who", 4),
            ("meta.model", 3),
            ("meta.capabilities", 3),
            ("meta.limits_chat", 3),
            ("meta.coder", 3),
            ("meta.github", 3),
            ("meta.open_source", 1),
            ("smalltalk.hello", 1),
            ("smalltalk.how_are_you", 1),
            ("smalltalk.test", 1),
            ("smalltalk.thanks", 1),
            ("smalltalk.bye", 1),
        ] {
            assert_eq!(
                bank.entry(id).map(|entry| entry.version),
                Some(version),
                "{id}"
            );
        }
    }

    /// The lint catches each rule it enforces.
    #[test]
    fn the_lint_refuses_what_the_rules_forbid() {
        let bad = r#"
bank = "test"
[[answer]]
id = "meta.bad"
version = 1
routes = ["meta", "weather"]
when = "anything"
text = "I can help. Tap Run Coder. {price} {model} It was retained."
facts = { model = "worker.nope", unused = "worker.lane.display" }
sources = ["no/such/file.md"]
followups = ["meta.missing", "meta.nochip"]
offer = { screen = "nowhere", label = "Go" }

[[answer]]
id = "meta.nochip"
version = 1
routes = ["meta"]
when = "x"
stem = "We'll"

[[answer]]
id = "meta.unlinked"
version = 1
routes = ["meta"]
when = "x"
text = "Open OpenAgents for Mac and it shows a code."
sources = ["README.md"]

[[answer]]
id = "meta.excused"
version = 1
routes = ["meta"]
when = "x"
text = "Open the door."
unlinked = "a test of the exception"
sources = ["README.md"]

[[opener]]
id = "ok"
text = "Sure."
when = "x"
"#;
        let bank = Bank::parse(bad).unwrap();
        let problems = lint(&bank, Some(&root())).join("\n");
        assert!(!problems.contains("meta.excused"), "{problems}");
        for expected in [
            "name an unknown route",
            "speaks in the singular (I)",
            "names a control (`run coder`)",
            "undeclared slot {price}",
            "unknown fact worker.nope",
            "unused slot unused",
            "which does not exist",
            "unknown entry meta.missing",
            "meta.nochip, which has no chip",
            "exactly one of run_coder or a known screen",
            "a stem needs a generic_end",
            "meta.nochip: cites no sources",
            "ok: is filler",
            "machine talk (\"retained\"",
            "meta.unlinked: tells the reader to act",
        ] {
            assert!(
                problems.contains(expected),
                "missing `{expected}` in\n{problems}"
            );
        }
    }

    /// A slot the worker cannot fill makes its entry ineligible, never a
    /// blank; with the fact set, the text names it.
    #[test]
    fn unfilled_slots_make_an_entry_ineligible() {
        let bank = Bank::builtin();
        let privacy = bank.entry("meta.privacy").unwrap();
        assert!(!privacy.eligible(&Facts::default()));
        let facts = Facts::default().set("worker.recipients", "Example for a model");
        assert!(
            privacy
                .render(&facts)
                .unwrap()
                .contains("we send your messages to Example for a model.")
        );
        // No entry names a message limit: there is none (#10120).
        for entry in &bank.answers {
            let text = entry.text.as_deref().unwrap_or_default().to_lowercase();
            assert!(
                !text.contains("limit") && !text.contains("a day") && !text.contains("quota"),
                "{}: {text}",
                entry.id
            );
        }
        // Followups skip entries whose slots are unfilled.
        let who = bank.entry("meta.who").unwrap();
        let chips: Vec<String> = bank
            .followups(who, &Facts::default())
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        assert_eq!(chips, ["meta.model", "meta.codebase", "meta.plugins"]);
    }

    /// An entry whose words would be wrong on openagents.com (Coder sent
    /// to "a computer you connect") has a `.website` variant: on the website
    /// the variant is shown and offered instead, and elsewhere the base.
    #[test]
    fn web_variants_show_only_on_the_website() {
        let bank = Bank::builtin();
        let away = Facts::default();
        let web = Facts::default().on_web(true);
        for id in [
            "meta.who",
            "meta.capabilities",
            "meta.tools",
            "meta.limits_chat",
            "meta.coder",
            "meta.github",
        ] {
            let base = bank.entry(id).unwrap();
            let variant = bank.entry(&format!("{id}{WEB_SUFFIX}")).unwrap();
            assert!(base.eligible(&away) && !base.eligible(&web), "{id}");
            assert!(variant.eligible(&web) && !variant.eligible(&away), "{id}");
            assert!(variant.selectable(&web) && !base.selectable(&web), "{id}");
            assert_eq!(bank.placed(id, &web), Some(variant));
            assert_eq!(bank.placed(id, &away), Some(base));
            // The website sends Coder nowhere: Coder is the terminal agent
            // a person gets from the download page.
            let text = variant.render(&web).unwrap();
            assert!(text.contains("openagents.com/download"), "{id}: {text}");
            assert!(!text.contains("computer you"), "{id}: {text}");
        }
        // Followups follow the place too.
        let coder = bank.entry("meta.coder.website").unwrap();
        let chips = bank.followups(coder, &web);
        assert!(
            chips.iter().any(|(id, _)| id == "meta.github.website"),
            "{chips:?}"
        );
    }

    /// An entry whose words assume the chat is not on a computer has a
    /// `.here` variant, and each shows only in its place: code that picks
    /// `dispatch.no_computer` gets the variant on a computer, the `answer`
    /// question offers one of the pair, and followups follow the place
    /// (#10077).
    #[test]
    fn here_variants_show_only_on_a_computer() {
        let bank = Bank::builtin();
        let away = Facts::default();
        let here = Facts::default().on_computer(true);
        for id in [
            "meta.who",
            "meta.capabilities",
            "meta.coder",
            "meta.github",
            "dispatch.no_computer",
        ] {
            let base = bank.entry(id).unwrap();
            let variant = bank.entry(&format!("{id}{HERE_SUFFIX}")).unwrap();
            assert!(base.eligible(&away) && !base.eligible(&here), "{id}");
            assert!(variant.eligible(&here) && !variant.eligible(&away), "{id}");
            assert_eq!(bank.placed(id, &here), Some(variant));
            assert_eq!(bank.placed(id, &away), Some(base));
            let text = variant.render(&here).unwrap().to_lowercase();
            assert!(!text.contains("connect"), "{id}: {text}");
            assert!(variant.offer().is_none(), "{id}");
        }
        // The working-directory answer needs the project folder.
        let limits = bank.entry("meta.limits_chat.here").unwrap();
        assert!(!limits.eligible(&here));
        let placed = here
            .clone()
            .set("chat.project", "openagents")
            .set("chat.project_path", "/Users/someone/work/openagents");
        assert!(
            limits
                .render(&placed)
                .unwrap()
                .contains("the project folder openagents at /Users/someone/work/openagents")
        );
        // A chip never offers to connect a computer on one.
        let capabilities = bank.entry("meta.capabilities.here").unwrap();
        let chips: Vec<String> = bank
            .followups(capabilities, &placed)
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        assert_eq!(chips, ["meta.coder.here", "meta.limits_chat.here"]);
        let lint = |source: &str| lint(&Bank::parse(source).unwrap(), None).join("\n");
        let problems = lint(
            r#"
bank = "test"
[[answer]]
id = "meta.x"
version = 1
routes = ["smalltalk"]
when = "x"
text = "Hi."

[[answer]]
id = "meta.x.here"
version = 1
routes = ["smalltalk"]
when = "x"
text = "Hi from {project}."
facts = { project = "chat.project" }

[[answer]]
id = "meta.y.here"
version = 1
routes = ["smalltalk"]
place = "here"
when = "x"
text = "Hi."
"#,
        );
        for expected in [
            "meta.x.here: a .here variant needs place = \"here\"",
            "meta.x.here: varies meta.x, which needs place = \"away\"",
            "meta.x.here: a chat.* slot needs a place",
            "meta.y.here: varies the unknown entry meta.y",
        ] {
            assert!(
                problems.contains(expected),
                "missing `{expected}` in\n{problems}"
            );
        }
    }

    #[test]
    #[should_panic(expected = "unknown fact key")]
    fn an_unknown_fact_key_is_a_bug() {
        let _ = Facts::default().set("worker.typo", "x");
    }

    /// The route map's answer (#10085): in the desktop app its variant
    /// offers the map (`routes.map`); a phone or a terminal gets the line
    /// that says where it opens, with no offer.
    #[test]
    fn the_map_answer_offers_the_map_only_in_the_desktop_app() {
        let bank = Bank::builtin();
        let desktop = Facts::default().on_computer(true).on_desktop(true);
        let placed = bank.placed("meta.map", &desktop).unwrap();
        assert_eq!(placed.id, "meta.map.desktop");
        assert!(matches!(
            placed.offer(),
            Some(super::super::Offer::OpenScreen {
                screen: super::super::Screen::RoutesMap,
                ..
            })
        ));
        assert!(!bank.entry("meta.map").unwrap().eligible(&desktop));
        for facts in [Facts::default().on_computer(true), Facts::default()] {
            let placed = bank.placed("meta.map", &facts).unwrap();
            assert_eq!(placed.id, "meta.map");
            assert!(placed.offer().is_none());
            assert!(!bank.entry("meta.map.desktop").unwrap().eligible(&facts));
        }
    }
}
