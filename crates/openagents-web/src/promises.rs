//! `/promises` and `/roadmap` (#11122): what OpenAgents promises, from one
//! registry compiled into the binary (`content/promises.toml`).
//!
//! `/promises` lists what works today (shipped, and new in today's
//! release), each with the one link or command to try it and links to the
//! evidence that proves it. `/roadmap` lists what comes next and later,
//! each linked to its GitHub issue. Both pages, and their Markdown twins
//! (`/promises.md`, `/roadmap.md`), are rendered from the same Markdown,
//! which is built from the registry, so they cannot disagree. A test fails
//! when a working promise names evidence that is gone. The process is in
//! `docs/promises/README.md`.

use std::sync::OnceLock;

use axum::Router;
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::get;
use maud::{PreEscaped, html};
use openagents_ui::content::{MarkdownRoot, PageColumn};
use serde::Deserialize;

use crate::App;
use crate::markdown;
use crate::pages::content::DOCS;
use crate::ui_page::{UiPage, action_link};

/// The registry, as written.
pub(crate) const SOURCE: &str = include_str!("../content/promises.toml");

/// Where the source code lives, for evidence links.
const REPO: &str = "https://github.com/OpenAgentsInc/openagents";

/// The registry: its groups, in page order, and its promises.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Registry {
    pub(crate) group: Vec<Group>,
    pub(crate) promise: Vec<Promise>,
}

/// A heading the promises sit under.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Group {
    pub(crate) id: String,
    pub(crate) title: String,
}

/// One promise: what we say the product does, where, how far along it is,
/// where to try it, the issues behind it, and what proves it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Promise {
    /// The promise's stable name: read by the tests and by people editing
    /// the registry, never shown.
    #[cfg_attr(not(test), expect(dead_code, reason = "read by the tests only"))]
    pub(crate) id: String,
    pub(crate) group: String,
    pub(crate) statement: String,
    pub(crate) surfaces: Vec<Surface>,
    pub(crate) status: Status,
    #[serde(default)]
    pub(crate) link: Option<String>,
    #[serde(default)]
    pub(crate) command: Option<String>,
    #[serde(default)]
    pub(crate) issues: Vec<u32>,
    #[serde(default)]
    pub(crate) evidence: Vec<Evidence>,
    /// The show episodes where it was promised (`docs/transcripts/NNN.md`).
    #[serde(default)]
    pub(crate) episodes: Vec<u32>,
    /// One short line: why a promise was dropped, or what part works.
    #[serde(default)]
    pub(crate) note: Option<String>,
}

/// How far along a promise is.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Status {
    /// Works today. The Kitchen Sink ledger's `live` reads as this.
    #[serde(alias = "live")]
    Shipped,
    /// New in today's 1.0 release.
    Launching,
    /// Works today, but only in part: on `/promises`, labeled, with its
    /// evidence for the part that works.
    Partial,
    /// Being built now.
    Next,
    /// Planned after that. The ledger's `missing` reads as this.
    #[serde(alias = "missing")]
    Later,
    /// Dropped on purpose: listed at the end of `/roadmap` with its
    /// `note` saying why.
    Dropped,
}

impl Status {
    /// Whether the promise works today: it is on `/promises`, and must
    /// name evidence.
    pub(crate) fn works(self) -> bool {
        matches!(self, Self::Shipped | Self::Launching | Self::Partial)
    }
}

/// Where a promise holds.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Surface {
    Web,
    Terminal,
    Iphone,
    Android,
    Desktop,
    Api,
    Verse,
    /// A phone, either kind.
    Mobile,
    /// The open network: open protocols and the API, from any client.
    Network,
}

impl Surface {
    fn label(self) -> &'static str {
        match self {
            Self::Web => "Web",
            Self::Terminal => "Terminal",
            Self::Iphone => "iPhone",
            Self::Android => "Android",
            Self::Desktop => "Desktop",
            Self::Api => "API",
            Self::Verse => "Verse",
            Self::Mobile => "Phone",
            Self::Network => "Open network",
        }
    }
}

