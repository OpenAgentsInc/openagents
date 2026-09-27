//! Relay to gateway to fake APNs, end to end, against a live Postgres.
//!
//! A real relay runs its PL executor with the `ApnsGateway` adapter pointed
//! at a running push gateway, which sends to a local cleartext HTTP/2 fake
//! APNs. The device side is the crate's `Enrollment`: it registers a native
//! token, obtains a capability, and publishes, rotates, and revokes its
//! lease over NIP-42. A second test runs one relay with an APNs profile
//! and an FCM profile at once and wakes an iPhone and an Android phone in
//! the same run. Set `NOSTR_RELAY_TEST_DATABASE_URL` and
//! `NOSTR_RELAY_TEST_ALLOW_DESTRUCTIVE=1` to run them.

mod common;

use std::{net::TcpListener as StdTcpListener, sync::Arc, time::Duration};

use common::{APNS_PROFILE, FCM_PROFILE, Fake, Scripted, TOPIC, es256_key, rsa_key};
use nostr_relay::{
    domain::{RelaySigner, Tag},
    gateway::{
        Gateway, GatewayConfig, PushExecutor, PushProfile, RetryPolicy,
        push::{ApnsGateway, FcmGateway, WakeTransport},
    },
};
use push_gateway::{
    client::{
        ClientError, Enrollment, LeaseSpec, SyncOutcome, discover, lease_event, pubkey_hex, publish,
    },
    wire::Transport,
};
use secp256k1::SecretKey;
use serde_json::json;

