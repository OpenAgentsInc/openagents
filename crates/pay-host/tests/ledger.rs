use pay_host::{EventType, Resource, Store};
use pay_ledger::{
    CallRecord, Ledger, OPENAGENTS, Payee, PayoutState, Rail, SettlementInput, Split,
};
use serde_json::json;

const START: i64 = 1_792_022_400;
fn input(key: &str, plugin: Option<&str>, received: i64, split: Split) -> SettlementInput {
    SettlementInput {
        key: key.into(),
        resource: "https://192.0.2.1/private-request-text".into(),
        plugin_id: plugin.map(str::to_owned),
        release_id: None,
        price_msat: received,
        received_msat: received,
        rail: Rail::Lightning,
        payer_alias: Some("private-payer-account".into()),
        settled_at: START,
        split,
    }
}
fn publication() -> nostr::domain::Event {
    let signer = nostr::domain::RelaySigner::from_secret_hex(&"11".repeat(32)).unwrap();
    signer.sign(START as u64,nostr::ext::LISTING_KIND,vec![
        nostr::domain::Tag::new(vec!["t".into(),"oa:ext:listing:v1".into()]),
        nostr::domain::Tag::new(vec!["d".into(),"explain-error".into()]),
    ],json!({"v":1,"requires":[],"type":"listing","package":format!("{}:explain-error",signer.pubkey()),
        "state":"published","release":{"id":"ab".repeat(32),"pubkey":signer.pubkey(),"kind":3184}}).to_string())
}
fn private(wire: &str) {
    for forbidden in [
        "private-",
        "192.0.2.1",
        "https://",
        "payment_hash",
        "request_hash",
        "destination",
        "invoice",
    ] {
        assert!(!wire.contains(forbidden), "{wire}");
    }
    assert!(
        !wire
            .as_bytes()
            .windows(64)
            .any(|w| w.iter().all(u8::is_ascii_hexdigit)),
        "{wire}"
    );
}

#[test]
fn real_ledger_calls_count_challenge_and_execution_once_without_disclosing_the_package() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("ledger.sqlite");
    let flow_path = temp.path().join("flow.sqlite");
    let mut ledger = Ledger::open(&path).unwrap();
    let author = publication().pubkey;
    let plugin = format!("{author}:explain-error");
    let mut call = CallRecord {
        at: START,
        route: "private-invoke-route".into(),
        resource: "https://192.0.2.1/private-resource".into(),
        plugin_id: Some(plugin.clone()),
        release_id: Some("cd".repeat(32)),
        outcome: "challenged".into(),
        paid: false,
        price_msat: Some(6_000),
    };
    ledger.record_call(&call).unwrap();
    call.at += 1;
    call.outcome = "executed".into();
    call.paid = true;
    ledger.record_call(&call).unwrap();
    let mut payment = input(
        "private-payment-hash",
        Some(&plugin),
        6_000,
        Split::Plugin {
            author,
            fee_msat: 6_000,
        },
    );
    payment.release_id = call.release_id.clone();
    ledger.record_settlement(payment).unwrap();
    let source =
        rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let mut flow = Store::open(&flow_path, [7; 32]).unwrap();
    flow.sync_sources(&source).unwrap();
    let events = flow.since(0).unwrap();
    let calls = events
        .iter()
        .filter(|event| event.kind == EventType::Call)
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 2);
    assert_eq!(
        [calls[0].at, calls[1].at],
        [START * 1000, (START + 1) * 1000]
    );
    let public_plugin = calls[0].plugin.clone().unwrap();
    assert!(public_plugin.starts_with("plugin-"));
    for event in &calls {
        assert!(matches!(event.resource, Resource::Plugin));
        assert_eq!(event.plugin.as_deref(), Some(public_plugin.as_str()));
        assert_eq!(event.node, format!("plugin:{public_plugin}"));
        assert!(event.amount_sats.is_none());
        assert!(event.split.is_empty());
        assert!(event.rail.is_none());
        assert!(event.payer.is_none());
        let wire = serde_json::to_value(event).unwrap();
        for field in [
            "source",
            "route",
            "release",
            "release_id",
            "outcome",
            "paid",
            "price_msat",
        ] {
            assert!(wire.get(field).is_none());
        }
    }
    assert_eq!(
        events
            .iter()
            .filter(|event| event.kind == EventType::Payment)
            .count(),
        1
    );
    let stats = flow.stats((START + 1) * 1000).unwrap();
    assert_eq!(stats.totals.calls, 2);
    assert_eq!(stats.per_plugin[&public_plugin].calls, 2);
    assert_eq!(stats.totals.received_sats.msat(), 6_000);
    private(&serde_json::to_string(&events).unwrap());
    let cursor = events.last().unwrap().seq;
    drop(flow);
    // A crash can leave stored events ahead of the source cursor.
    let flow_db = rusqlite::Connection::open(&flow_path).unwrap();
    flow_db
        .execute(
            "UPDATE flow_cursor SET seq=0 WHERE source='ledger-call'",
            [],
        )
        .unwrap();
    drop(flow_db);
    let mut flow = Store::open(&flow_path, [7; 32]).unwrap();
    flow.sync_sources(&source).unwrap();
    assert!(flow.since(cursor).unwrap().is_empty());
    // A new, byte-identical usage row is another request, even at the same time.
    ledger.record_call(&call).unwrap();
    flow.sync_sources(&source).unwrap();
    flow.sync_sources(&source).unwrap();
    let appended = flow.since(cursor).unwrap();
    assert_eq!(appended.len(), 1);
    assert_eq!(appended[0].kind, EventType::Call);
    let stats = flow.stats((START + 1) * 1000).unwrap();
    assert_eq!(stats.totals.calls, 3);
    assert_eq!(stats.per_plugin[&public_plugin].calls, 3);
    assert_eq!(stats.totals.received_sats.msat(), 6_000);
}

