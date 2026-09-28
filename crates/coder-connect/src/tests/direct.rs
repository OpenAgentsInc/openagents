//! The direct tailnet transport carries the same sealed requests and
//! replies: every host check but the relay binding applies, a request ID is
//! read at most once on either route, direct bounds apply only to direct
//! replies, and nudges carry nothing a read did not already disclose.
use super::*;
use crate::direct::{Change, Connection, HELLO, Welcome};
use std::sync::Arc;
use tokio::io::{AsyncWriteExt, BufReader};

fn page(source: &str, max_bytes: u32) -> Query {
    Query::Page(TranscriptRequest {
        source_id: source.into(),
        cursor: None,
        max_bytes,
        end: Some(coder_history::NEWEST),
    })
}

fn source(f: &Fixture) -> String {
    let pending = f.catalog();
    let reply = f
        .host()
        .handle(&pending.event, &f.code.relay, f.now)
        .unwrap();
    let Observation::Catalog(page) = f.client().verify_reply(&pending, &reply, f.now).unwrap()
    else {
        panic!("not a catalog")
    };
    page.entries[0].source_id.clone().unwrap()
}

#[test]
fn direct_requests_pass_every_relay_check_but_the_relay_binding() {
    let f = Fixture::new("wss://relay.example/");
    let client = f.client();
    // The same sealed request reads directly, with no relay named.
    let pending = client
        .prepare_for(
            Query::Catalog(CatalogRequest::default()),
            unix_time().unwrap(),
            Route::Direct,
        )
        .unwrap();
    let handled = f.host().handle_direct(&pending.event).unwrap();
    assert!(handled.read.is_some());
    assert!(
        client
            .verify_detached(
                &pending,
                &handled.reply,
                handled.payload.as_deref().unwrap(),
                unix_time().unwrap()
            )
            .is_ok()
    );
    // Another client's key, even with this connection's grant, is refused.
    let stranger = SecretKey::new(&mut secp256k1::rand::rng());
    let forged = seal(
        &pending.request,
        REQUEST,
        &stranger,
        &f.code.host,
        &pending.request.request,
        pending.request.issued_at,
        pending.request.expires_at,
    )
    .unwrap();
    assert_eq!(
        f.host().handle_direct(&forged).unwrap_err().code,
        ErrorCode::Forbidden
    );
    // An expired request is refused before any read.
    let stale = client
        .prepare_for(
            Query::Catalog(CatalogRequest::default()),
            unix_time().unwrap(),
            Route::Direct,
        )
        .unwrap();
    let later = stale.request.expires_at;
    assert_eq!(
        f.host()
            .handle_via(&stale.event, host::Via::Direct, || Ok(later))
            .unwrap_err()
            .code,
        ErrorCode::Expired
    );
    // Revocation ends direct reads as it ends relay reads.
    f.host().revoke(&f.code.grant, None, f.now).unwrap();
    let pending = client
        .prepare_for(
            Query::Catalog(CatalogRequest::default()),
            unix_time().unwrap(),
            Route::Direct,
        )
        .unwrap();
    let handled = f.host().handle_direct(&pending.event).unwrap();
    assert!(handled.read.is_none());
    assert_eq!(
        client
            .verify_reply_via(
                &pending,
                &handled.reply,
                unix_time().unwrap(),
                Route::Direct
            )
            .unwrap_err()
            .code,
        ErrorCode::Revoked
    );
}

#[test]
fn a_request_is_read_once_on_either_route() {
    let f = Fixture::new("wss://relay.example/");
    let client = f.client();
    let now = unix_time().unwrap();
    let pending = client
        .prepare_for(
            Query::Catalog(CatalogRequest::default()),
            now,
            Route::Direct,
        )
        .unwrap();
    let first = f.host().handle_direct(&pending.event).unwrap();
    // An exact direct retry gets the same bytes, and reads nothing.
    let again = f.host().handle_direct(&pending.event).unwrap();
    assert_eq!(again.reply.id, first.reply.id);
    assert!(again.read.is_none());
    // The same event through the relay reads nothing either: it gets the
    // signed conflict the book keeps in place of the direct reply's bytes.
    let relayed = f.host().handle(&pending.event, &f.code.relay, now).unwrap();
    assert_ne!(relayed.id, first.reply.id);
    // The host signed it at its own clock, which may be a second later.
    assert_eq!(
        client
            .verify_reply_via(&pending, &relayed, unix_time().unwrap(), Route::Direct)
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );
    // A different event with the same request ID conflicts.
    let mut other = pending.request.clone();
    other.query = Query::Catalog(CatalogRequest {
        cursor: None,
        limit: 2,
    });
    let other = seal(
        &other,
        REQUEST,
        &f.client_secret,
        &f.code.host,
        &other.request,
        other.issued_at,
        other.expires_at,
    )
    .unwrap();
    let conflict = f.host().handle_direct(&other).unwrap();
    assert!(conflict.read.is_none());
}

