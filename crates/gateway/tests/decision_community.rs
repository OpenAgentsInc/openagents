#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The community decision lane (#12190) against real pylons on an
//! in-process relay: a contributor pylon is asked only on the community
//! lane and only after a trusted check-pass, while the operator pylon is
//! asked as before; a contributor answering with perturbed probabilities is
//! struck by a sampled recompute at the reference and benched; a canary
//! answered wrong strikes; three strikes need a check-pass newer than the
//! third; a key's community opt-in puts its decisions on the lane.

mod common;

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::body::Bytes;
use axum::http::HeaderMap;
use axum::routing::post;
use gateway::decision_community::{self, Strike};
use gateway::decision_dispatch::{Decisions, Dispatch};
use nostr::decision::Refusal;
use pylon::decide::{Decider, Fixed, Identity as Served};
use pylon::engine::Pending;
use pylon::identity::Identity;
use pylon::lease::Dedicated;
use pylon::provider::{Config, Provider};
use serde_json::{Map, Value, json};
use tokio::sync::Mutex;

type Hub = Arc<Mutex<pylon::fixture::Hub>>;

fn pinned() -> Served {
    Served {
        model: "clef-flash".into(),
        artifact_digest: Some(format!("sha256:{}", pylon::setup::CLEF_FLASH_SHA256)),
        model_id: None,
    }
}

/// A pylon serving the pinned model with made-up probabilities that still
/// agree with themselves: every noul 0.05, every choice its last option,
/// every score its first level.
struct Perturbed;

impl Decider for Perturbed {
    fn identity(&self) -> Served {
        pinned()
    }

    fn healthy(&self) -> Pending<'_, bool> {
        Box::pin(async { true })
    }

    fn decide<'a>(
        &'a self,
        _state: &'a Value,
        questions: &'a Map<String, Value>,
    ) -> Pending<'a, Result<Value, Refusal>> {
        Box::pin(async move {
            let mut answers = Map::new();
            for (id, q) in questions {
                let answer = match q["type"].as_str() {
                    Some("choice") => {
                        let keys: Vec<&String> =
                            q["criteria"].as_object().unwrap().keys().collect();
                        let n = keys.len();
                        let rest = 0.1 / (n - 1) as f64;
                        let probabilities: Map<String, Value> = keys
                            .iter()
                            .enumerate()
                            .map(|(i, k)| {
                                ((*k).clone(), json!(if i == n - 1 { 0.9 } else { rest }))
                            })
                            .collect();
                        json!({"type": "choice", "choice": keys[n - 1], "confidence": 0.9,
                               "probabilities": probabilities})
                    }
                    Some("score") => {
                        let n = q["criteria"].as_array().unwrap().len();
                        let probabilities: Map<String, Value> = (0..n)
                            .map(|i| (i.to_string(), json!(if i == 0 { 1.0 } else { 0.0 })))
                            .collect();
                        json!({"type": "score", "score": 0.0, "confidence": 1.0,
                               "probabilities": probabilities})
                    }
                    _ => json!({"type": "noul", "noul": 0.05}),
                };
                answers.insert(id.clone(), answer);
            }
            Ok(json!({"model": "clef-flash", "answers": answers,
                      "usage": {"input_tokens": 1, "output_tokens": 0}}))
        })
    }
}

/// The reference: a Psionic-shaped `/v1/systemone` answering as the pinned
/// model ([`Fixed`]).
async fn reference() -> String {
    async fn answer(Json(body): Json<Value>) -> Json<Value> {
        let fixed = Fixed { identity: pinned() };
        let questions = body["questions"].as_object().cloned().unwrap_or_default();
        Json(fixed.decide(&body["state"], &questions).await.unwrap())
    }
    let app = axum::Router::new().route("/v1/systemone", post(answer));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(axum::serve(listener, app).into_future());
    url
}

