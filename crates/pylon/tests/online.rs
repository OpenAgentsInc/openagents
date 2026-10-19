#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Pylon 1.0's `up`, `down`, `status`, and `id` against an in-process
//! relay, with fake engine children (`sleep`) and fixed engines, so no GPU
//! and no Psionic build is needed.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use nostr::pylon::{Check, Verdict};
use pylon::decide::{Decider, Fixed, Identity as Served};
use pylon::engine::{Echo, Engine, Pending};
use pylon::lease::Dedicated;
use pylon::online::{
    self, Engines, LaneKind, LaneLive, LaneSpec, Live, Options, Phase, Up, load_live,
};
use pylon::share::{self, Settings};
use serde_json::Value;

/// Engines that need nothing: each child is `sleep`, decisions come from
/// [`Fixed`], text from [`Echo`]; the self-check passes unless `fail`.
struct Fake {
    fail: bool,
}

impl Engines for Fake {
    fn command(&self, _home: &Path, _lane: &LaneSpec) -> Result<tokio::process::Command, String> {
        let mut command = tokio::process::Command::new("sleep");
        command.arg("600");
        Ok(command)
    }

    fn decider(&self, _url: &str) -> Result<Arc<dyn Decider>, String> {
        Ok(Arc::new(Fixed {
            identity: Served {
                model: "clef-flash".into(),
                artifact_digest: Some("sha256:test".into()),
            },
        }))
    }

    fn text(&self, _url: &str, _model: &str) -> Result<Arc<dyn Engine>, String> {
        Ok(Arc::new(Echo))
    }

    fn ready<'a>(&'a self, _lane: &'a LaneSpec) -> Pending<'a, bool> {
        Box::pin(async { true })
    }

    fn selfcheck<'a>(
        &'a self,
        _home: &'a Path,
        _lane: &'a LaneSpec,
    ) -> Pending<'a, Result<(), String>> {
        let fail = self.fail;
        Box::pin(async move {
            if fail {
                Err("self-check failed: 80.0% of rounds pass, worst item 0.400".into())
            } else {
                Ok(())
            }
        })
    }
}

fn options(fail: bool) -> Options {
    Options {
        engines: Arc::new(Fake { fail }),
        every: Duration::from_millis(100),
        backoff: Duration::from_millis(50),
        max_backoff: Duration::from_secs(1),
        kind: "host",
        exit_when_off: false,
    }
}

/// `up`'s settings for a test pylon on `relay`, with a text lane.
fn up_settings(home: &Path, relay: &str) -> Settings {
    let text_model = home.join("Qwen3.5-0.8B-Q8_0.gguf");
    std::fs::write(&text_model, b"gguf").unwrap();
    let choice = Up {
        text: true,
        text_model: Some(text_model),
        serve: vec!["--pylon".into(), "test-pylon".into()],
        ..Up::default()
    };
    let (settings, notes) =
        online::settings_for_up(None, "host", relay, &choice, "cpu", Ok(())).unwrap();
    assert!(notes.is_empty(), "{notes:?}");
    share::save(home, &settings).unwrap();
    settings
}

