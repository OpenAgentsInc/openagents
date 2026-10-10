//! The web chat's goldens: what we expect openagents.com's chat to do
//! when people ask what they actually ask, and the grading of what it did
//! (`bench/web-chat/goldens-v1.json`, `docs/web/chat-goldens.md`).
//!
//! A golden is one expectation: several phrasings of one question (casual
//! and misspelled ones too), the routes and prepared answers or product
//! notes that are right, the tier, the words a right reply must and must
//! never contain, and how fast it must come. Each phrasing is a [`Case`].
//!
//! Something sends each case and reports an [`Observed`]; [`grade`] checks
//! it. The `chat-goldens` binary sends cases three ways: through the
//! website's own chat endpoints (`http`, the path the live site uses),
//! through Jev and the router in this process (`router`), and not at all
//! (`check`, [`check`]: every accepted answer's own text meets its
//! golden, offline, for CI).
//!
//! Grading reads the reply's words only after the route is chosen, as
//! bounded checks on output: whether a required fact appears, whether a
//! wrong claim does, and machine talk ([`oa_copy`]). Nothing here routes.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use crate::router::{self, Bank, Facts};

/// The set's schema.
pub const SCHEMA: &str = "openagents.web-chat.goldens.v1";

/// The checked-in set, from the repository root.
pub const PATH: &str = "bench/web-chat/goldens-v1.json";

/// The checked-in set.
pub const FIXTURE: &str = include_str!("../../../bench/web-chat/goldens-v1.json");

/// The set.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Set {
    pub schema: String,
    pub set: String,
    pub created: String,
    pub surface: String,
    pub budgets: Budgets,
    /// Words no reply may contain, whatever was asked.
    #[serde(default)]
    pub forbidden: Vec<String>,
    /// The bar a run must clear to ship (#11106); none means every case.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate: Option<Gate>,
    pub flows: Vec<Flow>,
}

/// The launch bar (#11106): the least share of cases that must be right
/// (a [`Outcome::Slow`] case counts as right), and the flows and goldens
/// where no case may be wrong.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Gate {
    /// Between 0 and 1.
    pub min_right: f64,
    /// Flow or golden ids.
    #[serde(default)]
    pub critical: Vec<String>,
}

/// How fast each speed must be.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Budgets {
    pub instant: Budget,
    pub model: Budget,
}

/// One speed's budget, in milliseconds.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Budget {
    /// From sending to the first words of the reply on the page.
    pub first_ms: u64,
    /// From sending to the whole reply.
    pub total_ms: u64,
    /// Jev's judgment alone, in the `router` mode.
    pub judge_ms: u64,
}

/// One way people use the product.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Flow {
    pub id: String,
    pub title: String,
    pub goldens: Vec<Golden>,
}

/// One expectation.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Golden {
    pub id: String,
    /// Messages sent first in the same chat, each answered before the
    /// phrasing is sent.
    #[serde(default)]
    pub earlier: Vec<String>,
    pub phrasings: Vec<String>,
    /// The routes that are right; empty is any.
    #[serde(default)]
    pub route: Vec<String>,
    /// The prepared answers or product notes whose text is right; empty is
    /// any, or none.
    #[serde(default)]
    pub answers: Vec<String>,
    /// The served tiers that are right; empty is any.
    #[serde(default)]
    pub tier: Vec<String>,
    pub speed: Speed,
    /// Each a list of alternatives, one of which must appear.
    #[serde(default)]
    pub required: Vec<Vec<String>>,
    #[serde(default)]
    pub forbidden: Vec<String>,
    /// The sources every number and URL in a right reply must trace to
    /// (#11114): [`GROUNDING_SOURCES`] words. Empty is no trace check.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub grounded: Vec<String>,
    /// What the reply's components must hold (#11113); none checks nothing
    /// about components.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui: Option<UiExpect>,
    /// The chat's project's repository as the website reads it for the
    /// turn (`context.repository`): the golden is asked in a project chat.
    /// Only the router mode can send it; the website's visitor chat has no
    /// project, so `http` and `local` skip such a golden.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<RepositoryFixture>,
    /// The note the model must be told, when the tier is the model's:
    /// `repo` (answer from the chat's repository), `web` (the website's
    /// "Coder does that" note), or `none`. Empty checks nothing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub note: Vec<String>,
}

/// A project chat's repository for a golden, as `context.repository`
/// carries it.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RepositoryFixture {
    pub name: String,
    pub branch: String,
    pub snapshot: String,
}

impl RepositoryFixture {
    /// The context's `repository` object.
    #[must_use]
    pub fn json(&self) -> serde_json::Value {
        serde_json::json!({"name": self.name, "branch": self.branch, "snapshot": self.snapshot})
    }
}

/// The sources a golden's `grounded` may name: the public rate card
/// (prices and model names).
pub const GROUNDING_SOURCES: [&str; 1] = [inference::grounded::source::RATE_CARD];

/// The ledger a golden's replies are traced against: one result per
/// source it names, or the first source no ledger can be built for.
///
/// # Errors
///
/// Returns a source that is not one of [`GROUNDING_SOURCES`].
pub fn grounding(golden: &Golden) -> Result<inference::grounded::Ledger, String> {
    let mut ledger = inference::grounded::Ledger::new();
    for source in &golden.grounded {
        match source.as_str() {
            inference::grounded::source::RATE_CARD => {
                ledger.record_rates(&router::grounding::rate_card());
            }
            other => return Err(other.to_string()),
        }
    }
    Ok(ledger)
}