#[test]
fn direct_bounds_apply_only_to_direct_replies() {
    let f = Fixture::new("wss://relay.example/");
    let source = source(&f);
    let client = f.client();
    let big = coder_history::Limits::DIRECT.page_bytes;
    // A relay request cannot ask for a direct page, and a host refuses one
    // that arrives through the relay.
    assert_eq!(
        client.prepare(page(&source, big), f.now).unwrap_err().code,
        ErrorCode::Bounds
    );
    let pending = client
        .prepare_for(page(&source, big), f.now, Route::Direct)
        .unwrap();
    assert_eq!(
        f.host()
            .handle(&pending.event, &f.code.relay, f.now)
            .unwrap_err()
            .code,
        ErrorCode::Bounds
    );
    // Directly, it reads.
    let pending = client
        .prepare_for(page(&source, big), unix_time().unwrap(), Route::Direct)
        .unwrap();
    let handled = f.host().handle_direct(&pending.event).unwrap();
    assert!(matches!(
        client
            .verify_detached(
                &pending,
                &handled.reply,
                handled.payload.as_deref().unwrap(),
                unix_time().unwrap()
            )
            .unwrap(),
        Observation::Page(_)
    ));
    // A body larger than a sealed envelope's inline bound travels beside it.
    let file = f.root.join("sessions/2026/01/01/one.jsonl");
    let mut bytes = std::fs::read(&file).unwrap();
    let line = format!(
        "{{\"type\":\"response_item\",\"payload\":{{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{{\"type\":\"output_text\",\"text\":\"{}\"}}]}}}}\n",
        "y".repeat(6000)
    );
    for _ in 0..20 {
        bytes.extend_from_slice(line.as_bytes());
    }
    std::fs::write(&file, bytes).unwrap();
    let pending = client
        .prepare_for(page(&source, big), unix_time().unwrap(), Route::Direct)
        .unwrap();
    let handled = f.host().handle_direct(&pending.event).unwrap();
    let payload = handled.payload.unwrap();
    assert!(payload.len() > 100 * 1024, "{}", payload.len());
    let Observation::Page(read) = client
        .verify_detached(&pending, &handled.reply, &payload, unix_time().unwrap())
        .unwrap()
    else {
        panic!("not a page")
    };
    let raw: u64 = read.chunks.iter().map(|c| c.end_offset - c.offset).sum();
    assert!(raw > 64 * 1024, "{raw}");
}

#[test]
fn a_detached_body_must_be_the_bytes_its_envelope_names() {
    let f = Fixture::new("wss://relay.example/");
    let key = f.host().key().unwrap();
    let seal = |expires: u64| {
        seal_detached(
            &serde_json::json!({"issued_at": 1, "expires_at": expires}),
            REPLY,
            &key,
            &f.code.client,
            (&"a".repeat(64), 1, 2),
            MAX_DIRECT_BODY,
        )
        .unwrap()
    };
    let (event, payload) = seal(2);
    let (_, swapped) = seal(3);
    let open = |payload: &str| {
        open_detached::<serde_json::Value>(
            &event,
            payload,
            &f.client_secret,
            (&f.code.host, &f.code.client),
            REPLY,
            MAX_DIRECT_BODY,
        )
    };
    assert_eq!(open(&payload).unwrap()["expires_at"], 2);
    assert_eq!(open(&swapped).unwrap_err().code, ErrorCode::Forbidden);
    // Another reader's key cannot open it.
    let stranger = SecretKey::new(&mut secp256k1::rand::rng());
    assert!(
        open_detached::<serde_json::Value>(
            &event,
            &payload,
            &stranger,
            (&f.code.host, &f.code.client),
            REPLY,
            MAX_DIRECT_BODY,
        )
        .is_err()
    );
}

/// Emulate the host's tailnet listener: welcome one connection, then serve
/// it. The owner check itself is `coder-host`'s.
pub(super) async fn listener(host: Arc<host::Host>) -> std::net::SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let host = host.clone();
            tokio::spawn(async move {
                let (read, mut write) = stream.into_split();
                let mut reader = BufReader::new(read);
                let hello = crate::direct::line(&mut reader, crate::direct::MAX_HELLO_BYTES)
                    .await
                    .unwrap()
                    .unwrap();
                assert!(crate::direct::Hello::parse(&hello).is_some());
                let mut welcome = serde_json::to_vec(&Welcome {
                    v: HELLO.into(),
                    refused: None,
                })
                .unwrap();
                welcome.push(b'\n');
                write.write_all(&welcome).await.unwrap();
                crate::direct::serve(reader, write, host).await;
            });
        }
    });
    address
}

