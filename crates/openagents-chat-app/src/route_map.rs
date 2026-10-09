//! The route map (#10085): OpenAgents' composition as one graph, the
//! visible form of the "agent of agents" in
//! `docs/essays/2026-10-01-the-return-of-the-general-agent.md`.
//!
//! The front (the chat router) decides where each request goes; its typed
//! routes are grouped in families; each route is served by members:
//! prepared answers, knowledge, the general chat model, Coder and its
//! engines, plugins (what people add: Wasm, workflows, skills, knowledge,
//! and tests in one package), and screens or actions. Edges follow a
//! request: front → route → what serves it → engine or plugin. Every node
//! has a [`Kind`], which the map colors and its legend names, and the
//! places where OpenAgents is thin are [`Gap`]s: typed facts computed from
//! the same sources, each with evidence and a next step.
//!
//! [`Map::build`] is deterministic and needs no network: the router's
//! sources ([`sources::Sources`]), the published records
//! ([`records::Records`]), and what only this computer knows
//! ([`Local`]: how often this person's own chats took each route, counted
//! from their saved judgments, and the engines' readiness). Local counts
//! never leave the device. The geometry is in [`layout`]; drawing is the
//! surface's.

pub mod layout;
pub mod records;
pub mod sources;

use std::collections::BTreeMap;

use openagents_connect::control::{EngineReport, RouteUsage};
use serde::Serialize;

use records::{Records, Verdict};
use sources::{PluginSource, Sources};

/// The repository the map's record links point into.
pub const REPOSITORY: &str = "https://github.com/OpenAgentsInc/openagents";

/// A route measured below this held-out precision is weak.
pub const PRECISION_FLOOR: f64 = 0.85;
/// A route measured below this held-out recall is weak.
pub const RECALL_FLOOR: f64 = 0.80;
/// The fewest scored rows on which a precision or recall reads as weak.
pub const MIN_SCORED: u64 = 5;
/// A route with fewer labeled rows than this has few examples.
pub const MIN_ROWS: u64 = 25;
/// This person's turns before their own clarify share counts.
pub const MIN_LOCAL_TURNS: u64 = 20;
/// The share of this person's turns that went to `clarify` past which the
/// router asks them back too often.
pub const CLARIFY_SHARE: f64 = 0.15;
/// The confirming checks by distinct trainers a result needs before it is
/// a candidate for Coder's defaults
/// (`docs/extensions/evaluation.md`, Checks, adoption, and credit).
pub const CHECKS_FOR_ADOPTION: u64 = 3;

/// What a node is. Each kind has its own color and a legend row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// The chat router: the front every request reaches first.
    Front,
    /// A group of routes.
    Family,
    /// One of the router's typed routes.
    Route,
    /// A prepared answer from the answer bank.
    Answer,
    /// A knowledge entry, or a body of them.
    Knowledge,
    /// The general chat model.
    Model,
    /// Coder, the coding agent work is handed to.
    Coder,
    /// An engine Coder delegates to.
    Engine,
    /// What a person adds: a plugin.
    Plugin,
    /// A screen, deck, or action a route opens.
    Screen,
}

impl Kind {
    /// Every kind, in the legend's order.
    pub const ALL: [Kind; 10] = [
        Kind::Front,
        Kind::Family,
        Kind::Route,
        Kind::Answer,
        Kind::Knowledge,
        Kind::Model,
        Kind::Coder,
        Kind::Engine,
        Kind::Plugin,
        Kind::Screen,
    ];

    /// The legend's words.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Kind::Front => "Router",
            Kind::Family => "Route family",
            Kind::Route => "Route",
            Kind::Answer => "Prepared answer",
            Kind::Knowledge => "Knowledge",
            Kind::Model => "Chat model",
            Kind::Coder => "Coder",
            Kind::Engine => "Engine",
            Kind::Plugin => "Plugin",
            Kind::Screen => "Screen or action",
        }
    }

    /// The kind's color in the scheme the app paints with
    /// ([`crate::visual::map::current`]).
    #[must_use]
    pub fn color(self) -> rust_native::style::Color {
        let map = crate::visual::map::current();
        match self {
            Kind::Front => map.front,
            Kind::Family => map.family,
            Kind::Route => map.route,
            Kind::Answer => map.answer,
            Kind::Knowledge => map.knowledge,
            Kind::Model => map.model,
            Kind::Coder => map.coder,
            Kind::Engine => map.engine,
            Kind::Plugin => map.plugin,
            Kind::Screen => map.screen,
        }
    }

    /// The word a filter and a test name it by.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Kind::Front => "front",
            Kind::Family => "family",
            Kind::Route => "route",
            Kind::Answer => "answer",
            Kind::Knowledge => "knowledge",
            Kind::Model => "model",
            Kind::Coder => "coder",
            Kind::Engine => "engine",
            Kind::Plugin => "plugin",
            Kind::Screen => "screen",
        }
    }
}

/// How well something is measured: drawn as the node's ring.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Health {
    /// Measured, and the numbers are good.
    Good,
    /// Measured, and the numbers are weak.
    Weak,
    /// Nothing measures it.
    Unmeasured,
}

impl Health {
    #[must_use]
    pub fn words(self) -> &'static str {
        match self {
            Health::Good => "Measured and good",
            Health::Weak => "Measured and weak",
            Health::Unmeasured => "Not measured",
        }
    }
}

/// Where a plugin stands on the Gym's ladder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// Code with no package record or tests yet.
    NotPackaged,
    /// Packaged, with no published result.
    Candidate,
    /// A published result, not yet checked by another trainer.
    Result(Verdict),
    /// A Better result that another trainer's check confirmed.
    Reproduced,
    /// A Better result confirmed and externally validated on a second
    /// test set.
    Validated,
    /// Adopted into Coder's defaults for everyone.
    Adopted,
}

impl Stage {
    #[must_use]
    pub fn words(self) -> &'static str {
        match self {
            Stage::NotPackaged => "Not packaged",
            Stage::Candidate => "Candidate",
            Stage::Result(verdict) => verdict.words(),
            Stage::Reproduced => "Reproduced",
            Stage::Validated => "Validated",
            Stage::Adopted => "Adopted",
        }
    }
}

/// What an edge says.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    /// The tree: the front reaches a family, a family holds a route.
    Groups,
    /// A route, or a member, is served by the target.
    ServedBy,
    /// Work is handed off: Coder to an engine.
    HandsOff,
    /// A route or an answer opens a screen.
    Opens,
    /// Coder can admit a plugin in its runs.
    Admits,
    /// A Gym route tests a plugin.
    Tests,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Edge {
    pub from: usize,
    pub to: usize,
    pub kind: EdgeKind,
}

/// A link to a record: a repository path, a URL, or a Nostr event id.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Link {
    pub label: String,
    pub target: Target,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum Target {
    /// A path in this repository.
    Path(String),
    /// A web address.
    Url(String),
    /// A Nostr event id on `relay.openagents.com`.
    Event(String),
}

