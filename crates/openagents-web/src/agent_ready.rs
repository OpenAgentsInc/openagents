//! What agents need to find, read, and use openagents.com (#11083):
//! `robots.txt`, `sitemap.xml`, `llms.txt` and `llms-full.txt`, a Markdown
//! twin of every page (`/index.md`, `/docs/{slug}.md`, ...) and
//! `Accept: text/markdown` negotiation, `auth.md`, the RFC 9727 API catalog,
//! the ARD resource catalog, the MCP server card, the OpenAPI document (from
//! the inference gateway), the agent skills index, and the WebMCP script.
//!
//! Everything is generated from the documents compiled into the binary
//! (`content/docs/`), so the Markdown, the indexes, and the pages cannot
//! disagree. The public docs MCP server is [`crate::docs_mcp`].

use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::Router;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, Uri, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::App;
use crate::markdown;
use crate::pages::agent_docs::AGENT_DOCS;
use crate::pages::api_docs::API_DOCS;
use crate::pages::content::{DOCS, PRIVACY, SECTIONS, TERMS};

/// The public site every page's metadata names.
pub(crate) const SITE: &str = "https://openagents.com";

/// What OpenAgents is, in one line: the docs' own opening sentence.
pub(crate) const DESCRIPTION: &str = "OpenAgents is an open network of agents you work with \
through one conversation, from anywhere: this website, your phone, or a terminal.";

/// Where people write to OpenAgents (the terms and privacy policy name it).
pub(crate) const CONTACT: &str = "chris@openagents.com";

/// The API catalog's link relation for each way to pay per request the
/// API takes now (an extension relation, RFC 8288).
pub(crate) const PAYMENT_REL: &str = "https://openagents.com/rel/payment-method";

/// The public docs MCP server's address on this site.
pub(crate) const MCP_PATH: &str = "/mcp/docs";

/// The WebMCP script: registers the site's tools with the browser's agent.
pub(crate) const WEBMCP_PATH: &str = "/static/webmcp.js";

/// The OpenAgents API skill, served under the skills index.
const API_SKILL_PATH: &str = "/.well-known/agent-skills/openagents-api/SKILL.md";
const API_SKILL: &str = include_str!("../content/skills/openagents-api/SKILL.md");

/// The same skill at the site root, where agentskills.io readers look.
const SKILL_PATH: &str = "/skill.md";

/// The Markdown twins and agent documents this module answers. The
/// upstream guard treats them as the site's own ([`owns`]).
const PATHS: [&str; 23] = [
    "/robots.txt",
    "/sitemap.xml",
    "/llms.txt",
    "/llms-full.txt",
    "/auth.md",
    "/index.md",
    "/docs.md",
    "/terms.md",
    "/privacy.md",
    "/download.md",
    "/pricing",
    "/pricing.md",
    "/promises.md",
    "/roadmap.md",
    "/openapi.json",
    "/.well-known/api-catalog",
    "/.well-known/ai-catalog.json",
    "/.well-known/mcp/server-card.json",
    "/.well-known/mcp.json",
    API_SKILL_PATH,
    SKILL_PATH,
    MCP_PATH,
    WEBMCP_PATH,
];

/// Whether `path` is one of this module's addresses.
pub(crate) fn owns(path: &str) -> bool {
    PATHS.contains(&path)
}

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/robots.txt", get(robots))
        .route("/sitemap.xml", get(sitemap))
        .route("/llms.txt", get(llms))
        .route("/llms-full.txt", get(llms_full))
        .route("/auth.md", get(auth_md))
        .route("/index.md", get(home_md))
        .route("/docs.md", get(docs_md))
        .route("/docs/api.md", get(api_md))
        .route("/docs/agents.md", get(agents_md))
        .route("/terms.md", get(terms_md))
        .route("/privacy.md", get(privacy_md))
        .route("/download.md", get(download_md))
        .route("/pricing.md", get(pricing_md))
        .route("/openapi.json", get(openapi))
        .route("/.well-known/api-catalog", get(api_catalog))
        .route("/.well-known/ai-catalog.json", get(ai_catalog))
        .route("/.well-known/mcp/server-card.json", get(mcp_card))
        .route("/.well-known/mcp.json", get(mcp_card))
        .route(API_SKILL_PATH, get(api_skill))
        .route(SKILL_PATH, get(api_skill))
        .route(WEBMCP_PATH, get(webmcp))
}

// ---------------------------------------------------------------------
// Markdown twins and negotiation

/// The Markdown twin of a page, by the page's path.
pub(crate) fn twin(path: &str) -> Option<String> {
    let path = path
        .strip_suffix('/')
        .filter(|p| !p.is_empty())
        .unwrap_or(path);
    match path {
        "/" => return Some("/index.md".to_owned()),
        "/docs" | "/docs/api" | "/docs/agents" | "/terms" | "/privacy" | "/download"
        | "/pricing" | "/promises" | "/roadmap" => {
            return Some(format!("{path}.md"));
        }
        _ => {}
    }
    if let Some(slug) = path.strip_prefix("/docs/agents/") {
        return AGENT_DOCS
            .iter()
            .any(|(name, _)| *name == slug)
            .then(|| format!("{path}.md"));
    }
    if let Some(slug) = path.strip_prefix("/docs/api/") {
        return API_DOCS
            .iter()
            .any(|(name, _)| *name == slug)
            .then(|| format!("{path}.md"));
    }
    let slug = path.strip_prefix("/docs/")?;
    DOCS.iter()
        .any(|(name, _)| *name == slug)
        .then(|| format!("{path}.md"))
}

/// A type other than HTML that a client asks for by name, at least as
/// much as HTML.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Wanted {
    /// `text/markdown`: the page's Markdown twin.
    Markdown,
    /// `text/plain`: the same twin, labelled as plain text.
    Plain,
    /// `application/json`: the site index on `/`, JSON errors elsewhere.
    Json,
}

