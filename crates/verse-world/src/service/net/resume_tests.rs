//! Real TLS requests queued while the durable writer blocks must precede resumed time.
use super::*;
use crate::{
    movement::frames::{Frame, MAX_LAG, MAX_STEPS, Segment},
    service::client::Client,
};
use std::sync::{
    Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};

struct Release(Arc<(Mutex<bool>, Condvar)>);
impl Drop for Release {
    fn drop(&mut self) {
        *self.0.0.lock().unwrap() = true;
        self.0.1.notify_all();
    }
}

#[tokio::test]
async fn queued_intervals_survive_storage_resume_without_expiry() {
    for attempt in 0..16 {
        let directory = tempfile::tempdir().unwrap();
        let keys = [tests::key(111), tests::key(112), tests::key(113)];
        let mut store = Store::open(&directory.path().join("state"), [8; 32], 120).unwrap();
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let release = Release(gate.clone());
        let armed = Arc::new(AtomicBool::new(false));
        let trigger = armed.clone();
        let (started, mut blocked) = mpsc::unbounded_channel();
        store.inject(Arc::new(move |stage| {
            if stage == "before_encode" && trigger.swap(false, Ordering::AcqRel) {
                started.send(()).unwrap();
                let mut open = gate.0.lock().unwrap();
                while !*open {
                    open = gate.1.wait(open).unwrap();
                }
            }
        }));
        let mut has_armed = false;
        let tick: Tick = Box::new(move |gateway, _| {
            let game = gateway.game();
            if let Some(baseline) = game.movement_baseline(game.player_life()).unwrap() {
                if !has_armed
                    && baseline.profile == crate::movement::Profile::Frames
                    && baseline.applied_sequence > 0
                    && baseline.world_step.saturating_sub(baseline.physics_step) >= MAX_LAG - 4
                {
                    has_armed = true;
                    armed.store(true, Ordering::Release);
                }
            }
        });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (tls, connector) = tests::tls();
        let (stop, stopped) = oneshot::channel();
        let server = tokio::spawn(serve_ticked(
            listener,
            tls,
            tests::gateway(&keys).with_content([8; 32]).unwrap(),
            Some(store),
            tick,
            async {
                let _ = stopped.await;
            },
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
        let entry = client.begin_movement_frames().await.unwrap();
        let Reply::Snapshot { state } = entry.body else {
            panic!("Missing interval baseline: {:?}", entry.body);
        };
        let baseline = state.movement.unwrap();
        let frame = |start, steps| Frame {
            life: baseline.life,
            epoch: baseline.epoch,
            sequence: 0,
            tick: 0,
            start,
            steps,
            segments: vec![Segment {
                offset: 0,
                axes: [0.; 2],
                yaw: 0.,
                until: start + crate::movement::HELD_STEPS,
                jump: false,
            }],
        };
        assert!(matches!(
            client
                .movement_frame(frame(baseline.physics_step, 4))
                .await
                .unwrap()
                .body,
            Reply::Accepted
        ));
        timeout(Duration::from_secs(2), blocked.recv())
            .await
            .unwrap()
            .unwrap();
        // Let the second persistence slot fill while the world clock is paused.
        tokio::time::sleep(Duration::from_millis(80)).await;
        let mut client = client.pipeline().unwrap();
        let frame = client
            .prepare_movement_frame(frame(baseline.physics_step + 4, MAX_STEPS))
            .unwrap();
        client.send(Body::MovementFrame { frame }).unwrap();
        // Transport tasks enqueue the request before the writer is released.
        tokio::time::sleep(Duration::from_millis(80)).await;
        drop(release);
        // Both storage completion and a simulation deadline become ready. This
        // exercises valid runtime schedules without changing the authority bounds.
        std::thread::sleep(Duration::from_millis(90));
        let (body, reply) = timeout(Duration::from_secs(3), client.receive())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(body, Body::MovementFrame { .. }));

        let _ = stop.send(());
        let exit = timeout(Duration::from_secs(3), server)
            .await
            .unwrap()
            .unwrap();
        assert!(exit.failure.is_none(), "{:?}", exit.failure);
        assert!(
            matches!(reply.body, Reply::Accepted),
            "Attempt {attempt}: {:?}; expiries {:?}; paused ticks {}",
            reply.body,
            exit.gateway.game().movement_expiry,
            exit.stats.storage_paused_ticks
        );
        assert_eq!(reply.control.as_ref().unwrap().epoch, baseline.epoch);
        assert_eq!(exit.stats.writer_queue_peak, 2);
        assert!(exit.stats.request_queue_peak > 0);
        assert_eq!(
            exit.gateway.game().movement_expiry.total,
            0,
            "Attempt {attempt}: resumed time expired queued movement"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queued_intervals_precede_delayed_simulation_catchup() {
    for attempt in 0..16 {
        let keys = [tests::key(114), tests::key(115), tests::key(116)];
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let release = Release(gate.clone());
        let (started, mut blocked) = mpsc::unbounded_channel();
        let mut has_blocked = false;
        let tick: Tick = Box::new(move |gateway, _| {
            let game = gateway.game();
            if let Some(baseline) = game.movement_baseline(game.player_life()).unwrap() {
                if !has_blocked
                    && baseline.profile == crate::movement::Profile::Frames
                    && baseline.applied_sequence > 0
                    && baseline.world_step.saturating_sub(baseline.physics_step) >= MAX_LAG - 8
                {
                    has_blocked = true;
                    started.send(()).unwrap();
                    let mut open = gate.0.lock().unwrap();
                    while !*open {
                        open = gate.1.wait(open).unwrap();
                    }
                }
            }
        });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (tls, connector) = tests::tls();
        let (stop, stopped) = oneshot::channel();
        let server = tokio::spawn(serve_ticked(
            listener,
            tls,
            tests::gateway(&keys),
            None,
            tick,
            async {
                let _ = stopped.await;
            },
        ));
        let mut client = Client::connect(
            address,
            rustls::pki_types::ServerName::try_from("localhost").unwrap(),
            connector.config().clone(),
            120,
            &keys[0],
        )
        .await
        .unwrap();
        timeout(Duration::from_secs(2), async {
            loop {
                let snapshot = client.snapshot().await.unwrap();
                if snapshot
                    .movement
                    .is_some_and(|movement| movement.character.support.is_some())
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(33)).await;
            }
        })
        .await
        .unwrap();
        let entry = client.begin_movement_frames().await.unwrap();
        let Reply::Snapshot { state } = entry.body else {
            panic!("Missing interval baseline: {:?}", entry.body);
        };
        let baseline = state.movement.unwrap();
        let frame = |start, steps| Frame {
            life: baseline.life,
            epoch: baseline.epoch,
            sequence: 0,
            tick: 0,
            start,
            steps,
            segments: vec![Segment {
                offset: 0,
                axes: [0.; 2],
                yaw: 0.,
                until: start + crate::movement::HELD_STEPS,
                jump: false,
            }],
        };
        assert!(matches!(
            client
                .movement_frame(frame(baseline.physics_step, 4))
                .await
                .unwrap()
                .body,
            Reply::Accepted
        ));
        timeout(Duration::from_secs(2), blocked.recv())
            .await
            .unwrap()
            .unwrap();
        let mut client = client.pipeline().unwrap();
        let frame = client
            .prepare_movement_frame(frame(baseline.physics_step + 4, MAX_STEPS))
            .unwrap();
        client.send(Body::MovementFrame { frame }).unwrap();
        // Other runtime tasks deliver this TLS interval while the authority is
        // delayed. Three overdue simulation ticks must not overtake admission.
        tokio::time::sleep(Duration::from_millis(110)).await;
        drop(release);
        let (_, reply) = timeout(Duration::from_secs(2), client.receive())
            .await
            .unwrap()
            .unwrap();
        let _ = stop.send(());
        let exit = timeout(Duration::from_secs(2), server)
            .await
            .unwrap()
            .unwrap();
        assert!(exit.failure.is_none(), "{:?}", exit.failure);
        assert!(
            matches!(reply.body, Reply::Accepted),
            "Attempt {attempt}: {:?}; expiries {:?}",
            reply.body,
            exit.gateway.game().movement_expiry
        );
        assert_eq!(reply.control.as_ref().unwrap().epoch, baseline.epoch);
        assert!(exit.stats.request_queue_peak > 0);
        assert_eq!(
            exit.gateway.game().movement_expiry.total,
            0,
            "Attempt {attempt}: catch-up expired an already queued interval"
        );
    }
}