#[test]
fn free_calls_without_plugin_metadata_do_not_infer_identity_or_money() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("ledger.sqlite");
    let mut ledger = Ledger::open(&path).unwrap();
    ledger
        .record_call(&CallRecord {
            at: START,
            route: "private-route".into(),
            resource: "https://192.0.2.1/private-resource".into(),
            plugin_id: None,
            release_id: Some("ab".repeat(32)),
            outcome: "caller_paid".into(),
            paid: false,
            price_msat: Some(0),
        })
        .unwrap();
    let source = rusqlite::Connection::open(&path).unwrap();
    let mut flow =
        Store::from_connection(rusqlite::Connection::open_in_memory().unwrap(), [7; 32]).unwrap();
    flow.sync_sources(&source).unwrap();
    let events = flow.since(0).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, EventType::Call);
    assert!(matches!(events[0].resource, Resource::Route));
    assert_eq!(events[0].node, "front");
    assert!(events[0].plugin.is_none());
    assert!(events[0].payer.is_none());
    assert!(events[0].amount_sats.is_none());
    private(&serde_json::to_string(&events).unwrap());
    let stats = flow.stats(START * 1000).unwrap();
    assert_eq!(stats.totals.calls, 1);
    assert_eq!(stats.totals.received_sats.msat(), 0);
    assert!(stats.per_plugin.is_empty());
}

