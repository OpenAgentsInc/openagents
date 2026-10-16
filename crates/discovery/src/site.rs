//! The machine-readable document set an unauthenticated client fetches.
//!
//! Every document here is bundled at build time from `docs/agents/` and
//! `plugins/` — what an origin serves is exactly what the repository
//! holds, digested. The generated documents (agent card, skills index,
//! sitemap, robots, API index, MCP card) are functions of the same
//! bundled bytes, so the surfaces cannot drift apart without the build
//! changing.

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// The `llms.txt` convention entry point.
pub const LLMS_TXT: &str = include_str!("../../../docs/agents/llms.txt");
/// The agent-facing entry document.
pub const AGENTS_MD: &str = include_str!("../../../docs/agents/agents.md");
/// The authentication contract.
pub const AUTH_MD: &str = include_str!("../../../docs/agents/auth.md");
/// What an agent needs to call the API.
pub const SKILL_MD: &str = include_str!("../../../docs/agents/skills.md");
/// The versioned route catalog.
pub const API_CATALOG: &str = include_str!("../../../docs/agents/api-catalog.json");
/// The OpenAPI 3.1 fragment covering the implemented routes.
pub const OPENAPI_YAML: &str = include_str!("../../../docs/agents/openapi.yaml");
/// The discovery document set's own index.
pub const DOCS_INDEX: &str = include_str!("../../../docs/agents/index.json");
/// The snapshot of the MCP server's tool list the mirrored card serves.
/// `oak`'s test suite asserts it equals the live `tools/list` answer.
pub const MCP_TOOLS: &str = include_str!("../../../docs/agents/mcp-tools.json");

/// The lowercase hexadecimal SHA-256 of `bytes`.
#[must_use]
pub fn digest_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// One representation a client may ask for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Repr {
    /// A rendered HTML page.
    Html,
    /// The raw Markdown source.
    Markdown,
    /// The raw JSON document.
    Json,
    /// The raw YAML document.
    Yaml,
    /// The raw plain-text document.
    Text,
}

impl Repr {
    /// The `Content-Type` value the representation answers with.
    #[must_use]
    pub fn content_type(self) -> &'static str {
        match self {
            Repr::Html => "text/html; charset=utf-8",
            Repr::Markdown => "text/markdown; charset=utf-8",
            Repr::Json => "application/json; charset=utf-8",
            Repr::Yaml => "application/yaml; charset=utf-8",
            Repr::Text => "text/plain; charset=utf-8",
        }
    }
}

/// Choose a representation for an `Accept` header, honoring `q` values.
///
/// `offered` lists the representations the document has, most-preferred
/// first; a request with no usable `Accept` gets the first. A header
/// that names nothing the document has yields the first offered — a
/// discovery document is small, and an honest fallback beats a `406`
/// an agent cannot recover from.
#[must_use]
pub fn negotiate(accept: Option<&str>, offered: &[Repr]) -> Repr {
    let Some(accept) = accept else {
        return offered[0];
    };
    let mut best = (offered[0], 0.0_f64);
    for entry in accept.split(',') {
        let mut parts = entry.split(';');
        let media = parts.next().unwrap_or("").trim().to_lowercase();
        let mut q = 1.0_f64;
        for parameter in parts {
            let parameter = parameter.trim();
            if let Some(value) = parameter.strip_prefix("q=")
                && let Ok(value) = value.parse::<f64>()
            {
                q = value;
            }
        }
        for repr in offered {
            let matches = matches!(
                (media.as_str(), repr),
                ("*/*", _)
                    | ("text/*", Repr::Html | Repr::Markdown | Repr::Text)
                    | ("application/*", Repr::Json | Repr::Yaml)
                    | ("text/html", Repr::Html)
                    | ("text/markdown", Repr::Markdown)
                    | ("text/x-markdown", Repr::Markdown)
                    | ("application/json", Repr::Json)
                    | ("application/yaml", Repr::Yaml)
                    | ("application/x-yaml", Repr::Yaml)
                    | ("text/yaml", Repr::Yaml)
                    | ("text/plain", Repr::Text)
            );
            if matches && q > best.1 {
                best = (*repr, q);
            }
        }
    }
    if best.1 == 0.0 { offered[0] } else { best.0 }
}