/// What proves a promise, each kind checked by
/// `every_working_promise_names_evidence_that_exists`.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Evidence {
    /// `path/to/file.rs::test_fn`: a test in the repository.
    Test(String),
    /// A check name in the web smoke suite (`scripts/smoke/web.py`).
    Smoke(String),
    /// A web chat golden's id (`bench/web-chat/goldens-v1.json`).
    Golden(String),
    /// A document's path in the repository.
    Doc(String),
}

impl Evidence {
    /// The link text and target a reader follows to this proof.
    fn link(&self) -> (String, String) {
        match self {
            Self::Test(spec) => {
                let (path, name) = spec.split_once("::").unwrap_or((spec, spec));
                (
                    format!("Test: {}", sentence(name)),
                    format!("{REPO}/blob/main/{path}"),
                )
            }
            Self::Smoke(name) => (
                format!("Live check: {name}"),
                format!("{REPO}/blob/main/scripts/smoke/web.py"),
            ),
            Self::Golden(id) => (
                format!("Chat check: {id}"),
                format!("{REPO}/blob/main/docs/web/chat-goldens.md"),
            ),
            Self::Doc(path) => match path
                .strip_prefix("crates/openagents-web/content/docs/")
                .and_then(|rest| rest.strip_suffix(".md"))
            {
                Some(slug) => {
                    let title = DOCS
                        .iter()
                        .find(|(name, _)| *name == slug)
                        .map(|(name, source)| markdown::title(source, name))
                        .unwrap_or_else(|| slug.to_owned());
                    (format!("Guide: {title}"), format!("/docs/{slug}"))
                }
                None => (
                    format!("Document: {path}"),
                    format!("{REPO}/blob/main/{path}"),
                ),
            },
        }
    }
}

/// A test function's name as words: `a_test_name` -> `A test name`.
fn sentence(name: &str) -> String {
    let words = name.replace('_', " ");
    let mut chars = words.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => words,
    }
}

/// The registry, parsed once. It is compiled in and its test parses it,
/// so a broken registry fails the build's tests, never a visitor.
pub(crate) fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| toml::from_str(SOURCE).expect("content/promises.toml parses"))
}

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/promises", get(promises_page))
        .route("/roadmap", get(roadmap_page))
        .route("/promises.md", get(promises_md))
        .route("/roadmap.md", get(roadmap_md))
}

fn issue_link(number: u32) -> String {
    format!("[#{number}]({REPO}/issues/{number})")
}

fn episode_link(number: u32) -> String {
    format!("[Episode {number}]({REPO}/blob/main/docs/transcripts/{number:03}.md)")
}

/// Where a promise's work happens (its issues) and where it was promised
/// (its episodes), as sentences.
fn sources(promise: &Promise) -> String {
    let mut out = String::new();
    if !promise.issues.is_empty() {
        let issues: Vec<String> = promise.issues.iter().map(|n| issue_link(*n)).collect();
        out.push_str(&format!(" Follow it: {}.", issues.join(", ")));
    }
    if !promise.episodes.is_empty() {
        let episodes: Vec<String> = promise.episodes.iter().map(|n| episode_link(*n)).collect();
        out.push_str(&format!(" Promised in {}.", episodes.join(", ")));
    }
    out
}

/// The promise's note, as a sentence after its statement.
fn note(promise: &Promise) -> String {
    promise
        .note
        .as_ref()
        .map(|note| format!(" {note}"))
        .unwrap_or_default()
}

fn surfaces(promise: &Promise) -> String {
    promise
        .surfaces
        .iter()
        .map(|surface| surface.label())
        .collect::<Vec<_>>()
        .join(", ")
}

/// The one way to try a promise: its link, or its command.
fn try_it(promise: &Promise) -> String {
    match (&promise.link, &promise.command) {
        (Some(link), _) => format!("[Try it]({link})"),
        (None, Some(command)) => format!("Try it: `{command}`"),
        (None, None) => String::new(),
    }
}

/// The lead paragraph of `/promises`.
const PROMISES_LEAD: &str = "Everything here works today, and each line links to the check \
that proves it.";

