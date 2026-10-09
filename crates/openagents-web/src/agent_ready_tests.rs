//! The agent-readiness surface through the whole router (#11083, #11086):
//! what each checker fetches answers, in the type it asks for.

use std::path::PathBuf;

use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, Method, Request, StatusCode, header};
use serde_json::{Value, json};
use tower::ServiceExt;

use super::*;

const LOCAL: &str = "127.0.0.1:4300";

fn site(root: &std::path::Path) -> Router {
    router(Config::development(PathBuf::from(root).join("tasks")))
}

async fn send(router: Router, request: Request<Body>) -> (StatusCode, HeaderMap, String) {
    let response = router.oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = to_bytes(response.into_body(), 16 * 1024 * 1024)
        .await
        .unwrap();
    (status, headers, String::from_utf8_lossy(&body).into_owned())
}

async fn fetch(router: Router, uri: &str, accept: Option<&str>) -> (StatusCode, HeaderMap, String) {
    let mut request = Request::builder().uri(uri).header(header::HOST, LOCAL);
    if let Some(accept) = accept {
        request = request.header(header::ACCEPT, accept);
    }
    send(router, request.body(Body::empty()).unwrap()).await
}

async fn mcp(router: Router, message: Value) -> (StatusCode, Value) {
    let (status, _, body) = send(
        router,
        Request::builder()
            .method(Method::POST)
            .uri("/mcp/docs")
            .header(header::HOST, LOCAL)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ACCEPT, "application/json, text/event-stream")
            .body(Body::from(message.to_string()))
            .unwrap(),
    )
    .await;
    (status, serde_json::from_str(&body).unwrap_or(Value::Null))
}

fn kind(headers: &HeaderMap) -> &str {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
}

