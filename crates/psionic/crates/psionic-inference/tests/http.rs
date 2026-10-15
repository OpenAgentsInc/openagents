#![cfg(feature = "http")]

use std::path::PathBuf;

use psionic_inference::{
    DeviceChoice, EngineKind,
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
        check_engine(
            &root,
            engine,
            DeviceChoice::Cpu,
            library.as_deref(),
            checksum.as_deref(),
        )
        .await;
    }
}

#[cfg(feature = "native-cuda")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires pinned router artifacts and CUDA"]
async fn native_cuda_http_matches_in_process_and_reports_no_cpu_fallback() {
    let root =
        PathBuf::from(std::env::var_os("PSIONIC_ROUTER_MODEL_DIR").expect("model directory"));
    check_engine(&root, EngineKind::Native, DeviceChoice::Cuda, None, None).await;
}

async fn check_engine(
    root: &std::path::Path,
    engine: EngineKind,
    device: DeviceChoice,
    library: Option<&std::path::Path>,
    checksum: Option<&str>,
) {
    let mut direct =
        RouterService::load_on(root, engine, 4, library, checksum, device, false).unwrap();
    let served = RouterService::load_on(root, engine, 4, library, checksum, device, false).unwrap();
    let expected_entry = served.model_entry();
    if device == DeviceChoice::Cuda && engine == EngineKind::Native {
        assert_eq!(served.identity().cpu_fallback, Some(false));
        assert_eq!(served.identity().provider, "psionic-cuda-encoder");
    }
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
    let models: serde_json::Value = client
        .get(format!("http://{address}/v1/models"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(models["data"][0], expected_entry);
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