async fn until(what: &str, secs: u64, mut check: impl FnMut() -> bool) {
    let started = Instant::now();
    while !check() {
        assert!(
            started.elapsed() < Duration::from_secs(secs),
            "timed out waiting for {what}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .is_ok_and(|s| s.success())
}

fn lane(live: &Live, kind: LaneKind) -> LaneLive {
    live.lanes.iter().find(|l| l.lane == kind).unwrap().clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn up_runs_the_engines_restarts_a_killed_one_and_down_stops_them() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().to_path_buf();
    let (relay, hub) = pylon::fixture::relay().await.unwrap();
    let settings = up_settings(&home, &relay);

    // `up` persisted the wanted state: on, both lanes, any key admitted.
    let held = share::load(&home).unwrap().unwrap();
    assert!(held.on);
    assert_eq!(held, settings);
    assert_eq!(held.decide.as_ref().unwrap().port, 18_096);
    assert_eq!(held.decide.as_ref().unwrap().device, "cpu");
    assert_eq!(held.text.as_ref().unwrap().port, 18_097);
    assert_eq!(held.text.as_ref().unwrap().model, "qwen3.5-0.8b-q8_0");
    assert_eq!(held.allow, None);
    assert_eq!(held.schema, share::SCHEMA);

    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let supervisor = tokio::spawn(online::supervise(
        home.clone(),
        Arc::new(Dedicated),
        options(false),
        Arc::new(|line| eprintln!("supervisor: {line}")),
        async {
            let _ = stopped.await;
        },
    ));

    until("serving with both engines", 20, || {
        load_live(&home).is_some_and(|l| {
            l.state == Phase::Serving && l.lanes.len() == 2 && l.lanes.iter().all(|x| x.up)
        })
    })
    .await;
    let live = load_live(&home).unwrap();
    assert_eq!(live.supervisor, "host");
    assert_eq!(live.model.as_deref(), Some("clef-flash@sha256:test"));
    assert!(live.npub.as_deref().unwrap().starts_with("npub1"));
    until("an online beacon", 20, || {
        hub.try_lock().is_ok_and(|h| {
            h.stored
                .iter()
                .any(|e| e.kind == 30_200 && e.content.contains("pylon/decision"))
        })
    })
    .await;

    // Kill the decision engine: the supervisor starts it again.
    let first = lane(&live, LaneKind::Decisions).pid.unwrap();
    assert!(alive(first));
    let text_pid = lane(&live, LaneKind::Text).pid.unwrap();
    std::process::Command::new("kill")
        .args(["-9", &first.to_string()])
        .status()
        .unwrap();
    until("the decision engine back", 20, || {
        load_live(&home).is_some_and(|l| {
            let d = lane(&l, LaneKind::Decisions);
            d.up && d.restarts >= 1 && d.pid.is_some_and(|p| p != first)
        })
    })
    .await;
    let live = load_live(&home).unwrap();
    let decisions = lane(&live, LaneKind::Decisions);
    assert!(
        decisions
            .last_exit
            .as_deref()
            .unwrap()
            .starts_with("exited"),
        "{decisions:?}"
    );
    assert_eq!(lane(&live, LaneKind::Text).pid, Some(text_pid));
    let second = decisions.pid.unwrap();

    // `down` waits for the supervisor to stop everything and records off.
    let (value, text) = online::down_at(&home, Duration::from_secs(30))
        .await
        .unwrap();
    assert_eq!(value["was_on"], true);
    assert_eq!(value["stopped_running"], true);
    assert!(text.contains("is off"), "{text}");
    assert!(!share::load(&home).unwrap().unwrap().on);
    let live = load_live(&home).unwrap();
    assert_eq!(live.state, Phase::Off);
    assert!(live.lanes.is_empty());
    until("the engines gone", 10, || {
        !alive(second) && !alive(text_pid)
    })
    .await;
    let offline = hub.lock().await.stored.iter().any(|e| {
        e.kind == 30_200
            && serde_json::from_str::<Value>(&e.content).unwrap()["status"] == "offline"
    });
    assert!(offline, "an offline beacon after down");

    // Off stays off: the supervisor does not bring it back.
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(load_live(&home).unwrap().state, Phase::Off);
    let _ = stop.send(());
    supervisor.await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failed_self_check_keeps_the_pylon_from_serving() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().to_path_buf();
    let (relay, hub) = pylon::fixture::relay().await.unwrap();
    up_settings(&home, &relay);
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let supervisor = tokio::spawn(online::supervise(
        home.clone(),
        Arc::new(Dedicated),
        options(true),
        Arc::new(|_| {}),
        async {
            let _ = stopped.await;
        },
    ));
    until("paused after the self-check", 20, || {
        load_live(&home).is_some_and(|l| {
            l.state == Phase::Paused && l.why.as_deref().is_some_and(|w| w.contains("self-check"))
        })
    })
    .await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !hub.lock().await.stored.iter().any(|e| e.kind == 30_200),
        "no beacon from a pylon that failed its self-check"
    );
    let _ = stop.send(());
    supervisor.await.unwrap();
    assert_eq!(load_live(&home).unwrap().state, Phase::Off);
}

#[tokio::test(flavor = "multi_thread")]
async fn down_with_nothing_running_publishes_the_offline_beacon_itself() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().to_path_buf();
    let (relay, hub) = pylon::fixture::relay().await.unwrap();
    up_settings(&home, &relay);
    let (value, _) = online::down_at(&home, Duration::from_secs(5))
        .await
        .unwrap();
    assert_eq!(value["stopped_running"], false);
    assert_eq!(value["warnings"].as_array().unwrap().len(), 0, "{value}");
    assert!(!share::load(&home).unwrap().unwrap().on);
    let stored = hub.lock().await.stored.clone();
    let beacon = stored.iter().find(|e| e.kind == 30_200).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&beacon.content).unwrap()["status"],
        "offline"
    );

    // A computer that never ran `up` has nothing to stop.
    let empty = tempfile::tempdir().unwrap();
    let (value, text) = online::down_at(empty.path(), Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(value["was_on"], false);
    assert!(text.contains("not a pylon"));
}

