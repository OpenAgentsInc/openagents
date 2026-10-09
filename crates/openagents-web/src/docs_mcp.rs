//! The public docs MCP server at `/mcp/docs` (#11086): Streamable HTTP,
//! stateless, no key. `POST` carries one JSON-RPC message (or a batch) and
//! the answer is JSON; `GET` answers `405` because the server never sends
//! anything unasked. Its tools only read: the guides compiled into this
//! binary and the API's rate card.
//!
//! The keyed `/mcp` stays with the server behind this site.

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use serde_json::{Value, json};

use crate::App;
use crate::agent_ready::{MCP_PATH, SITE, summary};
use crate::markdown;
use crate::pages::api_docs::API_DOCS;
use crate::pages::content::DOCS;

/// The protocol versions this server speaks, newest first.
const VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

const SERVER_NAME: &str = "openagents-docs";
const SERVER_TITLE: &str = "OpenAgents docs";

const INSTRUCTIONS: &str = "Read-only tools for the OpenAgents docs and the OpenAgents API's \
model prices. Use list_docs to see every guide, search_docs to find a word or phrase, read_doc \
to read one guide as Markdown, and list_models for the API's models and prices.";

/// The most search results one call returns.
const MAX_RESULTS: usize = 20;

pub(crate) fn routes() -> Router<App> {
    Router::new().route(
        MCP_PATH,
        post(rpc)
            .get(not_allowed)
            .delete(not_allowed)
            .options(preflight),
    )
}

/// The tools, as `tools/list` and the server card list them.
pub(crate) fn tools() -> Value {
    let read_only = json!({
        "readOnlyHint": true,
        "destructiveHint": false,
        "idempotentHint": true,
        "openWorldHint": false
    });
    json!([
        {
            "name": "list_docs",
            "title": "List the docs",
            "description": "List every OpenAgents guide and API guide: its name (pass it to read_doc), title, address, and a one-line summary.",
            "inputSchema": {"type": "object", "properties": {}, "additionalProperties": false},
            "annotations": read_only
        },
        {
            "name": "search_docs",
            "title": "Search the docs",
            "description": "Find the OpenAgents guides that mention a word or phrase. Returns each match's name, title, address, and the line it was found on.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {"type": "string", "minLength": 1, "description": "Words to find, such as \"connect a computer\" or \"API key\"."},
                    "limit": {"type": "integer", "minimum": 1, "maximum": MAX_RESULTS, "default": 5, "description": "Most results to return."}
                },
                "required": ["query"],
                "additionalProperties": false
            },
            "annotations": read_only
        },
        {
            "name": "read_doc",
            "title": "Read a guide",
            "description": "Read one OpenAgents guide as Markdown, by the name list_docs gives (such as \"chat\" or \"api/quickstart\").",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": {"type": "string", "minLength": 1, "description": "The guide's name from list_docs."}
                },
                "required": ["name"],
                "additionalProperties": false
            },
            "annotations": read_only
        },
        {
            "name": "list_models",
            "title": "List models and prices",
            "description": "List the OpenAgents API's models and what each costs per million tokens, as a Markdown table.",
            "inputSchema": {"type": "object", "properties": {}, "additionalProperties": false},
            "annotations": read_only
        },
        {
            "name": "list_payment_methods",
            "title": "List ways to pay",
            "description": "List the ways the OpenAgents API takes payment per request with no key right now, as JSON: each method's name, the 402 header it answers with, the header to send back, and the receipt header.",
            "inputSchema": {"type": "object", "properties": {}, "additionalProperties": false},
            "annotations": read_only
        }
    ])
}

/// The MCP server card (SEP-1649) at `/.well-known/mcp/server-card.json`.
pub(crate) fn server_card(origin: &str) -> Value {
    json!({
        "$schema": "https://static.modelcontextprotocol.io/schemas/mcp-server-card/v1.json",
        "version": "1.0",
        "protocolVersion": VERSIONS[0],
        "serverInfo": {
            "name": SERVER_NAME,
            "title": SERVER_TITLE,
            "version": env!("CARGO_PKG_VERSION")
        },
        "description": "Read and search the OpenAgents docs and the OpenAgents API's model prices. No key needed.",
        "documentationUrl": format!("{origin}/llms.txt"),
        "websiteUrl": format!("{origin}/"),
        "transport": {
            "type": "streamable-http",
            "endpoint": format!("{origin}{MCP_PATH}")
        },
        "remotes": [{"type": "streamable-http", "url": format!("{origin}{MCP_PATH}")}],
        "capabilities": {
            "tools": {"listChanged": false},
            "resources": {"listChanged": false}
        },
        "authentication": {"required": false, "schemes": []},
        "instructions": INSTRUCTIONS,
        "tools": tools()
    })
}

/// One guide: its name for the tools, title, address, and Markdown.
struct Guide {
    name: String,
    title: String,
    path: String,
    source: &'static str,
}

