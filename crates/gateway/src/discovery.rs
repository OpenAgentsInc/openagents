//! The public discovery surface: unauthenticated `GET` routes that
//! describe this deployment — the machine-readable document set, the
//! well-known indexes, the versioned docs API, and the browsable HTML
//! renderings of the same bytes.
//!
//! Nothing here authenticates and nothing here decides. Every document
//! is bundled at build time by `crates/discovery`; the generated
//! documents fold in the served origin — `public_origin` when the
//! operator declared one, the request's own `Host` otherwise.

use axum::extract::{Path, Query, State};
use axum::http::header::{ACCEPT, HOST, LINK};
use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{MethodRouter, get};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

use discovery::corpus;
use discovery::plugins;
use discovery::site::{self, Repr};

use crate::serve::ServeState;

/// One bundled document route: the path, the page title its HTML
/// rendering carries, the exact bytes, and the native representation.
struct Document {
    path: &'static str,
    title: &'static str,
    bytes: &'static str,
    native: Repr,
}

const fn doc(
    path: &'static str,
    title: &'static str,
    bytes: &'static str,
    native: Repr,
) -> Document {
    Document {
        path,
        title,
        bytes,
        native,
    }
}

/// Every bundled document, in sitemap order. The `document` handler
/// serves each by looking the request path up here — the table is the
/// route list, so a document cannot be mounted without being listed
/// and cannot be listed without being mounted.
static DOCUMENTS: &[Document] = &[
    doc("/", "OpenAgents decision API", "", Repr::Html),
    doc(
        "/agents.md",
        "The decision API for agents",
        site::AGENTS_MD,
        Repr::Markdown,
    ),
    doc(
        "/auth.md",
        "Authenticate to the decision API",
        site::AUTH_MD,
        Repr::Markdown,
    ),
    doc(
        "/skills.md",
        "Call the decision API from an agent",
        site::SKILL_MD,
        Repr::Markdown,
    ),
    doc("/llms.txt", "llms.txt", site::LLMS_TXT, Repr::Text),
    doc(
        "/api-catalog.json",
        "API route catalog",
        site::API_CATALOG,
        Repr::Json,
    ),
    doc(
        "/openapi.yaml",
        "OpenAPI 3.1 fragment",
        site::OPENAPI_YAML,
        Repr::Yaml,
    ),
    doc(
        "/index.json",
        "Discovery document index",
        site::DOCS_INDEX,
        Repr::Json,
    ),
    doc(
        "/mcp-tools.json",
        "MCP tool list snapshot",
        site::MCP_TOOLS,
        Repr::Json,
    ),
    doc(
        "/.well-known/agent-skills/openagents-decision-api/SKILL.md",
        "openagents-decision-api",
        plugins::CANONICAL_SKILL,
        Repr::Markdown,
    ),
    doc(
        "/plugins/README.md",
        "Agent plugins for the decision API",
        plugins::README,
        Repr::Markdown,
    ),
    doc(
        "/plugins/skills/openagents-decision-api/SKILL.md",
        "openagents-decision-api",
        plugins::CANONICAL_SKILL,
        Repr::Markdown,
    ),
    doc(
        "/plugins/claude/.claude-plugin/plugin.json",
        "Claude Code plugin manifest",
        plugins::CLAUDE_MANIFEST,
        Repr::Json,
    ),
    doc(
        "/plugins/claude/.mcp.json",
        "Claude Code MCP servers",
        plugins::CLAUDE_MCP,
        Repr::Json,
    ),
    doc(
        "/plugins/claude/skills/openagents-decision-api/SKILL.md",
        "openagents-decision-api",
        plugins::CLAUDE_SKILL,
        Repr::Markdown,
    ),
    doc(
        "/plugins/codex/.codex-plugin/plugin.json",
        "Codex plugin manifest",
        plugins::CODEX_MANIFEST,
        Repr::Json,
    ),
    doc(
        "/plugins/codex/.mcp.json",
        "Codex MCP servers",
        plugins::CODEX_MCP,
        Repr::Json,
    ),
    doc(
        "/plugins/codex/skills/openagents-decision-api/SKILL.md",
        "openagents-decision-api",
        plugins::CODEX_SKILL,
        Repr::Markdown,
    ),
];

/// The generated-document routes: computed per request so the served
/// origin folds into every link and canonical.
const GENERATED: &[(&str, &str)] = &[
    (
        "/api",
        "API index: every API, its version, and how to authenticate",
    ),
    ("/.well-known/agent-card.json", "A2A-format agent card"),
    ("/.well-known/agent-skills/index.json", "Agent skills index"),
    (
        "/mcp/server-card.json",
        "MCP server card: transport, sessions, and tools",
    ),
    ("/sitemap.xml", "Sitemap for search engines"),
    ("/robots.txt", "Rules for web crawlers"),
];

