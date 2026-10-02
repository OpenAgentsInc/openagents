use super::*;
use tower::ServiceExt;

fn amount(msat: u64) -> Sats {
    Sats::from_msat(msat)
}

fn record(kind: EventType, at: i64) -> SourceRecord {
    SourceRecord {
        source: format!("private-{kind:?}-{at}"),
        at,
        kind,
        resource: Resource::Plugin,
        plugin: Some("explain-error".into()),
        node: "plugin:explain-error".into(),
        amount_sats: (kind != EventType::Call).then_some(amount(31_000)),
        rail: Some(Rail::Lightning),
        split: if kind == EventType::Call {
            BTreeMap::new()
        } else {
            BTreeMap::from([
                (Role::Author, amount(10_000)),
                (Role::Openagents, amount(21_000)),
            ])
        },
        author_identity: Some("a".repeat(64)),
        published_author_npub: None,
        payer_identity: Some("b".repeat(64)),
    }
}

fn store() -> Store {
    Store::from_connection(Connection::open_in_memory().unwrap(), [7; 32]).unwrap()
}

#[test]
fn every_event_type_is_private() {
    let mut store = store();
    for kind in [
        EventType::Call,
        EventType::Payment,
        EventType::Share,
        EventType::Payout,
        EventType::Bonus,
        EventType::Run,
    ] {
        let event = store.record(record(kind, 1)).unwrap();
        let json = serde_json::to_string(&event).unwrap();
        for forbidden in [
            "request_hash",
            "payment_hash",
            "invoice",
            "destination",
            "ip",
            "text",
            "payer_identity",
            "author_identity",
            "private-",
        ] {
            assert!(!json.contains(&format!("\"{forbidden}")), "{json}");
        }
        assert!(
            !json
                .as_bytes()
                .windows(64)
                .any(|w| w.iter().all(u8::is_ascii_hexdigit)),
            "{json}"
        );
        assert!(event.author.unwrap().starts_with("author-"));
        assert!(event.payer.unwrap().starts_with("caller-"));
    }
}

#[test]
fn identifiers_cannot_publish_private_values_or_unknown_nodes() {
    let mut s = store();
    for value in [
        "a".repeat(64),
        "192.0.2.1".into(),
        "https://example.com".into(),
        "alice@example.com".into(),
        "npub1abcdef".into(),
        "nsec1abcdef".into(),
        "lnbc123invoice".into(),
        "LNURL1ABC".into(),
        "spark1destination".into(),
        "bc1destination".into(),
        "package/npub1abcdef".into(),
        "private message".into(),
    ] {
        let mut invalid = record(EventType::Payment, 2);
        invalid.plugin = Some(value.clone());
        invalid.node = format!("plugin:{value}");
        assert!(
            s.record(invalid).is_err(),
            "Accepted private plugin {value}"
        );
        let mut invalid = record(EventType::Payment, 2);
        invalid.node = value.clone();
        assert!(s.record(invalid).is_err(), "Accepted private node {value}");
    }
    let mut unknown = record(EventType::Call, 2);
    unknown.plugin = None;
    unknown.node = "route:not-a-committed-route".into();
    assert!(s.record(unknown).is_err());
    let mut known = record(EventType::Call, 3);
    known.plugin = None;
    known.node = "coder".into();
    assert_eq!(s.record(known).unwrap().node, "coder");
    let mut package = record(EventType::Call, 4);
    package.plugin = Some("crates/plugin-example".into());
    package.node = "plugin:crates/plugin-example".into();
    assert!(s.record(package).is_ok());
}

#[test]
fn amounts_round_trip_exactly_without_float_rounding() {
    for (msat, json) in [
        (0, "0"),
        (1, "0.001"),
        (10, "0.01"),
        (1000, "1"),
        (1001, "1.001"),
        (u64::MAX, "18446744073709551.615"),
    ] {
        let value = amount(msat);
        assert_eq!(serde_json::to_string(&value).unwrap(), json);
        assert_eq!(serde_json::from_str::<Sats>(json).unwrap().msat(), msat);
    }
    for (json, msat) in [
        ("1e-3", 1),
        ("1.2300", 1230),
        ("123e-2", 1230),
        ("1e3", 1_000_000),
    ] {
        assert_eq!(serde_json::from_str::<Sats>(json).unwrap().msat(), msat);
    }
    for invalid in [
        "-1",
        "-0.001",
        "0.0001",
        "1e-4",
        "1.0001",
        "18446744073709551.616",
        "18446744073709552",
        "1e999",
        "\"1\"",
        "null",
    ] {
        assert!(
            serde_json::from_str::<Sats>(invalid).is_err(),
            "Accepted {invalid}"
        );
    }
}

