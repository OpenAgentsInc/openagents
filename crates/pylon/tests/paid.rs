#![allow(clippy::unwrap_used, clippy::expect_used)]
//! P3 paid pylon jobs on a test network, against the in-process relay:
//!
//! - Direct: three priced pylons take per-job Lightning payments on
//!   regtest from a buyer's wallet; every receipt carries a preimage that
//!   hashes to its payment hash, and the pool's aggregate counts the sats
//!   under `regtest`, never `bitcoin`.
//! - Brokered: 1,000 jobs across three pylons, each paid by a customer's
//!   x402 payment to OpenAgents, settle in the split ledger with one row
//!   per receipt; providers are paid by balance sweeps, never per job; and
//!   failed checks forfeit the unpaid share of the failing jobs.
//!
//! All sats are worthless test sats in memory (`TestLightning`) and the
//! payout rails are a fake; nothing touches a wallet.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nostr::domain::Event;
use nostr::pylon::check::{Verdict, check_event};
use nostr::pylon::{Payment, PoolPolicy, parse_receipt};
use pay_ledger::payout::{Invoice, Lookup, Outcome, Rails};
use pay_ledger::{Ledger, Payee};
use pylon::broker::{Broker, party};
use pylon::client::{self, Ask, Pay};
use pylon::engine::Echo;
use pylon::fixture::{Oracle, relay};
use pylon::identity::Identity;
use pylon::paid::{Grant, Invoicer, Network, Payer, Price, Terms, TestLightning, pay};
use pylon::pool;
use pylon::provider::{Config, Provider};

type Running = Vec<(
    tokio::sync::oneshot::Sender<()>,
    tokio::task::JoinHandle<Result<(), String>>,
)>;

async fn start(
    url: &str,
    home: &std::path::Path,
    count: usize,
    price: Option<(Price, Arc<TestLightning>)>,
    allow: Option<BTreeSet<String>>,
) -> (Vec<Identity>, Running) {
    let mut keys = Vec::new();
    let mut running = Vec::new();
    for n in 0..count {
        let key = Identity::generate();
        let mut config = Config::new(url, &format!("pylon-{n}"), home.to_path_buf());
        config.allow = allow.clone();
        config.rate_per_minute = 1_000_000;
        config.slots = 8;
        let engine: Arc<dyn pylon::engine::Engine> = Arc::new(Oracle);
        let provider = match &price {
            Some((price, net)) => {
                config.price = Some(*price);
                Provider::priced(
                    config,
                    key.clone(),
                    engine,
                    Arc::new(pylon::lease::Dedicated),
                    Arc::clone(net) as Arc<dyn Invoicer>,
                )
                .unwrap()
            }
            None => Provider::new(config, key.clone(), engine).unwrap(),
        };
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
        running.push((
            stop_tx,
            tokio::spawn(Arc::clone(&provider).run(async {
                let _ = stop_rx.await;
            })),
        ));
        keys.push(key);
    }
    (keys, running)
}