/// The trace check (#11114): every number and URL in `text` comes from
/// one of the golden's [`Golden::grounded`] sources; skipped when it
/// names none.
#[must_use]
pub fn trace_check(golden: &Golden, text: &str) -> Check {
    if golden.grounded.is_empty() {
        return mk("grounded", Status::Skip, "no sources named");
    }
    let ledger = match grounding(golden) {
        Ok(ledger) => ledger,
        Err(source) => return mk("grounded", Status::Fail, format!("unknown source {source}")),
    };
    let untraced = inference::grounded::untraced(text, &ledger);
    if untraced.is_empty() {
        mk("grounded", Status::Pass, "")
    } else {
        mk(
            "grounded",
            Status::Fail,
            format!(
                "not from {}: {}",
                golden.grounded.join(", "),
                untraced
                    .iter()
                    .map(|found| format!("`{}`", found.text))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )
    }
}

/// A golden's expectations of the components a reply draws (#11113).
/// Every block a reply has must parse with no fixes (`ui_valid`), whether
/// or not this asks for any.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct UiExpect {
    /// Whether the reply must draw components; false lets a reply in prose
    /// alone pass.
    #[serde(default)]
    pub required: bool,
    /// Catalog component names that must appear.
    #[serde(default)]
    pub components: Vec<String>,
    /// Link targets a component must point to: site paths or URLs.
    #[serde(default)]
    pub links: Vec<String>,
    /// Commands or code a component must let the reader copy.
    #[serde(default)]
    pub commands: Vec<String>,
}

/// The components a reply drew, read from its source (#11113).
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct UiSeen {
    /// Catalog names, each once, in the order first drawn.
    pub components: Vec<String>,
    /// Link targets, absolute.
    pub links: Vec<String>,
    pub commands: Vec<String>,
    /// What the validator fixed or dropped in its blocks.
    pub problems: Vec<String>,
}

/// What `source`, a reply as written (Markdown with any ```` ```openui-lang ````
/// blocks), draws.
#[must_use]
pub fn ui_seen(source: &str) -> UiSeen {
    let mut seen = UiSeen::default();
    for segment in openui_lang::embed::segments(source) {
        let openui_lang::embed::Segment::Ui { source, closed } = segment else {
            continue;
        };
        if !closed {
            seen.problems.push("a block is not closed".into());
            continue;
        }
        let document = openui_lang::parse(source);
        seen.problems
            .extend(document.diagnostics.iter().map(|d| match &d.statement {
                Some(name) => format!("`{name}`: {}", d.message),
                None => d.message.clone(),
            }));
        let Some(root) = document.root else {
            seen.problems.push("a block draws nothing".into());
            continue;
        };
        openui_lang::embed::walk(&root, &mut |node| {
            let name = node.component();
            if !seen.components.iter().any(|c| c == name) {
                seen.components.push(name.to_owned());
            }
        });
        seen.links.extend(openui_lang::embed::links(&root));
        seen.commands.extend(openui_lang::embed::commands(&root));
    }
    seen
}

/// The component checks: `ui_valid` (every block parses clean) and `ui`
/// (what `expect` asks for is drawn). Skipped when the mode cannot see
/// the reply's source.
#[must_use]
pub fn ui_checks(expect: &UiExpect, seen: Option<&UiSeen>) -> Vec<Check> {
    let Some(seen) = seen else {
        return vec![mk("ui", Status::Skip, "not observed")];
    };
    let valid = if seen.problems.is_empty() {
        mk("ui_valid", Status::Pass, "")
    } else {
        mk("ui_valid", Status::Fail, seen.problems.join("; "))
    };
    if seen.components.is_empty() {
        let drawn = if expect.required {
            mk("ui", Status::Fail, "no components")
        } else {
            mk("ui", Status::Pass, "prose only")
        };
        return vec![valid, drawn];
    }
    let mut missing: Vec<String> = Vec::new();
    for name in &expect.components {
        if !seen.components.contains(name) {
            missing.push(format!("component {name}"));
        }
    }
    for link in &expect.links {
        let want = openui_lang::embed::absolute(link);
        if !seen.links.contains(&want) {
            missing.push(format!("link {link}"));
        }
    }
    for command in &expect.commands {
        if !seen.commands.iter().any(|c| c.trim() == command.trim()) {
            missing.push(format!("command `{command}`"));
        }
    }
    let drawn = if missing.is_empty() {
        mk("ui", Status::Pass, seen.components.join(", "))
    } else {
        mk(
            "ui",
            Status::Fail,
            format!("missing {}", missing.join("; ")),
        )
    };
    vec![valid, drawn]
}

/// Instant: Jev picks a prepared answer or note, shown at once. Model: the
/// chat model writes the reply.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Speed {
    Instant,
    Model,
}

/// One phrasing of one golden.
#[derive(Clone, Copy, Debug)]
pub struct Case<'a> {
    pub flow: &'a Flow,
    pub golden: &'a Golden,
    pub phrasing: &'a str,
    /// The phrasing's position in its golden.
    pub index: usize,
}

impl Case<'_> {
    /// `golden.id#index`.
    #[must_use]
    pub fn id(&self) -> String {
        format!("{}#{}", self.golden.id, self.index + 1)
    }
}

impl Set {
    /// Parses a set.
    ///
    /// # Errors
    ///
    /// The JSON error, or a schema that isn't [`SCHEMA`].
    pub fn parse(json: &str) -> Result<Self, String> {
        let set: Set = serde_json::from_str(json).map_err(|e| format!("goldens: {e}"))?;
        if set.schema != SCHEMA {
            return Err(format!("goldens: schema {}, not {SCHEMA}", set.schema));
        }
        Ok(set)
    }