#[tokio::test]
async fn real_ledger_bonus_and_payout_replay_are_exact_and_private() {
    use futures_util::StreamExt;
    use tower::ServiceExt;
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("ledger.sqlite");
    let flow_path = temp.path().join("flow.sqlite");
    let mut ledger = Ledger::open(&path).unwrap();
    ledger
        .record_settlement(input(
            "a-private-funding-hash",
            None,
            1_000_000,
            Split::OpenAgents,
        ))
        .unwrap();
    let author = publication().pubkey;
    ledger
        .record_settlement(input(
            &"a".repeat(64),
            Some("explain-error"),
            31_001,
            Split::Plugin {
                author: author.clone(),
                fee_msat: 10_001,
            },
        ))
        .unwrap();
    let source =
        rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let mut flow = Store::open(&flow_path, [7; 32]).unwrap();
    flow.sync_sources(&source).unwrap();
    let before = flow.since(0).unwrap();
    let cursor = before.last().unwrap().seq;
    assert_eq!(
        before
            .iter()
            .filter(|e| e.kind == EventType::Payment)
            .count(),
        2
    );
    assert_eq!(
        before.iter().filter(|e| e.kind == EventType::Bonus).count(),
        2
    );
    assert!(before.iter().all(|e| e.at == START * 1000));
    private(&serde_json::to_string(&before).unwrap());
    let stats = flow.stats(START * 1000).unwrap();
    assert_eq!(stats.totals.received_sats.msat(), 1_031_001);
    assert_eq!(stats.totals.earnings_sats.msat(), 1_020_002);
    assert_eq!(stats.totals.pending_accruals_sats.msat(), 1_031_001);
    assert_eq!(
        stats.per_plugin["explain-error"].earnings_sats.msat(),
        1_020_002
    );
    // Publication arriving after the first share still joins its payout to the
    // same author in stats, without rewriting the old public events.
    flow.register_publication(&publication()).unwrap();
    ledger
        .register_payee(Payee {
            party: author.clone(),
            destination_kind: "spark".into(),
            destination_value: "private-wallet-destination".into(),
            source: "account".into(),
            verified_at: START,
        })
        .unwrap();
    let claims = ledger.available_shares(&author).unwrap();
    ledger
        .reserve_payout("private-payout", &author, &claims, START)
        .unwrap();
    flow.sync_sources(&source).unwrap();
    assert!(flow.since(cursor).unwrap().is_empty());
    ledger
        .set_payout_state(
            "private-payout",
            PayoutState::Sent,
            Some("private-wallet-reference"),
            START + 1,
        )
        .unwrap();
    flow.sync_sources(&source).unwrap();
    let payouts = flow.since(cursor).unwrap();
    assert_eq!(payouts.len(), 3);
    assert!(payouts.iter().all(|e| e.kind == EventType::Payout));
    assert!(
        payouts
            .iter()
            .all(|e| e.author.as_deref().unwrap().starts_with("npub1"))
    );
    let stats = flow.stats((START + 1) * 1000).unwrap();
    assert_eq!(stats.totals.paid_out_sats.msat(), 1_020_002);
    assert_eq!(stats.totals.pending_accruals_sats.msat(), 10_999);
    assert_eq!(stats.per_author.len(), 1);
    assert_eq!(
        stats
            .per_author
            .values()
            .next()
            .unwrap()
            .pending_accruals_sats
            .msat(),
        0
    );
    assert_eq!(
        stats.per_plugin["explain-error"]
            .pending_accruals_sats
            .msat(),
        0
    );
    assert_eq!(
        serde_json::to_value(&stats).unwrap()["totals"]["pending_accruals_sats"].to_string(),
        "10.999"
    );
    assert_eq!(
        flow.since(0).unwrap()[..before.len()]
            .iter()
            .map(|e| e.seq)
            .collect::<Vec<_>>(),
        before.iter().map(|e| e.seq).collect::<Vec<_>>()
    );
    private(&serde_json::to_string(&payouts).unwrap());
    drop(flow);
    let mut flow = Store::open(&flow_path, [7; 32]).unwrap();
    flow.sync_sources(&source).unwrap();
    assert_eq!(flow.since(cursor).unwrap().len(), 3);
    let app = pay_host::router(std::sync::Arc::new(std::sync::Mutex::new(flow)));
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/flow/stream")
                .header("Last-Event-ID", cursor.to_string())
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let mut body = response.into_body().into_data_stream();
    for event in payouts {
        let bytes = tokio::time::timeout(std::time::Duration::from_secs(2), body.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let wire = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(wire.contains(&format!("id: {}\n", event.seq)), "{wire}");
        private(&wire);
    }
    assert_eq!(ledger.accrued(OPENAGENTS).unwrap(), 10_999);
}

#[test]
fn private_plugin_ids_are_aliased_and_unverified_publications_are_refused() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("ledger.sqlite");
    let mut ledger = Ledger::open(&path).unwrap();
    ledger
        .record_settlement(input(
            "private-hash",
            Some(&"b".repeat(64)),
            10_000,
            Split::Plugin {
                author: publication().pubkey,
                fee_msat: 10_000,
            },
        ))
        .unwrap();
    let source = rusqlite::Connection::open(&path).unwrap();
    let mut flow =
        Store::from_connection(rusqlite::Connection::open_in_memory().unwrap(), [7; 32]).unwrap();
    let mut tampered = publication();
    tampered.content.push(' ');
    assert!(flow.register_publication(&tampered).is_err());
    flow.sync_sources(&source).unwrap();
    let events = flow.since(0).unwrap();
    assert!(
        events
            .iter()
            .all(|e| e.plugin.as_deref().unwrap().starts_with("plugin-"))
    );
    assert!(
        events
            .iter()
            .filter_map(|e| e.author.as_deref())
            .all(|a| a.starts_with("author-"))
    );
    private(&serde_json::to_string(&events).unwrap());
}