/// The minimal HTML shell every browsable page shares. Its typeface, Paper
/// Mono, comes from [`paper_mono::WOFF2_PATH`] on the serving origin.
#[must_use]
pub fn page(title: &str, canonical: &str, body: &str) -> String {
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>{}</title><link rel=\"canonical\" href=\"{}\">\
         <style>{}body{{font-family:\"Paper Mono\",monospace;\
         max-width:60em;margin:2em auto;padding:0 1em;line-height:1.5;color:#111}}\
         nav a{{margin-right:1em}}pre{{overflow-x:auto;background:#f6f6f6;padding:1em}}\
         code{{font-size:.95em}}table{{border-collapse:collapse}}\
         td,th{{border:1px solid #ccc;padding:.2em .6em;text-align:left}}</style>\
         </head><body><nav><a href=\"/\">discovery</a><a href=\"/api\">api</a>\
         <a href=\"/agents.md\">agents</a><a href=\"/auth.md\">auth</a>\
         <a href=\"/api-catalog.json\">catalog</a><a href=\"/openapi.yaml\">openapi</a>\
         <a href=\"/v1/docs\">docs</a></nav><main>{}</main></body></html>",
        escape(title),
        escape(canonical),
        paper_mono::font_face(paper_mono::WOFF2_PATH),
        body
    )
}

/// Render Markdown to an HTML fragment with pulldown-cmark.
#[must_use]
pub fn render_markdown(markdown: &str) -> String {
    let parser = pulldown_cmark::Parser::new(markdown);
    let mut body = String::with_capacity(markdown.len() * 2);
    pulldown_cmark::html::push_html(&mut body, parser);
    body
}

/// Escape the five HTML-significant characters.
#[must_use]
pub fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// The A2A agent card for this service. On the public origins
/// (`openagents.com`, `api.openagents.com`) it describes OpenAgents: our
/// API, Decisions, agent self-signup with `oak_` keys, projects, Coder,
/// the docs MCP server, and Lightning payments, as the `/docs/agents`
/// pages do. TypeSafe's System One decision API (`api.typesafe.ai`, which
/// the public card named as its identity before; #10364) stays on it as a
/// separate service's skill. A self-hosted decision service keeps a card
/// for its own origin. Neither speaks A2A message flows: the card names
/// what exists, and `capabilities` declines what does not.
#[must_use]
pub fn agent_card(origin: &str) -> Value {
    let public = matches!(
        origin.trim_end_matches('/'),
        "https://openagents.com" | "https://api.openagents.com"
    );
    if public {
        return public_agent_card();
    }
    let origin = origin.trim_end_matches('/');
    json!({
        "name": "OpenAgents decision API",
        "description": "An HTTP service that answers typed questions (yes/no, choice, and score) with probabilities, and classifies batches of text. Calls need an API key.",
        "version": env!("CARGO_PKG_VERSION"),
        "protocolVersion": "openagents.systemone.v1",
        "provider": {
            "organization": "OpenAgents",
            "url": "https://github.com/OpenAgentsInc/openagents",
        },
        "url": format!("{origin}/v1/systemone"),
        "supportedInterfaces": [
            {"url": format!("{origin}/v1/systemone"), "protocolBinding": "HTTP+JSON", "protocolVersion": "openagents.systemone.v1"},
            {"url": format!("{origin}/v1/classify"), "protocolBinding": "HTTP+JSON", "protocolVersion": "openagents.classify.v1"},
        ],
        "capabilities": {
            "streaming": false,
            "pushNotifications": false,
            "extendedAgentCard": false,
        },
        "securitySchemes": {
            "bearer": {"type": "http", "scheme": "bearer", "description": "An API key in the `oak_<id>.<secret>` format. See /auth.md to get one."}
        },
        "security": [{"bearer": []}],
        "defaultInputModes": ["application/json"],
        "defaultOutputModes": ["application/json"],
        "skills": [
            {"id": "typed-decision", "name": "Typed decision", "description": "Answer yes/no, choice, and score questions about a state you send, each with probabilities.", "tags": ["decision", "classification", "scoring"]},
            {"id": "batch-classification", "name": "Batch classification", "description": "Classify batches of inputs with one label, several labels, several dimensions, a yes-or-no label, or a score.", "tags": ["classification", "batch"]},
            {"id": "durable-jobs", "name": "Durable jobs", "description": "Run a batch as a job that you can check, cancel, and download results from later.", "tags": ["jobs", "batch"]},
            {"id": "documentation", "name": "Bundled documentation", "description": "List, read, and search the OpenAgents documentation. No API key needed.", "tags": ["docs"]},
        ],
        "documentationUrl": format!("{origin}/agents.md"),
    })
}