    /// The checked-in set.
    ///
    /// # Panics
    ///
    /// When [`FIXTURE`] does not parse, which the tests rule out.
    #[must_use]
    pub fn fixture() -> Self {
        Self::parse(FIXTURE).expect("the checked-in goldens parse")
    }

    /// Every case, in order.
    #[must_use]
    pub fn cases(&self) -> Vec<Case<'_>> {
        self.flows
            .iter()
            .flat_map(|flow| {
                flow.goldens.iter().flat_map(move |golden| {
                    golden
                        .phrasings
                        .iter()
                        .enumerate()
                        .map(move |(index, phrasing)| Case {
                            flow,
                            golden,
                            phrasing,
                            index,
                        })
                })
            })
            .collect()
    }

    /// The budget for `speed`.
    #[must_use]
    pub fn budget(&self, speed: Speed) -> Budget {
        match speed {
            Speed::Instant => self.budgets.instant,
            Speed::Model => self.budgets.model,
        }
    }
}

/// What a system did with one case. `None` is not observed: the system
/// doesn't report it, and its check is skipped rather than failed.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct Observed {
    pub route: Option<String>,
    pub tier: Option<String>,
    /// The prepared answer or product note served whole, as `id` or
    /// `id@version`.
    pub answer: Option<String>,
    /// The reply as the person reads it.
    pub text: Option<String>,
    /// Sending to the first words, on the page.
    pub first_ms: Option<u64>,
    /// Sending to the whole reply.
    pub total_ms: Option<u64>,
    /// Jev's judgment (and a knowledge lookup) alone.
    pub judge_ms: Option<u64>,
    /// The case could not be run, or the reply was an error.
    pub error: Option<String>,
    /// What the system says about why it answered so, when it says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
    /// The components the reply drew, when the mode can read the reply as
    /// written ([`ui_seen`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui: Option<UiSeen>,
    /// The note the model was told on a model tier ([`Golden::note`]'s
    /// words), when the mode can see it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// A check's outcome.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pass,
    Fail,
    Skip,
}

/// One check of one case.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Check {
    pub name: String,
    pub status: Status,
    pub detail: String,
}

/// One graded case.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Grade {
    pub case: String,
    pub flow: String,
    pub golden: String,
    pub phrasing: String,
    pub pass: bool,
    pub checks: Vec<Check>,
    pub observed: Observed,
}

/// What a graded case means for the launch bar.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Every check passed.
    Pass,
    /// The person read a right reply, only not as fast as the golden wants
    /// it: every failed check is a time, or the reply was written by the
    /// model (from the product notes or not) instead of a prepared answer
    /// shown whole, and its text passed every text check (the required
    /// facts, the forbidden words, machine talk).
    Slow,
    /// A wrong, missing, or unchecked reply.
    Fail,
}

/// The time checks.
const TIMES: [&str; 3] = ["first_ms", "total_ms", "judge_ms"];

impl Grade {
    /// The checks that failed.
    pub fn failed(&self) -> impl Iterator<Item = &Check> {
        self.checks.iter().filter(|c| c.status == Status::Fail)
    }

    /// What this case means for the launch bar.
    #[must_use]
    pub fn outcome(&self) -> Outcome {
        if self.pass {
            return Outcome::Pass;
        }
        if self.failed().all(|c| TIMES.contains(&c.name.as_str())) {
            return Outcome::Slow;
        }
        let text_passed = ["required", "forbidden", "machine_talk"]
            .iter()
            .all(|name| {
                self.checks
                    .iter()
                    .any(|c| c.name == *name && c.status == Status::Pass)
            });
        let only_how = self.failed().all(|c| {
            TIMES.contains(&c.name.as_str())
                // A right reply in prose where components were expected
                // is slow, not wrong; a block that needed fixes is wrong.
                || ["route", "tier", "answer", "ui"].contains(&c.name.as_str())
        });
        if text_passed && only_how && self.observed.error.is_none() {
            Outcome::Slow
        } else {
            Outcome::Fail
        }
    }
}

/// `id@version` without the version.
#[must_use]
pub fn bare(answer: &str) -> &str {
    answer.split_once('@').map_or(answer, |(id, _)| id)
}

/// Case-insensitive, and blind to Markdown's code marks (the website
/// draws `` `coder login` `` as code, so the page's text has no
/// backticks) and to a link's `https://` (the page links
/// openagents.com/download either way).
fn contains(text: &str, needle: &str) -> bool {
    let plain = |s: &str| s.replace('`', "").replace("https://", "").to_lowercase();
    plain(text).contains(&plain(needle))
}

fn mk(name: &str, status: Status, detail: impl Into<String>) -> Check {
    Check {
        name: name.into(),
        status,
        detail: detail.into(),
    }
}

fn one_of(name: &str, expected: &[String], seen: Option<&str>) -> Check {
    match (expected.is_empty(), seen) {
        (true, _) => mk(name, Status::Pass, "any"),
        (false, None) => mk(name, Status::Skip, "not observed"),
        (false, Some(seen)) if expected.iter().any(|e| e == seen) => mk(name, Status::Pass, seen),
        (false, Some(seen)) => mk(
            name,
            Status::Fail,
            format!("{seen}; expected {}", expected.join(" or ")),
        ),
    }
}

