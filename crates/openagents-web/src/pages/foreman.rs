//! `/agent/foreman`: Foreman as Agent Lab defines it, drawn as one map.
//!
//! Foreman (#11926) is the agent that keeps the whole picture of open
//! issues and active agents. This page draws its definition: the runtime
//! and model, its instructions and playbook, its tools, where it looks, its
//! check-ins, its grants, its memory, its stall ladder, the suites that test
//! it, and what we measure it by. Every value comes from the definition this
//! build carries (`agent_lab::templates::foreman()`, its playbook files under
//! `crates/agent-lab/src/templates/foreman/`, `agent_contract`'s grants, and
//! the 2026-10-17 replay fixture), so the page changes when Foreman does.
//! A tool or command is listed only while Foreman's text names it.
//!
//! The map is an inline SVG drawn on the server; hover and focus are CSS
//! (`agent-map.css` in `openagents-ui`). The page runs no script and has
//! no controls.

use axum::Router;
use axum::extract::{Path as UrlPath, State};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use maud::{Markup, PreEscaped, Render, html};
use openagents_ui::content::{MarkdownRoot, PageColumn};

use agent_contract::{GrantScope, NEVER_GRANTED_REFUSAL};
use agent_graph::catalog::{self, MACHINE_SCOPES, named, plain};
use agent_graph::foreman::{LESSONS, PART_LESSONS, lesson_title, replay_lessons, scenario_title};
use agent_graph::lab::{
    DEFAULT_WAKES, HOST_STALL_MINUTES, grant_words as grant, minutes, wake_words as wake,
};
use agent_lab::{Check, Experiment, replay, templates};

use super::first_frame::FirstFrame;
use crate::App;
use crate::ui_page::UiPage;

pub(crate) const PATH: &str = "/agent/foreman";

const REPO: &str = "https://github.com/OpenAgentsInc/openagents";

/// The AgentGraph web build (`apps/openagents/scripts/build-web-demo.sh
/// --app agent-graph`), read from `--agent-graph DIR`: the glue, the module,
/// and the start script, served under [`GRAPH_ASSETS`].
pub(crate) const GRAPH_GLUE: &str = "openagents_web_agent_graph.js";
pub(crate) const GRAPH_WASM: &str = "openagents_web_agent_graph_bg.wasm";
pub(crate) const GRAPH_START: &str = "start.js";
pub(crate) const GRAPH_ASSETS: &str = "/agent-graph";

/// An agent page with the graph: the site's policy, plus scripts and the
/// module from this site and compiling WebAssembly. GPUI styles its canvas
/// through the CSSOM, which `style-src` doesn't govern.
pub(crate) const GRAPH_POLICY: &str = "default-src 'none'; style-src 'self'; font-src 'self'; \
img-src 'self'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self'; \
base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(PATH, get(foreman_page))
        .route("/agent/{id}", get(agent_page))
        .route("/agent/{id}/graph.json", get(graph_json))
        .route("/agent/{id}/definition.json", get(definition_json))
        .route("/agent-graph/{file}", get(graph_file))
}

/// Whether this site answers `path`: Foreman's page, a template's page or
/// graph, or the graph's build files. Other `/agent/` paths go upstream.
pub(crate) fn owns(path: &str) -> bool {
    if path == PATH || path.starts_with("/agent-graph/") {
        return true;
    }
    let Some(rest) = path.strip_prefix("/agent/") else {
        return false;
    };
    let id = rest
        .strip_suffix("/graph.json")
        .or_else(|| rest.strip_suffix("/definition.json"))
        .unwrap_or(rest);
    !id.contains('/') && (id == "foreman" || template(id).is_some())
}

/// The build directory, when it holds the glue, the module and the start
/// script.
fn graph_build(app: &App) -> Option<&std::path::Path> {
    let directory = app.config.agent_graph.as_deref()?;
    [GRAPH_GLUE, GRAPH_WASM, GRAPH_START]
        .iter()
        .all(|file| directory.join(file).is_file())
        .then_some(directory)
}

async fn foreman_page(State(app): State<App>, headers: HeaderMap) -> Response {
    live_page(&headers, graph_build(&app))
}

/// The page, with the GPUI graph over the server map when `live`.
pub(crate) fn page(headers: &HeaderMap, live: bool) -> Response {
    live_page_with(headers, live, None)
}

/// The page for a server with the AgentGraph `build`, if any: the graph,
/// and its first frame when the build has one for this graph.
fn live_page(headers: &HeaderMap, build: Option<&std::path::Path>) -> Response {
    let first = build.and_then(|build| {
        let json = serde_json::to_string(&agent_graph::foreman::graph()).ok()?;
        FirstFrame::cached(build, &json)
    });
    live_page_with(headers, build.is_some(), first.as_deref())
}