#[test]
fn aliases_rotate_and_ingestion_is_idempotent() {
    let mut s = store();
    let first = s.record(record(EventType::Payment, 1)).unwrap();
    let repeat = s.record(record(EventType::Payment, 1)).unwrap();
    assert_eq!(first.seq, repeat.seq);
    let day = s.record(record(EventType::Payment, 86_400_001)).unwrap();
    assert_ne!(first.payer, day.payer);
    assert_eq!(first.author, day.author);
    assert_eq!(s.since(0).unwrap().len(), 2);
}

#[test]
fn reopening_requires_the_original_alias_salt() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("flow.sqlite");
    let first = {
        let mut store = Store::open(&path, [7; 32]).unwrap();
        store.record(record(EventType::Payment, 1)).unwrap()
    };
    assert!(Store::open(&path, [8; 32]).is_err());
    let mut reopened = Store::open(&path, [7; 32]).unwrap();
    let next = reopened.record(record(EventType::Payment, 2)).unwrap();
    assert_eq!(first.author, next.author);
    assert_eq!(first.payer, next.payer);
}

#[test]
fn stats_count_recipient_earnings_and_all_outbound_funds_once() {
    let mut s = store();
    let events = [
        (EventType::Payment, 1, 2_000_000, None),
        (EventType::Share, 2, 500_000, Some(Role::Author)),
        (EventType::Share, 3, 1_500_000, Some(Role::Openagents)),
        (EventType::Bonus, 4, 1_000_000, Some(Role::Bonus)),
        (EventType::Payout, 5, 750_000, Some(Role::Author)),
        (EventType::Payout, 6, 500_000, Some(Role::Openagents)),
    ];
    let mut author = None;
    for (kind, at, msat, role) in events {
        let mut input = record(kind, at);
        input.amount_sats = Some(amount(msat));
        input.split = role
            .map(|role| BTreeMap::from([(role, amount(msat))]))
            .unwrap_or_default();
        let event = s.record(input).unwrap();
        author = event.author;
    }
    s.record(record(EventType::Call, 7)).unwrap();
    s.set_reconciliation(Reconciliation::Drift).unwrap();
    let stats = s.stats(8).unwrap();
    assert_eq!(stats.totals.received_sats.msat(), 2_000_000);
    assert_eq!(stats.totals.earnings_sats.msat(), 1_500_000);
    assert_eq!(stats.totals.paid_out_sats.msat(), 1_250_000);
    assert_eq!(stats.totals.pending_accruals_sats.msat(), 750_000);
    assert_eq!(stats.totals.calls, 1);
    for totals in [
        &stats.per_plugin["explain-error"],
        &stats.per_author[&author.unwrap()],
    ] {
        assert_eq!(totals.earnings_sats.msat(), 1_500_000);
        assert_eq!(totals.paid_out_sats.msat(), 750_000);
        assert_eq!(totals.pending_accruals_sats.msat(), 750_000);
    }
    assert_eq!(stats.series_24h.len(), 24);
    assert_eq!(stats.series_30d.len(), 30);
    assert_eq!(
        stats.series_24h.last().unwrap().totals.earnings_sats.msat(),
        1_500_000
    );
    assert_eq!(stats.reconciliation, "drift");
}