/// The text checks every reply gets: the golden's required facts, its
/// and the set's forbidden words, and machine talk.
#[must_use]
pub fn text_checks(set: &Set, golden: &Golden, text: &str) -> Vec<Check> {
    let mut checks = Vec::new();
    let missing: Vec<String> = golden
        .required
        .iter()
        .filter(|any| !any.iter().any(|needle| contains(text, needle)))
        .map(|any| any.join(" | "))
        .collect();
    checks.push(if missing.is_empty() {
        mk("required", Status::Pass, "")
    } else {
        mk(
            "required",
            Status::Fail,
            format!("missing {}", missing.join("; ")),
        )
    });
    let said: Vec<&String> = golden
        .forbidden
        .iter()
        .chain(&set.forbidden)
        .filter(|needle| contains(text, needle))
        .collect();
    checks.push(if said.is_empty() {
        mk("forbidden", Status::Pass, "")
    } else {
        mk(
            "forbidden",
            Status::Fail,
            format!(
                "says {}",
                said.iter()
                    .map(|s| format!("`{s}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )
    });
    let talk = oa_copy::violations(text, &[]);
    checks.push(if talk.is_empty() {
        mk("machine_talk", Status::Pass, "")
    } else {
        mk(
            "machine_talk",
            Status::Fail,
            talk.iter()
                .map(|v| format!("`{}` in \"{}\"", v.term, v.context))
                .collect::<Vec<_>>()
                .join("; "),
        )
    });
    checks
}

fn within(name: &str, budget: u64, seen: Option<u64>) -> Check {
    match seen {
        None => mk(name, Status::Skip, "not observed"),
        Some(ms) if ms <= budget => mk(name, Status::Pass, format!("{ms} ms")),
        Some(ms) => mk(name, Status::Fail, format!("{ms} ms; budget {budget} ms")),
    }
}

/// Grades one case.
#[must_use]
pub fn grade(set: &Set, case: &Case<'_>, observed: Observed) -> Grade {
    let golden = case.golden;
    let mut checks = Vec::new();
    if let Some(error) = &observed.error {
        checks.push(mk("error", Status::Fail, error.clone()));
    }
    // An instant golden must be served a listed answer whole; a model
    // golden names none.
    let answer = observed.answer.as_deref().map(bare);
    let listed = answer.is_some_and(|a| golden.answers.iter().any(|g| g == a));
    // The route is the router's reading on the way; a listed answer served
    // whole through another route (a knowledge note grounding a `general`
    // reading whose runner-up is `product.kb`) is still the right reply.
    checks.push(
        match one_of("route", &golden.route, observed.route.as_deref()) {
            c if c.status == Status::Fail && listed => mk(
                "route",
                Status::Pass,
                format!(
                    "{}, and a listed answer served",
                    observed.route.as_deref().unwrap_or("-")
                ),
            ),
            c => c,
        },
    );
    checks.push(one_of("tier", &golden.tier, observed.tier.as_deref()));
    if !golden.note.is_empty() {
        checks.push(one_of("note", &golden.note, observed.note.as_deref()));
    }
    checks.push(match (golden.answers.is_empty(), answer, &observed.tier) {
        (true, _, _) => mk("answer", Status::Pass, "any"),
        (false, Some(_), _) => one_of("answer", &golden.answers, answer),
        // A refusal is the bank's, but the wire names a prepared answer
        // only for a canned tier.
        (false, None, Some(tier)) if tier != "canned" && golden.tier.contains(tier) => {
            mk("answer", Status::Pass, format!("served as {tier}"))
        }
        (false, None, Some(_)) => mk(
            "answer",
            Status::Fail,
            format!("none served; expected {}", golden.answers.join(" or ")),
        ),
        (false, None, None) => mk("answer", Status::Skip, "not observed"),
    });
    match &observed.text {
        Some(text) => {
            let text = readable(text);
            checks.extend(text_checks(set, golden, &text));
            checks.push(trace_check(golden, &text));
        }
        None => checks.push(mk("text", Status::Skip, "no reply text in this mode")),
    }
    if let Some(expect) = &golden.ui {
        checks.extend(ui_checks(expect, observed.ui.as_ref()));
    }
    let budget = set.budget(golden.speed);
    checks.push(within("first_ms", budget.first_ms, observed.first_ms));
    checks.push(within("total_ms", budget.total_ms, observed.total_ms));
    checks.push(within("judge_ms", budget.judge_ms, observed.judge_ms));
    let pass = checks.iter().all(|c| c.status != Status::Fail);
    Grade {
        case: case.id(),
        flow: case.flow.id.clone(),
        golden: golden.id.clone(),
        phrasing: case.phrasing.to_string(),
        pass,
        checks,
        observed,
    }
}

/// Every problem with `set` against the shipped answers, offline: its
/// shape (unique ids, known routes and tiers), every listed answer
/// existing in `bank` (as the website places it) or among `notes`
/// (`id` to its served answer text), and every listed answer's own text
/// meeting its golden. A golden whose answers can't all be right is a
/// golden or a knowledge bug, caught before anything is sent.
#[must_use]
pub fn check(
    set: &Set,
    bank: &Bank,
    facts: &Facts,
    notes: &BTreeMap<String, String>,
) -> Vec<String> {
    let mut problems = Vec::new();
    let routes: BTreeSet<&str> = crate::router_eval::ROUTES.iter().copied().collect();
    let tiers = [
        "canned", "stem", "grounded", "model", "opener", "offer", "refuse", "gym", "author", "cli",
    ];
    let mut ids = BTreeSet::new();
    let plugin_names: Vec<String> = crate::builtin_plugins::names();
    for flow in &set.flows {
        for golden in &flow.goldens {
            let at = &golden.id;
            if !ids.insert(golden.id.clone()) {
                problems.push(format!("{at}: the id is used twice"));
            }
            if !golden.id.starts_with(&format!("{}.", flow.id)) {
                problems.push(format!(
                    "{at}: the id doesn't start with its flow, {}",
                    flow.id
                ));
            }
            if golden.phrasings.is_empty() {
                problems.push(format!("{at}: no phrasings"));
            }
            for route in &golden.route {
                if !routes.contains(route.as_str()) {
                    problems.push(format!("{at}: unknown route {route}"));
                }
            }
            for tier in &golden.tier {
                if !tiers.contains(&tier.as_str()) {
                    problems.push(format!("{at}: unknown tier {tier}"));
                }
            }
            if let Err(source) = grounding(golden) {
                problems.push(format!("{at}: unknown grounding source {source}"));
            }
            if golden.speed == Speed::Instant && golden.answers.is_empty() {
                problems.push(format!(
                    "{at}: an instant golden lists the answers that are right"
                ));
            }
            if let Some(expect) = &golden.ui {
                for name in &expect.components {
                    if openui_lang::catalog::component(name).is_none() {
                        problems.push(format!(
                            "{at}: ui names {name}, which is not in the catalog"
                        ));
                    }
                }
                for link in &expect.links {
                    if !openui_lang::safe_href(link) {
                        problems.push(format!(
                            "{at}: ui link {link} is not a link a component may have"
                        ));
                    }
                }
            }
            for id in &golden.answers {
                let text = match (bank.entry(id), notes.get(id)) {
                    (Some(entry), _) => {
                        if !entry.eligible(facts) {
                            problems.push(format!(
                                "{at}: {id} is never shown on the website (its place, or a slot \
                                 the worker can't fill)"
                            ));
                            continue;
                        }
                        let Some(mut text) = entry.render(facts) else {
                            problems.push(format!("{at}: {id} has no whole text"));
                            continue;
                        };
                        // The website draws the catalog's cards under it.
                        if entry.plugins {
                            text = format!("{text}\n{}", plugin_names.join("\n"));
                        }
                        text
                    }
                    (None, Some(text)) => text.clone(),
                    (None, None) => {
                        problems.push(format!(
                            "{at}: {id} is neither a bank answer nor a product note"
                        ));
                        continue;
                    }
                };
                let ui = golden
                    .ui
                    .as_ref()
                    .map(|expect| ui_checks(expect, Some(&ui_seen(&text))))
                    .unwrap_or_default();
                let text = readable(&text);
                for failed in text_checks(set, golden, &text)
                    .into_iter()
                    .chain([trace_check(golden, &text)])
                    .chain(ui)
                    .filter(|c| c.status == Status::Fail)
                {
                    problems.push(format!(
                        "{at}: {id}'s text fails {}: {}",
                        failed.name, failed.detail
                    ));
                }
            }
        }
    }
    if let Some(gate) = &set.gate {
        if !(gate.min_right > 0.0 && gate.min_right <= 1.0) {
            problems.push(format!(
                "gate: min_right {} is not in (0, 1]",
                gate.min_right
            ));
        }
        for id in &gate.critical {
            let known = set
                .flows
                .iter()
                .any(|f| f.id == *id || f.goldens.iter().any(|g| g.id == *id));
            if !known {
                problems.push(format!("gate: {id} is neither a flow nor a golden"));
            }
        }
    }
    problems
}

/// The served answers of the product notes, by id, for [`check`].
#[must_use]
pub fn note_answers(corpus: &knowledge::product::Corpus) -> BTreeMap<String, String> {
    corpus
        .base
        .entries
        .iter()
        .filter_map(|entry| Some((entry.id.clone(), crate::product_kb::served(entry)?)))
        .collect()
}

/// An answer's text as the checks read it: each component block replaced
/// by its Markdown ([`openui_lang::embed::fallback`]), so its buttons'
/// links, its commands, and its steps are read like the prose.
#[must_use]
pub fn readable(text: &str) -> String {
    openui_lang::embed::fallback(text)
}

/// The website's bank facts: the worker's own, on the website.
#[must_use]
pub fn web_facts(base: &Facts) -> Facts {
    router::Context {
        surface: Some(router::Surface::Web),
        ..router::Context::default()
    }
    .facts(base)
}

/// A run's grades and numbers.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Report {
    pub set: String,
    /// `http`, `router`, or `check`.
    pub mode: String,
    /// What was asked: the base URL, or the judge.
    pub target: String,
    pub started_unix: u64,
    pub cases: usize,
    pub passed: usize,
    /// Right replies over budget ([`Outcome::Slow`]).
    #[serde(default)]
    pub slow: usize,
    /// The launch bar's verdict, when the set has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate: Option<GateResult>,
    pub flows: BTreeMap<String, FlowScore>,
    /// Instant cases' first-answer times, when observed.
    pub instant_first_ms: Percentiles,
    pub model_first_ms: Percentiles,
    pub grades: Vec<Grade>,
}

/// A run against the launch bar.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GateResult {
    pub min_right: f64,
    /// Passing and slow cases over all cases.
    pub right: f64,
    /// Failed cases in a critical flow or golden.
    pub critical_failures: Vec<String>,
    pub met: bool,
}

impl GateResult {
    /// `gate` over `grades`.
    #[must_use]
    pub fn of(gate: &Gate, grades: &[Grade]) -> Self {
        let failed: Vec<&Grade> = grades
            .iter()
            .filter(|g| g.outcome() == Outcome::Fail)
            .collect();
        let right = if grades.is_empty() {
            0.0
        } else {
            (grades.len() - failed.len()) as f64 / grades.len() as f64
        };
        let critical_failures: Vec<String> = failed
            .iter()
            .filter(|g| gate.critical.iter().any(|c| *c == g.flow || *c == g.golden))
            .map(|g| g.case.clone())
            .collect();
        GateResult {
            min_right: gate.min_right,
            right,
            met: !grades.is_empty() && right >= gate.min_right && critical_failures.is_empty(),
            critical_failures,
        }
    }
}

/// One flow's numbers.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct FlowScore {
    pub cases: usize,
    pub passed: usize,
}

/// Median and 90th percentile, in milliseconds.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Percentiles {
    pub n: usize,
    pub p50: Option<u64>,
    pub p90: Option<u64>,
    pub max: Option<u64>,
}