impl Target {
    /// Where a person opens it: the repository file on GitHub, the URL, or
    /// the event on njump.
    #[must_use]
    pub fn url(&self) -> String {
        match self {
            Target::Path(path) => format!("{REPOSITORY}/blob/main/{path}"),
            Target::Url(url) => url.clone(),
            Target::Event(id) => format!("https://njump.me/{id}"),
        }
    }
}

/// A next step a gap offers. Each runs an existing typed path; none is
/// taken without the person's tap.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum NextStep {
    /// Start a chat with this message in the composer, unsent: the Gym's
    /// draft path (`eval.author`), a test (`eval.run`), or a check
    /// (`eval.check`), as the router reads it.
    Chat { label: String, message: String },
    /// Copy a command for a terminal, with the page that explains it.
    Command {
        label: String,
        command: String,
        doc: String,
    },
    /// Open a prefilled issue on GitHub.
    Issue { label: String, url: String },
    /// Open Settings' Coder page, where engines sign in.
    SignIn { label: String, engine: String },
}

impl NextStep {
    /// The button's words.
    #[must_use]
    pub fn label(&self) -> &str {
        match self {
            NextStep::Chat { label, .. }
            | NextStep::Command { label, .. }
            | NextStep::Issue { label, .. }
            | NextStep::SignIn { label, .. } => label,
        }
    }
}

/// What kind of gap it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GapKind {
    /// People ask for things no plugin serves (`capability.missing`).
    NoPlugin,
    /// An answers route served by the chat model alone.
    ModelOnly,
    /// Product questions with no knowledge entry.
    UnansweredQuestions,
    /// Held-out precision or recall below the floor.
    WeakRoute,
    /// Too few labeled rows.
    FewExamples,
    /// Not in the latest per-route record.
    RouteUnmeasured,
    /// The router asks this person back too often.
    FrequentClarify,
    /// A plugin with no package record or tests.
    NotPackaged,
    /// A packaged plugin with no published result.
    NoResult,
    /// The latest result didn't show the plugin helping.
    NoHelpYet,
    /// A Better result nobody has checked.
    NeedsCheck,
    /// A reproduced result with no second test set.
    NeedsValidation,
    /// Everything adoption needs except the operator's decision.
    ReadyToAdopt,
    /// An engine that isn't signed in.
    EngineSignedOut,
    /// An engine at its usage limit.
    EngineAtLimit,
}

impl GapKind {
    /// Every kind, in the Gaps panel's order.
    pub const ALL: [GapKind; 15] = [
        GapKind::NoPlugin,
        GapKind::ModelOnly,
        GapKind::UnansweredQuestions,
        GapKind::WeakRoute,
        GapKind::FewExamples,
        GapKind::RouteUnmeasured,
        GapKind::FrequentClarify,
        GapKind::NotPackaged,
        GapKind::NoResult,
        GapKind::NoHelpYet,
        GapKind::NeedsCheck,
        GapKind::NeedsValidation,
        GapKind::ReadyToAdopt,
        GapKind::EngineSignedOut,
        GapKind::EngineAtLimit,
    ];
}

/// A place OpenAgents is thin.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Gap {
    pub kind: GapKind,
    /// The node it marks.
    pub node: usize,
    /// One line, plain words.
    pub title: String,
    /// Why, with its numbers.
    pub detail: String,
    pub evidence: Vec<Link>,
    pub step: NextStep,
}

/// What only this computer knows. Never sent anywhere.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Local {
    /// This person's replies per route word, counted from their saved
    /// chats' typed judgments.
    pub routes: BTreeMap<String, u64>,
    /// This computer's engines, when its Coder reported them.
    pub engines: Option<EngineReport>,
}

impl Local {
    /// Every reply counted.
    #[must_use]
    pub fn turns(&self) -> u64 {
        self.routes.values().sum()
    }
}

/// One node.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Node {
    /// Stable across refreshes: `route:meta`, `plugin:crates/plugin-repo-map`.
    pub id: String,
    pub kind: Kind,
    pub label: String,
    /// What it is, in one line.
    pub line: String,
    /// Its parent in the map's tree: what it serves, or what holds it.
    pub parent: Option<usize>,
    pub depth: u8,
    /// How big it draws, from 0 to 1: traffic or labeled rows for a route.
    pub weight: f32,
    pub health: Health,
    /// A plugin's place on the Gym's ladder.
    pub stage: Option<Stage>,
    /// The route family it belongs to.
    pub family: Option<String>,
    /// The gaps that mark it, by index into [`Map::gaps`].
    pub gaps: Vec<usize>,
    /// This person's replies on this route, counted locally.
    pub local: Option<u64>,
    /// The plugin shown as the example to copy.
    pub showcase: bool,
}

/// The whole map.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Map {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub gaps: Vec<Gap>,
    /// The question set and bank the map was built from.
    pub set: String,
    pub bank: String,
    #[serde(skip)]
    sources: Sources,
    #[serde(skip)]
    records: Records,
    #[serde(skip)]
    local: Local,
}

/// A field of the inspector: a label and its value, with a record link.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Field {
    pub label: String,
    pub value: String,
    pub link: Option<Link>,
}

impl Field {
    fn new(label: &str, value: impl Into<String>) -> Self {
        Field {
            label: label.to_string(),
            value: value.into(),
            link: None,
        }
    }

    fn linked(label: &str, value: impl Into<String>, link: Link) -> Self {
        Field {
            label: label.to_string(),
            value: value.into(),
            link: Some(link),
        }
    }
}

/// What the inspector shows for a node.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Inspector {
    pub node: usize,
    pub title: String,
    pub kind: Kind,
    /// What it is.
    pub line: String,
    /// Why the router sends things here, for a route.
    pub why: Option<String>,
    /// Facts and numbers, each with its record when it has one.
    pub fields: Vec<Field>,
    /// What serves it, or what it holds.
    pub members: Vec<(usize, String)>,
    /// Its gaps, by index.
    pub gaps: Vec<usize>,
    /// The next steps its gaps offer, and for the showcase, "make one like
    /// this".
    pub steps: Vec<NextStep>,
}

fn path(label: &str, path: &str) -> Link {
    Link {
        label: label.to_string(),
        target: Target::Path(path.to_string()),
    }
}

fn event(label: &str, id: &str) -> Link {
    Link {
        label: label.to_string(),
        target: Target::Event(id.to_string()),
    }
}

/// `0.91`, as a percent.
fn percent(value: f64) -> String {
    format!("{:.0}%", value * 100.0)
}

/// A query string value: unreserved characters kept, everything else
/// percent-encoded.
fn query(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// The route family's words on screen.
#[must_use]
pub fn family_label(word: &str) -> &'static str {
    match word {
        "answers" => "Answers",
        "work" => "Work",
        "gym" => "The Gym",
        "screens" => "Screens and actions",
        "boundaries" => "Boundaries",
        _ => "Other",
    }
}

/// The family words in the map's order.
pub const FAMILIES: [&str; 5] = ["answers", "work", "gym", "screens", "boundaries"];

