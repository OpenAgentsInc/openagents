#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The pylon router (#12366) end to end against an in-process relay: real
//! providers on fake engines, a closed pylon that refuses strangers, a
//! pylon that serves decisions, and a fixed decision in place of the
//! Decisions API.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nostr::decision::Refusal;
use nostr::pylon::{Class, Family, Tier};
use pylon::decide::{Decider, Identity as Served};
use pylon::engine::{Echo, Engine, Pending, Turn};
use pylon::identity::Identity;
use pylon::lease::Dedicated;
use pylon::provider::{Config, Provider};
use pylon::router::{self, DEFAULT, Decide, Deciding, How, Outcome, Question, Settings, Verdict};
use serde_json::{Map, Value};
use tokio::sync::oneshot;

/// A decision server that never gets asked here: it only makes its pylon
/// advertise the decision lane.
struct Clef;

impl Decider for Clef {
    fn identity(&self) -> Served {
        Served {
            model: "clef-flash".into(),
            artifact_digest: Some(format!("sha256:{}", "cd".repeat(32))),
            model_id: None,
        }
    }

    fn healthy(&self) -> Pending<'_, bool> {
        Box::pin(async { true })
    }

    fn decide<'a>(
        &'a self,
        _state: &'a Value,
        _questions: &'a Map<String, Value>,
    ) -> Pending<'a, Result<Value, Refusal>> {
        Box::pin(async { Err(Refusal::new("unavailable")) })
    }
}

/// The Decisions API's stand-in: answers every question with `answer`
/// and keeps what it was asked.
struct Fixed {
    answer: Result<Verdict, String>,
    asked: Mutex<Vec<Question>>,
}

impl Fixed {
    fn new(answer: Result<Verdict, String>) -> Self {
        Self {
            answer,
            asked: Mutex::new(Vec::new()),
        }
    }

    fn picks(choice: &str, confidence: f64) -> Self {
        Self::new(Ok(Verdict {
            choice: choice.into(),
            confidence,
            door: Some("pylon:clef".into()),
            model: Some("clef-flash".into()),
        }))
    }
}

impl Decide for Fixed {
    fn decide<'a>(&'a self, question: &'a Question) -> Deciding<'a> {
        self.asked.lock().unwrap().push(question.clone());
        let answer = self.answer.clone();
        Box::pin(async move { answer })
    }
}

/// One running pylon.
struct Running {
    key: Identity,
    _home: tempfile::TempDir,
    _stop: oneshot::Sender<()>,
}

/// The relay and its pylons: `closed` (extra-large, refuses strangers),
/// `rig` (large, open, echoes), and, with `clef`, a pylon serving only
/// decisions.
struct Net {
    relay: String,
    closed: Running,
    rig: Running,
    _clef: Option<Running>,
}

async fn start(
    relay: &str,
    slug: &str,
    tier: Tier,
    open: bool,
    engine: Option<Arc<dyn Engine>>,
    decides: bool,
) -> Running {
    let home = tempfile::tempdir().unwrap();
    let key = Identity::generate();
    let mut config = Config::new(relay, slug, home.path().to_path_buf());
    config.label = slug.to_uppercase();
    config.allow = if open { None } else { Some(BTreeSet::new()) };
    config.rate_per_minute = 600;
    config.class = Class {
        family: Family::Gpu,
        tier,
        memory_gb: 32,
    };
    let provider = if decides {
        Provider::deciding(
            config,
            key.clone(),
            engine,
            Arc::new(Clef),
            Arc::new(Dedicated),
        )
    } else {
        Provider::new(config, key.clone(), engine.unwrap())
    }
    .unwrap();
    let (stop, stopped) = oneshot::channel::<()>();
    tokio::spawn(provider.run(async {
        let _ = stopped.await;
    }));
    Running {
        key,
        _home: home,
        _stop: stop,
    }
}