/// OpenAgents' own card, the same on both public origins.
fn public_agent_card() -> Value {
    const SITE: &str = "https://openagents.com";
    const API: &str = "https://api.openagents.com/v1";
    json!({
        "name": "OpenAgents",
        "description": "Many models through one API (Google's Gemini first, OpenAI-compatible), free typed answers from our Decisions API, projects, files, and chats for the person an agent works for, and Coder, our coding agent. An agent can sign itself up, with no person or card, and get an oak_ key and a free amount each day; past that it pays in bitcoin over Lightning.",
        "version": env!("CARGO_PKG_VERSION"),
        "protocolVersion": "openagents.api.v1",
        "provider": {
            "organization": "OpenAgents",
            "url": SITE,
        },
        "url": API,
        "supportedInterfaces": [
            {"url": format!("{API}/responses"), "protocolBinding": "HTTP+JSON", "protocolVersion": "openresponses.v1"},
            {"url": format!("{API}/chat/completions"), "protocolBinding": "HTTP+JSON", "protocolVersion": "openai.chat-completions.v1"},
            {"url": format!("{API}/decisions"), "protocolBinding": "HTTP+JSON", "protocolVersion": "openagents.decisions.v1"},
            {"url": format!("{SITE}/mcp/docs"), "protocolBinding": "MCP", "protocolVersion": "2025-06-18"},
        ],
        "capabilities": {
            "streaming": true,
            "pushNotifications": false,
            "extendedAgentCard": false,
        },
        "securitySchemes": {
            "bearer": {"type": "http", "scheme": "bearer", "description": format!("An API key, `oak_<id>.<secret>`. An agent gets its own with no person: GET {API}/agents/challenge, solve it, then POST {API}/agents/register ({SITE}/docs/agents/sign-up). A person makes one at {SITE}/settings/api-keys. Projects, files, and chats take a login key (`oa_sk_`) from {SITE}/settings. Keys and scopes: {SITE}/docs/agents/scopes.")}
        },
        "security": [{"bearer": []}],
        "defaultInputModes": ["application/json", "text/plain"],
        "defaultOutputModes": ["application/json", "text/event-stream"],
        "skills": [
            {"id": "models", "name": "Models", "description": format!("Answer with many models through one API, Google's Gemini first: POST {API}/responses or {API}/chat/completions, one answer or a stream. OpenAI SDKs work with base_url {API}. {SITE}/docs/agents/chat-and-stream"), "tags": ["inference", "models", "streaming"]},
            {"id": "decisions", "name": "Decisions", "description": format!("Typed answers in a fraction of a second: the chance a condition is true, one choice from a set, or a score. POST {API}/decisions, no key, free. {SITE}/docs/agents/decisions"), "tags": ["decision", "classification", "routing"]},
            {"id": "agent-signup", "name": "Sign up as an agent", "description": format!("Make an account with no person, email, or card: solve a proof of work at {API}/agents/challenge and register at {API}/agents/register for an oak_ key, a claim code for the person you work for, and a daily free amount. {SITE}/docs/agents/sign-up"), "tags": ["signup", "keys"]},
            {"id": "projects", "name": "Projects, files, and chats", "description": format!("Read and change the projects, files, and chats of the person you work for at {API}/projects and {API}/threads, with their login key. {SITE}/docs/agents/projects"), "tags": ["projects", "files", "chats"]},
            {"id": "coder", "name": "Coder", "description": format!("Run Coder, our coding agent, from a script: openagents coder chat -p \"...\" --json. {SITE}/docs/agents/coder-headless"), "tags": ["coding", "cli"]},
            {"id": "documentation", "name": "Documentation", "description": format!("List, read, and search our docs as MCP tools at {SITE}/mcp/docs, no key. Every page is also Markdown: {SITE}/llms.txt. {SITE}/docs/agents/mcp"), "tags": ["docs", "mcp"]},
            {"id": "lightning-payments", "name": "Pay in bitcoin", "description": format!("Buy credit over Lightning ({SITE}/docs/agents/buy-credits), or pay each request with no key over x402 ({SITE}/docs/agents/x402)."), "tags": ["payments", "lightning", "x402"]},
            {"id": "typesafe-system-one", "name": "TypeSafe System One", "description": "A separate service: typed yes/no, choice, and score answers and batch classification from TypeSafe's System One models, at https://api.typesafe.ai/v1/systemone and https://api.typesafe.ai/v1/classify, with a TypeSafe key. https://docs.typesafe.ai/", "tags": ["decision", "classification", "external"]},
        ],
        "documentationUrl": format!("{SITE}/docs/agents"),
    })
}