impl Percentiles {
    fn of(mut values: Vec<u64>) -> Self {
        values.sort_unstable();
        let at = |q: f64| {
            (!values.is_empty()).then(|| {
                let index = ((values.len() as f64 - 1.0) * q).round() as usize;
                values[index]
            })
        };
        Percentiles {
            n: values.len(),
            p50: at(0.5),
            p90: at(0.9),
            max: values.last().copied(),
        }
    }
}

impl Report {
    /// The report of `grades` from `set`.
    #[must_use]
    pub fn of(set: &Set, mode: &str, target: &str, started_unix: u64, grades: Vec<Grade>) -> Self {
        let mut flows: BTreeMap<String, FlowScore> = BTreeMap::new();
        for grade in &grades {
            let score = flows.entry(grade.flow.clone()).or_default();
            score.cases += 1;
            score.passed += usize::from(grade.pass);
        }
        let speed_of = |golden: &str| {
            set.flows
                .iter()
                .flat_map(|f| &f.goldens)
                .find(|g| g.id == golden)
                .map(|g| g.speed)
        };
        let firsts = |speed: Speed| {
            Percentiles::of(
                grades
                    .iter()
                    .filter(|g| speed_of(&g.golden) == Some(speed))
                    .filter_map(|g| g.observed.first_ms.or(g.observed.judge_ms))
                    .collect(),
            )
        };
        Report {
            set: set.set.clone(),
            mode: mode.to_string(),
            target: target.to_string(),
            started_unix,
            cases: grades.len(),
            passed: grades.iter().filter(|g| g.pass).count(),
            slow: grades
                .iter()
                .filter(|g| g.outcome() == Outcome::Slow)
                .count(),
            gate: set.gate.as_ref().map(|gate| GateResult::of(gate, &grades)),
            flows,
            instant_first_ms: firsts(Speed::Instant),
            model_first_ms: firsts(Speed::Model),
            grades,
        }
    }

