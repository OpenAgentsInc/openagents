//! Live M3 gateway conformance. The suite is destructive and must run only
//! through `scripts/test-postgres.sh` against its disposable database.

use std::{
    io::{ErrorKind, Read, Write},
    net::{SocketAddr, TcpStream as StdTcpStream},
    path::{Path, PathBuf},
    time::Duration,
};

use nostr_relay::{
    domain::{Event, RelaySigner, Tag},
    gateway::{Gateway, GatewayConfig, MediaConfig},
};
use secp256k1::{Keypair, Secp256k1, SecretKey};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};
use tokio_postgres::NoTls;
use tokio_tungstenite::tungstenite::{Message, WebSocket, client};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn m3_gateway_contract_against_postgres() {
    let Ok(database_url) = std::env::var("NOSTR_RELAY_TEST_DATABASE_URL") else {
        eprintln!("skipped: run scripts/test-postgres.sh");
        return;
    };
    if std::env::var("NOSTR_RELAY_TEST_ALLOW_DESTRUCTIVE").as_deref() != Ok("1") {
        eprintln!("skipped: live gateway suite requires a disposable database guard");
        return;
    }

    let media_root = temporary_media_root();
    std::fs::create_dir_all(&media_root).unwrap();
    let gateway_one = Gateway::start(test_config(database_url.clone(), media_root.clone()))
        .await
        .unwrap();
    let address_one = gateway_one.local_addr();
    let stop_one = gateway_one.shutdown_handle();
    let server_one = tokio::spawn(gateway_one.run());

    let verification_database_url = database_url.clone();
    let gateway_two = Gateway::start(test_config(database_url, media_root.clone()))
        .await
        .unwrap();
    let address_two = gateway_two.local_addr();
    let stop_two = gateway_two.shutdown_handle();
    let server_two = tokio::spawn(gateway_two.run());

    assert_nip11_http(address_one).await;
    let media_root_for_contract = media_root.clone();
    let expired_id = tokio::task::spawn_blocking(move || {
        websocket_contract(address_one, address_two);
        management_contract(address_one);
        media_contract(address_one, &media_root_for_contract);
        expanded_protocol_contract(address_one, address_two)
    })
    .await
    .unwrap();
    assert_physically_expired(&verification_database_url, &expired_id).await;
    let media_root_for_release = media_root.clone();
    tokio::task::spawn_blocking(move || {
        disconnected_upload_releases_reservation(address_one, &media_root_for_release)
    })
    .await
    .unwrap();
    stale_reservation_sweep_contract(&verification_database_url, address_one, &media_root).await;
    configure_closed_membership(&verification_database_url).await;
    tokio::task::spawn_blocking(move || closed_agent_auth_contract(address_one))
        .await
        .unwrap();
    tokio::task::spawn_blocking(move || malformed_legacy_wrap_contract(address_two))
        .await
        .unwrap();

    stop_one.shutdown();
    stop_two.shutdown();
    timeout(Duration::from_secs(5), server_one)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    timeout(Duration::from_secs(5), server_two)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    std::fs::remove_dir_all(media_root).unwrap();
}