fn live_page_with(headers: &HeaderMap, live: bool, first: Option<&FirstFrame>) -> Response {
    let ui = UiPage::new("Foreman").path(PATH).description(
        "Foreman, the agent that keeps the whole picture of OpenAgents work, \
         as Agent Lab defines it.",
    );
    if !live {
        return ui.scriptless().content(body(None, None)).respond(headers);
    }
    let graph = agent_graph::foreman::graph();
    let frame = first.and_then(|f| f.class(headers).map(|class| f.markup(class)));
    let policy = match (first, &frame) {
        (Some(first), Some(_)) => first.policy(GRAPH_POLICY),
        _ => GRAPH_POLICY.to_owned(),
    };
    let html = ui
        .head(graph_script())
        .content(body(Some(&graph), frame))
        .render(headers)
        .into_string();
    let mut response = compressed(headers, html);
    if let Ok(policy) = HeaderValue::from_str(&policy) {
        response
            .headers_mut()
            .insert(header::CONTENT_SECURITY_POLICY, policy);
    }
    response
}

/// The page, brotli-compressed when the request accepts it. The live page
/// carries the graph, and with a first frame the painter and the frame, as
/// text; compressed, that's a fraction of the bytes before first paint.
fn compressed(headers: &HeaderMap, html: String) -> Response {
    use std::io::Write;
    if super::everglade::accepts(headers, "br") {
        let mut out = Vec::with_capacity(html.len() / 3);
        let written = {
            let mut writer = brotli::CompressorWriter::new(&mut out, 16 * 1024, 6, 22);
            writer
                .write_all(html.as_bytes())
                .and_then(|_| writer.flush())
        };
        if written.is_ok() {
            return (
                [
                    (header::CONTENT_TYPE, "text/html; charset=utf-8"),
                    (header::CONTENT_ENCODING, "br"),
                    (header::VARY, "accept-encoding"),
                ],
                out,
            )
                .into_response();
        }
    }
    (
        [(header::VARY, "accept-encoding")],
        axum::response::Html(html),
    )
        .into_response()
}

/// The start script, which loads the module and hands it the graph.
fn graph_script() -> Markup {
    html! { script type="module" src=(format!("{GRAPH_ASSETS}/{GRAPH_START}")) {} }
}

/// A graph's JSON, safe inside a `<script type="application/json">`: every
/// `<`, `>` and `&` is a JSON escape, so no text can close the element.
fn graph_data(graph: &agent_graph::GraphSpec) -> Markup {
    let json = serde_json::to_string(graph)
        .unwrap_or_default()
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026");
    html! { script type="application/json" id="agent-graph-spec" { (PreEscaped(json)) } }
}

/// The box the graph draws in, holding the server's own view until it does,
/// and then the graph's first frame when the page carries one.
fn graph_box(
    id: &str,
    graph: Option<&agent_graph::GraphSpec>,
    fallback: Markup,
    first: Option<Markup>,
) -> Markup {
    match graph {
        None => html! {
            (tools(id, false))
            (fallback)
        },
        Some(graph) => html! {
            div.oa-agent-graph id="gpui-mount" role="img"
                aria-label=(format!("{} as a graph of its parts. Every part is described below.", graph.title)) {
                (tools(id, true))
                (fallback)
            }
            (graph_data(graph))
            @if let Some(first) = first { (first) }
        },
    }
}

/// The graph's toolbar. Download JSON is a plain link to the agent's whole
/// definition, so it works without script; full screen and Copy JSON need
/// the start script, which shows them.
fn tools(id: &str, live: bool) -> Markup {
    let href = format!("/agent/{id}/definition.json");
    html! {
        div.oa-agent-graph-tools role="toolbar" aria-label="Graph tools" {
            @if live {
                button.oa-agent-graph-tool type="button" data-graph-action="fullscreen" hidden { "Full screen" }
                button.oa-agent-graph-tool type="button" data-graph-action="copy"
                    data-graph-definition=(href) hidden { "Copy JSON" }
            }
            a.oa-agent-graph-tool href=(href) download=(agent_graph::definition::file_name(id)) { "Download JSON" }
            @if live {
                span.oa-agent-graph-status aria-live="polite" {}
            }
        }
    }
}

/// A template's whole definition, Foreman's with its notes.
fn template_definition(
    published: &str,
    experiment: &Experiment,
) -> agent_graph::definition::Definition {
    if published == templates::FOREMAN_ID {
        agent_graph::foreman::definition()
    } else {
        agent_graph::definition::of(
            experiment,
            &slug(&experiment.title),
            published,
            &agent_graph::lab::Notes::default(),
        )
    }
}