    /// The report as Markdown: the numbers, then every failure.
    #[must_use]
    pub fn markdown(&self) -> String {
        let mut out = String::new();
        let rate = |passed: usize, cases: usize| {
            if cases == 0 {
                0.0
            } else {
                100.0 * passed as f64 / cases as f64
            }
        };
        let _ = writeln!(out, "# Web chat goldens: {} ({})\n", self.set, self.mode);
        let _ = writeln!(out, "Target: `{}`\n", self.target);
        let _ = writeln!(
            out,
            "**{} of {} cases pass ({:.0} %).**\n",
            self.passed,
            self.cases,
            rate(self.passed, self.cases)
        );
        if let Some(gate) = &self.gate {
            let _ = writeln!(
                out,
                "**Launch bar {}:** {:.0} % right (passing, or right but slow; needs {:.0} %), \
                 {} wrong in a critical flow{}.\n",
                if gate.met { "met" } else { "NOT met" },
                100.0 * gate.right,
                100.0 * gate.min_right,
                gate.critical_failures.len(),
                if gate.critical_failures.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", gate.critical_failures.join(", "))
                },
            );
        }
        let ms = |p: &Percentiles| {
            format!(
                "n {}, p50 {}, p90 {}, max {}",
                p.n,
                p.p50.map_or("-".into(), |v| format!("{v} ms")),
                p.p90.map_or("-".into(), |v| format!("{v} ms")),
                p.max.map_or("-".into(), |v| format!("{v} ms")),
            )
        };
        let _ = writeln!(
            out,
            "- Instant answers, first words: {}",
            ms(&self.instant_first_ms)
        );
        let _ = writeln!(
            out,
            "- Model answers, first words: {}\n",
            ms(&self.model_first_ms)
        );
        let _ = writeln!(out, "| Flow | Pass | Cases |\n| --- | --- | --- |");
        for (flow, score) in &self.flows {
            let _ = writeln!(out, "| {flow} | {} | {} |", score.passed, score.cases);
        }
        for (heading, outcome) in [
            ("Failures", Outcome::Fail),
            ("Right but slow", Outcome::Slow),
        ] {
            let listed: Vec<&Grade> = self
                .grades
                .iter()
                .filter(|g| g.outcome() == outcome)
                .collect();
            if listed.is_empty() {
                continue;
            }
            let _ = writeln!(out, "\n## {heading}\n");
            for grade in listed {
                let o = &grade.observed;
                let _ = writeln!(
                    out,
                    "- `{}` \"{}\": route {}, tier {}, answer {}",
                    grade.case,
                    grade.phrasing,
                    o.route.as_deref().unwrap_or("-"),
                    o.tier.as_deref().unwrap_or("-"),
                    o.answer.as_deref().unwrap_or("-"),
                );
                for c in grade.failed() {
                    let _ = writeln!(out, "  - {}: {}", c.name, c.detail);
                }
                if let Some(why) = &o.why {
                    let _ = writeln!(out, "  - why: {why}");
                }
                if let Some(text) = &o.text {
                    let short: String = text.chars().take(240).collect();
                    let _ = writeln!(out, "  - reply: {}", short.replace('\n', " "));
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set() -> Set {
        Set::fixture()
    }

    fn case<'a>(set: &'a Set, golden: &str) -> Case<'a> {
        set.cases()
            .into_iter()
            .find(|c| c.golden.id == golden)
            .unwrap()
    }

    #[test]
    fn the_set_parses_and_covers_every_flow() {
        let set = set();
        assert_eq!(set.surface, "web");
        let flows: Vec<&str> = set.flows.iter().map(|f| f.id.as_str()).collect();
        for flow in [
            "about",
            "github",
            "account",
            "coder",
            "environments",
            "claude_key",
            "pricing",
            "chats",
            "privacy",
            "plugins",
            "limits",
            "smalltalk",
            "general",
        ] {
            assert!(flows.contains(&flow), "{flow}");
        }
        assert!(set.cases().len() >= 80, "{}", set.cases().len());
    }

    #[test]
    fn a_right_instant_answer_passes() {
        let set = set();
        let case = case(&set, "github.connect_repo");
        let grade = grade(
            &set,
            &case,
            Observed {
                route: Some("meta".into()),
                tier: Some("canned".into()),
                answer: Some("meta.github.website@1".into()),
                text: Some(
                    "Sign in with GitHub, open Projects at https://openagents.com/projects, and connect GitHub."
                        .into(),
                ),
                first_ms: Some(900),
                total_ms: Some(900),
                ..Observed::default()
            },
        );
        assert!(grade.pass, "{:?}", grade.checks);
    }

    #[test]
    fn a_slow_model_reply_with_a_wrong_claim_fails_each_check() {
        let set = set();
        let case = case(&set, "privacy.training");
        let grade = grade(
            &set,
            &case,
            Observed {
                route: Some("general".into()),
                tier: Some("model".into()),
                answer: None,
                text: Some("We don't train on your chats. The projection is retained.".into()),
                first_ms: Some(7_000),
                total_ms: Some(9_000),
                ..Observed::default()
            },
        );
        assert!(!grade.pass);
        let failed: Vec<&str> = grade.failed().map(|c| c.name.as_str()).collect();
        for name in [
            "route",
            "tier",
            "answer",
            "required",
            "forbidden",
            "machine_talk",
            "first_ms",
            "total_ms",
        ] {
            assert!(failed.contains(&name), "{name}: {failed:?}");
        }
    }

    #[test]
    fn what_a_mode_does_not_observe_is_skipped_not_failed() {
        let set = set();
        let case = case(&set, "general.explain");
        let grade = grade(
            &set,
            &case,
            Observed {
                route: Some("general".into()),
                tier: Some("model".into()),
                judge_ms: Some(600),
                ..Observed::default()
            },
        );
        assert!(grade.pass, "{:?}", grade.checks);
        assert!(grade.checks.iter().any(|c| c.status == Status::Skip));
    }

    #[test]
    fn the_report_counts_by_flow_and_lists_failures() {
        let set = set();
        let good = grade(
            &set,
            &case(&set, "smalltalk.hello"),
            Observed {
                route: Some("smalltalk".into()),
                tier: Some("canned".into()),
                answer: Some("smalltalk.hello@1".into()),
                text: Some("Hi! We're OpenAgents.".into()),
                first_ms: Some(800),
                total_ms: Some(800),
                ..Observed::default()
            },
        );
        let bad = grade(
            &set,
            &case(&set, "pricing.plan"),
            Observed {
                error: Some("no answer".into()),
                ..Observed::default()
            },
        );
        let report = Report::of(&set, "http", "http://127.0.0.1:4301", 0, vec![good, bad]);
        assert_eq!((report.cases, report.passed), (2, 1));
        assert_eq!(report.flows["smalltalk"].passed, 1);
        let md = report.markdown();
        assert!(md.contains("1 of 2 cases pass"), "{md}");
        assert!(md.contains("pricing.plan#1"), "{md}");
    }

    const DRAWN: &str = "Install it.\n\n```openui-lang\nroot = Steps([install, signin])\ninstall = Step(\"Install Coder\", [Command(\"curl -fsSL https://openagents.com/cli/install.sh | bash\")])\nsignin = Step(\"Sign in\", [CodeBlock(\"coder login\"), Button(\"Approve sign-in\", \"/device\"), CodeBlock(\"/sync on\")])\n```\n";

    #[test]
    fn a_reply_is_read_for_the_components_it_draws() {
        let seen = ui_seen(DRAWN);
        assert_eq!(seen.components, ["Steps", "Command", "CodeBlock", "Button"]);
        assert_eq!(seen.links, ["https://openagents.com/device"]);
        assert!(seen.commands.iter().any(|c| c == "coder login"));
        assert!(seen.problems.is_empty(), "{:?}", seen.problems);

        let broken = ui_seen("```openui-lang\nroot = Stack([Bogus(\"x\")])\n```\n");
        assert!(!broken.problems.is_empty());
        let open = ui_seen("Here.\n\n```openui-lang\nroot = Text(\"half");
        assert_eq!(open.problems, ["a block is not closed"]);
        assert_eq!(ui_seen("Prose alone."), UiSeen::default());
    }

    #[test]
    fn the_ui_checks_ask_for_what_the_golden_names() {
        let expect = UiExpect {
            required: true,
            components: vec!["Steps".into(), "Command".into()],
            links: vec!["/device".into()],
            commands: vec!["coder login".into()],
        };
        let pass = ui_checks(&expect, Some(&ui_seen(DRAWN)));
        assert!(pass.iter().all(|c| c.status == Status::Pass), "{pass:?}");
        let prose = ui_checks(&expect, Some(&ui_seen("Run coder login.")));
        assert!(
            prose
                .iter()
                .any(|c| c.name == "ui" && c.status == Status::Fail)
        );
        let optional = UiExpect::default();
        let fine = ui_checks(&optional, Some(&ui_seen("Run coder login.")));
        assert!(fine.iter().all(|c| c.status == Status::Pass), "{fine:?}");
        let fewer = UiExpect {
            links: vec!["/settings/terminal".into()],
            ..expect.clone()
        };
        let missing = ui_checks(&fewer, Some(&ui_seen(DRAWN)));
        assert!(
            missing
                .iter()
                .any(|c| c.name == "ui" && c.detail.contains("/settings/terminal")),
            "{missing:?}"
        );
        assert_eq!(ui_checks(&expect, None)[0].status, Status::Skip);
    }

    #[test]
    fn a_golden_with_components_grades_them() {
        let set = set();
        let case = case(&set, "coder.login");
        assert!(case.golden.ui.is_some(), "coder.login expects components");
        let observed = |text: &str| Observed {
            route: Some("product.kb".into()),
            tier: Some("canned".into()),
            answer: Some("openagents.coder-sync@2".into()),
            text: Some(text.into()),
            ui: Some(ui_seen(text)),
            first_ms: Some(900),
            total_ms: Some(900),
            ..Observed::default()
        };
        let drawn = grade(&set, &case, observed(DRAWN));
        assert!(drawn.pass, "{:?}", drawn.checks);
        // A block that needed fixes is wrong, not slow.
        let broken = grade(
            &set,
            &case,
            observed(
                "Run coder login and approve it at https://openagents.com/device.\n\n```openui-lang\nroot = Stack([Bogus(\"x\")])\n```\n",
            ),
        );
        assert_eq!(broken.outcome(), Outcome::Fail, "{:?}", broken.checks);
        // Right words without the components are slow.
        let prose = grade(
            &set,
            &case,
            observed(
                "Run coder login, approve it at https://openagents.com/device, then /sync on.",
            ),
        );
        assert_eq!(prose.outcome(), Outcome::Slow, "{:?}", prose.checks);
    }

    #[test]
    fn a_right_reply_over_budget_is_slow_and_a_wrong_one_fails_the_bar() {
        let set = set();
        let grounded = |text: &str| Observed {
            route: Some("product.kb".into()),
            tier: Some("grounded".into()),
            text: Some(text.into()),
            first_ms: Some(7_000),
            total_ms: Some(8_000),
            ..Observed::default()
        };
        let case = case(&set, "coder.login");
        let right = grade(
            &set,
            &case,
            grounded("Run `coder login` and approve its code at https://openagents.com/device."),
        );
        assert_eq!(right.outcome(), Outcome::Slow, "{:?}", right.checks);
        let wrong = grade(&set, &case, grounded("Open the app's settings."));
        assert_eq!(wrong.outcome(), Outcome::Fail);
        // The page draws code without its backticks.
        let page = grade(
            &set,
            &case,
            Observed {
                route: Some("product.kb".into()),
                tier: Some("canned".into()),
                answer: Some("openagents.coder-sync@2".into()),
                text: Some(
                    "Run coder login and approve its code at https://openagents.com/device.".into(),
                ),
                first_ms: Some(900),
                total_ms: Some(900),
                ..Observed::default()
            },
        );
        assert!(page.pass, "{:?}", page.checks);
        let gate = set.gate.clone().expect("the set has a launch bar");
        assert!(GateResult::of(&gate, &[right.clone(), page.clone()]).met);
        let missed = GateResult::of(&gate, &[right, page, wrong]);
        assert!(!missed.met);
        assert_eq!(missed.critical_failures, vec!["coder.login#1".to_string()]);
    }

    /// A golden that names the rate card checks that every number and URL
    /// in the reply comes from it (#11114): the card's own price passes, a
    /// retyped one fails, and a golden naming no source skips the check.
    #[test]
    fn a_grounded_golden_traces_numbers_to_the_rate_card() {
        let set = set();
        let mut golden = case(&set, "pricing.cost").golden.clone();
        let skipped = trace_check(&golden, "It costs $9.99.");
        assert_eq!(skipped.status, Status::Skip);
        golden.grounded = vec!["rate_card".into()];
        let card = router::grounding::rate_card();
        let row = card
            .rows
            .iter()
            .find(|row| row.kind == inference::rates::Kind::List)
            .expect("a list row");
        let right = format!(
            "{} costs ${} per million input tokens.",
            row.model, row.input.price_usd
        );
        assert_eq!(trace_check(&golden, &right).status, Status::Pass);
        let retyped = trace_check(&golden, "It costs $987.65 per million input tokens.");
        assert_eq!(retyped.status, Status::Fail);
        assert!(retyped.detail.contains("`987.65`"), "{}", retyped.detail);
        // An unknown source is a golden bug, caught offline.
        golden.grounded = vec!["horoscope".into()];
        assert_eq!(trace_check(&golden, &right).status, Status::Fail);
        let mut broken = set.clone();
        broken.flows[0].goldens[0].grounded = vec!["horoscope".into()];
        let problems = check(
            &broken,
            Bank::builtin(),
            &web_facts(&crate::router::worker_facts(
                crate::generate::DEFAULT_MODEL,
                Some(crate::generate::DEFAULT_DOOR_URL),
                &crate::router::Seams::default(),
            )),
            &BTreeMap::new(),
        );
        assert!(
            problems
                .iter()
                .any(|problem| problem.contains("unknown grounding source horoscope")),
            "{problems:?}"
        );
    }
}
