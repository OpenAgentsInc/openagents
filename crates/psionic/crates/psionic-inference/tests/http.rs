#![cfg(feature = "http")]

use std::path::PathBuf;

use psionic_inference::{
    EngineKind,
    router::file_sha256,
    service::{MODEL, RouterService, questions},
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    requests: Vec<Case>,
}
#[derive(Deserialize)]
struct Case {
    request: String,
    context: Option<String>,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires pinned router artifacts and optional ORT library"]
async fn real_http_and_in_process_answers_match_for_each_engine() {
    let root =
        PathBuf::from(std::env::var_os("PSIONIC_ROUTER_MODEL_DIR").expect("model directory"));
    let library = std::env::var_os("PSIONIC_ORT_LIBRARY").map(PathBuf::from);
    let checksum = library.as_ref().map(|p| file_sha256(p).unwrap());
    let mut engines = vec![EngineKind::Native];
    if cfg!(feature = "ort") {
        engines.push(EngineKind::Ort);
    }
    for engine in engines {
        let mut direct =
            RouterService::load(&root, engine, 4, library.as_deref(), checksum.as_deref()).unwrap();
        let served =
            RouterService::load(&root, engine, 4, library.as_deref(), checksum.as_deref()).unwrap();
        let router = psionic_inference::http::router(served, 2).unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let client = reqwest::Client::new();
        assert!(
            client
                .get(format!("http://{address}/health"))
                .send()
                .await
                .unwrap()
                .status()
                .is_success()
        );
        let fixture: Fixture =
            serde_json::from_str(include_str!("../fixtures/s1-router-reference.json")).unwrap();
        for case in fixture.requests.into_iter().take(32) {
            let request = serde_json::json!({"model":MODEL,"state":{"request":case.request,"context":case.context},"questions":questions()});
            let direct_answer = direct.decide(request.clone()).unwrap();
            // Compare the JSON representation used on the wire. Parsing a
            // decimal float can round to the adjacent f64 in serde_json.
            let expected: serde_json::Value =
                serde_json::from_slice(&serde_json::to_vec(&direct_answer).unwrap()).unwrap();
            let actual: serde_json::Value = client
                .post(format!("http://{address}/v1/systemone"))
                .json(&request)
                .send()
                .await
                .unwrap()
                .error_for_status()
                .unwrap()
                .json()
                .await
                .unwrap();
            assert_eq!(actual, expected, "{engine:?}");
        }
        let bad = serde_json::json!({"state":"Hello","questions":{"category":{"type":"noul","instructions":"Different question"}}});
        assert_eq!(
            client
                .post(format!("http://{address}/v1/systemone"))
                .json(&bad)
                .send()
                .await
                .unwrap()
                .status(),
            422
        );
        server.abort();
        let _ = server.await;
    }
}
