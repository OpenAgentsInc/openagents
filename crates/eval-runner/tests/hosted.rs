//! The hosted runner against a local relay, as deployed: it listens for
//! requests over NIP-42, releases a suite to the relay's Blossom server,
//! runs a trainer's signed request with the fake agent, streams progress,
//! seals the report, and on the trainer's publish request publishes a
//! `3189` that `nostr::eval_ext` accepts with the trainer credited, with no
//! request held by the relay. A second trainer's check through the same
//! runner is then a check of the first.
//!
//! Needs a disposable Postgres: set `NOSTR_RELAY_TEST_DATABASE_URL` and
//! `NOSTR_RELAY_TEST_ALLOW_DESTRUCTIVE=1`, as the relay's own suites do.

mod support;

use std::net::TcpListener;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use eval_runner::config::Limits;
use eval_runner::runner::Runner;
use eval_runner::wire::{Relay, Wire};
use futures_util::StreamExt as _;
use nostr::cj_conversation::{SubjectSource, SuiteSource};
use nostr::domain::Event;
use nostr::eval_ext::{self, EventPointer, hosted};
use serde_json::{Value, json};
use support::{FIXTURE, Phone, door, result_of};

struct LocalRelay {
    url: String,
    stop: nostr_relay::gateway::ShutdownHandle,
    _media: tempfile::TempDir,
}

async fn relay(database_url: String) -> LocalRelay {
    let media = tempfile::tempdir().unwrap();
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let url = format!("ws://127.0.0.1:{port}");
    let mut config = nostr_relay::gateway::GatewayConfig::new(
        database_url,
        format!("127.0.0.1:{port}").parse().unwrap(),
    );
    config.relay_url = Some(url.clone());
    config.db_connections = 4;
    config.shutdown_grace = Duration::from_secs(1);
    config.limits.events_per_minute_ip = 10_000;
    config.limits.events_per_minute_pubkey = 10_000;
    config.limits.media_per_minute_ip = 1_000;
    config.limits.media_per_minute_pubkey = 1_000;
    config.media = Some(nostr_relay::gateway::MediaConfig {
        root: media.path().to_path_buf(),
        cloud_base_url: None,
        max_blob_bytes: 10 * 1024 * 1024,
        max_bytes_per_pubkey: 1 << 30,
    });
    let gateway = nostr_relay::gateway::Gateway::start(config).await.unwrap();
    let stop = gateway.shutdown_handle();
    tokio::spawn(gateway.run());
    LocalRelay {
        url,
        stop,
        _media: media,
    }
}

/// A phone's live subscription to its answers: ephemeral events are only
/// ever delivered to a subscriber that is already listening.
async fn listen(url: &str, phone: &Phone) -> Arc<Mutex<Vec<Event>>> {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let identity =
        coder::relay::Identity::from_text(&support::hex(&phone.secret), "phone").unwrap();
    let mut socket = coder::relay::connect(url, &identity).await.unwrap();
    coder::relay::send(
        &mut socket,
        json!(["REQ", "answers", {"kinds": [26920, 27020], "#p": [phone.pubkey()]}]),
    )
    .await
    .unwrap();
    let into = Arc::clone(&seen);
    tokio::spawn(async move {
        while let Some(Ok(frame)) = socket.next().await {
            let tokio_tungstenite::tungstenite::Message::Text(text) = frame else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<Value>(&text) else {
                continue;
            };
            if value[0] == "EVENT"
                && let Ok(event) = serde_json::from_value::<Event>(value[2].clone())
            {
                into.lock().unwrap().push(event);
            }
        }
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    seen
}

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let path = entry.path();
        let target = to.join(entry.file_name());
        if path.is_dir() {
            if entry.file_name() != "results" {
                copy(&path, &target);
            }
        } else {
            std::fs::copy(&path, &target).unwrap();
        }
    }
}