#[tokio::test]
async fn a_direct_connection_reads_at_once_and_nudges_when_a_read_chat_grows() {
    let f = Fixture::new("wss://relay.example/");
    let source = source(&f);
    let address = listener(Arc::new(f.host())).await;
    let client = Arc::new(f.client());
    client.set_direct(Some(address));
    assert_eq!(client.route(), Route::Direct);
    let mut changes = client.changes();
    // Several reads in flight on one connection.
    let reads = (0..6).map(|_| {
        let (client, source) = (client.clone(), source.clone());
        tokio::spawn(async move {
            client
                .observe_with(|route| page(&source, route.limits().page_bytes))
                .await
        })
    });
    for read in reads {
        assert!(matches!(read.await.unwrap().unwrap(), Observation::Page(_)));
    }
    assert_eq!(client.route(), Route::Direct);
    // The chat grows: the host names it, and nothing else.
    let file = f.root.join("sessions/2026/01/01/one.jsonl");
    let mut bytes = std::fs::read(&file).unwrap();
    bytes.extend_from_slice(b"{\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"More\"}]}}\n");
    std::fs::write(&file, bytes).unwrap();
    let change = tokio::time::timeout(std::time::Duration::from_secs(5), changes.recv())
        .await
        .expect("a nudge")
        .unwrap();
    assert_eq!(change, Change::Source(source));
}

#[tokio::test]
async fn a_host_without_direct_reads_falls_back_to_the_relay_route() {
    // A listener that answers as an older host does: one admission refusal.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let _ = stream
                .write_all(
                    b"{\"v\":\"openagents.host-tailnet-admission.v1\",\"refused\":\"malformed\"}\n",
                )
                .await;
        }
    });
    let (sender, _) = tokio::sync::broadcast::channel(4);
    assert_eq!(
        Connection::open(address, sender).await.err().unwrap().code,
        ErrorCode::Transport
    );
    let f = Fixture::new("wss://relay.example/");
    let client = f.client();
    client.set_direct(Some(address));
    // The relay here is unreachable, so the read fails, but only after the
    // direct attempt was given up for a while.
    let _ = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        client.observe(Query::Catalog(CatalogRequest::default())),
    )
    .await;
    assert_eq!(client.route(), Route::Relay);
}

#[tokio::test]
async fn a_direct_connection_nudges_the_catalog_when_any_harness_starts_a_chat() {
    let f = Fixture::new("wss://relay.example/");
    let address = listener(Arc::new(f.host())).await;
    let client = Arc::new(f.client());
    client.set_direct(Some(address));
    let mut changes = client.changes();
    // No catalog read yet: a new chat nudges nothing.
    let early = f.root.join("sessions/2026/01/01/early.jsonl");
    std::fs::write(
        &early,
        b"{\"type\":\"session_meta\",\"payload\":{\"id\":\"early\"}}\n",
    )
    .unwrap();
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(300), changes.recv())
            .await
            .is_err()
    );
    client
        .observe(Query::Catalog(CatalogRequest::default()))
        .await
        .unwrap();
    // A Codex chat starts in a new day's folder: the list changed.
    let day = f.root.join("sessions/2026/01/02");
    std::fs::create_dir_all(&day).unwrap();
    std::fs::write(
        day.join("two.jsonl"),
        b"{\"type\":\"session_meta\",\"payload\":{\"id\":\"second-chat\"}}\n",
    )
    .unwrap();
    let change = tokio::time::timeout(std::time::Duration::from_secs(4), changes.recv())
        .await
        .expect("a nudge")
        .unwrap();
    assert_eq!(change, Change::Catalog);
}

#[tokio::test]
async fn a_coder_only_host_watches_only_the_coder_task_directory() {
    let f = Fixture::new("wss://relay.example/");
    let tasks = f.root.parent().unwrap().join("tasks");
    std::fs::create_dir_all(&tasks).unwrap();
    let device = SecretKey::new(&mut secp256k1::rand::rng());
    let code = f
        .host()
        .pair(
            &pubkey(&device),
            &f.code.relay,
            coder_history::Config {
                codex: Some(f.root.clone()),
                coder: Some(tasks.clone()),
                ..coder_history::Config::default()
            },
            f.now,
            f.now + 3600,
        )
        .unwrap();
    let address = listener(Arc::new(f.host().coder_only())).await;
    let client =
        Arc::new(Client::new_with_policy(code, device, RelayPolicy::LoopbackTest).unwrap());
    client.set_direct(Some(address));
    let mut changes = client.changes();
    client
        .observe(Query::Catalog(CatalogRequest::default()))
        .await
        .unwrap();
    // A Codex chat the grant names starts: the host does not watch it.
    std::fs::write(
        f.root.join("sessions/2026/01/01/two.jsonl"),
        b"{\"type\":\"session_meta\",\"payload\":{\"id\":\"second-chat\"}}\n",
    )
    .unwrap();
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(700), changes.recv())
            .await
            .is_err()
    );
    // A Coder task delegates to OpenCode: its copy lists, so the list changed.
    let task = "7d".repeat(32);
    std::fs::write(
        tasks.join(
            coder_history::delegate::file_name(&task, coder_history::Harness::OpenCode, "ses_0e")
                .unwrap(),
        ),
        b"{\"type\":\"opencode.session\",\"session_id\":\"ses_0e\"}\n",
    )
    .unwrap();
    let change = tokio::time::timeout(std::time::Duration::from_secs(4), changes.recv())
        .await
        .expect("a nudge")
        .unwrap();
    assert_eq!(change, Change::Catalog);
}
