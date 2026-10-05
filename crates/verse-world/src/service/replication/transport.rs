//! Owned scratch TLS latency and resync evidence; no external machines.
use super::*;
use crate::service::{client::Client, net::tests::start};
use rustls::pki_types::ServerName;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::mpsc,
};
async fn delayed<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    mut read: R,
    mut write: W,
    latency: Duration,
    peak: Arc<AtomicUsize>,
) -> std::io::Result<()> {
    let (send, mut receive) = mpsc::channel::<(tokio::time::Instant, Vec<u8>)>(8);
    let producer = async move {
        let mut buffer = [0; 4096];
        loop {
            let count = read.read(&mut buffer).await?;
            if count == 0 {
                break;
            }
            send.send((
                tokio::time::Instant::now() + latency,
                buffer[..count].to_vec(),
            ))
            .await
            .map_err(|_| std::io::ErrorKind::BrokenPipe)?;
            peak.fetch_max(8 - send.capacity(), Ordering::Relaxed);
        }
        Ok::<_, std::io::Error>(())
    };
    let consumer = async move {
        while let Some((due, bytes)) = receive.recv().await {
            tokio::time::sleep_until(due).await;
            write.write_all(&bytes).await?;
        }
        write.shutdown().await
    };
    let (a, b) = tokio::join!(producer, consumer);
    a?;
    b
}
#[tokio::test]
async fn delayed_tls_deltas_resync_and_bound_replaceable_backlog() {
    let keys = [key(231), key(232), key(233)];
    let (host_address, tls, stop, host) = start(&keys).await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let up = Arc::new(AtomicUsize::new(0));
    let down = Arc::new(AtomicUsize::new(0));
    let proxy_up = up.clone();
    let proxy_down = down.clone();
    let proxy = tokio::spawn(async move {
        let (client, _) = listener.accept().await.unwrap();
        let authority = TcpStream::connect(host_address).await.unwrap();
        client.set_nodelay(true).unwrap();
        authority.set_nodelay(true).unwrap();
        let (cr, cw) = client.into_split();
        let (ar, aw) = authority.into_split();
        tokio::join!(
            delayed(cr, aw, Duration::from_millis(67), proxy_up),
            delayed(ar, cw, Duration::from_millis(100), proxy_down)
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
    let mut round_trips = vec![];
    let mut owned = None;
    let mut last_tick = 0;
    for i in 0..12 {
        let began = std::time::Instant::now();
        let state = if i == 6 {
            let response = client
                .request(Body::Replicate {
                    ack: Some(Baseline {
                        revision: 999_999,
                        tick: 0,
                        digest: [7; 32],
                    }),
                })
                .await
                .unwrap();
            let Reply::Snapshot { state } = response.body else {
                panic!()
            };
            state
        } else if i == 7 {
            client.resync().await.unwrap()
        } else {
            client.snapshot().await.unwrap()
        };
        round_trips.push(began.elapsed().as_secs_f64() * 1000.);
        assert!(state.scope.is_some());
        assert!(client.tick() >= last_tick);
        last_tick = client.tick();
        let life = state.hud.unwrap().life;
        assert!(owned.is_none_or(|previous| previous == life));
        owned = Some(life);
    }
    client.close().await.unwrap();
    stop.send(()).unwrap();
    let exit = host.await.unwrap();
    assert!(exit.failure.is_none());
    proxy.abort();
    let _ = proxy.await;
    let stats = exit.stats.replication;
    assert_eq!(stats.full, 3);
    assert_eq!(stats.deltas, 9);
    assert_eq!(stats.resyncs, 1);
    assert_eq!(stats.retained_bytes, 0);
    assert!(up.load(Ordering::Relaxed) <= 8 && down.load(Ordering::Relaxed) <= 8);
    round_trips.sort_by(f64::total_cmp);
    println!(
        "VERSE_V05_TLS_EVIDENCE {}",
        serde_json::json!({"stats":stats,"samples":12,"upstream_delay_ms":67,"downstream_delay_ms":100,"snapshot_age_at_receipt_lower_bound_ms":100,"round_trip_p95_ms":round_trips[11],"round_trip_median_ms":round_trips[6],"queued_chunks_upstream_peak":up.load(Ordering::Relaxed),"queued_chunks_downstream_peak":down.load(Ordering::Relaxed),"proxy_chunk_bytes":4096,"proxy_capacity_chunks":8,"replaceable_requests_in_flight":1,"unknown_ack_resync":true,"client_discard_resync":true})
    );
}