/// What a screen word opens, in plain words.
#[must_use]
pub fn screen_label(word: &str) -> String {
    match word {
        "wallet" => "Wallet".into(),
        "account.computers" => "Computers".into(),
        "account.keys" => "Identity keys".into(),
        "account.playtest" => "Playtest".into(),
        "account.report_problem" => "Report a problem".into(),
        "verse.gym" => "The Gym's EVALS board".into(),
        "gym.result" => "Your latest result".into(),
        "gym.publish" => "Add to the Gym".into(),
        "gym.test_set" => "The test set".into(),
        other => other.to_string(),
    }
}

/// A plugin's place on the ladder, from its records.
#[must_use]
pub fn stage_of(plugin: &PluginSource, records: &Records) -> Stage {
    let (Some(slug), Some(publisher)) = (&plugin.slug, &plugin.publisher) else {
        return Stage::NotPackaged;
    };
    let package = format!("{publisher}:{slug}");
    if !records.adoptions_of(&package).is_empty() {
        return Stage::Adopted;
    }
    let results = records.of_package(&package);
    let originals: Vec<_> = results.iter().filter(|r| r.original()).collect();
    let Some(latest) = originals.first() else {
        return Stage::Candidate;
    };
    let better = originals
        .iter()
        .filter(|r| r.verdict == Verdict::Better)
        .collect::<Vec<_>>();
    let confirmed = better.iter().any(|r| r.confirmed > 0);
    let validated = results.iter().any(|r| {
        r.validates.is_some()
            && r.verdict == Verdict::Better
            && r.validates
                .as_deref()
                .is_some_and(|id| better.iter().any(|b| b.id == id))
    });
    if confirmed && validated {
        Stage::Validated
    } else if confirmed {
        Stage::Reproduced
    } else if let Some(best) = better.first() {
        Stage::Result(best.verdict)
    } else {
        Stage::Result(latest.verdict)
    }
}

struct Builder {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
}

impl Builder {
    fn add(
        &mut self,
        id: String,
        kind: Kind,
        label: &str,
        line: &str,
        parent: Option<usize>,
    ) -> usize {
        let depth = parent.map_or(0, |p| self.nodes[p].depth + 1);
        let family = parent.and_then(|p| self.nodes[p].family.clone());
        self.nodes.push(Node {
            id,
            kind,
            label: label.to_string(),
            line: line.to_string(),
            parent,
            depth,
            weight: 0.3,
            health: Health::Unmeasured,
            stage: None,
            family,
            gaps: Vec::new(),
            local: None,
            showcase: false,
        });
        let index = self.nodes.len() - 1;
        if let Some(parent) = parent {
            let kind = match (self.nodes[parent].kind, kind) {
                (Kind::Front, _) | (Kind::Family, _) => EdgeKind::Groups,
                (Kind::Coder, Kind::Engine) => EdgeKind::HandsOff,
                (Kind::Coder, Kind::Plugin) => EdgeKind::Admits,
                (_, Kind::Screen) => EdgeKind::Opens,
                _ => EdgeKind::ServedBy,
            };
            self.edges.push(Edge {
                from: parent,
                to: index,
                kind,
            });
        }
        index
    }

    fn find(&self, id: &str) -> Option<usize> {
        self.nodes.iter().position(|node| node.id == id)
    }

    fn link(&mut self, from: usize, to: usize, kind: EdgeKind) {
        if !self
            .edges
            .iter()
            .any(|e| e.from == from && e.to == to && e.kind == kind)
        {
            self.edges.push(Edge { from, to, kind });
        }
    }
}

impl Map {
    /// The map from the committed sources and records, with nothing local.
    #[must_use]
    pub fn committed() -> Self {
        Map::build(Sources::committed(), Records::committed(), Local::default())
    }