/// What the client asks for instead of HTML, if anything (RFC 9110
/// §12.5.1): of the types it names (HTML, Markdown, plain text, JSON), the
/// one with the highest `q`, the one listed first on a tie. `None` when
/// HTML wins or nothing is named; `*/*` names nothing.
pub(crate) fn preferred(headers: &HeaderMap) -> Option<Wanted> {
    let accept = headers.get(header::ACCEPT).and_then(|v| v.to_str().ok())?;
    // (type, q, position); `None` is HTML.
    let mut best: Option<(Option<Wanted>, f32)> = None;
    for item in accept.split(',') {
        let mut parts = item.split(';');
        let kind = parts.next().unwrap_or_default().trim().to_ascii_lowercase();
        let q = parts
            .filter_map(|p| p.trim().strip_prefix("q="))
            .find_map(|q| q.trim().parse::<f32>().ok())
            .unwrap_or(1.0);
        let named = match kind.as_str() {
            "text/markdown" | "text/x-markdown" => Some(Wanted::Markdown),
            "text/plain" => Some(Wanted::Plain),
            "application/json" => Some(Wanted::Json),
            "text/html" | "application/xhtml+xml" => None,
            _ => continue,
        };
        // Strictly higher wins, so an earlier entry keeps a tie.
        if q > 0.0 && best.is_none_or(|(_, top)| q > top) {
            best = Some((named, q));
        }
    }
    best.and_then(|(named, _)| named)
}

/// Whether the client asks for Markdown at least as much as HTML.
pub(crate) fn wants_markdown(headers: &HeaderMap) -> bool {
    preferred(headers) == Some(Wanted::Markdown)
}

/// The links every page of the site answers with (RFC 8288), for agents
/// that read headers before bodies.
const DISCOVERY_LINKS: &str = "</.well-known/api-catalog>; rel=\"api-catalog\", \
</docs/api>; rel=\"service-doc\", \
</openapi.json>; rel=\"service-desc\"; type=\"application/json\", \
</llms.txt>; rel=\"describedby\"; type=\"text/plain\", \
</.well-known/mcp/server-card.json>; rel=\"mcp-server-card\"; type=\"application/json\", \
</.well-known/ai-catalog.json>; rel=\"ai-catalog\"; type=\"application/json\"";

/// The middleware in front of the site's routes, for requests to the
/// site's own hosts (`site`): a `GET` that asks for Markdown (or plain
/// text) and has a twin is answered by the twin, `/` asked for as JSON
/// answers the site index, every page says it varies by `Accept` and
/// where its twin and the agent documents are, and a `404` asked for as
/// Markdown or JSON answers in that type.
pub(crate) async fn negotiate(site: bool, mut request: Request, next: Next) -> Response {
    let read = matches!(*request.method(), Method::GET | Method::HEAD);
    if !site || !read {
        return next.run(request).await;
    }
    let path = request.uri().path().to_owned();
    let wanted = preferred(request.headers());
    if wanted == Some(Wanted::Json) && path == "/" {
        let mut response = json_response(&site_index_json(), "application/json; charset=utf-8");
        response
            .headers_mut()
            .insert(header::VARY, HeaderValue::from_static("Accept"));
        return response;
    }
    let markdown = matches!(wanted, Some(Wanted::Markdown | Wanted::Plain));
    let twin = twin(&path);
    if markdown && let Some(twin) = &twin {
        let target = match request.uri().query() {
            Some(query) => format!("{twin}?{query}"),
            None => twin.clone(),
        };
        if let Ok(uri) = target.parse::<Uri>() {
            *request.uri_mut() = uri;
        }
    }
    let mut response = next.run(request).await;
    let html = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("text/html"));
    if html && response.status() == StatusCode::NOT_FOUND {
        match wanted {
            Some(Wanted::Markdown | Wanted::Plain) => return not_found_markdown(&path),
            Some(Wanted::Json) => return not_found_json(&path),
            None => {}
        }
    }
    let headers = response.headers_mut();
    if wanted == Some(Wanted::Plain)
        && headers
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.starts_with("text/markdown"))
    {
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        );
    }
    let varies = headers
        .get_all(header::VARY)
        .iter()
        .any(|v| v.to_str().is_ok_and(|v| v.contains("Accept")));
    if twin.is_some() && !varies {
        headers.append(header::VARY, HeaderValue::from_static("Accept"));
    }
    if html {
        let mut links = DISCOVERY_LINKS.to_owned();
        if let Some(twin) = &twin {
            links.push_str(&format!(
                ", <{twin}>; rel=\"alternate\"; type=\"text/markdown\""
            ));
        }
        if let Ok(value) = HeaderValue::from_str(&links) {
            headers.append(header::LINK, value);
        }
    }
    response
}

fn not_found_markdown(path: &str) -> Response {
    let body = format!(
        "# Not found\n\nNothing on openagents.com has the address `{}`.\n\n\
- [Every page, as Markdown](/llms.txt)\n- [Docs](/docs.md)\n- [API docs](/docs/api.md)\n- [Home](/index.md)\n",
        path.replace('`', "")
    );
    let mut response = markdown_response(body, &format!("{SITE}{path}"));
    *response.status_mut() = StatusCode::NOT_FOUND;
    response
}

/// A missing page asked for as JSON: a JSON error with where to start.
fn not_found_json(path: &str) -> Response {
    let mut response = json_response(
        &json!({
            "error": "not_found",
            "message": format!("Nothing on openagents.com has the address {path}."),
            "path": path,
            "index": format!("{SITE}/llms.txt"),
            "docs": format!("{SITE}/docs/api"),
        }),
        "application/json; charset=utf-8",
    );
    *response.status_mut() = StatusCode::NOT_FOUND;
    response
        .headers_mut()
        .insert(header::VARY, HeaderValue::from_static("Accept"));
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

/// `/` asked for as JSON: what the site is and where an agent goes next.
pub(crate) fn site_index_json() -> Value {
    json!({
        "name": "OpenAgents",
        "description": DESCRIPTION,
        "url": format!("{SITE}/"),
        "markdown": format!("{SITE}/index.md"),
        "links": {
            "llms_txt": format!("{SITE}/llms.txt"),
            "llms_full_txt": format!("{SITE}/llms-full.txt"),
            "docs": format!("{SITE}/docs.md"),
            "api_docs": format!("{SITE}/docs/api.md"),
            "agent_docs": format!("{SITE}/docs/agents.md"),
            "api_catalog": format!("{SITE}/.well-known/api-catalog"),
            "openapi": format!("{SITE}/openapi.json"),
            "ai_catalog": format!("{SITE}/.well-known/ai-catalog.json"),
            "mcp_server_card": format!("{SITE}/.well-known/mcp/server-card.json"),
            "mcp_server": format!("{SITE}{MCP_PATH}"),
            "agent_card": format!("{SITE}/.well-known/agent-card.json"),
            "agent_skills": format!("{SITE}/.well-known/agent-skills/index.json"),
            "skill": format!("{SITE}/skill.md"),
            "auth": format!("{SITE}/auth.md"),
            "agent_register": format!("{SITE}/v1/agents/register"),
        },
    })
}

/// A Markdown answer: its type, a rough token count (four bytes a token),
/// its canonical page, and open to any origin.
fn markdown_response(body: String, canonical: &str) -> Response {
    let tokens = body.len().div_ceil(4).to_string();
    let mut response = (StatusCode::OK, body).into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/markdown; charset=utf-8"),
    );
    if let Ok(value) = HeaderValue::from_str(&tokens) {
        headers.insert("x-markdown-tokens", value);
    }
    if let Ok(value) = HeaderValue::from_str(&format!("<{canonical}>; rel=\"canonical\"")) {
        headers.insert(header::LINK, value);
    }
    headers.insert(header::VARY, HeaderValue::from_static("Accept"));
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    response
}

