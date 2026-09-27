//! NIP-PL executor against a live Postgres database.
//!
//! Covers lease create, renew, expire, and revoke; a delivery job that
//! survives a restart and is claimed once; retries and the dead-letter
//! state; a revoked device and a removed group member that get no further
//! wakes; one executor serving an APNs profile and an FCM profile at once;
//! and one end-to-end wake through a running gateway. Set
//! `NOSTR_RELAY_TEST_DATABASE_URL` and `NOSTR_RELAY_TEST_ALLOW_DESTRUCTIVE=1`,
//! or run `scripts/test-postgres.sh`.

use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream as StdTcpStream},
    sync::Arc,
    time::Duration,
};

use nostr::nip44::{conversation_key, encrypt};
use nostr_relay::{
    domain::{Event, RelaySigner, Tag},
    gateway::{
        Gateway, GatewayConfig, PushExecutor, PushProfile, RetryPolicy,
        push::{self, Platform, TestTransport, WakeOutcome},
    },
    store::{AdmissionOutcome, AdmissionRejection, PushDeliveryCheck, PushDisposition, Store},
};
use secp256k1::{Keypair, Secp256k1, SecretKey, XOnlyPublicKey};
use serde_json::{Value, json};
use tokio::time::timeout;
use tokio_postgres::NoTls;
use tokio_tungstenite::tungstenite::{Message, WebSocket, client};

const EXECUTOR: u8 = 0x42;
const STORE_ORIGIN: &str = "ws://store.test";
const GATEWAY_ORIGIN: &str = "ws://relay.test";
const PROFILE: &str = "app.test/ios";
const ANDROID_PROFILE: &str = "app.test/android";
const PROFILES_ORIGIN: &str = "ws://profiles.test";
const ROOM: &str = "5c4b1d2e-7f3a-4b6c-9d8e-0f1a2b3c4d5e";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn push_executor_contract_against_postgres() {
    let Ok(database_url) = std::env::var("NOSTR_RELAY_TEST_DATABASE_URL") else {
        eprintln!("skipped: set NOSTR_RELAY_TEST_DATABASE_URL or run scripts/test-postgres.sh");
        return;
    };
    if std::env::var("NOSTR_RELAY_TEST_ALLOW_DESTRUCTIVE").as_deref() != Ok("1") {
        eprintln!("skipped: the push suite writes executor state; use a disposable database");
        return;
    }
    let mut store = Store::connect(&database_url).await.unwrap();
    let transport = TestTransport::new(Platform::Apns);
    let mut executor = executor(STORE_ORIGIN, transport.clone());
    executor.limits.max_leases_per_pubkey = 2;
    executor.retry = RetryPolicy {
        max_attempts: 3,
        base_delay_seconds: 10,
        max_delay_seconds: 600,
    };
    let t0 = now();

    lease_lifecycle(&mut store, &executor, t0).await;
    jobs_survive_restart_and_are_claimed_once(&database_url, store, &executor, t0).await;
    let mut store = Store::connect_verified(&database_url).await.unwrap();
    retries_end_in_the_dead_letter_state(&mut store, &executor, &transport, t0).await;
    revoked_devices_get_no_further_wakes(&mut store, &executor, &transport, t0).await;
    removed_members_get_no_further_wakes(&database_url, &mut store, &executor, &transport, t0)
        .await;
    one_executor_serves_both_platforms(&mut store, t0).await;
    drop(store);
    gateway_wakes_end_to_end(&database_url).await;
}