    /// Builds the map. Deterministic: the same inputs give the same nodes
    /// in the same order, so positions hold across refreshes.
    #[must_use]
    pub fn build(sources: Sources, records: Records, local: Local) -> Self {
        let mut b = Builder {
            nodes: Vec::new(),
            edges: Vec::new(),
        };
        let front = b.add(
            "front".into(),
            Kind::Front,
            "OpenAgents",
            "Reads every message first and decides where it goes.",
            None,
        );
        b.nodes[front].weight = 1.0;
        b.nodes[front].health = Health::Good;
        let max_rows = sources
            .labeled
            .routes
            .values()
            .map(|rows| rows.rows)
            .max()
            .unwrap_or(1)
            .max(1);
        let mut families = BTreeMap::new();
        for word in FAMILIES {
            if !sources.routes.iter().any(|route| route.family == word) {
                continue;
            }
            let index = b.add(
                format!("family:{word}"),
                Kind::Family,
                family_label(word),
                match word {
                    "answers" => "Replies in the chat: prepared answers, knowledge, or the chat model.",
                    "work" => "Work handed to Coder on a computer.",
                    "gym" => "Testing plugins: tests, results, checks, credit, and news.",
                    "screens" => "Screens, decks, and commands the chat opens or offers.",
                    _ => "Where the router stops: it asks back, refuses, ends, or finds nothing that serves the request.",
                },
                Some(front),
            );
            b.nodes[index].family = Some(word.to_string());
            b.nodes[index].weight = 0.8;
            b.nodes[index].health = Health::Good;
            families.insert(word.to_string(), index);
        }
        let mut routes = BTreeMap::new();
        for route in &sources.routes {
            let Some(&family) = families.get(&route.family) else {
                continue;
            };
            let index = b.add(
                format!("route:{}", route.id),
                Kind::Route,
                &route.id,
                &route.what,
                Some(family),
            );
            let rows = sources
                .labeled
                .routes
                .get(&route.id)
                .copied()
                .unwrap_or_default();
            b.nodes[index].weight = 0.35 + 0.65 * (rows.rows as f32 / max_rows as f32).sqrt();
            b.nodes[index].health = match sources.measurement.routes.get(&route.id) {
                None => Health::Unmeasured,
                Some(score) if weak(score) => Health::Weak,
                Some(_) => Health::Good,
            };
            b.nodes[index].local = local.routes.get(&route.id).copied();
            routes.insert(route.id.clone(), index);
        }
        // The general chat model serves `general`, and is the fallback the
        // router stands back to on every answers route.
        let model = routes.get("general").map(|&general| {
            let index = b.add(
                "model".into(),
                Kind::Model,
                "Chat model",
                "A hosted general model writes the reply when no prepared answer or knowledge entry fits.",
                Some(general),
            );
            b.nodes[index].health = Health::Good;
            index
        });
        // Prepared answers, under the first route each answers.
        for answer in &sources.answers {
            let Some(&route) = answer.routes.iter().find_map(|r| routes.get(r)) else {
                continue;
            };
            let label = answer.chip.clone().unwrap_or_else(|| answer.id.clone());
            let index = b.add(
                format!("answer:{}", answer.id),
                Kind::Answer,
                &label,
                &format!("Prepared answer {}@{}.", answer.id, answer.version),
                Some(route),
            );
            b.nodes[index].health = Health::Good;
            for other in answer.routes.iter().skip(1) {
                if let Some(&other) = routes.get(other) {
                    b.link(other, index, EdgeKind::ServedBy);
                }
            }
        }
        // Screens the answers open, under the first route whose answer
        // offers each.
        for answer in &sources.answers {
            let Some(screen) = answer.offer.as_ref().and_then(|o| o.screen.as_ref()) else {
                continue;
            };
            let Some(from) = b.find(&format!("answer:{}", answer.id)) else {
                continue;
            };
            let id = format!("screen:{screen}");
            let index = match b.find(&id) {
                Some(index) => index,
                None => {
                    let route = b.nodes[from].parent.unwrap_or(front);
                    let index = b.add(
                        id,
                        Kind::Screen,
                        &screen_label(screen),
                        &format!("The app's screen `{screen}`, opened from a reply's offer."),
                        Some(route),
                    );
                    b.nodes[index].health = Health::Good;
                    index
                }
            };
            b.link(from, index, EdgeKind::Opens);
        }
        // Decks, under presentation.open.
        if let Some(&route) = routes.get("presentation.open") {
            for deck in &sources.decks {
                let index = b.add(
                    format!("deck:{}", deck.id),
                    Kind::Screen,
                    &deck.title,
                    &format!(
                        "The deck `{}`, opened in the desktop slide viewer.",
                        deck.id
                    ),
                    Some(route),
                );
                b.nodes[index].health = Health::Good;
            }
        }
        // The command offers serve cli.
        for capability in &sources.capabilities {
            let (Some(route), true) = (capability.route.as_deref(), capability.id == "chat.cli")
            else {
                continue;
            };
            if let Some(&route) = routes.get(route) {
                let index = b.add(
                    format!("screen:{}", capability.id),
                    Kind::Screen,
                    &capability.name,
                    &capability.line,
                    Some(route),
                );
                b.nodes[index].health = Health::Good;
            }
        }
        // Knowledge: the product knowledge base serves product.kb; the
        // repository's code serves codebase.kb.
        if let Some(&route) = routes.get("product.kb") {
            let base = b.add(
                "knowledge:product".into(),
                Kind::Knowledge,
                "Product knowledge",
                "Reviewed, sourced entries about OpenAgents the chat answers product questions from (knowledge/openagents).",
                Some(route),
            );
            b.nodes[base].health = Health::Good;
            b.nodes[base].weight = 0.6;
            for entry in &sources.knowledge.product {
                let index = b.add(
                    format!("knowledge:{}", entry.id),
                    Kind::Knowledge,
                    &entry.title,
                    &format!("Knowledge entry {} ({}).", entry.id, entry.status),
                    Some(base),
                );
                b.nodes[index].health = if entry.status == "admitted" {
                    Health::Good
                } else {
                    Health::Unmeasured
                };
            }
        }
        if let Some(&route) = routes.get("codebase.kb") {
            let index = b.add(
                "knowledge:codebase".into(),
                Kind::Knowledge,
                "The OpenAgents code",
                "Questions about how OpenAgents is built are answered from this repository's code and docs.",
                Some(route),
            );
            b.nodes[index].health = Health::Good;
            b.nodes[index].weight = 0.5;
        }
        if let Some(model) = model {
            for word in ["meta", "smalltalk", "product.kb", "codebase.kb"] {
                if let Some(&route) = routes.get(word) {
                    b.link(route, model, EdgeKind::ServedBy);
                }
            }
        }
        // Coder, its engines, its knowledge, and the plugins it admits.
        if let Some(&route) = routes.get("work.dispatch") {
            let line = sources
                .capabilities
                .iter()
                .find(|c| c.id == "chat.coder")
                .map_or(
                    "Our coding agent, on a computer the person connected.",
                    |c| c.line.as_str(),
                )
                .to_string();
            let coder = b.add("coder".into(), Kind::Coder, "Coder", &line, Some(route));
            b.nodes[coder].weight = 0.9;
            b.nodes[coder].health = Health::Good;
            for engine in &sources.engines {
                let index = b.add(
                    format!("engine:{}", engine.id),
                    Kind::Engine,
                    &engine.name,
                    &format!("An engine Coder delegates to: {}.", engine.name),
                    Some(coder),
                );
                b.nodes[index].health = match engine_reading(&local, &engine.name) {
                    EngineReading::Ready => Health::Good,
                    EngineReading::SignedOut | EngineReading::AtLimit => Health::Weak,
                    EngineReading::Unknown => Health::Unmeasured,
                };
            }
            let coding = &sources.knowledge.coding;
            let index = b.add(
                "knowledge:coding".into(),
                Kind::Knowledge,
                "Coding knowledge",
                &format!(
                    "{} entries Coder looks up while it works (methods, edge cases, slips, environments, command guides); {} in use.",
                    coding.entries, coding.admitted
                ),
                Some(coder),
            );
            b.nodes[index].health = Health::Good;
            b.nodes[index].weight = 0.5;
            for plugin in &sources.plugins {
                let stage = stage_of(plugin, &records);
                let index = b.add(
                    format!("plugin:{}", plugin.dir),
                    Kind::Plugin,
                    &plugin.name,
                    &plugin.summary,
                    Some(coder),
                );
                b.nodes[index].stage = Some(stage);
                b.nodes[index].health = match stage {
                    Stage::NotPackaged | Stage::Candidate => Health::Unmeasured,
                    Stage::Result(Verdict::Better)
                    | Stage::Reproduced
                    | Stage::Validated
                    | Stage::Adopted => Health::Good,
                    Stage::Result(_) => Health::Weak,
                };
                let tests: u64 = plugin
                    .tests
                    .iter()
                    .map(|t| t.should_fire + t.should_not_fire)
                    .sum();
                b.nodes[index].weight = 0.35 + 0.05 * tests.min(12) as f32;
                b.nodes[index].showcase = sources.showcase.as_deref() == Some(plugin.dir.as_str());
                if let Some(&run) = routes.get("eval.run") {
                    b.link(run, index, EdgeKind::Tests);
                }
            }
        }
        // The Gym routes open the Gym's screens their answers offer; the
        // authoring route leads to the example to copy.
        if let (Some(&author), Some(showcase)) = (
            routes.get("eval.author"),
            sources
                .showcase
                .as_ref()
                .and_then(|dir| b.find(&format!("plugin:{dir}"))),
        ) {
            b.link(author, showcase, EdgeKind::Tests);
        }
        let mut map = Map {
            nodes: b.nodes,
            edges: b.edges,
            gaps: Vec::new(),
            set: sources.set.clone(),
            bank: sources.bank.clone(),
            sources,
            records,
            local,
        };
        map.gaps = gaps(&map);
        for (index, gap) in map.gaps.iter().enumerate() {
            map.nodes[gap.node].gaps.push(index);
        }
        map
    }

    /// The node with this id.
    #[must_use]
    pub fn find(&self, id: &str) -> Option<usize> {
        self.nodes.iter().position(|node| node.id == id)
    }

