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
    use super::super::net::tests::{key, start};
    use rustls::pki_types::ServerName;
    use tokio::net::{TcpListener, TcpStream};
    let keys = [key(218), key(219), key(220)];
    let (host_address, tls, host_stop, host) = start(&keys).await;
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
            delay(client_read, authority_write, Duration::from_millis(67)),
            delay(authority_read, client_write, Duration::from_millis(100))
        )
    });
    let mut client = Client::connect(
        address,
        ServerName::try_from("localhost").unwrap(),
        tls.config().clone(),
        120,
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
    let mut task = tokio::spawn(run(
        client,
        Cursor::new(120),
        NATIVE_CADENCE,
        inputs,
        updates,
        stopping,
    ));
    let started = Instant::now();
    let mut last = started;
    let mut next_input = started;
    let mut token = 0;
    let mut next_frame = baseline.physics_step;
    let mut bound = 0;
    let mut accepted = 0;
    let mut corrections = Vec::new();
    while started.elapsed() < Duration::from_secs(3) {
        while let Ok(update) = output.try_recv() {
            match update {
                Update::FrameBound { binding, .. } => {
                    local.bind_movement_frame(&binding.unwrap()).unwrap();
                    bound += 1;
                }
                Update::Outcome(response) => {
                    assert!(matches!(response.body, Reply::Accepted));
                    accepted += 1;
                }
                Update::Snapshot(response) => {
                    let Reply::Snapshot { state } = response.body else {
                        panic!();
                    };
                    let next = state.movement.unwrap();
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
        let steps = local.physics_step().saturating_sub(next_frame).min(12) as u32;
        if steps >= 4 && input.capacity() > 0 {
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
    corrections.sort_by(f32::total_cmp);
    let p95 = corrections[(corrections.len() as f64 * 0.95).ceil() as usize - 1];
    eprintln!(
        "VERSE_V04_TLS_EVIDENCE {}",
        serde_json::json!({"up_ms":67,"down_ms":100,"bound":bound,"accepted":accepted,"observations":corrections.len(),"p95_m":p95,"maximum_m":corrections.last().unwrap(),"mode_resets":0})
    );
    assert!(p95 < 0.1, "Actual delayed TLS correction p95: {p95}");
    let _ = stop.send(());
    task.await.unwrap().unwrap();
    proxy.abort();
    let _ = proxy.await;
    let _ = host_stop.send(());
    host.await.unwrap();
}
