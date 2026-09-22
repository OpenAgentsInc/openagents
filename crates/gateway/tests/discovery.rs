//! The public discovery surface, fetched the way an unauthenticated
//! agent fetches it: every document route answers without a credential,
//! content negotiation honors `Accept`, generated documents fold in the
//! served origin, the docs API pages and refuses honestly, and the
//! catalog the origin serves is checked against the routes the router
//! actually mounts.

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::http::StatusCode;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tenancy::{Registry, keys};

use gateway::config::{Config, Door, SCHEMA};
use gateway::serve::{self, ServeState};

/// A deployed gateway with a tunable config: registry, one key for
/// `acme`, one stub backend, and the served state kept in reach for
/// `mounted_paths` checks.
struct Deployment {
    tokens: BTreeMap<String, String>,
    address: String,
    state: Arc<ServeState>,
    _dir: tempfile::TempDir,
}

async fn deploy(tune: impl FnOnce(&mut Config)) -> Deployment {
    let (endpoint, _forwards) =
        common::backend(&common::artifact('a'), StatusCode::OK, common::answer(), 0).await;
    let dir = tempfile::tempdir().unwrap();
    let manifest = common::manifest(&common::artifact('a'), None);
    let registry = Registry::install(dir.path(), manifest.clone()).unwrap();
    let mut tokens = BTreeMap::new();
    for tenant in manifest.tenants.keys() {
        let issued = keys::issue(dir.path(), registry.manifest(), tenant).unwrap();
        tokens.insert(tenant.clone(), issued.token);
    }
    let mut config = Config {
        v: SCHEMA.to_string(),
        listen: "127.0.0.1:0".to_string(),
        registry: dir.path().to_path_buf(),
        require_workspace_membership: false,
        money: None,
        max_body_bytes: 1_048_576,
        max_response_bytes: 4_194_304,
        forward_timeout_ms: 10_000,
        classify_timeout_ms: None,
        max_tenant_classify_in_flight: None,
        reservation_ttl_secs: 300,
        max_in_flight: 8,
        max_classify_inputs: 1024,
        max_classify_inputs_per_tenant: 1024,
        max_questions: 256,
        max_options: 4096,
        doors: ["shared-kev", "acme-kev"]
            .into_iter()
            .map(|door| {
                (
                    door.to_string(),
                    Door {
                        endpoint: endpoint.clone(),
                        classify: None,
                        classify_item_concurrency: 1,
                        batching: None,
                    },
                )
            })
            .collect(),
        job_retention_ms: 604_800_000,
        job_cursor_ttl_ms: 3_600_000,
        public_origin: None,
    };
    tune(&mut config);
    let state = ServeState::open(config).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(axum::serve(listener, serve::router(state.clone())).into_future());
    Deployment {
        tokens,
        address,
        state,
        _dir: dir,
    }
}

async fn get(address: &str, path: &str, accept: Option<&str>) -> reqwest::Response {
    let mut request = reqwest::Client::new().get(format!("{address}{path}"));
    if let Some(accept) = accept {
        request = request.header("accept", accept);
    }
    request.send().await.unwrap()
}

fn content_type(response: &reqwest::Response) -> String {
    response
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string()
}

fn canonical(response: &reqwest::Response) -> Option<String> {
    response
        .headers()
        .get("link")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
}

#[tokio::test]
async fn every_public_route_answers_without_a_credential() {
    let deployment = deploy(|_| {}).await;
    let client = reqwest::Client::new();
    let mut checked = 0usize;
    for path in serve::mounted_paths(&deployment.state) {
        if path.starts_with("/v1/systemone")
            || path.starts_with("/v1/classify")
            || path.starts_with("/v1/jobs")
            || path.starts_with("/v1/models")
            || path == "/v1/docs/{id}"
            || path == "/v1/docs/search"
        {
            continue;
        }
        let response = client
            .get(format!("{}{path}", deployment.address))
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "{path} refused an unauthenticated reader"
        );
        checked += 1;
    }
    // The docs API family — path parameters have no mounted entry, so
    // they are exercised by id here.
    for path in [
        "/v1/docs",
        "/v1/docs/search?q=quota",
        "/v1/docs/examples",
        "/v1/docs/caller",
    ] {
        let response = client
            .get(format!("{}{path}", deployment.address))
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "{path} refused an unauthenticated reader"
        );
        checked += 1;
    }
    assert!(checked >= 30, "only {checked} public routes checked");
}

