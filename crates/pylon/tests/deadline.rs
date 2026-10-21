#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The declared-deadline contract (`pylon.deadline.v1`, W14, #12401) end to
//! end on the in-process relay: real providers and model-group heads on
//! fake engines, a recorded old provider, and W10's synthetic stages. The
//! buyer's budget and the beacon's ceiling meet at the smaller; heartbeats
//! never extend it; each side refuses what it can't serve at once instead
//! of timing out. Nothing reads the real home or reaches a network.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use nostr::pylon::deadline;
use nostr::pylon::{Layers, Readiness, Status, beacon_event, parse_beacon};
use pylon::client::{self, Answer, Ask, Within};
use pylon::engine::{Echo, Engine, Finish, Generation, Pending, Sampling, Turn};
use pylon::group::{GroupSpec, GroupStatus, SharedReadiness, Want};
use pylon::group_serve::GroupEngine;
use pylon::identity::Identity;
use pylon::job;
use pylon::provider::{Config, HeadGroup, Provider};
use pylon::relay::{self, Frame, LIFETIME};
use pylon::router::{self, Decide, Deciding, Outcome, Question, Settings};
use pylon::stage_group::{PROTOCOL, StageGroup, StageGroupConfig, StageSpec};
use serde_json::{Value, json};
use tokio::sync::oneshot;

const MODEL: &str = "deepseek-v4.1-flash";
const ARTIFACT: &str = "sha256:4c1f1a7e1b7d4a8c0e9f2b3d5a6c7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d";

/// An engine that answers after `delay`, and records whether a job was
/// dropped before it finished.
struct Slow {
    delay: Duration,
    finish: Finish,
    dropped: Arc<AtomicBool>,
}

impl Slow {
    fn new(delay: Duration) -> Arc<Self> {
        Arc::new(Self {
            delay,
            finish: Finish::Stop,
            dropped: Arc::new(AtomicBool::new(false)),
        })
    }
}

struct Dropped(Arc<AtomicBool>, bool);

impl Drop for Dropped {
    fn drop(&mut self) {
        if !self.1 {
            self.0.store(true, Ordering::SeqCst);
        }
    }
}

impl Engine for Slow {
    fn model(&self) -> &str {
        "slow"
    }

    fn healthy(&self) -> Pending<'_, bool> {
        Box::pin(async { true })
    }

    fn generate<'a>(
        &'a self,
        _turns: &'a [Turn],
        _max_tokens: u32,
    ) -> Pending<'a, Result<Generation, String>> {
        Box::pin(async move {
            let mut guard = Dropped(Arc::clone(&self.dropped), false);
            tokio::time::sleep(self.delay).await;
            guard.1 = true;
            Ok(Generation {
                text: "4".into(),
                input_tokens: Some(8),
                output_tokens: Some(1),
                model: "slow".into(),
                finish: self.finish,
            })
        })
    }
}

/// One running pylon.
struct Running {
    key: Identity,
    _home: tempfile::TempDir,
    _stop: oneshot::Sender<()>,
}

fn config(relay: &str, slug: &str, home: &std::path::Path) -> Config {
    let mut config = Config::new(relay, slug, home.to_path_buf());
    config.allow = None;
    config.rate_per_minute = 600;
    config.heartbeat = Duration::from_millis(250);
    config
}

fn spec(ceiling_secs: u64) -> GroupSpec {
    GroupSpec {
        id: "v41-a".into(),
        model: MODEL.into(),
        artifact: ARTIFACT.into(),
        deadline_secs: ceiling_secs,
        max_output_tokens: 4_096,
    }
}