/// The versioned docs API — the same corpus the MCP documentation
/// tools read, over unauthenticated HTTP.
const DOCS_API: &[(&str, &str)] = &[
    ("/v1/docs", "List the documentation"),
    ("/v1/docs/search?q=quota", "Search the documentation"),
    ("/v1/docs/examples", "List the examples"),
    ("/v1/docs/caller", "Read one page of a document by its ID"),
];

/// The route table the public surface mounts, as `(path, handler)`
/// pairs. `serve::router` iterates this — what is mounted is what the
/// catalog test enumerates, and a path absent here answers 404.
pub(crate) fn routes() -> Vec<(&'static str, MethodRouter<Arc<ServeState>>)> {
    let mut routes: Vec<(&'static str, MethodRouter<Arc<ServeState>>)> = DOCUMENTS
        .iter()
        .map(|route| (route.path, get(document)))
        .collect();
    routes.extend([
        ("/api", get(api_index)),
        ("/.well-known/agent-card.json", get(agent_card)),
        ("/.well-known/agent-skills/index.json", get(skills_index)),
        ("/mcp/server-card.json", get(mcp_server_card)),
        ("/sitemap.xml", get(sitemap)),
        ("/robots.txt", get(robots)),
        ("/v1/docs", get(docs_list)),
        ("/v1/docs/search", get(docs_search)),
        ("/v1/docs/examples", get(docs_examples)),
        ("/v1/docs/{id}", get(doc_read)),
    ]);
    routes
}

/// Every public path — the sitemap and the catalog test share it. The
/// docs API contributes its fixed entry points; parametrized reads and
/// searches are not crawl targets.
pub(crate) fn paths() -> Vec<&'static str> {
    DOCUMENTS
        .iter()
        .map(|route| route.path)
        .chain(GENERATED.iter().map(|(path, _)| *path))
        .chain(["/v1/docs", "/v1/docs/examples"])
        .collect()
}

/// The origin discovery documents fold into links: `public_origin`
/// when the operator declared the public name, the request's `Host`
/// over plain HTTP otherwise. Forwarded headers are not trusted — a
/// deployment behind a proxy names itself in config.
fn origin(state: &ServeState, headers: &HeaderMap) -> String {
    if let Some(origin) = &state.config.public_origin {
        return origin.clone();
    }
    let host = headers
        .get(HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("localhost");
    format!("http://{host}")
}

fn negotiate(headers: &HeaderMap, offered: &[Repr]) -> Repr {
    site::negotiate(
        headers.get(ACCEPT).and_then(|value| value.to_str().ok()),
        offered,
    )
}

/// A response body in one representation, with the canonical link
/// header the metadata rules ask for.
fn body(repr: Repr, canonical: &str, bytes: String) -> Response {
    let mut response = (StatusCode::OK, bytes).into_response();
    response.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static(match repr {
            Repr::Html => "text/html; charset=utf-8",
            Repr::Markdown => "text/markdown; charset=utf-8",
            Repr::Json => "application/json; charset=utf-8",
            Repr::Yaml => "application/yaml; charset=utf-8",
            Repr::Text => "text/plain; charset=utf-8",
        }),
    );
    if let Ok(link) = HeaderValue::from_str(&format!("<{canonical}>; rel=\"canonical\"")) {
        response.headers_mut().insert(LINK, link);
    }
    response
}

/// `GET` on every bundled-document path: negotiate the representation,
/// render or serve the bytes.
async fn document(State(state): State<Arc<ServeState>>, headers: HeaderMap, uri: Uri) -> Response {
    let Some(route) = DOCUMENTS.iter().find(|route| route.path == uri.path()) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let canonical = format!("{}{}", origin(&state, &headers), route.path);
    if route.path == "/" {
        return match negotiate(&headers, &[Repr::Html, Repr::Json]) {
            Repr::Json => body(Repr::Json, &canonical, site::DOCS_INDEX.to_string()),
            _ => body(Repr::Html, &canonical, home(&canonical)),
        };
    }
    match negotiate(&headers, &[route.native, Repr::Html]) {
        Repr::Html => {
            let rendered = if route.native == Repr::Markdown {
                site::render_markdown(route.bytes)
            } else {
                format!("<pre>{}</pre>", site::escape(route.bytes))
            };
            body(
                Repr::Html,
                &canonical,
                site::page(route.title, &canonical, &rendered),
            )
        }
        native => body(native, &canonical, route.bytes.to_string()),
    }
}

