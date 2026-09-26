use crate::{protocol::*, *};
use coder_history::{CatalogRequest, TranscriptRequest};
use secp256k1::SecretKey;
use std::path::PathBuf;

// Reuse the existing synthetic NIP-42/private-artifact fixture. It is not a
// production relay and does not claim production deployment or retention.
#[path = "../../../coder-control/src/tests/relay.rs"]
mod relay;

struct Fixture {
    _temp: tempfile::TempDir,
    root: PathBuf,
    state: PathBuf,
    client_secret: SecretKey,
    code: ConnectionCode,
    now: u64,
}
impl Fixture {
    fn new(url: &str) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("synthetic-source");
        let state = temp.path().join("connection");
        std::fs::create_dir_all(root.join("sessions/2026/01/01")).unwrap();
        std::fs::write(root.join("sessions/2026/01/01/one.jsonl"), b"{\"type\":\"session_meta\",\"payload\":{\"id\":\"synthetic-chat\"}}\n{\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"user\",\"content\":[{\"type\":\"input_text\",\"text\":\"Synthetic observation\"}]}}\n").unwrap();
        let client_secret = SecretKey::new(&mut secp256k1::rand::rng());
        let now = unix_time().unwrap();
        let host = host::Host::new(&state, RelayPolicy::LoopbackTest);
        let code = host
            .pair(
                &pubkey(&client_secret),
                url,
                coder_history::Config {
                    codex: Some(root.clone()),
                    claude: None,
                },
                now,
                now + 3600,
            )
            .unwrap();
        Self {
            _temp: temp,
            root,
            state,
            client_secret,
            code,
            now,
        }
    }
    fn host(&self) -> host::Host {
        host::Host::new(&self.state, RelayPolicy::LoopbackTest)
    }
    fn client(&self) -> Client {
        Client::new_with_policy(
            self.code.clone(),
            self.client_secret,
            RelayPolicy::LoopbackTest,
        )
        .unwrap()
    }
    fn catalog(&self) -> client::Pending {
        self.client()
            .prepare(Query::Catalog(CatalogRequest::default()), self.now)
            .unwrap()
    }
}

#[test]
fn connection_pins_identity_scope_and_expiry_without_disclosing_paths() {
    let f = Fixture::new("wss://relay.example/");
    let bytes = serde_json::to_vec(&f.code).unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains(f.root.to_str().unwrap()));
    let parsed = ConnectionCode::parse(&bytes).unwrap();
    assert!(
        parsed
            .verify(&f.client_secret, f.now, RelayPolicy::Production)
            .is_ok()
    );
    assert_eq!(
        parsed
            .verify(&f.client_secret, f.now + 3600, RelayPolicy::Production)
            .unwrap_err()
            .code,
        ErrorCode::Expired
    );
    let stranger = SecretKey::new(&mut secp256k1::rand::rng());
    assert_eq!(
        parsed
            .verify(&stranger, f.now, RelayPolicy::Production)
            .unwrap_err()
            .code,
        ErrorCode::Forbidden
    );
    let mut changed = parsed.clone();
    changed.relay = "wss://different.example/".into();
    assert!(
        changed
            .verify(&f.client_secret, f.now, RelayPolicy::Production)
            .is_err()
    );
    let mut changed = parsed;
    changed.sources[0].label = "Changed disclosure".into();
    assert!(
        changed
            .verify(&f.client_secret, f.now, RelayPolicy::Production)
            .is_err()
    );
}

#[test]
fn catalog_page_cursor_and_exact_request_correlation() {
    let f = Fixture::new("wss://relay.example/");
    let client = f.client();
    let pending = f.catalog();
    let response = f
        .host()
        .handle(&pending.event, &f.code.relay, f.now)
        .unwrap();
    let Observation::Catalog(catalog) = client.verify_reply(&pending, &response, f.now).unwrap()
    else {
        panic!("catalog expected")
    };
    assert_eq!(catalog.entries.len(), 1);
    let next = f.catalog();
    assert_eq!(
        client
            .verify_reply(&next, &response, f.now)
            .unwrap_err()
            .code,
        ErrorCode::Forbidden
    );
    assert_eq!(
        f.host()
            .handle(&pending.event, &f.code.relay, f.now)
            .unwrap()
            .id,
        response.id
    );
    let source_id = catalog.entries[0].source_id.clone().unwrap();
    let pending = client
        .prepare(
            Query::Page(TranscriptRequest {
                source_id,
                cursor: None,
                max_bytes: coder_history::MAX_PAGE_BYTES,
            }),
            f.now,
        )
        .unwrap();
    let response = f
        .host()
        .handle(&pending.event, &f.code.relay, f.now)
        .unwrap();
    let Observation::Page(page) = client.verify_reply(&pending, &response, f.now).unwrap() else {
        panic!("page expected")
    };
    assert!(!page.chunks.is_empty());
    assert_eq!(page.next.offset, page.snapshot_bytes);
    assert!(!page.has_more);
}