#[test]
fn status_reads_a_fixture_pylon() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let settings = up_settings(home, "ws://127.0.0.1:9");
    let at = 1_800_000_000;
    let live = Live {
        schema: online::LIVE_SCHEMA.into(),
        at: at - 3,
        supervisor: "host".into(),
        pid: 1,
        state: Phase::Paused,
        why: Some("owner busy: this computer's own work comes first".into()),
        pylon: Some("test-pylon".into()),
        npub: None,
        model: Some("clef-flash@sha256:test".into()),
        lanes: vec![
            LaneLive {
                lane: LaneKind::Decisions,
                port: 18_096,
                pid: Some(42),
                up: true,
                restarts: 2,
                last_exit: Some("exited (signal: 9)".into()),
            },
            LaneLive {
                lane: LaneKind::Text,
                port: 18_097,
                pid: Some(43),
                up: true,
                restarts: 0,
                last_exit: None,
            },
        ],
        day: at / 86_400,
        jobs_today: 7,
        refused_today: 1,
        failed_today: 0,
        serving_since: Some(at - 600),
    };
    let check = Check {
        id: "label".into(),
        checker: "a".repeat(64),
        verdict: Verdict::Pass,
        receipt: "receipt".into(),
        provider: "b".repeat(64),
        result_digest: "c".repeat(64),
        method: "decide recompute".into(),
        created_at: at - 120,
    };
    let (value, text) = online::report(
        home,
        "host",
        Some(&settings),
        Some(&live),
        Ok(vec![check]),
        at,
    )
    .unwrap();
    assert_eq!(value["state"], "paused");
    assert_eq!(value["jobs_today"], 7);
    assert_eq!(value["refused_today"], 1);
    assert_eq!(value["model"], "clef-flash@sha256:test");
    assert_eq!(value["device"], "cpu");
    assert_eq!(value["engines"].as_array().unwrap().len(), 2);
    assert_eq!(value["checks"][0]["verdict"], "pass");
    assert_eq!(value["owner"], Value::Null);
    assert!(text.starts_with("paused: owner busy"), "{text}");
    assert!(text.contains("7 today (1 refused, 0 failed)"), "{text}");
    assert!(text.contains("restarted 2 times"), "{text}");
    assert!(text.contains("checks   pass 2 min ago"), "{text}");
    assert!(text.contains("not linked"), "{text}");

    // No supervisor wrote lately: on, but not running.
    let (value, text) = online::report(
        home,
        "host",
        Some(&settings),
        Some(&live),
        Ok(vec![]),
        at + 600,
    )
    .unwrap();
    assert_eq!(value["state"], "paused");
    assert!(text.contains("not running"), "{text}");
    // Yesterday's counts are not today's.
    assert_eq!(
        online::report(
            home,
            "host",
            Some(&settings),
            Some(&live),
            Ok(vec![]),
            at + 86_400
        )
        .unwrap()
        .0["jobs_today"],
        0
    );

    // Turned off.
    let mut off = settings.clone();
    off.on = false;
    let (value, text) = online::report(
        home,
        "host",
        Some(&off),
        None,
        Err("the relay did not answer in time".into()),
        at,
    )
    .unwrap();
    assert_eq!(value["state"], "off");
    assert!(text.contains("checks   could not be read"), "{text}");
}