/// `/agent/{id}/definition.json`: a template's whole definition
/// (`openagents.agent-definition-export`, version 1).
async fn definition_json(UrlPath(id): UrlPath<String>) -> Response {
    let Some((published, experiment)) = template(&id) else {
        return crate::not_found().await;
    };
    let definition = template_definition(published, &experiment);
    let mut response = axum::Json(&definition).into_response();
    if let Ok(value) = HeaderValue::from_str(&format!(
        "inline; filename=\"{}\"",
        agent_graph::definition::file_name(&definition.id)
    )) {
        response
            .headers_mut()
            .insert(header::CONTENT_DISPOSITION, value);
    }
    response
}

/// An Agent Lab template by its published ID or its name.
fn template(id: &str) -> Option<(&'static str, Experiment)> {
    templates::offered()
        .into_iter()
        .find(|(published, experiment)| *published == id || slug(&experiment.title) == id)
}

fn slug(title: &str) -> String {
    title.to_lowercase().replace(' ', "-")
}

/// The graph of a template, Foreman's with its notes.
fn template_graph(published: &str, experiment: &Experiment) -> agent_graph::GraphSpec {
    if published == templates::FOREMAN_ID {
        agent_graph::foreman::graph()
    } else {
        agent_graph::lab::of(experiment, published, &agent_graph::lab::Notes::default())
    }
}

/// `/agent/{id}`: any other Agent Lab template, as its graph over a plain
/// list of its parts. Foreman has its own page.
async fn agent_page(
    State(app): State<App>,
    UrlPath(id): UrlPath<String>,
    headers: HeaderMap,
) -> Response {
    let Some((published, experiment)) = template(&id) else {
        return crate::not_found().await;
    };
    if published == templates::FOREMAN_ID {
        return Redirect::permanent(PATH).into_response();
    }
    let graph = template_graph(published, &experiment);
    let path = format!("/agent/{id}");
    let ui = UiPage::new(graph.title.clone())
        .path(path)
        .description(graph.description.clone());
    let live = graph_build(&app).is_some();
    let content = generic_body(&id, &graph, live);
    if !live {
        return ui.scriptless().content(content).respond(&headers);
    }
    let mut response = ui.head(graph_script()).content(content).respond(&headers);
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(GRAPH_POLICY),
    );
    response
}

/// Any graph as a page: its parts and what each one does, in order.
fn generic_body(id: &str, graph: &agent_graph::GraphSpec, live: bool) -> Markup {
    let parts = html! {
        div.oa-agent-map-cards {
            @for unit in graph.children(None) {
                article.oa-agent-map-card id=(format!("part-{}", unit.id)) data-accent=(accent(unit.accent)) {
                    h3 { (unit.title) }
                    (MarkdownRoot::new(html! {
                        @if !unit.line.is_empty() { p.oa-page-meta { (unit.line) } }
                        @for detail in &unit.details { (detail_markup(detail)) }
                        @let inside: Vec<_> = graph.children(Some(&unit.id)).collect();
                        @if !inside.is_empty() {
                            ul.oa-agent-map-list {
                                @for child in inside { li { strong { (child.title) } " " (child.line) } }
                            }
                        }
                    }))
                }
            }
        }
    };
    PageColumn::new(html! {
        section.oa-agent-map aria-labelledby="agent-title" {
            (MarkdownRoot::new(html! {
                p.oa-page-eyebrow { "Agent Lab" }
                h1 id="agent-title" { (graph.title) }
                p.oa-page-lead { (graph.description) }
            }))
            (graph_box(id, live.then_some(graph), html! {}, None))
            (parts)
        }
    })
    .wide()
    .render()
}

fn accent(accent: agent_graph::Accent) -> &'static str {
    match accent {
        agent_graph::Accent::Core | agent_graph::Accent::Info => "info",
        agent_graph::Accent::Discovery => "discovery",
        agent_graph::Accent::Warning => "warning",
        agent_graph::Accent::Success => "success",
        agent_graph::Accent::Danger => "danger",
    }
}

fn detail_markup(detail: &agent_graph::Detail) -> Markup {
    use agent_graph::Detail;
    match detail {
        Detail::Text { text } => html! { p { (text) } },
        Detail::Quote { text } => html! { p.oa-agent-map-quote { (text) } },
        Detail::Fact { label, value } => html! {
            dl.oa-agent-map-facts { div { dt { (label) } dd { (value) } } }
        },
        Detail::Chips { label, items } => html! {
            @if let Some(label) = label { h4 { (label) } }
            ul.oa-agent-map-chips { @for item in items { li { (item) } } }
        },
        Detail::List { label, items } => html! {
            @if let Some(label) = label { h4 { (label) } }
            ul.oa-agent-map-list { @for item in items { li { (item) } } }
        },
    }
}

/// `/agent/{id}/graph.json`: a template's graph (`openagents.agent-graph.v1`).
async fn graph_json(UrlPath(id): UrlPath<String>) -> Response {
    let found = template(&id)
        .or_else(|| (id == "foreman").then(|| (templates::FOREMAN_ID, templates::foreman())));
    let Some((published, experiment)) = found else {
        return crate::not_found().await;
    };
    axum::Json(template_graph(published, &experiment)).into_response()
}