fn text_response(body: String, kind: &'static str) -> Response {
    let mut response = (StatusCode::OK, body).into_response();
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(kind));
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=300"),
    );
    response
}

fn json_response(value: &Value, kind: &'static str) -> Response {
    text_response(
        serde_json::to_string_pretty(value).unwrap_or_default(),
        kind,
    )
}

/// A document with YAML frontmatter: its title, one-line summary, and
/// canonical page.
fn with_frontmatter(title: &str, description: &str, canonical: &str, body: &str) -> String {
    let quote = |value: &str| serde_json::to_string(value).unwrap_or_default();
    format!(
        "---\ntitle: {}\ndescription: {}\nurl: {}\n---\n\n{}",
        quote(title),
        quote(description),
        quote(canonical),
        body.trim_start()
    )
}

/// The first paragraph of a Markdown document, as plain text, cut at a
/// word near 200 characters.
pub(crate) fn summary(source: &str) -> String {
    let mut paragraph = Vec::new();
    for line in source.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.starts_with('|') || line.starts_with("```") {
            if paragraph.is_empty() {
                continue;
            }
            break;
        }
        if line.is_empty() {
            if paragraph.is_empty() {
                continue;
            }
            break;
        }
        paragraph.push(
            line.strip_prefix("- ")
                .or_else(|| line.strip_prefix("* "))
                .unwrap_or(line),
        );
    }
    let text = plain(&paragraph.join(" "));
    if text.chars().count() <= 200 {
        return text;
    }
    let cut: String = text.chars().take(200).collect();
    match cut.rfind(' ') {
        Some(at) => format!("{}…", cut[..at].trim_end_matches([',', ';', ':'])),
        None => cut,
    }
}