/// `GET /`: the browsable index of the whole surface.
fn home(canonical: &str) -> String {
    let mut body = String::from(
        "<h1>OpenAgents decision API</h1>\
         <p>The public discovery surface. Every document describes \
         implemented behavior; the <a href=\"/api-catalog.json\">catalog</a> \
         names what does not exist.</p>",
    );
    let row = |path: &str, title: &str| {
        format!(
            "<li><a href=\"{}\"><code>{}</code></a> — {}</li>",
            site::escape(path),
            site::escape(path),
            site::escape(title)
        )
    };
    let sections: Vec<(&str, String)> = vec![
        (
            "Documents",
            DOCUMENTS
                .iter()
                .filter(|route| route.path != "/" && !route.path.starts_with("/plugins/"))
                .map(|route| row(route.path, route.title))
                .collect(),
        ),
        (
            "Generated",
            GENERATED
                .iter()
                .map(|(path, title)| row(path, title))
                .collect(),
        ),
        (
            "Docs API",
            DOCS_API
                .iter()
                .map(|(path, title)| row(path, title))
                .collect(),
        ),
        (
            "Plugins",
            DOCUMENTS
                .iter()
                .filter(|route| route.path.starts_with("/plugins/"))
                .map(|route| row(route.path, route.title))
                .collect(),
        ),
    ];
    for (title, rows) in sections {
        body.push_str(&format!("<h2>{title}</h2><ul>{rows}</ul>"));
    }
    site::page("OpenAgents decision API", canonical, &body)
}

/// `GET /api`: the API index — every surface, its version, its auth.
async fn api_index(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    let origin = origin(&state, &headers);
    let catalog: Value = serde_json::from_str(site::API_CATALOG).unwrap_or_else(|_| json!({}));
    let index = site::api_index(&origin, &catalog);
    let canonical = format!("{origin}/api");
    match negotiate(&headers, &[Repr::Json, Repr::Html]) {
        Repr::Html => {
            let mut rows = String::new();
            for surface in index["surfaces"].as_array().into_iter().flatten() {
                rows.push_str(&format!(
                    "<tr><td><code>{}</code></td><td>{}</td><td>{}</td><td><a href=\"{}\">contract</a></td></tr>",
                    site::escape(surface["path"].as_str().unwrap_or("")),
                    site::escape(surface["v"].as_str().unwrap_or("")),
                    site::escape(surface["auth"].as_str().unwrap_or("")),
                    site::escape(surface["contract"].as_str().unwrap_or("")),
                ));
            }
            let mut links = String::new();
            for (name, url) in index["discovery"].as_object().into_iter().flatten() {
                links.push_str(&format!(
                    "<li><a href=\"{}\">{name}</a></li>",
                    site::escape(url.as_str().unwrap_or(""))
                ));
            }
            body(
                Repr::Html,
                &canonical,
                site::page(
                    "API index",
                    &canonical,
                    &format!(
                        "<h1>API index</h1><table><tr><th>Path</th><th>Schema</th><th>Authentication</th><th>Contract</th></tr>{rows}</table>\
                         <h2>Discovery</h2><ul>{links}</ul>"
                    ),
                ),
            )
        }
        _ => body(
            Repr::Json,
            &canonical,
            serde_json::to_string_pretty(&index).unwrap_or_default(),
        ),
    }
}

/// `GET /.well-known/agent-card.json`: the A2A-format agent card.
async fn agent_card(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    let origin = origin(&state, &headers);
    body(
        Repr::Json,
        &format!("{origin}/.well-known/agent-card.json"),
        serde_json::to_string_pretty(&site::agent_card(&origin)).unwrap_or_default(),
    )
}

/// `GET /.well-known/agent-skills/index.json`: the skills index with
/// digests of the served artifacts.
async fn skills_index(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    let origin = origin(&state, &headers);
    body(
        Repr::Json,
        &format!("{origin}/.well-known/agent-skills/index.json"),
        serde_json::to_string_pretty(&site::skills_index(&origin)).unwrap_or_default(),
    )
}

/// `GET /mcp/server-card.json`: the MCP server's card, mirrored from
/// the same shape `oak-mcp-http` answers at its own `/mcp/card`.
async fn mcp_server_card(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    let origin = origin(&state, &headers);
    body(
        Repr::Json,
        &format!("{origin}/mcp/server-card.json"),
        serde_json::to_string_pretty(&site::mcp_card(site::mcp_tools())).unwrap_or_default(),
    )
}

/// `GET /sitemap.xml`: every public document path, on the served origin.
async fn sitemap(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    let origin = origin(&state, &headers);
    let mut response = (StatusCode::OK, site::sitemap(&origin, &paths())).into_response();
    response.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("application/xml; charset=utf-8"),
    );
    response
}

/// `GET /robots.txt`: crawl rules for the discovery surface.
async fn robots(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    let origin = origin(&state, &headers);
    body(
        Repr::Text,
        &format!("{origin}/robots.txt"),
        site::robots(&origin),
    )
}