async fn graph_file(
    State(app): State<App>,
    UrlPath(file): UrlPath<String>,
    request: HeaderMap,
) -> Response {
    let (Some(directory), Some(content_type)) =
        (graph_build(&app), super::everglade::build_type(&file))
    else {
        return crate::not_found().await;
    };
    super::everglade::serve_build(directory, &file, content_type, &request).await
}

/// All of Foreman's own text: its instructions and every playbook skill.
fn foreman_text() -> String {
    agent_graph::lab::text(&templates::foreman())
}

/// A line of Foreman's Markdown as markup: `code` spans kept, `**` dropped.
fn inline(text: &str) -> Markup {
    let text = text.replace("**", "");
    html! {
        @for (at, part) in text.split('`').enumerate() {
            @if at % 2 == 1 { code { (part) } } @else { (part) }
        }
    }
}

/// One part of Foreman around the center of the map.
struct Part {
    id: &'static str,
    title: &'static str,
    /// The short line under the title on the map.
    line: String,
    /// The accent color's name in `agent-map.css`.
    accent: &'static str,
    lessons: Vec<String>,
    card: Markup,
}

/// The lessons a part answers (`agent_graph::foreman::PART_LESSONS`), so the
/// map and the graph tag the same parts.
fn lessons(part: &str) -> Vec<String> {
    PART_LESSONS
        .iter()
        .find(|(id, _)| *id == part)
        .map(|(_, numbers)| numbers.iter().map(|n| (*n).to_owned()).collect())
        .unwrap_or_default()
}