#[tokio::test]
async fn seeded_stream_resumes_across_pages_and_follows_new_records() {
    use futures_util::StreamExt;
    let mut s = store();
    for at in 1..=502 {
        s.record(record(EventType::Payment, at)).unwrap();
    }
    assert_eq!(s.since(0).unwrap().len(), 500);
    assert_eq!(s.since(500).unwrap().len(), 2);
    let s = Arc::new(Mutex::new(s));
    let response = router(s.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/flow/stream")
                .header("last-event-id", "1")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.headers()["content-type"], "text/event-stream");
    let mut body = response.into_body().into_data_stream();
    for id in 2..=502 {
        let bytes = tokio::time::timeout(std::time::Duration::from_secs(2), body.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let text = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(text.contains(&format!("id: {id}\n")), "{text}");
        assert!(text.contains(&format!("\"seq\":{id}")));
    }
    s.lock()
        .unwrap()
        .record(record(EventType::Call, 503))
        .unwrap();
    let bytes = tokio::time::timeout(std::time::Duration::from_secs(2), body.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(
        String::from_utf8(bytes.to_vec())
            .unwrap()
            .contains("id: 503\n")
    );
}

#[tokio::test]
async fn snapshot_is_bounded_and_places_dynamic_plugins_with_shared_layout() {
    let mut s = store();
    for at in 1..=501 {
        s.record(record(EventType::Call, at)).unwrap();
    }
    let map = s.topology_map().unwrap();
    let layout = openagents_chat_app::route_map::layout::Layout::of(&map);
    let dynamic = map
        .nodes
        .iter()
        .find(|node| node.id == "plugin:explain-error")
        .unwrap();
    assert_eq!(map.nodes[dynamic.parent.unwrap()].id, "coder");
    let response = router(Arc::new(Mutex::new(s)))
        .oneshot(
            axum::http::Request::builder()
                .uri("/flow/snapshot")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["events"].as_array().unwrap().len(), 500);
    assert_eq!(value["events"][0]["seq"], 2);
    assert_eq!(value["totals"]["calls"], 501);
    assert_eq!(value["topology"].as_array().unwrap().len(), map.nodes.len());
    for (i, node) in map.nodes.iter().enumerate() {
        assert_eq!(value["topology"][i]["id"], node.id);
        for (axis, expected) in [("x", layout.positions[i].x), ("y", layout.positions[i].y)] {
            let actual = value["topology"][i]["position"][axis].as_f64().unwrap();
            assert_eq!(actual as f32, expected, "{axis} of {}", node.id);
        }
    }
    for event in value["events"].as_array().unwrap() {
        assert!(
            value["topology"]
                .as_array()
                .unwrap()
                .iter()
                .any(|node| node["id"] == event["node"])
        );
    }
}

#[test]
fn published_authors_need_valid_npub_checksums_and_calls_have_no_amounts() {
    let mut s = store();
    let mut published = record(EventType::Share, 1);
    let npub = nostr::nip19::encode_npub(&[7; 32]);
    published.published_author_npub = Some(npub.clone());
    assert_eq!(
        s.record(published).unwrap().author.as_deref(),
        Some(npub.as_str())
    );
    let mut invalid = record(EventType::Share, 2);
    invalid.published_author_npub = Some(format!("npub1{}", "q".repeat(58)));
    assert!(s.record(invalid).is_err());
    let call = serde_json::to_value(s.record(record(EventType::Call, 3)).unwrap()).unwrap();
    assert!(call.get("amount_sats").is_none());
    assert!(call.get("split").is_none());
    let mut invalid = record(EventType::Call, 4);
    invalid.amount_sats = Some(amount(1));
    assert!(s.record(invalid).is_err());
    let mut invalid = record(EventType::Call, 5);
    invalid.split.insert(Role::Author, amount(1));
    assert!(s.record(invalid).is_err());
}

#[tokio::test]
async fn resume_ids_survive_restart_and_invalid_ids_are_rejected() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("flow.sqlite");
    {
        let mut s = Store::open(&path, [7; 32]).unwrap();
        s.record(record(EventType::Payment, 1)).unwrap();
        s.record(record(EventType::Payment, 2)).unwrap();
    }
    let s = Arc::new(Mutex::new(Store::open(&path, [7; 32]).unwrap()));
    assert_eq!(s.lock().unwrap().since(1).unwrap()[0].seq, 2);
    for id in [
        "invalid",
        "-1",
        "18446744073709551615",
        "9223372036854775808",
    ] {
        let response = router(s.clone())
            .oneshot(
                axum::http::Request::builder()
                    .uri("/flow/stream")
                    .header("Last-Event-ID", id)
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
    }
}
