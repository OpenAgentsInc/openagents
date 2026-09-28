//! Read latency and throughput, host handling alone and end to end. Ignored:
//! run with `cargo test -p coder-connect --release bench_ -- --ignored
//! --nocapture --test-threads 1`. Each group pairs its own grant so no
//! group meets the 240-reads-a-minute rate.
use super::*;
use std::sync::Arc;
use std::time::{Duration, Instant};

const READS: usize = 96;
const CONCURRENT: usize = 4;

fn report(label: &str, reads: usize, elapsed: Duration, mut each: Vec<Duration>) {
    each.sort();
    let at = |q: f64| each[((each.len() - 1) as f64 * q) as usize];
    println!(
        "{label:<34} {reads:>4} reads  {:>9.1} reads/s  p50 {:>8.2?}  p90 {:>8.2?}  max {:>8.2?}",
        reads as f64 / elapsed.as_secs_f64(),
        at(0.5),
        at(0.9),
        at(1.0),
    );
}

/// Run `read` `READS` times, one at a time and then `CONCURRENT` at once.
fn host_group(label: &str, read: impl Fn(&Fixture) + Sync) {
    for concurrent in [1, CONCURRENT] {
        let f = Fixture::new("wss://relay.example/");
        read(&f); // warm the store and the reader
        let started = Instant::now();
        let each: Vec<Duration> = std::thread::scope(|scope| {
            let workers: Vec<_> = (0..concurrent)
                .map(|_| {
                    scope.spawn(|| {
                        (0..READS / concurrent)
                            .map(|_| {
                                let one = Instant::now();
                                read(&f);
                                one.elapsed()
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            workers
                .into_iter()
                .flat_map(|w| w.join().unwrap())
                .collect()
        });
        report(
            &format!("{label} x{concurrent}"),
            READS,
            started.elapsed(),
            each,
        );
    }
}

#[test]
#[ignore = "benchmark"]
fn bench_host_handling() {
    host_group("host direct catalog", |f| {
        let pending = f
            .client()
            .prepare_for(
                Query::Catalog(CatalogRequest::default()),
                unix_time().unwrap(),
                Route::Direct,
            )
            .unwrap();
        assert!(
            f.host()
                .handle_direct(&pending.event)
                .unwrap()
                .read
                .is_some()
        );
    });
    host_group("host relay catalog", |f| {
        let pending = f
            .client()
            .prepare(
                Query::Catalog(CatalogRequest::default()),
                unix_time().unwrap(),
            )
            .unwrap();
        let reply = f
            .host()
            .handle_current(&pending.event, &f.code.relay)
            .unwrap();
        assert!(
            f.client()
                .verify_reply(&pending, &reply, unix_time().unwrap())
                .is_ok()
        );
    });
}

async fn end_to_end(label: &str, f: &Fixture, client: Arc<Client>) {
    let read = |client: Arc<Client>| async move {
        let one = Instant::now();
        let observed = client
            .observe(Query::Catalog(CatalogRequest::default()))
            .await
            .unwrap();
        assert!(matches!(observed, Observation::Catalog(_)));
        one.elapsed()
    };
    let _ = f;
    read(client.clone()).await; // connect
    for concurrent in [1, CONCURRENT] {
        let started = Instant::now();
        let mut each = Vec::new();
        for _ in 0..READS / concurrent {
            let batch: Vec<_> = (0..concurrent)
                .map(|_| tokio::spawn(read(client.clone())))
                .collect();
            for one in batch {
                each.push(one.await.unwrap());
            }
        }
        report(
            &format!("{label} x{concurrent}"),
            READS,
            started.elapsed(),
            each,
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "benchmark"]
async fn bench_end_to_end() {
    // Direct: the device's tailnet connection to the host's listener.
    let f = Fixture::new("wss://relay.example/");
    let address = direct::listener(Arc::new(f.host())).await;
    let client = Arc::new(f.client());
    client.set_direct(Some(address));
    end_to_end("direct end to end", &f, client).await;
    // Relay: an in-process NIP-42 relay and the host's own serve loop.
    let (url, relay, _) = relay::start().await;
    let f = Fixture::new(&url);
    let serving = tokio::spawn(crate::cli::serve_observer(
        f.host(),
        url.clone(),
        RelayPolicy::LoopbackTest,
    ));
    tokio::time::sleep(Duration::from_millis(200)).await;
    end_to_end("relay end to end", &f, Arc::new(f.client())).await;
    serving.abort();
    relay.abort();
}
