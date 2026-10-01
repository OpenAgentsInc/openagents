//! What the route map is built from: a typed snapshot of the router's own
//! sources, written by the router's crate and pinned by its test.
//!
//! The phone and the desktop don't link the router (`coder`), so the
//! router's facts reach them as one committed document,
//! [`SOURCES`] (`route_map/sources.json`). `crates/coder/tests/route_map_sources.rs`
//! builds it from the live code and data (`RouteId::ALL` and the rubric,
//! the answer bank, the labeled set, the latest per-route router record,
//! the product and coding knowledge, the admitted-capability registry, the
//! engines, the decks, and the screens) and fails when the committed file
//! differs, so the map never shows a hand-copied route list.
//! `ROUTE_MAP_WRITE=1` rewrites it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The committed snapshot.
pub const SOURCES: &str = include_str!("sources.json");

/// The snapshot's schema.
pub const SCHEMA: &str = "openagents.route-map.sources.v1";

/// Everything the router's crate says the map is made of.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sources {
    pub schema: String,
    /// The test that writes this document.
    pub generated_by: String,
    /// The question set's identity, `chat-router-v4@…`.
    pub set: String,
    /// The answer bank's identity, `chat-answers-v1@…`.
    pub bank: String,
    /// Every route the `route` question offers, in its order.
    pub routes: Vec<RouteSource>,
    /// Every prepared answer in the bank.
    pub answers: Vec<AnswerSource>,
    /// The labeled route set's rows per route.
    pub labeled: Labeled,
    /// The latest committed per-route router record.
    pub measurement: Measurement,
    pub knowledge: Knowledge,
    /// The product knowledge base's question set: what it answered and
    /// which questions had no entry.
    pub product_kb: ProductKb,
    /// The codebase route's question set.
    pub codebase_kb: CodebaseKb,
    /// The chat's admitted capabilities with no Gym seam: its built-ins
    /// and the catalog tool notes.
    pub capabilities: Vec<CapabilitySource>,
    /// The coding engines a dispatch may name.
    pub engines: Vec<EngineSource>,
    /// The decks the desktop app ships.
    pub decks: Vec<DeckSource>,
    /// The screens an `open_screen` offer may name, by their words.
    pub screens: Vec<String>,
    /// The plugins in this repository: every `crates/plugin-*` and
    /// `packages/*` directory, with the parts it bundles.
    pub plugins: Vec<PluginSource>,
    /// The plugin the map shows as the example to copy, by its directory,
    /// when it is in this repository.
    pub showcase: Option<String>,
}

/// One route.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteSource {
    /// The wire word, `work.dispatch`.
    pub id: String,
    /// `answers`, `work`, `gym`, `screens`, or `boundaries`.
    pub family: String,
    /// What Jev reads for the route.
    pub description: String,
    /// The rubric's `what`.
    pub what: String,
    /// The rubric's `not_for`, where a neighbor is close.
    pub not_for: Option<String>,
    /// How many messages the rubric shows Jev for it.
    pub examples: usize,
}

/// One prepared answer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnswerSource {
    pub id: String,
    pub version: u32,
    /// The routes it answers.
    pub routes: Vec<String>,
    /// The question its suggestion chip sends, when it has one.
    pub chip: Option<String>,
    /// `anywhere`, `here` (on a computer), or `away`.
    pub place: String,
    /// Whether it speaks from the Gym's records.
    pub records: bool,
    pub offer: Option<AnswerOffer>,
    /// Where its facts come from, as repository paths.
    pub sources: Vec<String>,
}

/// An answer's offer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnswerOffer {
    pub run_coder: bool,
    /// The screen word it opens.
    pub screen: Option<String>,
    pub label: String,
}

/// The labeled route set.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Labeled {
    pub set: String,
    pub path: String,
    pub created: String,
    /// Rows per route word.
    pub routes: BTreeMap<String, RouteRows>,
}

/// One route's labeled rows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteRows {
    pub rows: u64,
    pub held_out: u64,
    pub tune: u64,
    /// Rows labeled with this route that sit next to another route's
    /// boundary (tagged `near-miss`).
    pub near_misses: u64,
}

/// The latest committed per-route router record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Measurement {
    /// The `openagents.eval-report.v1` record's path.
    pub record: String,
    /// The measurement page that explains it.
    pub page: String,
    /// The question set it measured.
    pub set: String,
    /// Held-out rows read.
    pub rows: u64,
    pub route_accuracy: f64,
    /// When the run ended, in Unix seconds.
    pub ended_at: u64,
    /// Per route word: held-out precision and recall with their counts.
    pub routes: BTreeMap<String, RouteScore>,
}