async fn configure_closed_membership(database_url: &str) {
    let (client, connection) = tokio_postgres::connect(database_url, NoTls).await.unwrap();
    tokio::spawn(async move { connection.await.unwrap() });
    let close = client
        .prepare("UPDATE relay_policy SET closed_membership = TRUE WHERE singleton = TRUE")
        .await
        .unwrap();
    client.execute(&close, &[]).await.unwrap();
    let member = client
        .prepare(
            "INSERT INTO relay_member_pubkey (pubkey, note) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        )
        .await
        .unwrap();
    for secret in [21_u8, 24] {
        client
            .execute(&member, &[&pubkey(secret), &"NIP-AA live owner"])
            .await
            .unwrap();
    }
}

fn closed_agent_auth_contract(address: SocketAddr) {
    let mut agent = connect_client(address);
    let challenge = expect_auth_challenge(&mut agent);
    let accepted_auth = agent_auth_event(22, 21, &challenge);
    send_json(&mut agent, json!(["AUTH", accepted_auth]));
    assert_eq!(read_json(&mut agent)[2], true);
    let publication = signed_event(22, now(), 1, Vec::new(), "virtual member publication");
    send_json(&mut agent, json!(["EVENT", publication]));
    assert_eq!(read_json(&mut agent)[2], true);
    let cross_identity = signed_event(24, now(), 1, Vec::new(), "different direct member");
    send_json(&mut agent, json!(["EVENT", cross_identity]));
    let refusal = read_json(&mut agent);
    assert_eq!(refusal[2], false);
    assert!(refusal[3].as_str().unwrap().starts_with("auth-required:"));
    agent.close(None).unwrap();

    let mut uncredentialed = connect_client(address);
    let challenge = expect_auth_challenge(&mut uncredentialed);
    let ordinary = signed_event(
        23,
        now(),
        22_242,
        vec![
            Tag::new(vec!["relay".into(), "ws://relay.test".into()]),
            Tag::new(vec!["challenge".into(), challenge]),
        ],
        "",
    );
    send_json(&mut uncredentialed, json!(["AUTH", ordinary]));
    assert_eq!(read_json(&mut uncredentialed)[2], false);
    uncredentialed.close(None).unwrap();

    let mut conflict = connect_client(address);
    let challenge = expect_auth_challenge(&mut conflict);
    let conflicting_auth = agent_auth_event(22, 24, &challenge);
    send_json(&mut conflict, json!(["AUTH", conflicting_auth]));
    assert_eq!(read_json(&mut conflict)[2], false);
    conflict.close(None).unwrap();

    // A virtual identity never inherits relay-owner command authority, even
    // when its pubkey is the configured management pubkey.
    let mut virtual_manager = connect_client(address);
    let challenge = expect_auth_challenge(&mut virtual_manager);
    let manager_auth = agent_auth_event(91, 21, &challenge);
    send_json(&mut virtual_manager, json!(["AUTH", manager_auth]));
    assert_eq!(read_json(&mut virtual_manager)[2], true);
    let command = signed_event(
        91,
        now(),
        9_033,
        vec![Tag::new(vec![
            "icon".into(),
            "https://example.com/forbidden.png".into(),
        ])],
        "",
    );
    send_json(&mut virtual_manager, json!(["EVENT", command]));
    assert_eq!(read_json(&mut virtual_manager)[2], false);
    virtual_manager.close(None).unwrap();
}

fn agent_auth_event(agent_secret: u8, owner_secret: u8, challenge: &str) -> Event {
    let agent = pubkey(agent_secret);
    let secp = Secp256k1::new();
    let owner_key = Keypair::from_secret_key(
        &secp,
        &SecretKey::from_byte_array([owner_secret; 32]).unwrap(),
    );
    let digest: [u8; 32] = Sha256::digest(format!("nostr:agent-auth:{agent}:").as_bytes()).into();
    let signature = secp.sign_schnorr_no_aux_rand(&digest, &owner_key);
    signed_event(
        agent_secret,
        now(),
        22_242,
        vec![
            Tag::new(vec!["relay".into(), "ws://relay.test".into()]),
            Tag::new(vec!["challenge".into(), challenge.to_owned()]),
            Tag::new(vec![
                "auth".into(),
                pubkey(owner_secret),
                "".into(),
                signature.to_string(),
            ]),
        ],
        "",
    )
}

fn owner_binding_tag(agent_secret: u8, owner_secret: u8) -> Tag {
    let agent = pubkey(agent_secret);
    let secp = Secp256k1::new();
    let owner_key = Keypair::from_secret_key(
        &secp,
        &SecretKey::from_byte_array([owner_secret; 32]).unwrap(),
    );
    let digest: [u8; 32] = Sha256::digest(format!("nostr:agent-auth:{agent}:").as_bytes()).into();
    Tag::new(vec![
        "auth".into(),
        pubkey(owner_secret),
        "".into(),
        secp.sign_schnorr_no_aux_rand(&digest, &owner_key)
            .to_string(),
    ])
}

fn test_config(database_url: String, media_root: PathBuf) -> GatewayConfig {
    let mut config = GatewayConfig::new(database_url, "127.0.0.1:0".parse().unwrap());
    config.relay_url = Some("ws://relay.test".to_owned());
    config.auth_required = true;
    config.db_connections = 2;
    config.shutdown_grace = Duration::from_secs(2);
    config.expiration_sweep = Duration::from_secs(1);
    config.relay_signer = Some(RelaySigner::from_secret_hex(&hex(&[90; 32])).unwrap());
    config.identity.pubkey = config
        .relay_signer
        .as_ref()
        .map(|signer| signer.pubkey().to_owned());
    config.management_pubkey = Some(pubkey(91));
    config.openagents_profiles = true;
    config.media = Some(MediaConfig {
        root: media_root,
        cloud_base_url: None,
        max_blob_bytes: 1_024,
        max_bytes_per_pubkey: 1_024,
    });
    config.limits.max_frame_bytes = 131_072;
    config.limits.max_subscriptions = 8;
    config.limits.max_filters = 4;
    config.limits.max_limit = 10;
    config.limits.max_query_cost = 10_000;
    config.limits.events_per_minute_ip = 1_000;
    config.limits.events_per_minute_pubkey = 1_000;
    config.limits.gift_wraps_per_minute_recipient = 1;
    config.limits.req_per_minute_ip = 100;
    config.limits.media_per_minute_ip = 100;
    config.limits.media_per_minute_pubkey = 100;
    config.limits.max_connections_per_ip = 10;
    config.limits.send_queue_capacity = 64;
    config
}

async fn assert_nip11_http(address: SocketAddr) {
    let mut stream = TcpStream::connect(address).await.unwrap();
    stream
        .write_all(
            b"GET /nested/path HTTP/1.1\r\nHost: relay.test\r\nAccept: application/nostr+json\r\n\r\n",
        )
        .await
        .unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).await.unwrap();
    let response = String::from_utf8(response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(response.contains("Access-Control-Allow-Origin: *\r\n"));
    let body = response.split("\r\n\r\n").nth(1).unwrap();
    let document: Value = serde_json::from_str(body).unwrap();
    assert_eq!(document["name"], "nostr-relay");
    assert_eq!(document["limitation"]["auth_required"], true);
    assert_eq!(document["nip29"]["subgroups"], true);
    assert!(
        document["supported_nips"]
            .as_array()
            .unwrap()
            .contains(&json!(42))
    );
    for nip in [17, 29, 45, 50, 59, 65, 70, 86, 94, 98] {
        assert!(
            document["supported_nips"]
                .as_array()
                .unwrap()
                .contains(&json!(nip)),
            "missing NIP-{nip}"
        );
    }
    for extension in [
        "nip-aa",
        "nip-ae",
        "nip-am",
        "nip-ao",
        "nip-ap",
        "nip-cap-v1",
        "nip-cw",
        "nip-dv",
        "nip-er",
        "nip-ext-v1",
        "nip-ia",
        "nip-mp",
        "nip-oa",
        "nip-prg-v1",
        "nip-rs",
        "nip-run-v1",
        "nip-wp",
    ] {
        assert!(
            document["supported_extensions"]
                .as_array()
                .unwrap()
                .contains(&json!(extension)),
            "missing {extension}"
        );
    }
    let extensions = document["supported_extensions"].as_array().unwrap();
    assert!(
        extensions
            .windows(2)
            .all(|pair| pair[0].as_str() < pair[1].as_str())
    );
    assert!(
        !document["supported_nips"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| matches!(value.as_u64(), Some(396 | 39_600..=39_699)))
    );
}

fn media_contract(address: SocketAddr, media_root: &Path) {
    let payload = b"fixture blossom payload";
    let sha256 = hex(&Sha256::digest(payload));
    let upload_auth = signed_event(
        75,
        now(),
        27_235,
        vec![
            Tag::new(vec!["u".into(), "http://relay.test/upload".into()]),
            Tag::new(vec!["method".into(), "PUT".into()]),
            Tag::new(vec!["payload".into(), sha256.clone()]),
        ],
        "",
    );
    let upload_authorization = base64(&serde_json::to_vec(&upload_auth).unwrap());
    let upload = raw_http(
        address,
        &format!(
            "PUT /upload HTTP/1.1\r\nHost: relay.test\r\nContent-Type: text/plain\r\nX-SHA-256: {sha256}\r\nAuthorization: Nostr {upload_authorization}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            payload.len()
        ),
        payload,
    );
    assert!(upload.starts_with(b"HTTP/1.1 201 Created\r\n"));
    let descriptor: Value = serde_json::from_slice(http_body(&upload)).unwrap();
    assert_eq!(descriptor["sha256"], sha256);
    assert_eq!(descriptor["size"], payload.len());
    assert_eq!(descriptor["type"], "text/plain");
    assert_eq!(descriptor["nip94"][2], json!(["x", sha256]));

    let replay = raw_http(
        address,
        &format!(
            "PUT /upload HTTP/1.1\r\nHost: relay.test\r\nContent-Type: text/plain\r\nAuthorization: Nostr {upload_authorization}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            payload.len()
        ),
        payload,
    );
    assert!(replay.starts_with(b"HTTP/1.1 409 Conflict\r\n"));

    let shared_auth = signed_event(
        76,
        now(),
        27_235,
        vec![
            Tag::new(vec!["u".into(), "http://relay.test/upload".into()]),
            Tag::new(vec!["method".into(), "PUT".into()]),
            Tag::new(vec!["payload".into(), sha256.clone()]),
        ],
        "shared owner",
    );
    let shared_authorization = base64(&serde_json::to_vec(&shared_auth).unwrap());
    let shared = raw_http(
        address,
        &format!(
            "PUT /upload HTTP/1.1\r\nHost: relay.test\r\nContent-Type: text/plain\r\nAuthorization: Nostr {shared_authorization}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            payload.len()
        ),
        payload,
    );
    assert!(shared.starts_with(b"HTTP/1.1 200 OK\r\n"));

    let over_quota_payload = vec![b'q'; 1_010];
    let over_quota_hash = hex(&Sha256::digest(&over_quota_payload));
    let over_quota_auth = signed_event(
        75,
        now(),
        27_235,
        vec![
            Tag::new(vec!["u".into(), "http://relay.test/upload".into()]),
            Tag::new(vec!["method".into(), "PUT".into()]),
            Tag::new(vec!["payload".into(), over_quota_hash.clone()]),
        ],
        "",
    );
    let over_quota_authorization = base64(&serde_json::to_vec(&over_quota_auth).unwrap());
    let over_quota = raw_http(
        address,
        &format!(
            "PUT /upload HTTP/1.1\r\nHost: relay.test\r\nContent-Type: application/octet-stream\r\nAuthorization: Nostr {over_quota_authorization}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            over_quota_payload.len()
        ),
        &over_quota_payload,
    );
    assert!(over_quota.starts_with(b"HTTP/1.1 413 Payload Too Large\r\n"));
    let quota_missing = raw_http(
        address,
        &format!(
            "GET /{over_quota_hash} HTTP/1.1\r\nHost: relay.test\r\nConnection: close\r\n\r\n"
        ),
        &[],
    );
    assert!(quota_missing.starts_with(b"HTTP/1.1 404 Not Found\r\n"));

    // A request with no authorization is refused before its body is read:
    // no temporary file appears and the body's bytes are never stored.
    let stray_payload = b"nobody signed for this";
    let stray_hash = hex(&Sha256::digest(stray_payload));
    let temporary_files = || {
        std::fs::read_dir(media_root.join(".tmp"))
            .map(|entries| entries.count())
            .unwrap_or(0)
    };
    let temporary_before = temporary_files();
    let unsigned = raw_http(
        address,
        &format!(
            "PUT /upload HTTP/1.1\r\nHost: relay.test\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            stray_payload.len()
        ),
        stray_payload,
    );
    assert!(unsigned.starts_with(b"HTTP/1.1 401 Unauthorized\r\n"));
    assert_eq!(temporary_files(), temporary_before);
    let stray_missing = raw_http(
        address,
        &format!("GET /{stray_hash} HTTP/1.1\r\nHost: relay.test\r\nConnection: close\r\n\r\n"),
        &[],
    );
    assert!(stray_missing.starts_with(b"HTTP/1.1 404 Not Found\r\n"));

    // An authorized upload whose body is not what it claimed is refused,
    // and the quota it reserved is given back: a second upload of the same
    // size by the same pubkey fits where two reservations would not.
    let large_payload = vec![b'x'; 900];
    let large_hash = hex(&Sha256::digest(&large_payload));
    let wrong_body = vec![b'y'; 900];
    let mismatch_auth = signed_event(
        75,
        now(),
        27_235,
        vec![
            Tag::new(vec!["u".into(), "http://relay.test/upload".into()]),
            Tag::new(vec!["method".into(), "PUT".into()]),
            Tag::new(vec!["payload".into(), large_hash.clone()]),
        ],
        "mismatch",
    );
    let mismatch_authorization = base64(&serde_json::to_vec(&mismatch_auth).unwrap());
    let mismatch = raw_http(
        address,
        &format!(
            "PUT /upload HTTP/1.1\r\nHost: relay.test\r\nContent-Type: application/octet-stream\r\nAuthorization: Nostr {mismatch_authorization}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            wrong_body.len()
        ),
        &wrong_body,
    );
    assert!(mismatch.starts_with(b"HTTP/1.1 400 Bad Request\r\n"));
    assert_eq!(temporary_files(), temporary_before);
    let mismatch_missing = raw_http(
        address,
        &format!("GET /{large_hash} HTTP/1.1\r\nHost: relay.test\r\nConnection: close\r\n\r\n"),
        &[],
    );
    assert!(mismatch_missing.starts_with(b"HTTP/1.1 404 Not Found\r\n"));
    let other_large = vec![b'z'; 900];
    let other_large_hash = hex(&Sha256::digest(&other_large));
    let released_auth = signed_event(
        75,
        now(),
        27_235,
        vec![
            Tag::new(vec!["u".into(), "http://relay.test/upload".into()]),
            Tag::new(vec!["method".into(), "PUT".into()]),
            Tag::new(vec!["payload".into(), other_large_hash.clone()]),
        ],
        "released",
    );
    let released_authorization = base64(&serde_json::to_vec(&released_auth).unwrap());
    let released = raw_http(
        address,
        &format!(
            "PUT /upload HTTP/1.1\r\nHost: relay.test\r\nContent-Type: application/octet-stream\r\nAuthorization: Nostr {released_authorization}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            other_large.len()
        ),
        &other_large,
    );
    assert!(
        released.starts_with(b"HTTP/1.1 201 Created\r\n"),
        "{}",
        String::from_utf8_lossy(&released)
    );
    let other_large_path = format!("/{other_large_hash}");
    let other_large_delete_auth = signed_event(
        75,
        now(),
        27_235,
        vec![
            Tag::new(vec![
                "u".into(),
                format!("http://relay.test{other_large_path}"),
            ]),
            Tag::new(vec!["method".into(), "DELETE".into()]),
        ],
        "released",
    );
    let other_large_delete_authorization =
        base64(&serde_json::to_vec(&other_large_delete_auth).unwrap());
    let other_large_delete = raw_http(
        address,
        &format!(
            "DELETE {other_large_path} HTTP/1.1\r\nHost: relay.test\r\nAuthorization: Nostr {other_large_delete_authorization}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        ),
        &[],
    );
    assert!(other_large_delete.starts_with(b"HTTP/1.1 200 OK\r\n"));

    let head = raw_http(
        address,
        &format!("HEAD /{sha256}.txt HTTP/1.1\r\nHost: relay.test\r\nConnection: close\r\n\r\n"),
        &[],
    );
    assert!(head.starts_with(b"HTTP/1.1 200 OK\r\n"));
    assert!(
        head.windows(22)
            .any(|line| line == b"Accept-Ranges: bytes\r\n")
    );
    assert!(http_body(&head).is_empty());

    let ranged = raw_http(
        address,
        &format!(
            "GET /{sha256} HTTP/1.1\r\nHost: relay.test\r\nRange: bytes=2-6\r\nConnection: close\r\n\r\n"
        ),
        &[],
    );
    assert!(ranged.starts_with(b"HTTP/1.1 206 Partial Content\r\n"));
    assert_eq!(http_body(&ranged), &payload[2..=6]);

    let delete_path = format!("/{sha256}.txt");
    let delete_auth = signed_event(
        75,
        now(),
        27_235,
        vec![
            Tag::new(vec!["u".into(), format!("http://relay.test{delete_path}")]),
            Tag::new(vec!["method".into(), "DELETE".into()]),
        ],
        "",
    );
    let delete_authorization = base64(&serde_json::to_vec(&delete_auth).unwrap());
    let delete = raw_http(
        address,
        &format!(
            "DELETE {delete_path} HTTP/1.1\r\nHost: relay.test\r\nAuthorization: Nostr {delete_authorization}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        ),
        &[],
    );
    assert!(delete.starts_with(b"HTTP/1.1 200 OK\r\n"));
    assert_eq!(
        serde_json::from_slice::<Value>(http_body(&delete)).unwrap()["message"],
        "ownership removed"
    );
    let shared_still_present = raw_http(
        address,
        &format!("GET /{sha256} HTTP/1.1\r\nHost: relay.test\r\nConnection: close\r\n\r\n"),
        &[],
    );
    assert!(shared_still_present.starts_with(b"HTTP/1.1 200 OK\r\n"));

    let shared_delete_auth = signed_event(
        76,
        now(),
        27_235,
        vec![
            Tag::new(vec!["u".into(), format!("http://relay.test{delete_path}")]),
            Tag::new(vec!["method".into(), "DELETE".into()]),
        ],
        "shared owner",
    );
    let shared_delete_authorization = base64(&serde_json::to_vec(&shared_delete_auth).unwrap());
    let shared_delete = raw_http(
        address,
        &format!(
            "DELETE {delete_path} HTTP/1.1\r\nHost: relay.test\r\nAuthorization: Nostr {shared_delete_authorization}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        ),
        &[],
    );
    assert!(shared_delete.starts_with(b"HTTP/1.1 200 OK\r\n"));
    assert_eq!(
        serde_json::from_slice::<Value>(http_body(&shared_delete)).unwrap()["message"],
        "blob deleted"
    );
    let missing = raw_http(
        address,
        &format!("GET /{sha256} HTTP/1.1\r\nHost: relay.test\r\nConnection: close\r\n\r\n"),
        &[],
    );
    assert!(missing.starts_with(b"HTTP/1.1 404 Not Found\r\n"));
    let missing_head = raw_http(
        address,
        &format!("HEAD /{sha256} HTTP/1.1\r\nHost: relay.test\r\nConnection: close\r\n\r\n"),
        &[],
    );
    assert!(missing_head.starts_with(b"HTTP/1.1 404 Not Found\r\n"));
    assert!(http_body(&missing_head).is_empty());

    let reupload_auth = signed_event(
        75,
        now(),
        27_235,
        vec![
            Tag::new(vec!["u".into(), "http://relay.test/upload".into()]),
            Tag::new(vec!["method".into(), "PUT".into()]),
            Tag::new(vec!["payload".into(), sha256.clone()]),
        ],
        "generation two",
    );
    let reupload_authorization = base64(&serde_json::to_vec(&reupload_auth).unwrap());
    let reupload = raw_http(
        address,
        &format!(
            "PUT /upload HTTP/1.1\r\nHost: relay.test\r\nContent-Type: text/plain\r\nAuthorization: Nostr {reupload_authorization}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            payload.len()
        ),
        payload,
    );
    assert!(reupload.starts_with(b"HTTP/1.1 201 Created\r\n"));
    let reloaded = raw_http(
        address,
        &format!("GET /{sha256} HTTP/1.1\r\nHost: relay.test\r\nConnection: close\r\n\r\n"),
        &[],
    );
    assert!(reloaded.starts_with(b"HTTP/1.1 200 OK\r\n"));
    assert_eq!(http_body(&reloaded), payload);

    let final_delete_auth = signed_event(
        75,
        now(),
        27_235,
        vec![
            Tag::new(vec!["u".into(), format!("http://relay.test{delete_path}")]),
            Tag::new(vec!["method".into(), "DELETE".into()]),
        ],
        "generation two",
    );
    let final_delete_authorization = base64(&serde_json::to_vec(&final_delete_auth).unwrap());
    let final_delete = raw_http(
        address,
        &format!(
            "DELETE {delete_path} HTTP/1.1\r\nHost: relay.test\r\nAuthorization: Nostr {final_delete_authorization}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        ),
        &[],
    );
    assert!(final_delete.starts_with(b"HTTP/1.1 200 OK\r\n"));
}

fn upload_authorization(secret: u8, sha256: &str, content: &str) -> String {
    let auth = signed_event(
        secret,
        now(),
        27_235,
        vec![
            Tag::new(vec!["u".into(), "http://relay.test/upload".into()]),
            Tag::new(vec!["method".into(), "PUT".into()]),
            Tag::new(vec!["payload".into(), sha256.to_owned()]),
        ],
        content,
    );
    base64(&serde_json::to_vec(&auth).unwrap())
}

fn upload_head(authorization: &str, length: usize) -> String {
    format!(
        "PUT /upload HTTP/1.1\r\nHost: relay.test\r\nContent-Type: application/octet-stream\r\nAuthorization: Nostr {authorization}\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n"
    )
}

fn temporary_file_count(media_root: &Path) -> usize {
    std::fs::read_dir(media_root.join(".tmp"))
        .map(|entries| entries.count())
        .unwrap_or(0)
}

/// A client that hangs up halfway through an authorized body leaves no
/// temporary file and holds no quota: the same pubkey then fits a body of
/// the same size where two reservations would not.
fn disconnected_upload_releases_reservation(address: SocketAddr, media_root: &Path) {
    let temporary_before = temporary_file_count(media_root);
    let abandoned = vec![b'a'; 900];
    let abandoned_hash = hex(&Sha256::digest(&abandoned));
    let mut stream = StdTcpStream::connect(address).unwrap();
    stream
        .write_all(
            upload_head(&upload_authorization(77, &abandoned_hash, "abandoned"), 900).as_bytes(),
        )
        .unwrap();
    stream.write_all(&abandoned[..400]).unwrap();
    // Give the relay time to register and start streaming before the hangup.
    std::thread::sleep(Duration::from_millis(300));
    drop(stream);

    let replacement = vec![b'b'; 900];
    let replacement_hash = hex(&Sha256::digest(&replacement));
    let mut response = Vec::new();
    for _ in 0..50 {
        response = raw_http(
            address,
            &upload_head(
                &upload_authorization(77, &replacement_hash, "replacement"),
                900,
            ),
            &replacement,
        );
        if response.starts_with(b"HTTP/1.1 201 Created\r\n") {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(
        response.starts_with(b"HTTP/1.1 201 Created\r\n"),
        "{}",
        String::from_utf8_lossy(&response)
    );
    assert_eq!(temporary_file_count(media_root), temporary_before);
    let abandoned_missing = raw_http(
        address,
        &format!("GET /{abandoned_hash} HTTP/1.1\r\nHost: relay.test\r\nConnection: close\r\n\r\n"),
        &[],
    );
    assert!(abandoned_missing.starts_with(b"HTTP/1.1 404 Not Found\r\n"));
}

/// A registration a cancelled task never released, here planted directly
/// as an unpublished row older than the stale age, is swept with its
/// ownership and any bytes it left under the blob path, so the pubkey's
/// quota comes back without a restart.
async fn stale_reservation_sweep_contract(
    database_url: &str,
    address: SocketAddr,
    media_root: &Path,
) {
    let orphan = vec![b'o'; 1_000];
    let orphan_hash = hex(&Sha256::digest(&orphan));
    let orphan_dir = media_root.join(&orphan_hash[..2]);
    std::fs::create_dir_all(&orphan_dir).unwrap();
    let orphan_storage_key = "c".repeat(64);
    let orphan_path = orphan_dir.join(format!("{orphan_hash}.{orphan_storage_key}"));
    std::fs::write(&orphan_path, &orphan).unwrap();
    let (client, connection) = tokio_postgres::connect(database_url, NoTls).await.unwrap();
    let driver = tokio::spawn(connection);
    let stale_uploaded_at = i64::try_from(now()).unwrap() - 7_200;
    client
        .execute(
            "INSERT INTO media_blob (sha256, storage_key, size, media_type, uploaded_at, ready) \
             VALUES ($1, $2, 1000, 'application/octet-stream', $3, FALSE)",
            &[&orphan_hash, &orphan_storage_key, &stale_uploaded_at],
        )
        .await
        .unwrap();
    client
        .execute(
            "INSERT INTO media_owner (sha256, pubkey) VALUES ($1, $2)",
            &[&orphan_hash, &pubkey(78)],
        )
        .await
        .unwrap();
    let fresh = vec![b'f'; 1_000];
    let fresh_hash = hex(&Sha256::digest(&fresh));
    let mut response = Vec::new();
    for _ in 0..50 {
        let head = upload_head(&upload_authorization(78, &fresh_hash, "after sweep"), 1_000);
        let body = fresh.clone();
        response = tokio::task::spawn_blocking(move || raw_http(address, &head, &body))
            .await
            .unwrap();
        if response.starts_with(b"HTTP/1.1 201 Created\r\n") {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(
        response.starts_with(b"HTTP/1.1 201 Created\r\n"),
        "{}",
        String::from_utf8_lossy(&response)
    );
    let orphan_rows = client
        .query_one(
            "SELECT count(*) FROM media_blob WHERE sha256 = $1",
            &[&orphan_hash],
        )
        .await
        .unwrap()
        .get::<_, i64>(0);
    assert_eq!(orphan_rows, 0, "stale reservation row must be swept");
    let orphan_owners = client
        .query_one(
            "SELECT count(*) FROM media_owner WHERE sha256 = $1",
            &[&orphan_hash],
        )
        .await
        .unwrap()
        .get::<_, i64>(0);
    assert_eq!(
        orphan_owners, 0,
        "stale reservation ownership must be swept"
    );
    assert!(
        !orphan_path.exists(),
        "stale reservation bytes must be removed"
    );
    drop(client);
    driver.await.unwrap().unwrap();
}

fn raw_http(address: SocketAddr, head: &str, body: &[u8]) -> Vec<u8> {
    let mut stream = StdTcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream.write_all(head.as_bytes()).unwrap();
    stream.write_all(body).unwrap();
    let method = head.split_whitespace().next().unwrap_or_default();
    let mut response = Vec::new();
    let mut buffer = [0_u8; 8_192];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => response.extend_from_slice(&buffer[..read]),
            // The relay answers a refused upload and closes without
            // draining the body it never read, and macOS reports that
            // close as ECONNRESET rather than EOF. Tolerate the reset
            // only when the whole response has already arrived.
            Err(error)
                if error.kind() == ErrorKind::ConnectionReset
                    && http_response_complete(method, &response) =>
            {
                break;
            }
            Err(error) => panic!(
                "raw HTTP read failed ({error}); received: {}",
                String::from_utf8_lossy(&response)
            ),
        }
    }
    response
}

/// Whether `bytes` holds a whole `Connection: close` response: a
/// terminated header block with an HTTP status line and, except for
/// `HEAD`, every body byte its `Content-Length` declares.
fn http_response_complete(method: &str, bytes: &[u8]) -> bool {
    let Some(boundary) = bytes.windows(4).position(|window| window == b"\r\n\r\n") else {
        return false;
    };
    let Ok(head) = std::str::from_utf8(&bytes[..boundary]) else {
        return false;
    };
    let mut lines = head.split("\r\n");
    let mut status = lines.next().unwrap_or_default().split_whitespace();
    if !matches!(status.next(), Some("HTTP/1.0" | "HTTP/1.1"))
        || !status.next().is_some_and(|code| {
            code.len() == 3
                && code
                    .parse::<u16>()
                    .is_ok_and(|code| (100..600).contains(&code))
        })
    {
        return false;
    }
    let mut declared = None;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            return false;
        };
        if name.is_empty() || name.eq_ignore_ascii_case("transfer-encoding") {
            return false;
        }
        if name.eq_ignore_ascii_case("content-length") {
            let Ok(length) = value.trim().parse::<usize>() else {
                return false;
            };
            if declared.replace(length).is_some() {
                return false;
            }
        }
    }
    let received = bytes.len() - boundary - 4;
    if method == "HEAD" {
        return received == 0;
    }
    declared == Some(received)
}

fn http_body(response: &[u8]) -> &[u8] {
    let boundary = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .unwrap();
    &response[boundary + 4..]
}

#[test]
fn http_response_complete_accepts_full_responses() {
    assert!(http_response_complete(
        "PUT",
        b"HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nContent-Length: 4\r\nConnection: close\r\n\r\nbody"
    ));
    // Header names are case-insensitive, and a zero-length body completes
    // at the header boundary.
    assert!(http_response_complete(
        "DELETE",
        b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n"
    ));
    // A HEAD answer declares a length it never sends, so its headers alone
    // complete it.
    assert!(http_response_complete(
        "HEAD",
        b"HTTP/1.1 200 OK\r\nContent-Length: 40\r\n\r\n"
    ));
}

#[test]
fn http_response_complete_rejects_truncated_and_malformed() {
    // A body short of the declared Content-Length.
    assert!(!http_response_complete(
        "PUT",
        b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 4\r\n\r\nbod"
    ));
    // A header block that never terminates.
    assert!(!http_response_complete(
        "PUT",
        b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 4\r\n"
    ));
    // No Content-Length to complete against.
    assert!(!http_response_complete("PUT", b"HTTP/1.1 200 OK\r\n\r\n"));
    // A Content-Length that does not parse.
    assert!(!http_response_complete(
        "PUT",
        b"HTTP/1.1 200 OK\r\nContent-Length: lots\r\n\r\nbody"
    ));
    // A header block that is not an HTTP response.
    assert!(!http_response_complete("PUT", b"garbage\r\n\r\n"));
    for malformed in [
        b"HTTP/bogus 200 OK\r\nContent-Length: 0\r\n\r\n".as_slice(),
        b"HTTP/1.1 nope\r\nContent-Length: 0\r\n\r\n",
        b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nContent-Length: 4\r\n\r\n",
        b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\nextra",
        b"HTTP/1.1 200 OK\r\nnot-a-header\r\nContent-Length: 0\r\n\r\n",
    ] {
        assert!(!http_response_complete("PUT", malformed));
    }
    // Nothing received at all.
    assert!(!http_response_complete("PUT", b""));
}

fn temporary_media_root() -> PathBuf {
    std::env::temp_dir().join(format!(
        "nostr-relay-m7-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn websocket_contract(address_one: SocketAddr, address_two: SocketAddr) {
    let mut subscriber = connect_client(address_two);
    let subscriber_challenge = expect_auth_challenge(&mut subscriber);
    send_json(&mut subscriber, json!(["REQ", "sub", {}]));
    let closed = read_json(&mut subscriber);
    assert_eq!(closed[0], "CLOSED");
    assert!(closed[2].as_str().unwrap().starts_with("auth-required:"));
    authenticate(&mut subscriber, 20, &subscriber_challenge);
    send_json(&mut subscriber, json!(["UNSUPPORTED"]));
    assert_eq!(read_json(&mut subscriber)[0], "NOTICE");
    send_json(
        &mut subscriber,
        json!(["REQ", "too-many", {}, {}, {}, {}, {}]),
    );
    assert_eq!(read_json(&mut subscriber)[0], "CLOSED");
    send_json(&mut subscriber, json!(["REQ", "sub", {}]));
    assert_eq!(read_json(&mut subscriber), json!(["EOSE", "sub"]));

    let mut publisher = connect_client(address_one);
    let publisher_challenge = expect_auth_challenge(&mut publisher);
    let auth_event = authenticate(&mut publisher, 21, &publisher_challenge);

    send_json(&mut publisher, json!(["EVENT", auth_event]));
    let rejected_auth_publish = read_json(&mut publisher);
    assert_eq!(rejected_auth_publish[0], "OK");
    assert_eq!(rejected_auth_publish[2], false);

    let mut invalid = signed_event(21, now(), 1, Vec::new(), "invalid signature");
    invalid.content.push('!');
    send_json(&mut publisher, json!(["EVENT", invalid]));
    let invalid_response = read_json(&mut publisher);
    assert_eq!(invalid_response[0], "OK");
    assert_eq!(invalid_response[2], false);

    let regular = signed_event(21, now(), 1, Vec::new(), "cross-process durable");
    send_json(&mut publisher, json!(["EVENT", regular]));
    let accepted = read_json(&mut publisher);
    assert_eq!(accepted[0], "OK");
    assert_eq!(accepted[2], true);
    let delivered = read_json(&mut subscriber);
    assert_eq!(delivered[0], "EVENT");
    assert_eq!(delivered[1], "sub");
    assert_eq!(delivered[2]["id"], regular.id);

    send_json(&mut publisher, json!(["EVENT", regular]));
    let duplicate = read_json(&mut publisher);
    assert_eq!(duplicate[2], true);
    assert!(duplicate[3].as_str().unwrap().starts_with("duplicate:"));
    assert_no_message(&mut subscriber);

    subscription_limit_contract(address_two);
    oversized_frame_contract(address_two);

    let ephemeral = signed_event(22, now(), 20_000, Vec::new(), &"e".repeat(12_000));
    send_json(&mut publisher, json!(["EVENT", ephemeral]));
    assert_eq!(read_json(&mut publisher)[2], true);
    let delivered = read_json(&mut subscriber);
    assert_eq!(delivered[0], "EVENT");
    assert_eq!(delivered[2]["id"], ephemeral.id);

    send_json(
        &mut subscriber,
        json!(["REQ", "ephemeral-history", {"ids": [ephemeral.id]}]),
    );
    assert_eq!(
        read_json(&mut subscriber),
        json!(["EOSE", "ephemeral-history"])
    );

    send_json(&mut subscriber, json!(["CLOSE", "sub"]));
    let after_close = signed_event(23, now(), 1, Vec::new(), "after close");
    send_json(&mut publisher, json!(["EVENT", after_close]));
    assert_eq!(read_json(&mut publisher)[2], true);
    assert_no_message(&mut subscriber);

    subscriber.close(None).unwrap();
    publisher.close(None).unwrap();
    openagents_profile_contract(address_one);
}

fn openagents_profile_contract(address: SocketAddr) {
    let mut publisher = connect_client(address);
    let challenge = expect_auth_challenge(&mut publisher);
    authenticate(&mut publisher, 41, &challenge);

    let bad = signed_event(
        41,
        now(),
        30_180,
        vec![
            Tag::new(vec!["d".into(), "demo".into()]),
            Tag::new(vec!["t".into(), "oa:cap:v1".into()]),
        ],
        "{}",
    );
    send_json(&mut publisher, json!(["EVENT", bad]));
    let rejected = read_json(&mut publisher);
    assert_eq!(rejected[0], "OK");
    assert_eq!(rejected[2], false);
    assert!(rejected[3].as_str().unwrap_or("").starts_with("invalid:"));

    let author = pubkey(41);
    let record = signed_event(
        41,
        now(),
        3_187,
        vec![
            Tag::new(vec!["p".into(), author.clone()]),
            Tag::new(vec!["h".into(), "ab".repeat(32)]),
            Tag::new(vec!["t".into(), "oa:run:v1".into()]),
        ],
        &fake_nip44_v2(),
    );
    send_json(&mut publisher, json!(["EVENT", record.clone()]));
    let accepted = read_json(&mut publisher);
    assert_eq!(accepted[0], "OK", "{accepted}");
    assert_eq!(accepted[2], true, "{accepted}");
    assert!(!accepted[3].as_str().unwrap_or("").contains("executed"));

    let mut stranger = connect_client(address);
    let stranger_challenge = expect_auth_challenge(&mut stranger);
    authenticate(&mut stranger, 42, &stranger_challenge);
    send_json(
        &mut stranger,
        json!(["REQ", "hidden", {"ids": [record.id]}]),
    );
    assert_eq!(read_json(&mut stranger), json!(["EOSE", "hidden"]));

    let mut reader = connect_client(address);
    let reader_challenge = expect_auth_challenge(&mut reader);
    authenticate(&mut reader, 41, &reader_challenge);
    send_json(&mut reader, json!(["REQ", "own", {"ids": [record.id]}]));
    let found = read_json(&mut reader);
    assert_eq!(found[0], "EVENT");
    assert_eq!(found[2]["id"], record.id);
    assert_eq!(read_json(&mut reader), json!(["EOSE", "own"]));
}

fn management_contract(address: SocketAddr) {
    let methods = rpc_request(address, "supportedmethods", json!([]));
    assert!(methods["error"].is_null());
    for method in ["banpubkey", "allowkind", "creategroup", "putgroupuser"] {
        assert!(
            methods["result"]
                .as_array()
                .unwrap()
                .contains(&json!(method))
        );
    }

    let created = rpc_request(
        address,
        "creategroup",
        json!([
            "fixture-group",
            "Fixture Group",
            "M6 live group",
            "",
            false,
            pubkey(30),
            [1]
        ]),
    );
    assert_eq!(created["result"], true);
    let duplicate = rpc_request(
        address,
        "creategroup",
        json!([
            "fixture-group",
            "Duplicate Fixture Group",
            "must not restart the relay",
            "",
            false,
            pubkey(30),
            [1]
        ]),
    );
    assert!(
        duplicate["error"]
            .as_str()
            .unwrap()
            .contains("already exists")
    );
    assert!(
        rpc_request(address, "supportedmethods", json!([]))["error"].is_null(),
        "a rejected management mutation leaves the process current"
    );

    let banned = rpc_request(address, "banpubkey", json!([pubkey(88), "fixture block"]));
    assert_eq!(banned["result"], true);
    let listed = rpc_request(address, "listbannedpubkeys", json!([]));
    assert!(
        listed["result"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| { entry["pubkey"] == pubkey(88) && entry["reason"] == "fixture block" })
    );
    assert_eq!(
        rpc_request(address, "unbanpubkey", json!([pubkey(88)]))["result"],
        true
    );
}

fn expanded_protocol_contract(address_one: SocketAddr, address_two: SocketAddr) -> String {
    protected_and_private_contract(address_one, address_two);
    search_and_count_contract(address_one, address_two);
    group_contract(address_one, address_two);
    block_server_contract(address_one, address_two);
    expiration_contract(address_one, address_two)
}

fn block_server_contract(address_one: SocketAddr, address_two: SocketAddr) {
    let mut owner = connect_client(address_one);
    let challenge = expect_auth_challenge(&mut owner);
    authenticate(&mut owner, 21, &challenge);

    let mut agent = connect_client(address_two);
    let challenge = expect_auth_challenge(&mut agent);
    let agent_auth = agent_auth_event(22, 21, &challenge);
    send_json(&mut agent, json!(["AUTH", agent_auth]));
    assert_eq!(read_json(&mut agent)[2], true);

    send_json(
        &mut owner,
        json!(["REQ", "observer-owner", {"kinds":[24200], "#p":[pubkey(21)]}]),
    );
    assert_eq!(read_json(&mut owner), json!(["EOSE", "observer-owner"]));
    send_json(
        &mut agent,
        json!(["REQ", "observer-agent", {"kinds":[24200], "#p":[pubkey(22)]}]),
    );
    assert_eq!(read_json(&mut agent), json!(["EOSE", "observer-agent"]));
    let control = signed_event(
        21,
        now(),
        24_200,
        vec![
            Tag::new(vec!["p".into(), pubkey(22)]),
            Tag::new(vec!["agent".into(), pubkey(22)]),
            Tag::new(vec!["frame".into(), "control".into()]),
        ],
        &fake_nip44_v2(),
    );
    send_json(&mut owner, json!(["EVENT", control]));
    assert_eq!(read_json(&mut owner)[2], true);
    assert_eq!(read_json(&mut agent)[2]["id"], control.id);
    let telemetry = signed_event(
        22,
        now(),
        24_200,
        vec![
            Tag::new(vec!["p".into(), pubkey(21)]),
            Tag::new(vec!["agent".into(), pubkey(22)]),
            Tag::new(vec!["frame".into(), "telemetry".into()]),
        ],
        &fake_nip44_v2(),
    );
    send_json(&mut agent, json!(["EVENT", telemetry]));
    assert_eq!(read_json(&mut agent)[2], true);
    assert_eq!(read_json(&mut owner)[2]["id"], telemetry.id);
    send_json(
        &mut owner,
        json!(["REQ", "observer-history", {"kinds":[24200], "#p":[pubkey(21)]}]),
    );
    assert_eq!(read_json(&mut owner), json!(["EOSE", "observer-history"]));

    send_json(
        &mut owner,
        json!(["REQ", "turn-metrics", {"kinds":[44200], "#p":[pubkey(21)]}]),
    );
    assert_eq!(read_json(&mut owner), json!(["EOSE", "turn-metrics"]));
    let metric = signed_event(
        22,
        now(),
        44_200,
        vec![
            Tag::new(vec!["p".into(), pubkey(21)]),
            Tag::new(vec!["agent".into(), pubkey(22)]),
        ],
        &fake_nip44_v2(),
    );
    send_json(&mut agent, json!(["EVENT", metric]));
    assert_eq!(read_json(&mut agent)[2], true);
    assert_eq!(read_json(&mut owner)[2]["id"], metric.id);

    let engram = signed_event(
        22,
        now(),
        30_174,
        vec![
            Tag::new(vec!["d".into(), "a".repeat(64)]),
            Tag::new(vec!["p".into(), pubkey(21)]),
        ],
        &fake_nip44_v2(),
    );
    send_json(&mut owner, json!(["EVENT", engram]));
    assert_eq!(read_json(&mut owner)[2], true);
    send_json(
        &mut owner,
        json!(["REQ", "engram-owner", {"kinds":[30174], "#p":[pubkey(21)]}]),
    );
    assert_eq!(read_json(&mut owner)[2]["id"], engram.id);
    assert_eq!(read_json(&mut owner), json!(["EOSE", "engram-owner"]));

    let unshared = signed_event(
        21,
        now(),
        30_175,
        vec![Tag::new(vec!["d".into(), "private-agent".into()])],
        r#"{"display_name":"Private"}"#,
    );
    let shared = signed_event(
        21,
        now(),
        30_175,
        vec![
            Tag::new(vec!["d".into(), "shared-agent".into()]),
            Tag::new(vec!["shared".into(), "true".into()]),
        ],
        r#"{"display_name":"Shared"}"#,
    );
    for event in [&unshared, &shared] {
        send_json(&mut owner, json!(["EVENT", event]));
        assert_eq!(read_json(&mut owner)[2], true);
    }

    let reminder = signed_event(
        21,
        now(),
        30_300,
        vec![
            Tag::new(vec!["d".into(), "0123456789abcdef0123456789abcdef".into()]),
            Tag::new(vec!["not_before".into(), (now() + 60).to_string()]),
        ],
        &fake_nip44_v2(),
    );
    send_json(&mut owner, json!(["EVENT", reminder]));
    assert_eq!(read_json(&mut owner)[2], true);

    let project = signed_event(
        21,
        now(),
        30_621,
        vec![
            Tag::new(vec!["d".into(), "platform".into()]),
            Tag::new(vec![
                "a".into(),
                format!("30617:{}:nostr-relay", pubkey(21)),
            ]),
            Tag::new(vec!["h".into(), "not-a-group-scope".into()]),
            Tag::new(vec!["previous".into(), "opaque-to-nip-mp".into()]),
        ],
        "ignored",
    );
    send_json(&mut owner, json!(["EVENT", project]));
    assert_eq!(read_json(&mut owner)[2], true);

    let lease = signed_event(
        21,
        now(),
        30_350,
        vec![
            Tag::new(vec!["d".into(), "installation".into()]),
            Tag::new(vec!["expiration".into(), (now() + 3_600).to_string()]),
            Tag::new(vec!["exec".into(), "unadvertised".into()]),
        ],
        &fake_nip44_v2(),
    );
    send_json(&mut owner, json!(["EVENT", lease]));
    let refusal = read_json(&mut owner);
    assert_eq!(refusal[2], false);
    assert!(refusal[3].as_str().unwrap().starts_with("restricted:"));

    let archive = signed_event(
        21,
        now(),
        9_035,
        vec![
            Tag::new(vec!["-".into()]),
            Tag::new(vec!["p".into(), pubkey(21)]),
            Tag::new(vec!["reason".into(), "rotated".into()]),
        ],
        "retired",
    );
    send_json(&mut owner, json!(["EVENT", archive]));
    assert_eq!(read_json(&mut owner)[2], true);
    send_json(
        &mut owner,
        json!(["REQ", "archive-state", {"kinds":[8002,13535], "authors":[pubkey(90)]}]),
    );
    let mut archival_kinds = Vec::new();
    loop {
        let message = read_json(&mut owner);
        if message == json!(["EOSE", "archive-state"]) {
            break;
        }
        archival_kinds.push(message[2]["kind"].as_u64().unwrap());
    }
    archival_kinds.sort_unstable();
    assert_eq!(archival_kinds, vec![8_002, 13_535]);

    let owner_archive = signed_event(
        21,
        now(),
        9_035,
        vec![
            Tag::new(vec!["-".into()]),
            Tag::new(vec!["p".into(), pubkey(22)]),
            owner_binding_tag(22, 21),
        ],
        "owner retired agent",
    );
    send_json(&mut owner, json!(["EVENT", owner_archive]));
    assert_eq!(read_json(&mut owner)[2], true);

    let mut outsider = connect_client(address_two);
    let challenge = expect_auth_challenge(&mut outsider);
    authenticate(&mut outsider, 20, &challenge);
    send_json(
        &mut outsider,
        json!(["REQ", "personas", {"kinds":[30175], "authors":[pubkey(21)]}]),
    );
    assert_eq!(read_json(&mut outsider)[2]["id"], shared.id);
    assert_eq!(read_json(&mut outsider), json!(["EOSE", "personas"]));
    send_json(
        &mut outsider,
        json!(["REQ", "foreign-reminder", {"kinds":[30300], "authors":[pubkey(21)]}]),
    );
    assert_eq!(read_json(&mut outsider)[0], "CLOSED");

    // NIP-CW's permitted WebSocket degradation strips extension fields and
    // serves the standard channel filter without synthesizing fake overlays.
    send_json(
        &mut outsider,
        json!(["REQ", "cw-degrade", {"#h":["fixture-group"], "top_level":true, "include_summaries":true, "before_id":"0".repeat(64)}]),
    );
    loop {
        let message = read_json(&mut outsider);
        if message == json!(["EOSE", "cw-degrade"]) {
            break;
        }
        assert_ne!(message[2]["kind"], 39_006);
    }

    let mut admin = connect_client(address_one);
    let challenge = expect_auth_challenge(&mut admin);
    authenticate(&mut admin, 30, &challenge);
    let hide = signed_event(
        30,
        now(),
        41_012,
        vec![Tag::new(vec!["h".into(), "fixture-group".into()])],
        "",
    );
    send_json(&mut admin, json!(["EVENT", hide]));
    assert_eq!(read_json(&mut admin)[2], true);
    send_json(
        &mut admin,
        json!(["REQ", "hidden", {"kinds":[30622], "#p":[pubkey(30)]}]),
    );
    let snapshot = read_json(&mut admin);
    assert_eq!(snapshot[2]["kind"], 30_622);
    assert_eq!(snapshot[2]["pubkey"], pubkey(90));
    assert_eq!(read_json(&mut admin), json!(["EOSE", "hidden"]));
    send_json(&mut admin, json!(["CLOSE", "hidden"]));
    let open = signed_event(
        30,
        now(),
        41_010,
        vec![Tag::new(vec!["h".into(), "fixture-group".into()])],
        "",
    );
    send_json(&mut admin, json!(["EVENT", open]));
    assert_eq!(read_json(&mut admin)[2], true);
    send_json(
        &mut admin,
        json!(["REQ", "visible", {"kinds":[30622], "#p":[pubkey(30)]}]),
    );
    let visible = read_json(&mut admin);
    assert_eq!(visible[2]["kind"], 30_622);
    assert!(
        visible[2]["tags"]
            .as_array()
            .unwrap()
            .iter()
            .all(|tag| tag[0] != "h")
    );
    assert_eq!(read_json(&mut admin), json!(["EOSE", "visible"]));

    let mut manager = connect_client(address_one);
    let challenge = expect_auth_challenge(&mut manager);
    authenticate(&mut manager, 91, &challenge);
    let profile = signed_event(
        91,
        now(),
        9_033,
        vec![Tag::new(vec![
            "icon".into(),
            "https://example.com/workspace.png".into(),
        ])],
        "",
    );
    send_json(&mut manager, json!(["EVENT", profile]));
    assert_eq!(read_json(&mut manager)[2], true);
    assert_nip11_icon(address_two, "https://example.com/workspace.png");

    manager.close(None).unwrap();
    admin.close(None).unwrap();
    outsider.close(None).unwrap();
    agent.close(None).unwrap();
    owner.close(None).unwrap();
}

fn assert_nip11_icon(address: SocketAddr, expected: &str) {
    let mut stream = StdTcpStream::connect(address).unwrap();
    write!(
        stream,
        "GET / HTTP/1.1\r\nHost: relay.test\r\nAccept: application/nostr+json\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let document: Value = serde_json::from_str(response.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(document["icon"], expected);
}

fn protected_and_private_contract(address_one: SocketAddr, address_two: SocketAddr) {
    let mut publisher = connect_client(address_one);
    let publisher_challenge = expect_auth_challenge(&mut publisher);
    authenticate(&mut publisher, 21, &publisher_challenge);

    let protected = signed_event(21, now(), 1, vec![Tag::new(vec!["-".into()])], "protected");
    send_json(&mut publisher, json!(["EVENT", protected]));
    assert_eq!(read_json(&mut publisher)[2], true);

    let forwarded = signed_event(22, now(), 1, vec![Tag::new(vec!["-".into()])], "forwarded");
    send_json(&mut publisher, json!(["EVENT", forwarded]));
    let refusal = read_json(&mut publisher);
    assert_eq!(refusal[2], false);
    assert!(refusal[3].as_str().unwrap().starts_with("auth-required:"));

    let repost = signed_event(
        21,
        now(),
        6,
        Vec::new(),
        &serde_json::to_string(&protected).unwrap(),
    );
    send_json(&mut publisher, json!(["EVENT", repost]));
    assert_eq!(read_json(&mut publisher)[2], false);
    let generic_repost = signed_event(
        21,
        now(),
        16,
        Vec::new(),
        &serde_json::to_string(&protected).unwrap(),
    );
    send_json(&mut publisher, json!(["EVENT", generic_repost]));
    assert_eq!(read_json(&mut publisher)[2], false);

    let expired_public = signed_event(
        21,
        now(),
        39_600,
        vec![
            Tag::new(vec!["d".into(), "expired-provider".into()]),
            Tag::new(vec!["status".into(), "active".into()]),
            Tag::new(vec!["profile".into(), "conformance".into(), "1".into()]),
            Tag::new(vec!["published_at".into(), now().to_string()]),
            Tag::new(vec!["expiration".into(), now().to_string()]),
        ],
        "{}",
    );
    send_json(&mut publisher, json!(["EVENT", expired_public]));
    let refusal = read_json(&mut publisher);
    assert_eq!(refusal[2], false);
    assert!(refusal[3].as_str().unwrap().contains("expired"));

    let expired_wrap = signed_event(
        54,
        now(),
        1_059,
        vec![
            Tag::new(vec!["p".into(), pubkey(32)]),
            Tag::new(vec!["expiration".into(), now().to_string()]),
        ],
        "expired encrypted gift wrap",
    );
    send_json(&mut publisher, json!(["EVENT", expired_wrap]));
    let refusal = read_json(&mut publisher);
    assert_eq!(refusal[2], false);
    assert!(refusal[3].as_str().unwrap().contains("expired"));

    let mut recipient = connect_client(address_two);
    let challenge = expect_auth_challenge(&mut recipient);
    authenticate(&mut recipient, 30, &challenge);
    send_json(
        &mut recipient,
        json!(["REQ", "dm", {"kinds": [1059], "#p": [pubkey(30)]}]),
    );
    assert_eq!(read_json(&mut recipient), json!(["EOSE", "dm"]));

    let mut unauthenticated = connect_client(address_two);
    let _challenge = expect_auth_challenge(&mut unauthenticated);
    send_json(
        &mut unauthenticated,
        json!(["REQ", "unauth-wrap", {"kinds": [1059], "#p": [pubkey(30)]}]),
    );
    assert_eq!(
        read_json(&mut unauthenticated),
        json!([
            "CLOSED",
            "unauth-wrap",
            "auth-required: authenticate before subscribing"
        ])
    );
    unauthenticated.close(None).unwrap();

    let mut outsider = connect_client(address_two);
    let challenge = expect_auth_challenge(&mut outsider);
    authenticate(&mut outsider, 31, &challenge);
    send_json(
        &mut outsider,
        json!(["REQ", "not-my-dm", {"kinds": [1059], "#p": [pubkey(30)]}]),
    );
    assert_eq!(
        read_json(&mut outsider),
        json!([
            "CLOSED",
            "not-my-dm",
            "restricted: gift-wrap reads must be scoped to #p self"
        ])
    );
    send_json(&mut outsider, json!(["REQ", "broad-outsider", {}]));
    loop {
        let message = read_json(&mut outsider);
        if message == json!(["EOSE", "broad-outsider"]) {
            break;
        }
        assert_ne!(message[2]["kind"], 1_059);
    }

    let wrap = signed_event(
        55,
        now(),
        1_059,
        vec![Tag::new(vec!["p".into(), pubkey(30)])],
        "encrypted gift wrap",
    );
    let mut forged_wrap = wrap.clone();
    forged_wrap.content.push('!');
    send_json(&mut publisher, json!(["EVENT", forged_wrap]));
    let refusal = read_json(&mut publisher);
    assert_eq!(refusal[2], false);
    assert!(refusal[3].as_str().unwrap().starts_with("invalid:"));
    send_json(&mut publisher, json!(["EVENT", wrap]));
    assert_eq!(read_json(&mut publisher)[2], true);
    assert_eq!(read_json(&mut recipient)[2]["id"], wrap.id);
    assert_no_message(&mut outsider);

    let recovery_wrap = signed_event(
        58,
        now(),
        1_059,
        vec![Tag::new(vec!["p".into(), pubkey(21)])],
        "encrypted gift wrap",
    );
    assert_ne!(recovery_wrap.id, wrap.id);
    assert_eq!(recovery_wrap.content, wrap.content);
    send_json(&mut publisher, json!(["EVENT", recovery_wrap]));
    assert_eq!(read_json(&mut publisher)[2], true);

    let mut history = connect_client(address_two);
    let challenge = expect_auth_challenge(&mut history);
    authenticate(&mut history, 30, &challenge);
    send_json(
        &mut history,
        json!(["REQ", "wrap-history", {"kinds": [1059], "#p": [pubkey(30)]}]),
    );
    assert_eq!(read_json(&mut history)[2]["id"], wrap.id);
    assert_eq!(read_json(&mut history), json!(["EOSE", "wrap-history"]));

    send_json(&mut history, json!(["REQ", "wrap-id", {"ids": [wrap.id]}]));
    assert_eq!(read_json(&mut history)[2]["id"], wrap.id);
    assert_eq!(read_json(&mut history), json!(["EOSE", "wrap-id"]));

    send_json(
        &mut history,
        json!(["COUNT", "wrap-count", {"kinds": [1059], "#p": [pubkey(30)]}]),
    );
    assert_eq!(
        read_json(&mut history),
        json!(["COUNT", "wrap-count", {"count": 1}])
    );

    send_json(
        &mut history,
        json!(["REQ", "wrap-search", {"search": "encrypted gift wrap"}]),
    );
    assert_eq!(read_json(&mut history), json!(["EOSE", "wrap-search"]));
    send_json(
        &mut history,
        json!(["COUNT", "wrap-search-count", {"search": "encrypted gift wrap"}]),
    );
    assert_eq!(
        read_json(&mut history),
        json!(["COUNT", "wrap-search-count", {"count": 0}])
    );

    send_json(
        &mut outsider,
        json!(["REQ", "wrap-id-outsider", {"ids": [wrap.id]}]),
    );
    assert_eq!(
        read_json(&mut outsider),
        json!(["EOSE", "wrap-id-outsider"])
    );
    send_json(
        &mut outsider,
        json!(["COUNT", "wrap-id-count-outsider", {"ids": [wrap.id]}]),
    );
    assert_eq!(
        read_json(&mut outsider),
        json!(["COUNT", "wrap-id-count-outsider", {"count": 0}])
    );

    let second_wrap = signed_event(
        56,
        now(),
        1_059,
        vec![Tag::new(vec!["p".into(), pubkey(30)])],
        "second encrypted gift wrap",
    );
    send_json(&mut publisher, json!(["EVENT", second_wrap]));
    let refusal = read_json(&mut publisher);
    assert_eq!(refusal[2], false);
    assert_eq!(
        refusal[3],
        "rate-limited: gift-wrap recipient rate exceeded"
    );

    history.close(None).unwrap();

    recipient.close(None).unwrap();
    outsider.close(None).unwrap();
    publisher.close(None).unwrap();
}

fn insert_legacy_wrap(event: &Event) {
    let database_url = std::env::var("NOSTR_RELAY_TEST_DATABASE_URL").unwrap();
    let event = event.clone();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async move {
            let (mut client, connection) = tokio_postgres::connect(&database_url, NoTls).await.unwrap();
            tokio::spawn(async move { connection.await.unwrap() });
            let transaction = client.transaction().await.unwrap();
            let tags = serde_json::to_string(&event.tags).unwrap();
            transaction
                .execute(
                    "INSERT INTO nostr_event (id, pubkey, created_at, kind, tags, content, sig, replacement_identifier) VALUES ($1, $2, $3, $4, $5::text::jsonb, $6, $7, NULL)",
                    &[&event.id, &event.pubkey, &i64::try_from(event.created_at).unwrap(), &i32::from(event.kind), &tags, &event.content, &event.sig],
                )
                .await
                .unwrap();
            for recipient in event.tag_values("p") {
                transaction
                    .execute(
                        "INSERT INTO nostr_indexed_tag (event_id, tag_name, tag_value, created_at) VALUES ($1, 'p', $2, $3)",
                        &[&event.id, &recipient, &i64::try_from(event.created_at).unwrap()],
                    )
                    .await
                    .unwrap();
            }
            transaction.commit().await.unwrap();
        });
}

fn malformed_legacy_wrap_contract(address: SocketAddr) {
    let malformed_wrap = signed_event(
        58,
        now(),
        1_059,
        vec![
            Tag::new(vec!["p".into(), pubkey(21)]),
            Tag::new(vec!["p".into(), pubkey(31)]),
        ],
        "legacy malformed multi recipient wrap",
    );
    insert_legacy_wrap(&malformed_wrap);

    let mut reader = connect_client(address);
    let challenge = expect_auth_challenge(&mut reader);
    authenticate(&mut reader, 21, &challenge);
    send_json(
        &mut reader,
        json!(["REQ", "malformed-wrap-id", {"ids": [malformed_wrap.id]}]),
    );
    assert_eq!(read_json(&mut reader), json!(["EOSE", "malformed-wrap-id"]));
    send_json(
        &mut reader,
        json!(["COUNT", "malformed-wrap-count", {"ids": [malformed_wrap.id]}]),
    );
    assert_eq!(
        read_json(&mut reader),
        json!(["COUNT", "malformed-wrap-count", {"count": 0}])
    );
    reader.close(None).unwrap();
}

fn search_and_count_contract(address_one: SocketAddr, address_two: SocketAddr) {
    let mut publisher = connect_client(address_one);
    let challenge = expect_auth_challenge(&mut publisher);
    authenticate(&mut publisher, 21, &challenge);
    let searchable = signed_event(21, now(), 1, Vec::new(), "violet protocol expansion marker");
    let unrelated = signed_event(21, now(), 1, Vec::new(), "ordinary unrelated text");
    for event in [&searchable, &unrelated] {
        send_json(&mut publisher, json!(["EVENT", event]));
        let response = read_json(&mut publisher);
        assert_eq!(response[2], true, "{response}");
    }

    let mut reader = connect_client(address_two);
    let challenge = expect_auth_challenge(&mut reader);
    authenticate(&mut reader, 20, &challenge);
    send_json(
        &mut reader,
        json!(["REQ", "search", {"search": "violet expansion"}]),
    );
    let result = read_json(&mut reader);
    assert_eq!(result[0], "EVENT");
    assert_eq!(result[2]["id"], searchable.id);
    assert_eq!(read_json(&mut reader), json!(["EOSE", "search"]));

    send_json(
        &mut reader,
        json!(["COUNT", "count-search", {"search": "violet expansion"}]),
    );
    assert_eq!(
        read_json(&mut reader),
        json!(["COUNT", "count-search", {"count": 1}])
    );

    // Differential fixture: the same corpus published before and after the
    // subscription yields the same members through replay and live
    // delivery. The contract is ASCII-case-insensitive substring matching
    // with gift wraps excluded, so `cat` finds `catwalk` and `Cat,` in both
    // paths, and neither finds `chát`, `ÇAT`, or wrapped ciphertext.
    let corpus: [(u16, &str); 8] = [
        (1, "cat"),
        (1, "catwalk"),
        (1, "Cat, sat."),
        (1, "concatenate"),
        (1, "dog"),
        (1, "chát"),
        (1, "ÇAT"),
        (1_059, "cat"),
    ];
    let expected = ["cat", "catwalk", "Cat, sat.", "concatenate"];
    // Each corpus wraps to its own recipient, within the per-recipient
    // gift-wrap rate; the live wrap is addressed to the reader itself.
    let corpus_event = |secret_byte: u8, recipient: u8, kind: u16, content: &str| {
        let tags = if kind == 1_059 {
            vec![Tag::new(vec!["p".into(), pubkey(recipient)])]
        } else {
            Vec::new()
        };
        signed_event(secret_byte, now(), kind, tags, content)
    };
    let mut before = Vec::new();
    for (kind, content) in corpus {
        let event = corpus_event(21, 23, kind, content);
        send_json(&mut publisher, json!(["EVENT", event]));
        let response = read_json(&mut publisher);
        assert_eq!(response[2], true, "{response}");
        before.push(event);
    }
    send_json(&mut reader, json!(["REQ", "cat", {"search": "cat"}]));
    let mut replayed = Vec::new();
    loop {
        let message = read_json(&mut reader);
        if message == json!(["EOSE", "cat"]) {
            break;
        }
        assert_eq!(message[0], "EVENT");
        assert_eq!(message[1], "cat");
        let event: Event = serde_json::from_value(message[2].clone()).unwrap();
        assert!(
            before.iter().any(|published| published.id == event.id),
            "replayed a foreign event: {}",
            event.content
        );
        replayed.push(event.content);
    }
    replayed.sort();
    let mut expected_sorted = expected.map(str::to_owned).to_vec();
    expected_sorted.sort();
    assert_eq!(replayed, expected_sorted);

    let mut live_publisher = connect_client(address_one);
    let challenge = expect_auth_challenge(&mut live_publisher);
    authenticate(&mut live_publisher, 22, &challenge);
    let mut after = Vec::new();
    for (kind, content) in corpus {
        let event = corpus_event(22, 20, kind, content);
        send_json(&mut live_publisher, json!(["EVENT", event]));
        let response = read_json(&mut live_publisher);
        assert_eq!(response[2], true, "{response}");
        after.push(event);
    }
    let sentinel = corpus_event(22, 20, 1, "cat sentinel");
    send_json(&mut live_publisher, json!(["EVENT", sentinel.clone()]));
    assert_eq!(read_json(&mut live_publisher)[2], true);
    let mut delivered = Vec::new();
    loop {
        let message = read_json(&mut reader);
        assert_eq!(message[0], "EVENT");
        assert_eq!(message[1], "cat");
        let event: Event = serde_json::from_value(message[2].clone()).unwrap();
        if event.id == sentinel.id {
            break;
        }
        assert!(
            after.iter().any(|published| published.id == event.id),
            "delivered a foreign event: {}",
            event.content
        );
        delivered.push(event.content);
    }
    delivered.sort();
    assert_eq!(delivered, replayed);
    send_json(&mut reader, json!(["CLOSE", "cat"]));

    send_json(&mut reader, json!(["REQ", "accent", {"search": "chát"}]));
    let mut accented = Vec::new();
    loop {
        let message = read_json(&mut reader);
        if message == json!(["EOSE", "accent"]) {
            break;
        }
        let event: Event = serde_json::from_value(message[2].clone()).unwrap();
        accented.push(event.content);
    }
    assert_eq!(accented, vec!["chát".to_owned(), "chát".to_owned()]);
    live_publisher.close(None).unwrap();
    reader.close(None).unwrap();
    publisher.close(None).unwrap();
}

fn group_contract(address_one: SocketAddr, address_two: SocketAddr) {
    let mut metadata_reader = connect_client(address_two);
    let challenge = expect_auth_challenge(&mut metadata_reader);
    authenticate(&mut metadata_reader, 20, &challenge);
    send_json(
        &mut metadata_reader,
        json!(["REQ", "metadata", {
            "kinds": [39000, 39001, 39002, 39003, 39004, 39005],
            "#d": ["fixture-group"]
        }]),
    );
    let mut metadata_kinds = Vec::new();
    loop {
        let message = read_json(&mut metadata_reader);
        if message == json!(["EOSE", "metadata"]) {
            break;
        }
        let event: Event = serde_json::from_value(message[2].clone()).unwrap();
        event.validate_crypto().unwrap();
        assert_eq!(event.pubkey, pubkey(90));
        metadata_kinds.push(event.kind);
    }
    metadata_kinds.sort_unstable();
    assert_eq!(
        metadata_kinds,
        vec![39_000, 39_001, 39_002, 39_003, 39_004, 39_005]
    );
    metadata_reader.close(None).unwrap();

    let mut admin = connect_client(address_one);
    let challenge = expect_auth_challenge(&mut admin);
    authenticate(&mut admin, 30, &challenge);
    let group_message = signed_event(
        30,
        now(),
        1,
        vec![Tag::new(vec!["h".into(), "fixture-group".into()])],
        "member message",
    );
    let group_message_prefix = group_message.id[..8].to_owned();
    send_json(&mut admin, json!(["EVENT", group_message]));
    assert_eq!(read_json(&mut admin)[2], true);

    let mut member = connect_client(address_one);
    let challenge = expect_auth_challenge(&mut member);
    authenticate(&mut member, 31, &challenge);
    let refused = signed_event(
        31,
        now(),
        1,
        vec![Tag::new(vec!["h".into(), "fixture-group".into()])],
        "not a member",
    );
    send_json(&mut member, json!(["EVENT", refused]));
    assert_eq!(read_json(&mut member)[2], false);

    let put_user = signed_event(
        30,
        now(),
        9_000,
        vec![
            Tag::new(vec!["h".into(), "fixture-group".into()]),
            Tag::new(vec!["p".into(), pubkey(31)]),
        ],
        "",
    );
    send_json(&mut admin, json!(["EVENT", put_user]));
    assert_eq!(read_json(&mut admin)[2], true);
    let admitted = signed_event(
        31,
        now(),
        1,
        vec![
            Tag::new(vec!["h".into(), "fixture-group".into()]),
            Tag::new(vec!["previous".into(), group_message_prefix]),
        ],
        "now a member",
    );
    send_json(&mut member, json!(["EVENT", admitted]));
    assert_eq!(read_json(&mut member)[2], true);
    let unknown_previous = signed_event(
        31,
        now(),
        1,
        vec![
            Tag::new(vec!["h".into(), "fixture-group".into()]),
            Tag::new(vec!["previous".into(), "deadbeef".into()]),
        ],
        "unknown timeline",
    );
    send_json(&mut member, json!(["EVENT", unknown_previous]));
    assert_eq!(read_json(&mut member)[2], false);

    let mut joiner = connect_client(address_one);
    let challenge = expect_auth_challenge(&mut joiner);
    authenticate(&mut joiner, 32, &challenge);
    let join = signed_event(
        32,
        now(),
        9_021,
        vec![Tag::new(vec!["h".into(), "fixture-group".into()])],
        "",
    );
    send_json(&mut joiner, json!(["EVENT", join]));
    assert_eq!(read_json(&mut joiner)[2], true);
    let joined_message = signed_event(
        32,
        now(),
        1,
        vec![Tag::new(vec!["h".into(), "fixture-group".into()])],
        "joined",
    );
    send_json(&mut joiner, json!(["EVENT", joined_message]));
    assert_eq!(read_json(&mut joiner)[2], true);
    let leave = signed_event(
        32,
        now(),
        9_022,
        vec![Tag::new(vec!["h".into(), "fixture-group".into()])],
        "",
    );
    send_json(&mut joiner, json!(["EVENT", leave]));
    assert_eq!(read_json(&mut joiner)[2], true);
    let after_leave = signed_event(
        32,
        now(),
        1,
        vec![Tag::new(vec!["h".into(), "fixture-group".into()])],
        "after leave",
    );
    send_json(&mut joiner, json!(["EVENT", after_leave]));
    assert_eq!(read_json(&mut joiner)[2], false);

    send_json(
        &mut joiner,
        json!(["REQ", "relay-membership-history", {
            "authors": [pubkey(90)],
            "kinds": [9000, 9001],
            "#h": ["fixture-group"],
            "#p": [pubkey(32)]
        }]),
    );
    let mut accepted_requests = std::collections::HashSet::new();
    loop {
        let message = read_json(&mut joiner);
        if message == json!(["EOSE", "relay-membership-history"]) {
            break;
        }
        let event: Event = serde_json::from_value(message[2].clone()).unwrap();
        event.validate_crypto().unwrap();
        accepted_requests.extend(event.tag_values("e").map(str::to_owned));
    }
    assert_eq!(
        accepted_requests,
        std::collections::HashSet::from([join.id, leave.id])
    );

    let create_secret = signed_event(
        90,
        now(),
        9_007,
        vec![Tag::new(vec!["h".into(), "secret-room".into()])],
        "",
    );
    send_json(&mut admin, json!(["EVENT", create_secret]));
    assert_eq!(
        read_json(&mut admin)[2],
        true,
        "relay creates the private group"
    );
    let seal_secret = signed_event(
        90,
        now(),
        9_002,
        vec![
            Tag::new(vec!["h".into(), "secret-room".into()]),
            Tag::new(vec!["name".into(), "Secret".into()]),
            Tag::new(vec!["private".into()]),
            Tag::new(vec!["hidden".into()]),
            Tag::new(vec!["restricted".into()]),
        ],
        "",
    );
    send_json(&mut admin, json!(["EVENT", seal_secret]));
    assert_eq!(
        read_json(&mut admin)[2],
        true,
        "private and hidden flags store"
    );
    let secret_message = signed_event(
        90,
        now(),
        1,
        vec![Tag::new(vec!["h".into(), "secret-room".into()])],
        "members only",
    );
    send_json(&mut admin, json!(["EVENT", secret_message.clone()]));
    assert_eq!(read_json(&mut admin)[2], true);

    let mut stranger = connect_client(address_one);
    let challenge = expect_auth_challenge(&mut stranger);
    authenticate(&mut stranger, 41, &challenge);
    send_json(
        &mut stranger,
        json!(["REQ", "secret-chat", {"kinds":[1], "#h":["secret-room"]}]),
    );
    assert_eq!(read_json(&mut stranger), json!(["EOSE", "secret-chat"]));
    send_json(
        &mut stranger,
        json!(["REQ", "secret-meta", {"kinds":[39000], "#d":["secret-room"]}]),
    );
    assert_eq!(read_json(&mut stranger), json!(["EOSE", "secret-meta"]));

    let mut member_reader = connect_client(address_one);
    let challenge = expect_auth_challenge(&mut member_reader);
    authenticate(&mut member_reader, 90, &challenge);
    send_json(
        &mut member_reader,
        json!(["REQ", "secret-member", {"kinds":[1], "#h":["secret-room"]}]),
    );
    let visible = read_json(&mut member_reader);
    assert_eq!(visible[0], "EVENT", "{visible}");
    let visible_event: Event = serde_json::from_value(visible[2].clone()).unwrap();
    assert_eq!(visible_event.id, secret_message.id);
    assert_eq!(
        read_json(&mut member_reader),
        json!(["EOSE", "secret-member"])
    );
    member_reader.close(None).unwrap();

    let create_parent = signed_event(
        90,
        now(),
        9_007,
        vec![Tag::new(vec!["h".into(), "parent-room".into()])],
        "",
    );
    send_json(&mut admin, json!(["EVENT", create_parent]));
    assert_eq!(read_json(&mut admin)[2], true);
    let create_child = signed_event(
        90,
        now(),
        9_007,
        vec![Tag::new(vec!["h".into(), "child-room".into()])],
        "",
    );
    send_json(&mut admin, json!(["EVENT", create_child]));
    assert_eq!(read_json(&mut admin)[2], true);
    let attach_child = signed_event(
        90,
        now(),
        9_002,
        vec![
            Tag::new(vec!["h".into(), "child-room".into()]),
            Tag::new(vec!["parent".into(), "parent-room".into()]),
            Tag::new(vec!["restricted".into()]),
        ],
        "",
    );
    send_json(&mut admin, json!(["EVENT", attach_child]));
    assert_eq!(
        read_json(&mut admin)[2],
        true,
        "subgroup parent link stores"
    );
    let cycle = signed_event(
        90,
        now(),
        9_002,
        vec![
            Tag::new(vec!["h".into(), "parent-room".into()]),
            Tag::new(vec!["parent".into(), "child-room".into()]),
            Tag::new(vec!["child".into(), "child-room".into()]),
            Tag::new(vec!["restricted".into()]),
        ],
        "",
    );
    send_json(&mut admin, json!(["EVENT", cycle]));
    assert_eq!(read_json(&mut admin)[2], false, "a parent cycle is refused");

    stranger.close(None).unwrap();
    admin.close(None).unwrap();
    member.close(None).unwrap();
    joiner.close(None).unwrap();
}

fn expiration_contract(address_one: SocketAddr, address_two: SocketAddr) -> String {
    let mut publisher = connect_client(address_one);
    let challenge = expect_auth_challenge(&mut publisher);
    authenticate(&mut publisher, 21, &challenge);
    let expires_at = now() + 2;
    let event = signed_event(
        21,
        now(),
        1,
        vec![Tag::new(vec!["expiration".into(), expires_at.to_string()])],
        "swept",
    );
    send_json(&mut publisher, json!(["EVENT", event]));
    assert_eq!(read_json(&mut publisher)[2], true);

    std::thread::sleep(Duration::from_secs(4));
    let mut reader = connect_client(address_two);
    let challenge = expect_auth_challenge(&mut reader);
    authenticate(&mut reader, 20, &challenge);
    send_json(&mut reader, json!(["REQ", "expired", {"ids": [event.id]}]));
    assert_eq!(read_json(&mut reader), json!(["EOSE", "expired"]));
    reader.close(None).unwrap();
    publisher.close(None).unwrap();
    event.id
}

async fn assert_physically_expired(database_url: &str, event_id: &str) {
    let (client, connection) = tokio_postgres::connect(database_url, NoTls).await.unwrap();
    let driver = tokio::spawn(connection);
    let statement = client
        .prepare("SELECT count(*) FROM nostr_event WHERE id = $1")
        .await
        .unwrap();
    let count = client
        .query_one(&statement, &[&event_id])
        .await
        .unwrap()
        .get::<_, i64>(0);
    assert_eq!(count, 0, "NIP-40 sweeper must physically delete expiration");
    drop(client);
    driver.await.unwrap().unwrap();
}

fn rpc_request(address: SocketAddr, method: &str, params: Value) -> Value {
    let body = json!({ "method": method, "params": params }).to_string();
    let payload_hash = hex(&sha2::Sha256::digest(body.as_bytes()));
    let event = signed_event(
        91,
        now(),
        27_235,
        vec![
            Tag::new(vec!["u".into(), "http://relay.test/manage".into()]),
            Tag::new(vec!["method".into(), "POST".into()]),
            Tag::new(vec!["payload".into(), payload_hash]),
        ],
        "",
    );
    let authorization = base64(&serde_json::to_vec(&event).unwrap());
    let mut stream = StdTcpStream::connect(address).unwrap();
    write!(
        stream,
        "POST /manage HTTP/1.1\r\nHost: relay.test\r\nContent-Type: application/nostr+json+rpc\r\nAuthorization: Nostr {authorization}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"), "{response}");
    serde_json::from_str(response.split("\r\n\r\n").nth(1).unwrap()).unwrap()
}

fn subscription_limit_contract(address: SocketAddr) {
    let mut websocket = connect_client(address);
    let challenge = expect_auth_challenge(&mut websocket);
    authenticate(&mut websocket, 24, &challenge);
    for sequence in 0..8 {
        let subscription_id = format!("limit-{sequence}");
        send_json(
            &mut websocket,
            json!(["REQ", subscription_id, {"ids": ["f".repeat(64)]}]),
        );
        assert_eq!(read_json(&mut websocket)[0], "EOSE");
    }
    send_json(
        &mut websocket,
        json!(["REQ", "limit-refused", {"ids": ["f".repeat(64)]}]),
    );
    let response = read_json(&mut websocket);
    assert_eq!(response[0], "CLOSED");
    assert!(response[2].as_str().unwrap().starts_with("restricted:"));
    websocket.close(None).unwrap();
}

fn oversized_frame_contract(address: SocketAddr) {
    let mut websocket = connect_client(address);
    expect_auth_challenge(&mut websocket);
    websocket.send(Message::text("x".repeat(131_073))).unwrap();
    match websocket.read() {
        Ok(Message::Close(_))
        | Err(tokio_tungstenite::tungstenite::Error::ConnectionClosed)
        | Err(tokio_tungstenite::tungstenite::Error::Protocol(_)) => {}
        other => panic!("oversized frame did not close the connection: {other:?}"),
    }
}

fn connect_client(address: SocketAddr) -> WebSocket<StdTcpStream> {
    let stream = StdTcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let (websocket, _) = client(format!("ws://{address}/any/path"), stream).unwrap();
    websocket
}

fn expect_auth_challenge(websocket: &mut WebSocket<StdTcpStream>) -> String {
    let message = read_json(websocket);
    assert_eq!(message[0], "AUTH");
    message[1].as_str().unwrap().to_owned()
}

fn authenticate(websocket: &mut WebSocket<StdTcpStream>, secret: u8, challenge: &str) -> Event {
    let event = signed_event(
        secret,
        now(),
        22_242,
        vec![
            Tag::new(vec!["relay".into(), "ws://relay.test".into()]),
            Tag::new(vec!["challenge".into(), challenge.to_owned()]),
        ],
        "",
    );
    send_json(websocket, json!(["AUTH", event]));
    let response = read_json(websocket);
    assert_eq!(response[0], "OK");
    assert_eq!(response[2], true);
    event
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

fn assert_no_message(websocket: &mut WebSocket<StdTcpStream>) {
    websocket
        .get_mut()
        .set_read_timeout(Some(Duration::from_millis(200)))
        .unwrap();
    match websocket.read() {
        Err(tokio_tungstenite::tungstenite::Error::Io(error))
            if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
        other => panic!("expected no WebSocket message, got {other:?}"),
    }
    websocket
        .get_mut()
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
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

fn pubkey(secret_byte: u8) -> String {
    let secp = Secp256k1::new();
    let secret = SecretKey::from_byte_array([secret_byte; 32]).unwrap();
    Keypair::from_secret_key(&secp, &secret)
        .x_only_public_key()
        .0
        .to_string()
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::new();
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        output.push(char::from(TABLE[usize::from(first >> 2)]));
        output.push(char::from(
            TABLE[usize::from((first & 0x03) << 4 | second >> 4)],
        ));
        if chunk.len() > 1 {
            output.push(char::from(
                TABLE[usize::from((second & 0x0f) << 2 | third >> 6)],
            ));
        } else {
            output.push('=');
        }
        if chunk.len() > 2 {
            output.push(char::from(TABLE[usize::from(third & 0x3f)]));
        } else {
            output.push('=');
        }
    }
    output
}

fn fake_nip44_v2() -> String {
    let mut bytes = [0_u8; 99];
    bytes[0] = 0x02;
    base64(&bytes)
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