async fn until<T>(within: Duration, mut find: impl FnMut() -> Option<T>) -> Option<T> {
    let started = std::time::Instant::now();
    while started.elapsed() < within {
        if let Some(found) = find() {
            return Some(found);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    None
}

/// Sends `request` as `phone` and waits for its result.
async fn ask(
    relay: &str,
    phone: &Phone,
    seen: &Arc<Mutex<Vec<Event>>>,
    runner: &str,
    input: &Value,
) -> (Event, Value, Value) {
    let (request, body) = phone.request(runner, input);
    let identity =
        coder::relay::Identity::from_text(&support::hex(&phone.secret), "phone").unwrap();
    Relay::new(relay, Arc::new(identity))
        .publish(request.clone())
        .await
        .unwrap();
    let result = until(Duration::from_secs(180), || {
        result_of(&phone.answers(runner, &request, &body, &seen.lock().unwrap()))
    })
    .await
    .expect("an answer");
    (request, body, result)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 6)]
async fn a_hosted_run_over_a_local_relay_credits_its_trainer() {
    let Ok(database_url) = std::env::var("NOSTR_RELAY_TEST_DATABASE_URL") else {
        eprintln!("skipped: set NOSTR_RELAY_TEST_DATABASE_URL or run scripts/test-postgres.sh");
        return;
    };
    if std::env::var("NOSTR_RELAY_TEST_ALLOW_DESTRUCTIVE").as_deref() != Ok("1") {
        eprintln!("skipped: set NOSTR_RELAY_TEST_ALLOW_DESTRUCTIVE=1");
        return;
    }
    let local = relay(database_url).await;
    let work = tempfile::tempdir().unwrap();
    let door = door();
    let key = support::secret();
    let identity = || coder::relay::Identity::from_text(&support::hex(&key), "runner").unwrap();
    let mut config = support::config(
        work.path(),
        &door,
        Limits {
            runs_per_trainer: None,
            turns_per_day: None,
            jobs: 2,
            concurrency: 2,
        },
    );
    config.relay = local.url.clone();
    // The catalog tool, published by the runner: a copy of the fixture
    // whose package names the runner as its publisher.
    let tool_dir = work.path().join("tool");
    copy(Path::new(FIXTURE), &tool_dir);
    let record = tool_dir.join("package.json");
    let mut package: Value = serde_json::from_slice(&std::fs::read(&record).unwrap()).unwrap();
    package["publisher"] = json!(identity().pubkey());
    std::fs::write(&record, serde_json::to_vec_pretty(&package).unwrap()).unwrap();
    config.catalog = vec![tool_dir.clone()];
    let wire: Arc<dyn Wire> = Arc::new(Relay::new(&local.url, Arc::new(identity())));
    let blobs = Arc::new(eval_runner::wire::Blossom::new(None, &local.url).unwrap());
    let runner = Runner::new(config, identity(), wire.clone(), blobs).unwrap();
    let runner_key = runner.pubkey().to_string();
    {
        let (runner, url) = (Arc::clone(&runner), local.url.clone());
        tokio::spawn(async move {
            let identity =
                Arc::new(coder::relay::Identity::from_text(&support::hex(&key), "runner").unwrap());
            loop {
                let _ = eval_runner::runner::listen(
                    &url,
                    &identity,
                    &runner,
                    coder::relay::liveness::Liveness::default(),
                )
                .await;
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        });
    }
    tokio::time::sleep(Duration::from_millis(500)).await;

    // The starter-style release: the suite's files on Blossom, the 3184
    // signed by the runner.
    let release = runner
        .release_extension_suite(Path::new(FIXTURE), "repo-map-brief-tests")
        .await
        .unwrap();
    let pointer = EventPointer {
        id: release["id"].as_str().unwrap().into(),
        pubkey: runner_key.clone(),
        kind: nostr::kinds::EXT_RELEASE,
    };
    let tool = runner.catalog().tools[0].definition.clone();
    let input = |check: Option<&str>| {
        hosted::run_input(
            &SuiteSource::Published(pointer.clone()),
            &SubjectSource::Definition(Box::new(tool.clone())),
            None,
            2,
            check,
        )
        .unwrap()
    };

    let dana = Phone::new();
    let dana_seen = listen(&local.url, &dana).await;
    let (request, body, result) =
        ask(&local.url, &dana, &dana_seen, &runner_key, &input(None)).await;
    assert_eq!(result["outcome"], "completed", "{result:#}");
    let output = hosted::parse_run_output(&result["output"]).unwrap();
    let tool_releases = runner.release_tools().await.unwrap();
    assert_eq!(
        tool_releases.len(),
        1,
        "the runner released its catalog tool"
    );
    let progress = dana
        .answers(&runner_key, &request, &body, &dana_seen.lock().unwrap())
        .into_iter()
        .filter(|(_, p)| p["type"] == "progress")
        .count();
    assert!(progress >= 8, "{progress} progress notes");
    // The sealed report is on the relay, readable by the trainer.
    let dana_wire = Relay::new(
        &local.url,
        Arc::new(coder::relay::Identity::from_text(&support::hex(&dana.secret), "dana").unwrap()),
    );
    let sealed = dana_wire
        .query(json!({"ids": [output.sealed.id], "kinds": [3188]}))
        .await
        .unwrap();
    let opened = nostr::private_artifact::open(&sealed[0], &dana.secret).unwrap();
    assert_eq!(opened.artifact().digest, output.report.digest);

    let (_, _, published) = ask(
        &local.url,
        &dana,
        &dana_seen,
        &runner_key,
        &hosted::publish_input(&output.report),
    )
    .await;
    assert_eq!(published["outcome"], "completed", "{published:#}");
    let original = hosted::parse_publish_output(&published["output"])
        .unwrap()
        .result;
    let stored = wire
        .query(json!({"kinds": [3189], "#t": [eval_ext::PROFILE_MARKER]}))
        .await
        .unwrap();
    let original_event = stored
        .iter()
        .find(|e| e.id == original.id)
        .expect("the 3189 is stored");
    let publication = eval_ext::parse_publication(original_event).unwrap();
    // Relays keep no 25920: the credit rests on the request inline.
    assert!(
        wire.query(json!({"kinds": [25920]}))
            .await
            .unwrap()
            .is_empty(),
        "the relay holds no requests"
    );
    assert_eq!(
        eval_ext::verified_trainer(&publication, &[]).unwrap(),
        dana.pubkey()
    );
    // The subject is named by its release, which is on the relay.
    let subject_release = publication
        .subject_release
        .clone()
        .expect("the tool's release");
    assert_eq!(
        subject_release.id,
        tool_releases[0].1["id"].as_str().unwrap()
    );
    assert_eq!(
        wire.query(json!({"ids": [subject_release.id]}))
            .await
            .unwrap()
            .len(),
        1
    );

    // Erin checks Dana's result through the same runner.
    let erin = Phone::new();
    let erin_seen = listen(&local.url, &erin).await;
    let (_, _, checked) = ask(
        &local.url,
        &erin,
        &erin_seen,
        &runner_key,
        &input(Some(&original.id)),
    )
    .await;
    assert_eq!(checked["outcome"], "completed", "{checked:#}");
    let check_report = hosted::parse_run_output(&checked["output"]).unwrap().report;
    let (_, _, check_published) = ask(
        &local.url,
        &erin,
        &erin_seen,
        &runner_key,
        &hosted::publish_input(&check_report),
    )
    .await;
    let check_id = hosted::parse_publish_output(&check_published["output"])
        .unwrap()
        .result
        .id;
    let stored = wire
        .query(json!({"kinds": [3189], "#t": [eval_ext::PROFILE_MARKER]}))
        .await
        .unwrap();
    let check_event = stored.iter().find(|e| e.id == check_id).unwrap();
    let check = eval_ext::parse_publication(check_event).unwrap();
    assert_eq!(
        eval_ext::verified_trainer(&check, &[]).unwrap(),
        erin.pubkey()
    );
    assert_eq!(check.checks.as_deref(), Some(original.id.as_str()));
    assert_ne!(
        eval_ext::linkage(&publication, &check),
        eval_ext::Linkage::NotACheck
    );
    // The phone's ledger reads both results and credits the trainers with
    // no request from the relay.
    let results = xp_ledger::eval::publications(&stored);
    assert!(results.contains_key(&original.id) && results.contains_key(&check_id));
    local.stop.shutdown();
}