/// Run `config` on `engine` and wait for its beacon.
async fn run(
    hub: &Arc<tokio::sync::Mutex<pylon::fixture::Hub>>,
    config: Config,
    home: tempfile::TempDir,
    engine: Arc<dyn Engine>,
) -> Running {
    let key = Identity::generate();
    let provider = Provider::new(config, key.clone(), engine).unwrap();
    let (stop, stopped) = oneshot::channel::<()>();
    tokio::spawn(provider.run(async {
        let _ = stopped.await;
    }));
    // Wait for a beacon that takes a job: online, a free slot, and, for a
    // group head, ready.
    for _ in 0..800 {
        let up = hub.lock().await.stored.iter().any(|e| {
            e.kind == 30_200
                && e.pubkey == key.pubkey()
                && parse_beacon(e).is_ok_and(|b| {
                    b.status == Status::Online
                        && b.slots.free > 0
                        && b.group_head()
                            .is_none_or(|g| g.readiness == Some(Readiness::Ready))
                })
        });
        if up {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    Running {
        key,
        _home: home,
        _stop: stop,
    }
}

/// A group head whose beacon declares `ceiling_secs`, running jobs on
/// `engine`, as group mode sets it up (`job_timeout` is the ceiling).
async fn head(
    relay: &str,
    hub: &Arc<tokio::sync::Mutex<pylon::fixture::Hub>>,
    ceiling_secs: u64,
    engine: Arc<dyn Engine>,
) -> Running {
    let home = tempfile::tempdir().unwrap();
    let mut config = config(relay, "v41-head", home.path());
    config.slots = 1;
    config.job_timeout = Duration::from_secs(ceiling_secs);
    let readiness = SharedReadiness::default();
    readiness.set(GroupStatus {
        readiness: Readiness::Ready,
        generation: 1,
    });
    config.group = Some(HeadGroup {
        spec: spec(ceiling_secs),
        readiness: Arc::new(readiness),
        stages: vec![("s0".into(), Layers { from: 0, to: 43 })],
    });
    run(hub, config, home, engine).await
}

/// A complete pylon: the ordinary 90-second ceiling.
async fn complete(
    relay: &str,
    hub: &Arc<tokio::sync::Mutex<pylon::fixture::Hub>>,
    engine: Arc<dyn Engine>,
) -> Running {
    let home = tempfile::tempdir().unwrap();
    let config = config(relay, "rig", home.path());
    run(hub, config, home, engine).await
}

fn ask(relay: &str, home: &std::path::Path, pylon: Option<&Identity>) -> Ask {
    Ask {
        relay: relay.into(),
        pylon: pylon.map(|p| p.pubkey().to_string()),
        prompt: "What is 2 + 2?".into(),
        wait: Duration::from_secs(10),
        publish_receipt: false,
        home: home.to_path_buf(),
        checkers: BTreeSet::new(),
        pay: None,
    }
}

fn want() -> Want {
    Want::Artifact(ARTIFACT.into())
}

/// A request that names a model never takes a decision.
struct NoDecision;

impl Decide for NoDecision {
    fn decide<'a>(&'a self, _question: &'a Question) -> Deciding<'a> {
        Box::pin(async { Err("no decision".to_string()) })
    }
}

fn turns() -> Vec<Turn> {
    vec![Turn {
        role: "user".into(),
        content: "What is 2 + 2?".into(),
    }]
}

fn routing(relay: &str, home: &std::path::Path, budget: Option<u64>) -> Settings {
    let mut settings = Settings::new(relay, home.to_path_buf());
    settings.want = Some(want());
    settings.try_wait = Duration::from_secs(10);
    settings.deadline = budget.map(Duration::from_secs);
    settings
}

fn answered(routed: &router::Routed) -> &Answer {
    match &routed.outcome {
        Outcome::Answered(answer) => answer,
        other => panic!("no pylon answered: {other:?} after {:?}", routed.failed),
    }
}

/// The router sends a V4.1 request to a group whose deadline passes 90
/// seconds under the extension, and the answer comes back within the
/// buyer's budget, with the heartbeats it got on the way.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_long_group_answers_an_extended_job_within_the_budget() {
    let (relay, hub) = pylon::fixture::relay().await.unwrap();
    let slow = Slow::new(Duration::from_millis(1_500));
    let group = head(&relay, &hub, 600, slow).await;
    let home = tempfile::tempdir().unwrap();
    let routed = router::route(
        &Identity::generate(),
        &routing(&relay, home.path(), Some(20)),
        &NoDecision,
        &turns(),
        None,
    )
    .await;
    let answer = answered(&routed);
    assert!(answer.pylon.contains(group.key.pubkey()));
    assert_eq!(answer.text.as_deref(), Some("4"));
    let declared = answer.declared.as_ref().unwrap();
    assert_eq!(declared.budget_secs, 20);
    assert_eq!(declared.ceiling_secs, Some(600));
    assert_eq!(declared.deadline_secs, 20);
    assert_eq!(declared.admitted_secs, Some(20));
    assert_eq!(declared.finish.as_deref(), Some("stop"));
    assert!(declared.heartbeats >= 3, "{declared:?}");
    assert!(answer.answer_ms.unwrap() < 20_000);
}

/// Heartbeats keep coming while the job runs, and the job still ends at
/// the buyer's budget: the provider refuses it with `limit_exceeded` and
/// drops the work, and the buyer hears so instead of waiting on.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn heartbeats_never_extend_the_deadline() {
    let (relay, hub) = pylon::fixture::relay().await.unwrap();
    let slow = Slow::new(Duration::from_secs(60));
    let dropped = Arc::clone(&slow.dropped);
    let group = head(&relay, &hub, 600, slow).await;
    let home = tempfile::tempdir().unwrap();
    let started = Instant::now();
    let answer = client::ask_model_within(
        &Identity::generate(),
        &ask(&relay, home.path(), Some(&group.key)),
        &want(),
        &Within::new(Duration::from_secs(2)),
        &[],
        None,
    )
    .await
    .unwrap();
    let took = started.elapsed();
    assert_eq!(answer.text, None);
    assert_eq!(
        answer.error.as_deref(),
        Some("limit_exceeded: the job ran out of time")
    );
    let declared = answer.declared.unwrap();
    assert_eq!(declared.admitted_secs, Some(2));
    assert!(declared.heartbeats >= 4, "{declared:?}");
    assert!(took >= Duration::from_secs(2), "{took:?}");
    assert!(took < Duration::from_secs(2) + client::GRACE, "{took:?}");
    assert!(dropped.load(Ordering::SeqCst), "the work kept running");

    // Through the router, the miss is a failed try, never another model.
    let routed = router::route(
        &Identity::generate(),
        &routing(&relay, home.path(), Some(2)),
        &NoDecision,
        &turns(),
        None,
    )
    .await;
    assert!(matches!(routed.outcome, Outcome::Unavailable(_)));
    assert!(routed.failed[0].1.contains("limit_exceeded"), "{routed:?}");
}

