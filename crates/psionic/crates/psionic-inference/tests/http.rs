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
    if device != DeviceChoice::Cpu && engine == EngineKind::Native {
        assert_eq!(served.identity().cpu_fallback, Some(false));
        assert_eq!(
            served.identity().provider,
            if device == DeviceChoice::Cuda {
                "psionic-cuda-encoder"
            } else {
                "psionic-metal-encoder"
            }
        );
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

#[cfg(all(feature = "native-metal", target_os = "macos"))]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires pinned router artifacts and an Apple GPU"]
async fn native_metal_http_matches_in_process_and_reports_no_cpu_fallback() {
    let root =
        PathBuf::from(std::env::var_os("PSIONIC_ROUTER_MODEL_DIR").expect("model directory"));
    check_engine(&root, EngineKind::Native, DeviceChoice::Metal, None, None).await;
}

#[cfg(all(feature = "ort", feature = "native-cuda"))]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires pinned assets, CUDA ORT in PSIONIC_HTTP_CUDA_ORT_LIBRARY, and PSIONIC_HTTP_BENCH_OUTPUT"]
async fn paired_cuda_http_latency_and_answers() {
    use std::io::Write;
    use std::time::Instant;
    let root =
        PathBuf::from(std::env::var_os("PSIONIC_ROUTER_MODEL_DIR").expect("model directory"));
    let library =
        PathBuf::from(std::env::var_os("PSIONIC_HTTP_CUDA_ORT_LIBRARY").expect("CUDA ORT library"));
    let checksum = file_sha256(&library).unwrap();
    let output = std::env::var_os("PSIONIC_HTTP_BENCH_OUTPUT").expect("scratch output file");
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .unwrap();
    let fixture: Fixture =
        serde_json::from_str(include_str!("../fixtures/s1-router-reference.json")).unwrap();
    let requests: Vec<_> = fixture.requests.into_iter().map(|case| serde_json::json!({"model":MODEL,"state":{"request":case.request,"context":case.context},"questions":questions()})).collect();
    assert_eq!(requests.len(), 100);
    let client = reqwest::Client::new();
    for repetition in 0..2 {
        for engine in if repetition == 0 {
            [EngineKind::Ort, EngineKind::Native]
        } else {
            [EngineKind::Native, EngineKind::Ort]
        } {
            let load = || {
                RouterService::load_on(
                    &root,
                    engine,
                    4,
                    Some(&library),
                    Some(&checksum),
                    DeviceChoice::Cuda,
                    engine == EngineKind::Ort,
                )
                .unwrap()
            };
            let mut direct = load();
            let served = load();
            let identity = served.model_entry();
            let router = psionic_inference::http::router(served, 2).unwrap();
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                axum::serve(listener, router).await.unwrap();
            });
            let url = format!("http://{address}/v1/systemone");
            let mut expected = Vec::new();
            for request in &requests {
                let answer = direct.decide(request.clone()).unwrap();
                let answer: serde_json::Value =
                    serde_json::from_slice(&serde_json::to_vec(&answer).unwrap()).unwrap();
                let actual: serde_json::Value = client
                    .post(&url)
                    .json(request)
                    .send()
                    .await
                    .unwrap()
                    .error_for_status()
                    .unwrap()
                    .json()
                    .await
                    .unwrap();
                assert_eq!(actual, answer);
                expected.push(answer);
            }
            drop(direct);
            let mut timings = Vec::with_capacity(1000);
            let wall = Instant::now();
            for i in 0..1000 {
                let start = Instant::now();
                let answer: serde_json::Value = client
                    .post(&url)
                    .json(&requests[i % 100])
                    .send()
                    .await
                    .unwrap()
                    .error_for_status()
                    .unwrap()
                    .json()
                    .await
                    .unwrap();
                timings.push(start.elapsed().as_secs_f64() * 1000.0);
                assert_eq!(answer, expected[i % 100]);
            }
            let requests_per_second = 1000.0 / wall.elapsed().as_secs_f64();
            let mut sorted = timings.clone();
            sorted.sort_by(f64::total_cmp);
            let binary = std::env::current_exe().unwrap();
            let mut record = serde_json::json!({"schema":"psionic.router.http.cuda.benchmark.v1","repetition":repetition,"samples":1000,"reference_cases":100,"http_answer_matches":1100,"concurrency":1,"execution":identity,"p50_ms":sorted[499],"p95_ms":sorted[949],"p99_ms":sorted[989],"requests_per_second":requests_per_second,"timings_ms":timings,"binary_sha256":file_sha256(&binary).unwrap(),"binary_bytes":binary.metadata().unwrap().len(),"source_commit":std::process::Command::new("git").args(["rev-parse","HEAD"]).output().ok().map(|r|String::from_utf8_lossy(&r.stdout).trim().to_string()),"rustc":std::process::Command::new("rustc").arg("--version").output().ok().map(|r|String::from_utf8_lossy(&r.stdout).trim().to_string()),"host":std::process::Command::new("uname").arg("-a").output().ok().map(|r|String::from_utf8_lossy(&r.stdout).trim().to_string()),"scope":"Production RouterService and Axum routes hosted by an integration test; serial reqwest client, JSON encoding and decoding included, loopback only. Warm model and shape caches. Separate processes provide model memory evidence in the tensor studies."});
            record["cpu_affinity"] = serde_json::json!(
                std::fs::read_to_string("/proc/self/status")
                    .ok()
                    .and_then(|status| status.lines().find_map(|line| line
                        .strip_prefix("Cpus_allowed_list:")
                        .map(|value| value.trim().to_string())))
            );
            record["model"] =
                serde_json::to_value(psionic_inference::router::pinned_manifest().unwrap())
                    .unwrap();
            record["native_weights_sha256"] =
                serde_json::json!(file_sha256(&root.join("native.safetensors")).unwrap());
            record["cuda_environment"] = serde_json::json!({"NVIDIA_TF32_OVERRIDE":std::env::var("NVIDIA_TF32_OVERRIDE").ok(),"CUDA_MODULE_LOADING":std::env::var("CUDA_MODULE_LOADING").ok(),"CUDA_MODULE_DATA_LOADING":std::env::var("CUDA_MODULE_DATA_LOADING").ok()});
            record["gpu"] = serde_json::json!(
                std::process::Command::new("nvidia-smi")
                    .args([
                        "--query-gpu=name,uuid,driver_version",
                        "--format=csv,noheader"
                    ])
                    .output()
                    .ok()
                    .map(|r| String::from_utf8_lossy(&r.stdout).trim().to_string())
            );
            serde_json::to_writer(&mut output, &record).unwrap();
            writeln!(output).unwrap();
            output.flush().unwrap();
            eprintln!(
                "{engine:?}: p50 {:.3} ms; p99 {:.3} ms; {requests_per_second:.1} requests/s",
                sorted[499], sorted[989]
            );
            server.abort();
            let _ = server.await;
        }
    }
}