/// Markdown inline syntax removed: `[text](url)` to `text`, emphasis and
/// code marks dropped.
fn plain(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find("](").and_then(|close| {
            after[close + 2..]
                .find(')')
                .map(|end| (close, close + 2 + end))
        }) {
            Some((close, end)) => {
                out.push_str(&after[..close]);
                rest = &after[end + 1..];
            }
            None => {
                out.push('[');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out.replace("**", "").replace('`', "")
}

pub(crate) fn doc_md(path: &str, source: &str, fallback: &str) -> Response {
    let canonical = format!("{SITE}{path}");
    markdown_response(
        with_frontmatter(
            &markdown::title(source, fallback),
            &summary(source),
            &canonical,
            source,
        ),
        &canonical,
    )
}

/// `/docs/{slug}.md`: one guide (called by the guide's route).
pub(crate) fn guide_md(slug: &str, source: &str) -> Response {
    doc_md(&format!("/docs/{slug}"), source, slug)
}

fn doc_source(slug: &str) -> &'static str {
    DOCS.iter()
        .find(|(name, _)| *name == slug)
        .map_or("", |(_, source)| source)
}

async fn terms_md() -> Response {
    doc_md("/terms", TERMS, "Terms of service")
}

async fn privacy_md() -> Response {
    doc_md("/privacy", PRIVACY, "Privacy policy")
}

async fn download_md() -> Response {
    doc_md("/download", doc_source("download"), "Download")
}

async fn pricing_md(State(app): State<App>) -> Response {
    let card = crate::pages::api_docs::card(&app).await;
    let body = format!(
        "{}\n\n## API prices\n\nThe OpenAgents API charges each model's price per million tokens. \
Every model, in US dollars ([as JSON](https://api.openagents.com/v1/rates)):\n\n{}\n",
        doc_source("pricing").trim_end(),
        crate::pages::api_docs::rate_card(&card)
    );
    doc_md("/pricing", &body, "Pricing")
}

/// The site's entry document for an agent: what OpenAgents is, then where
/// everything is.
fn home_markdown() -> String {
    let mut body = format!(
        "# OpenAgents\n\n{}\n\n## Start here\n\n\
- [Download](/download.md): the apps and Coder for your computer.\n\
- [Docs](/docs.md): guides to the chat, Coder, the apps, plugins, and the Gym.\n\
- [API](/docs/api.md): use our models from your own code. Beta.\n\
- [For agents](/docs/agents.md): sign up as an agent, pay in bitcoin, and use our API, MCP server, and command.\n\
- [Pricing](/pricing.md)\n\
- [What works today](/promises.md): each thing that works, with its proof.\n\
- [Roadmap](/roadmap.md): what comes next.\n\n",
        summary(doc_source("what-is-openagents"))
    );
    body.push_str(AGENT_LINKS);
    body
}

/// Where an agent finds the site's machine-readable documents.
const AGENT_LINKS: &str = "## For agents\n\n\
- [Start here](/docs/agents/start-here.md), then the [docs for agents](/docs/agents.md): \
tutorials and reference.\n\
- [llms.txt](/llms.txt) and [llms-full.txt](/llms-full.txt): every guide, as Markdown.\n\
- Any page as Markdown: add `.md` to its address, or send `Accept: text/markdown`.\n\
- [auth.md](/auth.md): register as an agent yourself, or get and use an API key.\n\
- [MCP server](/mcp/docs): read and search the docs as tools, no key needed \
([server card](/.well-known/mcp/server-card.json)).\n\
- [OpenAPI](/openapi.json) and the [API catalog](/.well-known/api-catalog).\n\
- [Agent card](/.well-known/agent-card.json) and [agent skills](/.well-known/agent-skills/index.json).\n";

async fn home_md() -> Response {
    let canonical = format!("{SITE}/");
    markdown_response(
        with_frontmatter("OpenAgents", DESCRIPTION, &canonical, &home_markdown()),
        &canonical,
    )
}

/// The guides, grouped by section, as Markdown list items.
fn guide_list(absolute: bool) -> String {
    let base = if absolute { SITE } else { "" };
    let mut out = String::new();
    for (slug, source) in DOCS {
        if let Some((title, lead, _)) = SECTIONS.iter().find(|(_, _, first)| *first == slug) {
            out.push_str(&format!("\n## {title}\n\n{lead}\n\n"));
        }
        out.push_str(&format!(
            "- [{}]({base}/docs/{slug}.md): {}\n",
            markdown::title(source, slug),
            summary(source)
        ));
    }
    out
}

fn api_list(absolute: bool) -> String {
    let base = if absolute { SITE } else { "" };
    API_DOCS
        .iter()
        .map(|(slug, source)| {
            format!(
                "- [{}]({base}/docs/api/{slug}.md): {}\n",
                markdown::title(source, slug),
                summary(source)
            )
        })
        .collect()
}

const API_LEAD: &str = "One API for many models: Open Responses and Chat Completions at \
`https://api.openagents.com/v1` (also `https://openagents.com/api/v1`). Beta.";

async fn docs_md() -> Response {
    let canonical = format!("{SITE}/docs");
    let body = format!(
        "# Docs\n\nGuides to OpenAgents: the chat, Coder, the apps, plugins, and the Gym.\n{}\n\
## API\n\n- [API docs](/docs/api.md): {API_LEAD}\n\n\
## For agents\n\n- [Docs for agents](/docs/agents.md): {}\n\n\
## What works and what's next\n\n- [What works today](/promises.md): each thing that works today, \
with its proof.\n- [Roadmap](/roadmap.md): what we're building next.\n",
        guide_list(false),
        crate::pages::agent_docs::LEAD
    );
    markdown_response(
        with_frontmatter(
            "Docs",
            "Guides to OpenAgents: the chat, Coder, the apps, plugins, and the Gym.",
            &canonical,
            &body,
        ),
        &canonical,
    )
}

async fn api_md() -> Response {
    let canonical = format!("{SITE}/docs/api");
    let body = format!("# API\n\n{API_LEAD}\n\n## Guides\n\n{}", api_list(false));
    markdown_response(
        with_frontmatter("API", &plain(API_LEAD), &canonical, &body),
        &canonical,
    )
}

async fn agents_md() -> Response {
    let canonical = format!("{SITE}/docs/agents");
    markdown_response(
        with_frontmatter(
            "For agents",
            crate::pages::agent_docs::LEAD,
            &canonical,
            &crate::pages::agent_docs::index_markdown(),
        ),
        &canonical,
    )
}

// ---------------------------------------------------------------------
// llms.txt, llms-full.txt, auth.md

/// `/llms.txt` (llmstxt.org): the title, a one-line summary, then every
/// guide and the agent documents, each with what it covers.
pub(crate) fn llms_txt(methods: &[crate::payments::Method]) -> String {
    format!(
        "# OpenAgents\n\n> {DESCRIPTION} Coder, its coding agent, works on your own computer \
with the coding agents you already use. Developers can call our models through the OpenAgents API.\n\n\
Every page here is also Markdown: add `.md` to its address, or send `Accept: text/markdown`.\n\
{}\n## For agents\n\n{}\n\
- [Start here]({SITE}/docs/agents/start-here.md) if you are an agent: what we offer, sign-up, keys, and errors.\n\
- [Docs for agents]({SITE}/docs/agents.md): every page below, grouped.\n{}\n## API\n\n- [API docs]({SITE}/docs/api.md): {API_LEAD}\n{}\
- [Authentication]({SITE}/auth.md): get an API key and send it.\n\
- [Agent sign-up]({SITE}/auth.md#register-yourself-as-an-agent): an agent registers itself with a small proof of work (`POST {SITE}/v1/agents/register`), gets a key and a small daily free tier, and pays over Lightning past it.\n\
- [For agents]({SITE}/docs/api/for-agents.md): find us, say who you are, and pay. {}\n\
- [OpenAPI]({SITE}/openapi.json): the API's machine-readable description.\n\
- [API catalog]({SITE}/.well-known/api-catalog): where each API and its description live.\n\n\
## Agent tools\n\n\
- [Docs MCP server]({SITE}{MCP_PATH}): list, search, and read these docs and the model prices as MCP tools over Streamable HTTP. No key needed.\n\
- [MCP server card]({SITE}/.well-known/mcp/server-card.json): the server's tools and transport.\n\
- [Agent card]({SITE}/.well-known/agent-card.json): A2A agent card.\n\
- [Agent skills]({SITE}/.well-known/agent-skills/index.json): skills an agent can install.\n\n\
## Optional\n\n\
- [Everything in one file]({SITE}/llms-full.txt): every guide, API guide, and page for agents.\n\
- [Pricing]({SITE}/pricing.md)\n\
- [What works today]({SITE}/promises.md): each thing that works, with its proof.\n\
- [Roadmap]({SITE}/roadmap.md): what comes next.\n\
- [Download]({SITE}/download.md)\n\
- [Terms of service]({SITE}/terms.md)\n\
- [Privacy policy]({SITE}/privacy.md)\n",
        guide_list(true),
        crate::pages::agent_docs::LEAD,
        crate::pages::agent_docs::list(true),
        api_list(true),
        crate::payments::sentence(methods)
    )
}

async fn llms(State(app): State<App>) -> Response {
    let methods = crate::payments::live(&app).await;
    text_response(llms_txt(&methods), "text/plain; charset=utf-8")
}

/// `/llms-full.txt`: every guide and API guide, in reading order, each
/// under a rule and its address.
async fn llms_full(State(app): State<App>) -> Response {
    let mut out = format!("# OpenAgents\n\n> {DESCRIPTION}\n\n");
    for (slug, source) in DOCS {
        out.push_str(&format!(
            "\n---\n\nSource: {SITE}/docs/{slug}\n\n{}\n",
            source.trim()
        ));
    }
    for (slug, source) in API_DOCS {
        let source = crate::pages::api_docs::source(&app, source).await;
        out.push_str(&format!(
            "\n---\n\nSource: {SITE}/docs/api/{slug}\n\n{}\n",
            source.trim()
        ));
    }
    for (slug, source) in AGENT_DOCS {
        let source = crate::pages::api_docs::source(&app, source).await;
        out.push_str(&format!(
            "\n---\n\nSource: {SITE}/docs/agents/{slug}\n\n{}\n",
            source.trim()
        ));
    }
    text_response(out, "text/plain; charset=utf-8")
}

/// `/auth.md`: how an agent gets and uses a credential here.
const AUTH_MD: &str = include_str!("../content/agents/auth.md");

async fn auth_md(State(app): State<App>) -> Response {
    let canonical = format!("{SITE}/auth.md");
    let methods = crate::payments::live(&app).await;
    markdown_response(auth_markdown(&methods), &canonical)
}

/// `auth.md` with the ways to pay per request this API takes now.
pub(crate) fn auth_markdown(methods: &[crate::payments::Method]) -> String {
    let mut out = AUTH_MD.trim_end().to_owned();
    out.push_str("\n\n## Pay per request instead\n\n");
    out.push_str(&crate::payments::sentence(methods));
    for method in methods {
        out.push_str(&format!(
            "\n- {}: the `402` carries `{}`; send `{}`; the answer carries `{}`.",
            method.name, method.challenge, method.credential, method.receipt
        ));
    }
    out.push_str("\n\nEvery step: [For agents](https://openagents.com/docs/api/for-agents.md).\n");
    out
}

// ---------------------------------------------------------------------
// robots.txt and sitemap.xml

/// The paths no crawler should fetch: sign-in, accounts, private chats,
/// and the APIs.
const PRIVATE: [&str; 16] = [
    "/app",
    "/chat",
    "/composer/",
    "/ask",
    "/settings",
    "/projects",
    "/environments",
    "/auth/",
    "/login",
    "/signup",
    "/sign-in",
    "/sign-out",
    "/device",
    "/api/",
    "/mcp",
    "/oauth/",
];

/// The crawler groups, each named so a site owner can see who is allowed:
/// AI search, AI training, and fetches a person asks an assistant for.
const GROUPS: [(&str, &[&str]); 3] = [
    (
        "AI search crawlers: they find pages for answers in AI assistants.",
        &[
            "OAI-SearchBot",
            "Claude-SearchBot",
            "PerplexityBot",
            "Google-CloudVertexBot",
        ],
    ),
    (
        "AI training crawlers.",
        &[
            "GPTBot",
            "ClaudeBot",
            "Google-Extended",
            "Applebot-Extended",
            "CCBot",
            "Bytespider",
            "Amazonbot",
            "meta-externalagent",
        ],
    ),
    (
        "Agents fetching a page because a person asked them to.",
        &[
            "ChatGPT-User",
            "Claude-User",
            "Perplexity-User",
            "MistralAI-User",
        ],
    ),
];

pub(crate) fn robots_txt(origin: &str) -> String {
    let rules = |out: &mut String| {
        out.push_str("Content-Signal: search=yes, ai-input=yes, ai-train=yes\nAllow: /\n");
        for path in PRIVATE {
            out.push_str(&format!("Disallow: {path}\n"));
        }
    };
    let mut out = String::from(
        "# openagents.com: crawlers and agents are welcome.\n\
# Every page as Markdown: /llms.txt. Docs as MCP tools: /mcp/docs.\n\n",
    );
    for (comment, agents) in GROUPS {
        out.push_str(&format!("# {comment}\n"));
        for agent in agents {
            out.push_str(&format!("User-agent: {agent}\n"));
        }
        rules(&mut out);
        out.push('\n');
    }
    out.push_str("User-agent: *\n");
    rules(&mut out);
    out.push_str(&format!(
        "\nSitemap: {origin}/sitemap.xml\nAgentmap: {origin}/.well-known/ai-catalog.json\n"
    ));
    out
}

async fn robots(State(app): State<App>) -> Response {
    text_response(
        robots_txt(&crate::wellknown::origin(&app)),
        "text/plain; charset=utf-8",
    )
}

/// The public pages, then every guide and API guide.
pub(crate) fn public_paths() -> Vec<String> {
    let mut paths: Vec<String> = [
        "/",
        "/download",
        "/docs",
        "/docs/api",
        "/docs/agents",
        "/pilot",
        "/live",
        "/stats",
        "/promises",
        "/roadmap",
        "/terms",
        "/privacy",
    ]
    .iter()
    .map(|p| (*p).to_owned())
    .collect();
    paths.extend(DOCS.iter().map(|(slug, _)| format!("/docs/{slug}")));
    paths.extend(API_DOCS.iter().map(|(slug, _)| format!("/docs/api/{slug}")));
    paths.extend(
        AGENT_DOCS
            .iter()
            .map(|(slug, _)| format!("/docs/agents/{slug}")),
    );
    paths
}

/// The day this server started, as `YYYY-MM-DD`: every page is compiled
/// into the build it runs, so a page changes at the latest on the day its
/// build starts serving.
fn started() -> &'static str {
    static DAY: OnceLock<String> = OnceLock::new();
    DAY.get_or_init(|| {
        let days = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() / 86_400);
        civil(days as i64)
    })
}

