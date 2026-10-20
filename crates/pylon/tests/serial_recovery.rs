#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The real supervisor and HTTP engine, with private relay/model fixtures.

use pylon::{
    client::{self, Ask},
    engine::Sampling,
    identity::Identity,
    online::{self, Live, Options, Phase},
    share::{self, Settings, TextEngine, TextLane},
};
use std::{
    collections::BTreeSet,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

async fn until(mut check: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !check() {
        assert!(
            Instant::now() < deadline,
            "recovery did not reach the expected state"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn events(home: &Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(home.join("events.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

struct Rig {
    _occupied: Option<std::net::TcpListener>,
    home: tempfile::TempDir,
    settings: Settings,
    buyer: Identity,
    stop: tokio::sync::oneshot::Sender<()>,
    task: tokio::task::JoinHandle<()>,
}

impl Rig {
    async fn start(flags: &[&str]) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let home = tempfile::tempdir().unwrap();
        let file = home.path().join("fixture.gguf");
        let script = home.path().join("worker.py");
        std::fs::write(&file, b"fixture").unwrap();
        std::fs::write(&script, include_bytes!("fixtures/serial_worker.py")).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        for flag in flags {
            std::fs::write(home.path().join(flag), b"1").unwrap();
        }
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let occupied = flags.contains(&"occupied-port").then_some(listener);
        let (url, _) = pylon::fixture::relay().await.unwrap();
        let buyer = Identity::generate();
        let mut settings = Settings::off("serial-fixture");
        settings.on = true;
        settings.relay = url;
        settings.slots = 1;
        settings.allow = Some(BTreeSet::from([buyer.pubkey().to_string()]));
        settings.rate_per_minute = 600;
        settings.max_tokens = 32;
        settings.dedicated = true;
        settings.text = Some(TextLane {
            profile: None,
            serial: Some(pylon::serial::Config {
                server: script,
                generation_seconds: 1,
                startup_seconds: 1,
                stop_seconds: 1,
                max_restarts: 2,
                sampling: Sampling::new(0.6, 0.95, 1).unwrap(),
            }),
            port,
            model: "fixture".into(),
            model_file: file,
            device: Some("cpu".into()),
            engine: TextEngine::LlamaCpp,
            chat_template: None,
            stop: vec![],
            metadata_overrides: vec![],
            gpu_layers: None,
        });
        share::save(home.path(), &settings).unwrap();
        let stored: serde_json::Value =
            serde_json::from_slice(&std::fs::read(share::path(home.path())).unwrap()).unwrap();
        assert_eq!(stored["schema"], share::SERIAL_SCHEMA);
        let mut downgraded = stored;
        downgraded["schema"] = share::SCHEMA.into();
        std::fs::write(
            share::path(home.path()),
            serde_json::to_vec(&downgraded).unwrap(),
        )
        .unwrap();
        assert!(share::load(home.path()).is_err());
        share::save(home.path(), &settings).unwrap();
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let mut options = Options::host(Duration::from_millis(20));
        options.backoff = Duration::from_millis(20);
        options.max_backoff = Duration::from_millis(80);
        let task = tokio::spawn(online::supervise(
            home.path().to_path_buf(),
            Arc::new(pylon::lease::Dedicated),
            options,
            Arc::new(|s| eprintln!("serial fixture: {s}")),
            async {
                let _ = stopped.await;
            },
        ));
        Self {
            home,
            settings,
            buyer,
            stop,
            task,
            _occupied: occupied,
        }
    }

    fn live(&self) -> Live {
        online::load_live(self.home.path()).unwrap()
    }

    async fn ready(&self, old: Option<u32>) -> u32 {
        until(|| {
            online::load_live(self.home.path()).is_some_and(|l| {
                l.state == Phase::Serving
                    && l.lanes
                        .first()
                        .is_some_and(|w| w.up && w.pid.is_some() && w.pid != old)
            })
        })
        .await;
        self.live().lanes[0].pid.unwrap()
    }

    fn ask(&self) -> Ask {
        Ask {
            relay: self.settings.relay.clone(),
            pylon: None,
            prompt: "1+1?".into(),
            wait: Duration::from_secs(5),
            publish_receipt: false,
            home: self.home.path().into(),
            checkers: BTreeSet::new(),
            pay: None,
        }
    }

    async fn answer(&self) {
        // The provider publishes readiness changes immediately. Discovery
        // can briefly race the transition, so poll before sending a job.
        until(|| self.live().state == Phase::Serving).await;
        tokio::time::sleep(Duration::from_millis(250)).await;
        let answer = client::ask(&self.buyer, &self.ask()).await.unwrap();
        assert_eq!(answer.text.as_deref(), Some("\n\n2\n"));
    }

    async fn close(self) {
        self.stop.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(8), self.task)
            .await
            .unwrap()
            .unwrap();
        assert!(
            !events(self.home.path())
                .iter()
                .any(|e| e["event"] == "OVERLAP")
        );
        assert!(
            online::load_live(self.home.path())
                .unwrap()
                .lanes
                .is_empty()
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_crashed_worker_is_reaped_before_automatic_restart() {
    let rig = Rig::start(&[]).await;
    let old = rig.ready(None).await;
    rig.answer().await;
    unsafe {
        libc::kill(old as i32, libc::SIGKILL);
    }
    let new = rig.ready(Some(old)).await;
    assert_ne!(new, old);
    assert_eq!(unsafe { libc::kill(old as i32, 0) }, -1);
    rig.answer().await;
    assert!(rig.live().lanes[0].restarts >= 1);
    rig.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn healthy_http_cannot_hide_a_stuck_generation_or_keep_its_descendant_alive() {
    let rig = Rig::start(&["hang", "fork"]).await;
    let old = rig.ready(None).await;
    let ask = rig.ask();
    let buyer = rig.buyer.clone();
    let mut pending = tokio::spawn(async move { client::ask(&buyer, &ask).await });
    // Poll the buyer concurrently with the model's observable start.
    tokio::select! {
        _ = until(|| events(rig.home.path()).iter().any(|e| e["event"] == "generation")) => {}
        result = &mut pending => panic!("job finished before hanging: {result:?}"),
    }
    std::fs::remove_file(rig.home.path().join("hang")).unwrap();
    std::fs::remove_file(rig.home.path().join("fork")).unwrap();
    let answer = pending.await.unwrap().unwrap();
    assert!(answer.text.is_none());
    let _ = rig.ready(Some(old)).await;
    assert_eq!(unsafe { libc::kill(old as i32, 0) }, -1);
    let descendant = events(rig.home.path())
        .into_iter()
        .find(|e| e["event"] == "descendant")
        .unwrap();
    assert_eq!(
        unsafe { libc::kill(descendant["child"].as_u64().unwrap() as i32, 0) },
        -1
    );
    rig.answer().await;
    let requests = events(rig.home.path());
    assert_eq!(
        requests
            .iter()
            .filter(|e| e["event"] == "generation")
            .count(),
        2,
        "the interrupted job must not be retried"
    );
    rig.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn failed_readiness_exhausts_its_budget_without_accepting_work() {
    let rig = Rig::start(&["never-ready"]).await;
    until(|| {
        online::load_live(rig.home.path()).is_some_and(|l| {
            l.state == Phase::Paused
                && l.why
                    .as_deref()
                    .is_some_and(|s| s.contains("repeated failures"))
        })
    })
    .await;
    assert!(rig.live().lanes[0].pid.is_none());
    assert!(!rig.live().lanes[0].up);
    let starts = events(rig.home.path()).len();
    assert_eq!(starts, 3);
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(events(rig.home.path()).len(), starts);
    assert!(client::ask(&rig.buyer, &rig.ask()).await.is_err());
    rig.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_lost_response_restarts_instead_of_reusing_an_uncertain_worker() {
    let rig = Rig::start(&["drop-response"]).await;
    let old = rig.ready(None).await;
    let answer = client::ask(&rig.buyer, &rig.ask()).await.unwrap();
    assert!(answer.text.is_none());
    std::fs::remove_file(rig.home.path().join("drop-response")).unwrap();
    rig.ready(Some(old)).await;
    rig.answer().await;
    rig.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn shutdown_kills_a_busy_owned_worker_without_waiting_for_the_provider_deadline() {
    let rig = Rig::start(&["hang"]).await;
    let old = rig.ready(None).await;
    let home = rig.home.path().to_path_buf();
    let buyer = rig.buyer.clone();
    let ask = rig.ask();
    let pending = tokio::spawn(async move { client::ask(&buyer, &ask).await });
    until(|| events(&home).iter().any(|e| e["event"] == "generation")).await;
    rig.close().await;
    assert_eq!(unsafe { libc::kill(old as i32, 0) }, -1);
    let _ = pending.await;
}

#[test]
fn persisted_sampling_cannot_bypass_validation() {
    assert!(
        serde_json::from_str::<Sampling>(r#"{"temperature":-1,"top_p":0.95,"seed":1}"#).is_err()
    );
    assert!(serde_json::from_str::<Sampling>(r#"{"temperature":0.6,"top_p":0,"seed":1}"#).is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_occupied_port_is_not_adopted_or_killed() {
    let rig = Rig::start(&["occupied-port"]).await;
    until(|| {
        online::load_live(rig.home.path()).is_some_and(|l| {
            l.state == Phase::Paused
                && l.why
                    .as_deref()
                    .is_some_and(|s| s.contains("already in use"))
        })
    })
    .await;
    assert!(events(rig.home.path()).is_empty());
    assert!(
        std::net::TcpStream::connect(("127.0.0.1", rig.settings.text.as_ref().unwrap().port))
            .is_ok()
    );
    rig.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_complete_truncation_response_does_not_restart_the_worker() {
    let rig = Rig::start(&["truncate"]).await;
    let pid = rig.ready(None).await;
    let answer = client::ask(&rig.buyer, &rig.ask()).await.unwrap();
    assert!(answer.text.is_none());
    std::fs::remove_file(rig.home.path().join("truncate")).unwrap();
    rig.answer().await;
    assert_eq!(rig.live().lanes[0].pid, Some(pid));
    assert_eq!(rig.live().lanes[0].restarts, 0);
    let request = events(rig.home.path())
        .into_iter()
        .find(|e| e["event"] == "generation")
        .unwrap();
    assert_eq!(request["request"]["temperature"], 0.6);
    assert_eq!(request["request"]["top_p"], 0.95);
    assert_eq!(request["request"]["seed"], 1);
    rig.close().await;
}