    /// The sources the map was built from.
    #[must_use]
    pub fn sources(&self) -> &Sources {
        &self.sources
    }

    /// The records the map was built from.
    #[must_use]
    pub fn records(&self) -> &Records {
        &self.records
    }

    /// A node's children in the tree, in order.
    #[must_use]
    pub fn children(&self, node: usize) -> Vec<usize> {
        (0..self.nodes.len())
            .filter(|&i| self.nodes[i].parent == Some(node))
            .collect()
    }

    /// Every node in the outline's order: depth first, children in order.
    #[must_use]
    pub fn outline(&self) -> Vec<usize> {
        let mut out = Vec::with_capacity(self.nodes.len());
        let mut stack: Vec<usize> = (0..self.nodes.len())
            .filter(|&i| self.nodes[i].parent.is_none())
            .rev()
            .collect();
        while let Some(node) = stack.pop() {
            out.push(node);
            let mut children = self.children(node);
            children.reverse();
            stack.extend(children);
        }
        out
    }

    /// The name a screen reader reads for a node: its kind, its label, its
    /// state, and how many gaps mark it.
    #[must_use]
    pub fn accessible_name(&self, node: usize) -> String {
        let n = &self.nodes[node];
        let mut name = format!("{}: {}", n.kind.label(), n.label);
        if let Some(stage) = n.stage {
            name.push_str(&format!(", {}", stage.words()));
        } else if matches!(n.kind, Kind::Route | Kind::Engine) {
            name.push_str(&format!(", {}", n.health.words().to_lowercase()));
        }
        match n.gaps.len() {
            0 => {}
            1 => name.push_str(", 1 gap"),
            count => name.push_str(&format!(", {count} gaps")),
        }
        name
    }

    /// The short line a zoomed-in label adds under a node's name: a
    /// route's held-out numbers and rows, a plugin's stage, an engine's
    /// reading, or Coder's members.
    #[must_use]
    pub fn evidence(&self, node: usize) -> Option<String> {
        let n = &self.nodes[node];
        match n.kind {
            Kind::Route => {
                let rows = self
                    .sources
                    .labeled
                    .routes
                    .get(&n.label)
                    .map_or(0, |rows| rows.rows);
                Some(match self.sources.measurement.routes.get(&n.label) {
                    Some(score) => format!(
                        "P {} · R {} · {rows} rows",
                        percent(score.precision),
                        percent(score.recall)
                    ),
                    None => format!("Not measured · {rows} rows"),
                })
            }
            Kind::Plugin => n.stage.map(|stage| stage.words().to_string()),
            Kind::Engine => Some(
                match engine_reading(&self.local, &n.label) {
                    EngineReading::Ready => "Ready",
                    EngineReading::SignedOut => "Not signed in",
                    EngineReading::AtLimit => "Temporarily unavailable",
                    EngineReading::Unknown => "No reading",
                }
                .to_string(),
            ),
            Kind::Coder => Some(format!(
                "{} engines · {} plugins",
                self.sources.engines.len(),
                self.nodes.iter().filter(|x| x.kind == Kind::Plugin).count()
            )),
            _ => None,
        }
    }