/// Days since 1970-01-01 to a proleptic Gregorian date (Howard Hinnant's
/// `civil_from_days`).
fn civil(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

pub(crate) fn sitemap_xml(origin: &str) -> String {
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
    );
    for path in public_paths() {
        out.push_str(&format!(
            "  <url><loc>{origin}{path}</loc><lastmod>{}</lastmod></url>\n",
            started()
        ));
    }
    out.push_str("</urlset>\n");
    out
}

async fn sitemap(State(app): State<App>) -> Response {
    text_response(
        sitemap_xml(&crate::wellknown::origin(&app)),
        "application/xml; charset=utf-8",
    )
}

// ---------------------------------------------------------------------
// Catalogs and cards

/// `/.well-known/api-catalog` (RFC 9727): each API, where its description
/// and docs are.
pub(crate) fn api_catalog_json(origin: &str, methods: &[crate::payments::Method]) -> Value {
    let payment: Vec<Value> = methods
        .iter()
        .map(|method| json!({"href": method.spec, "title": method.name, "id": method.id}))
        .collect();
    let api = |anchor: &str| {
        let mut entry = json!({
            "anchor": anchor,
            "service-desc": [{"href": format!("{origin}/openapi.json"), "type": "application/json"}],
            "service-doc": [
                {"href": format!("{origin}/docs/api"), "type": "text/html"},
                {"href": format!("{origin}/docs/api.md"), "type": "text/markdown"},
                {"href": format!("{origin}/docs/agents/start-here"), "type": "text/html", "title": "For agents: start here"},
                {"href": format!("{origin}/docs/agents.md"), "type": "text/markdown", "title": "Docs for agents: tutorials and reference"},
                {"href": format!("{origin}/docs/api/for-agents"), "type": "text/html", "title": "For agents: find, log in, pay"},
                {"href": format!("{origin}/docs/api/pay-per-request"), "type": "text/html", "title": "Pay per request"},
                {"href": format!("{origin}/auth.md#register-yourself-as-an-agent"), "type": "text/markdown", "title": "Register as an agent"}
            ],
            "describedby": [{"href": format!("{origin}/auth.md"), "type": "text/markdown"}]
        });
        if !payment.is_empty() {
            entry[PAYMENT_REL] = json!(payment);
        }
        entry
    };
    json!({
        "linkset": [
            {
                "anchor": format!("{origin}/.well-known/api-catalog"),
                "item": [
                    {"href": "https://api.openagents.com/v1"},
                    {"href": format!("{origin}/api/v1")},
                    {"href": format!("{origin}{MCP_PATH}")}
                ]
            },
            api("https://api.openagents.com/v1"),
            api(&format!("{origin}/api/v1")),
            {
                "anchor": format!("{origin}{MCP_PATH}"),
                "service-desc": [{"href": format!("{origin}/.well-known/mcp/server-card.json"), "type": "application/json"}],
                "service-doc": [{"href": format!("{origin}/llms.txt"), "type": "text/plain"}]
            }
        ]
    })
}