async fn lease_lifecycle(store: &mut Store, executor: &PushExecutor, t0: u64) {
    let author = pubkey(0x21);
    let subs = json!([{ "filter": { "kinds": [1], "#p": [author] }, "class": "default" }]);

    // Create.
    let created = lease(
        0x21,
        "install-1",
        t0,
        t0 + 3_600,
        active(1, "grant-1", &subs),
    );
    assert_stored(admit(store, executor, &created, t0).await);
    let state = store
        .push_lease_state(STORE_ORIGIN, &author, "install-1")
        .await
        .unwrap()
        .unwrap();
    assert!(state.active);
    assert_eq!(state.generation, 1);
    assert_eq!(
        state.endpoint_hash.as_deref(),
        Some(nostr::push_lease::endpoint_digest("grant-1").as_str())
    );

    // Renew with a higher generation and a later expiration.
    let renewed = lease(
        0x21,
        "install-1",
        t0 + 1,
        t0 + 7_200,
        active(2, "grant-1", &subs),
    );
    assert_stored(admit(store, executor, &renewed, t0 + 1).await);
    let state = store
        .push_lease_state(STORE_ORIGIN, &author, "install-1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!((state.generation, state.expires_at), (2, t0 + 7_200));

    // A newer event that does not raise the generation, and a higher
    // generation that loses NIP-01 ordering, both leave state unchanged.
    let same_generation = lease(
        0x21,
        "install-1",
        t0 + 2,
        t0 + 7_200,
        active(2, "grant-1", &subs),
    );
    assert_eq!(
        admit(store, executor, &same_generation, t0 + 2).await,
        AdmissionOutcome::Rejected(AdmissionRejection::PushLease("stale generation"))
    );
    let older = lease(
        0x21,
        "install-1",
        t0,
        t0 + 7_200,
        active(9, "grant-1", &subs),
    );
    assert_eq!(
        admit(store, executor, &older, t0 + 2).await,
        AdmissionOutcome::Rejected(AdmissionRejection::Superseded)
    );
    let state = store
        .push_lease_state(STORE_ORIGIN, &author, "install-1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (state.generation, state.event_id.as_str()),
        (2, renewed.id.as_str())
    );

    // One active lease per endpoint, and at most two active addresses.
    let duplicate = lease(
        0x21,
        "install-2",
        t0 + 3,
        t0 + 100,
        active(1, "grant-1", &subs),
    );
    assert_eq!(
        admit(store, executor, &duplicate, t0 + 3).await,
        AdmissionOutcome::Rejected(AdmissionRejection::PushLease("endpoint already leased"))
    );
    let short = lease(
        0x21,
        "install-2",
        t0 + 3,
        t0 + 100,
        active(1, "grant-2", &subs),
    );
    assert_stored(admit(store, executor, &short, t0 + 3).await);
    let third = lease(
        0x21,
        "install-3",
        t0 + 4,
        t0 + 3_600,
        active(1, "grant-2b", &subs),
    );
    assert_eq!(
        admit(store, executor, &third, t0 + 4).await,
        AdmissionOutcome::Rejected(AdmissionRejection::PushLease("lease quota exceeded"))
    );

    // Expire: once install-2 passes its expiration, its endpoint and quota
    // slot are free, and it no longer matches.
    let reuse = lease(
        0x21,
        "install-3",
        t0 + 150,
        t0 + 3_600,
        active(1, "grant-2", &subs),
    );
    assert_stored(admit(store, executor, &reuse, t0 + 150).await);
    let expired = store
        .push_lease_state(STORE_ORIGIN, &author, "install-2")
        .await
        .unwrap()
        .unwrap();
    assert!(!expired.active);
    assert!(expired.retain_until >= t0 + 100);

    // Revoke with a higher-generation tombstone.
    let revoked = lease(0x21, "install-1", t0 + 5, t0 + 7_200, tombstone(3));
    assert_stored(admit(store, executor, &revoked, t0 + 5).await);
    let state = store
        .push_lease_state(STORE_ORIGIN, &author, "install-1")
        .await
        .unwrap()
        .unwrap();
    assert!(!state.active);
    assert_eq!(state.generation, 3);
    assert!(state.endpoint_hash.is_none());
    assert!(state.retain_until >= t0 + 5 + executor.limits.max_lease_ttl);
    // A replay of the older active lease cannot resurrect it.
    assert!(matches!(
        admit(store, executor, &renewed, t0 + 6).await,
        AdmissionOutcome::Rejected(_)
    ));
    assert!(
        !store
            .push_lease_state(STORE_ORIGIN, &author, "install-1")
            .await
            .unwrap()
            .unwrap()
            .active
    );

    // NIP-09 deletion never removes a lease; revocation is the only path.
    let deletion = signed_event(
        0x21,
        t0 + 7,
        5,
        vec![Tag::new(vec![
            "a".into(),
            format!("30350:{author}:install-3"),
        ])],
        "",
    );
    assert_stored(store.admit(&deletion, t0 + 7).await.unwrap());
    let fresh = lease(
        0x21,
        "install-3",
        t0 + 151,
        t0 + 3_600,
        active(2, "grant-2", &subs),
    );
    assert_stored(admit(store, executor, &fresh, t0 + 151).await);
    assert!(
        store
            .push_lease_state(STORE_ORIGIN, &author, "install-3")
            .await
            .unwrap()
            .unwrap()
            .active
    );
}

async fn jobs_survive_restart_and_are_claimed_once(
    database_url: &str,
    mut store: Store,
    executor: &PushExecutor,
    t0: u64,
) {
    let author = pubkey(0x31);
    let subs = json!([{ "filter": { "kinds": [1], "#p": [author] }, "class": "default" }]);
    let lease_event = lease(
        0x31,
        "phone",
        t0 + 10,
        t0 + 3_600,
        active(1, "grant-b", &subs),
    );
    assert_stored(admit(&mut store, executor, &lease_event, t0 + 10).await);
    push::match_events(&mut store, executor, t0 + 10)
        .await
        .unwrap();

    let mention = signed_event(
        0x32,
        t0 + 11,
        1,
        vec![Tag::new(vec!["p".into(), author.clone()])],
        "private words",
    );
    let other = signed_event(
        0x32,
        t0 + 11,
        1,
        vec![Tag::new(vec!["p".into(), pubkey(0x33)])],
        "not for the lease",
    );
    assert_stored(store.admit(&mention, t0 + 11).await.unwrap());
    assert_stored(store.admit(&other, t0 + 11).await.unwrap());
    let report = push::match_events(&mut store, executor, t0 + 11)
        .await
        .unwrap();
    assert_eq!(report.jobs, 1);
    // The cursor moved in the same transaction; matching again adds nothing.
    assert_eq!(
        push::match_events(&mut store, executor, t0 + 11)
            .await
            .unwrap()
            .jobs,
        0
    );

    // Restart: a new connection sees the durable job.
    drop(store);
    let mut first = Store::connect_verified(database_url).await.unwrap();
    let mut second = Store::connect_verified(database_url).await.unwrap();
    let jobs = first.push_jobs(STORE_ORIGIN, 100).await.unwrap();
    let job = jobs
        .iter()
        .find(|job| job.event_id == mention.id)
        .expect("the job survived the restart");
    assert_eq!(job.state, "pending");
    assert!(!jobs.iter().any(|job| job.event_id == other.id));

    // Claimed once: two workers race and exactly one wins.
    let (a, b) = tokio::join!(
        first.claim_push_jobs(STORE_ORIGIN, "worker-a", t0 + 12, 60, 10),
        second.claim_push_jobs(STORE_ORIGIN, "worker-b", t0 + 12, 60, 10),
    );
    let (a, b) = (a.unwrap(), b.unwrap());
    assert_eq!(a.len() + b.len(), 1, "one claim for one job");
    let claimed = a.into_iter().chain(b).next().unwrap();
    assert_eq!(claimed.event_id, mention.id);
    assert_eq!(claimed.attempts, 1);

    // The claimant crashes. After its claim expires another worker reclaims,
    // and the stale claimant can no longer finish the job.
    assert!(
        first
            .claim_push_jobs(STORE_ORIGIN, "worker-c", t0 + 30, 60, 10)
            .await
            .unwrap()
            .is_empty()
    );
    let reclaimed = first
        .claim_push_jobs(STORE_ORIGIN, "worker-c", t0 + 73, 60, 10)
        .await
        .unwrap();
    assert_eq!(reclaimed.len(), 1);
    assert_eq!(reclaimed[0].attempts, 2);
    assert!(
        !second
            .finish_push_job(&claimed, PushDisposition::Delivered, t0 + 74)
            .await
            .unwrap()
    );
    assert!(
        first
            .finish_push_job(&reclaimed[0], PushDisposition::Delivered, t0 + 74)
            .await
            .unwrap()
    );
    assert!(
        first
            .claim_push_jobs(STORE_ORIGIN, "worker-d", t0 + 500, 60, 10)
            .await
            .unwrap()
            .is_empty()
    );
}

async fn retries_end_in_the_dead_letter_state(
    store: &mut Store,
    executor: &PushExecutor,
    transport: &TestTransport,
    t0: u64,
) {
    let author = pubkey(0x31);
    let event = mention(0x34, &author, t0 + 20, "retry me");
    assert_stored(store.admit(&event, t0 + 20).await.unwrap());
    push::match_events(store, executor, t0 + 20).await.unwrap();
    let retry = WakeOutcome::Retry {
        after_seconds: None,
        reason: "gateway_unavailable",
    };
    transport.script([retry, retry, retry]);
    let before = transport.sent().len();
    let first = push::deliver_due(store, executor, "w", t0 + 20)
        .await
        .unwrap();
    assert_eq!(first.retried, 1);
    // Not due yet.
    assert_eq!(
        push::deliver_due(store, executor, "w", t0 + 25)
            .await
            .unwrap()
            .claimed,
        0
    );
    assert_eq!(
        push::deliver_due(store, executor, "w", t0 + 30)
            .await
            .unwrap()
            .retried,
        1
    );
    assert_eq!(
        push::deliver_due(store, executor, "w", t0 + 50)
            .await
            .unwrap()
            .dead,
        1
    );
    assert_eq!(transport.sent().len(), before + 3);
    let job = job_for(store, &event.id).await;
    assert_eq!(job.state, "dead");
    assert_eq!(job.last_error.as_deref(), Some("retries_exhausted"));
    assert_eq!(job.attempts, 3);

    // A delivered wake carries the endpoint and the job ID, nothing else.
    let delivered = mention(0x34, &author, t0 + 60, "deliver me");
    assert_stored(store.admit(&delivered, t0 + 60).await.unwrap());
    push::match_events(store, executor, t0 + 60).await.unwrap();
    assert_eq!(
        push::deliver_due(store, executor, "w", t0 + 60)
            .await
            .unwrap()
            .delivered,
        1
    );
    let job = job_for(store, &delivered.id).await;
    assert_eq!(job.state, "delivered");
    let request = transport.sent().last().unwrap().clone();
    assert_eq!(request.endpoint, "grant-b");
    assert_eq!(request.request_id, job.job_id);
    assert!(request.expires_at <= t0 + 60 + push::WAKE_TTL_SECONDS);
    let body = push::transport::delivery_body(&request);
    assert!(!body.contains(&delivered.id));
    assert!(!body.contains("deliver me"));

    // A permanently invalid endpoint disables that lease generation.
    transport.script([WakeOutcome::InvalidEndpoint]);
    let invalid = mention(0x34, &author, t0 + 61, "invalid");
    assert_stored(store.admit(&invalid, t0 + 61).await.unwrap());
    push::match_events(store, executor, t0 + 61).await.unwrap();
    assert_eq!(
        push::deliver_due(store, executor, "w", t0 + 61)
            .await
            .unwrap()
            .dead,
        1
    );
    let state = store
        .push_lease_state(STORE_ORIGIN, &author, "phone")
        .await
        .unwrap()
        .unwrap();
    assert!(state.endpoint_invalid_at.is_some());
    let ignored = mention(0x34, &author, t0 + 62, "ignored");
    assert_stored(store.admit(&ignored, t0 + 62).await.unwrap());
    assert_eq!(
        push::match_events(store, executor, t0 + 62)
            .await
            .unwrap()
            .jobs,
        0
    );
    // A rotated endpoint at a higher generation reactivates the lease.
    let subs = json!([{ "filter": { "kinds": [1], "#p": [author] }, "class": "default" }]);
    let rotated = lease(
        0x31,
        "phone",
        t0 + 63,
        t0 + 3_600,
        active(2, "grant-b2", &subs),
    );
    assert_stored(admit(store, executor, &rotated, t0 + 63).await);
    let after = mention(0x34, &author, t0 + 64, "after rotation");
    assert_stored(store.admit(&after, t0 + 64).await.unwrap());
    assert_eq!(
        push::match_events(store, executor, t0 + 64)
            .await
            .unwrap()
            .jobs,
        1
    );
    push::deliver_due(store, executor, "w", t0 + 64)
        .await
        .unwrap();
    assert_eq!(transport.sent().last().unwrap().endpoint, "grant-b2");
}

async fn revoked_devices_get_no_further_wakes(
    store: &mut Store,
    executor: &PushExecutor,
    transport: &TestTransport,
    t0: u64,
) {
    let author = pubkey(0x41);
    let subs = json!([{ "filter": { "kinds": [1], "#p": [author] }, "class": "time_sensitive" }]);
    let device = lease(
        0x41,
        "tablet",
        t0 + 70,
        t0 + 3_600,
        active(1, "grant-d", &subs),
    );
    assert_stored(admit(store, executor, &device, t0 + 70).await);

    // One job waits and one is in flight when the device is revoked.
    let queued = mention(0x44, &author, t0 + 71, "queued");
    let in_flight = mention(0x44, &author, t0 + 71, "in flight");
    assert_stored(store.admit(&queued, t0 + 71).await.unwrap());
    assert_stored(store.admit(&in_flight, t0 + 71).await.unwrap());
    assert_eq!(
        push::match_events(store, executor, t0 + 71)
            .await
            .unwrap()
            .jobs,
        2
    );
    let claimed = store
        .claim_push_jobs(STORE_ORIGIN, "slow", t0 + 71, 60, 1)
        .await
        .unwrap();
    assert_eq!(claimed.len(), 1);

    let revoke = lease(0x41, "tablet", t0 + 72, t0 + 3_600, tombstone(2));
    assert_stored(admit(store, executor, &revoke, t0 + 72).await);
    for event in [&queued, &in_flight] {
        let job = job_for(store, &event.id).await;
        assert_eq!(job.state, "suppressed");
        assert_eq!(job.last_error.as_deref(), Some("lease_replaced"));
    }
    assert_eq!(
        store
            .push_delivery_check(&claimed[0], t0 + 73)
            .await
            .unwrap(),
        PushDeliveryCheck::Suppress("lease_revoked")
    );
    assert!(
        !store
            .finish_push_job(&claimed[0], PushDisposition::Delivered, t0 + 73)
            .await
            .unwrap()
    );

    let before = transport.sent().len();
    assert_eq!(
        push::deliver_due(store, executor, "w", t0 + 200)
            .await
            .unwrap()
            .claimed,
        0
    );
    let later = mention(0x44, &author, t0 + 74, "after revocation");
    assert_stored(store.admit(&later, t0 + 74).await.unwrap());
    assert_eq!(
        push::match_events(store, executor, t0 + 74)
            .await
            .unwrap()
            .jobs,
        0
    );
    assert_eq!(transport.sent().len(), before);
}

async fn removed_members_get_no_further_wakes(
    database_url: &str,
    store: &mut Store,
    executor: &PushExecutor,
    transport: &TestTransport,
    t0: u64,
) {
    let (sql, connection) = tokio_postgres::connect(database_url, NoTls).await.unwrap();
    tokio::spawn(connection);
    let member = pubkey(0x51);
    let speaker = pubkey(0x52);
    sql.execute(
        "INSERT INTO relay_group (id, private) VALUES ($1, TRUE)",
        &[&ROOM],
    )
    .await
    .unwrap();
    for key in [&member, &speaker] {
        sql.execute(
            "INSERT INTO relay_group_member (group_id, pubkey) VALUES ($1, $2)",
            &[&ROOM, key],
        )
        .await
        .unwrap();
    }
    let subs = json!([{ "filter": { "kinds": [9], "#h": [ROOM] }, "class": "default" }]);
    let device = lease(
        0x51,
        "laptop",
        t0 + 80,
        t0 + 3_600,
        active(1, "grant-m", &subs),
    );
    assert_stored(admit(store, executor, &device, t0 + 80).await);

    let message = signed_event(
        0x52,
        t0 + 81,
        9,
        vec![Tag::new(vec!["h".into(), ROOM.into()])],
        "room message",
    );
    assert_stored(store.admit(&message, t0 + 81).await.unwrap());
    assert_eq!(
        push::match_events(store, executor, t0 + 81)
            .await
            .unwrap()
            .jobs,
        1
    );

    // Membership ends after the match and before delivery.
    sql.execute(
        "DELETE FROM relay_group_member WHERE group_id = $1 AND pubkey = $2",
        &[&ROOM, &member],
    )
    .await
    .unwrap();
    let before = transport.sent().len();
    let report = push::deliver_due(store, executor, "w", t0 + 82)
        .await
        .unwrap();
    assert_eq!(report.suppressed, 1);
    assert_eq!(transport.sent().len(), before);
    let job = job_for(store, &message.id).await;
    assert_eq!(job.last_error.as_deref(), Some("not_authorized"));

    // Later room messages no longer match for the former member.
    let later = signed_event(
        0x52,
        t0 + 83,
        9,
        vec![Tag::new(vec!["h".into(), ROOM.into()])],
        "later message",
    );
    assert_stored(store.admit(&later, t0 + 83).await.unwrap());
    assert_eq!(
        push::match_events(store, executor, t0 + 83)
            .await
            .unwrap()
            .jobs,
        0
    );
}

async fn gateway_wakes_end_to_end(database_url: &str) {
    let transport = TestTransport::new(Platform::Apns);
    let mut config = GatewayConfig::new(database_url.to_owned(), "127.0.0.1:0".parse().unwrap());
    config.relay_url = Some(GATEWAY_ORIGIN.to_owned());
    config.auth_required = true;
    config.db_connections = 2;
    config.shutdown_grace = Duration::from_secs(2);
    config.relay_signer = Some(RelaySigner::from_secret_hex(&"5a".repeat(32)).unwrap());
    config.identity.pubkey = config
        .relay_signer
        .as_ref()
        .map(|signer| signer.pubkey().to_owned());
    config.limits.events_per_minute_ip = 1_000;
    config.limits.events_per_minute_pubkey = 1_000;
    config.push = Some(executor(GATEWAY_ORIGIN, transport.clone()));
    let gateway = Gateway::start(config).await.unwrap();
    let address = gateway.local_addr();
    let stop = gateway.shutdown_handle();
    let server = tokio::spawn(gateway.run());

    let information = nip11(address);
    assert!(
        information["supported_extensions"]
            .as_array()
            .unwrap()
            .contains(&json!("nip-pl"))
    );
    assert_eq!(information["push"]["origin"], GATEWAY_ORIGIN);
    assert_eq!(information["push"]["app_profiles"][0]["transport"], "apns");

    let author = pubkey(0x61);
    let subs = json!([{ "filter": { "kinds": [1], "#p": [author] }, "class": "default" }]);
    let installed = lease_for(
        0x61,
        "e2e",
        now(),
        now() + 3_600,
        active_at(GATEWAY_ORIGIN, 1, "grant-e2e", &subs),
    );
    let revoke = lease_for(
        0x61,
        "e2e",
        now() + 1,
        now() + 3_600,
        tombstone_at(GATEWAY_ORIGIN, 2),
    );
    let sent = tokio::task::spawn_blocking(move || {
        let mut owner = connect_client(address);
        let challenge = expect_auth_challenge(&mut owner);
        authenticate(&mut owner, 0x61, &challenge);
        send_json(&mut owner, json!(["EVENT", installed]));
        let ok = read_json(&mut owner);
        assert_eq!(ok[2], true, "{ok}");
        let mut sender = connect_client(address);
        let challenge = expect_auth_challenge(&mut sender);
        authenticate(&mut sender, 0x62, &challenge);
        let first = mention(0x62, &pubkey(0x61), now(), "wake the phone");
        send_json(&mut sender, json!(["EVENT", first]));
        assert_eq!(read_json(&mut sender)[2], true);
        (owner, sender, revoke)
    })
    .await
    .unwrap();
    let (mut owner, mut sender, revoke) = sent;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while transport.sent().is_empty() && tokio::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let requests = transport.sent();
    assert_eq!(requests.len(), 1, "one wake for one mention");
    assert_eq!(requests[0].endpoint, "grant-e2e");

    tokio::task::spawn_blocking(move || {
        send_json(&mut owner, json!(["EVENT", revoke]));
        assert_eq!(read_json(&mut owner)[2], true);
        let second = mention(0x62, &pubkey(0x61), now(), "after revocation");
        send_json(&mut sender, json!(["EVENT", second]));
        assert_eq!(read_json(&mut sender)[2], true);
    })
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert_eq!(transport.sent().len(), 1, "a revoked device gets no wake");

    stop.shutdown();
    timeout(Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

/// One executor with an APNs profile and an FCM profile. Each lease wakes
/// through its own profile's transport; an unknown profile refuses; each
/// profile keeps its own quota, retry bounds, and dead-letter state; and
/// jobs for a withdrawn profile are suppressed.
async fn one_executor_serves_both_platforms(store: &mut Store, t0: u64) {
    let apns = TestTransport::new(Platform::Apns);
    let fcm = TestTransport::new(Platform::Fcm);
    let mut ios = PushProfile::new(PROFILE, apns.clone());
    ios.retry = Some(RetryPolicy {
        max_attempts: 2,
        base_delay_seconds: 10,
        max_delay_seconds: 600,
    });
    let mut android = PushProfile::new(ANDROID_PROFILE, fcm.clone());
    android.retry = Some(RetryPolicy {
        max_attempts: 4,
        base_delay_seconds: 5,
        max_delay_seconds: 600,
    });
    android.max_leases_per_pubkey = Some(1);
    let executor = PushExecutor::with_profiles(
        SecretKey::from_byte_array([EXECUTOR; 32]).unwrap(),
        PROFILES_ORIGIN.to_owned(),
        vec![ios, android],
    );
    executor.validate().unwrap();
    let document = nostr::push_lease::descriptor_document(&executor.descriptor());
    assert_eq!(
        document["app_profiles"],
        json!([
            { "id": PROFILE, "transport": "apns" },
            { "id": ANDROID_PROFILE, "transport": "fcm" },
        ])
    );
    // Fix this origin's cursor before its first event.
    push::match_events(store, &executor, t0 + 100)
        .await
        .unwrap();

    let author = pubkey(0x71);
    let subs = json!([{ "filter": { "kinds": [1], "#p": [author] }, "class": "default" }]);
    let at = |profile: &str, transport: &str, generation: u64, endpoint: &str| {
        json!({
            "v": 1,
            "origin": PROFILES_ORIGIN,
            "app_profile": profile,
            "transport": transport,
            "endpoint": endpoint,
            "generation": generation,
            "active": true,
            "subscriptions": subs,
        })
    };
    let iphone = lease(
        0x71,
        "iphone",
        t0 + 100,
        t0 + 3_600,
        at(PROFILE, "apns", 1, "grant-ios"),
    );
    let pixel = lease(
        0x71,
        "pixel",
        t0 + 100,
        t0 + 3_600,
        at(ANDROID_PROFILE, "fcm", 1, "grant-android"),
    );
    assert_stored(admit(store, &executor, &iphone, t0 + 100).await);
    assert_stored(admit(store, &executor, &pixel, t0 + 100).await);

    // A profile the executor does not serve, and a served profile named
    // with the other transport, both refuse before any state changes.
    for plaintext in [
        at("app.test/web", "apns", 1, "grant-web"),
        at("app.test/web", "fcm", 1, "grant-web"),
        at(ANDROID_PROFILE, "apns", 1, "grant-crossed"),
        at(PROFILE, "fcm", 1, "grant-crossed"),
    ] {
        let refused = lease(0x71, "other", t0 + 101, t0 + 3_600, plaintext);
        assert_eq!(
            push::lease_write(&executor, &refused, t0 + 101).unwrap_err(),
            "transport mismatch"
        );
    }
    // The FCM profile's own quota of one refuses a second Android lease,
    // while the APNs profile still takes a second device.
    let tablet = lease(
        0x71,
        "tablet",
        t0 + 101,
        t0 + 3_600,
        at(ANDROID_PROFILE, "fcm", 1, "grant-tablet"),
    );
    assert_eq!(
        admit(store, &executor, &tablet, t0 + 101).await,
        AdmissionOutcome::Rejected(AdmissionRejection::PushLease("lease quota exceeded"))
    );
    let ipad = lease(
        0x71,
        "ipad",
        t0 + 101,
        t0 + 3_600,
        at(PROFILE, "apns", 1, "grant-ipad"),
    );
    assert_stored(admit(store, &executor, &ipad, t0 + 101).await);
    let revoke_ipad = lease(
        0x71,
        "ipad",
        t0 + 102,
        t0 + 3_600,
        tombstone_at(PROFILES_ORIGIN, 2),
    );
    assert_stored(admit(store, &executor, &revoke_ipad, t0 + 102).await);

    // One mention wakes the iPhone through APNs and the Pixel through FCM.
    let first = mention(0x74, &author, t0 + 103, "both platforms");
    assert_stored(store.admit(&first, t0 + 103).await.unwrap());
    assert_eq!(
        push::match_events(store, &executor, t0 + 103)
            .await
            .unwrap()
            .jobs,
        2
    );
    let report = push::deliver_due(store, &executor, "w", t0 + 103)
        .await
        .unwrap();
    assert_eq!((report.claimed, report.delivered), (2, 2));
    let apns_sent = apns.sent();
    let fcm_sent = fcm.sent();
    assert_eq!(apns_sent.len(), 1);
    assert_eq!(fcm_sent.len(), 1);
    assert_eq!(apns_sent[0].endpoint, "grant-ios");
    assert_eq!(fcm_sent[0].endpoint, "grant-android");
    assert_ne!(apns_sent[0].request_id, fcm_sent[0].request_id);
    let jobs = jobs_at(store, PROFILES_ORIGIN, &first.id).await;
    assert_eq!(jobs.len(), 2);
    assert!(jobs.iter().all(|job| job.state == "delivered"));
    // A second pass claims nothing: each job was claimed once.
    assert_eq!(
        push::deliver_due(store, &executor, "w", t0 + 104)
            .await
            .unwrap()
            .claimed,
        0
    );

    // Both gateways fail. APNs allows two attempts and FCM four, with
    // their own backoff, so the APNs job reaches the dead-letter state
    // while the FCM job keeps retrying and then succeeds.
    let retry = WakeOutcome::Retry {
        after_seconds: None,
        reason: "gateway_unavailable",
    };
    apns.script([retry, retry]);
    fcm.script([retry, retry]);
    let second = mention(0x74, &author, t0 + 110, "retry both");
    assert_stored(store.admit(&second, t0 + 110).await.unwrap());
    push::match_events(store, &executor, t0 + 110)
        .await
        .unwrap();
    let report = push::deliver_due(store, &executor, "w", t0 + 110)
        .await
        .unwrap();
    assert_eq!(report.retried, 2);
    // FCM retries after 5 seconds, APNs after 10.
    let report = push::deliver_due(store, &executor, "w", t0 + 115)
        .await
        .unwrap();
    assert_eq!((report.claimed, report.retried), (1, 1));
    let report = push::deliver_due(store, &executor, "w", t0 + 120)
        .await
        .unwrap();
    assert_eq!((report.claimed, report.dead), (1, 1));
    let report = push::deliver_due(store, &executor, "w", t0 + 125)
        .await
        .unwrap();
    assert_eq!((report.claimed, report.delivered), (1, 1));
    let jobs = jobs_at(store, PROFILES_ORIGIN, &second.id).await;
    let ios_job = jobs
        .iter()
        .find(|job| job.installation == "iphone")
        .unwrap();
    let android_job = jobs.iter().find(|job| job.installation == "pixel").unwrap();
    assert_eq!(ios_job.state, "dead");
    assert_eq!(ios_job.last_error.as_deref(), Some("retries_exhausted"));
    assert_eq!(ios_job.attempts, 2);
    assert_eq!(android_job.state, "delivered");
    assert_eq!(android_job.attempts, 3);
    assert_eq!(apns.sent().len(), 3);
    assert_eq!(fcm.sent().len(), 4);

    // A restart without the FCM profile suppresses its pending job rather
    // than sending it through the wrong transport, and still wakes iOS.
    let third = mention(0x74, &author, t0 + 130, "withdrawn");
    assert_stored(store.admit(&third, t0 + 130).await.unwrap());
    assert_eq!(
        push::match_events(store, &executor, t0 + 130)
            .await
            .unwrap()
            .jobs,
        2
    );
    let mut ios_only = executor.clone();
    ios_only.profiles.truncate(1);
    ios_only.validate().unwrap();
    let report = push::deliver_due(store, &ios_only, "w", t0 + 130)
        .await
        .unwrap();
    assert_eq!((report.delivered, report.suppressed), (1, 1));
    assert_eq!(fcm.sent().len(), 4);
    assert_eq!(apns.sent().len(), 4);
    let jobs = jobs_at(store, PROFILES_ORIGIN, &third.id).await;
    let android_job = jobs.iter().find(|job| job.installation == "pixel").unwrap();
    assert_eq!(android_job.last_error.as_deref(), Some("profile_withdrawn"));
    // Revoking the withdrawn profile's lease still succeeds.
    let revoke_pixel = lease(
        0x71,
        "pixel",
        t0 + 131,
        t0 + 3_600,
        tombstone_at(PROFILES_ORIGIN, 2),
    );
    assert_stored(admit(store, &ios_only, &revoke_pixel, t0 + 131).await);
}

async fn jobs_at(
    store: &Store,
    origin: &str,
    event_id: &str,
) -> Vec<nostr_relay::store::PushJobRecord> {
    store
        .push_jobs(origin, 1_000)
        .await
        .unwrap()
        .into_iter()
        .filter(|job| job.event_id == event_id)
        .collect()
}

fn executor(origin: &str, transport: Arc<TestTransport>) -> PushExecutor {
    PushExecutor::new(
        SecretKey::from_byte_array([EXECUTOR; 32]).unwrap(),
        origin.to_owned(),
        PROFILE.to_owned(),
        transport,
    )
}

async fn admit(
    store: &mut Store,
    executor: &PushExecutor,
    event: &Event,
    now: u64,
) -> AdmissionOutcome {
    let write = push::lease_write(executor, event, now).unwrap();
    store
        .admit_push_lease(event, now, &write, None)
        .await
        .unwrap()
}

async fn job_for(store: &Store, event_id: &str) -> nostr_relay::store::PushJobRecord {
    store
        .push_jobs(STORE_ORIGIN, 1_000)
        .await
        .unwrap()
        .into_iter()
        .find(|job| job.event_id == event_id)
        .unwrap()
}

#[track_caller]
fn assert_stored(outcome: AdmissionOutcome) {
    assert!(
        matches!(outcome, AdmissionOutcome::Stored { .. }),
        "{outcome:?}"
    );
}

fn active(generation: u64, endpoint: &str, subscriptions: &Value) -> Value {
    active_at(STORE_ORIGIN, generation, endpoint, subscriptions)
}

fn active_at(origin: &str, generation: u64, endpoint: &str, subscriptions: &Value) -> Value {
    json!({
        "v": 1,
        "origin": origin,
        "app_profile": PROFILE,
        "transport": "apns",
        "endpoint": endpoint,
        "generation": generation,
        "active": true,
        "subscriptions": subscriptions,
    })
}

fn tombstone(generation: u64) -> Value {
    tombstone_at(STORE_ORIGIN, generation)
}

fn tombstone_at(origin: &str, generation: u64) -> Value {
    json!({ "v": 1, "origin": origin, "generation": generation, "active": false })
}

fn lease(author: u8, d: &str, created_at: u64, expiration: u64, plaintext: Value) -> Event {
    lease_for(author, d, created_at, expiration, plaintext)
}

fn lease_for(author: u8, d: &str, created_at: u64, expiration: u64, plaintext: Value) -> Event {
    let secret = SecretKey::from_byte_array([author; 32]).unwrap();
    let executor = xonly(EXECUTOR);
    let mut nonce = [author; 32];
    nonce[..8].copy_from_slice(&created_at.to_be_bytes());
    let ciphertext = encrypt(
        &plaintext.to_string(),
        &conversation_key(&secret, &executor),
        nonce,
    )
    .unwrap();
    signed_event(
        author,
        created_at,
        30_350,
        vec![
            Tag::new(vec!["d".into(), d.into()]),
            Tag::new(vec!["expiration".into(), expiration.to_string()]),
            Tag::new(vec!["exec".into(), "current".into()]),
        ],
        &ciphertext,
    )
}

fn mention(author: u8, target: &str, created_at: u64, content: &str) -> Event {
    signed_event(
        author,
        created_at,
        1,
        vec![Tag::new(vec!["p".into(), target.to_owned()])],
        content,
    )
}

fn signed_event(
    secret_byte: u8,
    created_at: u64,
    kind: u16,
    tags: Vec<Tag>,
    content: &str,
) -> Event {
    let secp = Secp256k1::new();
    let secret = SecretKey::from_byte_array([secret_byte; 32]).unwrap();
    let keypair = Keypair::from_secret_key(&secp, &secret);
    let mut event = Event {
        id: "0".repeat(64),
        pubkey: keypair.x_only_public_key().0.to_string(),
        created_at,
        kind,
        tags,
        content: content.to_owned(),
        sig: "0".repeat(128),
    };
    let id = event.computed_id_bytes().unwrap();
    event.id = event.computed_id().unwrap();
    event.sig = secp.sign_schnorr_no_aux_rand(&id, &keypair).to_string();
    event
}

fn xonly(secret_byte: u8) -> XOnlyPublicKey {
    let secret = SecretKey::from_byte_array([secret_byte; 32]).unwrap();
    Keypair::from_secret_key(&Secp256k1::new(), &secret)
        .x_only_public_key()
        .0
}

fn pubkey(secret_byte: u8) -> String {
    xonly(secret_byte).to_string()
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn nip11(address: SocketAddr) -> Value {
    let mut stream = StdTcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream
        .write_all(
            b"GET / HTTP/1.1\r\nHost: relay.test\r\nAccept: application/nostr+json\r\nConnection: close\r\n\r\n",
        )
        .unwrap();
    let mut response = Vec::new();
    let _ = stream.read_to_end(&mut response);
    let split = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .unwrap();
    serde_json::from_slice(&response[split + 4..]).unwrap()
}

fn connect_client(address: SocketAddr) -> WebSocket<StdTcpStream> {
    let stream = StdTcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let (websocket, _) = client(format!("ws://{address}/"), stream).unwrap();
    websocket
}

fn expect_auth_challenge(websocket: &mut WebSocket<StdTcpStream>) -> String {
    let message = read_json(websocket);
    assert_eq!(message[0], "AUTH");
    message[1].as_str().unwrap().to_owned()
}

fn authenticate(websocket: &mut WebSocket<StdTcpStream>, secret: u8, challenge: &str) {
    let event = signed_event(
        secret,
        now(),
        22_242,
        vec![
            Tag::new(vec!["relay".into(), GATEWAY_ORIGIN.into()]),
            Tag::new(vec!["challenge".into(), challenge.to_owned()]),
        ],
        "",
    );
    send_json(websocket, json!(["AUTH", event]));
    let response = read_json(websocket);
    assert_eq!(response[0], "OK");
    assert_eq!(response[2], true);
}

fn send_json(websocket: &mut WebSocket<StdTcpStream>, value: Value) {
    websocket.send(Message::text(value.to_string())).unwrap();
}

#[track_caller]
fn read_json(websocket: &mut WebSocket<StdTcpStream>) -> Value {
    loop {
        match websocket.read().unwrap() {
            Message::Text(text) => return serde_json::from_str(text.as_str()).unwrap(),
            Message::Ping(_) | Message::Pong(_) => {}
            other => panic!("unexpected WebSocket message: {other:?}"),
        }
    }
}