/// The lead paragraph of `/roadmap`.
const ROADMAP_LEAD: &str = "What we're building next, what comes after, and what we set aside. \
Each line links to its issue on GitHub, where the work happens in the open, or to the episode \
where we promised it.";

/// `/promises` as Markdown: each group's working promises, with where it
/// works, how to try it, and its proof.
pub(crate) fn promises_markdown() -> String {
    let registry = registry();
    let working: Vec<&Promise> = registry
        .promise
        .iter()
        .filter(|p| p.status.works())
        .collect();
    let mut out = format!(
        "# What works today\n\n{PROMISES_LEAD} {} things work today; what comes next is on \
the [roadmap](/roadmap).\n",
        working.len()
    );
    for group in &registry.group {
        let items: Vec<&&Promise> = working.iter().filter(|p| p.group == group.id).collect();
        if items.is_empty() {
            continue;
        }
        out.push_str(&format!("\n## {}\n\n", group.title));
        for promise in items {
            let new = match promise.status {
                Status::Launching => " New in 1.0.",
                Status::Partial => " Works in part.",
                _ => "",
            };
            let proof = promise
                .evidence
                .iter()
                .map(|evidence| {
                    let (text, href) = evidence.link();
                    format!("[{text}]({href})")
                })
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!(
                "- **{}**{} {}.{new} {}. Proof: {proof}.\n",
                promise.statement,
                note(promise),
                surfaces(promise),
                try_it(promise)
            ));
        }
    }
    out.push_str(&format!(
        "\nSomething here doesn't work for you? [Tell us on GitHub]({REPO}/issues/new).\n"
    ));
    out
}

/// `/roadmap` as Markdown: next, later, and what was dropped on purpose,
/// each grouped, each linked to its issues and the episodes that
/// promised it.
pub(crate) fn roadmap_markdown() -> String {
    let registry = registry();
    let mut out = format!(
        "# Roadmap\n\n{ROADMAP_LEAD} What works today is on [What works today](/promises).\n"
    );
    for (status, title) in [
        (Status::Next, "Next"),
        (Status::Later, "Later"),
        (Status::Dropped, "Dropped on purpose"),
    ] {
        if !registry.promise.iter().any(|p| p.status == status) {
            continue;
        }
        out.push_str(&format!("\n## {title}\n"));
        for group in &registry.group {
            let items: Vec<&Promise> = registry
                .promise
                .iter()
                .filter(|p| p.status == status && p.group == group.id)
                .collect();
            if items.is_empty() {
                continue;
            }
            out.push_str(&format!("\n### {}\n\n", group.title));
            for promise in items {
                out.push_str(&format!(
                    "- **{}**{} {}.{}\n",
                    promise.statement,
                    note(promise),
                    surfaces(promise),
                    sources(promise)
                ));
            }
        }
    }
    out.push_str(&format!(
        "\nWant something that isn't here? [Ask for it on GitHub]({REPO}/issues/new).\n"
    ));
    out
}

fn page(title: &str, path: &str, lead: &str, source: &str, other: (&str, &str)) -> UiPage {
    let content = PageColumn::new(html! {
        (MarkdownRoot::new(PreEscaped(markdown::render_document(source))))
        nav.oa-page-actions aria-label="More" {
            (action_link(other.0, other.1))
            (action_link("Docs", "/docs"))
        }
    });
    UiPage::new(title)
        .path(path)
        .description(lead)
        .scriptless()
        .content(content)
}

async fn promises_page(headers: HeaderMap) -> Response {
    page(
        "What works today",
        "/promises",
        PROMISES_LEAD,
        &promises_markdown(),
        ("Roadmap", "/roadmap"),
    )
    .respond(&headers)
}

async fn roadmap_page(headers: HeaderMap) -> Response {
    page(
        "Roadmap",
        "/roadmap",
        ROADMAP_LEAD,
        &roadmap_markdown(),
        ("What works today", "/promises"),
    )
    .respond(&headers)
}

async fn promises_md() -> Response {
    crate::agent_ready::doc_md("/promises", &promises_markdown(), "What works today")
}