async fn api_catalog(State(app): State<App>) -> Response {
    let methods = crate::payments::live(&app).await;
    let mut response = json_response(
        &api_catalog_json(&crate::wellknown::origin(&app), &methods),
        "application/linkset+json; charset=utf-8",
    );
    if let Ok(value) =
        HeaderValue::from_str("<https://www.rfc-editor.org/info/rfc9727>; rel=\"profile\"")
    {
        response.headers_mut().insert(header::LINK, value);
    }
    response
}

/// `/.well-known/ai-catalog.json` (Agentic Resource Discovery): the MCP
/// server, the agent card, the skills, and the API.
pub(crate) fn ai_catalog_json(origin: &str, methods: &[crate::payments::Method]) -> Value {
    let host = origin
        .split("://")
        .nth(1)
        .unwrap_or("openagents.com")
        .split(':')
        .next()
        .unwrap_or("openagents.com")
        .to_owned();
    let mut api = json!({
        "identifier": format!("urn:air:{host}:api:openagents"),
        "displayName": "OpenAgents API",
        "description": API_LEAD.replace('`', ""),
        "type": "application/linkset+json",
        "url": format!("{origin}/.well-known/api-catalog"),
        "documentationUrl": format!("{origin}/docs/api/for-agents"),
        "representativeQueries": [
            "an OpenAI-compatible API with many models",
            "call Open Responses with one key",
            "cheapest model for a quick answer"
        ]
    });
    if !methods.is_empty() {
        api["paymentProtocols"] = json!(crate::payments::ids(methods));
        api["paymentDocumentationUrl"] = json!(format!("{origin}/docs/api/pay-per-request"));
    }
    json!({
        "specVersion": "1.0",
        "host": {
            "displayName": "OpenAgents",
            "identifier": format!("did:web:{host}"),
            "documentationUrl": format!("{origin}/llms.txt")
        },
        "entries": [
            {
                "identifier": format!("urn:air:{host}:server:docs"),
                "displayName": "OpenAgents docs MCP server",
                "description": "List, search, and read the OpenAgents docs and the API's model prices. No key needed.",
                "type": "application/mcp-server-card+json",
                "url": format!("{origin}/.well-known/mcp/server-card.json"),
                "representativeQueries": [
                    "how do I connect my phone to my computer in OpenAgents",
                    "what does the OpenAgents API cost per model",
                    "how do I write an OpenAgents plugin"
                ]
            },
            api,
            {
                "identifier": format!("urn:air:{host}:agent:card"),
                "displayName": "OpenAgents agent card",
                "type": "application/a2a-agent-card+json",
                "url": format!("{origin}/.well-known/agent-card.json"),
                "representativeQueries": [
                    "an agent that answers typed yes/no and score questions",
                    "classify a batch of text"
                ]
            },
            {
                "identifier": format!("urn:air:{host}:skills:index"),
                "displayName": "OpenAgents agent skills",
                "type": "application/agent-skills-index+json",
                "url": format!("{origin}/.well-known/agent-skills/index.json"),
                "representativeQueries": [
                    "a skill for calling the OpenAgents API",
                    "install an OpenAgents skill in Claude Code"
                ]
            }
        ]
    })
}

async fn ai_catalog(State(app): State<App>) -> Response {
    let methods = crate::payments::live(&app).await;
    json_response(
        &ai_catalog_json(&crate::wellknown::origin(&app), &methods),
        "application/json; charset=utf-8",
    )
}

async fn mcp_card(State(app): State<App>) -> Response {
    json_response(
        &crate::docs_mcp::server_card(&crate::wellknown::origin(&app)),
        "application/json; charset=utf-8",
    )
}