fn parts(foreman: &Experiment) -> Vec<Part> {
    let text = foreman_text();
    let spec = &foreman.draft.spec;
    let autonomy = spec.autonomy.clone().unwrap_or_default();
    let stall = autonomy.stall_minutes.unwrap_or(HOST_STALL_MINUTES);
    let model = spec
        .model
        .clone()
        .unwrap_or_else(|| "the runtime's own".into());
    let fallbacks: Vec<String> = spec.fallbacks.iter().map(|f| f.runtime.clone()).collect();
    let access = match spec.access {
        agent_contract::Access::HostDefault => "What the computer allows by default",
        agent_contract::Access::ReadOnly => "Read only",
        agent_contract::Access::WorkspaceWrite => "It may write in its project",
    };

    // The instructions' first paragraph is who Foreman says it is; the
    // first bold line after it is its rule for work.
    let paragraphs: Vec<&str> = templates::FOREMAN_INSTRUCTIONS
        .split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    let identity = paragraphs.first().copied().unwrap_or("");
    let rule = paragraphs
        .iter()
        .find_map(|p| p.strip_prefix("**")?.split_once("**").map(|(r, _)| r))
        .unwrap_or("");
    let words = text.split_whitespace().count();

    let tools = catalog::tools(&text);
    let tool_count: usize = tools.iter().map(|(_, names)| names.len()).sum();
    let commands = catalog::commands(&text);
    let scopes: Vec<&str> = MACHINE_SCOPES
        .iter()
        .copied()
        .filter(|s| text.contains(s))
        .collect();
    let sources = catalog::sources(&text);

    let skill = |name: &str| {
        templates::FOREMAN_SKILLS
            .iter()
            .find(|(n, _)| *n == name)
            .map_or("", |(_, t)| *t)
    };
    let working = skill("Check-ins").lines().find_map(|l| {
        l.strip_prefix("- ")
            .filter(|l| l.starts_with("My working state"))
    });
    let rungs: Vec<&str> = skill("Stalls")
        .lines()
        .filter_map(|l| {
            let (n, rest) = l.split_once(". ")?;
            n.parse::<u8>().ok().map(|_| rest)
        })
        .collect();
    let responsive: Vec<&str> = skill("Staying responsive")
        .lines()
        .filter_map(|l| l.strip_prefix("- "))
        .take(1)
        .collect();

    let day = replay::day();
    let replay_lessons = replay_lessons();
    let automatic = foreman
        .suite
        .cases
        .iter()
        .filter(|c| !matches!(c.check, Check::Review { .. }))
        .count();

    vec![
        Part {
            id: "runtime",
            title: "Runtime and model",
            line: model.clone(),
            accent: "info",
            lessons: lessons("runtime"),
            card: html! {
                dl.oa-agent-map-facts {
                    div { dt { "Runtime" } dd { code { (spec.runtime) } } }
                    div { dt { "Model" } dd { code { (model) } } }
                    div {
                        dt { "If the runtime isn't ready" }
                        dd {
                            @if fallbacks.is_empty() {
                                "No fallback: it runs only where its runtime is ready."
                            } @else {
                                "Tries " (fallbacks.join(", ")) "."
                            }
                        }
                    }
                    div { dt { "Access" } dd { (access) } }
                    @if let Some(secs) = autonomy.shell_timeout_secs {
                        div { dt { "Longest shell call" } dd { (minutes(u64::from(secs))) } }
                    }
                }
                @for line in &responsive { p.oa-agent-map-quote { (inline(line)) } }
                p {
                    "It runs as a chat in the OpenAgents app, on a computer you choose, \
                     so the owner can watch and steer it from the desktop, the phone or the web."
                }
            },
        },
        Part {
            id: "instructions",
            title: "Instructions",
            line: format!("{} skills", templates::FOREMAN_SKILLS.len()),
            accent: "discovery",
            lessons: lessons("instructions"),
            card: html! {
                p.oa-agent-map-quote { (inline(identity)) }
                @if !rule.is_empty() { p { strong { (rule) } } }
                p.oa-page-meta { (words) " words in all." }
                ol.oa-agent-map-skills {
                    @for (name, body) in templates::FOREMAN_SKILLS {
                        li {
                            details {
                                summary { (plain(name)) }
                                pre { (body.trim_end()) }
                            }
                        }
                    }
                }
            },
        },
        Part {
            id: "tools",
            title: "Tools",
            line: format!("{tool_count} app tools"),
            accent: "warning",
            lessons: lessons("tools"),
            card: html! {
                @for (group, names) in &tools {
                    h4 { (group) }
                    ul.oa-agent-map-chips {
                        @for name in names { li { code { (name) } } }
                    }
                }
                @if !commands.is_empty() {
                    h4 { "Commands it runs" }
                    ul.oa-agent-map-chips {
                        @for c in &commands { li { code { (c) } } }
                    }
                }
            },
        },
        Part {
            id: "sources",
            title: "Where I look",
            line: format!("{} places", sources.len()),
            accent: "success",
            lessons: lessons("sources"),
            card: html! {
                ul.oa-agent-map-list {
                    @for (place, what, via) in &sources {
                        li {
                            strong { (place) } " " (what) " "
                            span.oa-page-meta {
                                @for (at, v) in via.iter().filter(|v| named(&text, v)).enumerate() {
                                    @if at > 0 { ", " }
                                    code { (v) }
                                }
                            }
                        }
                    }
                }
                p {
                    "It gathers fresh facts before every answer, and says plainly what it \
                     couldn't reach instead of guessing."
                }
            },
        },
        Part {
            id: "check-ins",
            title: "Check-ins",
            line: autonomy
                .every_minutes
                .map_or_else(|| "when asked".to_owned(), |m| format!("every {m} minutes")),
            accent: "info",
            lessons: lessons("check-ins"),
            card: html! {
                dl.oa-agent-map-facts {
                    div {
                        dt { "On a schedule" }
                        dd {
                            @if autonomy.check_ins {
                                @match autonomy.every_minutes {
                                    Some(m) => { "Every " (m) " minutes" }
                                    None => { "On the computer's schedule" }
                                }
                            } @else { "Off" }
                        }
                    }
                    @if let Some(secs) = autonomy.coalesce_secs {
                        div { dt { "Waits for quiet" } dd { (minutes(u64::from(secs))) } }
                    }
                }
                p { "It also wakes when:" }
                ul.oa-agent-map-chips {
                    @if autonomy.wakes.is_empty() {
                        @for w in DEFAULT_WAKES { li { (wake(w)) } }
                    } @else {
                        @for w in &autonomy.wakes { li { (wake(*w)) } }
                    }
                }
                p {
                    "The computer, not Foreman's own process, keeps the schedule, so a \
                     restart doesn't lose it. The owner turns check-ins off in Settings."
                }
            },
        },
        Part {
            id: "authority",
            title: "Authority",
            line: format!("{} grants", GrantScope::ALL.len()),
            accent: "danger",
            lessons: lessons("authority"),
            card: html! {
                p {
                    @if autonomy.authority {
                        "Foreman asks for standing authority, so it acts on its check-ins \
                         without asking first. Without a grant it only looks and reports."
                    } @else {
                        "Foreman acts only when someone asks."
                    }
                }
                ul.oa-agent-map-chips {
                    @for scope in GrantScope::ALL {
                        li title=(scope.word()) { (grant(scope)) }
                    }
                }
                @if !scopes.is_empty() {
                    p {
                        "Machine actions need their own grant: "
                        @for (at, s) in scopes.iter().enumerate() {
                            @if at > 0 { ", " }
                            code { (s) }
                        }
                        "."
                    }
                }
                p.oa-agent-map-never { (NEVER_GRANTED_REFUSAL) }
            },
        },
        Part {
            id: "memory",
            title: "Memory",
            line: "a decision log".to_owned(),
            accent: "discovery",
            lessons: lessons("memory"),
            card: html! {
                p {
                    "Every action that changes something gets its own "
                    code { "record_decision" }
                    ", and every check-in ends with one more: what it saw, decided and did. \
                     It reads that log before acting, so it doesn't repeat work."
                }
                @if let Some(line) = working { p.oa-agent-map-quote { (inline(line)) } }
            },
        },
        Part {
            id: "stalls",
            title: "Stall ladder",
            line: format!("{} rungs", rungs.len()),
            accent: "warning",
            lessons: lessons("stalls"),
            card: html! {
                p {
                    "Work is stalled after " (stall)
                    " minutes with no output, commit, push, comment or build. Then it climbs:"
                }
                ol.oa-agent-map-ladder {
                    @for rung in &rungs { li { (inline(rung)) } }
                }
                p { "Any progress starts the ladder over." }
            },
        },
        Part {
            id: "tests",
            title: "Tested by",
            line: format!("{} cases", foreman.suite.cases.len() + day.scenarios.len()),
            accent: "success",
            lessons: replay_lessons,
            card: html! {
                h4 { (foreman.suite.title) }
                p.oa-page-meta {
                    (automatic) " checked automatically, "
                    (foreman.suite.cases.len() - automatic) " reviewed by a person. Each trial gets "
                    (minutes(foreman.max_seconds))
                    @if foreman.no_project_files { " and no project files" }
                    "."
                }
                ul.oa-agent-map-list {
                    @for case in &foreman.suite.cases {
                        li {
                            (case.title)
                            @if matches!(case.check, Check::Review { .. }) {
                                " " span.oa-agent-map-tag { "reviewed" }
                            }
                        }
                    }
                }
                h4 { "Replay of " (day.id) }
                p.oa-page-meta {
                    (day.scenarios.len()) " moments from the day we learned most from, each \
                     scored by its deadline. A new revision is saved only if every case passes."
                }
                ul.oa-agent-map-list {
                    @for s in &day.scenarios {
                        li {
                            (scenario_title(&s.title))
                            @for l in &s.lessons { " " span.oa-agent-map-tag title=(lesson_title(l)) { (l) } }
                        }
                    }
                }
            },
        },
        Part {
            id: "measures",
            title: "Measures",
            line: "time to close".to_owned(),
            accent: "danger",
            lessons: lessons("measures"),
            card: html! {
                dl.oa-agent-map-facts {
                    div { dt { "Code complete to close" } dd { "Minutes from the last push to a closed issue" } }
                    div { dt { "Stall minutes" } dd { "Minutes each chat sat with no progress" } }
                }
                p {
                    "On 2026-10-17, issues coordinated by hand took 4 h 47 min to 5 h 17 min \
                     from code complete to close. Foreman's six parallel chats closed #12018 \
                     in 21 minutes, #12022 in 33 and #12019 in 35."
                }
                p {
                    "Over 2026-10-18 and 19, 91 issues closed with a median of 2.3 hours from \
                     filing. Only 3 of 380 commits went through the landing queue, and after an \
                     account switch every Foreman turn ended empty for 35 minutes with no alarm."
                }
            },
        },
    ]
}