/// The beacon's ceiling caps a larger budget.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_ceiling_caps_a_larger_budget() {
    let (relay, hub) = pylon::fixture::relay().await.unwrap();
    let group = head(&relay, &hub, 2, Slow::new(Duration::from_secs(60))).await;
    let home = tempfile::tempdir().unwrap();
    let started = Instant::now();
    let answer = client::ask_model_within(
        &Identity::generate(),
        &ask(&relay, home.path(), Some(&group.key)),
        &want(),
        &Within::new(Duration::from_secs(600)),
        &[],
        None,
    )
    .await
    .unwrap();
    let declared = answer.declared.unwrap();
    assert_eq!(declared.ceiling_secs, Some(2));
    assert_eq!(declared.deadline_secs, 2);
    assert_eq!(declared.admitted_secs, Some(2));
    assert_eq!(
        answer.error.as_deref(),
        Some("limit_exceeded: the job ran out of time")
    );
    assert!(started.elapsed() < Duration::from_secs(2) + client::GRACE);
}

/// An extended job whose answer the output limit cut off is no answer.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_cut_off_answer_is_refused_under_the_extension() {
    let (relay, hub) = pylon::fixture::relay().await.unwrap();
    let cut = Arc::new(Slow {
        delay: Duration::from_millis(100),
        finish: Finish::Length,
        dropped: Arc::new(AtomicBool::new(false)),
    });
    let group = head(&relay, &hub, 600, cut).await;
    let home = tempfile::tempdir().unwrap();
    let answer = client::ask_model_within(
        &Identity::generate(),
        &ask(&relay, home.path(), Some(&group.key)),
        &want(),
        &Within::new(Duration::from_secs(20)),
        &[],
        None,
    )
    .await
    .unwrap();
    assert_eq!(answer.text, None);
    assert_eq!(
        answer.error.as_deref(),
        Some("limit_exceeded: the answer reached its output limit before it finished")
    );
}