/// The skills index: the decision API skill `crates/discovery` publishes,
/// and the OpenAgents API skill.
pub(crate) fn skills_index(origin: &str) -> Value {
    let mut index = discovery::site::skills_index(origin);
    let digest: String = Sha256::digest(API_SKILL.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if let Some(skills) = index["skills"].as_array_mut() {
        skills.push(json!({
            "name": "openagents-api",
            "type": "skill-md",
            "description": "Call the OpenAgents API (Open Responses and Chat Completions, many models, one key) and read its docs.",
            "url": format!("{origin}{API_SKILL_PATH}"),
            "digest": format!("sha256:{digest}"),
        }));
    }
    index
}

async fn api_skill() -> Response {
    markdown_response(API_SKILL.to_owned(), &format!("{SITE}{API_SKILL_PATH}"))
}

/// `/openapi.json`: the inference gateway's OpenAPI document. Without a
/// gateway (or before it serves one) this answers `404`.
async fn openapi(State(app): State<App>, request: Request) -> Response {
    let Some(upstream) = &app.config.inference else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let (mut parts, body) = request.into_parts();
    parts.uri = Uri::from_static("/openapi.json");
    parts.headers.remove(header::COOKIE);
    parts.headers.remove(header::AUTHORIZATION);
    let mut response = upstream.forward(Request::from_parts(parts, body)).await;
    response.headers_mut().insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    response
}

async fn webmcp() -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        include_str!("../static/webmcp.js"),
    )
        .into_response()
}

// ---------------------------------------------------------------------
// Page metadata

/// The `<head>` metadata every page carries: description, Open Graph,
/// canonical and Markdown links, the resource catalog, and JSON-LD.
pub(crate) fn head(title: &str, description: Option<&str>, path: Option<&str>) -> String {
    let description = description.unwrap_or(DESCRIPTION);
    let escape = crate::layout::escape;
    let image = crate::link_preview::image(path);
    let mut out = format!(
        "<meta name=\"description\" content=\"{d}\">\
<meta property=\"og:title\" content=\"{t}\">\
<meta property=\"og:description\" content=\"{d}\">\
<meta property=\"og:type\" content=\"website\">\
<meta property=\"og:site_name\" content=\"OpenAgents\">\
<meta property=\"og:image\" content=\"{i}\">\
<meta property=\"og:image:type\" content=\"image/png\">\
<meta property=\"og:image:width\" content=\"1200\">\
<meta property=\"og:image:height\" content=\"630\">\
<meta property=\"og:image:alt\" content=\"{a}\">\
<meta name=\"twitter:card\" content=\"summary_large_image\">\
<meta name=\"twitter:image\" content=\"{i}\">\
<link rel=\"ai-catalog\" href=\"/.well-known/ai-catalog.json\">\
<link rel=\"help\" type=\"text/plain\" href=\"/llms.txt\">\
<link rel=\"alternate\" type=\"text/plain\" href=\"/llms.txt\" title=\"llms.txt\">\
<link rel=\"alternate\" type=\"text/plain\" href=\"/llms-full.txt\" title=\"llms-full.txt\">",
        d = escape(description),
        t = escape(title),
        i = escape(&image.url),
        a = escape(&image.alt),
    );
    if let Some(path) = path {
        out.push_str(&format!(
            "<link rel=\"canonical\" href=\"{SITE}{p}\"><meta property=\"og:url\" content=\"{SITE}{p}\">",
            p = escape(path)
        ));
        if let Some(twin) = twin(path) {
            out.push_str(&format!(
                "<link rel=\"alternate\" type=\"text/markdown\" href=\"{}\">",
                escape(&twin)
            ));
        }
    }
    let mut graph = vec![
        json!({
            "@type": "Organization",
            "@id": format!("{SITE}/#organization"),
            "name": "OpenAgents",
            "legalName": "OpenAgents, Inc.",
            "url": format!("{SITE}/"),
            "logo": format!("{SITE}/favicon.svg"),
            "email": CONTACT,
            "contactPoint": [
                {"@type": "ContactPoint", "contactType": "customer support", "email": CONTACT, "url": format!("{SITE}/docs")},
                {"@type": "ContactPoint", "contactType": "security", "email": crate::wellknown::SECURITY_CONTACT.trim_start_matches("mailto:")}
            ],
            "sameAs": ["https://github.com/OpenAgentsInc", "https://x.com/OpenAgentsInc"]
        }),
        json!({
            "@type": "WebSite",
            "@id": format!("{SITE}/#website"),
            "name": "OpenAgents",
            "url": format!("{SITE}/"),
            "description": DESCRIPTION,
            "publisher": {"@id": format!("{SITE}/#organization")}
        }),
    ];
    if let Some(path) = path {
        let url = format!("{SITE}{path}");
        let page = if path == "/download" {
            json!({
                "@type": "SoftwareApplication",
                "name": "OpenAgents",
                "url": url,
                "description": description,
                "applicationCategory": "DeveloperApplication",
                "operatingSystem": "macOS, iOS, Linux, Windows",
                "offers": {"@type": "Offer", "price": "0", "priceCurrency": "USD"},
                "publisher": {"@id": format!("{SITE}/#organization")}
            })
        } else if path.starts_with("/docs/") {
            json!({
                "@type": "TechArticle",
                "headline": title,
                "url": url,
                "description": description,
                "isPartOf": {"@id": format!("{SITE}/#website")},
                "publisher": {"@id": format!("{SITE}/#organization")}
            })
        } else {
            json!({
                "@type": "WebPage",
                "name": title,
                "url": url,
                "description": description,
                "isPartOf": {"@id": format!("{SITE}/#website")}
            })
        };
        graph.push(page);
    }
    let ld = json!({"@context": "https://schema.org", "@graph": graph});
    // `</` cannot end the script block early.
    let ld = serde_json::to_string(&ld)
        .unwrap_or_default()
        .replace("</", "<\\/");
    out.push_str(&format!(
        "<script type=\"application/ld+json\">{ld}</script>"
    ));
    out
}

