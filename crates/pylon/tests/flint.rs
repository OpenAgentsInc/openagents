#![allow(clippy::unwrap_used)]
//! Native Flint's strict HTTP contract through a real encrypted Pylon job.
use std::time::Duration;

use nostr::pylon::{Status, parse_beacon};
use pylon::client::{self, Ask, Within};
use pylon::flint;
use pylon::group::Want;
use pylon::identity::Identity;
use pylon::provider::{Config, Provider};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

fn model_info() -> Value {
    json!({"data":[{"id":"RecursiveSI/flint"}],"psionic":{
        "revision":flint::REVISION,"artifact":flint::ARTIFACT,
        "numerical_profile":flint::PROFILE,"max_context_tokens":flint::CONTEXT,"busy":false,"backend":"cuda"}})
}

async fn http() -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let serving = tokio::spawn(async move {
        loop {
            let (socket, _) = listener.accept().await.unwrap();
            tokio::spawn(async move {
                let mut reader = BufReader::new(socket);
                let mut line = String::new();
                reader.read_line(&mut line).await.unwrap();
                let completion = line.starts_with("POST /v1/chat/completions ");
                assert!(completion || line.starts_with("GET /v1/models "));
                let mut length = 0;
                loop {
                    line.clear();
                    reader.read_line(&mut line).await.unwrap();
                    if line == "\r\n" {
                        break;
                    }
                    if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
                        length = value.trim().parse::<usize>().unwrap();
                    }
                }
                let mut bytes = vec![0; length];
                reader.read_exact(&mut bytes).await.unwrap();
                let body = if completion {
                    let request: Value = serde_json::from_slice(&bytes).unwrap();
                    for key in request.as_object().unwrap().keys() {
                        assert!(["model","messages","max_tokens","stream","temperature","top_p","top_k","seed"].contains(&key.as_str()), "{key}");
                    }
                    assert_eq!(request["model"], "RecursiveSI/flint");
                    assert_eq!(request["max_tokens"], flint::OUTPUT);
                    assert_eq!(request["top_k"], 20);
                    assert_eq!(request["temperature"], 1.0);
                    assert_eq!(request["top_p"], 0.95);
                    assert_eq!(request["seed"], 42);
                    json!({"choices":[{"finish_reason":"stop","message":{"content":"4","reasoning_content":"private thought"}}],"usage":{"prompt_tokens":17,"completion_tokens":40}})
                } else { model_info() }.to_string();
                reader.get_mut().write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
            });
        }
    });
    (url, serving)
}

#[tokio::test]
async fn native_flint_answers_an_encrypted_job_with_its_declared_limits() {
    let (url, serving) = http().await;
    let engine = flint::Native::connect(&url).unwrap();
    assert!(engine.healthy().await);
    let (relay, hub) = pylon::fixture::relay().await.unwrap();
    let home = tempfile::tempdir().unwrap();
    let mut config = Config::new(&relay, "flint-cuda", home.path().into());
    flint::configure(&mut config);
    config.allow = None;
    let key = Identity::generate();
    let provider = Provider::new(config, key.clone(), engine).unwrap();
    let beacon = provider.beacon(None).await;
    assert!(beacon.group.is_none());
    assert_eq!(beacon.slots.total, 1);
    assert_eq!(beacon.services[0].model, flint::advertised());
    assert_eq!(flint::deadline(&beacon), Some(900));
    assert_eq!(pylon::job::input_limit(&beacon), flint::INPUT_BYTES);
    assert_eq!(
        beacon.meta.as_ref().unwrap()["request_limits"]["max_input_tokens"],
        2048
    );
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(provider.run(async {
        let _ = stopped.await;
    }));
    for _ in 0..200 {
        if hub
            .lock()
            .await
            .stored
            .iter()
            .any(|event| parse_beacon(event).is_ok_and(|b| b.status == Status::Online))
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let buyer_home = tempfile::tempdir().unwrap();
    let ask = Ask {
        relay,
        pylon: Some(key.pubkey().into()),
        prompt: "What is 2+2?".into(),
        wait: Duration::from_secs(10),
        publish_receipt: true,
        home: buyer_home.path().into(),
        checkers: Default::default(),
        pay: None,
    };
    let answer = client::ask_model_within(
        &Identity::generate(),
        &ask,
        &Want::Model(flint::MODEL.into()),
        &Within::new(Duration::from_secs(1000)),
        &[],
        None,
    )
    .await
    .unwrap();
    assert_eq!(answer.text.as_deref(), Some("4"));
    assert_eq!(answer.model, flint::advertised());
    let declared = answer.declared.unwrap();
    assert_eq!(declared.ceiling_secs, Some(900));
    assert_eq!(declared.deadline_secs, 900);
    assert_eq!(answer.usage.unwrap()["output"], 40);
    let _ = stop.send(());
    serving.abort();
}

#[test]
fn release_manifest_still_binds_the_profile() {
    use sha2::{Digest, Sha256};
    let bytes = include_bytes!("../../psionic/crates/psionic-flint/testdata/artifact.json");
    assert_eq!(
        flint::ARTIFACT,
        format!("sha256:{:x}", Sha256::digest(bytes))
    );
    let value: Value = serde_json::from_slice(bytes).unwrap();
    assert_eq!(value["revision"], flint::REVISION);
}