fn body(graph: Option<&agent_graph::GraphSpec>, first: Option<Markup>) -> Markup {
    let foreman = templates::foreman();
    let parts = parts(&foreman);
    let spec = &foreman.draft.spec;
    PageColumn::new(html! {
        section.oa-agent-map aria-labelledby="foreman-title" {
            (MarkdownRoot::new(html! {
                p.oa-page-eyebrow { "Agent Lab" }
                h1 id="foreman-title" { (spec.name) }
                p.oa-page-lead { (spec.description) }
                p {
                    "We built Foreman in Agent Lab. This map is drawn from the definition you \
                     start from when you fork it, so it shows what Foreman is today: what runs \
                     it, what it reads, what it may do, and how we test it."
                }
            }))
            (graph_box("foreman", graph, map(&parts, &spec.runtime, spec.model.as_deref().unwrap_or("")), first))
            div.oa-agent-map-cards {
                @for part in &parts {
                    article.oa-agent-map-card id=(format!("part-{}", part.id)) data-accent=(part.accent) {
                        h3 { (part.title) }
                        (MarkdownRoot::new(part.card.clone()))
                        @if !part.lessons.is_empty() {
                            p.oa-agent-map-lessons {
                                "Answers "
                                @for (at, l) in part.lessons.iter().enumerate() {
                                    @if at > 0 { ", " }
                                    span title=(lesson_title(l)) { (l) }
                                }
                            }
                        }
                    }
                }
            }
            (MarkdownRoot::new(html! {
                h2 { "What we learned" }
                p {
                    "Each part above answers a lesson from Foreman's first days. "
                    a href=(format!("{REPO}/blob/main/docs/foreman/lessons-learned.md")) { "Read the lessons" }
                    ", or "
                    a href="/labs/agent" { "open Agent Lab" }
                    " to fork Foreman."
                }
                ol.oa-agent-map-lesson-list {
                    @for (n, title) in LESSONS {
                        li { span.oa-agent-map-tag { (n) } " " (title) }
                    }
                }
            }))
        }
    })
    .wide()
    .render()
}