async fn roadmap_md() -> Response {
    crate::agent_ready::doc_md("/roadmap", &roadmap_markdown(), "Roadmap")
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::{Path, PathBuf};

    use super::*;

    fn repo() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn read(path: &str) -> String {
        std::fs::read_to_string(repo().join(path)).unwrap_or_default()
    }

    /// The registry parses, ids are unique, every promise sits in a group
    /// that exists, every group is used, and every promise is in the shape
    /// its status needs: a working one has a way to try it, a planned one
    /// has an issue.
    #[test]
    fn the_registry_is_well_formed() {
        let registry: Registry = toml::from_str(SOURCE).expect("the registry parses");
        let groups: HashSet<&str> = registry.group.iter().map(|g| g.id.as_str()).collect();
        assert_eq!(groups.len(), registry.group.len(), "group ids are unique");
        let mut ids = HashSet::new();
        for promise in &registry.promise {
            let id = &promise.id;
            assert!(ids.insert(id.as_str()), "{id} is listed twice");
            assert!(
                groups.contains(promise.group.as_str()),
                "{id}: no such group"
            );
            assert!(!promise.surfaces.is_empty(), "{id} names where it works");
            if promise.status.works() {
                assert!(
                    promise.link.is_some() || promise.command.is_some(),
                    "{id}: a working promise says where to try it"
                );
            } else {
                assert!(
                    !promise.issues.is_empty() || !promise.episodes.is_empty(),
                    "{id}: a planned or dropped promise names its issue or episode"
                );
                assert!(
                    promise.status != Status::Dropped || promise.note.is_some(),
                    "{id}: a dropped promise says why"
                );
                assert!(
                    promise.evidence.is_empty(),
                    "{id}: evidence belongs to a working promise"
                );
            }
            if let Some(link) = &promise.link {
                assert!(
                    link.starts_with('/') || link.starts_with("https://"),
                    "{id}: {link}"
                );
            }
        }
        for group in &registry.group {
            assert!(
                registry.promise.iter().any(|p| p.group == group.id),
                "{} has no promises",
                group.id
            );
        }
    }

    /// Each statement is one short plain sentence: it ends with a full
    /// stop, has no second sentence, stays short, carries no machine talk,
    /// and names no internal piece of the system.
    #[test]
    fn every_statement_is_one_short_plain_sentence() {
        const INTERNAL: [&str; 10] = [
            "Jev", "ATIF", "ACP", "NIP", "x402", "Psionic", "gateway", "registry", "Pylon",
            "golden",
        ];
        for promise in &registry().promise {
            let text = &promise.statement;
            let id = &promise.id;
            assert!(text.ends_with('.'), "{id}: ends with a full stop");
            assert!(
                !text.trim_end_matches('.').contains(". "),
                "{id}: one sentence"
            );
            assert!(text.len() <= 120, "{id}: {} characters", text.len());
            let hits = oa_copy::violations(text, &[]);
            assert!(hits.is_empty(), "{id}: machine talk {hits:?}");
            for word in INTERNAL {
                assert!(!text.contains(word), "{id} names {word}");
            }
        }
    }

    /// The rule that keeps the pages honest: every promise on `/promises`
    /// names at least one piece of evidence, and each one exists. A test
    /// is a `fn` of that name in that file; a live check is a check name
    /// in the smoke suite; a chat check is a golden id in the set; a
    /// document is a file. Remove or rename the evidence and this fails
    /// until the promise is updated.
    #[test]
    fn every_working_promise_names_evidence_that_exists() {
        let smoke = read("scripts/smoke/web.py");
        let staging = read("scripts/smoke/staging.sh");
        assert!(staging.contains("web.py"), "staging.sh runs the web suite");
        let goldens = read("bench/web-chat/goldens-v1.json");
        let mut missing = Vec::new();
        for promise in registry().promise.iter().filter(|p| p.status.works()) {
            let id = &promise.id;
            if promise.evidence.is_empty() {
                missing.push(format!("{id}: names no evidence"));
            }
            for evidence in &promise.evidence {
                let found = match evidence {
                    Evidence::Test(spec) => spec.split_once("::").is_some_and(|(path, name)| {
                        let source = read(path);
                        source.contains(&format!("fn {name}("))
                            && (source.contains("#[test]") || source.contains("#[tokio::test"))
                    }),
                    Evidence::Smoke(name) => {
                        smoke.contains(&format!("\"{name}\""))
                            || smoke.contains(&format!("f\"{name}\""))
                    }
                    Evidence::Golden(golden) => goldens.contains(&format!("\"id\": \"{golden}\"")),
                    Evidence::Doc(path) => repo().join(path).is_file(),
                };
                if !found {
                    missing.push(format!("{id}: {evidence:?}"));
                }
            }
        }
        assert!(
            missing.is_empty(),
            "promises whose evidence is missing (update the promise in content/promises.toml, \
see docs/promises/README.md):\n{}",
            missing.join("\n")
        );
    }

    /// The pages are the registry: every working promise is on
    /// `/promises` with its try-it and its proof, every planned one on
    /// `/roadmap` with its issues, and neither page shows the other's.
    #[test]
    fn the_pages_render_from_the_registry() {
        let promises = promises_markdown();
        let roadmap = roadmap_markdown();
        for promise in &registry().promise {
            let line = format!("**{}**", promise.statement);
            assert_eq!(
                promises.contains(&line),
                promise.status.works(),
                "{}",
                promise.id
            );
            assert_eq!(
                roadmap.contains(&line),
                !promise.status.works(),
                "{}",
                promise.id
            );
            if promise.status.works() {
                assert!(promises.contains(&try_it(promise)), "{}", promise.id);
            }
            if !promise.status.works() {
                for number in &promise.issues {
                    assert!(roadmap.contains(&issue_link(*number)), "{}", promise.id);
                }
                for number in &promise.episodes {
                    assert!(roadmap.contains(&episode_link(*number)), "{}", promise.id);
                }
            }
        }
        assert!(promises.contains("](/roadmap)") && roadmap.contains("](/promises)"));
    }

    /// The Kitchen Sink ledger's words (#11125) read into the registry:
    /// `live` is shipped, `missing` is later, `partial` and `dropped` are
    /// their own, the Verse is a surface, and a promise can name the
    /// episodes that made it and a note.
    #[test]
    fn the_kitchen_sink_ledger_vocabulary_parses() {
        let ledger = r#"
[[group]]
id = "verse"
title = "The Verse"

[[promise]]
id = "a"
group = "verse"
statement = "Walk the Verse."
surfaces = ["verse"]
status = "live"
link = "/grid"
evidence = [{ doc = "LICENSE" }]

[[promise]]
id = "b"
group = "verse"
statement = "Battle in the Verse."
surfaces = ["verse", "desktop"]
status = "partial"
note = "Only on a Mac."
link = "/grid"
evidence = [{ doc = "LICENSE" }]

[[promise]]
id = "c"
group = "verse"
statement = "Train a model together."
surfaces = ["verse"]
status = "missing"
episodes = [236]

[[promise]]
id = "d"
group = "verse"
statement = "Earn bitcoin for training."
surfaces = ["verse"]
status = "dropped"
note = "We stopped paying for training."
episodes = [235, 236]
"#;
        let registry: Registry = toml::from_str(ledger).expect("the ledger parses");
        let statuses: Vec<Status> = registry.promise.iter().map(|p| p.status).collect();
        assert_eq!(
            statuses,
            [
                Status::Shipped,
                Status::Partial,
                Status::Later,
                Status::Dropped
            ]
        );
        assert!(registry.promise[1].status.works() && !registry.promise[3].status.works());
        assert_eq!(registry.promise[3].episodes, [235, 236]);
        assert!(episode_link(5).contains("docs/transcripts/005.md"));
    }

    /// Advice is always a link or a command (#11098): every line of both
    /// pages that tells the reader to try something carries one.
    #[test]
    fn every_line_carries_a_link_or_a_command() {
        for source in [promises_markdown(), roadmap_markdown()] {
            for line in source.lines().filter(|l| l.starts_with("- ")) {
                assert!(line.contains("](") || line.contains('`'), "{line}");
            }
        }
    }
}
