//! The public rate card (#11066): `GET /v1/rates` is the meter's rate
//! card with no key needed, the same card the website's models page
//! draws, and `GET /v1/models` carries each model's rows.

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use inference::rates::{Card, SatsRate};
use inference::request::CreateResponse;
use inference::upstream::{
    Account, AttemptError, BoxFuture, Capabilities, CostBasis, ErrorClass, ModelRow, Price,
    PrivacyTerms, Sent, Upstream,
};
use serde_json::{Value, json};
use tenancy::{Registry, keys};

use gateway::config::{Config, Inference, SCHEMA};
use gateway::serve::{self, ServeState};

/// An adapter that lists one model and is never sent a request.
struct Listed {
    account: Account,
    privacy: PrivacyTerms,
    models: Vec<ModelRow>,
}

impl Upstream for Listed {
    fn name(&self) -> &'static str {
        "zai"
    }
    fn account(&self) -> &Account {
        &self.account
    }
    fn privacy(&self) -> &PrivacyTerms {
        &self.privacy
    }
    fn models(&self) -> &[ModelRow] {
        &self.models
    }
    fn configured(&self) -> bool {
        true
    }
    fn send<'a>(
        &'a self,
        _request: &'a CreateResponse,
        _model: &'a str,
    ) -> BoxFuture<'a, Result<Sent, AttemptError>> {
        Box::pin(async { Err(AttemptError::new(ErrorClass::Upstream, "not in this test")) })
    }
}

fn listed() -> Arc<dyn Upstream> {
    Arc::new(Listed {
        account: Account {
            id: "zai-credit".into(),
            basis: CostBasis::PrepaidCredit,
        },
        privacy: PrivacyTerms::zero_retention("test"),
        models: vec![ModelRow {
            id: "zai/glm-5.3-flash".into(),
            upstream_model: "glm-5.3-flash".into(),
            capabilities: Capabilities {
                tools: true,
                reasoning: true,
                reasoning_always_on: true,
                json_schema: true,
                images: false,
                context: 1_000_000,
                max_output: 128_000,
            },
            price: Price::micro(150_000, 30_000, 500_000),
            price_source: "test",
        }],
    })
}

fn sats() -> SatsRate {
    SatsRate {
        usd_per_btc: 100_000,
        as_of: "2026-10-09".into(),
    }
}

struct Deployment {
    token: String,
    address: String,
    _state: Arc<ServeState>,
    _dir: tempfile::TempDir,
}

async fn deploy(upstreams: Option<Vec<Arc<dyn Upstream>>>) -> Deployment {
    let dir = tempfile::tempdir().unwrap();
    let manifest = common::manifest(&common::artifact('a'), None);
    let registry = Registry::install(dir.path(), manifest).unwrap();
    let token = keys::issue(dir.path(), registry.manifest(), "acme")
        .unwrap()
        .token;
    let inference: Inference = serde_json::from_value(json!({
        "admin_token_env": "INFERENCE_RATES_TEST_ADMIN",
        "sats_rate": sats(),
    }))
    .unwrap();
    let config = Config {
        v: SCHEMA.to_string(),
        listen: "127.0.0.1:0".to_string(),
        registry: dir.path().to_path_buf(),
        require_workspace_membership: false,
        team_policy: None,
        team_reports: None,
        inference: Some(inference),
        accounts: None,
        billing: None,
        funding: None,
        earnings: None,
        commercial: None,
        skills: None,
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
        cors_origins: vec![],
        max_options: 4096,
        doors: Default::default(),
        job_retention_ms: 604_800_000,
        job_cursor_ttl_ms: 3_600_000,
        public_origin: None,
    };
    let state = ServeState::open_with_upstreams(config, upstreams).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(axum::serve(listener, serve::router(state.clone())).into_future());
    Deployment {
        token,
        address,
        _state: state,
        _dir: dir,
    }
}

/// With the gateway's own adapters and no rate rows in its config, the
/// card it serves is the published card, row for row: the card the
/// website draws when it cannot reach a gateway.
#[tokio::test]
async fn the_rate_card_is_public_and_is_the_published_card() {
    let d = deploy(None).await;
    let answer = reqwest::get(format!("{}/v1/rates", d.address))
        .await
        .unwrap();
    assert_eq!(answer.status(), StatusCode::OK);
    let card: Card = answer.json().await.unwrap();
    assert_eq!(card, Card::published(Some(&sats())));
    assert!(card.rows.len() >= 10, "{}", card.rows.len());
}

#[tokio::test]
async fn the_models_list_carries_each_providers_rate_rows() {
    let d = deploy(Some(vec![listed()])).await;
    let models: Value = reqwest::Client::new()
        .get(format!("{}/v1/models", d.address))
        .bearer_auth(&d.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(models["object"], "list");
    assert!(models["models"].is_array(), "the decision models stay");
    let glm = &models["data"][0];
    assert_eq!(glm["id"], "zai/glm-5.3-flash");
    assert_eq!(glm["object"], "model");
    let provider = &glm["openagents"]["providers"][0];
    assert_eq!(provider["provider"], "Z.ai");
    assert_eq!(provider["context"], 1_000_000);
    let row = &provider["prices"][0];
    assert_eq!(row["input"]["list_usd"], "0.15");
    assert_eq!(row["input"]["margin_usd"], "0.0075");
    assert_eq!(row["input"]["price_usd"], "0.1575");
    assert_eq!(row["input"]["price_sats"], 158);
    // The same row as /v1/rates.
    let card: Card = reqwest::get(format!("{}/v1/rates", d.address))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&card.rows[0]).unwrap(),
        provider["prices"][0]
    );
}