/// The map: Foreman in the middle, its playbook on the ring around it, and
/// each part on an outer ellipse, linked to its card below.
fn map(parts: &[Part], runtime: &str, model: &str) -> Markup {
    const CX: f64 = 550.0;
    const CY: f64 = 500.0;
    const RX: f64 = 410.0;
    const RY: f64 = 388.0;
    const RING: f64 = 178.0;
    const W: f64 = 236.0;
    const H: f64 = 88.0;
    let n = parts.len().max(1) as f64;
    let at = |i: usize, rx: f64, ry: f64| {
        // Start a half step past the top so no part sits on the vertical axis
        // and the widest pairs, side by side at the top and bottom, clear.
        let a = -std::f64::consts::FRAC_PI_2 + std::f64::consts::TAU * (i as f64 + 0.5) / n;
        (CX + rx * a.cos(), CY + ry * a.sin())
    };
    let skills = templates::FOREMAN_SKILLS.len().max(1) as f64;
    html! {
        figure.oa-agent-map-figure {
            svg viewBox="0 0 1100 1000" role="img"
                aria-label=(format!("Foreman at the center, with its {} parts around it", parts.len())) {
                ellipse.oa-agent-map-orbit cx=(CX) cy=(CY) rx=(RX) ry=(RY) {}
                circle.oa-agent-map-ring cx=(CX) cy=(CY) r=(RING) {}
                circle.oa-agent-map-ring-spin cx=(CX) cy=(CY) r=(RING + 32.0) {}
                @for (i, part) in parts.iter().enumerate() {
                    @let (x, y) = at(i, RX, RY);
                    @let (sx, sy) = at(i, RING + 32.0, RING + 32.0);
                    g.oa-agent-map-node data-accent=(part.accent) {
                        a href=(format!("#part-{}", part.id)) {
                            title { (part.title) ": " (part.line) }
                            line.oa-agent-map-spoke x1=(f(sx)) y1=(f(sy)) x2=(f(x)) y2=(f(y)) {}
                            circle.oa-agent-map-dot cx=(f(sx)) cy=(f(sy)) r="7" {}
                            rect.oa-agent-map-box x=(f(x - W / 2.0)) y=(f(y - H / 2.0))
                                width=(W) height=(H) rx="16" {}
                            rect.oa-agent-map-bar x=(f(x - W / 2.0)) y=(f(y - H / 2.0 + 16.0))
                                width="5" height=(f(H - 32.0)) rx="2" {}
                            text.oa-agent-map-title x=(f(x)) y=(f(y - 6.0)) text-anchor="middle" { (part.title) }
                            text.oa-agent-map-line x=(f(x)) y=(f(y + 24.0)) text-anchor="middle" { (part.line) }
                            @if !part.lessons.is_empty() {
                                text.oa-agent-map-badge x=(f(x + W / 2.0 - 10.0)) y=(f(y - H / 2.0 - 8.0))
                                    text-anchor="end" { (badge(&part.lessons)) }
                            }
                        }
                    }
                }
                @for (i, (name, _)) in templates::FOREMAN_SKILLS.iter().enumerate() {
                    @let a = -std::f64::consts::FRAC_PI_2
                        + std::f64::consts::TAU * i as f64 / skills;
                    g.oa-agent-map-skill {
                        title { "Playbook: " (plain(name)) }
                        circle cx=(f(CX + RING * a.cos())) cy=(f(CY + RING * a.sin())) r="11" {}
                        text x=(f(CX + RING * a.cos())) y=(f(CY + RING * a.sin() + 5.0))
                            text-anchor="middle" { (i + 1) }
                    }
                }
                circle.oa-agent-map-core-glow cx=(CX) cy=(CY) r="140" {}
                circle.oa-agent-map-core cx=(CX) cy=(CY) r="122" {}
                text.oa-agent-map-name x=(CX) y=(CY - 8.0) text-anchor="middle" { "Foreman" }
                text.oa-agent-map-runtime x=(CX) y=(CY + 28.0) text-anchor="middle" { (runtime) }
                text.oa-agent-map-runtime x=(CX) y=(CY + 54.0) text-anchor="middle" { (model) }
            }
            figcaption {
                "Foreman in the middle, its eight playbook skills on the ring, and each part \
                 of its definition around it. Each part links to its details below; the "
                "small labels name the lesson it answers."
            }
        }
    }
}