    /// What the inspector shows for `node`.
    #[must_use]
    pub fn inspect(&self, node: usize) -> Inspector {
        let n = &self.nodes[node];
        let mut fields = Vec::new();
        let mut why = None;
        let mut steps: Vec<NextStep> = Vec::new();
        match n.kind {
            Kind::Front => {
                fields.push(Field::linked(
                    "Question set",
                    &self.set,
                    path(
                        "The router's design",
                        "docs/coder/design/2026-09-28-chat-router.md",
                    ),
                ));
                fields.push(Field::linked(
                    "Answer bank",
                    &self.bank,
                    path("The bank", "crates/coder/answers/chat-answers-v1.toml"),
                ));
                fields.push(Field::new(
                    "Routes",
                    self.nodes
                        .iter()
                        .filter(|x| x.kind == Kind::Route)
                        .count()
                        .to_string(),
                ));
                let m = &self.sources.measurement;
                fields.push(Field::linked(
                    "Held-out route accuracy",
                    format!(
                        "{} of {} rows ({})",
                        percent(m.route_accuracy),
                        m.rows,
                        m.set
                    ),
                    path("The record", &m.record),
                ));
                if self.local.turns() > 0 {
                    fields.push(Field::new(
                        "Your replies",
                        format!(
                            "{} replies in your chats, counted on this computer",
                            self.local.turns()
                        ),
                    ));
                }
            }
            Kind::Route => {
                let word = n.label.as_str();
                if let Some(route) = self.sources.route(word) {
                    why = Some(route.description.clone());
                    if let Some(not_for) = &route.not_for {
                        fields.push(Field::new("Not for", not_for.clone()));
                    }
                    fields.push(Field::new("Family", family_label(&route.family)));
                }
                let rows = self
                    .sources
                    .labeled
                    .routes
                    .get(word)
                    .copied()
                    .unwrap_or_default();
                fields.push(Field::linked(
                    "Labeled examples",
                    format!(
                        "{} rows: {} to tune, {} held out, {} near misses",
                        rows.rows, rows.tune, rows.held_out, rows.near_misses
                    ),
                    path("The labeled set", &self.sources.labeled.path),
                ));
                let m = &self.sources.measurement;
                match m.routes.get(word) {
                    Some(score) => {
                        fields.push(Field::linked(
                            "Held-out precision",
                            format!(
                                "{} of {} read as {word}",
                                percent(score.precision),
                                score.predicted
                            ),
                            path("The record", &m.record),
                        ));
                        fields.push(Field::linked(
                            "Held-out recall",
                            format!(
                                "{} of {} labeled {word}",
                                percent(score.recall),
                                score.labeled
                            ),
                            path("The measurement", &m.page),
                        ));
                    }
                    None => fields.push(Field::linked(
                        "Held-out numbers",
                        format!("Not in the latest per-route record ({})", m.set),
                        path("The record", &m.record),
                    )),
                }
                let answers = self
                    .sources
                    .answers
                    .iter()
                    .filter(|a| a.routes.iter().any(|r| r == word))
                    .count();
                fields.push(Field::new("Prepared answers", answers.to_string()));
                if let Some(count) = n.local {
                    fields.push(Field::new(
                        "Your replies here",
                        format!(
                            "{count} of your replies took this route (counted on this computer)"
                        ),
                    ));
                }
            }
            Kind::Answer => {
                let id = n.id.trim_start_matches("answer:");
                if let Some(answer) = self.sources.answers.iter().find(|a| a.id == id) {
                    fields.push(Field::linked(
                        "Answer",
                        format!("{}@{}", answer.id, answer.version),
                        path("The bank", "crates/coder/answers/chat-answers-v1.toml"),
                    ));
                    fields.push(Field::new("Routes", answer.routes.join(", ")));
                    fields.push(Field::new(
                        "Shown",
                        match answer.place.as_str() {
                            "here" => "In a chat on this computer",
                            "away" => "In a chat away from a computer",
                            _ => "Anywhere",
                        },
                    ));
                    if let Some(offer) = &answer.offer {
                        fields.push(Field::new(
                            "Offers",
                            match &offer.screen {
                                Some(screen) => {
                                    format!("{} ({})", offer.label, screen_label(screen))
                                }
                                None => offer.label.clone(),
                            },
                        ));
                    }
                    for source in &answer.sources {
                        fields.push(Field::linked(
                            "Source",
                            source.clone(),
                            path("Source", source),
                        ));
                    }
                }
            }
            Kind::Knowledge => {
                let id = n.id.trim_start_matches("knowledge:");
                if let Some(entry) = self.sources.knowledge.product.iter().find(|e| e.id == id) {
                    fields.push(Field::linked(
                        "Entry",
                        &entry.id,
                        path("The entry", &entry.path),
                    ));
                    fields.push(Field::new(
                        "Status",
                        match entry.status.as_str() {
                            "admitted" => "In use".to_owned(),
                            "candidate" => "Under review".to_owned(),
                            "withdrawn" => "Withdrawn".to_owned(),
                            other => other.to_owned(),
                        },
                    ));
                    fields.push(Field::new(
                        "Reviewed answer",
                        if entry.answer {
                            "Yes, shown whole when it fits"
                        } else {
                            "No; the model answers from it"
                        },
                    ));
                } else if id == "product" {
                    let in_use = self
                        .sources
                        .knowledge
                        .product
                        .iter()
                        .filter(|e| e.status == "admitted")
                        .count();
                    fields.push(Field::linked(
                        "Entries",
                        format!("{} ({in_use} in use)", self.sources.knowledge.product.len()),
                        path("The entries", "knowledge/openagents"),
                    ));
                    let kb = &self.sources.product_kb;
                    fields.push(Field::linked(
                        "Questions with no entry",
                        format!(
                            "{} of {} in {}",
                            kb.unanswerable.len(),
                            kb.questions,
                            kb.set
                        ),
                        path("The measurement", &kb.record),
                    ));
                } else if id == "codebase" {
                    let kb = &self.sources.codebase_kb;
                    fields.push(Field::linked(
                        "Question set",
                        format!("{} questions", kb.questions),
                        path("The questions", &kb.path),
                    ));
                } else if id == "coding" {
                    let coding = &self.sources.knowledge.coding;
                    fields.push(Field::linked(
                        "Entries",
                        format!(
                            "{} ({} in use, {} under review)",
                            coding.entries, coding.admitted, coding.candidates
                        ),
                        path("The entries", &coding.path),
                    ));
                    for (kind, count) in &coding.kinds {
                        let label = match kind.as_str() {
                            "method" => "Methods",
                            "edge-case" => "Edge cases",
                            "slip" => "Slips",
                            "environment" => "Environments",
                            "tool" => "Command guides",
                            "product" => "Product",
                            other => other,
                        };
                        fields.push(Field::new(label, count.to_string()));
                    }
                }
            }
            Kind::Engine => {
                let reading = engine_reading(&self.local, &n.label);
                fields.push(Field::new(
                    "On this computer",
                    match reading {
                        EngineReading::Ready => "Signed in, with capacity",
                        EngineReading::SignedOut => "Not signed in",
                        EngineReading::AtLimit => "Temporarily unavailable; another engine runs",
                        EngineReading::Unknown => "No reading from this computer's Coder",
                    },
                ));
            }
            Kind::Plugin => {
                let dir = n.id.trim_start_matches("plugin:");
                if let Some(plugin) = self.sources.plugins.iter().find(|p| p.dir == dir) {
                    self.plugin_fields(plugin, &mut fields);
                    if n.showcase {
                        steps.push(NextStep::Chat {
                            label: "Make one like this".into(),
                            message: format!("Help me make a plugin like {} that ", plugin.name),
                        });
                    }
                }
            }
            Kind::Coder => {
                let plugins = self.nodes.iter().filter(|x| x.kind == Kind::Plugin).count();
                fields.push(Field::new(
                    "Engines",
                    self.sources.engines.len().to_string(),
                ));
                fields.push(Field::new("Plugins it can use", plugins.to_string()));
                fields.push(Field::linked(
                    "Adopted into everyone's Coder",
                    self.nodes
                        .iter()
                        .filter(|x| x.stage == Some(Stage::Adopted))
                        .map(|x| x.label.clone())
                        .collect::<Vec<_>>()
                        .join(", "),
                    path("Coder's defaults", "packages/coder-defaults/policy.md"),
                ));
            }
            Kind::Family | Kind::Model | Kind::Screen => {}
        }
        for &gap in &n.gaps {
            let step = self.gaps[gap].step.clone();
            if !steps.contains(&step) {
                steps.push(step);
            }
        }
        Inspector {
            node,
            title: n.label.clone(),
            kind: n.kind,
            line: n.line.clone(),
            why,
            fields,
            members: self
                .children(node)
                .into_iter()
                .map(|child| (child, self.nodes[child].label.clone()))
                .collect(),
            gaps: n.gaps.clone(),
            steps,
        }
    }

    fn plugin_fields(&self, plugin: &PluginSource, fields: &mut Vec<Field>) {
        fields.push(Field::linked(
            "Where",
            &plugin.dir,
            path("Its directory", &plugin.dir),
        ));
        let parts = [
            ("Wasm", &plugin.wasm),
            ("Workflows", &plugin.workflows),
            ("Skills", &plugin.skills),
            ("Knowledge", &plugin.knowledge),
        ];
        for (label, items) in parts {
            if !items.is_empty() {
                fields.push(Field::new(label, items.join(", ")));
            }
        }
        for set in &plugin.tests {
            fields.push(Field::linked(
                "Tests",
                format!(
                    "{}: {} where it should help, {} where it should stay out",
                    set.dir, set.should_fire, set.should_not_fire
                ),
                path("The test set", &set.dir),
            ));
        }
        let (Some(slug), Some(publisher)) = (&plugin.slug, &plugin.publisher) else {
            fields.push(Field::new("Package", "No package record yet"));
            return;
        };
        let package = format!("{publisher}:{slug}");
        let results = self.records.of_package(&package);
        if results.is_empty() {
            fields.push(Field::new("Results", "No published result yet"));
        }
        for result in results {
            let role = if result.checks.is_some() {
                "Check"
            } else if result.validates.is_some() {
                "Validation"
            } else {
                "Result"
            };
            let mut value = format!("{}: {}", result.verdict.words(), result.headline());
            if result.original() && result.confirmed > 0 {
                value.push_str(&format!(
                    ", confirmed by {}",
                    plural(result.confirmed, "check")
                ));
            }
            fields.push(Field::linked(role, value, event("The record", &result.id)));
        }
        for adoption in self.records.adoptions_of(&package) {
            fields.push(Field::linked(
                "Adopted",
                "In Coder's defaults for everyone",
                event("The defaults release", &adoption.defaults_release),
            ));
        }
    }

