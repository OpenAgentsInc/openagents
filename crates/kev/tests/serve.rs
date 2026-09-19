//! The HTTP surface end-to-end: a live `kev-serve` router driven by the `jev`
//! client, unmodified, for all three question types.
//!
//! Needs the artifact bundle and base checkpoint (see `conformance.rs`);
//! builds only under `--features serve`.

#![cfg(feature = "serve")]

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use candle_core::Device;
use kev::decision::DecisionModel;
use kev::lora::LoraConfig;
use kev::serve::{ServeState, router};

fn dir(env: &str, fallback: &str) -> Option<PathBuf> {
    if let Ok(dir) = std::env::var(env) {
        let dir = PathBuf::from(dir);
        return dir.exists().then_some(dir);
    }
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(fallback);
    dir.exists().then_some(dir)
}

fn state() -> Option<&'static Arc<ServeState>> {
    static STATE: OnceLock<Option<Arc<ServeState>>> = OnceLock::new();
    STATE
        .get_or_init(|| {
            let adapter = dir("KEV_ARTIFACT_DIR", "../../../kev-artifacts/kev-0.5b")?;
            let base = dir("KEV_BASE_DIR", "../../../kev-artifacts/qwen2.5-0.5b")?;
            let lora: LoraConfig = serde_json::from_str(
                &std::fs::read_to_string(adapter.join("adapter_config.json")).ok()?,
            )
            .ok()?;
            let model = DecisionModel::load(&base, &adapter, Device::Cpu).ok()?;
            Some(Arc::new(ServeState {
                model,
                model_id: "kev-latest".to_string(),
                aliases: vec!["jev-latest".to_string()],
                run: adapter.display().to_string(),
                base: "Qwen/Qwen2.5-0.5B".to_string(),
                lora: lora.r,
                device: "cpu".to_string(),
            }))
        })
        .as_ref()
}

/// Bind the router on an ephemeral port; returns its base URL.
async fn serve() -> Option<String> {
    let state = state()?.clone();
    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    tokio::spawn(async move {
        axum::serve(listener, router(state)).await.expect("serve");
    });
    Some(format!("http://{addr}"))
}

/// Run the blocking jev client off the test's runtime thread.
fn blocking<F, T>(f: F) -> T
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    std::thread::spawn(f).join().expect("client thread")
}

fn client(url: &str) -> jev::BlockingClient {
    jev::BlockingClient::new(
        jev::Config::new()
            .api_key("kev-test")
            .base_url(url)
            .default_model("kev-latest"),
    )
    .expect("client")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn jev_client_round_trips_all_types() {
    let Some(url) = serve().await else {
        eprintln!("skipping: no artifact bundle");
        return;
    };
    blocking(move || {
        use indexmap::IndexMap;
        use jev::{Choice, Noul, Questions, Score};

        let mut criteria = IndexMap::new();
        criteria.insert("returns".to_string(), None);
        criteria.insert("shipping".to_string(), None);
        criteria.insert("billing".to_string(), None);
        let questions = Questions::new()
            .with("department", Choice::new("Which team?", criteria))
            .with("escalate", Noul::new("Urgent human attention?"))
            .with(
                "frustration",
                Score::new(
                    "How frustrated?",
                    vec![None, None, None],
                ),
            );
        let request = jev::SystemOneRequest::new(
            "Shoes arrived late and I see two charges on my card.",
            questions,
        )
        .model("kev-latest");
        let response = client(&url).system_one(request).expect("system_one");

        assert_eq!(response.model, "kev-latest");
        assert_eq!(response.answers.len(), 3);
        let department = response.choice("department").expect("choice answer");
        assert!(
            ["returns", "shipping", "billing"].contains(&department.choice.as_str()),
            "unexpected choice {}",
            department.choice
        );
        let total: f64 = department.probabilities.values().sum();
        assert!((total - 1.0).abs() < 0.02, "probabilities sum {total}");
        let noul = response.noul("escalate").expect("noul answer");
        assert!((0.0..=1.0).contains(&noul.noul));
        let score = response.score("frustration").expect("score answer");
        assert!((0.0..=2.0).contains(&score.score));
        assert_eq!(score.legend.len(), 3);
        assert!(response.usage.input_tokens.unwrap_or(0) > 0);
    });
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn models_lists_kev() {
    let Some(url) = serve().await else {
        eprintln!("skipping: no artifact bundle");
        return;
    };
    blocking(move || {
        let cards = client(&url)
            .list_models(jev::ListOptions::new())
            .expect("list_models");
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].name, "kev-latest");
        assert!(!cards[0].description.is_empty());
    });
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn refusals_are_typed() {
    let Some(url) = serve().await else {
        eprintln!("skipping: no artifact bundle");
        return;
    };
    // Empty questions: validation refusal, `{"detail": …}`, 422.
    let http = reqwest::Client::new();
    let response = http
        .post(format!("{url}/v1/systemone"))
        .json(&serde_json::json!({"state": "x", "questions": {}}))
        .send()
        .await
        .expect("send");
    assert_eq!(response.status(), 422);
    let body: serde_json::Value = response.json().await.expect("json");
    assert_eq!(
        body["detail"],
        "questions must hold at least one question"
    );

    // Malformed body: same refusal shape, not an HTML error page.
    let response = http
        .post(format!("{url}/v1/systemone"))
        .header("content-type", "application/json")
        .body("{not json")
        .send()
        .await
        .expect("send");
    assert_eq!(response.status(), 422);
    let body: serde_json::Value = response.json().await.expect("json");
    assert!(body["detail"].as_str().unwrap().starts_with("request body:"));
}