/// Start a decision pylon named `slug` answering with `decider`, admitting
/// the gateway's dispatch key; wait for its beacon.
async fn start(
    url: &str,
    hub: &Hub,
    slug: &str,
    dispatch: &str,
    decider: Arc<dyn Decider>,
) -> (
    Identity,
    tokio::sync::oneshot::Sender<()>,
    tempfile::TempDir,
) {
    let home = tempfile::tempdir().unwrap();
    let key = Identity::generate();
    let mut config = Config::new(url, slug, home.path().to_path_buf());
    config.allow = Some(BTreeSet::from([dispatch.to_string()]));
    config.rate_per_minute = 600;
    config.slots = 4;
    let provider =
        Provider::deciding(config, key.clone(), None, decider, Arc::new(Dedicated)).unwrap();
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(Arc::clone(&provider).run(async {
        let _ = stopped.await;
    }));
    for _ in 0..200 {
        if hub
            .lock()
            .await
            .stored
            .iter()
            .any(|e| e.kind == 30_200 && e.pubkey == key.pubkey())
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    (key, stop, home)
}

/// A trusted checker's verdict on a decision receipt for `pylon`, put on
/// the relay.
async fn verdict(
    hub: &Hub,
    checker: &Identity,
    pylon: &Identity,
    slug: &str,
    verdict: nostr::pylon::Verdict,
    at: u64,
) {
    let buyer = Identity::generate();
    let receipt = nostr::pylon::Receipt {
        v: nostr::pylon::RECEIPT_V.into(),
        requires: Vec::new(),
        meta: None,
        buyer: buyer.pubkey().to_string(),
        provider: pylon.pubkey().to_string(),
        pylon: slug.into(),
        lane: nostr::pylon::Lane::CjDecision,
        capability: format!("{}:pylon/decision", pylon.pubkey()),
        request: "11".repeat(32),
        request_digest: format!("{at:064x}"),
        result_digest: Some("22".repeat(32)),
        started_at: at - 5,
        finished_at: at - 1,
        units: nostr::pylon::Units {
            kind: nostr::pylon::UnitKind::Jobs,
            count: 8,
        },
        outcome: nostr::pylon::Outcome::Accepted,
        payment: None,
    };
    let receipt = nostr::pylon::receipt_event(buyer.signer(), &receipt, at).unwrap();
    let label = nostr::pylon::check_event(
        checker.signer(),
        verdict,
        &receipt,
        &"33".repeat(32),
        "decision-recompute clef-flash verified:8/8",
        at,
    )
    .unwrap();
    let mut hub = hub.lock().await;
    hub.stored.push(receipt);
    hub.stored.push(label);
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn decisions(url: &str, operators: &[&str], checker: &Identity) -> Decisions {
    Decisions {
        relay: Some(url.to_owned()),
        pylons: operators.iter().map(|k| (*k).to_owned()).collect(),
        checkers: vec![checker.npub()],
        vertex: None,
        no_jev: true,
        jev_secret: None,
        shadow: 0.0,
        sample: 0.0,
        canary_rate: 0.0,
        deadline_ms: 6_000,
        pylon_ms: 3_000,
        ..Decisions::default()
    }
}

fn request() -> Bytes {
    Bytes::from(
        json!({
            "model": "openagents/decide",
            "state": "I was billed twice for my subscription",
            "questions": {
                "topic": {"type": "choice", "instructions": "Which team?",
                          "criteria": {"billing": "money", "technical": "bugs"}},
                "refund": {"type": "noul", "instructions": "Money back?"},
                "sev": {"type": "score", "criteria": ["low", "mid", "high"]}
            }
        })
        .to_string(),
    )
}

fn lane(name: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("x-decision-lane", name.parse().unwrap());
    headers
}

/// Ask once: the status and the answering door.
async fn ask(dispatch: &Arc<Dispatch>, headers: &HeaderMap) -> (u16, Value) {
    let response = dispatch.handle(headers, &request()).await.unwrap();
    let status = response.status().as_u16();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

async fn strikes(registry: &std::path::Path) -> Vec<Strike> {
    for _ in 0..200 {
        let found = decision_community::read_strikes(registry, 0);
        if !found.is_empty() {
            return found;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    Vec::new()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_contributor_is_asked_only_on_the_community_lane_after_a_check_pass() {
    let (url, hub) = pylon::fixture::relay().await.unwrap();
    let registry = tempfile::tempdir().unwrap();
    tenancy::Registry::install(
        registry.path(),
        common::manifest(&common::artifact('a'), None),
    )
    .unwrap();
    let checker = Identity::generate();
    // The operator pylon's key is known before it starts.
    let operator_key = Identity::generate();
    let dispatch = Dispatch::open(
        decisions(&url, &[operator_key.pubkey()], &checker),
        registry.path(),
    )
    .unwrap();
    let home = tempfile::tempdir().unwrap();
    let mut config = Config::new(&url, "operator", home.path().to_path_buf());
    config.allow = Some(BTreeSet::from([dispatch.pubkey().to_string()]));
    config.rate_per_minute = 600;
    // The operator serves an unpinned build: operators are trusted by key.
    let operator = Provider::deciding(
        config,
        operator_key.clone(),
        None,
        Arc::new(Fixed {
            identity: Served {
                model: "clef-flash".into(),
                artifact_digest: Some(format!("sha256:{}", "ab".repeat(32))),
                model_id: None,
            },
        }),
        Arc::new(Dedicated),
    )
    .unwrap();
    let (_stop_operator, stopped) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(Arc::clone(&operator).run(async {
        let _ = stopped.await;
    }));
    let (contributor, _stop, _home) = start(
        &url,
        &hub,
        "contrib",
        dispatch.pubkey(),
        Arc::new(Fixed { identity: pinned() }),
    )
    .await;

    // Not checked yet: every lane is answered by the operator.
    let (status, answer) = ask(&dispatch, &HeaderMap::new()).await;
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["service"]["door"], "pylon:operator");
    let (_, answer) = ask(&dispatch, &lane("community")).await;
    assert_eq!(answer["service"]["door"], "pylon:operator");

    // A trusted check-pass admits it, on the community lane only.
    verdict(
        &hub,
        &checker,
        &contributor,
        "contrib",
        nostr::pylon::Verdict::Pass,
        now() - 60,
    )
    .await;
    dispatch.forget_book().await;
    let (_, answer) = ask(&dispatch, &HeaderMap::new()).await;
    assert_eq!(answer["service"]["door"], "pylon:operator");
    let (_, answer) = ask(&dispatch, &lane("default")).await;
    assert_eq!(answer["service"]["door"], "pylon:operator");
    let (status, answer) = ask(&dispatch, &lane("community")).await;
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["service"]["door"], "pylon:contrib");
    assert_eq!(answer["service"]["lane"], "community");
    assert_eq!(answer["service"]["provider"], contributor.pubkey());

    // A key whose holder opted in is on the community lane without the
    // header; `X-Decision-Lane: default` keeps a decision off it.
    let manifest = tenancy::Registry::open(registry.path())
        .unwrap()
        .manifest()
        .clone();
    let issued = tenancy::keys::issue(registry.path(), &manifest, "acme").unwrap();
    let mut keyed = HeaderMap::new();
    keyed.insert(
        "authorization",
        format!("Bearer {}", issued.token).parse().unwrap(),
    );
    let (_, answer) = ask(&dispatch, &keyed).await;
    assert_eq!(answer["service"]["door"], "pylon:operator");
    tenancy::keys::set_community(registry.path(), &issued.key.id, true).unwrap();
    let (_, answer) = ask(&dispatch, &keyed).await;
    assert_eq!(answer["service"]["door"], "pylon:contrib");
    keyed.insert("x-decision-lane", "default".parse().unwrap());
    let (_, answer) = ask(&dispatch, &keyed).await;
    assert_eq!(answer["service"]["door"], "pylon:operator");

    // A later trusted check-fail takes it out again.
    verdict(
        &hub,
        &checker,
        &contributor,
        "contrib",
        nostr::pylon::Verdict::Fail,
        now() - 30,
    )
    .await;
    dispatch.forget_book().await;
    let (_, answer) = ask(&dispatch, &lane("community")).await;
    assert_eq!(answer["service"]["door"], "pylon:operator");

    // The decision log names the lane.
    let log = std::fs::read_dir(registry.path().join("decisions"))
        .unwrap()
        .filter_map(Result::ok)
        .find(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.ends_with(".jsonl") && name.starts_with("20")
        })
        .map(|e| std::fs::read_to_string(e.path()).unwrap())
        .unwrap_or_default();
    assert!(log.contains("\"lane\":\"community\""), "{log}");
    assert!(!log.contains("billed twice"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn perturbed_probabilities_are_struck_by_sampled_recompute_and_benched() {
    let (url, hub) = pylon::fixture::relay().await.unwrap();
    let registry = tempfile::tempdir().unwrap();
    let checker = Identity::generate();
    let mut config = decisions(&url, &[], &checker);
    config.reference = Some(reference().await);
    config.sample = 1.0;
    let dispatch = Dispatch::open(config, registry.path()).unwrap();
    let (cheat, _stop, _home) =
        start(&url, &hub, "cheat", dispatch.pubkey(), Arc::new(Perturbed)).await;
    verdict(
        &hub,
        &checker,
        &cheat,
        "cheat",
        nostr::pylon::Verdict::Pass,
        now() - 60,
    )
    .await;

    // The caller is answered; the recompute comes after.
    let (status, answer) = ask(&dispatch, &lane("community")).await;
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["service"]["door"], "pylon:cheat");
    let struck = strikes(registry.path()).await;
    assert_eq!(struck.len(), 1, "{struck:?}");
    assert_eq!(struck[0].pylon, cheat.pubkey());
    assert_eq!(struck[0].reason, "sampled_item");
    assert!(struck[0].value.unwrap() > 0.5);

    // Benched: with no other door the community lane has no answer.
    let (status, refused) = ask(&dispatch, &lane("community")).await;
    assert_eq!(status, 503, "{refused}");
    // The strike is in the decision log without question text.
    let log = std::fs::read_dir(registry.path().join("decisions"))
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().starts_with("20"))
        .map(|e| std::fs::read_to_string(e.path()).unwrap())
        .collect::<String>();
    assert!(log.contains("\"event\":\"strike\""), "{log}");
    assert!(!log.contains("billed twice"));

    // A restart keeps the strike.
    drop(dispatch);
    let mut config = decisions(&url, &[], &checker);
    config.reference = Some(reference().await);
    let again = Dispatch::open(config, registry.path()).unwrap();
    let (status, _) = ask(&again, &lane("community")).await;
    assert_eq!(status, 503);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_honest_contributor_passes_sampled_recompute() {
    let (url, hub) = pylon::fixture::relay().await.unwrap();
    let registry = tempfile::tempdir().unwrap();
    let checker = Identity::generate();
    let mut config = decisions(&url, &[], &checker);
    config.reference = Some(reference().await);
    config.sample = 1.0;
    let dispatch = Dispatch::open(config, registry.path()).unwrap();
    let (honest, _stop, _home) = start(
        &url,
        &hub,
        "honest",
        dispatch.pubkey(),
        Arc::new(Fixed { identity: pinned() }),
    )
    .await;
    verdict(
        &hub,
        &checker,
        &honest,
        "honest",
        nostr::pylon::Verdict::Pass,
        now() - 60,
    )
    .await;
    for _ in 0..4 {
        let (status, answer) = ask(&dispatch, &lane("community")).await;
        assert_eq!(status, 200, "{answer}");
        assert_eq!(answer["service"]["door"], "pylon:honest");
    }
    // Every recompute was logged as a pass and none struck.
    let mut passes = 0;
    for _ in 0..200 {
        let log = std::fs::read_dir(registry.path().join("decisions"))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().starts_with("20"))
            .map(|e| std::fs::read_to_string(e.path()).unwrap())
            .collect::<String>();
        passes = log
            .lines()
            .filter(|l| l.contains("\"event\":\"sampled\"") && l.contains("\"outcome\":\"pass\""))
            .count();
        if passes == 4 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(passes, 4);
    assert!(decision_community::read_strikes(registry.path(), 0).is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_canary_answered_wrong_strikes() {
    let (url, hub) = pylon::fixture::relay().await.unwrap();
    let registry = tempfile::tempdir().unwrap();
    let checker = Identity::generate();
    // A checker-private pool answered by the pinned model.
    let pool = pylon::canary::make(
        pylon::canary::seed(b"the checker's secret seed file"),
        &Fixed { identity: pinned() },
        6,
    )
    .await
    .unwrap();
    let path = registry.path().join("canaries.jsonl");
    pylon::canary::write(&path, &pool).unwrap();
    let mut config = decisions(&url, &[], &checker);
    config.canaries = Some(path);
    config.canary_rate = 1.0;
    let dispatch = Dispatch::open(config, registry.path()).unwrap();
    let (cheat, _stop, _home) =
        start(&url, &hub, "cheat", dispatch.pubkey(), Arc::new(Perturbed)).await;
    verdict(
        &hub,
        &checker,
        &cheat,
        "cheat",
        nostr::pylon::Verdict::Pass,
        now() - 60,
    )
    .await;
    // The caller gets its own answer, never the canary's.
    let (status, answer) = ask(&dispatch, &lane("community")).await;
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["service"]["door"], "pylon:cheat");
    assert!(answer["answers"]["topic"].is_object());
    let struck = strikes(registry.path()).await;
    assert_eq!(struck.len(), 1, "{struck:?}");
    assert_eq!(struck[0].reason, "canary_item");
    assert_eq!(struck[0].pylon, cheat.pubkey());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn three_strikes_need_a_check_pass_newer_than_the_third() {
    let (url, hub) = pylon::fixture::relay().await.unwrap();
    let registry = tempfile::tempdir().unwrap();
    let checker = Identity::generate();
    let dispatch = Dispatch::open(decisions(&url, &[], &checker), registry.path()).unwrap();
    let (pylon_key, _stop, _home) = start(
        &url,
        &hub,
        "struck",
        dispatch.pubkey(),
        Arc::new(Fixed { identity: pinned() }),
    )
    .await;
    drop(dispatch);
    let t = now();
    verdict(
        &hub,
        &checker,
        &pylon_key,
        "struck",
        nostr::pylon::Verdict::Pass,
        t - 400,
    )
    .await;
    // Three strikes this past hour, written as the gateway writes them.
    let dir = registry.path().join("decisions");
    let strikes = decision_community::Strikes::open(&dir);
    for at in [t - 300, t - 200, t - 100] {
        strikes.record(&Strike {
            at,
            pylon: pylon_key.pubkey().to_string(),
            address: format!("30200:{}:struck", pylon_key.pubkey()),
            owner: pylon_key.pubkey().to_string(),
            reason: "canary_item".into(),
            value: Some(1.0),
            request: None,
        });
    }
    // A pass between the second and third strikes does not admit it.
    verdict(
        &hub,
        &checker,
        &pylon_key,
        "struck",
        nostr::pylon::Verdict::Pass,
        t - 150,
    )
    .await;
    let dispatch = Dispatch::open(decisions(&url, &[], &checker), registry.path()).unwrap();
    let (status, _) = ask(&dispatch, &lane("community")).await;
    assert_eq!(status, 503);
    // One newer than the third does.
    verdict(
        &hub,
        &checker,
        &pylon_key,
        "struck",
        nostr::pylon::Verdict::Pass,
        t - 50,
    )
    .await;
    dispatch.forget_book().await;
    let (status, answer) = ask(&dispatch, &lane("community")).await;
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["service"]["door"], "pylon:struck");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn without_checkers_the_lane_header_changes_nothing() {
    let (url, hub) = pylon::fixture::relay().await.unwrap();
    let registry = tempfile::tempdir().unwrap();
    // Today's config: no checkers, no pylon list, so every pylon counts.
    let dispatch = Dispatch::open(
        Decisions {
            relay: Some(url.clone()),
            vertex: None,
            no_jev: true,
            jev_secret: None,
            shadow: 0.0,
            deadline_ms: 6_000,
            pylon_ms: 3_000,
            ..Decisions::default()
        },
        registry.path(),
    )
    .unwrap();
    let (key, _stop, _home) = start(
        &url,
        &hub,
        "any",
        dispatch.pubkey(),
        Arc::new(Fixed {
            identity: Served {
                model: "clef-flash".into(),
                artifact_digest: Some(format!("sha256:{}", "cd".repeat(32))),
                model_id: None,
            },
        }),
    )
    .await;
    for headers in [HeaderMap::new(), lane("community")] {
        let (status, answer) = ask(&dispatch, &headers).await;
        assert_eq!(status, 200, "{answer}");
        assert_eq!(answer["service"]["door"], "pylon:any");
        assert_eq!(answer["service"]["provider"], key.pubkey());
        assert!(answer["service"]["lane"].is_null());
    }
}