    /// The gaps the Gaps panel lists, filtered.
    #[must_use]
    pub fn gaps_where(&self, filter: &Filter) -> Vec<usize> {
        (0..self.gaps.len())
            .filter(|&gap| filter.admits(self, self.gaps[gap].node))
            .collect()
    }
}

fn plural(count: u64, word: &str) -> String {
    if count == 1 {
        format!("1 {word}")
    } else {
        format!("{count} {word}s")
    }
}

fn weak(score: &sources::RouteScore) -> bool {
    (score.predicted >= MIN_SCORED && score.precision < PRECISION_FLOOR)
        || (score.labeled >= MIN_SCORED && score.recall < RECALL_FLOOR)
}

/// What this computer says about an engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineReading {
    Ready,
    SignedOut,
    AtLimit,
    /// No report, or the report doesn't name it.
    Unknown,
}

/// The reading for the engine named `name` (`Claude Code`), matched by
/// the report's own engine names.
#[must_use]
pub fn engine_reading(local: &Local, name: &str) -> EngineReading {
    let Some(report) = &local.engines else {
        return EngineReading::Unknown;
    };
    if let Some(route) = report.routes.iter().find(|r| r.name == name) {
        if !route.signed_in {
            return EngineReading::SignedOut;
        }
        if let RouteUsage::Windows {
            limit_reached: true,
            ..
        } = route.usage
        {
            return EngineReading::AtLimit;
        }
        return EngineReading::Ready;
    }
    match report.accounts.iter().find(|a| a.name == name) {
        Some(account) if account.signed_in => EngineReading::Ready,
        Some(_) => EngineReading::SignedOut,
        None => EngineReading::Unknown,
    }
}

/// The issue a person opens to add labeled examples for `route`.
#[must_use]
pub fn examples_issue(route: &str) -> String {
    let title = format!("Router: labeled examples for {route}");
    let body = format!(
        "The route map shows `{route}` needs more or better labeled examples.\n\n\
         Real first messages that should go to `{route}`, and near misses that should not:\n\n- \n\n\
         Rows go in `crates/coder/fixtures/chat-router/routes-v4.json` with a recalibration, \
         per docs/coder/design/2026-09-28-chat-router.md."
    );
    format!(
        "{REPOSITORY}/issues/new?title={}&body={}",
        query(&title),
        query(&body)
    )
}

fn examples_step(route: &str) -> NextStep {
    NextStep::Issue {
        label: "Add labeled examples".into(),
        url: examples_issue(route),
    }
}

fn knowledge_step(topic: &str) -> NextStep {
    NextStep::Command {
        label: "Write a knowledge entry".into(),
        command: format!(
            "microcoder kb add openagents.{topic} --kind product --title \"…\" --dir knowledge/openagents"
        ),
        doc: "docs/coder/design/knowledge-base.md".into(),
    }
}

