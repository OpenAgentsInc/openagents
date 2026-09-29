//! Generated identities, roots, and relay traffic only; no ambient chat discovery.
use super::*;
use crate::pairing::{self, Invitation};
use std::os::unix::fs::PermissionsExt;

fn invite(f: &Fixture) -> String {
    f.host()
        .invite(
            &f.code.relay,
            coder_history::Config {
                codex: Some(f.root.clone()),
                claude: None,
                coder: None,
                opencode: None,
                devin: None,
            },
            f.now,
            f.now + 3600,
        )
        .unwrap()
}
fn prepare(f: &Fixture, code: &str, secret: &SecretKey) -> (Invitation, pairing::Pending) {
    let invitation = Invitation::parse(code, f.now, RelayPolicy::LoopbackTest).unwrap();
    let pending = pairing::prepare(&invitation, secret, f.now, RelayPolicy::LoopbackTest).unwrap();
    (invitation, pending)
}
fn redeem_local(
    f: &Fixture,
    invitation: &Invitation,
    pending: &pairing::Pending,
    secret: &SecretKey,
) -> Result<ConnectionCode> {
    let reply = f.host().handle(&pending.event, &f.code.relay, f.now)?;
    pairing::verify(
        invitation,
        pending,
        &reply,
        secret,
        f.now,
        RelayPolicy::LoopbackTest,
    )
}
#[test]
fn compact_invitation_is_bounded_and_expiry_and_destination_are_local() {
    let f = Fixture::new("wss://relay.example/");
    let code = invite(&f);
    let invitation = Invitation::parse(&code, f.now, RelayPolicy::Production).unwrap();
    assert!(code.len() < 300);
    assert_eq!(invitation.encode().unwrap(), code);
    assert!(!code.contains(f.root.to_str().unwrap()));
    assert!(
        !std::fs::read_to_string(f.state.join("observer.json"))
            .unwrap()
            .contains(&invitation.capability)
    );
    assert_eq!(
        Invitation::parse(&code, f.now + 300, RelayPolicy::Production)
            .err()
            .unwrap()
            .code,
        ErrorCode::Expired
    );
    assert!(Invitation::parse(&(code.clone() + "="), f.now, RelayPolicy::Production).is_err());
    assert!(Invitation::parse(&(code + "A"), f.now, RelayPolicy::Production).is_err());
    let mut wrong = invitation;
    wrong.relay = "ws://127.0.0.1:1".into();
    let code = wrong.encode().unwrap();
    assert_eq!(
        Invitation::parse(&code, f.now, RelayPolicy::Production)
            .err()
            .unwrap()
            .code,
        ErrorCode::Forbidden
    );
    assert!(Invitation::parse(&"x".repeat(641), f.now, RelayPolicy::Production).is_err());
}
#[test]
fn successful_claim_is_single_device_and_same_grant_after_reopen() {
    let f = Fixture::new("wss://relay.example/");
    let code = invite(&f);
    let (invitation, pending) = prepare(&f, &code, &f.client_secret);
    let first = redeem_local(&f, &invitation, &pending, &f.client_secret).unwrap();
    assert_eq!(
        f.host().invitation_grant(&invitation.id).unwrap(),
        Some(first.grant.clone())
    );
    let duplicate = f
        .host()
        .handle(&pending.event, &f.code.relay, f.now)
        .unwrap();
    assert_eq!(
        duplicate,
        f.host()
            .handle(&pending.event, &f.code.relay, f.now)
            .unwrap()
    );
    let retry = pairing::prepare(
        &invitation,
        &f.client_secret,
        f.now + 1,
        RelayPolicy::Production,
    )
    .unwrap();
    let reply = f
        .host()
        .handle(&retry.event, &f.code.relay, f.now + 1)
        .unwrap();
    let again = pairing::verify(
        &invitation,
        &retry,
        &reply,
        &f.client_secret,
        f.now + 1,
        RelayPolicy::Production,
    )
    .unwrap();
    assert_eq!(first.authorization, again.authorization);
    assert_eq!(first.expires_at, again.expires_at);
    let stranger = SecretKey::new(&mut secp256k1::rand::rng());
    let (_, wrong) = prepare(&f, &code, &stranger);
    assert_eq!(
        redeem_local(&f, &invitation, &wrong, &stranger)
            .unwrap_err()
            .code,
        ErrorCode::Forbidden
    );
    assert_eq!(
        f.host().invitation_grant(&invitation.id).unwrap(),
        Some(first.grant)
    );
}
#[test]
fn wrong_capability_or_signature_cannot_consume_invitation() {
    let f = Fixture::new("wss://relay.example/");
    let code = invite(&f);
    let (mut invitation, _) = prepare(&f, &code, &f.client_secret);
    invitation.capability = random_id();
    let pending = pairing::prepare(
        &invitation,
        &f.client_secret,
        f.now,
        RelayPolicy::Production,
    )
    .unwrap();
    assert_eq!(
        f.host()
            .handle(&pending.event, &f.code.relay, f.now)
            .unwrap_err()
            .code,
        ErrorCode::Forbidden
    );
    let (_, mut pending) = prepare(&f, &code, &f.client_secret);
    pending.event.content.push('A');
    assert!(
        f.host()
            .handle(&pending.event, &f.code.relay, f.now)
            .is_err()
    );
    assert!(f.host().invitation_grant(&invitation.id).unwrap().is_none());
}
#[test]
fn bootstrap_cancellation_revocation_and_changed_roots_refuse() {
    let f = Fixture::new("wss://relay.example/");
    let code = invite(&f);
    let (invitation, pending) = prepare(&f, &code, &f.client_secret);
    f.host().cancel_invitation(&invitation.id).unwrap();
    assert_eq!(
        redeem_local(&f, &invitation, &pending, &f.client_secret)
            .unwrap_err()
            .code,
        ErrorCode::Revoked
    );
    let code = invite(&f);
    let (invitation, pending) = prepare(&f, &code, &f.client_secret);
    let paired = redeem_local(&f, &invitation, &pending, &f.client_secret).unwrap();
    f.host().revoke(&paired.grant, None, f.now).unwrap();
    assert_eq!(
        redeem_local(&f, &invitation, &pending, &f.client_secret)
            .unwrap_err()
            .code,
        ErrorCode::Revoked
    );
    let code = invite(&f);
    let (invitation, pending) = prepare(&f, &code, &f.client_secret);
    std::fs::rename(&f.root, f.root.with_extension("retired")).unwrap();
    std::fs::create_dir(&f.root).unwrap();
    assert_eq!(
        redeem_local(&f, &invitation, &pending, &f.client_secret)
            .unwrap_err()
            .code,
        ErrorCode::SourceChanged
    );
    assert!(f.host().invitation_grant(&invitation.id).unwrap().is_none());
}
#[test]
fn expiry_is_rechecked_and_failed_save_returns_no_success_or_memory_claim() {
    let f = Fixture::new("wss://relay.example/");
    let code = invite(&f);
    let (invitation, pending) = prepare(&f, &code, &f.client_secret);
    let mut times = [f.now, f.now + 60].into_iter();
    assert_eq!(
        f.host()
            .redeem_with_clock(&pending.event, &f.code.relay, || Ok(times.next().unwrap()))
            .unwrap_err()
            .code,
        ErrorCode::Expired
    );
    assert!(f.host().invitation_grant(&invitation.id).unwrap().is_none());
    let pending_path = f.state.join(".observer.pending");
    std::fs::write(&pending_path, b"synthetic failed write").unwrap();
    std::fs::set_permissions(&pending_path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(
        f.host()
            .handle(&pending.event, &f.code.relay, f.now)
            .is_err()
    );
    assert!(f.host().invitation_grant(&invitation.id).unwrap().is_none());
    std::fs::remove_file(pending_path).unwrap();
    assert!(redeem_local(&f, &invitation, &pending, &f.client_secret).is_ok());
}
#[test]
fn response_identity_and_original_request_are_checked() {
    let f = Fixture::new("wss://relay.example/");
    let code = invite(&f);
    let (invitation, pending) = prepare(&f, &code, &f.client_secret);
    let response = f
        .host()
        .handle(&pending.event, &f.code.relay, f.now)
        .unwrap();
    let (_, other) = prepare(&f, &code, &f.client_secret);
    assert_eq!(
        pairing::verify(
            &invitation,
            &other,
            &response,
            &f.client_secret,
            f.now,
            RelayPolicy::Production
        )
        .unwrap_err()
        .code,
        ErrorCode::Forbidden
    );
    let (_, mut wrong_host) = prepare(&f, &code, &f.client_secret);
    wrong_host.event.pubkey = pubkey(&SecretKey::new(&mut secp256k1::rand::rng()));
    assert!(
        f.host()
            .handle(&wrong_host.event, &f.code.relay, f.now)
            .is_err()
    );
}
#[tokio::test]
async fn encrypted_pairing_then_catalog_and_page_reuses_existing_observer() {
    let (url, relay, _) = super::relay::start().await;
    bootstrap_roundtrip(&url).await;
    relay.abort();
    let _ = relay.await;
}
async fn bootstrap_roundtrip(url: &str) {
    let f = Fixture::new(url);
    let code = invite(&f);
    let host = f.host();
    let secret = host.key().unwrap();
    let mut receiver = transport::Receiver::connect(url, &secret, RelayPolicy::LoopbackTest)
        .await
        .unwrap();
    let remote_url = url.to_owned();
    let server = tokio::spawn(async move {
        for _ in 0..3 {
            let event = receiver.next_request().await.unwrap();
            let response = host.handle_current(&event, &remote_url).unwrap();
            receiver.publish(&response).await.unwrap();
        }
    });
    let connection = pairing::redeem(&code, &f.client_secret, RelayPolicy::LoopbackTest)
        .await
        .unwrap();
    let client =
        Client::new_with_policy(connection, f.client_secret, RelayPolicy::LoopbackTest).unwrap();
    let Observation::Catalog(catalog) = client
        .observe(Query::Catalog(CatalogRequest::default()))
        .await
        .unwrap()
    else {
        panic!("catalog")
    };
    assert_eq!(catalog.entries.len(), 1);
    let source = catalog.entries[0].source_id.clone().unwrap();
    let Observation::Page(page) = client
        .observe(Query::Page(TranscriptRequest {
            source_id: source,
            cursor: None,
            max_bytes: coder_history::MAX_PAGE_BYTES,
            end: None,
        }))
        .await
        .unwrap()
    else {
        panic!("page")
    };
    assert!(!page.chunks.is_empty());
    server.await.unwrap();
}
#[tokio::test]
#[ignore = "publishes generated pairing and synthetic history only to an explicitly selected relay"]
async fn production_bootstrap_reads_only_generated_history() {
    let url = std::env::var("CODER_CONNECT_SYNTHETIC_RELAY")
        .expect("select the production relay explicitly");
    RelayPolicy::Production.validate(&url).unwrap();
    tokio::time::timeout(
        std::time::Duration::from_secs(45),
        bootstrap_roundtrip(&url),
    )
    .await
    .unwrap();
    println!(
        "Synthetic production bootstrap, catalog, and transcript passed; no ambient roots or identity were used."
    );
}
#[test]
fn qr_modules_are_a_square_with_a_quiet_zone_and_finder_patterns() {
    let f = Fixture::new("wss://relay.example/");
    let code = invite(&f);
    let rows = pairing::qr_modules_prefixed(pairing::PREFIX, &code).unwrap();
    let side = rows.len();
    assert!(rows.iter().all(|row| row.len() == side));
    // Version sizes are 21 + 4k modules, plus four quiet modules per side.
    assert_eq!((side - 8 - 21) % 4, 0);
    assert!(rows[..4].iter().flatten().all(|dark| !dark));
    assert!(rows.iter().all(|row| row[..4].iter().all(|dark| !dark)));
    // Each finder pattern's outer ring starts dark at its corner.
    assert!(rows[4][4] && rows[4][side - 5] && rows[side - 5][4]);
    // Another profile's prefix is refused, as the other renderers refuse it.
    assert!(pairing::qr_modules_prefixed("coder-host:", &code).is_err());
}

#[test]
#[ignore = "writes a synthetic invitation SVG and exact payload for an independent native QR decoder"]
fn export_synthetic_qr_fixture() {
    let dir = std::env::var_os("CODER_CONNECT_QR_FIXTURE_DIR")
        .map(PathBuf::from)
        .expect("select a private temporary output directory");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    let f = Fixture::new("wss://relay.example/");
    let code = invite(&f);
    let svg = pairing::qr_svg(&code).unwrap();
    std::fs::write(dir.join("pairing.svg"), svg).unwrap();
    std::fs::write(dir.join("payload.txt"), &code).unwrap();
    std::fs::set_permissions(
        dir.join("payload.txt"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    println!("Synthetic QR fixture written; payload is intentionally omitted from the test log.");
}

#[test]
fn expired_admissions_are_pruned_by_the_invitation_flow() {
    let f = Fixture::new("wss://relay.example/");
    // Live grants to 64 devices fill the book; the fixture's is the first.
    for _ in 1..64 {
        f.host()
            .pair(
                &pubkey(&SecretKey::new(&mut secp256k1::rand::rng())),
                &f.code.relay,
                coder_history::Config {
                    codex: Some(f.root.clone()),
                    claude: None,
                    coder: None,
                    opencode: None,
                    devin: None,
                },
                f.now,
                f.now + 3600,
            )
            .unwrap();
    }
    let code = invite(&f);
    let newcomer = SecretKey::new(&mut secp256k1::rand::rng());
    let (_, pending) = prepare(&f, &code, &newcomer);
    assert_eq!(
        f.host()
            .handle(&pending.event, &f.code.relay, f.now)
            .unwrap_err()
            .code,
        ErrorCode::Bounds
    );
    let later = f.now + 3661;
    let code = f
        .host()
        .invite(
            &f.code.relay,
            coder_history::Config {
                codex: Some(f.root.clone()),
                claude: None,
                coder: None,
                opencode: None,
                devin: None,
            },
            later,
            later + 3600,
        )
        .unwrap();
    let invitation = Invitation::parse(&code, later, RelayPolicy::Production).unwrap();
    let pending = pairing::prepare(
        &invitation,
        &f.client_secret,
        later,
        RelayPolicy::Production,
    )
    .unwrap();
    let reply = f
        .host()
        .handle(&pending.event, &f.code.relay, later)
        .unwrap();
    assert!(
        pairing::verify(
            &invitation,
            &pending,
            &reply,
            &f.client_secret,
            later,
            RelayPolicy::Production
        )
        .is_ok()
    );
}
#[test]
fn two_devices_racing_for_one_invitation_cannot_both_claim_it() {
    let f = Fixture::new("wss://relay.example/");
    let code = invite(&f);
    let second = SecretKey::new(&mut secp256k1::rand::rng());
    let (invitation, a) = prepare(&f, &code, &f.client_secret);
    let (_, b) = prepare(&f, &code, &second);
    let gate = std::sync::Arc::new(std::sync::Barrier::new(2));
    std::thread::scope(|scope| {
        for event in [&a.event, &b.event] {
            let gate = gate.clone();
            let f = &f;
            scope.spawn(move || {
                gate.wait();
                let _ = f.host().handle(event, &f.code.relay, f.now);
            });
        }
    });
    let first = redeem_local(&f, &invitation, &a, &f.client_secret);
    let second = redeem_local(&f, &invitation, &b, &second);
    assert_ne!(first.is_ok(), second.is_ok());
    let failure = first.err().or_else(|| second.err()).unwrap();
    assert_eq!(failure.code, ErrorCode::Forbidden);
}

#[test]
fn coder_task_source_pairs_and_pages_backward_through_the_observer() {
    let f = Fixture::new("wss://relay.example/");
    let tasks = f.root.parent().unwrap().join("tasks");
    std::fs::create_dir_all(&tasks).unwrap();
    let task = "c".repeat(64);
    std::fs::write(
        tasks.join(format!("{task}.1.atif.jsonl")),
        concat!(
            r#"{"record":"session","schema_version":"ATIF-v1.8","at":1,"session":{"id":"synthetic-1"}}"#,
            "\n",
            r#"{"record":"step","step":{"at":2,"source":"User","message":"Synthetic Coder task"}}"#,
            "\n",
            r#"{"record":"step","step":{"at":3,"source":"Agent","message":"","call":{"id":"c1","name":"shell","arguments":{"command":"ls"},"output":"README.md","outcome":"Completed","milliseconds":1}}}"#,
            "\n",
            r#"{"record":"step","step":{"at":4,"source":"Agent","message":"Done."}}"#,
            "\n",
        ),
    )
    .unwrap();
    let code = f
        .host()
        .invite(
            &f.code.relay,
            coder_history::Config {
                codex: Some(f.root.clone()),
                claude: None,
                coder: Some(tasks.clone()),
                opencode: None,
                devin: None,
            },
            f.now,
            f.now + 3600,
        )
        .unwrap();
    let (invitation, pending) = prepare(&f, &code, &f.client_secret);
    let connection = redeem_local(&f, &invitation, &pending, &f.client_secret).unwrap();
    assert_eq!(
        connection
            .sources
            .iter()
            .map(|s| s.kind)
            .collect::<Vec<_>>(),
        [SourceKind::Codex, SourceKind::Coder]
    );
    let bytes = serde_json::to_vec(&connection).unwrap();
    assert!(String::from_utf8_lossy(&bytes).contains("\"coder\""));
    assert!(!String::from_utf8_lossy(&bytes).contains(tasks.to_str().unwrap()));

    let client = Client::new_with_policy(
        connection.clone(),
        f.client_secret,
        RelayPolicy::LoopbackTest,
    )
    .unwrap();
    let ask = |query: Query| {
        let pending = client.prepare(query, f.now).unwrap();
        let reply = f
            .host()
            .handle(&pending.event, &connection.relay, f.now)
            .unwrap();
        client.verify_reply(&pending, &reply, f.now).unwrap()
    };
    let Observation::Catalog(catalog) = ask(Query::Catalog(CatalogRequest::default())) else {
        panic!("catalog expected")
    };
    let chat = catalog
        .entries
        .iter()
        .find(|c| c.harness == coder_history::Harness::Coder)
        .unwrap();
    assert_eq!(chat.native_id.as_deref(), Some(task.as_str()));
    assert_eq!(chat.title, "Synthetic Coder task");
    let Observation::Page(page) = ask(Query::Page(TranscriptRequest {
        source_id: chat.source_id.clone().unwrap(),
        cursor: None,
        max_bytes: coder_history::MAX_PAGE_BYTES,
        end: Some(coder_history::NEWEST),
    })) else {
        panic!("page expected")
    };
    assert_eq!(page.previous, None);
    let views: Vec<_> = page
        .chunks
        .iter()
        .map(|c| {
            let r = c.readable.as_ref().unwrap();
            (r.kind.as_str(), r.role.as_deref(), r.text.as_str())
        })
        .collect();
    assert_eq!(
        views,
        [
            ("session", None, ""),
            ("message", Some("user"), "Synthetic Coder task"),
            ("tool_call", None, "ls\n\nREADME.md"),
            ("message", Some("assistant"), "Done."),
        ]
    );
}

#[test]
fn a_coder_only_host_serves_only_coder_chats_and_their_delegates_under_any_grant() {
    let f = Fixture::new("wss://relay.example/");
    let tasks = f.root.parent().unwrap().join("tasks");
    std::fs::create_dir_all(&tasks).unwrap();
    let task = "5c".repeat(32);
    std::fs::write(
        tasks.join(format!("{task}.1.atif.jsonl")),
        concat!(
            r#"{"record":"session","schema_version":"ATIF-v1.7","at":1,"session":{"id":"TASK-1"}}"#,
            "\n",
            r#"{"record":"step","step":{"at":2,"source":"User","message":"Synthetic Coder task"}}"#,
            "\n",
        ),
    )
    .unwrap();
    // The OpenCode session the task delegated to, kept beside it.
    std::fs::write(
        tasks.join(
            coder_history::delegate::file_name(&task, coder_history::Harness::OpenCode, "ses_0d")
                .unwrap(),
        ),
        concat!(
            r#"{"type":"opencode.session","session_id":"ses_0d","parent_id":null,"directory":"/w","version":"1.18.26","time":1}"#,
            "\n",
            r#"{"type":"opencode.part","session_id":"ses_0d","message_id":"msg_1","part_id":"prt_1","role":"assistant","model":null,"time":2,"part":{"type":"text","text":"Delegated."}}"#,
            "\n",
        ),
    )
    .unwrap();
    // A grant made while the host also offered Codex history.
    let code = f
        .host()
        .invite(
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
    // A second device, so the fixture's own Codex-only grant stays.
    let device = SecretKey::new(&mut secp256k1::rand::rng());
    let (invitation, pending) = prepare(&f, &code, &device);
    let connection = redeem_local(&f, &invitation, &pending, &device).unwrap();
    let client =
        Client::new_with_policy(connection.clone(), device, RelayPolicy::LoopbackTest).unwrap();
    let ask = |host: &host::Host, query: Query| {
        let pending = client.prepare(query, f.now).unwrap();
        let reply = host
            .handle(&pending.event, &connection.relay, f.now)
            .unwrap();
        client.verify_reply(&pending, &reply, f.now)
    };
    let catalog = |host: &host::Host| match ask(host, Query::Catalog(CatalogRequest::default())) {
        Ok(Observation::Catalog(catalog)) => catalog,
        other => panic!("catalog expected, got {other:?}"),
    };
    // Every root of the grant, as a host that serves them all reads it.
    let all = catalog(&f.host());
    let codex = all
        .entries
        .iter()
        .find(|c| c.harness == coder_history::Harness::Codex)
        .unwrap()
        .source_id
        .clone()
        .unwrap();
    let coder_only = f.host().coder_only();
    let listed = catalog(&coder_only);
    let mut kinds: Vec<_> = listed
        .entries
        .iter()
        .map(|c| (c.harness, c.subagent, c.native_id.clone().unwrap()))
        .collect();
    kinds.sort_by_key(|(_, subagent, _)| *subagent);
    assert_eq!(
        kinds,
        [
            (coder_history::Harness::Coder, false, task.clone()),
            (coder_history::Harness::OpenCode, true, task.clone()),
        ]
    );
    let delegate = listed.entries.iter().find(|c| c.subagent).unwrap();
    let page = |source_id: String| {
        ask(
            &coder_only,
            Query::Page(TranscriptRequest {
                source_id,
                cursor: None,
                max_bytes: coder_history::MAX_PAGE_BYTES,
                end: Some(coder_history::NEWEST),
            }),
        )
    };
    let Ok(Observation::Page(read)) = page(delegate.source_id.clone().unwrap()) else {
        panic!("the delegate's transcript reads")
    };
    assert_eq!(read.chunks[1].readable.as_ref().unwrap().text, "Delegated.");
    // The Codex chat the grant names is not read.
    assert_eq!(page(codex).unwrap_err().code, ErrorCode::Unavailable);
    // A grant that names no Coder root lists nothing, without an error.
    let old = f.client();
    let pending = f.catalog();
    let reply = coder_only
        .handle(&pending.event, &f.code.relay, f.now)
        .unwrap();
    let Observation::Catalog(empty) = old.verify_reply(&pending, &reply, f.now).unwrap() else {
        panic!("catalog expected")
    };
    assert!(empty.entries.is_empty() && empty.next.is_none());
}

fn paired(f: &Fixture, client: &SecretKey, now: u64) -> ConnectionCode {
    f.host()
        .pair(
            &pubkey(client),
            &f.code.relay,
            coder_history::Config {
                codex: Some(f.root.clone()),
                claude: None,
                coder: None,
                opencode: None,
                devin: None,
            },
            now,
            now + 3600,
        )
        .unwrap()
}
fn catalog_as(
    f: &Fixture,
    code: &ConnectionCode,
    secret: SecretKey,
    now: u64,
) -> std::result::Result<(), ErrorCode> {
    let client = Client::new_with_policy(code.clone(), secret, RelayPolicy::LoopbackTest).unwrap();
    let pending = client
        .prepare(Query::Catalog(CatalogRequest::default()), now)
        .map_err(|e| e.code)?;
    let reply = f
        .host()
        .handle(&pending.event, &code.relay, now)
        .map_err(|e| e.code)?;
    client
        .verify_reply(&pending, &reply, now)
        .map(|_| ())
        .map_err(|e| e.code)
}
fn grants_of(f: &Fixture, client: &SecretKey) -> usize {
    let book: serde_json::Value =
        serde_json::from_slice(&std::fs::read(f.state.join("observer.json")).unwrap()).unwrap();
    book["admissions"]
        .as_object()
        .unwrap()
        .values()
        .filter(|a| a["grant"]["client"] == pubkey(client).as_str())
        .count()
}
/// Redeem a fresh invitation issued at `now` as `secret`.
fn repair(f: &Fixture, secret: &SecretKey, now: u64) -> (pairing::Pending, ConnectionCode) {
    let code = f
        .host()
        .invite(
            &f.code.relay,
            coder_history::Config {
                codex: Some(f.root.clone()),
                claude: None,
                coder: None,
                opencode: None,
                devin: None,
            },
            now,
            now + 3600,
        )
        .unwrap();
    let invitation = Invitation::parse(&code, now, RelayPolicy::LoopbackTest).unwrap();
    let pending = pairing::prepare(&invitation, secret, now, RelayPolicy::LoopbackTest).unwrap();
    let reply = f.host().handle(&pending.event, &f.code.relay, now).unwrap();
    let connection = pairing::verify(
        &invitation,
        &pending,
        &reply,
        secret,
        now,
        RelayPolicy::LoopbackTest,
    )
    .unwrap();
    (pending, connection)
}
#[test]
fn a_device_that_pairs_again_holds_one_grant() {
    let f = Fixture::new("wss://relay.example/");
    // Past times, so each connection code is current for the client.
    let t0 = f.now - 1000;
    let (_, first) = repair(&f, &f.client_secret, t0);
    // The fixture's grant, which no invitation names, leaves the book.
    assert_eq!(grants_of(&f, &f.client_secret), 1);
    let (pending, second) = repair(&f, &f.client_secret, t0);
    assert_ne!(second.grant, first.grant);
    // An invitation still names the first grant: it stays, revoked.
    assert_eq!(grants_of(&f, &f.client_secret), 2);
    assert_eq!(
        catalog_as(&f, &first, f.client_secret, t0),
        Err(ErrorCode::Revoked)
    );
    assert_eq!(catalog_as(&f, &second, f.client_secret, t0), Ok(()));
    // An exact retry of the redemption gets the same grant.
    let retried = f.host().handle(&pending.event, &f.code.relay, t0).unwrap();
    let again = f.host().handle(&pending.event, &f.code.relay, t0).unwrap();
    assert_eq!(retried.id, again.id);
    // Once no invitation names them, earlier grants leave the book.
    let later = t0 + pairing::LIFETIME + MAX_REQUEST_LIFETIME + 1;
    let third = paired(&f, &f.client_secret, later);
    assert_eq!(grants_of(&f, &f.client_secret), 1);
    assert_eq!(catalog_as(&f, &third, f.client_secret, later), Ok(()));
    assert_eq!(
        catalog_as(&f, &second, f.client_secret, later),
        Err(ErrorCode::Forbidden)
    );
    // Another device's grant is untouched.
    let other = SecretKey::new(&mut secp256k1::rand::rng());
    let theirs = paired(&f, &other, later);
    paired(&f, &f.client_secret, later);
    assert_eq!(catalog_as(&f, &theirs, other, later), Ok(()));
}
#[test]
fn revoked_grants_never_block_a_new_pairing() {
    let f = Fixture::new("wss://relay.example/");
    let mut devices = vec![(f.client_secret, f.code.clone())];
    for _ in 1..64 {
        let secret = SecretKey::new(&mut secp256k1::rand::rng());
        devices.push((secret, paired(&f, &secret, f.now)));
    }
    // A full book of live grants refuses a new device.
    let newcomer = SecretKey::new(&mut secp256k1::rand::rng());
    let refused = f.host().pair(
        &pubkey(&newcomer),
        &f.code.relay,
        coder_history::Config {
            codex: Some(f.root.clone()),
            claude: None,
            coder: None,
            opencode: None,
            devin: None,
        },
        f.now,
        f.now + 3600,
    );
    assert_eq!(refused.unwrap_err().code, ErrorCode::Bounds);
    // Revoked grants make room, the longest-revoked first.
    f.host()
        .revoke(&devices[5].1.grant, None, f.now - 2)
        .unwrap();
    f.host()
        .revoke(&devices[3].1.grant, None, f.now - 1)
        .unwrap();
    let first = paired(&f, &newcomer, f.now);
    assert_eq!(catalog_as(&f, &first, newcomer, f.now), Ok(()));
    assert_eq!(
        catalog_as(&f, &devices[5].1, devices[5].0, f.now),
        Err(ErrorCode::Forbidden)
    );
    assert_eq!(
        catalog_as(&f, &devices[3].1, devices[3].0, f.now),
        Err(ErrorCode::Revoked)
    );
    let another = SecretKey::new(&mut secp256k1::rand::rng());
    let second = paired(&f, &another, f.now);
    assert_eq!(catalog_as(&f, &second, another, f.now), Ok(()));
    assert_eq!(
        catalog_as(&f, &devices[3].1, devices[3].0, f.now),
        Err(ErrorCode::Forbidden)
    );
    // Every live grant still reads.
    for (secret, code) in devices
        .iter()
        .filter(|(_, c)| c.grant != devices[5].1.grant && c.grant != devices[3].1.grant)
    {
        assert_eq!(catalog_as(&f, code, *secret, f.now), Ok(()));
    }
}
