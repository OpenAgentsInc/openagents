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

/// Fixed 120 Hz movement steps simulated by each 30 Hz authority tick.
const TICK_STEPS: u64 = 4;

#[tokio::test]
async fn queued_intervals_survive_storage_resume_without_expiry() {
    let mut valid = 0;
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
        let sent = Arc::new(AtomicBool::new(false));
        let premature = Arc::new(AtomicBool::new(false));
        // The first interval's reply waits for a checkpoint; never block that one.
        let ready = Arc::new(AtomicBool::new(false));
        let (sent_seen, premature_seen, ready_seen) =
            (sent.clone(), premature.clone(), ready.clone());
        let tick: Tick = Box::new(move |gateway, _| {
            let game = gateway.game();
            // A clock that lapsed before the queued interval was even sent is a
            // missed fixture precondition, not a resume ordering failure.
            if game.movement_expiry.total > 0 && !sent_seen.load(Ordering::Acquire) {
                premature_seen.store(true, Ordering::Release);
            }
            if let Some(baseline) = game.movement_baseline(game.player_life()).unwrap() {
                // The world pauses only once both persistence slots are full: the
                // blocked checkpoint leaves at this tick or the next, and the
                // second follows within two more ticks. Arming three ticks before
                // the bound pauses at a lag of 24..=32 steps, still live and within
                // one catch-up batch of expiry. Arming one tick before the bound
                // raced the writer thread's dequeue and often expired the clock
                // before the queued interval was sent.
                if !has_armed
                    && ready_seen.load(Ordering::Acquire)
                    && baseline.profile == crate::movement::Profile::Frames
                    && baseline.applied_sequence > 0
                    && baseline.world_step.saturating_sub(baseline.physics_step)
                        >= MAX_LAG - 3 * TICK_STEPS
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
        ready.store(true, Ordering::Release);
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
        sent.store(true, Ordering::Release);
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
        if premature.load(Ordering::Acquire) {
            // Only an overloaded host batches enough ticks to pass the bound
            // before the second slot fills; nothing was queued to protect yet.
            continue;
        }
        valid += 1;
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
            "Attempt {attempt}: resumed time expired queued movement {:?}",
            exit.gateway.game().movement_expiry
        );
    }
    assert!(
        valid >= 12,
        "Only {valid} attempts paused before the movement bound"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queued_intervals_precede_simulation_deadlines() {
    for (attempt, delay_ms) in [40u64, 110].into_iter().cycle().take(32).enumerate() {
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
                    && baseline.world_step.saturating_sub(baseline.physics_step)
                        >= if delay_ms == 40 { MAX_LAG } else { MAX_LAG - 8 }
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
        // delayed. Neither an ordinary deadline nor a catch-up batch may overtake admission.
        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
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

/// Handing a checkpoint to the writer fills its one-slot channel until the
/// writer thread dequeues it. That transient full slot must not hold queued
/// movement behind the next simulation deadline: the loop wakes when the slot
/// frees and the deadline re-reads current admission room.
#[tokio::test]
async fn movement_queued_after_a_checkpoint_handoff_precedes_the_next_step() {
    let mut valid = 0;
    for attempt in 0..8 {
        let directory = tempfile::tempdir().unwrap();
        let keys = [tests::key(117), tests::key(118), tests::key(119)];
        let mut store = Store::open(&directory.path().join("state"), [8; 32], 120).unwrap();
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let release = Release(gate.clone());
        let armed = Arc::new(AtomicBool::new(false));
        let trigger = armed.clone();
        let (started, mut blocked) = mpsc::unbounded_channel();
        // Block after the writer thread has dequeued the checkpoint, leaving
        // one checkpoint in flight and the channel slot free again.
        store.inject(Arc::new(move |stage| {
            if stage == "before_encode" && trigger.swap(false, Ordering::AcqRel) {
                started.send(()).unwrap();
                let mut open = gate.0.lock().unwrap();
                while !*open {
                    open = gate.1.wait(open).unwrap();
                }
            }
        }));
        let world = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let sent = Arc::new(AtomicBool::new(false));
        let premature = Arc::new(AtomicBool::new(false));
        let (observed, sent_seen, premature_seen) =
            (world.clone(), sent.clone(), premature.clone());
        let tick: Tick = Box::new(move |gateway, _| {
            let game = gateway.game();
            if game.movement_expiry.total > 0 && !sent_seen.load(Ordering::Acquire) {
                premature_seen.store(true, Ordering::Release);
            }
            observed.store(game.physics_steps, Ordering::Release);
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
                .movement_frame(frame(baseline.physics_step, MAX_STEPS))
                .await
                .unwrap()
                .body,
            Reply::Accepted
        ));
        armed.store(true, Ordering::Release);
        timeout(Duration::from_secs(2), blocked.recv())
            .await
            .unwrap()
            .unwrap();
        // The checkpoint was captured after this step and nothing has
        // simulated since; the next deadline is a full period away.
        let handoff = world.load(Ordering::Acquire);
        let mut client = client.pipeline().unwrap();
        let next = client
            .prepare_movement_frame(frame(baseline.physics_step + u64::from(MAX_STEPS), 4))
            .unwrap();
        sent.store(true, Ordering::Release);
        client.send(Body::MovementFrame { frame: next }).unwrap();
        // Keep the writer busy across later deadlines so its completion
        // cannot be what admits the queued interval.
        tokio::time::sleep(Duration::from_millis(100)).await;
        drop(release);
        let (_, reply) = timeout(Duration::from_secs(3), client.receive())
            .await
            .unwrap()
            .unwrap();
        let _ = stop.send(());
        let exit = timeout(Duration::from_secs(3), server)
            .await
            .unwrap()
            .unwrap();
        assert!(exit.failure.is_none(), "{:?}", exit.failure);
        if premature.load(Ordering::Acquire) {
            // An overloaded host let the clock lapse before the interval existed.
            continue;
        }
        valid += 1;
        assert!(
            matches!(reply.body, Reply::Accepted),
            "Attempt {attempt}: {:?}",
            reply.body
        );
        let admitted = reply.control.as_ref().unwrap().world_step;
        assert_eq!(
            admitted, handoff,
            "Attempt {attempt}: interval queued at world step {handoff} waited behind simulation to {admitted}"
        );
    }
    assert!(valid >= 6, "Only {valid} attempts queued a live interval");
}