/// Old buyer, new provider: an ordinary request to a model whose deadline
/// passes 90 seconds is refused at once with the recorded body, not run
/// past the old buyer's wait.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_ordinary_request_to_a_long_model_is_refused_at_once() {
    let (relay, hub) = pylon::fixture::relay().await.unwrap();
    let group = head(&relay, &hub, 600, Slow::new(Duration::from_secs(60))).await;
    let home = tempfile::tempdir().unwrap();
    let started = Instant::now();
    let answer = client::ask_model(
        &Identity::generate(),
        &ask(&relay, home.path(), Some(&group.key)),
        &want(),
        &[],
        None,
    )
    .await
    .unwrap();
    assert!(started.elapsed() < Duration::from_secs(5));
    let recorded: Value = serde_json::from_str(include_str!(
        "../../nostr/src/pylon/fixtures/job-long-model-refusal.json"
    ))
    .unwrap();
    assert_eq!(
        answer.error.as_deref(),
        Some(
            format!(
                "{}: {}",
                recorded["code"].as_str().unwrap(),
                recorded["message"].as_str().unwrap()
            )
            .as_str()
        )
    );
    assert!(answer.declared.is_none());
}

/// Ordinary jobs keep the 90-second profile: the request body is the
/// recorded one, and a complete pylon answers it as before. The same pylon
/// also takes an extended job, for the smaller of the budget and 90.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ordinary_jobs_keep_the_ninety_second_profile() {
    let mut body = job::request_body("What is 2 + 2?", &[]);
    body["client"] = json!("openagents-pylon 0.1.0");
    let recorded: Value = serde_json::from_str(include_str!(
        "../../nostr/src/pylon/fixtures/job-ordinary-request.json"
    ))
    .unwrap();
    assert_eq!(body, recorded);
    let mut extended = job::request_body_within("What is 2 + 2?", &[], 300);
    extended["client"] = json!("openagents-pylon 0.1.0");
    let recorded: Value = serde_json::from_str(include_str!(
        "../../nostr/src/pylon/fixtures/job-deadline-request.json"
    ))
    .unwrap();
    assert_eq!(extended, recorded);
    assert_eq!(
        Config::new("ws://x", "p", "/nonexistent".into()).job_timeout,
        Duration::from_secs(90)
    );

    let (relay, hub) = pylon::fixture::relay().await.unwrap();
    let rig = complete(&relay, &hub, Arc::new(Echo)).await;
    let home = tempfile::tempdir().unwrap();
    let plain = client::ask(
        &Identity::generate(),
        &ask(&relay, home.path(), Some(&rig.key)),
    )
    .await
    .unwrap();
    assert_eq!(plain.text.as_deref(), Some("echo: ?2 + 2 si tahW"));
    assert!(plain.declared.is_none());
    let within = client::ask_within(
        &Identity::generate(),
        &ask(&relay, home.path(), Some(&rig.key)),
        &Within::new(Duration::from_secs(600)),
        &[],
        None,
    )
    .await
    .unwrap();
    assert_eq!(within.text.as_deref(), Some("echo: ?2 + 2 si tahW"));
    let declared = within.declared.unwrap();
    assert_eq!(declared.ceiling_secs, None);
    assert_eq!(declared.deadline_secs, 90);
    assert_eq!(declared.admitted_secs, Some(90));
}