#[test]
fn durable_revocation_blocks_cached_retries_and_source_revocation_ends_grant() {
    let f = Fixture::new("wss://relay.example/");
    let pending = f.catalog();
    f.host()
        .handle(&pending.event, &f.code.relay, f.now)
        .unwrap();
    f.host()
        .revoke(&f.code.grant, Some(&f.code.sources[0].id), f.now)
        .unwrap();
    let reply = f
        .host()
        .handle(&pending.event, &f.code.relay, f.now)
        .unwrap();
    assert_eq!(
        f.client()
            .verify_reply(&pending, &reply, f.now)
            .unwrap_err()
            .code,
        ErrorCode::Revoked
    );
    assert!(f.host().relays(f.now).unwrap().is_empty());
    f.host().revoke(&f.code.grant, None, f.now).unwrap();
}

#[test]
fn failed_atomic_reply_save_returns_no_reply_and_reopen_reconciles_retained_state() {
    let f = Fixture::new("wss://relay.example/");
    let pending = f.catalog();
    let before = std::fs::read(f.state.join("observer.json")).unwrap();
    // A directory cannot be replaced as the private regular staging file. This
    // fails after the read but before any terminal reply can be published.
    let blocked = f.state.join(".observer.pending");
    std::fs::create_dir(&blocked).unwrap();
    assert!(
        f.host()
            .handle(&pending.event, &f.code.relay, f.now)
            .is_err()
    );
    assert_eq!(
        std::fs::read(f.state.join("observer.json")).unwrap(),
        before
    );
    std::fs::remove_dir(blocked).unwrap();
    let reply = f
        .host()
        .handle(&pending.event, &f.code.relay, f.now)
        .unwrap();
    assert!(f.client().verify_reply(&pending, &reply, f.now).is_ok());
    assert_eq!(
        f.host()
            .handle(&pending.event, &f.code.relay, f.now)
            .unwrap()
            .id,
        reply.id
    );
}

#[test]
fn active_store_lock_refuses_concurrent_administration_without_losing_revocation() {
    let f = Fixture::new("wss://relay.example/");
    let lock = crate::store::Store::open(&f.state, false).unwrap();
    assert_eq!(
        f.host()
            .revoke(&f.code.grant, None, f.now)
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );
    drop(lock);
    f.host().revoke(&f.code.grant, None, f.now).unwrap();
    assert!(f.host().relays(f.now).unwrap().is_empty());
}

#[test]
fn changed_root_and_unadmitted_source_refuse() {
    let f = Fixture::new("wss://relay.example/");
    let pending = f
        .client()
        .prepare(
            Query::Page(TranscriptRequest {
                source_id: "../auth.json".into(),
                cursor: None,
                max_bytes: 128,
            }),
            f.now,
        )
        .unwrap();
    let reply = f
        .host()
        .handle(&pending.event, &f.code.relay, f.now)
        .unwrap();
    assert!(f.client().verify_reply(&pending, &reply, f.now).is_err());
    std::fs::rename(&f.root, f.root.with_extension("old")).unwrap();
    std::fs::create_dir(&f.root).unwrap();
    let pending = f.catalog();
    let reply = f
        .host()
        .handle(&pending.event, &f.code.relay, f.now)
        .unwrap();
    assert_eq!(
        f.client()
            .verify_reply(&pending, &reply, f.now)
            .unwrap_err()
            .code,
        ErrorCode::SourceChanged
    );
}

#[test]
fn request_expiry_rechecked_after_read_and_rate_is_durable() {
    let f = Fixture::new("wss://relay.example/");
    let pending = f.catalog();
    let mut clock = [f.now, f.now + 60].into_iter();
    assert_eq!(
        f.host()
            .handle_with_clock(&pending.event, &f.code.relay, || Ok(clock.next().unwrap()))
            .unwrap_err()
            .code,
        ErrorCode::Expired
    );
    // Seed a retained near-limit window rather than repeating hundreds of
    // equivalent cryptographic reads. Reopening must honor this durable count.
    {
        let mut store = crate::store::Store::open(&f.state, false).unwrap();
        let mut book: serde_json::Value = store.load().unwrap().unwrap();
        book["admissions"][&f.code.grant]["reads"] =
            serde_json::json!(host::MAX_READS_PER_MINUTE - 1);
        store.save(&book).unwrap();
    }
    let pending = f.catalog();
    let reply = f
        .host()
        .handle(&pending.event, &f.code.relay, f.now)
        .unwrap();
    assert!(f.client().verify_reply(&pending, &reply, f.now).is_ok());
    let pending = f.catalog();
    let reply = f
        .host()
        .handle(&pending.event, &f.code.relay, f.now)
        .unwrap();
    assert_eq!(
        f.client()
            .verify_reply(&pending, &reply, f.now)
            .unwrap_err()
            .code,
        ErrorCode::RateLimited
    );
}