/// Every gap in `map`, in a stable order: by kind in [`GapKind::ALL`]'s
/// order (requests nothing serves first), then routes, plugins, and
/// engines in the map's order.
fn gaps(map: &Map) -> Vec<Gap> {
    let sources = &map.sources;
    let mut out = Vec::new();
    for route in &sources.routes {
        let Some(node) = map.find(&format!("route:{}", route.id)) else {
            continue;
        };
        let word = route.id.as_str();
        let rows = sources
            .labeled
            .routes
            .get(word)
            .copied()
            .unwrap_or_default();
        let labeled = path("The labeled set", &sources.labeled.path);
        if word == "capability.missing" {
            let mut detail = format!(
                "People ask for things no plugin serves: {} labeled examples land here.",
                rows.rows
            );
            if let Some(count) = map.local.routes.get(word).filter(|&&c| c > 0) {
                detail.push_str(&format!(" {count} of your own replies did too."));
            }
            out.push(Gap {
                kind: GapKind::NoPlugin,
                node,
                title: "Requests nothing serves".into(),
                detail,
                evidence: vec![
                    labeled.clone(),
                    path(
                        "The measurement",
                        "docs/coder/measurements/2026-09-29-missing-capability.md",
                    ),
                ],
                step: NextStep::Chat {
                    label: "Draft a plugin in chat".into(),
                    message: "Help me make a plugin that ".into(),
                },
            });
        }
        let answers = sources
            .answers
            .iter()
            .filter(|a| a.routes.iter().any(|r| r == word))
            .count();
        if route.family == "answers"
            && answers == 0
            && !matches!(word, "product.kb" | "codebase.kb")
        {
            out.push(Gap {
                kind: GapKind::ModelOnly,
                node,
                title: format!("{word} is answered by the chat model alone"),
                detail: "No prepared answer or knowledge entry serves it.".into(),
                evidence: vec![path(
                    "The bank",
                    "crates/coder/answers/chat-answers-v1.toml",
                )],
                step: knowledge_step(&word.replace('.', "-")),
            });
        }
        if word == "product.kb" && !sources.product_kb.unanswerable.is_empty() {
            let kb = &sources.product_kb;
            let examples: Vec<&str> = kb
                .unanswerable
                .iter()
                .take(3)
                .map(|q| q.question.as_str())
                .collect();
            out.push(Gap {
                kind: GapKind::UnansweredQuestions,
                node,
                title: format!(
                    "{} product questions have no knowledge entry",
                    kb.unanswerable.len()
                ),
                detail: format!("Such as: {}", examples.join(" · ")),
                evidence: vec![path("The measurement", &kb.record)],
                step: knowledge_step("new-topic"),
            });
        }
        match sources.measurement.routes.get(word) {
            Some(score) if weak(score) => out.push(Gap {
                kind: GapKind::WeakRoute,
                node,
                title: format!("{word} is weak held out"),
                detail: format!(
                    "Precision {} of {} read, recall {} of {} labeled (floors {} and {}).",
                    percent(score.precision),
                    score.predicted,
                    percent(score.recall),
                    score.labeled,
                    percent(PRECISION_FLOOR),
                    percent(RECALL_FLOOR)
                ),
                evidence: vec![
                    path("The record", &sources.measurement.record),
                    path("The measurement", &sources.measurement.page),
                ],
                step: examples_step(word),
            }),
            Some(_) => {}
            None => out.push(Gap {
                kind: GapKind::RouteUnmeasured,
                node,
                title: format!("{word} has no held-out numbers"),
                detail: format!(
                    "The latest per-route record ({}) doesn't measure it.",
                    sources.measurement.set
                ),
                evidence: vec![path("The record", &sources.measurement.record)],
                step: examples_step(word),
            }),
        }
        if rows.rows < MIN_ROWS {
            out.push(Gap {
                kind: GapKind::FewExamples,
                node,
                title: format!("{word} has few labeled examples"),
                detail: format!("{} rows, fewer than {MIN_ROWS}.", rows.rows),
                evidence: vec![labeled.clone()],
                step: examples_step(word),
            });
        }
        if word == "clarify" {
            let turns = map.local.turns();
            let clarify = map.local.routes.get(word).copied().unwrap_or(0);
            if turns >= MIN_LOCAL_TURNS && clarify as f64 / turns as f64 > CLARIFY_SHARE {
                out.push(Gap {
                    kind: GapKind::FrequentClarify,
                    node,
                    title: "We ask you back often".into(),
                    detail: format!(
                        "{clarify} of your {turns} replies asked you to clarify (counted on this computer)."
                    ),
                    evidence: vec![labeled.clone()],
                    step: examples_step(word),
                });
            }
        }
    }
    for plugin in &sources.plugins {
        let Some(node) = map.find(&format!("plugin:{}", plugin.dir)) else {
            continue;
        };
        let stage = map.nodes[node].stage.unwrap_or(Stage::Candidate);
        let catalog = plugin
            .knowledge
            .iter()
            .any(|note| sources.capabilities.iter().any(|c| &c.id == note));
        let test = if catalog {
            NextStep::Chat {
                label: "Test it in chat".into(),
                message: format!("Test {} on Coder", plugin.name),
            }
        } else {
            NextStep::Command {
                label: "Run its tests".into(),
                command: format!("openagents ext eval run {} --trust", plugin.dir),
                doc: "docs/extensions/evaluation.md".into(),
            }
        };
        let package = plugin
            .slug
            .as_ref()
            .zip(plugin.publisher.as_ref())
            .map(|(slug, publisher)| format!("{publisher}:{slug}"));
        let results = package
            .as_deref()
            .map(|p| map.records.of_package(p))
            .unwrap_or_default();
        let evidence: Vec<Link> = results
            .iter()
            .take(3)
            .map(|r| event("A result", &r.id))
            .chain(std::iter::once(path("Its directory", &plugin.dir)))
            .collect();
        let gap = |kind, title: String, detail: String, step| Gap {
            kind,
            node,
            title,
            detail,
            evidence: evidence.clone(),
            step,
        };
        match stage {
            Stage::NotPackaged => out.push(gap(
                GapKind::NotPackaged,
                format!("{} has no package or tests", plugin.name),
                "Code with no package record or test set can't be tested with and without it."
                    .into(),
                NextStep::Command {
                    label: "Write its tests".into(),
                    command: format!("openagents ext eval init {}", plugin.dir),
                    doc: "docs/extensions/evaluation.md".into(),
                },
            )),
            Stage::Candidate => out.push(gap(
                GapKind::NoResult,
                format!("{} has no published result", plugin.name),
                "Installed and packaged, never measured with and without it.".into(),
                test.clone(),
            )),
            Stage::Result(Verdict::Better) => out.push(gap(
                GapKind::NeedsCheck,
                format!("Check the {} result", plugin.name),
                "A Better result nobody else has rerun yet.".into(),
                NextStep::Chat {
                    label: "Check it in chat".into(),
                    message: format!("Check someone's {} result", plugin.name),
                },
            )),
            Stage::Result(verdict) => out.push(gap(
                GapKind::NoHelpYet,
                format!("{} didn't help yet", plugin.name),
                format!("Its latest result reads {}.", verdict.words()),
                test.clone(),
            )),
            Stage::Reproduced => {
                let checks = results
                    .iter()
                    .filter(|r| r.original())
                    .map(|r| r.confirmed)
                    .max()
                    .unwrap_or(0);
                out.push(gap(
                    GapKind::NeedsValidation,
                    format!("Validate {} on a second test set", plugin.name),
                    format!(
                        "Confirmed by {}; adoption also needs a Better result on a test set someone else wrote, and {CHECKS_FOR_ADOPTION} checks.",
                        plural(checks, "check")
                    ),
                    NextStep::Chat {
                        label: "Write a second test set".into(),
                        message: format!("Help me write a second test set for {}", plugin.name),
                    },
                ));
            }
            Stage::Validated => {
                let checks = results
                    .iter()
                    .filter(|r| r.original())
                    .map(|r| r.confirmed)
                    .max()
                    .unwrap_or(0);
                if checks >= CHECKS_FOR_ADOPTION {
                    out.push(gap(
                        GapKind::ReadyToAdopt,
                        format!("{} is one step from adoption", plugin.name),
                        "Better, checked, and validated: an operator can adopt it into Coder's defaults.".into(),
                        NextStep::Command {
                            label: "Adopt it (operator)".into(),
                            command: "microcoder xp adopt --subject RELEASE_ID".into(),
                            doc: "docs/extensions/evaluation.md".into(),
                        },
                    ));
                } else {
                    out.push(gap(
                        GapKind::NeedsCheck,
                        format!("{} needs more checks", plugin.name),
                        format!(
                            "Validated with {}; adoption needs {CHECKS_FOR_ADOPTION} by distinct trainers.",
                            plural(checks, "check")
                        ),
                        NextStep::Chat {
                            label: "Check it in chat".into(),
                            message: format!("Check someone's {} result", plugin.name),
                        },
                    ));
                }
            }
            Stage::Adopted => {}
        }
    }
    for engine in &sources.engines {
        let Some(node) = map.find(&format!("engine:{}", engine.id)) else {
            continue;
        };
        let step = NextStep::SignIn {
            label: format!("Sign in {}", engine.name),
            engine: engine.id.clone(),
        };
        match engine_reading(&map.local, &engine.name) {
            EngineReading::SignedOut => out.push(Gap {
                kind: GapKind::EngineSignedOut,
                node,
                title: format!("{} isn't signed in", engine.name),
                detail: "This computer's Coder can't hand work to it until it is.".into(),
                evidence: vec![],
                step,
            }),
            EngineReading::AtLimit => out.push(Gap {
                kind: GapKind::EngineAtLimit,
                node,
                title: format!("{} is used up for now", engine.name),
                detail: "This computer's Coder runs another engine until it's back.".into(),
                evidence: vec![],
                step: NextStep::SignIn {
                    label: "See usage".into(),
                    engine: engine.id.clone(),
                },
            }),
            EngineReading::Ready | EngineReading::Unknown => {}
        }
    }
    out.sort_by_key(|gap| gap.kind);
    out
}

/// Which nodes and gaps show: a family, kinds, and only gaps or only
/// unmeasured nodes. The default shows everything.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    /// A family word, or every family.
    pub family: Option<String>,
    /// Kinds to show; empty shows every kind.
    pub kinds: Vec<Kind>,
    pub gaps_only: bool,
    pub unmeasured_only: bool,
}

impl Filter {
    /// Whether nothing is filtered.
    #[must_use]
    pub fn is_clear(&self) -> bool {
        *self == Filter::default()
    }

    /// Whether `node` passes.
    #[must_use]
    pub fn admits(&self, map: &Map, node: usize) -> bool {
        let n = &map.nodes[node];
        if let Some(family) = &self.family
            && n.family.as_deref() != Some(family.as_str())
        {
            return false;
        }
        if !self.kinds.is_empty() && !self.kinds.contains(&n.kind) {
            return false;
        }
        if self.gaps_only && n.gaps.is_empty() {
            return false;
        }
        if self.unmeasured_only && n.health != Health::Unmeasured {
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests;
