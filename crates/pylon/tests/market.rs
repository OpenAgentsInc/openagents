#![allow(clippy::unwrap_used, clippy::expect_used)]
//! P4, the agent market on pooled compute, against the in-process relay:
//! Victor offers a priced service under NIP-MKT, Alice hires him through
//! a NIP-LAB order both desks check, the order's compute runs on a pylon
//! the broker buys it from, Alice pays the order's price to OpenAgents'
//! receiver after she accepts the answer, and the split ledger shows
//! Victor's fee, the provider's share, and OpenAgents' share tied to the
//! job's `3201` receipt.
//!
//! All sats are worthless testnet sats in memory (`TestLightning`);
//! nothing touches a wallet or a live relay.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use nostr::market_contracts::Ingest;
use nostr::pylon::parse_receipt;
use openagents_x402::FileReplayStore;
use pay_ledger::{Ledger, OPENAGENTS};
use pylon::broker::{Broker, party};
use pylon::fixture::{Oracle, relay};
use pylon::identity::Identity;
use pylon::market::{self, Desk, Hire, Service};
use pylon::paid::{Network, TestLightning, pay};
use pylon::provider::{Config, Provider};

/// The broker's price for one job, msat.
const COMPUTE_MSAT: u64 = 10_000;
/// Victor's price for one plan review, msat.
const PRICE_MSAT: u64 = 25_000;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn one_agent_hires_another_and_the_ledger_ties_every_share_to_the_receipt() {
    let (url, hub) = relay().await.unwrap();
    let home = tempfile::tempdir().unwrap();
    let broker_key = Identity::generate();
    let alice = Identity::generate();
    let victor = Identity::generate();

    // Two free pylons that serve only the broker.
    let mut pylons = Vec::new();
    for n in 0..2 {
        let key = Identity::generate();
        let mut config = Config::new(&url, &format!("pylon-{n}"), home.path().to_path_buf());
        config.allow = Some(BTreeSet::from([broker_key.pubkey().to_string()]));
        let provider = Provider::new(config, key.clone(), Arc::new(Oracle)).unwrap();
        tokio::spawn(Arc::clone(&provider).run(std::future::pending()));
        pylons.push(key.pubkey().to_string());
    }
    for _ in 0..200 {
        if hub
            .lock()
            .await
            .stored
            .iter()
            .filter(|e| e.kind == 30_200)
            .count()
            >= 2
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // Victor's offering is public and lists on the Agora's wall.
    let now = pylon::now();
    let service = Service {
        offer: "plan-review".into(),
        summary: "Victor reviews a day plan on pooled compute".into(),
        price_msat: PRICE_MSAT,
        network: Network::Testnet,
    };
    let offering = market::offering(&victor, &service, now, now + 3_600).unwrap();
    let listing = market::listing(&offering).unwrap();
    assert_eq!(listing.seller, victor.pubkey());
    assert_eq!(listing.price_msat, Some(PRICE_MSAT));
    assert!(listing.test());

    // Alice hires him: rfq, quote with exact terms, order, and ack, each
    // checked by both desks.
    let hire = Hire::new(
        &offering,
        alice.pubkey(),
        broker_key.pubkey(),
        "Review this day plan: standup, two sales calls, and a demo.",
        PRICE_MSAT,
        Network::Testnet,
        now,
    )
    .unwrap();
    let mut buyer = Desk::new(alice.clone(), hire.clone()).unwrap();
    let mut seller = Desk::new(victor.clone(), hire.clone()).unwrap();
    assert!(
        market::run(
            &broker_key,
            &seller,
            &url,
            None,
            home.path(),
            BTreeSet::new()
        )
        .await
        .is_err(),
        "no compute before the order is confirmed"
    );
    let rfq = hire.rfq(&alice, now).unwrap();
    let (quote, terms_doc) = hire.quote(&victor, &rfq, now).unwrap();
    let order = hire.order(&alice, &rfq, &quote, now).unwrap();
    let ack = hire.ack(&victor, &quote, &order, now).unwrap();
    for desk in [&mut buyer, &mut seller] {
        assert_eq!(desk.ingest(&rfq, None, now).unwrap(), Ingest::Applied);
        assert_eq!(
            desk.ingest(&quote, Some(&terms_doc), now).unwrap(),
            Ingest::Applied
        );
        assert_eq!(desk.ingest(&order, None, now).unwrap(), Ingest::Applied);
        assert!(desk.confirmed().is_none());
        assert_eq!(desk.ingest(&ack, None, now).unwrap(), Ingest::Applied);
    }
    let confirmed = seller.confirmed().unwrap().clone();
    assert_eq!(buyer.confirmed(), Some(&confirmed));
    assert_eq!(confirmed.buyer, alice.pubkey());
    assert_eq!(confirmed.provider, victor.pubkey());
    // A stranger's key opens nothing.
    assert!(market::open_record(&Identity::generate(), &order).is_err());

    // The order's compute runs on the pool, bought by the broker.
    let answer = market::run(
        &broker_key,
        &seller,
        &url,
        None,
        home.path(),
        BTreeSet::new(),
    )
    .await
    .unwrap();
    assert_eq!(market::accept(&answer).unwrap(), "Hi.");
    let receipt = answer.receipt_event.clone().unwrap();
    let parsed = parse_receipt(&receipt, None).unwrap();
    assert_eq!(parsed.buyer, broker_key.pubkey());
    assert!(pylons.contains(&parsed.provider));
    assert!(parsed.payment.is_none());

    // Alice accepts and pays the order's price to OpenAgents' receiver.
    let receiver = Arc::new(TestLightning::new(Network::Testnet).unwrap());
    let replay: pylon::broker::Replay =
        Arc::new(FileReplayStore::open(&home.path().join("broker-replay")).unwrap());
    let mut book = Broker::open(
        Ledger::in_memory().unwrap(),
        Network::Testnet,
        broker_key.pubkey(),
        replay,
    )
    .unwrap();
    let terms = hire.terms().unwrap();
    let invoice = market::instruction(receiver.as_ref(), &confirmed, &terms).unwrap();
    assert_eq!(invoice.amount_msat, PRICE_MSAT);
    // Before she pays, nothing settles.
    let unpaid = book.settle_order(
        receiver.as_ref(),
        &receipt,
        None,
        &confirmed,
        &terms,
        &invoice.bolt11,
        &"0".repeat(64),
        COMPUTE_MSAT,
        now as i64,
    );
    assert!(unpaid.is_err());
    let payment = pay(receiver.as_ref(), &invoice, Network::Testnet, PRICE_MSAT).unwrap();
    // A wrong preimage, or an invoice bound to another order, refuses.
    let mut wrong = payment.preimage.clone();
    wrong.replace_range(..1, if wrong.starts_with('0') { "1" } else { "0" });
    assert!(
        book.settle_order(
            receiver.as_ref(),
            &receipt,
            None,
            &confirmed,
            &terms,
            &invoice.bolt11,
            &wrong,
            COMPUTE_MSAT,
            now as i64,
        )
        .is_err()
    );
    let mut other = confirmed.clone();
    other.order_id = "ab".repeat(32);
    assert!(
        book.settle_order(
            receiver.as_ref(),
            &receipt,
            None,
            &other,
            &terms,
            &invoice.bolt11,
            &payment.preimage,
            COMPUTE_MSAT,
            now as i64,
        )
        .is_err()
    );
    let recorded = book
        .settle_order(
            receiver.as_ref(),
            &receipt,
            None,
            &confirmed,
            &terms,
            &invoice.bolt11,
            &payment.preimage,
            COMPUTE_MSAT,
            now as i64,
        )
        .unwrap();
    let share = |who: &str, role: &str| {
        recorded
            .shares
            .iter()
            .find(|s| s.party == who && s.role == role)
            .map_or(0, |s| s.amount_msat)
    };
    let provider = party(&parsed.provider, None);
    // Victor's fee is the price less the compute; the provider gets 85
    // percent of the compute and OpenAgents the rest.
    assert_eq!(share(victor.pubkey(), "author"), 15_000);
    assert_eq!(share(&provider, "provider"), 8_500);
    assert_eq!(share(OPENAGENTS, "openagents"), 1_500);
    let row = book
        .ledger()
        .agent_order(&confirmed.order_id)
        .unwrap()
        .unwrap();
    assert_eq!(row.receipt, receipt.id);
    assert_eq!(row.settlement, payment.payment_hash);
    assert_eq!(row.seller, victor.pubkey());
    assert_eq!(row.provider, provider);
    // The receipt is public: a reader matches the shares to it.
    assert!(
        hub.lock()
            .await
            .stored
            .iter()
            .any(|e| e.id == row.receipt && e.kind == 3_201)
    );
    // A replay is the same settlement.
    assert_eq!(
        book.settle_order(
            receiver.as_ref(),
            &receipt,
            None,
            &confirmed,
            &terms,
            &invoice.bolt11,
            &payment.preimage,
            COMPUTE_MSAT,
            now as i64,
        )
        .unwrap(),
        recorded
    );
}

#[test]
fn desks_refuse_mainnet_orders_and_strangers() {
    let now = pylon::now();
    let victor = Identity::generate();
    let alice = Identity::generate();
    let broker = Identity::generate();
    let mainnet = Service {
        offer: "plan-review".into(),
        summary: "Mainnet".into(),
        price_msat: PRICE_MSAT,
        network: Network::Bitcoin,
    };
    let offering = market::offering(&victor, &mainnet, now, now + 3_600).unwrap();
    let hire = Hire::new(
        &offering,
        alice.pubkey(),
        broker.pubkey(),
        "Hi.",
        PRICE_MSAT,
        Network::Bitcoin,
        now,
    )
    .unwrap();
    assert!(Desk::new(alice.clone(), hire).is_err());
    let testnet = Service {
        network: Network::Testnet,
        ..mainnet
    };
    let offering = market::offering(&victor, &testnet, now, now + 3_600).unwrap();
    let hire = Hire::new(
        &offering,
        alice.pubkey(),
        broker.pubkey(),
        "Hi.",
        PRICE_MSAT,
        Network::Testnet,
        now,
    )
    .unwrap();
    assert!(Desk::new(Identity::generate(), hire.clone()).is_err());
    // The seller cannot hire itself, and signet is not a market network.
    assert!(
        Hire::new(
            &offering,
            victor.pubkey(),
            broker.pubkey(),
            "Hi.",
            1,
            Network::Testnet,
            now
        )
        .is_err()
    );
    assert!(
        market::offering(
            &victor,
            &Service {
                network: Network::Signet,
                ..testnet
            },
            now,
            now + 60
        )
        .is_err()
    );
}