/// One route's held-out numbers.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteScore {
    pub precision: f64,
    /// Rows the router read as this route.
    pub predicted: u64,
    pub recall: f64,
    /// Rows labeled with this route.
    pub labeled: u64,
}

/// The knowledge entries.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Knowledge {
    /// The product knowledge base (`knowledge/openagents/`), every entry.
    pub product: Vec<KnowledgeSource>,
    /// The coding knowledge Coder's runs retrieve (`knowledge/`), counted.
    pub coding: CodingKnowledge,
}

/// One product knowledge entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnowledgeSource {
    pub id: String,
    pub title: String,
    pub path: String,
    /// `admitted`, `candidate`, or `withdrawn`.
    pub status: String,
    pub tags: Vec<String>,
    /// Whether it carries a reviewed answer the chat can show whole.
    pub answer: bool,
}

/// The coding knowledge, counted.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodingKnowledge {
    pub path: String,
    pub entries: u64,
    pub admitted: u64,
    pub candidates: u64,
    /// Entries per kind word.
    pub kinds: BTreeMap<String, u64>,
}

/// The product knowledge base's question set.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductKb {
    pub record: String,
    pub set: String,
    pub questions: u64,
    /// The questions no entry answers.
    pub unanswerable: Vec<Question>,
}

/// The codebase route's question set: questions about how this
/// repository is built, answered from its code.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodebaseKb {
    pub path: String,
    pub questions: u64,
}

/// One question of a question set.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Question {
    pub id: String,
    pub question: String,
}

/// One admitted capability of the chat.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilitySource {
    /// `chat.coder`, or a tool note's id.
    pub id: String,
    pub name: String,
    /// `program`, `plugin`, `skill`, or `knowledge`.
    pub kind: String,
    /// `chat` or `coder`.
    pub reach: String,
    /// The route that serves it from chat, for a built-in.
    pub route: Option<String>,
    pub source: String,
    pub line: String,
}

/// One coding engine.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineSource {
    /// The NIP-CJ word, `claude_code`.
    pub id: String,
    pub name: String,
}

/// One deck.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeckSource {
    pub id: String,
    pub title: String,
}

/// One plugin: what a person adds. It bundles tools (Wasm code),
/// workflows (programs), skills, knowledge, and tests.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginSource {
    /// Its directory, `crates/plugin-repo-map`: the map's stable id.
    pub dir: String,
    /// Its package record's slug, `project-map`, when it has a record.
    pub slug: Option<String>,
    pub name: String,
    pub summary: String,
    /// The key its package record names as publisher.
    pub publisher: Option<String>,
    pub version: Option<String>,
    /// Wasm tools: the guest crates it carries.
    pub tools: Vec<String>,
    /// Workflows: its programs, by file name.
    pub workflows: Vec<String>,
    /// Skills: its guidance files, by name.
    pub skills: Vec<String>,
    /// Knowledge: the product entries that describe it.
    pub knowledge: Vec<String>,
    /// Tests: each test set's directory with its counts.
    pub tests: Vec<TestSetSource>,
}

/// One test set of a plugin.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestSetSource {
    pub dir: String,
    pub should_fire: u64,
    pub should_not_fire: u64,
}

impl Sources {
    /// The committed snapshot.
    ///
    /// # Panics
    ///
    /// The committed file does not parse, which its test rules out.
    #[must_use]
    pub fn committed() -> Self {
        Self::parse(SOURCES).expect("the committed route map sources parse")
    }

    /// Reads a snapshot.
    ///
    /// # Errors
    ///
    /// The JSON doesn't parse or names another schema.
    pub fn parse(text: &str) -> Result<Self, String> {
        let sources: Sources = serde_json::from_str(text).map_err(|error| error.to_string())?;
        if sources.schema != SCHEMA {
            return Err(format!("the sources name {}, not {SCHEMA}", sources.schema));
        }
        Ok(sources)
    }

    /// The document as the generator writes it: pretty JSON and a newline.
    #[must_use]
    pub fn document(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default() + "\n"
    }

    /// The route with this word.
    #[must_use]
    pub fn route(&self, id: &str) -> Option<&RouteSource> {
        self.routes.iter().find(|route| route.id == id)
    }
}