/// The `.well-known/agent-skills/index.json` document: the canonical
/// skill artifact with the digest of exactly the bytes served.
#[must_use]
pub fn skills_index(origin: &str) -> Value {
    json!({
        "$schema": "https://schemas.agentskills.io/discovery/0.2.0/schema.json",
        "skills": [{
            "name": "openagents-decision-api",
            "type": "skill-md",
            "description": "Call the OpenAgents decision API for typed decisions, classification, and batch jobs through oak, oak-mcp, or HTTP, and authenticate with an API key.",
            "url": format!("{origin}/.well-known/agent-skills/openagents-decision-api/SKILL.md"),
            "digest": format!("sha256:{}", digest_hex(crate::plugins::CANONICAL_SKILL.as_bytes())),
        }],
    })
}

/// The `sitemap.xml` for the discovery surface — one entry per public
/// document route, built on the served origin.
#[must_use]
pub fn sitemap(origin: &str, paths: &[&str]) -> String {
    let mut body = String::with_capacity(paths.len() * 64);
    for path in paths {
        body.push_str(&format!(
            "  <url><loc>{}{}</loc></url>\n",
            escape(origin),
            path
        ));
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n{body}</urlset>\n"
    )
}

/// The `robots.txt`: crawlers may read discovery and are told the
/// sitemap; inference routes are not crawl targets.
#[must_use]
pub fn robots(origin: &str) -> String {
    format!(
        "User-agent: *\nDisallow: /v1/systemone\nDisallow: /v1/classify\nDisallow: /v1/jobs\n\nSitemap: {origin}/sitemap.xml\n"
    )
}

/// The `/api` document: every API surface this origin serves, its
/// version tag, and where its contract lives. Discovery describes; it
/// never authenticates.
#[must_use]
pub fn api_index(origin: &str, catalog: &Value) -> Value {
    json!({
        "v": "openagents.api-index.v1",
        "name": "OpenAgents decision API",
        "version": env!("CARGO_PKG_VERSION"),
        "origin": origin,
        "surfaces": [
            {"path": "/v1/systemone", "v": "openagents.systemone.v1", "auth": "bearer", "contract": format!("{origin}/openapi.yaml")},
            {"path": "/v1/classify", "v": "openagents.classify.v1", "auth": "bearer", "contract": format!("{origin}/openapi.yaml")},
            {"path": "/v1/jobs", "v": "openagents.job.v1", "auth": "bearer", "contract": format!("{origin}/openapi.yaml")},
            {"path": "/v1/models", "response": "{\"models\": [...]}", "auth": "bearer", "contract": format!("{origin}/openapi.yaml")},
            {"path": "/v1/docs", "v": "openagents.docs.v1", "auth": "none", "contract": format!("{origin}/api-catalog.json")},
            {"path": "/healthz", "response": "{\"status\": \"ok\"}", "auth": "none", "contract": format!("{origin}/api-catalog.json")},
        ],
        "catalog": catalog,
        "discovery": {
            "llms_txt": format!("{origin}/llms.txt"),
            "agents": format!("{origin}/agents.md"),
            "auth": format!("{origin}/auth.md"),
            "skill": format!("{origin}/skills.md"),
            "openapi": format!("{origin}/openapi.yaml"),
            "api_catalog": format!("{origin}/api-catalog.json"),
            "agent_card": format!("{origin}/.well-known/agent-card.json"),
            "agent_skills": format!("{origin}/.well-known/agent-skills/index.json"),
            "mcp_server_card": format!("{origin}/mcp/server-card.json"),
            "sitemap": format!("{origin}/sitemap.xml"),
        },
    })
}