/// The page script that registers WebMCP tools, for pages that load
/// scripts.
pub(crate) fn webmcp_tag() -> String {
    format!("<script src=\"{WEBMCP_PATH}\" defer></script>")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_is_wanted_only_when_asked_for_at_least_as_much_as_html() {
        let ask = |accept: &str| {
            let mut headers = HeaderMap::new();
            headers.insert(header::ACCEPT, HeaderValue::from_str(accept).unwrap());
            wants_markdown(&headers)
        };
        assert!(ask("text/markdown"));
        assert!(ask("text/markdown, text/html;q=0.9"));
        assert!(ask("text/html;q=0.5, text/markdown"));
        assert!(!ask("text/html,application/xhtml+xml,*/*;q=0.8"));
        assert!(!ask("text/html, text/markdown;q=0.5"));
        assert!(!ask("text/markdown;q=0"));
        assert!(!wants_markdown(&HeaderMap::new()));
    }

    #[test]
    fn the_preferred_type_has_the_highest_q_and_comes_first_on_a_tie() {
        let ask = |accept: &str| {
            let mut headers = HeaderMap::new();
            headers.insert(header::ACCEPT, HeaderValue::from_str(accept).unwrap());
            preferred(&headers)
        };
        assert_eq!(ask("text/plain"), Some(Wanted::Plain));
        assert_eq!(ask("application/json"), Some(Wanted::Json));
        assert_eq!(ask("application/json, text/plain, */*"), Some(Wanted::Json));
        // Order breaks a tie, and `q` beats order (RFC 9110 §12.5.1).
        assert_eq!(ask("text/markdown, text/html, */*"), Some(Wanted::Markdown));
        assert_eq!(ask("text/html, text/markdown, */*"), None);
        assert_eq!(ask("text/markdown;q=0.5, text/html;q=1.0"), None);
        assert_eq!(ask("application/json, text/html, */*"), Some(Wanted::Json));
        assert_eq!(
            ask("text/plain;q=0.5, application/json"),
            Some(Wanted::Json)
        );
        assert_eq!(ask("text/markdown, text/plain"), Some(Wanted::Markdown));
        assert_eq!(ask("text/html, application/json;q=0.9"), None);
        assert_eq!(ask("*/*"), None);
        assert_eq!(ask("application/json;q=0"), None);
    }

    #[test]
    fn every_page_with_a_document_has_a_twin() {
        assert_eq!(twin("/").as_deref(), Some("/index.md"));
        assert_eq!(twin("/docs").as_deref(), Some("/docs.md"));
        assert_eq!(twin("/docs/").as_deref(), Some("/docs.md"));
        assert_eq!(twin("/docs/chat").as_deref(), Some("/docs/chat.md"));
        assert_eq!(twin("/docs/api").as_deref(), Some("/docs/api.md"));
        assert_eq!(
            twin("/docs/api/quickstart").as_deref(),
            Some("/docs/api/quickstart.md")
        );
        assert_eq!(twin("/docs/nope"), None);
        assert_eq!(twin("/settings"), None);
        for (slug, _) in DOCS {
            assert!(twin(&format!("/docs/{slug}")).is_some(), "{slug}");
        }
    }

    #[test]
    fn summaries_are_plain_first_paragraphs() {
        assert_eq!(
            summary("# T\n\nOne [link](/x) and **bold** `code`.\nSecond line.\n\nNext."),
            "One link and bold code. Second line."
        );
        let long = format!("# T\n\n{}", "word ".repeat(80));
        let cut = summary(&long);
        assert!(cut.chars().count() <= 201 && cut.ends_with('…'), "{cut}");
        assert_eq!(
            summary(DOCS[0].1).split(':').next(),
            Some(
                "OpenAgents is an open network of agents you work with through one conversation, from anywhere"
            )
        );
    }

    #[test]
    fn civil_dates_match_known_days() {
        assert_eq!(civil(0), "1970-01-01");
        assert_eq!(civil(20_735), "2026-10-09");
        assert_eq!(civil(11_016), "2000-02-29");
    }

    #[test]
    fn robots_names_search_training_and_user_agents_with_signals_and_maps() {
        let robots = robots_txt("https://openagents.com");
        for agent in ["GPTBot", "ClaudeBot", "OAI-SearchBot", "Claude-User", "*"] {
            assert!(
                robots.contains(&format!("User-agent: {agent}\n")),
                "{agent}"
            );
        }
        assert_eq!(robots.matches("Content-Signal: ").count(), 4);
        assert!(robots.contains("Disallow: /settings\n"));
        assert!(robots.contains("Sitemap: https://openagents.com/sitemap.xml\n"));
        assert!(robots.contains("Agentmap: https://openagents.com/.well-known/ai-catalog.json\n"));
        // Each group carries its own rules (RFC 9309 picks one group).
        assert_eq!(robots.matches("Allow: /\n").count(), 4);
    }

    #[test]
    fn llms_txt_links_every_guide_and_the_agent_documents() {
        let llms = llms_txt(&[]);
        assert!(llms.starts_with("# OpenAgents\n\n> "));
        for (slug, _) in DOCS {
            assert!(llms.contains(&format!("{SITE}/docs/{slug}.md)")), "{slug}");
        }
        for (slug, _) in API_DOCS {
            assert!(
                llms.contains(&format!("{SITE}/docs/api/{slug}.md)")),
                "{slug}"
            );
        }
        for path in [
            "/auth.md",
            "/llms-full.txt",
            MCP_PATH,
            "/.well-known/api-catalog",
            "/openapi.json",
        ] {
            assert!(llms.contains(&format!("{SITE}{path})")), "{path}");
        }
        assert!(llms.contains("\n## Optional\n"));
    }

    #[test]
    fn the_head_is_escaped_and_its_json_ld_cannot_close_the_script() {
        let head = head("A \"quoted\" </script> title", None, Some("/docs/chat"));
        assert!(head.contains("<meta name=\"description\""));
        assert!(
            head.contains("<link rel=\"canonical\" href=\"https://openagents.com/docs/chat\">")
        );
        assert!(
            head.contains("<link rel=\"alternate\" type=\"text/markdown\" href=\"/docs/chat.md\">")
        );
        assert!(head.contains("&quot;quoted&quot;"));
        assert_eq!(head.matches("</script>").count(), 1);
        let start = head.find("application/ld+json\">").unwrap() + 21;
        let end = head.rfind("</script>").unwrap();
        let ld: Value = serde_json::from_str(&head[start..end]).unwrap();
        let types: Vec<&str> = ld["@graph"]
            .as_array()
            .unwrap()
            .iter()
            .map(|node| node["@type"].as_str().unwrap())
            .collect();
        assert_eq!(types, ["Organization", "WebSite", "TechArticle"]);
    }
}