#[test]
fn strict_destinations_schemas_and_private_store_permissions() {
    for bad in [
        "ws://127.0.0.1:1234/",
        "http://relay.example/",
        "wss://name:secret@relay.example/",
        "wss://relay.example/#secret",
        "wss://relay.example/?token=secret",
    ] {
        assert!(RelayPolicy::Production.validate(bad).is_err());
    }
    for bad in [
        "ws://relay.example/",
        "ws://localhost/",
        "ws://192.168.1.4/",
    ] {
        assert!(RelayPolicy::LoopbackTest.validate(bad).is_err());
    }
    assert!(
        RelayPolicy::Production
            .validate("wss://relay.example")
            .is_ok()
    );
    let f = Fixture::new("wss://relay.example/");
    let bytes = serde_json::to_vec(&f.code).unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    value["mutation"] = serde_json::json!(true);
    assert!(ConnectionCode::parse(&serde_json::to_vec(&value).unwrap()).is_err());
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(
        f.state.join("observer.json"),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert_eq!(
        f.host().relays(f.now).unwrap_err().code,
        ErrorCode::Forbidden
    );
}

#[test]
fn signed_corrupt_base64_or_offsets_are_refused_by_client() {
    let f = Fixture::new("wss://relay.example/");
    let cat = f.catalog();
    let response = f.host().handle(&cat.event, &f.code.relay, f.now).unwrap();
    let Observation::Catalog(page) = f.client().verify_reply(&cat, &response, f.now).unwrap()
    else {
        panic!()
    };
    let pending = f
        .client()
        .prepare(
            Query::Page(TranscriptRequest {
                source_id: page.entries[0].source_id.clone().unwrap(),
                cursor: None,
                max_bytes: 8192,
            }),
            f.now,
        )
        .unwrap();
    let response = f
        .host()
        .handle(&pending.event, &f.code.relay, f.now)
        .unwrap();
    let mut reply: Reply = open(
        &response,
        &f.client_secret,
        &f.code.host,
        &f.code.client,
        REPLY,
    )
    .unwrap();
    let ReplyResult::Ok {
        ref mut observation,
    } = reply.result
    else {
        panic!()
    };
    let Observation::Page(page) = observation.as_mut() else {
        panic!()
    };
    page.chunks[0].raw_base64 = "invalid!".into();
    let forged = seal(
        &reply,
        REPLY,
        &f.host().key().unwrap(),
        &f.code.client,
        &pending.request.request,
        f.now,
        pending.request.expires_at,
    )
    .unwrap();
    assert_eq!(
        f.client()
            .verify_reply(&pending, &forged, f.now)
            .unwrap_err()
            .code,
        ErrorCode::Malformed
    );
}

#[test]
fn signed_transcript_record_regrouping_and_false_cursors_are_refused() {
    let f = Fixture::new("wss://relay.example/");
    let client = f.client();
    let catalog = f.catalog();
    let response = f
        .host()
        .handle(&catalog.event, &f.code.relay, f.now)
        .unwrap();
    let Observation::Catalog(page) = client.verify_reply(&catalog, &response, f.now).unwrap()
    else {
        panic!()
    };
    let pending = client
        .prepare(
            Query::Page(TranscriptRequest {
                source_id: page.entries[0].source_id.clone().unwrap(),
                cursor: None,
                max_bytes: coder_history::MAX_PAGE_BYTES,
            }),
            f.now,
        )
        .unwrap();
    let response = f
        .host()
        .handle(&pending.event, &f.code.relay, f.now)
        .unwrap();
    let original: Reply = open(
        &response,
        &f.client_secret,
        &f.code.host,
        &f.code.client,
        REPLY,
    )
    .unwrap();
    for mutation in 0..7 {
        let mut reply = original.clone();
        let ReplyResult::Ok { observation } = &mut reply.result else {
            panic!()
        };
        let Observation::Page(page) = observation.as_mut() else {
            panic!()
        };
        match mutation {
            0 => page.chunks[0].id = "0".repeat(64),
            1 => page.chunks[0].index += 1,
            2 => page.chunks[1].record_offset = 0,
            3 => page.next.record_index += 1,
            4 => page.next.record_offset = 0,
            5 => page.has_more = !page.has_more,
            6 => page.pending_line = !page.pending_line,
            _ => unreachable!(),
        }
        let signed = seal(
            &reply,
            REPLY,
            &f.host().key().unwrap(),
            &f.code.client,
            &pending.request.request,
            f.now,
            pending.request.expires_at,
        )
        .unwrap();
        assert!(
            client.verify_reply(&pending, &signed, f.now).is_err(),
            "mutation {mutation}"
        );
    }
}

/// Explicit operator smoke only. Sends generated fixture text and throwaway keys.
/// It does not open the operator's history or execute a model or agent.
#[tokio::test]
#[ignore = "requires an explicitly selected production relay and synthetic publication"]
async fn production_relay_reads_only_generated_history() {
    let url = std::env::var("CODER_CONNECT_SYNTHETIC_RELAY")
        .expect("set CODER_CONNECT_SYNTHETIC_RELAY to the operator-selected wss relay");
    RelayPolicy::Production.validate(&url).unwrap();
    let f = Fixture::new(&url);
    let client = Client::new(f.code.clone(), f.client_secret).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(25), async {
        let mut receiver =
            transport::Receiver::connect(&url, &f.host().key().unwrap(), RelayPolicy::Production)
                .await
                .unwrap();
        let (catalog, ()) = tokio::join!(
            client.observe(Query::Catalog(CatalogRequest::default())),
            async {
                let incoming = receiver.next_request().await.unwrap();
                let reply = f.host().handle_current(&incoming, &url).unwrap();
                receiver.publish(&reply).await.unwrap();
            }
        );
        let Observation::Catalog(page) = catalog.unwrap() else {
            panic!()
        };
        assert_eq!(page.entries.len(), 1);
        let (transcript, ()) = tokio::join!(
            client.observe(Query::Page(TranscriptRequest {
                source_id: page.entries[0].source_id.clone().unwrap(),
                cursor: None,
                max_bytes: coder_history::MAX_PAGE_BYTES,
            })),
            async {
                let incoming = receiver.next_request().await.unwrap();
                let reply = f.host().handle_current(&incoming, &url).unwrap();
                receiver.publish(&reply).await.unwrap();
            }
        );
        let Observation::Page(page) = transcript.unwrap() else {
            panic!()
        };
        assert_eq!(page.next.offset, page.snapshot_bytes);
        assert_eq!(page.chunks.len(), 2);
    })
    .await
    .expect("synthetic production exchange exceeded 25 seconds");
}