/// The MCP server's published name — `oak`'s initialize answer and
/// both server cards share it.
pub const MCP_SERVER_NAME: &str = "oak-mcp";
/// The MCP protocol versions the transport negotiates, most-preferred
/// first — `oak`'s session enforcement reads the same list.
pub const MCP_PROTOCOL_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18"];
/// How long an idle MCP session lives — `oak`'s reaper enforces the
/// same bound the card publishes.
pub const MCP_SESSION_TTL_SECS: u64 = 1_800;
/// The most live MCP sessions the transport holds — `oak`'s mint
/// refuses past the same bound the card publishes.
pub const MCP_SESSION_BOUND: usize = 256;

/// The MCP server card the mirrored `/mcp/server-card.json` route
/// serves and `oak-mcp-http` answers at `GET /mcp/card` — one shape,
/// one set of negotiated constants, different tool lists only if a
/// build drifts (which `oak`'s test suite refuses).
#[must_use]
pub fn mcp_card(tools: Value) -> Value {
    json!({
        "v": "openagents.mcp-server.v1",
        "name": MCP_SERVER_NAME,
        "title": "oak, the OpenAgents decision API client",
        "version": env!("CARGO_PKG_VERSION"),
        "transport": "streamable-http",
        "endpoint": "/mcp",
        "protocol_versions": MCP_PROTOCOL_VERSIONS,
        "session": {
            "header": "Mcp-Session-Id",
            "issued_on": "initialize",
            "terminate": "DELETE /mcp",
            "idle_ttl_seconds": MCP_SESSION_TTL_SECS,
            "bound": MCP_SESSION_BOUND,
        },
        "auth": {
            "scheme": "bearer",
            "detail": "Send an `oak_` API key; the server uses it for that call only. Without one, the server uses the OPENAGENTS_API_KEY or config file it was started with.",
        },
        "tools": tools,
    })
}

/// The bundled snapshot of `tools/list` the mirrored card serves.
///
/// # Panics
///
/// Never — the file is a fixture this crate's tests parse.
#[must_use]
pub fn mcp_tools() -> Value {
    serde_json::from_str(MCP_TOOLS).expect("mcp-tools.json is valid JSON")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_cards_describe_openagents_with_typesafe_as_a_separate_skill() {
        for origin in ["https://openagents.com", "https://api.openagents.com"] {
            let card = agent_card(origin);
            assert_eq!(card["name"], "OpenAgents");
            assert_eq!(card["url"], "https://api.openagents.com/v1");
            assert_eq!(card["provider"]["url"], "https://openagents.com");
            assert_eq!(
                card["documentationUrl"],
                "https://openagents.com/docs/agents"
            );
            let interfaces: Vec<&str> = card["supportedInterfaces"]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| i["url"].as_str().unwrap())
                .collect();
            assert_eq!(
                interfaces,
                [
                    "https://api.openagents.com/v1/responses",
                    "https://api.openagents.com/v1/chat/completions",
                    "https://api.openagents.com/v1/decisions",
                    "https://openagents.com/mcp/docs",
                ]
            );
            assert!(interfaces.iter().all(|url| !url.contains("typesafe")));
            let auth = card["securitySchemes"]["bearer"]["description"]
                .as_str()
                .unwrap();
            assert!(auth.contains("oak_") && auth.contains("/v1/agents/register"));
            let skills: Vec<&str> = card["skills"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s["id"].as_str().unwrap())
                .collect();
            for id in ["models", "decisions", "agent-signup", "projects", "coder"] {
                assert!(skills.contains(&id), "{id}: {skills:?}");
            }
            // TypeSafe is advertised only as a separate service's skill.
            assert_eq!(skills.last(), Some(&"typesafe-system-one"));
            let typesafe = &card["skills"][skills.len() - 1]["description"];
            assert!(
                typesafe
                    .as_str()
                    .unwrap()
                    .contains("https://api.typesafe.ai/v1/systemone")
            );
        }
    }

    #[test]
    fn self_hosted_cards_keep_their_own_endpoints() {
        let card = agent_card("http://localhost:8080");
        assert_eq!(card["url"], "http://localhost:8080/v1/systemone");
        assert_eq!(
            card["supportedInterfaces"][1]["url"],
            "http://localhost:8080/v1/classify"
        );
    }
}