fn guides() -> Vec<Guide> {
    let docs = DOCS.iter().map(|(slug, source)| Guide {
        name: (*slug).to_owned(),
        title: markdown::title(source, slug),
        path: format!("/docs/{slug}"),
        source,
    });
    let api = API_DOCS.iter().map(|(slug, source)| Guide {
        name: format!("api/{slug}"),
        title: markdown::title(source, slug),
        path: format!("/docs/api/{slug}"),
        source,
    });
    docs.chain(api).collect()
}

/// A JSON-RPC error.
struct Fault {
    code: i64,
    message: String,
}

impl Fault {
    fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

async fn rpc(State(app): State<App>, body: Bytes) -> Response {
    let message: Value = match serde_json::from_slice(&body) {
        Ok(message) => message,
        Err(_) => {
            return answer(
                StatusCode::BAD_REQUEST,
                error_reply(&Value::Null, Fault::new(-32700, "The body isn't JSON.")),
            );
        }
    };
    match message {
        Value::Array(batch) if !batch.is_empty() => {
            let mut replies = Vec::new();
            for message in batch {
                if let Some(reply) = handle(&app, message).await {
                    replies.push(reply);
                }
            }
            if replies.is_empty() {
                return accepted();
            }
            answer(StatusCode::OK, Value::Array(replies))
        }
        Value::Object(_) => match handle(&app, message).await {
            Some(reply) => answer(StatusCode::OK, reply),
            None => accepted(),
        },
        _ => answer(
            StatusCode::BAD_REQUEST,
            error_reply(
                &Value::Null,
                Fault::new(-32600, "Send one JSON-RPC message or a batch."),
            ),
        ),
    }
}

/// One message's reply, or `None` for a notification or a response.
async fn handle(app: &App, message: Value) -> Option<Value> {
    let id = message.get("id").cloned();
    let Some(method) = message.get("method").and_then(Value::as_str) else {
        // A response to nothing we asked, or a malformed message.
        return id.map(|id| error_reply(&id, Fault::new(-32600, "A request needs a method.")));
    };
    let id = id?;
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    let result = match method {
        "initialize" => Ok(initialize(&params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools": tools()})),
        "tools/call" => call(app, &params).await,
        "resources/list" => Ok(resources()),
        "resources/read" => read_resource(app, &params).await,
        "resources/templates/list" => Ok(json!({"resourceTemplates": []})),
        "prompts/list" => Ok(json!({"prompts": []})),
        other => Err(Fault::new(-32601, format!("No method named {other}."))),
    };
    Some(match result {
        Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
        Err(fault) => error_reply(&id, fault),
    })
}

fn initialize(params: &Value) -> Value {
    let asked = params.get("protocolVersion").and_then(Value::as_str);
    let version = asked
        .filter(|asked| VERSIONS.contains(asked))
        .unwrap_or(VERSIONS[0]);
    json!({
        "protocolVersion": version,
        "capabilities": {
            "tools": {"listChanged": false},
            "resources": {"listChanged": false}
        },
        "serverInfo": {
            "name": SERVER_NAME,
            "title": SERVER_TITLE,
            "version": env!("CARGO_PKG_VERSION"),
            "websiteUrl": format!("{SITE}/")
        },
        "instructions": INSTRUCTIONS
    })
}

async fn call(app: &App, params: &Value) -> Result<Value, Fault> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| Fault::new(-32602, "tools/call needs a tool name."))?;
    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
    if !arguments.is_object() {
        return Err(Fault::new(-32602, "The arguments must be an object."));
    }
    let text = match name {
        "list_docs" => Ok(list_docs()),
        "search_docs" => search_docs(&arguments),
        "read_doc" => read_doc(app, &arguments).await,
        "list_models" => Ok(list_models(app).await),
        "list_payment_methods" => Ok(list_payment_methods(app).await),
        other => return Err(Fault::new(-32602, format!("No tool named {other}."))),
    };
    Ok(match text {
        Ok(text) => json!({"content": [{"type": "text", "text": text}], "isError": false}),
        // A tool's own failure is a result the model can read and fix.
        Err(message) => json!({"content": [{"type": "text", "text": message}], "isError": true}),
    })
}

fn list_docs() -> String {
    let mut out = String::from("| Name | Title | Address | Summary |\n| --- | --- | --- | --- |\n");
    for guide in guides() {
        out.push_str(&format!(
            "| {} | {} | {SITE}{}.md | {} |\n",
            guide.name,
            guide.title,
            guide.path,
            summary(guide.source).replace('|', "/")
        ));
    }
    out
}

fn search_docs(arguments: &Value) -> Result<String, String> {
    let query = arguments
        .get("query")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|query| !query.is_empty())
        .ok_or("Give a query: the words to find.")?;
    let limit = match arguments.get("limit") {
        None | Some(Value::Null) => 5,
        Some(value) => value
            .as_u64()
            .filter(|limit| (1..=MAX_RESULTS as u64).contains(limit))
            .ok_or(format!("The limit must be from 1 to {MAX_RESULTS}."))?
            as usize,
    };
    let needle = query.to_lowercase();
    let mut hits = Vec::new();
    for guide in guides() {
        let in_title = guide.title.to_lowercase().contains(&needle);
        let lines: Vec<&str> = guide
            .source
            .lines()
            .filter(|line| !line.trim_start().starts_with('#'))
            .filter(|line| line.to_lowercase().contains(&needle))
            .collect();
        if !in_title && lines.is_empty() {
            continue;
        }
        let score = usize::from(in_title) * 10 + lines.len();
        let line = lines
            .first()
            .map_or_else(|| summary(guide.source), |line| line.trim().to_owned());
        hits.push((score, guide, line));
    }
    hits.sort_by(|a, b| b.0.cmp(&a.0));
    if hits.is_empty() {
        return Ok(format!(
            "No guide mentions \"{query}\". Try other words, or list_docs to see every guide."
        ));
    }
    let mut out = String::new();
    for (_, guide, line) in hits.into_iter().take(limit) {
        out.push_str(&format!(
            "- **{}** (`{}`, {SITE}{}.md): {}\n",
            guide.title, guide.name, guide.path, line
        ));
    }
    Ok(out)
}