#[tokio::test]
async fn authenticated_encrypted_relay_roundtrip_uses_synthetic_data_only() {
    let (url, relay, events) = relay::start().await;
    let f = Fixture::new(&url);
    let mut receiver =
        transport::Receiver::connect(&url, &f.host().key().unwrap(), RelayPolicy::LoopbackTest)
            .await
            .unwrap();
    let client = f.client();
    let (observed, _) = tokio::join!(
        client.observe(Query::Catalog(CatalogRequest::default())),
        async {
            let request = receiver.next_request().await.unwrap();
            let response = f.host().handle_current(&request, &url).unwrap();
            receiver.publish(&response).await.unwrap();
        }
    );
    assert!(matches!(observed.unwrap(), Observation::Catalog(_)));
    // A second request uses the same Client and its bounded reusable socket.
    let (observed, _) = tokio::join!(
        client.observe(Query::Catalog(CatalogRequest::default())),
        async {
            let request = receiver.next_request().await.unwrap();
            let response = f.host().handle_current(&request, &url).unwrap();
            receiver.publish(&response).await.unwrap();
        }
    );
    assert!(matches!(observed.unwrap(), Observation::Catalog(_)));
    assert!(
        events
            .lock()
            .await
            .values()
            .filter(|e| e.kind == 3188)
            .all(|e| !e.content.contains("Synthetic observation"))
    );
    drop(receiver);
    relay.abort();
    let _ = relay.await;
}

#[tokio::test]
async fn cancelled_exchange_discards_socket_before_the_next_observation() {
    let (url, relay, _) = relay::start().await;
    let f = Fixture::new(&url);
    let mut receiver =
        transport::Receiver::connect(&url, &f.host().key().unwrap(), RelayPolicy::LoopbackTest)
            .await
            .unwrap();
    let client = f.client();
    let mut first = Box::pin(client.observe(Query::Catalog(CatalogRequest::default())));
    let old_request = tokio::select! {
        result = &mut first => panic!("unexpected early result: {result:?}"),
        request = receiver.next_request() => request.unwrap(),
    };
    drop(first);
    let (observed, _) = tokio::join!(
        client.observe(Query::Catalog(CatalogRequest::default())),
        async {
            let request = receiver.next_request().await.unwrap();
            assert_ne!(request.id, old_request.id);
            let response = f.host().handle_current(&request, &url).unwrap();
            receiver.publish(&response).await.unwrap();
        }
    );
    assert!(matches!(observed.unwrap(), Observation::Catalog(_)));
    drop(client);
    drop(receiver);
    relay.abort();
    let _ = relay.await;
}

mod pairing;
