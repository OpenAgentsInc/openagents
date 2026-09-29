//! A trainer's side of a hosted run, for smoke tests and the live
//! measurement: signs requests the way the phone does
//! (`nostr::eval_ext::hosted`), sends them to the hosted runner, and
//! prints every answer.
//!
//! ```text
//! trainer run SUITE_RELEASE_ID EXTENSION_DIR [--runs N] [--check RESULT_ID]
//! trainer publish REPORT_JSON     (the report ArtifactRef a run printed)
//! ```
//!
//! `TRAINER_KEY_FILE` holds the trainer's key (64 hex; made when missing),
//! `EVAL_RUNNER_RELAY` the relay (default production), and
//! `EVAL_RUNNER_PUBKEY` the runner (default `hosted::RUNNER`). It prints
//! ids, counts, and outcomes, never a key.

use std::str::FromStr as _;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::StreamExt as _;
use nostr::cj_conversation::{SubjectSource, SuiteSource};
use nostr::domain::Event;
use nostr::eval_ext::{EventPointer, hosted};
use nostr::execution::{self, Pending, Seal};
use secp256k1::XOnlyPublicKey;
use serde_json::{Value, json};

fn key() -> coder::relay::Identity {
    let path = std::env::var("TRAINER_KEY_FILE").expect("TRAINER_KEY_FILE");
    if !std::path::Path::new(&path).exists() {
        let secret = secp256k1::SecretKey::new(&mut secp256k1::rand::rng());
        std::fs::write(&path, secret.display_secret().to_string()).unwrap();
    }
    coder::relay::Identity::from_text(&std::fs::read_to_string(&path).unwrap(), "trainer").unwrap()
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let relay = std::env::var("EVAL_RUNNER_RELAY").unwrap_or_else(|_| hosted::RELAY.into());
    let runner = std::env::var("EVAL_RUNNER_PUBKEY").unwrap_or_else(|_| hosted::RUNNER.into());
    let me = key();
    let flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|at| args.get(at + 1))
            .cloned()
    };
    let input = match args.first().map(String::as_str) {
        Some("run") => {
            let release = args.get(1).expect("SUITE_RELEASE_ID");
            let tool = eval_runner::catalog::resolve(std::path::Path::new(
                args.get(2).expect("EXTENSION_DIR"),
            ))
            .unwrap();
            let runs = flag("--runs").map_or(3, |r| r.parse().unwrap());
            hosted::run_input(
                &SuiteSource::Published(EventPointer {
                    id: release.clone(),
                    pubkey: std::env::var("SUITE_AUTHOR").unwrap_or_else(|_| runner.clone()),
                    kind: nostr::kinds::EXT_RELEASE,
                }),
                &SubjectSource::Definition(Box::new(tool.definition)),
                None,
                runs,
                flag("--check").as_deref(),
            )
            .unwrap()
        }
        Some("publish") => {
            let report: Value = serde_json::from_str(args.get(1).expect("REPORT_JSON")).unwrap();
            hosted::publish_input(&nostr::contracts::parse_artifact(&report).unwrap())
        }
        _ => {
            eprintln!(
                "usage: trainer run SUITE EXTENSION [--runs N] [--check ID] | publish REPORT"
            );
            std::process::exit(64);
        }
    };
    println!("trainer {}", me.pubkey());
    let body = hosted::request_body(
        &runner,
        &format!("smoke-{}", eval_runner::unix_now()),
        &input,
        eval_runner::unix_now(),
    )
    .unwrap();
    let peer = XOnlyPublicKey::from_str(&runner).unwrap();
    let conversation = nostr::nip44::conversation_key(me.secret(), &peer);
    let request = Seal {
        signer: me.signer(),
        conversation,
        nonce: secp256k1::rand::random(),
        created_at: eval_runner::unix_now(),
    }
    .event(
        execution::REQUEST_KIND,
        hosted::request_tags(&runner, body["deadline"].as_u64().unwrap()),
        &body,
    )
    .unwrap();

    // Listen before sending: the answers are ephemeral.
    let mut socket = coder::relay::connect(&relay, &me).await.unwrap();
    coder::relay::send(
        &mut socket,
        json!(["REQ", "answers", {"kinds": [26920, 27020], "#p": [me.pubkey()], "#e": [request.id]}]),
    )
    .await
    .unwrap();
    let identity = Arc::new(key());
    use eval_runner::wire::Wire as _;
    eval_runner::wire::Relay::new(&relay, identity)
        .publish(request.clone())
        .await
        .unwrap();
    println!("request {}", request.id);
    let pending = Pending {
        execute_event: &request.id,
        worker: &runner,
        customer: me.pubkey(),
        request: body["request"].as_str().unwrap(),
        attempt: 1,
    };
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(3_600) {
        let Ok(Some(Ok(frame))) =
            tokio::time::timeout(Duration::from_secs(30), socket.next()).await
        else {
            continue;
        };
        let tokio_tungstenite::tungstenite::Message::Text(text) = frame else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        if value[0] != "EVENT" {
            continue;
        }
        let Ok(event) = serde_json::from_value::<Event>(value[2].clone()) else {
            continue;
        };
        let Ok(payload) = execution::bind_worker_event(&event, &pending, me.secret()) else {
            continue;
        };
        match payload["type"].as_str() {
            Some("accepted") => println!("accepted after {:.1} s", started.elapsed().as_secs_f64()),
            Some("progress") => {
                if let Ok(progress) = hosted::parse_progress(&payload) {
                    println!(
                        "{} {} of {} at {:.0} s",
                        payload["status"].as_str().unwrap_or_default(),
                        progress.completed,
                        progress.planned,
                        started.elapsed().as_secs_f64()
                    );
                }
            }
            Some("result") => {
                println!(
                    "result {} after {:.1} s",
                    payload["outcome"].as_str().unwrap_or_default(),
                    started.elapsed().as_secs_f64()
                );
                if let Some(code) = payload["code"].as_str() {
                    println!(
                        "code {code}: {}",
                        payload["message"].as_str().unwrap_or_default()
                    );
                }
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload["output"]).unwrap()
                );
                return;
            }
            _ => {}
        }
    }
    eprintln!("no result within an hour");
    std::process::exit(1);
}