/// New buyer, old provider: a pylon from before the extension refuses any
/// feature, with the recorded body, and the buyer stops at once.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_old_provider_refuses_the_extended_form_at_once() {
    let (relay, hub) = pylon::fixture::relay().await.unwrap();
    // The old pylon's beacon: a complete pylon's, under its own key.
    let base = complete(&relay, &hub, Arc::new(Echo)).await;
    let old = Identity::generate();
    {
        let mut hub = hub.lock().await;
        let own = hub
            .stored
            .iter()
            .find(|e| e.kind == 30_200 && e.pubkey == base.key.pubkey())
            .cloned()
            .unwrap();
        let mut beacon = parse_beacon(&own).unwrap();
        beacon.provider = old.pubkey().to_string();
        beacon.pylon = "old".into();
        hub.stored
            .push(beacon_event(old.signer(), &beacon).unwrap());
    }
    // It answers every request with what an old pylon sends a request
    // that names a feature.
    let recorded: Value = serde_json::from_str(include_str!(
        "../../nostr/src/pylon/fixtures/job-old-provider-refusal.json"
    ))
    .unwrap();
    let mut conn = relay::connect(&relay, &old, LIFETIME).await.unwrap();
    conn.send(json!(["REQ", "jobs", {"kinds": [job::REQUEST_KIND], "#p": [old.pubkey()]}]))
        .await
        .unwrap();
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let saw = Arc::clone(&seen);
    let replier = old.clone();
    tokio::spawn(async move {
        while let Ok(frame) = conn.next().await {
            if let Frame::Event { event, .. } = Frame::parse(frame) {
                let body: Value =
                    serde_json::from_str(&job::open(&replier, &event).unwrap()).unwrap();
                // The old parser: any feature is refused.
                let refusal = deadline::request_profile(&body, false).unwrap_err();
                assert_eq!(recorded["code"], refusal.code);
                saw.lock().unwrap().push(body);
                let answer = job::seal(
                    &replier,
                    &event.pubkey,
                    job::FEEDBACK_KIND,
                    job::response_tags(&event),
                    &recorded,
                    pylon::now(),
                )
                .unwrap();
                conn.send(json!(["EVENT", answer])).await.unwrap();
            }
        }
    });
    let home = tempfile::tempdir().unwrap();
    let started = Instant::now();
    let answer = client::ask_within(
        &Identity::generate(),
        &ask(&relay, home.path(), Some(&old)),
        &Within::new(Duration::from_secs(600)),
        &[],
        None,
    )
    .await
    .unwrap();
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(
        answer.error.as_deref(),
        Some("unsupported_feature: this pylon requires no features")
    );
    assert_eq!(
        seen.lock().unwrap()[0]["requires"],
        json!([deadline::FEATURE])
    );
}

/// A wait longer than one relay connection's lifetime keeps the answer
/// subscription alive on fresh connections; the answer arrives after the
/// first connection is gone.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_answer_subscription_outlives_relay_connections() {
    let (relay, hub) = pylon::fixture::relay().await.unwrap();
    let rig = complete(&relay, &hub, Slow::new(Duration::from_millis(2_500))).await;
    let home = tempfile::tempdir().unwrap();
    let mut within = Within::new(Duration::from_secs(20));
    within.rotate = Duration::from_millis(500);
    let answer = client::ask_within(
        &Identity::generate(),
        &ask(&relay, home.path(), Some(&rig.key)),
        &within,
        &[],
        None,
    )
    .await
    .unwrap();
    assert_eq!(answer.text.as_deref(), Some("4"));
    let declared = answer.declared.unwrap();
    assert!(declared.connections >= 4, "{declared:?}");
}

fn stage_group(root: &std::path::Path) -> StageGroupConfig {
    use std::os::unix::fs::PermissionsExt;
    let script = root.join("stage.py");
    std::fs::write(&script, include_bytes!("fixtures/stage_worker.py")).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    StageGroupConfig {
        model: format!("{MODEL}@{ARTIFACT}"),
        weights: "sha256:fixture".into(),
        runtime: "fixture-runtime/1".into(),
        protocol: PROTOCOL,
        stages: ["s0", "s1"]
            .iter()
            .map(|name| StageSpec {
                name: (*name).into(),
                program: script.clone(),
                args: vec!["--root".into(), root.display().to_string()],
            })
            .collect(),
        step_timeout: Duration::from_millis(1_500),
        health_timeout: Duration::from_millis(600),
        startup_timeout: Duration::from_secs(5),
        stop_timeout: Duration::from_millis(800),
        max_restarts: 3,
        backoff: Duration::from_millis(20),
        max_backoff: Duration::from_millis(80),
    }
}

fn alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

