#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::tests::{NOW, beacon, receipt, signer};
use super::*;
use crate::identity::Identity;
use crate::market::{Service, offering};
use crate::paid::Network;
use nostr::pylon::beacon_event;

#[test]
fn the_market_counts_broker_sales_threads_and_valid_offerings() {
    let provider = signer(1);
    let buyer = signer(3);
    let broker = signer(5);
    let mut live = Live::new(Some("everglade"))
        .trusting_brokers(BTreeSet::from([broker.pubkey().to_string()]));
    live.offer(
        beacon_event(&provider, &beacon(&provider, NOW, 2)).unwrap(),
        NOW,
    );
    assert!(live.offer(receipt(&buyer, &provider, &"aa".repeat(32), NOW - 600), NOW));
    assert!(live.offer(
        receipt(&broker, &provider, &"bb".repeat(32), NOW - 600),
        NOW
    ));
    assert!(live.offer(receipt(&broker, &provider, &"cc".repeat(32), NOW - 5), NOW));

    let seller = Identity::generate();
    let service = Service {
        offer: "plan-review".into(),
        summary: "Plan review".into(),
        price_msat: 25_000,
        network: Network::Testnet,
    };
    let first = offering(&seller, &service, NOW - 100, NOW + 3_600).unwrap();
    let newer = offering(&seller, &service, NOW - 50, NOW + 3_600).unwrap();
    assert!(live.offer(first.clone(), NOW));
    assert!(live.offer(newer.clone(), NOW));
    // An older offering for the same service does not replace the newer.
    assert!(!live.offer(first, NOW));
    let mut tampered = newer.clone();
    tampered.content = tampered.content.replace("25000", "1");
    assert!(!live.offer(tampered, NOW));

    let market = live.market(NOW);
    assert_eq!(market.jobs, 3);
    assert_eq!(market.sales, 2);
    assert_eq!(market.threads.len(), 1);
    assert!(!market.threads[0].mainnet);
    assert_eq!(market.listings.len(), 1);
    assert_eq!(market.listings[0].id, newer.id);
    // Threads fade after THREAD_SECS; offerings leave at their expiry.
    assert!(live.market(NOW + THREAD_SECS + 10).threads.is_empty());
    live.settle(NOW + 3_601);
    assert!(live.market(NOW + 3_601).listings.is_empty());
}