/// The query parameters the docs API shares — each optional, matching
/// the corpus tool arguments one for one.
#[derive(Deserialize)]
struct DocsParams {
    cursor: Option<String>,
    limit: Option<usize>,
    q: Option<String>,
    max_bytes: Option<usize>,
}

fn arguments(params: &DocsParams, keys: &[&str]) -> Value {
    let mut arguments = serde_json::Map::new();
    for key in keys {
        let value = match *key {
            "cursor" => params.cursor.clone().map(Value::from),
            "limit" => params.limit.map(|limit| json!(limit)),
            "query" => params.q.clone().map(Value::from),
            "max_bytes" => params.max_bytes.map(|max| json!(max)),
            _ => None,
        };
        if let Some(value) = value {
            arguments.insert((*key).to_string(), value);
        }
    }
    Value::Object(arguments)
}

fn refused(error: corpus::Error) -> Response {
    let status = match error.code {
        "document_not_found" => StatusCode::NOT_FOUND,
        _ => StatusCode::BAD_REQUEST,
    };
    (
        status,
        axum::Json(json!({"error": {"code": error.code, "message": error.message}})),
    )
        .into_response()
}

/// Answer one corpus call as the JSON envelope or a rendered page.
fn docs_answer(
    name: &str,
    state: &ServeState,
    headers: &HeaderMap,
    uri: &Uri,
    arguments: Value,
) -> Response {
    match corpus::call(name, arguments) {
        Ok(result) => {
            let canonical = format!("{}{}", origin(state, headers), uri.path());
            match negotiate(headers, &[Repr::Json, Repr::Html]) {
                Repr::Html => body(
                    Repr::Html,
                    &canonical,
                    site::page(name, &canonical, &docs_html(name, &result)),
                ),
                _ => body(
                    Repr::Json,
                    &canonical,
                    serde_json::to_string_pretty(&result).unwrap_or_default(),
                ),
            }
        }
        Err(error) => refused(error),
    }
}

/// The HTML rendering of a docs-API answer: lists as link rows, a read
/// as the rendered page slice with its pager.
fn docs_html(name: &str, result: &Value) -> String {
    if name == "read_doc" {
        let mut html = format!(
            "<h1>{}</h1>",
            site::escape(result["document"]["title"].as_str().unwrap_or(""))
        );
        let content = result["content"].as_str().unwrap_or("");
        let markdown = result["document"]["source_path"]
            .as_str()
            .is_some_and(|path| path.ends_with(".md"));
        if markdown {
            html.push_str(&site::render_markdown(content));
        } else {
            html.push_str(&format!("<pre>{}</pre>", site::escape(content)));
        }
        if let Some(cursor) = result["next_cursor"].as_str() {
            let id = result["document"]["id"].as_str().unwrap_or("");
            html.push_str(&format!(
                "<p><a href=\"/v1/docs/{}?cursor={}\">next page →</a></p>",
                site::escape(id),
                site::escape(cursor)
            ));
        }
        return html;
    }
    let mut html = format!("<h1>{name}</h1><ul>");
    for doc in result["documents"].as_array().into_iter().flatten() {
        html.push_str(&format!(
            "<li><a href=\"/v1/docs/{}\">{}</a> — {}{}</li>",
            site::escape(doc["id"].as_str().unwrap_or("")),
            site::escape(doc["title"].as_str().unwrap_or("")),
            site::escape(doc["snippet"].as_str().unwrap_or("")),
            if doc["snippet_truncated"].as_bool() == Some(true) {
                "…"
            } else {
                ""
            },
        ));
    }
    html.push_str("</ul>");
    if let Some(cursor) = result["next_cursor"].as_str() {
        html.push_str(&format!(
            "<p><a href=\"?cursor={}\">next page →</a></p>",
            site::escape(cursor)
        ));
    }
    html
}

async fn docs_list(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    uri: Uri,
    Query(params): Query<DocsParams>,
) -> Response {
    docs_answer(
        "list_docs",
        &state,
        &headers,
        &uri,
        arguments(&params, &["cursor", "limit"]),
    )
}

async fn docs_search(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    uri: Uri,
    Query(params): Query<DocsParams>,
) -> Response {
    docs_answer(
        "search_docs",
        &state,
        &headers,
        &uri,
        arguments(&params, &["query", "cursor", "limit"]),
    )
}

async fn docs_examples(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    uri: Uri,
    Query(params): Query<DocsParams>,
) -> Response {
    docs_answer(
        "get_examples",
        &state,
        &headers,
        &uri,
        arguments(&params, &["cursor", "limit"]),
    )
}

async fn doc_read(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    uri: Uri,
    Path(id): Path<String>,
    Query(params): Query<DocsParams>,
) -> Response {
    let mut arguments = arguments(&params, &["cursor", "max_bytes"]);
    arguments["id"] = json!(id);
    docs_answer("read_doc", &state, &headers, &uri, arguments)
}