/// Group mode with W10's synthetic stages: a deadline over 90 seconds now
/// serves extended jobs, a missed deadline cancels the group's run, and the
/// head's slot stays held until every stage has stopped, even one that
/// ignores the cancellation. Then the group answers again.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_missed_deadline_holds_the_group_until_its_stages_stop() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("slow-s1"), "200").unwrap();
    std::fs::write(root.path().join("ignore-cancel-s1"), "").unwrap();
    let group = Arc::new(
        StageGroup::start(
            stage_group(root.path()),
            Arc::new(|s| eprintln!("group: {s}")),
        )
        .await
        .unwrap(),
    );
    for _ in 0..500 {
        if group.status().admits() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(group.status().admits());
    let stubborn = group.status().stages[1].pid.unwrap();
    let (relay, hub) = pylon::fixture::relay().await.unwrap();
    let home = tempfile::tempdir().unwrap();
    let mut config = config(&relay, "v41-head", home.path());
    config.slots = 1;
    config.max_tokens = 64;
    config.job_timeout = Duration::from_secs(600);
    config.group = Some(HeadGroup {
        spec: spec(600),
        readiness: Arc::clone(&group) as Arc<dyn pylon::group::GroupReadiness>,
        stages: vec![
            ("s0".into(), Layers { from: 0, to: 20 }),
            ("s1".into(), Layers { from: 20, to: 43 }),
        ],
    });
    let engine = Arc::new(GroupEngine::new(
        Arc::clone(&group),
        format!("{MODEL}@{ARTIFACT}"),
        Sampling::new(0.6, 0.95, 7).unwrap(),
        64,
    ));
    let head = run(&hub, config, home, engine).await;
    let buyer = tempfile::tempdir().unwrap();
    let started = Instant::now();
    let missed = client::ask_model_within(
        &Identity::generate(),
        &ask(&relay, buyer.path(), Some(&head.key)),
        &want(),
        &Within::new(Duration::from_secs(1)),
        &[],
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        missed.error.as_deref(),
        Some("limit_exceeded: the job ran out of time"),
        "{missed:?} {:?}",
        group.status()
    );
    // The refusal comes only once the stubborn stage is gone.
    assert!(
        !alive(stubborn),
        "the head answered while a stage still ran"
    );
    assert!(started.elapsed() >= Duration::from_millis(1_700));
    assert!(!group.status().busy);

    // The group comes back with the stage replaced and answers in full:
    // the last stage ends the answer at step 3.
    std::fs::remove_file(root.path().join("ignore-cancel-s1")).unwrap();
    std::fs::remove_file(root.path().join("slow-s1")).unwrap();
    for _ in 0..800 {
        if group.status().admits() && group.status().generation >= 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    std::fs::write(root.path().join("end-s1"), "3").unwrap();
    let mut answer = None;
    for _ in 0..40 {
        let try_once = client::ask_model_within(
            &Identity::generate(),
            &ask(&relay, buyer.path(), Some(&head.key)),
            &want(),
            &Within::new(Duration::from_secs(30)),
            &[],
            None,
        )
        .await;
        // The head's beacon may still say busy for a moment.
        if let Ok(found) = try_once
            && found.text.is_some()
        {
            answer = Some(found);
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    let answer = answer.expect("the group answered after the replacement");
    assert_eq!(answer.text.as_deref(), Some("<7000><7001><7002><7003>"));
    let declared = answer.declared.unwrap();
    assert_eq!(declared.finish.as_deref(), Some("stop"));
    assert_eq!(declared.ceiling_secs, Some(600));
    group.stop().await;
}

/// A group head over W10's synthetic stages whose last stage answers with
/// `pieces`, `slow_ms` a step, on the in-process relay.
struct Fake {
    relay: String,
    root: tempfile::TempDir,
    group: Arc<StageGroup>,
    _head: Running,
}

async fn fake_group(pieces: &[&str], slow_ms: u64) -> Fake {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("slow-s1"), slow_ms.to_string()).unwrap();
    std::fs::write(
        root.path().join("answer-s1"),
        serde_json::to_string(pieces).unwrap(),
    )
    .unwrap();
    let mut stages = stage_group(root.path());
    stages.step_timeout = Duration::from_millis(slow_ms + 3_000);
    let group = Arc::new(
        StageGroup::start(stages, Arc::new(|s| eprintln!("group: {s}")))
            .await
            .unwrap(),
    );
    for _ in 0..500 {
        if group.status().admits() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let (relay, hub) = pylon::fixture::relay().await.unwrap();
    let home = tempfile::tempdir().unwrap();
    let mut config = config(&relay, "v41-head", home.path());
    config.slots = 1;
    config.max_tokens = 64;
    config.job_timeout = Duration::from_secs(600);
    config.group = Some(HeadGroup {
        spec: spec(600),
        readiness: Arc::clone(&group) as Arc<dyn pylon::group::GroupReadiness>,
        stages: vec![
            ("s0".into(), Layers { from: 0, to: 20 }),
            ("s1".into(), Layers { from: 20, to: 43 }),
        ],
    });
    let engine = Arc::new(GroupEngine::new(
        Arc::clone(&group),
        format!("{MODEL}@{ARTIFACT}"),
        Sampling::new(0.6, 0.95, 7).unwrap(),
        64,
    ));
    let head = run(&hub, config, home, engine).await;
    Fake {
        relay,
        root,
        group,
        _head: head,
    }
}

/// Run the W14 qualification harness (`run.py --v41-group`) against
/// `fake` with a budget of `budget` seconds, the real `pylon` binary, and a
/// private `HOME`. Returns its exit code and report.
async fn harness(fake: &Fake, budget: u64) -> (i32, Value) {
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scripts/bench/pylon-models/run.py");
    let output = fake.root.path().join("harness");
    let private = fake.root.path().join("home");
    std::fs::create_dir_all(&private).unwrap();
    let status = tokio::process::Command::new("python3")
        .arg(&script)
        .args(["--v41-group", "--pylon-bin", env!("CARGO_BIN_EXE_pylon")])
        .args(["--relay", &fake.relay, "--v41-want", ARTIFACT])
        .args(["--v41-budget", &budget.to_string(), "--cases", "arithmetic"])
        .arg("--output")
        .arg(&output)
        .env("HOME", &private)
        .env_remove("OPENAGENTS_PYLON_HOME")
        .kill_on_drop(true)
        .status()
        .await
        .unwrap();
    let report = std::fs::read(output.join("v41-group/results.json")).unwrap();
    (
        status.code().unwrap_or(-1),
        serde_json::from_slice(&report).unwrap(),
    )
}

fn passed(case: &Value, name: &str) -> bool {
    case["checks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["name"] == name && c["passed"] == true)
}

/// The harness qualifies a group that answers slowly but within the
/// deadline: the router's `Settings::want` request reaches it under the
/// extension, and the answer splits into reasoning and a final answer that
/// scores.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_harness_qualifies_a_slow_group_within_its_deadline() {
    let fake = fake_group(
        &["17 x 24 = 408, and 408 - 35 = 373.", "</think>", "373"],
        400,
    )
    .await;
    let (code, report) = harness(&fake, 20).await;
    assert_eq!(code, 0, "{report:#}");
    assert_eq!(report["qualified"], true);
    let case = &report["cases"][0];
    for check in [
        "answered",
        "extended",
        "finish_stop",
        "within_declared_deadline",
        "reasoning_and_final",
        "final_scores",
    ] {
        assert!(passed(case, check), "{check}: {case:#}");
    }
    let data = &case["cli"]["response"]["data"];
    assert!(data["answer_ms"].as_u64().unwrap() >= 1_000, "{data:#}");
    assert_eq!(data["declared"]["ceiling_secs"], 600);
    assert_eq!(data["declared"]["deadline_secs"], 20);
    fake.group.stop().await;
}

/// The harness fails a group that misses the deadline: the head ends the
/// job at the budget and the buyer hears `limit_exceeded`.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_harness_fails_a_group_that_misses_its_deadline() {
    let fake = fake_group(
        &["17 x 24 = 408, and 408 - 35 = 373.", "</think>", "373"],
        2_000,
    )
    .await;
    let (code, report) = harness(&fake, 2).await;
    assert_eq!(code, 1, "{report:#}");
    assert_eq!(report["qualified"], false);
    let case = &report["cases"][0];
    assert!(!passed(case, "answered"), "{case:#}");
    assert!(
        case["cli"]["response"]
            .to_string()
            .contains("limit_exceeded"),
        "{case:#}"
    );
    fake.group.stop().await;
}
