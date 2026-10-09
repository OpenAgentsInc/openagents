#![allow(clippy::unwrap_used, clippy::expect_used)]
//! A pylon and a buyer against an in-process relay, with the echo engine:
//! discovery from the beacon, an encrypted free job, the receipt, the field
//! projection, and a pool aggregate that recomputes.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use nostr::domain::{Event, EventClass, Filter};
use nostr::pylon::{PoolPolicy, parse_receipt};
use pylon::client::{self, Ask};
use pylon::engine::Echo;
use pylon::field::{Live, RelayField};
use pylon::identity::Identity;
use pylon::lease::Leases;
use pylon::pool;
use pylon::provider::{Config, Provider};
use pylon::share;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, mpsc};
use tokio_tungstenite::tungstenite::Message;

struct Sub {
    conn: u64,
    id: String,
    filters: Vec<Filter>,
    tx: mpsc::UnboundedSender<String>,
}

#[derive(Default)]
struct Hub {
    stored: Vec<Event>,
    subs: Vec<Sub>,
}

/// A minimal NIP-01/NIP-42 relay: stores regular and addressable events,
/// fans every accepted event out to matching subscriptions, and keeps no
/// ephemeral event.
async fn relay() -> (String, Arc<Mutex<Hub>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let hub = Arc::new(Mutex::new(Hub::default()));
    let shared = Arc::clone(&hub);
    tokio::spawn(async move {
        let mut next = 0_u64;
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            next += 1;
            let conn = next;
            let hub = Arc::clone(&shared);
            tokio::spawn(async move {
                let Ok(socket) = tokio_tungstenite::accept_async(stream).await else {
                    return;
                };
                let (mut sink, mut source) = socket.split();
                let (tx, mut rx) = mpsc::unbounded_channel::<String>();
                tokio::spawn(async move {
                    while let Some(text) = rx.recv().await {
                        if sink.send(Message::Text(text.into())).await.is_err() {
                            return;
                        }
                    }
                });
                tx.send(json!(["AUTH", "challenge"]).to_string()).unwrap();
                while let Some(Ok(Message::Text(text))) = source.next().await {
                    let frame: Value = serde_json::from_str(&text).unwrap();
                    match frame[0].as_str().unwrap() {
                        "AUTH" => {
                            let _ = tx.send(json!(["OK", frame[1]["id"], true, ""]).to_string());
                        }
                        "EVENT" => {
                            let event: Event = serde_json::from_value(frame[1].clone()).unwrap();
                            let ok = event.validate_crypto().is_ok();
                            let _ = tx.send(json!(["OK", event.id, ok, ""]).to_string());
                            if !ok {
                                continue;
                            }
                            let mut hub = hub.lock().await;
                            match event.class() {
                                EventClass::Ephemeral => {}
                                EventClass::Addressable => {
                                    let d = event
                                        .tag_values("d")
                                        .next()
                                        .unwrap_or_default()
                                        .to_string();
                                    hub.stored.retain(|e| {
                                        !(e.kind == event.kind
                                            && e.pubkey == event.pubkey
                                            && e.tag_values("d").next().unwrap_or_default() == d)
                                    });
                                    hub.stored.push(event.clone());
                                }
                                _ => hub.stored.push(event.clone()),
                            }
                            for sub in &hub.subs {
                                if nostr::domain::matches_any(&sub.filters, &event) {
                                    let _ =
                                        sub.tx.send(json!(["EVENT", sub.id, event]).to_string());
                                }
                            }
                        }
                        "REQ" => {
                            let id = frame[1].as_str().unwrap().to_string();
                            let filters: Vec<Filter> = frame.as_array().unwrap()[2..]
                                .iter()
                                .map(|f| serde_json::from_value(f.clone()).unwrap())
                                .collect();
                            let mut hub = hub.lock().await;
                            for event in &hub.stored {
                                if nostr::domain::matches_any(&filters, event) {
                                    let _ = tx.send(json!(["EVENT", id, event]).to_string());
                                }
                            }
                            let _ = tx.send(json!(["EOSE", id]).to_string());
                            hub.subs.push(Sub {
                                conn,
                                id,
                                filters,
                                tx: tx.clone(),
                            });
                        }
                        "CLOSE" => {
                            let id = frame[1].as_str().unwrap();
                            hub.lock()
                                .await
                                .subs
                                .retain(|s| !(s.conn == conn && s.id == id));
                        }
                        _ => {}
                    }
                }
                hub.lock().await.subs.retain(|s| s.conn != conn);
            });
        }
    });
    (url, hub)
}

