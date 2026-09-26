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
    for _ in 1..64 {
        f.host()
            .pair(
                &pubkey(&f.client_secret),
                &f.code.relay,
                coder_history::Config {
                    codex: Some(f.root.clone()),
                    claude: None,
                },
                f.now,
                f.now + 3600,
            )
            .unwrap();
    }
    let code = invite(&f);
    let (_, pending) = prepare(&f, &code, &f.client_secret);
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