#[tokio::test]
async fn content_negotiation_serves_each_representation() {
    let deployment = deploy(|_| {}).await;

    // Markdown is the native representation; HTML renders on request.
    let response = get(&deployment.address, "/agents.md", None).await;
    assert_eq!(content_type(&response), "text/markdown; charset=utf-8");
    let response = get(&deployment.address, "/agents.md", Some("text/html")).await;
    assert_eq!(content_type(&response), "text/html; charset=utf-8");
    let body = response.text().await.unwrap();
    assert!(body.contains("<link rel=\"canonical\""));
    assert!(body.contains("<h1"));

    // JSON documents serve natively and render as a page for browsers.
    let response = get(&deployment.address, "/api-catalog.json", None).await;
    assert_eq!(content_type(&response), "application/json; charset=utf-8");
    let catalog: Value = response.json().await.unwrap();
    assert!(catalog["routes"].as_array().unwrap().len() >= 10);
    let response = get(&deployment.address, "/api-catalog.json", Some("text/html")).await;
    assert_eq!(content_type(&response), "text/html; charset=utf-8");

    // The home page negotiates HTML for browsers and the index for
    // JSON clients.
    let response = get(&deployment.address, "/", Some("text/html")).await;
    assert_eq!(content_type(&response), "text/html; charset=utf-8");
    let response = get(&deployment.address, "/", Some("application/json")).await;
    assert_eq!(content_type(&response), "application/json; charset=utf-8");
    let index: Value = response.json().await.unwrap();
    assert_eq!(index["v"], "openagents.agents-index.v1");

    // YAML and plain text serve their own media types.
    let response = get(&deployment.address, "/openapi.yaml", None).await;
    assert_eq!(content_type(&response), "application/yaml; charset=utf-8");
    let body = response.text().await.unwrap();
    assert!(body.starts_with("openapi: 3.1.0"));
    let response = get(&deployment.address, "/llms.txt", None).await;
    assert_eq!(content_type(&response), "text/plain; charset=utf-8");

    // q values are honored: a client preferring JSON over HTML gets JSON.
    let response = get(
        &deployment.address,
        "/api-catalog.json",
        Some("text/html;q=0.1, application/json;q=0.9"),
    )
    .await;
    assert_eq!(content_type(&response), "application/json; charset=utf-8");
}

#[tokio::test]
async fn generated_documents_fold_in_the_served_origin() {
    // Without public_origin the request's Host names the origin.
    let deployment = deploy(|_| {}).await;
    let response = get(&deployment.address, "/sitemap.xml", None).await;
    let sitemap = response.text().await.unwrap();
    assert!(sitemap.contains(&format!(
        "<loc>{}/api-catalog.json</loc>",
        deployment.address
    )));

    let card: Value = get(&deployment.address, "/.well-known/agent-card.json", None)
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(card["url"], format!("{}/v1/systemone", deployment.address));
    assert_eq!(card["capabilities"]["streaming"], false);

    // A declared public_origin replaces the request Host everywhere.
    let deployment = deploy(|config| {
        config.public_origin = Some("https://decisions.example.com".to_string());
    })
    .await;
    let card: Value = get(&deployment.address, "/.well-known/agent-card.json", None)
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(card["url"], "https://decisions.example.com/v1/systemone");
    let response = get(&deployment.address, "/agents.md", None).await;
    assert_eq!(
        canonical(&response).as_deref(),
        Some("<https://decisions.example.com/agents.md>; rel=\"canonical\"")
    );
    let response = get(&deployment.address, "/sitemap.xml", None).await;
    let sitemap = response.text().await.unwrap();
    assert!(sitemap.contains("<loc>https://decisions.example.com/api</loc>"));
    assert!(!sitemap.contains(&deployment.address));
}