#[tokio::test]
async fn the_agent_documents_answer_in_their_types() {
    let root = tempfile::tempdir().unwrap();
    for (uri, expected) in [
        ("/robots.txt", "text/plain"),
        ("/sitemap.xml", "application/xml"),
        ("/llms.txt", "text/plain"),
        ("/llms-full.txt", "text/plain"),
        ("/auth.md", "text/markdown"),
        ("/index.md", "text/markdown"),
        ("/docs.md", "text/markdown"),
        ("/docs/api.md", "text/markdown"),
        ("/docs/chat.md", "text/markdown"),
        ("/docs/api/quickstart.md", "text/markdown"),
        ("/terms.md", "text/markdown"),
        ("/privacy.md", "text/markdown"),
        ("/download.md", "text/markdown"),
        ("/pricing.md", "text/markdown"),
        ("/.well-known/api-catalog", "application/linkset+json"),
        ("/.well-known/ai-catalog.json", "application/json"),
        ("/.well-known/mcp/server-card.json", "application/json"),
        ("/.well-known/mcp.json", "application/json"),
        ("/.well-known/agent-skills/index.json", "application/json"),
        (
            "/.well-known/agent-skills/openagents-api/SKILL.md",
            "text/markdown",
        ),
        ("/static/webmcp.js", "text/javascript"),
    ] {
        let (status, headers, body) = fetch(site(root.path()), uri, None).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
        assert!(
            kind(&headers).starts_with(expected),
            "{uri}: {}",
            kind(&headers)
        );
        assert!(!body.trim().is_empty(), "{uri}");
    }
    let (status, headers, _) = fetch(site(root.path()), "/pricing", None).await;
    assert_eq!(status, StatusCode::PERMANENT_REDIRECT);
    assert_eq!(headers[header::LOCATION], "/docs/pricing");
    // Without a gateway there is no OpenAPI document to pass on.
    let (status, _, _) = fetch(site(root.path()), "/openapi.json", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn pages_answer_markdown_when_asked_and_say_so_in_headers() {
    let root = tempfile::tempdir().unwrap();
    for (page, heading) in [
        ("/", "# OpenAgents"),
        ("/docs", "# Docs"),
        ("/docs/chat", "# Chat"),
        ("/docs/api", "# API"),
        ("/docs/api/quickstart", "# Quickstart"),
        ("/terms", "# "),
    ] {
        let (status, headers, body) = fetch(site(root.path()), page, Some("text/markdown")).await;
        assert_eq!(status, StatusCode::OK, "{page}");
        assert!(kind(&headers).starts_with("text/markdown"), "{page}");
        assert!(body.contains(heading), "{page}: {body}");
        assert!(
            headers
                .get_all(header::VARY)
                .iter()
                .any(|v| v.to_str().unwrap().contains("Accept")),
            "{page}"
        );
        // The same page as HTML says where its twin and the agent documents are.
        let (_, headers, html) = fetch(site(root.path()), page, Some("text/html")).await;
        assert!(kind(&headers).starts_with("text/html"), "{page}");
        let links: String = headers
            .get_all(header::LINK)
            .iter()
            .map(|v| v.to_str().unwrap().to_owned())
            .collect::<Vec<_>>()
            .join(", ");
        assert!(links.contains("rel=\"api-catalog\""), "{page}: {links}");
        assert!(
            links.contains("rel=\"alternate\"; type=\"text/markdown\""),
            "{page}"
        );
        assert!(
            html.contains("<link rel=\"alternate\" type=\"text/markdown\""),
            "{page}"
        );
        assert!(html.contains("<meta name=\"description\""), "{page}");
        assert!(html.contains("application/ld+json"), "{page}");
    }
    // The guide twins carry frontmatter and a token estimate.
    let (_, headers, body) = fetch(site(root.path()), "/docs/chat.md", None).await;
    assert!(body.starts_with("---\ntitle: "), "{body}");
    assert!(body.contains("url: \"https://openagents.com/docs/chat\""));
    assert!(headers.contains_key("x-markdown-tokens"));
    // A missing page asked for as Markdown says so in Markdown.
    let (status, headers, body) = fetch(site(root.path()), "/nope", Some("text/markdown")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(kind(&headers).starts_with("text/markdown"));
    assert!(body.contains("/llms.txt"));
    // A browser still gets HTML.
    let (_, headers, _) = fetch(
        site(root.path()),
        "/",
        Some("text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"),
    )
    .await;
    assert!(kind(&headers).starts_with("text/html"));
}

#[tokio::test]
async fn the_catalogs_name_the_api_and_the_mcp_server() {
    let root = tempfile::tempdir().unwrap();
    let (_, _, body) = fetch(site(root.path()), "/.well-known/api-catalog", None).await;
    let catalog: Value = serde_json::from_str(&body).unwrap();
    let anchors: Vec<&str> = catalog["linkset"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["anchor"].as_str().unwrap())
        .collect();
    assert!(anchors.contains(&"https://api.openagents.com/v1"));
    assert!(anchors.iter().any(|a| a.ends_with("/mcp/docs")));
    for entry in &catalog["linkset"].as_array().unwrap()[1..] {
        assert!(entry["service-desc"][0]["href"].is_string(), "{entry}");
        assert!(entry["service-doc"][0]["href"].is_string(), "{entry}");
    }
    let (_, headers, body) = fetch(site(root.path()), "/.well-known/ai-catalog.json", None).await;
    assert_eq!(headers[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
    let ard: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(ard["specVersion"], "1.0");
    for entry in ard["entries"].as_array().unwrap() {
        assert!(
            entry["identifier"]
                .as_str()
                .unwrap()
                .starts_with("urn:air:")
        );
        assert!(entry["url"].is_string() && entry.get("data").is_none());
        let queries = entry["representativeQueries"].as_array().unwrap().len();
        assert!((2..=5).contains(&queries), "{entry}");
    }
    let (_, _, body) = fetch(site(root.path()), "/.well-known/mcp/server-card.json", None).await;
    let card: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(card["serverInfo"]["name"], "openagents-docs");
    assert!(
        card["transport"]["endpoint"]
            .as_str()
            .unwrap()
            .ends_with("/mcp/docs")
    );
    assert_eq!(card["tools"].as_array().unwrap().len(), 5);
    let (_, _, body) = fetch(
        site(root.path()),
        "/.well-known/agent-skills/index.json",
        None,
    )
    .await;
    let skills: Value = serde_json::from_str(&body).unwrap();
    let names: Vec<&str> = skills["skills"]
        .as_array()
        .unwrap()
        .iter()
        .map(|skill| skill["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["openagents-decision-api", "openagents-api"]);
}

#[tokio::test]
async fn the_docs_mcp_server_initializes_lists_and_answers_every_tool() {
    let root = tempfile::tempdir().unwrap();
    let (status, reply) = mcp(
        site(root.path()),
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}
        }}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(reply["result"]["protocolVersion"], "2025-03-26");
    assert_eq!(reply["result"]["serverInfo"]["name"], "openagents-docs");
    let (status, _) = mcp(
        site(root.path()),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let (_, reply) = mcp(
        site(root.path()),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
    )
    .await;
    let tools = reply["result"]["tools"].as_array().unwrap();
    for tool in tools {
        assert_eq!(tool["annotations"]["readOnlyHint"], true, "{tool}");
        assert_eq!(tool["inputSchema"]["type"], "object", "{tool}");
    }
    let call = |name: &str, arguments: Value| json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": name, "arguments": arguments}});
    let text = |reply: &Value| {
        reply["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let (_, reply) = mcp(site(root.path()), call("list_docs", json!({}))).await;
    assert!(text(&reply).contains("| api/quickstart |"), "{reply}");
    let (_, reply) = mcp(
        site(root.path()),
        call("search_docs", json!({"query": "API key"})),
    )
    .await;
    assert_eq!(reply["result"]["isError"], false);
    assert!(text(&reply).contains(".md"), "{reply}");
    let (_, reply) = mcp(site(root.path()), call("read_doc", json!({"name": "chat"}))).await;
    assert!(text(&reply).starts_with("# Chat"), "{reply}");
    let (_, reply) = mcp(site(root.path()), call("read_doc", json!({"name": "nope"}))).await;
    assert_eq!(reply["result"]["isError"], true);
    let (_, reply) = mcp(site(root.path()), call("list_models", json!({}))).await;
    assert!(text(&reply).contains('|'), "{reply}");
    let (_, reply) = mcp(site(root.path()), call("no_such_tool", json!({}))).await;
    assert_eq!(reply["error"]["code"], -32602);
    let (_, reply) = mcp(
        site(root.path()),
        json!({"jsonrpc": "2.0", "id": 4, "method": "resources/read", "params": {"uri": "https://openagents.com/docs/chat.md"}}),
    )
    .await;
    assert_eq!(reply["result"]["contents"][0]["mimeType"], "text/markdown");
    let (_, reply) = mcp(
        site(root.path()),
        json!({"jsonrpc": "2.0", "id": 5, "method": "nope"}),
    )
    .await;
    assert_eq!(reply["error"]["code"], -32601);
    // Not JSON at all.
    let (status, _, body) = send(
        site(root.path()),
        Request::builder()
            .method(Method::POST)
            .uri("/mcp/docs")
            .header(header::HOST, LOCAL)
            .body(Body::from("nope"))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("-32700"));
    let (status, headers, _) = fetch(site(root.path()), "/mcp/docs", None).await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(headers[header::ALLOW], "POST, OPTIONS");
}

#[test]
fn the_agent_documents_are_the_sites_own_and_never_proxied() {
    for path in [
        "/robots.txt",
        "/llms.txt",
        "/index.md",
        "/terms.md",
        "/mcp/docs",
        "/.well-known/api-catalog",
        "/.well-known/mcp/server-card.json",
        "/docs/chat.md",
        "/docs/api.md",
    ] {
        assert!(upstream::owned(path), "{path}");
    }
    // The keyed MCP server and its metadata stay with the server behind us.
    assert!(!upstream::owned("/mcp"));
    assert!(!upstream::owned("/.well-known/oauth-protected-resource"));
}

#[tokio::test]
async fn the_agent_documents_have_no_machine_talk() {
    let root = tempfile::tempdir().unwrap();
    for uri in [
        "/llms.txt",
        "/index.md",
        "/docs.md",
        "/docs/api.md",
        "/auth.md",
    ] {
        let (_, _, body) = fetch(site(root.path()), uri, None).await;
        let hits = oa_copy::violations(&body, &[]);
        assert!(hits.is_empty(), "{uri}: {hits:?}");
    }
}

// ---------------------------------------------------------------------
// #11137: every surface names exactly the ways to pay the API takes now

/// A method as the gateway's router describes it (`MethodInfo`).
fn gateway_method(id: &str) -> Value {
    match id {
        "x402" => json!({"id": "x402", "name": "x402 on Lightning", "protocol": "x402",
            "rail": "lightning", "network": "lnbtc:000000000019d6689c085ae165831e93", "asset": "BTC",
            "challenge": "PAYMENT-REQUIRED", "credential": "PAYMENT-SIGNATURE",
            "receipt": "PAYMENT-RESPONSE", "spec": "https://github.com/x402-foundation/x402"}),
        _ => {
            json!({"id": "mpp", "name": "The Payment scheme on Lightning (MPP)", "protocol": "mpp",
            "rail": "lightning", "network": "lnbtc:000000000019d6689c085ae165831e93", "asset": "BTC",
            "challenge": "WWW-Authenticate: Payment", "credential": "Authorization: Payment",
            "receipt": "Payment-Receipt", "spec": "https://paymentauth.org/draft-httpauth-payment-01.txt"})
        }
    }
}

/// A site in front of a stand-in gateway whose OpenAPI document lists
/// `ids` (or no gateway at all for `None`).
async fn site_paying(root: &std::path::Path, ids: Option<&[&str]>) -> Router {
    let mut config = Config::development(PathBuf::from(root).join("tasks"));
    if let Some(ids) = ids {
        let methods: Vec<Value> = ids.iter().map(|id| gateway_method(id)).collect();
        let document = json!({"openapi": "3.1.0", "x-openagents-payment-methods": methods});
        let gateway = axum::Router::new().route(
            "/v1/openapi.json",
            axum::routing::get(move || {
                let document = document.clone();
                async move { axum::Json(document) }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, gateway).await.unwrap() });
        config.inference = Some(std::sync::Arc::new(
            upstream::Upstream::new(&format!("http://{addr}")).unwrap(),
        ));
    }
    router(config)
}

#[tokio::test]
async fn every_surface_lists_exactly_the_ways_to_pay_the_api_takes() {
    let root = tempfile::tempdir().unwrap();
    let cases: [(Option<&[&str]>, &[&str]); 4] = [
        (None, &[]),
        (Some(&[]), &[]),
        (Some(&["x402"]), &["x402"]),
        (Some(&["x402", "mpp"]), &["x402", "mpp"]),
    ];
    for (gateway, expected) in cases {
        let site = || site_paying(root.path(), gateway);
        let named = |text: &str, id: &str| {
            let method = gateway_method(id);
            text.contains(method["credential"].as_str().unwrap())
        };
        let check_text = |uri: &str, text: &str| {
            for id in ["x402", "mpp"] {
                assert_eq!(
                    named(text, id),
                    expected.contains(&id),
                    "{uri} with {gateway:?}: {id}\n{text}"
                );
            }
        };

        let (_, _, llms) = fetch(site().await, "/llms.txt", None).await;
        check_text("/llms.txt", &llms);
        let (_, _, auth) = fetch(site().await, "/auth.md", None).await;
        check_text("/auth.md", &auth);
        assert!(
            oa_copy::violations(&auth, &[]).is_empty(),
            "auth.md machine talk"
        );

        let (_, _, body) = fetch(site().await, "/.well-known/api-catalog", None).await;
        let catalog: Value = serde_json::from_str(&body).unwrap();
        let api = catalog["linkset"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["anchor"] == "https://api.openagents.com/v1")
            .unwrap();
        let listed: Vec<&str> = api[agent_ready::PAYMENT_REL]
            .as_array()
            .map(|links| links.iter().map(|l| l["id"].as_str().unwrap()).collect())
            .unwrap_or_default();
        assert_eq!(listed, expected, "api-catalog with {gateway:?}");
        assert!(
            body.contains("/docs/api/for-agents") && body.contains("/docs/api/pay-per-request")
        );

        let (_, _, body) = fetch(site().await, "/.well-known/ai-catalog.json", None).await;
        let ard: Value = serde_json::from_str(&body).unwrap();
        let api = ard["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["displayName"] == "OpenAgents API")
            .unwrap();
        let listed: Vec<&str> = api["paymentProtocols"]
            .as_array()
            .map(|ids| ids.iter().map(|id| id.as_str().unwrap()).collect())
            .unwrap_or_default();
        assert_eq!(listed, expected, "ai-catalog with {gateway:?}");

        let (_, _, body) = fetch(site().await, "/.well-known/agent-card.json", None).await;
        let card: Value = serde_json::from_str(&body).unwrap();
        let listed: Vec<&str> = card["capabilities"]["extensions"]
            .as_array()
            .and_then(|extensions| {
                extensions
                    .iter()
                    .find(|e| e["uri"] == wellknown::PAYMENTS_EXTENSION)
            })
            .map(|e| {
                e["params"]["methods"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|m| m["id"].as_str().unwrap())
                    .collect()
            })
            .unwrap_or_default();
        assert_eq!(listed, expected, "agent card with {gateway:?}");

        let (_, reply) = mcp(
            site().await,
            json!({"jsonrpc": "2.0", "id": 9, "method": "tools/call",
                   "params": {"name": "list_payment_methods", "arguments": {}}}),
        )
        .await;
        let text = reply["result"]["content"][0]["text"].as_str().unwrap();
        let tool: Value = serde_json::from_str(text).unwrap();
        let listed: Vec<&str> = tool["methods"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["id"].as_str().unwrap())
            .collect();
        assert_eq!(
            listed, expected,
            "MCP list_payment_methods with {gateway:?}"
        );

        let (_, _, page) = fetch(site().await, "/docs/api/for-agents.md", None).await;
        assert!(!page.contains("{{"), "every placeholder is drawn in");
        for id in ["x402", "mpp"] {
            let method = gateway_method(id);
            let row = format!(
                "| [{}]({}) |",
                method["name"].as_str().unwrap(),
                method["spec"].as_str().unwrap()
            );
            assert_eq!(
                page.contains(&row),
                expected.contains(&id),
                "for-agents with {gateway:?}: {id}"
            );
        }
        let (_, _, page) = fetch(site().await, "/docs/api/pay-per-request.md", None).await;
        assert!(!page.contains("{{"));
    }
}
