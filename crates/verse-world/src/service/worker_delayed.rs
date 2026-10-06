//! Scratch TCP delay transport around the real TLS authority and native worker.
use super::*;
use crate::{movement::Profile, prediction::Local};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

async fn delay<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    mut read: R,
    mut write: W,
    latency: Duration,
) -> std::io::Result<()> {
    let (send, mut receive) = mpsc::channel::<(tokio::time::Instant, Vec<u8>)>(32);
    let reader = async move {
        let mut bytes = [0; 8192];
        loop {
            let count = read.read(&mut bytes).await?;
            if count == 0 {
                break;
            }
            send.send((
                tokio::time::Instant::now() + latency,
                bytes[..count].to_vec(),
            ))
            .await
            .map_err(|_| std::io::ErrorKind::BrokenPipe)?;
        }
        Ok::<_, std::io::Error>(())
    };
    let writer = async move {
        while let Some((due, bytes)) = receive.recv().await {
            tokio::time::sleep_until(due).await;
            write.write_all(&bytes).await?;
        }
        write.shutdown().await
    };
    let (a, b) = tokio::join!(reader, writer);
    a?;
    b
}
#[tokio::test]
async fn delayed_bootstrap_and_native_interval_stream_use_one_owned_timeline() {
    interval_stream(false, 0, 67, 100, 0, false).await;
}
#[tokio::test]
async fn durable_writer_stall_preserves_owned_interval_timeline() {
    interval_stream(true, 300, 0, 0, 0, false).await;
}
#[tokio::test]
async fn delayed_route_and_long_writer_stall_preserve_owned_interval_timeline() {
    interval_stream(true, 1200, 40, 40, 0, false).await;
}
#[tokio::test]
async fn shared_player_pressure_and_long_writer_stall_preserve_owned_interval_timeline() {
    interval_stream(true, 1200, 40, 40, 19, false).await;
}
#[tokio::test]
async fn mixed_cast_and_native_interval_stream_preserve_control() {
    interval_stream(false, 0, 67, 100, 0, true).await;
}
#[tokio::test]
async fn cast_during_shared_writer_stall_preserves_native_intervals() {
    interval_stream(true, 1200, 40, 40, 19, true).await;
}
async fn interval_stream(
    durable_stall: bool,
    stall_ms: u64,
    up_ms: u64,
    down_ms: u64,
    pressure_players: usize,
    cast_during_movement: bool,
) {
    use super::super::net::tests::{key, start};
    use rustls::pki_types::ServerName;
    use tokio::net::{TcpListener, TcpStream};
    let keys = [key(218), key(219), key(220)];
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let pressure_keys: Vec<_> = (0..pressure_players)
        .map(|index| {
            if index == 0 {
                keys[1]
            } else {
                key(30 + index as u8)
            }
        })
        .collect();
    let directory = tempfile::tempdir().unwrap();
    let armed = Arc::new(AtomicBool::new(false));
    let injected = Arc::new(AtomicBool::new(false));
    let (host_address, tls, host_stop, host) = if durable_stall {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (server_tls, connector) = super::super::net::tests::tls();
        let mut store =
            super::super::persistence::Store::open(&directory.path().join("state"), [8; 32], 120)
                .unwrap();
        let trigger = armed.clone();
        let observed = injected.clone();
        store.inject(Arc::new(move |stage| {
            if stage == "before_encode" && trigger.swap(false, Ordering::AcqRel) {
                observed.store(true, Ordering::Release);
                std::thread::sleep(Duration::from_millis(stall_ms));
            }
        }));
        let mut gateway = super::super::net::tests::gateway(&keys)
            .with_content([8; 32])
            .unwrap();
        for key in pressure_keys.iter().skip(1) {
            let spawn = gateway.game().account_spawn().unwrap();
            gateway
                .enroll_player(key.x_only_public_key().0.serialize(), spawn)
                .unwrap();
        }
        let (stop, stopping) = oneshot::channel();
        let host = tokio::spawn(super::super::net::serve_durable(
            listener,
            server_tls,
            gateway,
            store,
            async {
                let _ = stopping.await;
            },
        ));
        (address, connector, stop, host)
    } else {
        start(&keys).await
    };
    let pressure_stop = Arc::new(AtomicBool::new(false));
    let mut pressure_tasks = tokio::task::JoinSet::new();
    for key in pressure_keys {
        let mut client = Client::connect_with_content(
            host_address,
            ServerName::try_from("localhost").unwrap(),
            tls.config().clone(),
            120,
            Some([8; 32]),
            &key,
        )
        .await
        .unwrap();
        let stop = pressure_stop.clone();
        pressure_tasks.spawn(async move {
            let mut requests = 0;
            while !stop.load(Ordering::Acquire) {
                client.snapshot().await.unwrap();
                let response = client
                    .command(Intent::Move {
                        axes: [0.; 2],
                        yaw: 0.,
                    })
                    .await
                    .unwrap();
                assert!(matches!(response.body, Reply::Accepted));
                requests += 2;
                tokio::time::sleep(Duration::from_millis(33)).await;
            }
            client.close().await.unwrap();
            requests
        });
    }
    let up = Duration::from_millis(up_ms);
    let down = Duration::from_millis(down_ms);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let proxy = tokio::spawn(async move {
        let (client, _) = listener.accept().await.unwrap();
        let authority = TcpStream::connect(host_address).await.unwrap();
        client.set_nodelay(true).unwrap();
        authority.set_nodelay(true).unwrap();
        let (client_read, client_write) = client.into_split();
        let (authority_read, authority_write) = authority.into_split();
        tokio::join!(
            delay(client_read, authority_write, up),
            delay(authority_read, client_write, down)
        )
    });
    let mut client = Client::connect_with_content(
        address,
        ServerName::try_from("localhost").unwrap(),
        tls.config().clone(),
        120,
        durable_stall.then_some([8; 32]),
        &keys[0],
    )
    .await
    .unwrap();
    let entry = client.begin_movement_frames().await.unwrap();
    let Reply::Snapshot { state } = entry.body else {
        panic!("Entry must carry the initial snapshot");
    };
    let baseline = state.movement.unwrap();
    assert_eq!(baseline.profile, Profile::Frames);
    let mut local = Local::new(120);
    local
        .observe(
            baseline,
            state.collision.as_ref().unwrap(),
            entry.tick,
            entry.request_id,
        )
        .unwrap();
    local.advance(0.).unwrap();
    let (input, inputs, updates, mut output) = channels();
    let (stop, stopping) = oneshot::channel();
    let observer = Observer::default();
    let mut task = tokio::spawn(run_profiled(
        client,
        Cursor::new(120),
        NATIVE_CADENCE,
        inputs,
        updates,
        stopping,
        observer.clone(),
    ));
    let started = Instant::now();
    let mut last = started;
    let mut next_input = started;
    let mut token = 0;
    let mut next_frame = baseline.physics_step;
    let mut bound = 0;
    let mut accepted = 0;
    let mut cast_sent = false;
    let mut cast_sequence = None;
    let mut accepted_casts = 0;
    let mut corrections = Vec::new();
    let mut stall_armed = false;
    let mut recent = std::collections::VecDeque::new();
    while started.elapsed() < Duration::from_secs(if stall_ms > 300 { 4 } else { 3 }) {
        if durable_stall && !stall_armed && started.elapsed() >= Duration::from_secs(1) {
            armed.store(true, Ordering::Release);
            stall_armed = true;
        }
        while let Ok(update) = output.try_recv() {
            match update {
                Update::FrameBound { binding, .. } => {
                    local.bind_movement_frame(&binding.unwrap()).unwrap();
                    bound += 1;
                }
                Update::CommandBound { binding, .. } if cast_during_movement => {
                    let command = binding.unwrap();
                    assert!(matches!(
                        command.intent,
                        Intent::Cast {
                            ability: crate::play::Ability::Shield,
                            ..
                        }
                    ));
                    assert!(cast_sequence.replace(command.sequence).is_none());
                }
                Update::Outcome(response) => {
                    if response
                        .control
                        .as_ref()
                        .is_some_and(|c| Some(c.accepted_sequence) == cast_sequence)
                    {
                        assert!(
                            matches!(response.body, Reply::Accepted),
                            "Cast must execute during movement: {:?}",
                            response.body
                        );
                        accepted_casts += 1;
                    }
                    if let Some(control) = &response.control {
                        if local.context() == Some((control.life.into(), control.epoch))
                            && local.movement_profile() == Some(Profile::Frames)
                        {
                            local
                                .grant_world_credit(
                                    control.life.into(),
                                    control.epoch,
                                    control.credit_step,
                                )
                                .unwrap();
                        }
                    }
                    assert!(matches!(response.body, Reply::Accepted));
                    accepted += 1;
                }
                Update::Snapshot(response) => {
                    let Reply::Snapshot { state } = response.body else {
                        panic!();
                    };
                    let next = state.movement.unwrap();
                    recent.push_back(serde_json::json!({"elapsed_ms":started.elapsed().as_millis(),"world_step":next.world_step,"confirmed_step":next.physics_step,"local_step":local.physics_step(),"next_frame":next_frame,"credit_limit":local.movement_frame_limit(),"epoch":next.epoch,"bound":bound,"accepted":accepted}));
                    if recent.len() > 12 {
                        recent.pop_front();
                    }
                    if next.epoch != baseline.epoch || next.profile != Profile::Frames {
                        eprintln!(
                            "VERSE_INTERVAL_RESET {}",
                            serde_json::to_string(&recent).unwrap()
                        );
                    }
                    assert_eq!(
                        (next.life, next.epoch, next.profile),
                        (baseline.life, baseline.epoch, Profile::Frames),
                        "Interval mode expired under supported latency"
                    );
                    let before = local.pose().unwrap().position;
                    local
                        .observe(
                            next,
                            state.collision.as_ref().unwrap(),
                            response.tick,
                            response.request_id,
                        )
                        .unwrap();
                    let control = response.control.as_ref().unwrap();
                    local
                        .grant_world_credit(next.life, next.epoch, control.credit_step)
                        .unwrap();
                    local.advance(0.).unwrap();
                    corrections.push(before.distance(local.pose().unwrap().position));
                }
                Update::Inventory(_) | Update::Events { .. } => {}
                _ => panic!("Complete intervals cannot supersede local events"),
            }
        }
        if task.is_finished() {
            panic!(
                "Interval worker stopped at {:?}: {:?}",
                started.elapsed(),
                (&mut task).await.unwrap()
            );
        }
        let now = Instant::now();
        local
            .advance(now.duration_since(last).as_secs_f64().min(0.1))
            .unwrap();
        last = now;
        if now >= next_input {
            token += 1;
            local
                .queue(
                    token,
                    Intent::Move {
                        axes: if started.elapsed() < Duration::from_secs(2) {
                            [1., 0.]
                        } else {
                            [0.; 2]
                        },
                        yaw: 0.,
                    },
                )
                .unwrap();
            next_input = now + Duration::from_millis(33);
        }
        if cast_during_movement
            && !cast_sent
            && started.elapsed() >= Duration::from_millis(if durable_stall { 1500 } else { 500 })
        {
            token += 1;
            input
                .send(Input::TrackedCommand {
                    token,
                    life: baseline.life,
                    epoch: baseline.epoch,
                    intent: Intent::Cast {
                        ability: crate::play::Ability::Shield,
                        target: None,
                        aim: [0., 0., 1.],
                    },
                })
                .await
                .unwrap();
            cast_sent = true;
        }
        let steps = local
            .movement_frame_limit()
            .unwrap()
            .saturating_sub(next_frame)
            .min(u64::from(crate::movement::frames::SEND_STEPS)) as u32;
        if steps >= crate::movement::frames::SEND_STEPS && input.capacity() > 0 {
            let frame = local.movement_frame(next_frame, steps).unwrap();
            token += 1;
            input
                .send(Input::MovementFrame { token, frame })
                .await
                .unwrap();
            next_frame += u64::from(steps);
        }
        tokio::time::sleep(Duration::from_millis(8)).await;
    }
    assert!(
        bound > 30 && accepted > 30 && corrections.len() > 10,
        "bound={bound} accepted={accepted} observations={}",
        corrections.len()
    );
    if cast_during_movement {
        assert!(cast_sent && cast_sequence.is_some());
        assert_eq!(accepted_casts, 1);
    }
    corrections.sort_by(f32::total_cmp);
    let p95 = corrections[(corrections.len() as f64 * 0.95).ceil() as usize - 1];
    eprintln!(
        "VERSE_V04_TLS_EVIDENCE {}",
        serde_json::json!({"up_ms":up.as_millis(),"down_ms":down.as_millis(),"durable_stall_ms":stall_ms,"bound":bound,"accepted":accepted,"observations":corrections.len(),"p95_m":p95,"maximum_m":corrections.last().unwrap(),"mode_resets":0})
    );
    assert!(p95 < 0.1, "Actual delayed TLS correction p95: {p95}");
    let reads: Vec<_> = observer
        .drain()
        .samples
        .into_iter()
        .filter(|sample| sample.accepted_snapshot)
        .collect();
    assert!(reads.len() > 10);
    assert!(
        reads
            .iter()
            .any(|read| read.turnaround_ms > NATIVE_CADENCE.as_secs_f64() * 1000.),
        "Fixture must observe a slow snapshot response"
    );
    assert!(
        reads.windows(2).all(|pair| {
            pair[0].turnaround_ms <= NATIVE_CADENCE.as_secs_f64() * 1000.
                || pair[1].started_at.duration_since(pair[0].verified_at)
                    >= Duration::from_millis(40)
        }),
        "Slow snapshot responses must yield request capacity before the next read"
    );
    pressure_stop.store(true, Ordering::Release);
    let mut pressure_requests = 0;
    while let Some(result) = pressure_tasks.join_next().await {
        let requests = result.unwrap();
        assert!(
            requests > 10,
            "Pressure player completed only {requests} requests"
        );
        pressure_requests += requests;
    }
    eprintln!(
        "VERSE_PRESSURE_EVIDENCE {}",
        serde_json::json!({"players":pressure_players + 1,"background_requests":pressure_requests})
    );
    let _ = stop.send(());
    task.await.unwrap().unwrap();
    proxy.abort();
    let _ = proxy.await;
    let _ = host_stop.send(());
    let exit = host.await.unwrap();
    assert!(exit.failure.is_none(), "{:?}", exit.failure);
    if durable_stall {
        assert!(injected.load(Ordering::Acquire));
        assert!(exit.stats.storage_paused_ticks > 0);
        assert!(exit.stats.commits.maximum_seconds >= stall_ms as f64 / 1000.);
    }
}