/// The lessons over a part on the map: the first three, then how many more.
fn badge(lessons: &[String]) -> String {
    let mut shown = lessons
        .iter()
        .take(3)
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    if lessons.len() > 3 {
        shown.push_str(&format!(" +{}", lessons.len() - 3));
    }
    shown
}

fn f(value: f64) -> String {
    format!("{value:.1}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_and_command_is_named_in_foremans_text() {
        let text = foreman_text();
        for (_, names) in catalog::TOOLS {
            for name in names {
                assert!(named(&text, name), "Foreman no longer names {name}");
            }
        }
        for command in catalog::COMMANDS {
            assert!(named(&text, command), "Foreman no longer runs {command}");
        }
        for (place, _, via) in catalog::SOURCES {
            assert!(via.iter().any(|v| named(&text, v)), "{place}");
        }
    }

    #[test]
    fn skill_names_read_plainly() {
        for (name, _) in templates::FOREMAN_SKILLS {
            let shown = plain(name);
            assert!(oa_copy::violations(&shown, &[]).is_empty(), "{shown}");
        }
        assert_eq!(plain("Dispatching issue work"), "Handing out issue work");
    }

    #[test]
    fn the_map_draws_every_part_from_the_definition() {
        let html = body(None, None).into_string();
        assert!(html.contains("<svg viewBox=\"0 0 1100 1000\""));
        let foreman = templates::foreman();
        assert!(html.contains(&foreman.draft.spec.runtime));
        assert!(html.contains(foreman.draft.spec.model.as_deref().unwrap()));
        assert_eq!(html.matches("class=\"oa-agent-map-node\"").count(), 10);
        assert_eq!(html.matches("class=\"oa-agent-map-skill\"").count(), 8);
        for case in &foreman.suite.cases {
            assert!(html.contains(&maud::html! { (case.title) }.into_string()));
        }
        assert!(html.contains("record_decision"));
        assert!(html.contains("wait_for_turn"));
        assert!(html.contains("href=\"/agent/foreman/definition.json\""));
        assert!(!html.contains("<form"));
        assert!(!html.contains("<button"));
        assert!(!html.contains("<script"));
        crate::copy_guard::assert_plain(PATH, &html);
    }

    #[test]
    fn the_live_page_hands_the_graph_to_the_canvas_and_keeps_the_map() {
        let graph = agent_graph::foreman::graph();
        let html = body(Some(&graph), None).into_string();
        assert!(html.contains("data-graph-action=\"fullscreen\""));
        assert!(html.contains("data-graph-action=\"copy\""));
        assert!(
            html.contains(
                "href=\"/agent/foreman/definition.json\" download=\"foreman.agent.json\""
            )
        );
        assert!(html.contains("id=\"gpui-mount\""));
        assert!(html.contains("<svg viewBox=\"0 0 1100 1000\""));
        let start =
            html.find("id=\"agent-graph-spec\">").unwrap() + "id=\"agent-graph-spec\">".len();
        let end = start + html[start..].find("</script>").unwrap();
        let json = &html[start..end];
        assert!(!json.contains('<') && !json.contains('&'));
        let back: agent_graph::GraphSpec = serde_json::from_str(json).unwrap();
        assert_eq!(back, graph);
        let script = graph_script().into_string();
        assert!(script.contains("src=\"/agent-graph/start.js\""));
        assert!(GRAPH_POLICY.contains("script-src 'self' 'wasm-unsafe-eval';"));
        assert!(!GRAPH_POLICY.contains("unsafe-inline"));
        crate::copy_guard::assert_plain(PATH, &html);
    }

    #[test]
    fn any_template_has_a_plain_graph_page() {
        let mut experiment = templates::foreman();
        experiment.draft.spec.name = "Helper".into();
        let graph =
            agent_graph::lab::of(&experiment, "helper", &agent_graph::lab::Notes::default());
        let html = generic_body("helper", &graph, true).into_string();
        assert!(html.contains("id=\"gpui-mount\""));
        assert!(html.contains("id=\"part-runtime\""));
        crate::copy_guard::assert_plain("/agent/helper", &html);
        assert_eq!(template("foreman").unwrap().0, templates::FOREMAN_ID);
        assert!(template("nobody").is_none());
        assert!(owns("/agent/foreman") && owns("/agent/foreman/graph.json"));
        assert!(owns("/agent/foreman/definition.json"));
        assert!(owns(&format!("/agent/{}", templates::FOREMAN_ID)));
        assert!(owns("/agent-graph/start.js"));
        assert!(!owns("/agent/nobody") && !owns("/agents"));
    }

    #[test]
    fn scenario_titles_drop_their_item_numbers() {
        assert_eq!(
            scenario_title("Item 1: a hung tool call is caught (2:11 PM)"),
            "A hung tool call is caught (2:11 PM)"
        );
        assert_eq!(scenario_title("Plain title"), "Plain title");
    }
}