#[tokio::test]
async fn the_docs_api_lists_searches_reads_and_pages() {
    let deployment = deploy(|_| {}).await;
    let client = reqwest::Client::new();
    let base = &deployment.address;

    let list: Value = client
        .get(format!("{base}/v1/docs?limit=50"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(list["v"], "openagents.docs.v1");
    assert!(list["corpus_sha256"].as_str().unwrap().len() == 64);
    let documents = list["documents"].as_array().unwrap();
    assert!(documents.len() >= 15);
    for document in documents {
        assert!(document["id"].is_string());
        assert_eq!(document["sha256"].as_str().unwrap().len(), 64);
    }

    // Search returns bounded snippets — one row per matching document.
    let found: Value = client
        .get(format!("{base}/v1/docs/search?q=idempotency"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let rows = found["documents"].as_array().unwrap();
    assert!(!rows.is_empty());
    for row in rows {
        assert!(row["snippet"].as_str().unwrap().len() <= 240);
    }

    // Paged reads reconstruct a document exactly, cursor by cursor.
    let mut content = String::new();
    let mut cursor: Option<String> = None;
    loop {
        let mut url = format!("{base}/v1/docs/caller?max_bytes=200");
        if let Some(cursor) = &cursor {
            url.push_str(&format!("&cursor={cursor}"));
        }
        let page: Value = client.get(&url).send().await.unwrap().json().await.unwrap();
        content.push_str(page["content"].as_str().unwrap());
        match page["next_cursor"].as_str() {
            Some(next) => cursor = Some(next.to_string()),
            None => break,
        }
    }
    assert!(content.contains("/v1/systemone"));

    // Bounded refusals are typed, not crashes.
    let response = client
        .get(format!("{base}/v1/docs/not-a-document"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "document_not_found"
    );
    let response = client
        .get(format!("{base}/v1/docs?limit=0"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let response = client
        .get(format!("{base}/v1/docs?cursor=forged:cursor:7"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "stale_cursor"
    );
}

#[tokio::test]
async fn the_served_catalog_matches_the_mounted_router() {
    let deployment = deploy(|_| {}).await;
    let catalog: Value = get(&deployment.address, "/api-catalog.json", None)
        .await
        .json()
        .await
        .unwrap();
    let cataloged: Vec<String> = catalog["routes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|route| route["path"].as_str().unwrap().to_string())
        .chain(
            catalog["discovery"]["documents"]
                .as_array()
                .unwrap()
                .iter()
                .map(|document| document["path"].as_str().unwrap().to_string()),
        )
        .collect();
    let mounted = serve::mounted_paths(&deployment.state);
    for path in &mounted {
        let listed = cataloged.iter().any(|cataloged| {
            cataloged.as_str() == *path
                || (cataloged.ends_with('/') && path.starts_with(cataloged.as_str()))
        });
        assert!(listed, "mounted route {path} is absent from the catalog");
    }
    for route in catalog["routes"].as_array().unwrap() {
        let path = route["path"].as_str().unwrap();
        if route["condition"].is_string() {
            continue;
        }
        assert!(
            mounted.contains(&path),
            "catalog lists {path} but the router does not mount it"
        );
    }
    // The OpenAPI fragment covers the same API-shaped routes.
    let openapi = get(&deployment.address, "/openapi.yaml", None)
        .await
        .text()
        .await
        .unwrap();
    for path in &mounted {
        if path.starts_with("/v1/") || *path == "/healthz" {
            assert!(
                openapi.contains(&format!("{path}:")),
                "openapi.yaml does not cover {path}"
            );
        }
    }
}

#[tokio::test]
async fn well_known_indexes_digest_the_served_bytes() {
    let deployment = deploy(|_| {}).await;
    let base = &deployment.address;

    let skill = get(
        base,
        "/.well-known/agent-skills/openagents-decision-api/SKILL.md",
        None,
    )
    .await
    .text()
    .await
    .unwrap();
    let digest = format!("sha256:{:x}", Sha256::digest(skill.as_bytes()));
    let index: Value = get(base, "/.well-known/agent-skills/index.json", None)
        .await
        .json()
        .await
        .unwrap();
    let entry = &index["skills"][0];
    assert_eq!(entry["name"], "openagents-decision-api");
    assert_eq!(entry["digest"], digest);
    assert_eq!(
        entry["url"],
        format!("{base}/.well-known/agent-skills/openagents-decision-api/SKILL.md")
    );

    // The MCP server card mirrors the bundled tool snapshot byte for
    // byte — what oak-mcp-http's own GET /mcp/card answers.
    let card: Value = get(base, "/mcp/server-card.json", None)
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(card["v"], "openagents.mcp-server.v1");
    assert_eq!(card["transport"], "streamable-http");
    let snapshot: Value = get(base, "/mcp-tools.json", None)
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(card["tools"], snapshot);
}

#[tokio::test]
async fn sitemap_and_robots_cover_the_public_surface() {
    let deployment = deploy(|_| {}).await;
    let base = &deployment.address;

    let sitemap = get(base, "/sitemap.xml", None).await.text().await.unwrap();
    for path in serve::mounted_paths(&deployment.state) {
        if path.starts_with("/v1/") && path != "/v1/docs" {
            continue;
        }
        if path == "/healthz" {
            continue;
        }
        assert!(
            sitemap.contains(&format!("<loc>{base}{path}</loc>")),
            "sitemap omits {path}"
        );
    }
    let robots = get(base, "/robots.txt", None).await.text().await.unwrap();
    assert!(robots.contains("Disallow: /v1/systemone"));
    assert!(robots.contains(&format!("Sitemap: {base}/sitemap.xml")));
}

#[tokio::test]
async fn plugin_packages_are_served_and_their_manifests_validate() {
    let deployment = deploy(|_| {}).await;
    let base = &deployment.address;
    for path in [
        "/plugins/README.md",
        "/plugins/skills/openagents-decision-api/SKILL.md",
        "/plugins/claude/.claude-plugin/plugin.json",
        "/plugins/claude/.mcp.json",
        "/plugins/claude/skills/openagents-decision-api/SKILL.md",
        "/plugins/codex/.codex-plugin/plugin.json",
        "/plugins/codex/.mcp.json",
        "/plugins/codex/skills/openagents-decision-api/SKILL.md",
    ] {
        let response = get(base, path, None).await;
        assert_eq!(response.status(), StatusCode::OK, "{path}");
    }
    // Every package's skill copy is byte-identical to the artifact the
    // well-known index digests — validation installs nothing.
    assert!(
        discovery::plugins::validate().is_empty(),
        "{:?}",
        discovery::plugins::validate()
    );
}

#[tokio::test]
async fn inference_still_authenticates_while_discovery_stays_open() {
    let deployment = deploy(|_| {}).await;
    let client = reqwest::Client::new();
    let base = &deployment.address;

    // A garbage key on an inference route is a typed 401 — discovery
    // being open does not open inference.
    let response = client
        .post(format!("{base}/v1/systemone"))
        .bearer_auth("oak_forged.forged")
        .json(&json!({
            "model": "shared-kev",
            "state": "a state",
            "questions": {"q1": {"type": "noul", "instructions": "yes?", "criteria": "yes/no"}},
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // The published instructions — auth.md's bearer contract — are the
    // whole setup: the issued key answers /v1/models and a call.
    let token = &deployment.tokens["acme"];
    let models: Value = client
        .get(format!("{base}/v1/models"))
        .bearer_auth(token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let names: Vec<&str> = models["models"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|model| model["id"].as_str())
        .collect();
    assert!(names.contains(&"acme-kev"));
    assert!(names.contains(&"shared-kev"));
}

#[tokio::test]
async fn public_origin_validation_refuses_malformed_config() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = common::manifest(&common::artifact('a'), None);
    Registry::install(dir.path(), manifest).unwrap();
    for origin in [
        "decisions.example.com",
        "https://",
        "https://decisions.example.com/some/path",
        "https://decisions.example.com?query=1",
        "ftp://decisions.example.com",
    ] {
        let config = Config {
            v: SCHEMA.to_string(),
            listen: "127.0.0.1:0".to_string(),
            registry: dir.path().to_path_buf(),
            require_workspace_membership: false,
            money: None,
            max_body_bytes: 1_048_576,
            max_response_bytes: 4_194_304,
            forward_timeout_ms: 10_000,
            classify_timeout_ms: None,
            max_tenant_classify_in_flight: None,
            reservation_ttl_secs: 300,
            max_in_flight: 8,
            max_classify_inputs: 1024,
            max_classify_inputs_per_tenant: 1024,
            max_questions: 256,
            max_options: 4096,
            doors: BTreeMap::new(),
            job_retention_ms: 604_800_000,
            job_cursor_ttl_ms: 3_600_000,
            public_origin: Some(origin.to_string()),
        };
        let name = dir.path().join("gateway.json");
        assert!(
            config.check(&name).is_err(),
            "public_origin {origin} should refuse"
        );
    }
}
