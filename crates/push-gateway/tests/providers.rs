//! The gateway against local fake APNs (cleartext HTTP/2) and FCM servers.
//!
//! Deliveries are posted by the relay's own `ApnsGateway` and `FcmGateway`
//! adapters, so these tests check the exact request the relay sends and the
//! outcome it reads back. Registration uses the device client.

mod common;

use common::{
    APNS_PROFILE, FCM_PROFILE, Fake, KEY_ID, Scripted, TEAM_ID, TOPIC, es256_key, rsa_key,
    verify_jwt,
};
use nostr_relay::{
    domain::RelaySigner,
    gateway::push::{ApnsGateway, FcmGateway, WakeOutcome, WakeRequest, WakeTransport},
};
use push_gateway::{
    client::{ClientError, GatewayClient},
    unix_now,
    wire::{self, DelegationRequest, InstallationRevokeRequest},
};
use secp256k1::SecretKey;
use serde_json::json;

const APNS_TOKEN: &str = "0a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f9";

fn relay() -> RelaySigner {
    RelaySigner::from_secret_hex(&"5a".repeat(32)).unwrap()
}

fn device(byte: u8) -> SecretKey {
    SecretKey::from_byte_array([byte; 32]).unwrap()
}

fn wake(grant: &str) -> WakeRequest {
    WakeRequest {
        request_id: wire::random_uuid(),
        endpoint: grant.to_owned(),
        expires_at: unix_now() + 120,
    }
}

