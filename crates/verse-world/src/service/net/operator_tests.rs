//! Exercises live diagnostics through scratch TLS and the ordered storage writer.
use super::*;
use crate::service::{
    client::Client,
    operator::{Build, Monitor, Phase, Reason},
};
use std::sync::{
    Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};
pub(super) fn monitor() -> Monitor {
    Monitor::new(
        Build {
            package_version: "test".into(),
            source_revision: "scratch-operations".into(),
            wire_version: VERSION,
        },
        120,
        Some([8; 32]),
    )
    .unwrap()
}
#[tokio::test]
async fn live_operator_attributes_stalls_and_drains_before_releasing_writer() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("state");
    let mut store = Store::open(&root, [8; 32], 120).unwrap();
    let armed = Arc::new(AtomicBool::new(false));
    let blocked = Arc::new(AtomicBool::new(false));
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let (arm, waiting, release) = (armed.clone(), blocked.clone(), gate.clone());
    store.inject(Arc::new(move |stage| {
        if stage == "before_encode" && arm.swap(false, Ordering::AcqRel) {
            waiting.store(true, Ordering::Release);
            let (lock, wake) = &*release;
            let _ = wake
                .wait_timeout_while(lock.lock().unwrap(), Duration::from_secs(5), |released| {
                    !*released
                })
                .unwrap();
        }
    }));
    let keys = [tests::key(231), tests::key(232), tests::key(233)];
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (tls, connector) = tests::tls();
    let monitor = monitor();
    let mut expensive = false;
    let hook: Tick = Box::new(move |_, _| {
        if !expensive {
            expensive = true;
            std::thread::sleep(Duration::from_millis(45));
        }
    });
    let host = tokio::spawn(serve_monitored(
        listener,
        tls,
        tests::gateway(&keys).with_content([8; 32]).unwrap(),
        Some(store),
        Some(hook),
        monitor.clone(),
        std::future::pending(),
    ));
    let mut client = Client::connect_with_content(
        address,
        rustls::pki_types::ServerName::try_from("localhost").unwrap(),
        connector.config().clone(),
        120,
        Some([8; 32]),
        &keys[0],
    )
    .await
    .unwrap();
    let acknowledged = client.request(Body::Snapshot {}).await.unwrap();
    // A pending handshake and an authenticated client that stops polling remain distinct.
    let slow = TcpStream::connect(address).await.unwrap();
    armed.store(true, Ordering::Release);
    let observed = timeout(Duration::from_secs(4), async {
        loop {
            let snapshot = monitor.snapshot();
            if snapshot.reasons.contains(&Reason::WriterStalled) {
                break snapshot;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert!(blocked.load(Ordering::Acquire));
    assert!(observed.live && !observed.ready);
    assert_eq!(observed.metrics.writer_queue, 2);
    assert!(observed.metrics.pending_commit_age_ms.unwrap() >= 1000);
    assert!(observed.metrics.simulation.maximum_ms >= 45.);
    assert!(observed.reasons.contains(&Reason::SimulationOverBudget));
    assert_eq!(observed.metrics.admission.pending, 1);
    let player = observed.clients.iter().find(|c| c.authenticated).unwrap();
    assert!(player.received_payload_bytes > 0 && player.sent_payload_bytes > 0);
    assert!(player.last_delivery_ms_ago.unwrap() >= 500);
    assert_eq!(player.delivered_tick, Some(acknowledged.tick));
    let encoded = serde_json::to_string(&observed).unwrap();
    let public = keys[0]
        .x_only_public_key()
        .0
        .serialize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    assert!(
        !encoded.contains(&public)
            && !encoded.contains("127.0.0.1")
            && !encoded.contains("signature")
    );
    monitor.request_drain();
    timeout(Duration::from_secs(1), async {
        while monitor.snapshot().phase != Phase::Draining {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(!host.is_finished());
    assert!(Store::open(&root, [8; 32], 120).is_err());
    *gate.0.lock().unwrap() = true;
    gate.1.notify_all();
    let exit = timeout(Duration::from_secs(3), host)
        .await
        .unwrap()
        .unwrap();
    assert!(exit.failure.is_none(), "{:?}", exit.failure);
    let stopped = monitor.snapshot();
    assert_eq!(stopped.phase, Phase::Stopped);
    assert!(!stopped.live && !stopped.ready);
    assert!(stopped.metrics.durable_revision > 0);
    let mut recovered = Store::open(&root, [8; 32], 120).unwrap();
    assert_eq!(
        recovered.recover().unwrap().game().authority_tick,
        exit.gateway.game().authority_tick
    );
    eprintln!(
        "{}",
        serde_json::json!({"schema":"verse.operations.live.v1", "stalled":observed, "final":stopped, "drain_recovered_tick":exit.gateway.game().authority_tick})
    );
    drop(slow);
}
#[test]
fn client_work_exhaustion_is_visible_without_credentials() {
    let limits = admission::Limits::new();
    let mut slot = limits.open("127.0.0.1".parse().unwrap()).unwrap();
    slot.authenticate([77; 32]).unwrap();
    for _ in 0..16 {
        assert!(slot.request(&Body::Snapshot {}));
    }
    assert!(!slot.request(&Body::Snapshot {}));
    let stats = limits.stats();
    assert_eq!(stats.principal_work_refusals, 1);
    let clients = limits.clients(120);
    assert_eq!(clients.len(), 1);
    assert_eq!(clients[0].work_refusals, 1);
    let m = monitor();
    let mut snapshot = Stats::default();
    snapshot.admission = stats;
    let g = tests::gateway(&[tests::key(231), tests::key(232), tests::key(233)])
        .with_content([8; 32])
        .unwrap();
    m.running(&g, &snapshot, Instant::now(), 0, 0, 0, None, true, clients);
    assert!(m.snapshot().reasons.contains(&Reason::WorkBudget));
    drop(slot);
    assert!(limits.clients(120).is_empty());
}