const EXECUTOR: u8 = 0x42;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_relay_wakes_a_device_through_the_gateway() {
    let Ok(database_url) = std::env::var("NOSTR_RELAY_TEST_DATABASE_URL") else {
        eprintln!("skipped: set NOSTR_RELAY_TEST_DATABASE_URL or run scripts/test-postgres.sh");
        return;
    };
    if std::env::var("NOSTR_RELAY_TEST_ALLOW_DESTRUCTIVE").as_deref() != Ok("1") {
        eprintln!("skipped: the suite writes relay state; use a disposable database");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let apns = Fake::start(Scripted::new(200, "")).await;
    let (pem, _) = es256_key();
    let relay_signer = RelaySigner::from_secret_hex(&"5a".repeat(32)).unwrap();
    let gateway = push_gateway::server::start(common::config(
        dir.path(),
        relay_signer.pubkey(),
        Some((&apns.base, &pem)),
        None,
    ))
    .await
    .unwrap();

    let (relay_url, stop, server) = start_relay(
        database_url,
        &relay_signer,
        vec![PushProfile::new(
            APNS_PROFILE,
            Arc::new(
                ApnsGateway::new(
                    &format!("http://{}", gateway.delivery_addr),
                    relay_signer.clone(),
                )
                .unwrap(),
            ),
        )],
    )
    .await;

    let device = SecretKey::from_byte_array([0x61; 32]).unwrap();
    let device_pubkey = pubkey_hex(&device);
    let sender = SecretKey::from_byte_array([0x62; 32]).unwrap();
    let mut enrollment = Enrollment::new(
        &relay_url,
        &format!("http://{}", gateway.registration_addr),
        APNS_PROFILE,
        &device_pubkey,
    );
    enrollment.subscriptions =
        json!([{ "filter": { "kinds": [1], "#p": [device_pubkey] }, "class": "default" }]);
    let mut saves = 0;
    let mut save = |_: &Enrollment| -> Result<(), String> {
        saves += 1;
        Ok(())
    };

    // Enroll, then wake once.
    let first_token = "a1".repeat(32);
    let outcome = enrollment
        .sync(&device, &first_token, push_gateway::unix_now(), &mut save)
        .await
        .unwrap();
    assert_eq!(outcome, SyncOutcome::Published { generation: 1 });
    assert_eq!(
        enrollment
            .sync(&device, &first_token, push_gateway::unix_now(), &mut save)
            .await
            .unwrap(),
        SyncOutcome::Current
    );
    mention(&relay_url, &sender, &device_pubkey, "first").await;
    let sent = wait_for(&apns, 1).await;
    assert_eq!(sent[0].path, format!("/3/device/{first_token}"));
    assert_eq!(sent[0].version, axum::http::Version::HTTP_2);
    assert_eq!(sent[0].body, nostr::push_lease::APNS_BODY.as_bytes());
    assert_eq!(sent[0].headers["apns-topic"], TOPIC);
    assert!(push_gateway::wire::valid_uuid(&sent[0].headers["apns-id"]));

    // A rotated token gets the next wake; the old one gets none.
    tokio::time::sleep(Duration::from_millis(1_100)).await;
    let second_token = "b2".repeat(32);
    assert_eq!(
        enrollment
            .sync(&device, &second_token, push_gateway::unix_now(), &mut save)
            .await
            .unwrap(),
        SyncOutcome::Published { generation: 2 }
    );
    mention(&relay_url, &sender, &device_pubkey, "second").await;
    let sent = wait_for(&apns, 2).await;
    assert_eq!(sent[1].path, format!("/3/device/{second_token}"));

    // After revocation, no wake arrives.
    tokio::time::sleep(Duration::from_millis(1_100)).await;
    enrollment
        .revoke(&device, push_gateway::unix_now(), &mut save)
        .await
        .unwrap();
    assert!(enrollment.lease.is_none() && enrollment.installation.is_none());
    mention(&relay_url, &sender, &device_pubkey, "after revocation").await;
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert_eq!(apns.received().len(), 2, "a revoked device gets no wake");
    assert!(saves >= 6);

    stop.shutdown();
    let _ = tokio::time::timeout(Duration::from_secs(5), server).await;
    gateway.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn one_relay_wakes_ios_and_android_devices() {
    let Ok(database_url) = std::env::var("NOSTR_RELAY_TEST_DATABASE_URL") else {
        eprintln!("skipped: set NOSTR_RELAY_TEST_DATABASE_URL or run scripts/test-postgres.sh");
        return;
    };
    if std::env::var("NOSTR_RELAY_TEST_ALLOW_DESTRUCTIVE").as_deref() != Ok("1") {
        eprintln!("skipped: the suite writes relay state; use a disposable database");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let apns = Fake::start(Scripted::new(200, "")).await;
    let oauth = Fake::start(Scripted::new(
        200,
        r#"{"access_token":"access-token-1","expires_in":3600,"token_type":"Bearer"}"#,
    ))
    .await;
    let fcm = Fake::start(Scripted::new(
        200,
        r#"{"name":"projects/demo-project/messages/1"}"#,
    ))
    .await;
    let (apns_pem, _) = es256_key();
    let (rsa_pem, _) = rsa_key(dir.path());
    let account = json!({
        "type": "service_account",
        "project_id": "demo-project",
        "private_key_id": "key-1",
        "private_key": rsa_pem,
        "client_email": "sender@demo-project.iam.gserviceaccount.com",
        "token_uri": format!("{}/token", oauth.base),
    })
    .to_string();
    let relay_signer = RelaySigner::from_secret_hex(&"5b".repeat(32)).unwrap();
    let gateway = push_gateway::server::start(common::config(
        dir.path(),
        relay_signer.pubkey(),
        Some((&apns.base, &apns_pem)),
        Some((&fcm.base, &account)),
    ))
    .await
    .unwrap();
    let delivery = format!("http://{}", gateway.delivery_addr);
    let registration = format!("http://{}", gateway.registration_addr);

    // One relay, two profiles. The FCM profile retries more often than the
    // APNs profile, and both reach the same gateway here; a deployment may
    // point each profile at its own gateway.
    let ios = PushProfile::new(
        APNS_PROFILE,
        Arc::new(ApnsGateway::new(&delivery, relay_signer.clone()).unwrap()),
    );
    let mut android = PushProfile::new(
        FCM_PROFILE,
        Arc::new(FcmGateway::new(&delivery, relay_signer.clone()).unwrap())
            as Arc<dyn WakeTransport>,
    );
    android.retry = Some(RetryPolicy {
        max_attempts: 8,
        base_delay_seconds: 1,
        max_delay_seconds: 60,
    });
    let (relay_url, stop, server) =
        start_relay(database_url, &relay_signer, vec![ios, android]).await;

    let advertised = discover(&relay_url).await.unwrap();
    assert_eq!(advertised.transport(APNS_PROFILE), Some(Transport::Apns));
    assert_eq!(advertised.transport(FCM_PROFILE), Some(Transport::Fcm));

    let iphone = SecretKey::from_byte_array([0x63; 32]).unwrap();
    let android_phone = SecretKey::from_byte_array([0x64; 32]).unwrap();
    let sender = SecretKey::from_byte_array([0x65; 32]).unwrap();
    let mut saved = |_: &Enrollment| -> Result<(), String> { Ok(()) };
    let mut enrollments = Vec::new();
    for (device, profile, token) in [
        (&iphone, APNS_PROFILE, "c3".repeat(32)),
        (
            &android_phone,
            FCM_PROFILE,
            "fcm-token-android_1:APA91b".to_owned(),
        ),
    ] {
        let device_pubkey = pubkey_hex(device);
        let mut enrollment = Enrollment::new(&relay_url, &registration, profile, &device_pubkey);
        enrollment.subscriptions =
            json!([{ "filter": { "kinds": [1], "#p": [device_pubkey] }, "class": "default" }]);
        assert_eq!(
            enrollment
                .sync(device, &token, push_gateway::unix_now(), &mut saved)
                .await
                .unwrap(),
            SyncOutcome::Published { generation: 1 }
        );
        enrollments.push((device_pubkey, token));
    }

    // One event names both devices; each wakes through its own provider.
    let signer = RelaySigner::from_secret_hex(&sender.display_secret().to_string()).unwrap();
    let event = signer.sign(
        push_gateway::unix_now(),
        1,
        enrollments
            .iter()
            .map(|(pubkey, _)| Tag::new(vec!["p".into(), pubkey.clone()]))
            .collect(),
        "both platforms".to_owned(),
    );
    publish(&relay_url, &sender, &event).await.unwrap();
    let apns_sent = wait_for(&apns, 1).await;
    let fcm_sent = wait_for(&fcm, 1).await;
    assert_eq!(apns_sent[0].path, format!("/3/device/{}", enrollments[0].1));
    assert_eq!(apns_sent[0].body, nostr::push_lease::APNS_BODY.as_bytes());
    let message = fcm_sent[0].json();
    assert_eq!(fcm_sent[0].path, "/v1/projects/demo-project/messages:send");
    assert_eq!(message["message"]["token"], enrollments[1].1);
    assert_eq!(
        message["message"]["data"].to_string(),
        nostr::push_lease::FCM_DATA
    );

    // A transient FCM failure retries under the FCM profile's policy while
    // APNs delivers at once.
    fcm.script([Scripted::new(503, r#"{"error":{"status":"UNAVAILABLE"}}"#)]);
    tokio::time::sleep(Duration::from_millis(1_100)).await;
    let retried = signer.sign(
        push_gateway::unix_now(),
        1,
        enrollments
            .iter()
            .map(|(pubkey, _)| Tag::new(vec!["p".into(), pubkey.clone()]))
            .collect(),
        "retry android".to_owned(),
    );
    publish(&relay_url, &sender, &retried).await.unwrap();
    wait_for(&apns, 2).await;
    let fcm_sent = wait_for(&fcm, 3).await;
    assert_eq!(fcm_sent[2].json()["message"]["token"], enrollments[1].1);

    // A lease for a profile the relay does not serve, or for a served
    // profile with the other transport, is refused.
    for (profile, transport) in [
        ("app.test/web", Transport::Apns),
        (FCM_PROFILE, Transport::Apns),
    ] {
        let lease = lease_event(
            &iphone,
            &advertised,
            &LeaseSpec {
                d: "0f0e0d0c0b0a09080706050403020100",
                generation: 1,
                app_profile: profile,
                transport,
                endpoint: "unused-grant",
                subscriptions: &json!([{ "filter": { "kinds": [1], "#p": [enrollments[0].0] }, "class": "default" }]),
                expiration: push_gateway::unix_now() + 3_600,
            },
            push_gateway::unix_now(),
        )
        .unwrap();
        match publish(&relay_url, &iphone, &lease).await {
            Err(ClientError::RelayRefused(message)) => {
                assert_eq!(message, "invalid: transport mismatch");
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    stop.shutdown();
    let _ = tokio::time::timeout(Duration::from_secs(5), server).await;
    gateway.stop().await;
}

/// Start a relay whose push executor serves `profiles`. The relay URL is
/// its push origin and its NIP-42 relay tag, so it must name the port
/// before the relay starts.
async fn start_relay(
    database_url: String,
    relay_signer: &RelaySigner,
    profiles: Vec<PushProfile>,
) -> (
    String,
    nostr_relay::gateway::ShutdownHandle,
    tokio::task::JoinHandle<Result<(), nostr_relay::gateway::GatewayError>>,
) {
    let port = {
        let probe = StdTcpListener::bind("127.0.0.1:0").unwrap();
        probe.local_addr().unwrap().port()
    };
    let relay_url = format!("ws://127.0.0.1:{port}");
    let mut config = GatewayConfig::new(database_url, format!("127.0.0.1:{port}").parse().unwrap());
    config.relay_url = Some(relay_url.clone());
    config.auth_required = true;
    config.db_connections = 2;
    config.shutdown_grace = Duration::from_secs(2);
    config.relay_signer = Some(relay_signer.clone());
    config.identity.pubkey = Some(relay_signer.pubkey().to_owned());
    config.limits.events_per_minute_ip = 1_000;
    config.limits.events_per_minute_pubkey = 1_000;
    config.push = Some(PushExecutor::with_profiles(
        SecretKey::from_byte_array([EXECUTOR; 32]).unwrap(),
        relay_url.clone(),
        profiles,
    ));
    let relay = Gateway::start(config).await.unwrap();
    let stop = relay.shutdown_handle();
    let server = tokio::spawn(relay.run());
    (relay_url, stop, server)
}

async fn mention(relay_url: &str, sender: &SecretKey, target: &str, content: &str) {
    let signer = RelaySigner::from_secret_hex(&sender.display_secret().to_string()).unwrap();
    let event = signer.sign(
        push_gateway::unix_now(),
        1,
        vec![Tag::new(vec!["p".into(), target.to_owned()])],
        content.to_owned(),
    );
    publish(relay_url, sender, &event).await.unwrap();
}

async fn wait_for(fake: &Fake, count: usize) -> Vec<common::Recorded> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    while fake.received().len() < count && tokio::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let received = fake.received();
    assert_eq!(received.len(), count, "expected {count} provider requests");
    received
}
