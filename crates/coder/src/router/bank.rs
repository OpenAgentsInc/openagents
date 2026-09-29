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
/// of the admitted-capability set, each for an entry that sets
/// [`Entry::records`].
pub const FACT_KEYS: &[&str] = &[
    "worker.lane.display",
    "worker.door.display",
    "worker.quota.day",
    "worker.quota.minute",
    "worker.recipients",
    "gym.tool",
    "gym.tests",
    "capability.name",
    "capability.line",
];

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

    /// Whether the entry can be shown at all with these facts.
    #[must_use]
    pub fn eligible(&self, facts: &Facts) -> bool {
        self.render(facts).is_some() || self.stem(facts).is_some()
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
        Ok(Self {
            name: file.bank,
            digest,
            answers: file.answer,
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
            .filter_map(|id| self.entry(id))
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
}

impl Facts {
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
        // Gym or eval route, or from the admitted-capability set on
        // `capability.missing` or a dispatch stem that names a capability.
        let capability_slot = entry
            .facts
            .values()
            .any(|key| key.starts_with("capability."));
        let picked_by_code = routes.iter().all(|route| {
            route.is_gym()
                || *route == RouteId::CapabilityMissing
                || (*route == RouteId::WorkDispatch && capability_slot)
        });
        if entry.records && !picked_by_code {
            push(
                id,
                "a records entry answers Gym and eval routes, capability.missing, or a \
                 dispatch stem with a capability slot only"
                    .into(),
            );
        }
        if !entry.records
            && (capability_slot || entry.facts.values().any(|key| key.starts_with("gym.")))
        {
            push(
                id,
                "only a records entry fills a slot from the Gym's records or the admitted \
                 capabilities"
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
            ("meta.who", 1),
            ("meta.model", 1),
            ("meta.capabilities", 2),
            ("meta.limits_chat", 2),
            ("meta.coder", 2),
            ("meta.github", 1),
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
text = "I can help. Tap Run Coder. {price} {model}"
facts = { model = "worker.nope", unused = "worker.quota.day" }
sources = ["no/such/file.md"]
followups = ["meta.missing", "meta.nochip"]
offer = { screen = "nowhere", label = "Go" }

[[answer]]
id = "meta.nochip"
version = 1
routes = ["meta"]
when = "x"
stem = "We'll"

[[opener]]
id = "ok"
text = "Sure."
when = "x"
"#;
        let bank = Bank::parse(bad).unwrap();
        let problems = lint(&bank, Some(&root())).join("\n");
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
        let pricing = bank.entry("meta.pricing").unwrap();
        assert!(!pricing.eligible(&Facts::default()));
        let facts = Facts::default().set("worker.quota.day", "40");
        assert!(
            pricing
                .render(&facts)
                .unwrap()
                .starts_with("Chatting with us is free right now, up to 40 messages a day.")
        );
        // Followups skip entries whose slots are unfilled.
        let who = bank.entry("meta.who").unwrap();
        let chips: Vec<String> = bank
            .followups(who, &Facts::default())
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        assert_eq!(chips, ["meta.capabilities"]);
    }

    #[test]
    #[should_panic(expected = "unknown fact key")]
    fn an_unknown_fact_key_is_a_bug() {
        let _ = Facts::default().set("worker.typo", "x");
    }
}