#[test]
fn publishing_one_plugin_does_not_reveal_unrelated_author_earnings() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("ledger.sqlite");
    let mut ledger = Ledger::open(&path).unwrap();
    let author = publication().pubkey;
    ledger
        .record_settlement(input(
            "public-plugin",
            Some("explain-error"),
            10_000,
            Split::Plugin {
                author: author.clone(),
                fee_msat: 10_000,
            },
        ))
        .unwrap();
    ledger
        .record_settlement(input(
            "unpublished-plugin",
            Some("unpublished"),
            10_000,
            Split::Plugin {
                author: author.clone(),
                fee_msat: 10_000,
            },
        ))
        .unwrap();
    ledger
        .record_settlement(input(
            "hosted",
            None,
            10_000,
            Split::HostedResource { owner: author },
        ))
        .unwrap();
    let source = rusqlite::Connection::open(&path).unwrap();
    let mut flow =
        Store::from_connection(rusqlite::Connection::open_in_memory().unwrap(), [7; 32]).unwrap();
    flow.sync_sources(&source).unwrap();
    flow.register_publication(&publication()).unwrap();
    let stats = flow.stats(START * 1000).unwrap();
    assert_eq!(stats.per_author.len(), 2);
    let public = stats
        .per_author
        .iter()
        .find(|(id, _)| id.starts_with("npub1"))
        .unwrap()
        .1;
    let private = stats
        .per_author
        .iter()
        .find(|(id, _)| id.starts_with("author-"))
        .unwrap()
        .1;
    assert_eq!(public.earnings_sats.msat(), 10_000);
    assert_eq!(private.earnings_sats.msat(), 19_000);
}

#[test]
fn a_registered_registry_plugin_is_named_by_its_slug_on_its_topology_node() {
    // The pay front records a paid invoke under the registry id
    // `<publisher>:<slug>`. Once its signed listing is registered, the flow
    // names it by the slug, as the design's `"plugin":"explain-error"`, so
    // /live places the dot on the committed `plugin-explain-error` node and
    // the author shows by npub; the publisher's hex key never appears.
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("ledger.sqlite");
    let mut ledger = Ledger::open(&path).unwrap();
    let author = publication().pubkey;
    let plugin = format!("{author}:explain-error");
    ledger
        .record_call(&CallRecord {
            at: START,
            route: "plugin-invoke".into(),
            resource: "route:plugin-invoke".into(),
            plugin_id: Some(plugin.clone()),
            release_id: Some("cd".repeat(32)),
            outcome: "challenged".into(),
            paid: false,
            price_msat: Some(15_000),
        })
        .unwrap();
    ledger
        .record_settlement(input(
            "private-paid-invoke",
            Some(&plugin),
            15_000,
            Split::Plugin {
                author: author.clone(),
                fee_msat: 10_000,
            },
        ))
        .unwrap();
    let source =
        rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let mut flow =
        Store::from_connection(rusqlite::Connection::open_in_memory().unwrap(), [7; 32]).unwrap();
    flow.register_publication(&publication()).unwrap();
    flow.sync_sources(&source).unwrap();
    let events = flow.since(0).unwrap();
    assert!(!events.is_empty());
    for event in &events {
        assert_eq!(event.plugin.as_deref(), Some("explain-error"));
        assert_eq!(event.node, "plugin:explain-error");
    }
    let npub = nostr::nip19::encode_npub(&hex::decode(&author).unwrap().try_into().unwrap());
    assert!(
        events
            .iter()
            .filter(|e| e.kind == EventType::Share)
            .any(|e| e.author.as_deref() == Some(npub.as_str()))
    );
    let stats = flow.stats(START * 1000 + 1_000).unwrap();
    assert_eq!(stats.per_plugin["explain-error"].calls, 1);
    assert!(!serde_json::to_string(&events).unwrap().contains(&author));
}