async fn wait_for_beacons(hub: &Arc<tokio::sync::Mutex<pylon::fixture::Hub>>, count: usize) {
    for _ in 0..200 {
        if hub
            .lock()
            .await
            .stored
            .iter()
            .filter(|e| e.kind == 30_200)
            .count()
            >= count
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("the beacons never arrived");
}

fn ask(url: &str, home: &std::path::Path, pylon: Option<String>, pay: Option<Pay>) -> Ask {
    Ask {
        relay: url.into(),
        pylon,
        prompt: "Say hi.".into(),
        wait: Duration::from_secs(20),
        publish_receipt: true,
        home: home.to_path_buf(),
        checkers: BTreeSet::new(),
        pay,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn priced_pylons_take_direct_per_job_payments_on_regtest() {
    let (url, hub) = relay().await.unwrap();
    let home = tempfile::tempdir().unwrap();
    let net = Arc::new(TestLightning::new(Network::Regtest).unwrap());
    let price = Price {
        msat: 3_000,
        network: Network::Regtest,
    };
    // A mainnet price never starts.
    let mut mainnet = Config::new(&url, "mainnet", home.path().to_path_buf());
    mainnet.price = Some(Price {
        msat: 3_000,
        network: Network::Bitcoin,
    });
    assert!(
        Provider::priced(
            mainnet,
            Identity::generate(),
            Arc::new(Echo),
            Arc::new(pylon::lease::Dedicated),
            Arc::clone(&net) as Arc<dyn Invoicer>,
        )
        .is_err()
    );
    let (keys, running) = start(&url, home.path(), 3, Some((price, Arc::clone(&net))), None).await;
    wait_for_beacons(&hub, 3).await;

    let buyer = Identity::generate();
    let wallet = Pay::Wallet {
        payer: Arc::clone(&net) as Arc<dyn Payer>,
        max_msat: 5_000,
    };
    let mut receipts = Vec::new();
    for round in 0..3 {
        for key in &keys {
            let answer = client::ask(
                &buyer,
                &ask(
                    &url,
                    home.path(),
                    Some(key.pubkey().into()),
                    Some(wallet.clone()),
                ),
            )
            .await
            .unwrap();
            assert_eq!(answer.text.as_deref(), Some("Hi."), "round {round}");
            assert_eq!(answer.paid_msat, Some(3_000));
            receipts.push(answer.receipt.unwrap());
        }
    }
    assert_eq!(net.paid(), (9, 27_000));
    let stored = hub.lock().await.stored.clone();
    for id in &receipts {
        let event = stored.iter().find(|e| &e.id == id).unwrap();
        let receipt = parse_receipt(event, None).unwrap();
        let payment = receipt.payment.unwrap();
        assert_eq!(payment.network, "regtest");
        assert!(pylon::paid::preimage_matches(
            &payment.preimage,
            &payment.payment_hash
        ));
    }

    // A buyer without a wallet, or with a lower ceiling, never pays and
    // gets no answer; the pylon refuses once the invoice goes unpaid.
    let unpaid = client::ask(
        &Identity::generate(),
        &ask(&url, home.path(), Some(keys[0].pubkey().into()), None),
    )
    .await
    .unwrap();
    assert!(unpaid.text.is_none());
    assert!(unpaid.error.unwrap().contains("no wallet"));
    let stingy = client::ask(
        &Identity::generate(),
        &ask(
            &url,
            home.path(),
            Some(keys[0].pubkey().into()),
            Some(Pay::Wallet {
                payer: Arc::clone(&net) as Arc<dyn Payer>,
                max_msat: 2_999,
            }),
        ),
    )
    .await
    .unwrap();
    assert!(stingy.error.unwrap().contains("ceiling"));
    assert_eq!(net.paid(), (9, 27_000));

    // The pool counts the sats under regtest and never under bitcoin.
    let policy = PoolPolicy::open("everglade", pool::SLICES);
    let (aggregate, _) = pool::aggregate(&Identity::generate(), &url, &policy, 60, false)
        .await
        .unwrap();
    assert_eq!(aggregate.totals.paid_msat.regtest, 27_000);
    assert_eq!(aggregate.totals.paid_msat.bitcoin, 0);
    for (stop, task) in running {
        stop.send(()).unwrap();
        task.await.unwrap().unwrap();
    }
}

/// Fake payout rails: record every payment, never touch a wallet.
#[derive(Default)]
struct Rail(Mutex<Vec<(String, i64)>>);

impl Rails for Rail {
    fn lightning_invoice(&self, address: &str, amount_msat: i64) -> Result<Invoice, String> {
        let n = self.0.lock().unwrap().len();
        Ok(Invoice {
            bolt11: format!("lnbcrt-{address}-{n}"),
            payment_hash: nostr::pylon::sha256_hex(format!("{address}-{n}").as_bytes()),
            amount_msat,
        })
    }
    fn pay_lightning(&self, invoice: &Invoice, _max_fee_msat: i64) -> Outcome {
        let address = invoice.bolt11.split('-').nth(1).unwrap_or_default();
        self.0
            .lock()
            .unwrap()
            .push((address.to_string(), invoice.amount_msat));
        Outcome::Sent { fee_msat: 0 }
    }
    fn lookup_lightning(&self, _payment_hash: &str) -> Result<Lookup, String> {
        Ok(Lookup::Sent { fee_msat: 0 })
    }
    fn fund_spark(&self, _amount_sats: u64) -> Result<(), String> {
        Err("no spark here".into())
    }
    fn pay_spark(&self, _address: &str, _amount_sats: u64, _key: &str) -> Outcome {
        Outcome::Failed("no spark here".into())
    }
    fn lookup_spark(&self, _key: &str) -> Result<Lookup, String> {
        Ok(Lookup::Absent)
    }
}

/// Buy `count` brokered jobs on `pylon`: the customer pays OpenAgents an
/// x402 invoice on regtest, and the broker buys the job with that payment
/// in its receipt.
async fn brokered_jobs(
    url: String,
    home: std::path::PathBuf,
    broker: Identity,
    pylon: String,
    count: usize,
    receiver: Arc<TestLightning>,
) -> Vec<String> {
    let mut out = Vec::new();
    for _ in 0..count {
        let invoice = receiver.invoice(10_000, "x402 pylon job").unwrap();
        let payment: Payment = pay(
            receiver.as_ref(),
            &Terms {
                network: Network::Regtest,
                invoice,
            },
            10_000,
        )
        .unwrap();
        let payment = Payment {
            profile: "x402-exact".into(),
            ..payment
        };
        let answer = client::ask(
            &broker,
            &ask(
                &url,
                &home,
                Some(pylon.clone()),
                Some(Pay::Brokered(payment)),
            ),
        )
        .await
        .unwrap();
        assert_eq!(answer.text.as_deref(), Some("Hi."));
        out.push(answer.receipt.unwrap());
    }
    out
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn a_thousand_brokered_jobs_settle_through_balance_sweeps() {
    const JOBS: usize = 1_000;
    let (url, hub) = relay().await.unwrap();
    let home = tempfile::tempdir().unwrap();
    let broker_key = Identity::generate();
    let (keys, running) = start(
        &url,
        home.path(),
        3,
        None,
        Some(BTreeSet::from([broker_key.pubkey().to_string()])),
    )
    .await;
    wait_for_beacons(&hub, 3).await;
    let receiver = Arc::new(TestLightning::new(Network::Regtest).unwrap());
    let mut broker = Broker::open(
        Ledger::in_memory().unwrap(),
        Network::Regtest,
        broker_key.pubkey(),
    )
    .unwrap();
    let mut ledger_payees = BTreeMap::new();
    for key in &keys {
        ledger_payees.insert(
            party(key.pubkey(), None),
            format!("{}@regtest.example", &key.pubkey()[..12]),
        );
    }
    let rails = Rail::default();
    let mut payout_ids = 0;
    let mut sweep = |broker: &mut Broker, at: i64| {
        let mut resolve = |_: &mut Ledger, party: &str, at: i64| {
            Ok(ledger_payees.get(party).map(|address| Payee {
                party: party.into(),
                destination_kind: "lud16".into(),
                destination_value: address.clone(),
                source: "fixture".into(),
                verified_at: at,
            }))
        };
        let mut next = || {
            payout_ids += 1;
            format!("sweep-{payout_ids}")
        };
        broker
            .sweep(&rails, at, None, &mut resolve, &mut next)
            .unwrap()
    };
    // The payout worker pays registered destinations only.
    for (party, address) in &ledger_payees {
        broker
            .register_payee(Payee {
                party: party.clone(),
                destination_kind: "lud16".into(),
                destination_value: address.clone(),
                source: "fixture".into(),
                verified_at: 0,
            })
            .unwrap();
    }

    // Two halves, a sweep between them.
    let start_at = pylon::now() as i64;
    let mut receipts: Vec<String> = Vec::new();
    for half in 0..2 {
        let mut tasks = Vec::new();
        for (key, count) in keys.iter().zip([167, 167, 166]) {
            tasks.push(tokio::spawn(brokered_jobs(
                url.clone(),
                home.path().to_path_buf(),
                broker_key.clone(),
                key.pubkey().to_string(),
                count,
                Arc::clone(&receiver),
            )));
        }
        let mut batch = Vec::new();
        for task in tasks {
            batch.extend(task.await.unwrap());
        }
        let stored = hub.lock().await.stored.clone();
        for id in &batch {
            let event = stored.iter().find(|e| &e.id == id).unwrap();
            // What reached the receiver: the whole price, no LSP fee here.
            broker.settle(event, None, 10_000, start_at).unwrap();
        }
        receipts.extend(batch);
        if half == 0 {
            // Every provider is owed over 1,000 sats: the sweep pays all three.
            let steps = sweep(&mut broker, start_at + 1);
            assert!(!steps.is_empty());
            assert_eq!(rails.0.lock().unwrap().len(), 3);
        }
    }
    assert_eq!(receipts.len(), JOBS);
    assert_eq!(receiver.paid(), (JOBS, JOBS as u64 * 10_000));

    // Victor checks some of the third pylon's jobs and fails them: ten
    // from the first half (already swept: a recorded loss) and ten from
    // the second (unpaid: forfeited).
    let stored = hub.lock().await.stored.clone();
    let by_id: BTreeMap<String, Event> = stored
        .iter()
        .filter(|e| receipts.contains(&e.id))
        .map(|e| (e.id.clone(), e.clone()))
        .collect();
    let third: Vec<&String> = receipts
        .iter()
        .filter(|id| parse_receipt(&by_id[*id], None).unwrap().provider == keys[2].pubkey())
        .collect();
    let checker = Identity::generate();
    let failed: Vec<&String> = third[..10]
        .iter()
        .chain(&third[third.len() - 10..])
        .copied()
        .collect();
    let labels: Vec<Event> = failed
        .iter()
        .map(|id| {
            check_event(
                checker.signer(),
                Verdict::Fail,
                &by_id[*id],
                &"0".repeat(64),
                "redundant exact-match",
                pylon::now(),
            )
            .unwrap()
        })
        .collect();
    let checked: Vec<Event> = failed.iter().map(|id| by_id[*id].clone()).collect();
    // An untrusted checker forfeits nothing.
    assert!(
        broker
            .forfeit(&labels, &checked, &BTreeSet::new(), start_at + 2)
            .unwrap()
            .is_empty()
    );
    let trusted = BTreeSet::from([checker.pubkey().to_string()]);
    let forfeits = broker
        .forfeit(&labels, &checked, &trusted, start_at + 2)
        .unwrap();
    assert_eq!(forfeits.len(), 20);
    assert_eq!(
        forfeits.iter().map(|a| a.reduced_msat).sum::<i64>(),
        10 * 8_500
    );
    assert_eq!(
        forfeits.iter().map(|a| a.loss_msat).sum::<i64>(),
        10 * 8_500
    );

    // Ten minutes on, the second half sweeps.
    sweep(&mut broker, start_at + 2 + 600);
    let jobs = broker.ledger().pylon_jobs().unwrap();
    assert_eq!(jobs.len(), JOBS);

    // Ledger rows match receipts: one settlement per receipt, keyed by the
    // receipt's payment hash, at the receipt's price.
    let rows: BTreeMap<&str, &pay_ledger::pylon::Job> =
        jobs.iter().map(|j| (j.receipt.as_str(), j)).collect();
    assert_eq!(rows.len(), JOBS);
    for id in &receipts {
        let receipt = parse_receipt(&by_id[id], None).unwrap();
        let payment = receipt.payment.unwrap();
        let row = rows[id.as_str()];
        assert_eq!(row.settlement, payment.payment_hash);
        assert_eq!(row.received_msat, 10_000);
        assert_eq!(row.provider, party(&receipt.provider, None));
        assert_eq!(row.provider_msat, 8_500);
        let forfeited = failed[10..].contains(&id);
        assert_eq!(
            row.forfeited_msat,
            if forfeited { 8_500 } else { 0 },
            "{id}"
        );
        assert_eq!(row.paid_msat, if forfeited { 0 } else { 8_500 }, "{id}");
    }

    // Balance sweeps, never per job: six payouts for 1,000 jobs, and they
    // drained exactly the unforfeited shares.
    let payouts = rails.0.lock().unwrap().clone();
    assert_eq!(payouts.len(), 6);
    let drained: i64 = broker
        .ledger()
        .payouts(None)
        .unwrap()
        .iter()
        .map(|p| p.amount_msat)
        .sum();
    assert_eq!(drained, (JOBS as i64 - 10) * 8_500);
    for key in &keys {
        assert_eq!(broker.ledger().accrued(key.pubkey()).unwrap(), 0);
    }

    // A mainnet book never sweeps without the owner's grant, and a grant's
    // ceilings hold every payout under them.
    let mut mainnet = Broker::open(
        Ledger::in_memory().unwrap(),
        Network::Bitcoin,
        broker_key.pubkey(),
    )
    .unwrap();
    // A regtest receipt never settles in the mainnet book.
    assert!(
        mainnet
            .settle(&by_id[&receipts[0]], None, 10_000, start_at)
            .unwrap_err()
            .contains("regtest")
    );
    let mut none = |_: &mut Ledger, _: &str, _: i64| Ok(None);
    let mut id = || "m".to_string();
    assert!(
        mainnet
            .sweep(&rails, start_at, None, &mut none, &mut id)
            .is_err()
    );
    assert!(
        mainnet
            .sweep(
                &rails,
                start_at,
                Some(Grant {
                    per_payment_msat: 1,
                    daily_msat: 1
                }),
                &mut none,
                &mut id
            )
            .is_ok()
    );

    for (stop, task) in running {
        stop.send(()).unwrap();
        task.await.unwrap().unwrap();
    }
}