async fn net(clef: bool) -> Net {
    let (relay, hub) = pylon::fixture::relay().await.unwrap();
    let closed = start(
        &relay,
        "closed",
        Tier::Xl,
        false,
        Some(Arc::new(Echo)),
        false,
    )
    .await;
    let rig = start(
        &relay,
        "rig",
        Tier::Large,
        true,
        Some(Arc::new(Echo)),
        false,
    )
    .await;
    let clef = if clef {
        Some(start(&relay, "clef", Tier::Medium, true, None, true).await)
    } else {
        None
    };
    let want = 2 + usize::from(clef.is_some());
    for _ in 0..200 {
        let beacons = hub
            .lock()
            .await
            .stored
            .iter()
            .filter(|e| e.kind == 30_200)
            .count();
        if beacons >= want {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    Net {
        relay,
        closed,
        rig,
        _clef: clef,
    }
}

fn settings(relay: &str, home: &std::path::Path) -> Settings {
    let mut settings = Settings::new(relay, home.to_path_buf());
    settings.try_wait = Duration::from_secs(10);
    settings
}

fn turns(text: &str) -> Vec<Turn> {
    vec![Turn {
        role: "user".into(),
        content: text.into(),
    }]
}

fn answered(routed: &router::Routed) -> &pylon::client::Answer {
    match &routed.outcome {
        Outcome::Answered(answer) => answer,
        other => panic!("no pylon answered: {other:?} after {:?}", routed.failed),
    }
}

#[tokio::test]
async fn with_no_decision_model_the_ranking_chooses_and_fails_over() {
    let net = net(false).await;
    let home = tempfile::tempdir().unwrap();
    let decide = Fixed::picks("p1", 1.0);
    let routed = router::route(
        &Identity::generate(),
        &settings(&net.relay, home.path()),
        &decide,
        &turns("abc"),
        Some("Answer briefly."),
    )
    .await;
    assert_eq!(routed.candidates, 2);
    assert_eq!(routed.deciders, 0);
    assert_eq!(
        routed.how,
        How::Ranked {
            why: "no decision model online".into()
        }
    );
    // Nothing asked the decision model.
    assert!(decide.asked.lock().unwrap().is_empty());
    // The extra-large pylon ranks first and refuses a stranger; the next
    // one answers.
    assert_eq!(routed.failed.len(), 1);
    assert!(routed.failed[0].0.contains(net.closed.key.pubkey()));
    assert!(
        routed.failed[0].1.starts_with("not_admitted"),
        "{:?}",
        routed.failed
    );
    let answer = answered(&routed);
    assert!(answer.pylon.contains(net.rig.key.pubkey()));
    assert_eq!(answer.text.as_deref(), Some("echo: cba"));
}

#[tokio::test]
async fn a_decision_pylon_online_lets_the_model_choose() {
    let net = net(true).await;
    let home = tempfile::tempdir().unwrap();
    // `rig` ranks second (`p2`), after the extra-large `closed`.
    let decide = Fixed::picks("p2", 0.7);
    let routed = router::route(
        &Identity::generate(),
        &settings(&net.relay, home.path()),
        &decide,
        &turns("hello"),
        None,
    )
    .await;
    assert_eq!(routed.deciders, 1);
    // The decision pylon serves no conversation lane, so it isn't offered.
    let asked = decide.asked.lock().unwrap().clone();
    assert_eq!(asked.len(), 1);
    let values: Vec<&str> = asked[0].choices.iter().map(|(v, _)| v.as_str()).collect();
    assert_eq!(values, ["p1", "p2", DEFAULT]);
    assert!(asked[0].choices[1].1.starts_with("RIG: runs echo"));
    assert!(asked[0].input.contains("hello"));
    match &routed.how {
        How::Decided {
            choice,
            door,
            model,
            ..
        } => {
            assert_eq!(choice, "p2");
            assert_eq!(door.as_deref(), Some("pylon:clef"));
            assert_eq!(model.as_deref(), Some("clef-flash"));
        }
        other => panic!("the model didn't decide: {other:?}"),
    }
    // The chosen pylon answered first; nothing failed.
    assert!(routed.failed.is_empty(), "{:?}", routed.failed);
    let answer = answered(&routed);
    assert!(answer.pylon.contains(net.rig.key.pubkey()));
    assert_eq!(answer.text.as_deref(), Some("echo: olleh"));
}

#[tokio::test]
async fn a_confident_default_hands_the_message_back() {
    let net = net(true).await;
    let home = tempfile::tempdir().unwrap();
    let routed = router::route(
        &Identity::generate(),
        &settings(&net.relay, home.path()),
        &Fixed::picks(DEFAULT, 0.9),
        &turns("What's the weather in Austin right now?"),
        None,
    )
    .await;
    assert!(matches!(routed.outcome, Outcome::Declined), "{routed:?}");
    assert!(routed.failed.is_empty());
}

#[tokio::test]
async fn a_failed_decision_falls_back_to_the_ranking() {
    let net = net(true).await;
    let home = tempfile::tempdir().unwrap();
    let routed = router::route(
        &Identity::generate(),
        &settings(&net.relay, home.path()),
        &Fixed::new(Err("busy".into())),
        &turns("xyz"),
        None,
    )
    .await;
    assert_eq!(
        routed.how,
        How::Ranked {
            why: "the decision failed: busy".into()
        }
    );
    assert_eq!(answered(&routed).text.as_deref(), Some("echo: zyx"));
}

#[tokio::test]
async fn an_unreachable_relay_leaves_the_message_for_the_hosted_chat() {
    let home = tempfile::tempdir().unwrap();
    let mut settings = Settings::new("ws://127.0.0.1:9", home.path().to_path_buf());
    settings.discover = Duration::from_secs(2);
    let routed = router::route(
        &Identity::generate(),
        &settings,
        &Fixed::picks("p1", 1.0),
        &turns("hi"),
        None,
    )
    .await;
    assert!(
        matches!(routed.outcome, Outcome::Unavailable(_)),
        "{routed:?}"
    );
}