async fn wait_for_beacon(hub: &Arc<Mutex<Hub>>) {
    for _ in 0..100 {
        if hub.lock().await.stored.iter().any(|e| e.kind == 30_200) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("no beacon arrived");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_free_job_runs_end_to_end_and_the_pool_counts_it() {
    let (url, hub) = relay().await;
    let home = tempfile::tempdir().unwrap();
    let provider_key = Identity::generate();
    let buyer = Identity::generate();
    let stranger = Identity::generate();

    let mut config = Config::new(&url, "test-4080", home.path().to_path_buf());
    config.allow = Some(BTreeSet::from([buyer.pubkey().to_string()]));
    config.rate_per_minute = 2;
    let provider = Provider::new(config, provider_key.clone(), Arc::new(Echo)).unwrap();
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let running = tokio::spawn(Arc::clone(&provider).run(async {
        let _ = stop_rx.await;
    }));
    wait_for_beacon(&hub).await;

    // The buyer finds the pylon from its beacon and gets an answer.
    let ask = |prompt: &str| Ask {
        relay: url.clone(),
        pylon: None,
        prompt: prompt.into(),
        wait: Duration::from_secs(10),
        publish_receipt: true,
        home: home.path().to_path_buf(),
        checkers: BTreeSet::new(),
        pay: None,
    };
    let answer = client::ask(&buyer, &ask("hello pylon")).await.unwrap();
    assert_eq!(answer.text.as_deref(), Some("echo: nolyp olleh"));
    assert_eq!(answer.outcome, "accepted");
    assert_eq!(answer.pylon_npub, provider_key.npub());
    assert!(answer.contact_ms.is_some() && answer.answer_ms.is_some());

    // The receipt is on the relay, verifies, names the request, and is free.
    let receipt_id = answer.receipt.clone().unwrap();
    let stored = hub
        .lock()
        .await
        .stored
        .iter()
        .find(|e| e.id == receipt_id)
        .cloned()
        .unwrap();
    let receipt = parse_receipt(&stored, None).unwrap();
    assert_eq!(receipt.provider, provider_key.pubkey());
    assert_eq!(receipt.request, answer.request);
    assert!(receipt.payment.is_none());
    assert!(home.path().join("receipts.jsonl").exists());
    // No job content reached the relay in the clear.
    for event in &hub.lock().await.stored {
        assert!(!event.content.contains("hello pylon"));
    }

    // A key that is not on the allowlist is refused before any model work.
    let refused = client::ask(&stranger, &ask("let me in")).await.unwrap();
    assert!(refused.text.is_none());
    assert!(refused.error.unwrap().starts_with("not_admitted"));

    // The admitted buyer's rate limit (2 per minute) refuses a third job.
    let second = client::ask(&buyer, &ask("again")).await.unwrap();
    assert!(second.text.is_some());
    let third = client::ask(&buyer, &ask("and again")).await.unwrap();
    assert!(third.error.unwrap().starts_with("rate_limited"));
    assert_eq!(provider.counters().await.served, 2);

    // The field shows the pylon online with its receipt-backed jobs.
    let field = RelayField::new(&url, Some("everglade"), Identity::generate());
    let states = field.poll().await.unwrap();
    assert_eq!(states.len(), 1);
    assert_eq!(states[0].status, "online");
    assert_eq!(states[0].jobs, 2);

    // The aggregator counts the accepted jobs, and a reader recomputes it.
    let aggregator = Identity::generate();
    let policy = PoolPolicy::open("everglade", pool::SLICES);
    let (aggregate, event) = pool::aggregate(&aggregator, &url, &policy, 60, true)
        .await
        .unwrap();
    assert!(event.is_some());
    assert_eq!(aggregate.totals.pylons_online, 1);
    assert_eq!(aggregate.totals.jobs.accepted, 2);
    assert_eq!(aggregate.totals.jobs.failed, 2);
    let verified = pool::verify(&Identity::generate(), &url, aggregator.pubkey(), &policy)
        .await
        .unwrap();
    assert_eq!(verified, aggregate);
    // Every job's in-flight mark is gone once it ended.
    assert!(pylon::inflight::read(home.path(), pylon::now()).is_empty());

    // A live subscription shows the same pylon and jobs, and recomputes
    // the aggregate.
    let live = Arc::new(std::sync::Mutex::new(Live::new(Some("everglade"))));
    let stop = Arc::new(AtomicBool::new(false));
    let watcher = {
        let (live, stop, url) = (Arc::clone(&live), Arc::clone(&stop), url.clone());
        tokio::spawn(async move {
            RelayField::new(&url, Some("everglade"), Identity::generate())
                .watch(&live, &stop)
                .await;
        })
    };
    let seen = |check: fn(&Live) -> bool| {
        let live = Arc::clone(&live);
        async move {
            for _ in 0..100 {
                if check(&live.lock().unwrap()) {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            panic!(
                "the live field never showed it: {:?}",
                live.lock().unwrap().error
            );
        }
    };
    seen(|l| l.synced).await;
    {
        let l = live.lock().unwrap();
        let pylons = l.pylons(pylon::now());
        assert_eq!(pylons.len(), 1);
        assert_eq!(pylons[0].state.status, "online");
        assert_eq!(pylons[0].state.jobs, 2);
        let (held, recomputed) = l.aggregate(pylon::now()).unwrap();
        assert_eq!(held.totals.jobs.accepted, 2);
        assert!(recomputed);
    }

    // Stopping publishes an offline beacon, which the live field sees
    // without polling. A beacon sampled in the same second as the one held
    // doesn't replace it, so the stop waits for the clock to turn.
    tokio::time::sleep(Duration::from_millis(1_100)).await;
    stop_tx.send(()).unwrap();
    running.await.unwrap().unwrap();
    let states = field.poll().await.unwrap();
    assert_eq!(states[0].status, "offline");
    seen(|l| l.pylons(pylon::now())[0].state.status == "offline").await;
    stop.store(true, Ordering::Relaxed);
    watcher.await.unwrap();
}

fn lease_broker(dir: &tempfile::TempDir) -> coder_lease::Broker {
    let limits = coder_lease::Limits {
        build: 2,
        memory_gib: 16,
        disk_floor_gb: 0,
        build_disk_gb: 0,
    };
    coder_lease::Broker::new(dir.path().join("leases"), limits).with_poll(Duration::from_millis(10))
}

fn owner_build(broker: &coder_lease::Broker) -> coder_lease::Lease {
    let holder = coder_lease::Holder {
        session: "owner:1".into(),
        agent: "none".into(),
        pid: std::process::id(),
        command: "cargo".into(),
    };
    broker
        .acquire(
            coder_lease::Request::new(coder_lease::Resource::Build, holder)
                .priority(coder_lease::Priority::Owner)
                .wait(coder_lease::Wait::No),
        )
        .unwrap()
}

/// A pylon linked to its owner over NIP-OA, sharing this computer through
/// the lease broker: each job holds a background `pylon` lease, the owner's
/// own work refuses new jobs and drains the beacon, and the owner's own
/// receipts never count.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_owned_pylon_yields_to_the_owner_and_never_counts_their_receipts() {
    let (url, hub) = relay().await;
    let home = tempfile::tempdir().unwrap();
    let leases = tempfile::tempdir().unwrap();
    let broker = lease_broker(&leases);
    let provider_key = Identity::generate();
    let owner = Identity::generate();
    let buyer = Identity::generate();

    let credential =
        nostr::domain::mint_owner_attestation(owner.secret(), provider_key.pubkey(), "kind=30200")
            .unwrap();
    let mut config = Config::new(&url, "studio-mac", home.path().to_path_buf());
    config.allow = Some(BTreeSet::from([
        buyer.pubkey().to_string(),
        owner.pubkey().to_string(),
    ]));
    config.owner = Some(credential.clone());
    // A credential for another key is refused before the pylon starts.
    let mut wrong = config.clone();
    wrong.owner = Some(
        nostr::domain::mint_owner_attestation(owner.secret(), buyer.pubkey(), "kind=30200")
            .unwrap(),
    );
    assert!(
        Provider::on(
            wrong,
            provider_key.clone(),
            Arc::new(Echo),
            Arc::new(Leases::new(broker.clone())),
        )
        .is_err()
    );
    let provider = Provider::on(
        config,
        provider_key.clone(),
        Arc::new(Echo),
        Arc::new(Leases::new(broker.clone())),
    )
    .unwrap();
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let running = tokio::spawn(Arc::clone(&provider).run(async {
        let _ = stop_rx.await;
    }));
    wait_for_beacon(&hub).await;

    // The beacon names its verified owner.
    let beacon = hub
        .lock()
        .await
        .stored
        .iter()
        .find(|e| e.kind == 30_200)
        .cloned()
        .unwrap();
    let (_, found) = nostr::pylon::parse_owned_beacon(&beacon).unwrap();
    assert_eq!(found.as_deref(), Some(owner.pubkey()));

    let ask = |prompt: &str| Ask {
        relay: url.clone(),
        pylon: None,
        prompt: prompt.into(),
        wait: Duration::from_secs(10),
        publish_receipt: true,
        home: home.path().to_path_buf(),
        checkers: BTreeSet::new(),
        pay: None,
    };
    // A buyer's job and the owner's own job both run; each took a lease
    // that is gone once the job ended.
    assert!(
        client::ask(&buyer, &ask("from a buyer"))
            .await
            .unwrap()
            .text
            .is_some()
    );
    assert!(
        client::ask(&owner, &ask("from the owner"))
            .await
            .unwrap()
            .text
            .is_some()
    );
    assert!(broker.list().unwrap().is_empty());

    // The field and the pool count the buyer's job, not the owner's.
    let field = RelayField::new(&url, Some("everglade"), Identity::generate());
    assert_eq!(field.poll().await.unwrap()[0].jobs, 1);
    let policy = PoolPolicy::open("everglade", pool::SLICES);
    let aggregator = Identity::generate();
    let (aggregate, _) = pool::aggregate(&aggregator, &url, &policy, 60, true)
        .await
        .unwrap();
    assert_eq!(aggregate.totals.jobs.accepted, 1);
    let verified = pool::verify(&Identity::generate(), &url, aggregator.pubkey(), &policy)
        .await
        .unwrap();
    assert_eq!(verified, aggregate);

    // While the owner's build holds the machine, a new job is refused and
    // the pylon drains.
    let build = owner_build(&broker);
    let refused = client::ask(&buyer, &ask("while you build")).await.unwrap();
    assert!(refused.text.is_none());
    assert!(refused.error.unwrap().starts_with("rate_limited"));
    assert_eq!(
        provider.beacon(None).await.status,
        nostr::pylon::Status::Draining
    );
    assert_eq!(provider.beacon(None).await.slots.free, 0);
    drop(build);

    stop_tx.send(()).unwrap();
    running.await.unwrap().unwrap();
}

/// `openagents host share on|off`: the host's supervisor starts the pylon
/// when the setting turns on and stops it, with an offline beacon, when it
/// turns off.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn host_share_turns_the_pylon_on_and_off() {
    let (url, hub) = relay().await;
    let home = tempfile::tempdir().unwrap();
    let leases = tempfile::tempdir().unwrap();
    let buyer = Identity::generate();
    let words = |w: &[&str]| w.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();

    // Off by default; `on` needs to know who may send jobs.
    assert_eq!(
        share::load(home.path()).unwrap(),
        None,
        "sharing starts off"
    );
    assert_eq!(
        share::command(&words(&["on", "--relay", &url]), home.path(), "box"),
        1
    );
    assert_eq!(
        share::command(
            &words(&[
                "on",
                "--relay",
                &url,
                "--allow",
                &buyer.npub(),
                "--engine",
                "http://127.0.0.1:9",
            ]),
            home.path(),
            "box",
        ),
        0
    );
    let settings = share::load(home.path()).unwrap().unwrap();
    assert!(settings.on);
    assert_eq!(settings.pylon, "box");

    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let supervisor = tokio::spawn(share::supervise(
        home.path().to_path_buf(),
        Arc::new(Leases::new(lease_broker(&leases))),
        Duration::from_millis(100),
        |_| {},
        async {
            let _ = stop_rx.await;
        },
    ));
    wait_for_beacon(&hub).await;
    let field = RelayField::new(&url, Some("everglade"), Identity::generate());
    let states = field.poll().await.unwrap();
    assert_eq!(states.len(), 1);
    // No model answers on that port, so the pylon shows draining.
    assert_eq!(states[0].status, "draining");

    // Turning it off stops the pylon with an offline beacon.
    tokio::time::sleep(Duration::from_millis(1_100)).await;
    assert_eq!(share::command(&words(&["off"]), home.path(), "box"), 0);
    let mut offline = false;
    for _ in 0..100 {
        if field.poll().await.unwrap()[0].status == "offline" {
            offline = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(offline);
    stop_tx.send(()).unwrap();
    supervisor.await.unwrap();
}

/// An engine that knows every pinned canary's answer and says `hi` to
/// anything else: the honest pylon in the check fixtures.
struct Oracle;

impl pylon::engine::Engine for Oracle {
    fn model(&self) -> &str {
        "oracle"
    }

    fn healthy(&self) -> pylon::engine::Pending<'_, bool> {
        Box::pin(async { true })
    }

    fn generate<'a>(
        &'a self,
        turns: &'a [pylon::engine::Turn],
        _max_tokens: u32,
    ) -> pylon::engine::Pending<'a, Result<pylon::engine::Generation, String>> {
        Box::pin(async move {
            let last = turns.last().ok_or("no message")?;
            let text = pylon::check::suites()
                .into_iter()
                .flat_map(|s| s.canaries)
                .find(|c| c.prompt == last.content)
                .map_or_else(|| "Hi.".to_string(), |c| format!("{}.", c.expect));
            Ok(pylon::engine::Generation {
                text,
                input_tokens: Some(4),
                output_tokens: Some(1),
                model: "oracle".into(),
            })
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_pylon_with_wrong_canary_answers_is_marked_check_fail_and_leaves_the_pool() {
    use nostr::pylon::{Standing, Verdict, parse_check};
    use pylon::check::Checker;

    let (url, hub) = relay().await;
    let home = tempfile::tempdir().unwrap();
    let mut running = Vec::new();
    let mut keys = Vec::new();
    for (slug, engine) in [
        (
            "honest-a",
            Arc::new(Oracle) as Arc<dyn pylon::engine::Engine>,
        ),
        ("honest-b", Arc::new(Oracle)),
        ("liar", Arc::new(Echo)),
    ] {
        let key = Identity::generate();
        let mut config = Config::new(&url, slug, home.path().to_path_buf());
        config.allow = None;
        let provider = Provider::new(config, key.clone(), engine).unwrap();
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
        running.push((
            stop_tx,
            tokio::spawn(Arc::clone(&provider).run(async {
                let _ = stop_rx.await;
            })),
        ));
        keys.push(key);
    }
    for _ in 0..100 {
        if hub
            .lock()
            .await
            .stored
            .iter()
            .filter(|e| e.kind == 30_200)
            .count()
            == 3
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let (honest, liar) = (keys[0].pubkey().to_string(), keys[2].pubkey().to_string());
    let checker = Checker {
        relay: url.clone(),
        checker: Identity::generate(),
        buyer: Identity::generate(),
        home: home.path().to_path_buf(),
        wait: Duration::from_secs(10),
    };
    let trusted = BTreeSet::from([checker.checker.pubkey().to_string()]);

    // Canaries through the normal job path: the honest pylon passes every
    // one, the echoing pylon fails every one, and each verdict is a label
    // on the receipt from a key that is neither its buyer nor its pylon.
    let good = checker.canaries(&honest).await.unwrap();
    let bad = checker.canaries(&liar).await.unwrap();
    assert_eq!(good.len(), 3);
    assert!(good.iter().all(|c| c.verdict == Verdict::Pass));
    assert!(bad.iter().all(|c| c.verdict == Verdict::Fail));
    let stored = hub.lock().await.stored.clone();
    for c in good.iter().chain(&bad) {
        let label = stored
            .iter()
            .find(|e| Some(&e.id) == c.label.as_ref())
            .unwrap();
        let parsed = parse_check(label).unwrap();
        assert_eq!(Some(&parsed.receipt), c.receipt.as_ref());
        assert_eq!(parsed.checker, checker.checker.pubkey());
        assert!(parsed.suite().is_some());
    }

    // The pool's admission drops the failing pylon; the open policy still
    // counts it.
    let checked = PoolPolicy::open("everglade", pool::SLICES).checked(trusted.clone());
    let (aggregate, _) = pool::aggregate(&Identity::generate(), &url, &checked, 60, false)
        .await
        .unwrap();
    assert_eq!(aggregate.totals.pylons_online, 2);
    assert_eq!(aggregate.totals.checks.pass, 3);
    assert_eq!(aggregate.totals.checks.fail, 3);
    assert_eq!(aggregate.totals.jobs.accepted, 3);
    let open = PoolPolicy::open("everglade", pool::SLICES);
    let (aggregate, _) = pool::aggregate(&Identity::generate(), &url, &open, 60, false)
        .await
        .unwrap();
    assert_eq!(aggregate.totals.pylons_online, 3);
    let aggregator = Identity::generate();
    pool::aggregate(&aggregator, &url, &checked, 60, true)
        .await
        .unwrap();
    pool::verify(&Identity::generate(), &url, aggregator.pubkey(), &checked)
        .await
        .unwrap();

    // A buyer that trusts the checker never routes to the failing pylon.
    let buyer = Identity::generate();
    for _ in 0..3 {
        let answer = client::ask(
            &buyer,
            &Ask {
                relay: url.clone(),
                pylon: None,
                prompt: "hello".into(),
                wait: Duration::from_secs(10),
                publish_receipt: false,
                home: home.path().to_path_buf(),
                checkers: trusted.clone(),
                pay: None,
            },
        )
        .await
        .unwrap();
        assert!(!answer.pylon.contains(&liar));
    }
    let only_liar = client::ask(
        &buyer,
        &Ask {
            relay: url.clone(),
            pylon: Some(liar.clone()),
            prompt: "hello".into(),
            wait: Duration::from_secs(10),
            publish_receipt: false,
            home: home.path().to_path_buf(),
            checkers: trusted.clone(),
            pay: None,
        },
    )
    .await;
    assert!(only_liar.unwrap_err().contains("passes its checks"));

    // Redundant execution: the two honest pylons agree, the liar doesn't.
    let pylons: Vec<String> = keys.iter().map(|k| k.pubkey().to_string()).collect();
    let runs = checker.redundant("Say hi.", &pylons).await.unwrap();
    let verdicts: Vec<Verdict> = runs.iter().map(|c| c.verdict).collect();
    assert_eq!(verdicts, [Verdict::Pass, Verdict::Pass, Verdict::Fail]);
    assert!(runs.iter().all(|c| c.label.is_some()));

    // NIP-XP: a passing canary earns the pylon one award per suite, never
    // two, and a failing pylon earns none.
    let award = checker.award(&good).await.unwrap().unwrap();
    let stored = hub.lock().await.stored.clone();
    let event = stored.iter().find(|e| e.id == award).unwrap();
    let parsed = nostr::xp::parse_award(event).unwrap();
    assert_eq!(parsed.awardees[0].pubkey, honest);
    assert_eq!(parsed.total(), pylon::check::XP_PER_SUITE);
    assert!(checker.award(&good).await.unwrap().is_none());
    assert!(checker.award(&bad).await.unwrap().is_none());

    // The league: one GPU class on its pinned suite, the honest pylon
    // first with the sigil, the liar last at 0%.
    let league = pylon::league::fetch(&Identity::generate(), &url, &trusted)
        .await
        .unwrap();
    assert_eq!(league.classes.len(), 1);
    let class = &league.classes[0];
    assert_eq!(
        class.suite,
        pylon::check::suite_for(nostr::pylon::Family::Gpu).digest()
    );
    let first = &class.rows[0];
    assert!(first.pylon.contains(&honest));
    assert_eq!(first.pass_rate, Some(1.0));
    assert!(first.sigil);
    let row = |key: &str| class.rows.iter().find(|r| r.pylon.contains(key)).unwrap();
    let failing = row(&liar);
    assert_eq!(failing.pass_rate, Some(0.0));
    assert_eq!(failing.standing, Standing::Failing);
    assert!(!failing.sigil);
    // A pylon checked only by redundant runs has no suite rate, ranks
    // last, and still carries the sigil from its passing check.
    let unranked = row(keys[1].pubkey());
    assert_eq!(unranked.pass_rate, None);
    assert!(unranked.sigil);
    assert!(class.rows.last().unwrap().pylon.contains(keys[1].pubkey()));
    assert!(pylon::league::render(&league).contains("100%"));

    // The live field gives the honest pylons their sigils, and recomputes
    // a checked aggregate published after the last verdict.
    pool::aggregate(&aggregator, &url, &checked, 60, true)
        .await
        .unwrap();
    let live = Arc::new(std::sync::Mutex::new(
        Live::new(Some("everglade")).trusting(trusted.clone()),
    ));
    let stop = Arc::new(AtomicBool::new(false));
    let watcher = {
        let (live, stop, url, trusted) = (
            Arc::clone(&live),
            Arc::clone(&stop),
            url.clone(),
            trusted.clone(),
        );
        tokio::spawn(async move {
            RelayField::new(&url, Some("everglade"), Identity::generate())
                .trusting(trusted)
                .watch(&live, &stop)
                .await;
        })
    };
    for _ in 0..100 {
        if live.lock().unwrap().synced {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    {
        let l = live.lock().unwrap();
        let pylons = l.pylons(pylon::now());
        let standing = |key: &str| {
            pylons
                .iter()
                .find(|p| p.state.pylon.contains(key))
                .unwrap()
                .standing
        };
        assert_eq!(standing(&honest), Standing::Passing);
        assert_eq!(standing(keys[1].pubkey()), Standing::Passing);
        assert_eq!(standing(&liar), Standing::Failing);
        // The checked aggregate recomputes from what the field holds.
        assert!(l.aggregate(pylon::now()).unwrap().1);
    }
    stop.store(true, Ordering::Relaxed);
    watcher.await.unwrap();
    for (stop_tx, task) in running {
        stop_tx.send(()).unwrap();
        task.await.unwrap().unwrap();
    }
}