#[test]
fn up_choices_and_old_settings() {
    // `--text off`, a custom port, and a held allowlist that stays.
    let choice = Up::parse(&[
        "--text".into(),
        "off".into(),
        "--port".into(),
        "18200".into(),
        "--device".into(),
        "metal".into(),
        "--allow".into(),
        "a".repeat(64),
        "--yes".into(),
    ])
    .unwrap();
    assert!(choice.yes && !choice.text);
    assert_eq!(choice.device.as_deref(), Some("metal"));
    let (settings, notes) =
        online::settings_for_up(None, "box", "wss://r", &choice, "metal", Ok(())).unwrap();
    assert_eq!(settings.decide.as_ref().unwrap().port, 18_200);
    assert!(settings.text.is_none());
    assert_eq!(notes, vec!["text answers are off (--text off)".to_string()]);
    assert_eq!(settings.allow.as_ref().unwrap().len(), 1);
    assert_eq!(settings.pylon, "box");
    assert!(Up::parse(&["--device".into(), "rocm".into()]).is_err());
    let slow = Up::parse(&["--allow-slow".into()]).unwrap();
    assert!(slow.allow_slow && slow.serve.is_empty());
    assert!(Up::parse(&["--text".into(), "maybe".into()]).is_err());

    // No room for text: decisions only, with the reason.
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("t.gguf");
    std::fs::write(&file, b"gguf").unwrap();
    let choice = Up {
        text: true,
        text_model: Some(file),
        ..Up::default()
    };
    let (settings, notes) = online::settings_for_up(
        None,
        "box",
        "wss://r",
        &choice,
        "cpu",
        Err("no room".into()),
    )
    .unwrap();
    assert!(settings.text.is_none());
    assert_eq!(notes, vec!["no room".to_string()]);

    // With room, the text engine runs on the decision lane's device:
    // Psionic's text backend would otherwise default to the CPU.
    let (settings, _) =
        online::settings_for_up(None, "box", "wss://r", &choice, "cuda", Ok(())).unwrap();
    let specs = online::lanes(&settings);
    let text = specs.iter().find(|l| l.kind == LaneKind::Text).unwrap();
    assert_eq!(text.device.as_deref(), Some("cuda"));
    let command = online::psionic_command(Path::new("psionic-openai-server"), text);
    let args: Vec<String> = command
        .as_std()
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    assert!(
        args.windows(2).any(|w| w == ["--backend", "cuda"]),
        "{args:?}"
    );
    let host = pylon::setup::Host {
        gpu: "apple".into(),
        memory_gb: Some(16.0),
        ..pylon::setup::Host::default()
    };
    assert!(online::text_fits(&host).is_err());

    // A version 1 settings file (no lanes) still loads, and saves as v2.
    let v1 = serde_json::json!({
        "schema": "openagents.pylon.share.v1", "on": true, "changed_at": 1,
        "relay": "wss://r", "engine": "http://127.0.0.1:18080", "model": "m",
        "pylon": "p", "label": "p", "slots": 2, "allow": null, "rate_per_minute": 10,
        "max_tokens": 512, "vram_gb": 16, "pool": "everglade",
    });
    std::fs::write(dir.path().join("share.json"), v1.to_string()).unwrap();
    let held = share::load(dir.path()).unwrap().unwrap();
    assert!(held.on && held.decide.is_none() && held.text.is_none() && !held.dedicated);
    share::save(dir.path(), &held).unwrap();
    let saved: Value =
        serde_json::from_slice(&std::fs::read(dir.path().join("share.json")).unwrap()).unwrap();
    assert_eq!(saved["schema"], share::SCHEMA);
}

#[test]
fn id_names_the_pylon() {
    let dir = tempfile::tempdir().unwrap();
    let (value, text) = online::id_at(dir.path(), "studio").unwrap();
    let pubkey = value["pubkey"].as_str().unwrap();
    assert_eq!(value["address"], format!("30200:{pubkey}:studio"));
    assert_eq!(value["npub"], value["provider"]);
    assert_eq!(value["owner"], Value::Null);
    assert_eq!(value["relays"][0], "wss://relay.openagents.com");
    assert!(text.contains("not linked"), "{text}");
    assert!(value["buyer"].as_str().unwrap().starts_with("npub1"));
}