async fn delegate(
    client: &GatewayClient,
    secret: &SecretKey,
    handle: &str,
    epoch: u64,
    generation: u64,
    relay_pubkey: &str,
) -> Result<String, ClientError> {
    let now = unix_now();
    client
        .delegate(
            secret,
            &DelegationRequest {
                v: 1,
                installation_handle: handle.to_owned(),
                endpoint_epoch: epoch,
                generation,
                relay_pubkey: relay_pubkey.to_owned(),
                not_before: now - 60,
                expires_at: now + 86_400,
            },
        )
        .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apns_delivery_is_fixed_idempotent_bounded_and_typed() {
    let dir = tempfile::tempdir().unwrap();
    let apns = Fake::start(Scripted::new(200, "")).await;
    let (pem, public_key) = es256_key();
    let relay = relay();
    let running = push_gateway::server::start(common::config(
        dir.path(),
        relay.pubkey(),
        Some((&apns.base, &pem)),
        None,
    ))
    .await
    .unwrap();
    let client = GatewayClient::new(&format!("http://{}", running.registration_addr)).unwrap();
    let transport =
        ApnsGateway::new(&format!("http://{}", running.delivery_addr), relay.clone()).unwrap();
    let owner = device(0x61);

    // Register and delegate.
    let installation = client
        .register(&owner, APNS_PROFILE, APNS_TOKEN, unix_now() + 86_400)
        .await
        .unwrap();
    assert_eq!(installation.endpoint_epoch, 1);
    // A different body, because an identical signed request within the
    // same second is a replay and refuses.
    let again = client
        .register(&owner, APNS_PROFILE, APNS_TOKEN, unix_now() + 86_401)
        .await
        .unwrap();
    assert_eq!(again.installation_handle, installation.installation_handle);
    let handle = installation.installation_handle.clone();
    let grant = delegate(&client, &owner, &handle, 1, 1, relay.pubkey())
        .await
        .unwrap();
    assert!(grant.starts_with("pg1_") && !grant.contains(APNS_TOKEN));

    // One accepted wake with the fixed body over HTTP/2.
    let first = wake(&grant);
    assert_eq!(transport.deliver(&first).await, WakeOutcome::Accepted);
    let sent = apns.received();
    assert_eq!(sent.len(), 1);
    let request = &sent[0];
    assert_eq!(request.version, axum::http::Version::HTTP_2);
    assert_eq!(request.path, format!("/3/device/{APNS_TOKEN}"));
    assert_eq!(request.body, nostr::push_lease::APNS_BODY.as_bytes());
    assert_eq!(request.headers["apns-topic"], TOPIC);
    assert_eq!(request.headers["apns-push-type"], "alert");
    assert_eq!(request.headers["apns-priority"], "10");
    assert_eq!(request.headers["apns-id"], first.request_id);
    assert!(request.headers["apns-expiration"].parse::<u64>().unwrap() <= first.expires_at);
    let bearer = request.headers["authorization"]
        .strip_prefix("bearer ")
        .unwrap();
    let (header, claims) = verify_jwt(bearer, &public_key, false);
    assert_eq!(header, json!({"alg": "ES256", "kid": KEY_ID}));
    assert_eq!(claims["iss"], TEAM_ID);

    // The same request ID replays the recorded outcome without a send.
    assert_eq!(transport.deliver(&first).await, WakeOutcome::Accepted);
    assert_eq!(apns.received().len(), 1);

    // A transient failure is released, so the relay's retry reaches APNs.
    apns.script([Scripted::new(503, r#"{"reason":"ServiceUnavailable"}"#)]);
    let transient = wake(&grant);
    assert!(matches!(
        transport.deliver(&transient).await,
        WakeOutcome::Retry {
            reason: "provider_retry",
            ..
        }
    ));
    // The relay retries after its backoff, with a fresh authorization; an
    // identical authorization is a replay.
    assert_eq!(
        transport.deliver(&transient).await,
        WakeOutcome::Rejected("invalid_grant")
    );
    tokio::time::sleep(std::time::Duration::from_millis(1_100)).await;
    assert_eq!(transport.deliver(&transient).await, WakeOutcome::Accepted);
    assert_eq!(apns.received().len(), 3);

    // An expired provider token permits one refresh and one retry.
    apns.script([Scripted::new(403, r#"{"reason":"ExpiredProviderToken"}"#)]);
    assert_eq!(
        transport.deliver(&wake(&grant)).await,
        WakeOutcome::Accepted
    );
    assert_eq!(apns.received().len(), 5);

    // A credential fault is a configuration fault the relay retries.
    apns.script([Scripted::new(403, r#"{"reason":"InvalidProviderToken"}"#)]);
    assert!(matches!(
        transport.deliver(&wake(&grant)).await,
        WakeOutcome::Retry {
            reason: "gateway_configuration_fault",
            ..
        }
    ));

    // A permanent refusal of the request is terminal.
    apns.script([Scripted::new(400, r#"{"reason":"BadTopic"}"#)]);
    assert_eq!(
        transport.deliver(&wake(&grant)).await,
        WakeOutcome::Rejected("invalid_request")
    );

    // An unregistered token becomes an invalid endpoint, and later wakes
    // for it stop at the gateway.
    apns.script([Scripted::new(
        410,
        r#"{"reason":"Unregistered","timestamp":1800000000000}"#,
    )]);
    assert_eq!(
        transport.deliver(&wake(&grant)).await,
        WakeOutcome::InvalidEndpoint
    );
    let before = apns.received().len();
    assert_eq!(
        transport.deliver(&wake(&grant)).await,
        WakeOutcome::InvalidEndpoint
    );
    assert_eq!(apns.received().len(), before);

    // Rotation retires the old capability; a new delegation works again.
    let rotated_token = "ff".repeat(32);
    let epoch = client
        .rotate(&owner, &handle, 1, &rotated_token)
        .await
        .unwrap();
    assert_eq!(epoch, 2);
    assert_eq!(
        transport.deliver(&wake(&grant)).await,
        WakeOutcome::Rejected("invalid_grant")
    );
    assert!(
        delegate(&client, &owner, &handle, 1, 2, relay.pubkey())
            .await
            .is_err()
    );
    let rotated = delegate(&client, &owner, &handle, 2, 2, relay.pubkey())
        .await
        .unwrap();
    assert_eq!(
        transport.deliver(&wake(&rotated)).await,
        WakeOutcome::Accepted
    );
    assert_eq!(
        apns.received().last().unwrap().path,
        format!("/3/device/{rotated_token}")
    );

    // Authority: another owner, a stale generation, an unknown relay, a
    // foreign signer, an expired request, and the wrong listener.
    let stranger = device(0x62);
    assert!(matches!(
        delegate(&client, &stranger, &handle, 2, 3, relay.pubkey()).await,
        Err(ClientError::Refused { status: 404, .. })
    ));
    assert!(matches!(
        client
            .register(&stranger, APNS_PROFILE, &rotated_token, unix_now() + 86_400)
            .await,
        Err(ClientError::Refused { status: 409, .. })
    ));
    assert!(matches!(
        delegate(&client, &owner, &handle, 2, 1, relay.pubkey()).await,
        Err(ClientError::Refused { status: 400, .. })
    ));
    assert!(matches!(
        delegate(&client, &owner, &handle, 2, 3, &"c".repeat(64)).await,
        Err(ClientError::Refused { status: 400, .. })
    ));
    let foreign = RelaySigner::from_secret_hex(&"6b".repeat(32)).unwrap();
    let foreign = ApnsGateway::new(&format!("http://{}", running.delivery_addr), foreign).unwrap();
    assert_eq!(
        foreign.deliver(&wake(&rotated)).await,
        WakeOutcome::Rejected("invalid_auth")
    );
    let mut expired = wake(&rotated);
    expired.expires_at = unix_now() - 10;
    assert_eq!(
        transport.deliver(&expired).await,
        WakeOutcome::Rejected("invalid_grant")
    );
    let misrouted = ApnsGateway::new(
        &format!("http://{}", running.registration_addr),
        relay.clone(),
    )
    .unwrap();
    assert!(matches!(
        misrouted.deliver(&wake(&rotated)).await,
        WakeOutcome::Rejected(_)
    ));
    let fcm_route =
        FcmGateway::new(&format!("http://{}", running.delivery_addr), relay.clone()).unwrap();
    assert_eq!(
        fcm_route.deliver(&wake(&rotated)).await,
        WakeOutcome::Rejected("invalid_grant")
    );

    // Revocation forgets the token and every capability.
    client
        .revoke_installation(
            &owner,
            &InstallationRevokeRequest {
                v: 1,
                installation_handle: handle.clone(),
                endpoint_epoch: 2,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        transport.deliver(&wake(&rotated)).await,
        WakeOutcome::Rejected("invalid_grant")
    );

    // No token appears in the state file.
    running.stop().await;
    let state = std::fs::read_to_string(dir.path().join("state/state.json")).unwrap();
    assert!(!state.contains(APNS_TOKEN) && !state.contains(&rotated_token));
    assert!(!state.contains(&rotated));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fcm_delivery_uses_service_account_oauth_and_the_data_constant() {
    let dir = tempfile::tempdir().unwrap();
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
    let (pem, public_key) = rsa_key(dir.path());
    let account = json!({
        "type": "service_account",
        "project_id": "demo-project",
        "private_key_id": "key-1",
        "private_key": pem,
        "client_email": "sender@demo-project.iam.gserviceaccount.com",
        "token_uri": format!("{}/token", oauth.base),
    })
    .to_string();
    let relay = relay();
    let running = push_gateway::server::start(common::config(
        dir.path(),
        relay.pubkey(),
        None,
        Some((&fcm.base, &account)),
    ))
    .await
    .unwrap();
    let client = GatewayClient::new(&format!("http://{}", running.registration_addr)).unwrap();
    let transport =
        FcmGateway::new(&format!("http://{}", running.delivery_addr), relay.clone()).unwrap();
    let owner = device(0x71);
    let token = "fcm-registration-token_1:APA91b";
    let installation = client
        .register(&owner, FCM_PROFILE, token, unix_now() + 86_400)
        .await
        .unwrap();
    let grant = delegate(
        &client,
        &owner,
        &installation.installation_handle,
        1,
        1,
        relay.pubkey(),
    )
    .await
    .unwrap();
    // An APNs-profile route cannot reach an FCM installation.
    assert!(
        client
            .register(&owner, APNS_PROFILE, "abcd", unix_now() + 60)
            .await
            .is_err()
    );

    assert_eq!(
        transport.deliver(&wake(&grant)).await,
        WakeOutcome::Accepted
    );
    let exchange = &oauth.received()[0];
    assert_eq!(exchange.path, "/token");
    let form = String::from_utf8(exchange.body.clone()).unwrap();
    let assertion = form
        .split('&')
        .find_map(|pair| pair.strip_prefix("assertion="))
        .unwrap();
    assert!(form.contains("grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Ajwt-bearer"));
    let (header, claims) = verify_jwt(assertion, &public_key, true);
    assert_eq!(header["alg"], "RS256");
    assert_eq!(header["kid"], "key-1");
    assert_eq!(claims["scope"], push_gateway::server::fcm::SCOPE);
    assert_eq!(claims["aud"], format!("{}/token", oauth.base));
    let send = &fcm.received()[0];
    assert_eq!(send.path, "/v1/projects/demo-project/messages:send");
    assert_eq!(send.headers["authorization"], "Bearer access-token-1");
    let body = send.json();
    assert_eq!(body["message"]["token"], token);
    assert_eq!(
        body["message"]["data"].to_string(),
        nostr::push_lease::FCM_DATA
    );
    assert!(body["message"].get("notification").is_none());

    // A rejected access token is refreshed once.
    fcm.script([Scripted::new(
        401,
        r#"{"error":{"code":401,"status":"UNAUTHENTICATED"}}"#,
    )]);
    assert_eq!(
        transport.deliver(&wake(&grant)).await,
        WakeOutcome::Accepted
    );
    assert_eq!(oauth.received().len(), 2);

    // Quota exhaustion carries the provider's delay.
    fcm.script([Scripted {
        status: 429,
        headers: vec![("retry-after", "30".into())],
        body: r#"{"error":{"code":429,"status":"RESOURCE_EXHAUSTED","details":[{"errorCode":"QUOTA_EXCEEDED"}]}}"#.into(),
    }]);
    assert_eq!(
        transport.deliver(&wake(&grant)).await,
        WakeOutcome::Retry {
            after_seconds: Some(30),
            reason: "provider_retry"
        }
    );

    // An unregistered token is an invalid endpoint.
    fcm.script([Scripted::new(
        404,
        r#"{"error":{"code":404,"status":"NOT_FOUND","details":[{"@type":"type.googleapis.com/google.firebase.fcm.v1.FcmError","errorCode":"UNREGISTERED"}]}}"#,
    )]);
    assert_eq!(
        transport.deliver(&wake(&grant)).await,
        WakeOutcome::InvalidEndpoint
    );
    running.stop().await;
}