async fn read_doc(app: &App, arguments: &Value) -> Result<String, String> {
    let name = arguments
        .get("name")
        .and_then(Value::as_str)
        .map(|name| {
            name.trim()
                .trim_start_matches("/docs/")
                .trim_end_matches(".md")
        })
        .filter(|name| !name.is_empty())
        .ok_or("Give the guide's name from list_docs.")?;
    let guide = guides()
        .into_iter()
        .find(|guide| guide.name == name)
        .ok_or(format!(
            "No guide is named \"{name}\". list_docs gives every name."
        ))?;
    Ok(crate::pages::api_docs::source(app, guide.source).await)
}

async fn list_payment_methods(app: &App) -> String {
    let methods = crate::payments::live(app).await;
    serde_json::to_string_pretty(&json!({
        "api": "https://api.openagents.com/v1",
        "summary": crate::payments::sentence(&methods),
        "methods": methods,
        "docs": format!("{SITE}/docs/api/for-agents.md"),
    }))
    .unwrap_or_default()
}

async fn list_models(app: &App) -> String {
    let card = crate::pages::api_docs::card(app).await;
    format!(
        "Prices in US dollars per million tokens. Base URL: https://api.openagents.com/v1\n\n{}",
        crate::pages::api_docs::rate_card(&card)
    )
}

fn resources() -> Value {
    let resources: Vec<Value> = guides()
        .into_iter()
        .map(|guide| {
            json!({
                "uri": format!("{SITE}{}.md", guide.path),
                "name": guide.name,
                "title": guide.title,
                "description": summary(guide.source),
                "mimeType": "text/markdown"
            })
        })
        .collect();
    json!({"resources": resources})
}

async fn read_resource(app: &App, params: &Value) -> Result<Value, Fault> {
    let uri = params
        .get("uri")
        .and_then(Value::as_str)
        .ok_or_else(|| Fault::new(-32602, "resources/read needs a uri."))?;
    let guide = guides()
        .into_iter()
        .find(|guide| format!("{SITE}{}.md", guide.path) == uri)
        .ok_or_else(|| Fault::new(-32002, format!("No resource at {uri}.")))?;
    let text = crate::pages::api_docs::source(app, guide.source).await;
    Ok(json!({"contents": [{"uri": uri, "mimeType": "text/markdown", "text": text}]}))
}

fn error_reply(id: &Value, fault: Fault) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": fault.code, "message": fault.message}})
}

fn cors(response: &mut Response) {
    let headers = response.headers_mut();
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    headers.insert(
        header::ACCESS_CONTROL_EXPOSE_HEADERS,
        HeaderValue::from_static("Mcp-Session-Id, MCP-Protocol-Version"),
    );
}

fn answer(status: StatusCode, body: Value) -> Response {
    let mut response = (
        status,
        [(header::CONTENT_TYPE, "application/json")],
        body.to_string(),
    )
        .into_response();
    cors(&mut response);
    response
}

fn accepted() -> Response {
    let mut response = StatusCode::ACCEPTED.into_response();
    cors(&mut response);
    response
}

async fn not_allowed() -> Response {
    let mut response = (
        StatusCode::METHOD_NOT_ALLOWED,
        [(header::ALLOW, "POST, OPTIONS")],
        "This MCP server answers POST only.",
    )
        .into_response();
    cors(&mut response);
    response
}

async fn preflight() -> Response {
    let mut response = StatusCode::NO_CONTENT.into_response();
    cors(&mut response);
    let headers = response.headers_mut();
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("POST, OPTIONS"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static(
            "Content-Type, Accept, MCP-Protocol-Version, Mcp-Session-Id, Last-Event-ID",
        ),
    );
    response
}
