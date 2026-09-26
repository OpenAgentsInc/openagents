//! NIP-RS over actual HTTP and a disposable writer database.
//! Run through `scripts/test-postgres.sh`; this suite deliberately corrupts
//! retained rows to verify that the relay never calls an incomplete cut complete.

use std::{
    net::SocketAddr,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use nostr::read_state_snapshot::{ReadStateSnapshot, SnapshotDescriptor, parse_snapshot};
use nostr_relay::{
    domain::{Event, RelaySigner, Tag},
    gateway::{Gateway, GatewayConfig, GatewayError},
    store::{AdmissionOutcome, Store},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};
use tokio_postgres::{Client, NoTls};

const COMMUNITY: &str = "00000000-0000-0000-0000-000000000001";
const HOST: &str = "relay.test";
static NONCE: AtomicU64 = AtomicU64::new(0);

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn complete_private_read_state_over_http() {
    let Ok(database_url) = std::env::var("NOSTR_RELAY_TEST_DATABASE_URL") else {
        eprintln!("skipped: run scripts/test-postgres.sh");
        return;
    };
    if std::env::var("NOSTR_RELAY_TEST_ALLOW_DESTRUCTIVE").as_deref() != Ok("1") {
        eprintln!("skipped: snapshot suite requires a disposable database guard");
        return;
    }

    let gateway = Gateway::start(config(&database_url)).await.unwrap();
    let address = gateway.local_addr();
    let stop = gateway.shutdown_handle();
    let server = tokio::spawn(gateway.run());
    let mut store = Store::connect(&database_url).await.unwrap();
    let (database, connection) = tokio_postgres::connect(&database_url, NoTls).await.unwrap();
    let driver = tokio::spawn(connection);
    let owner = signer(17);
    let stranger = signer(18);
    let descriptor = SnapshotDescriptor::new(COMMUNITY).unwrap();

    // RS discovery and reads do not require a relay signing key. The origin,
    // rather than a caller-provided community ID, chooses the community.
    let discovery = http(address, "GET", HOST, "", None).await;
    assert_eq!(discovery.status, 200);
    assert_eq!(
        discovery.json()["read_state_snapshot"],
        serde_json::to_value(&descriptor).unwrap()
    );
    let foreign = http(address, "GET", "other.test", "", None).await;
    assert_eq!(foreign.status, 200);
    assert!(foreign.json().get("read_state_snapshot").is_none());

    let earlier = now() - 60;
    let mut events = Vec::new();
    for coordinate in ["cursor", "unrelated-app", "unread"] {
        let event = state(&owner, coordinate, earlier, "private cursor state");
        stored(&mut store, &event).await;
        events.push(event);
    }
    let other = state(&stranger, "cursor", earlier, "other private state");
    stored(&mut store, &other).await;
    let body = request(&owner);
    let auth = authorization(&owner, &body);
    let first = http(address, "POST", HOST, &body, Some(&auth)).await;
    let first = snapshot(first, &descriptor, &owner);
    assert_eq!(first.events.len(), 3, "ordinary query limit is one");
    assert!(events.iter().all(|event| first.events.contains(event)));
    assert!(!first.events.contains(&other));
    assert_eq!(first.community_id, COMMUNITY);
    assert_eq!(
        snapshot(query(address, &owner, &body).await, &descriptor, &owner).snapshot_id,
        first.snapshot_id,
        "an unchanged writer cut has a stable identity"
    );

    refused(http(address, "POST", HOST, &body, Some(&auth)).await, 409);
    refused(http(address, "POST", HOST, &body, None).await, 401);
    let wrong_payload = authorization(&owner, "[]");
    refused(
        http(address, "POST", HOST, &body, Some(&wrong_payload)).await,
        401,
    );
    let foreign_auth = authorization(&owner, &body);
    refused(
        http(address, "POST", "other.test", &body, Some(&foreign_auth)).await,
        404,
    );
    refused(query(address, &stranger, &body).await, 400);
    let stranger_snapshot = snapshot(
        query(address, &stranger, &request(&stranger)).await,
        &descriptor,
        &stranger,
    );
    assert_eq!(stranger_snapshot.events, vec![other]);

    malformed_requests(address, &owner).await;

    let replacement = state(&owner, "cursor", earlier + 1, "advanced cursor");
    stored(&mut store, &replacement).await;
    let replaced = snapshot(query(address, &owner, &body).await, &descriptor, &owner);
    assert_eq!(replaced.events.len(), 3);
    assert!(replaced.events.contains(&replacement));
    assert!(!replaced.events.contains(&events[0]));
    assert_ne!(replaced.snapshot_id, first.snapshot_id);
    let deletion = owner.sign(
        earlier + 2,
        5,
        vec![Tag::new(vec![
            "a".into(),
            format!("30078:{}:unread", owner.pubkey()),
        ])],
        String::new(),
    );
    stored(&mut store, &deletion).await;
    let deleted = snapshot(query(address, &owner, &body).await, &descriptor, &owner);
    assert_eq!(deleted.events.len(), 2);
    assert!(!deleted.events.contains(&events[2]));
    assert_ne!(deleted.snapshot_id, replaced.snapshot_id);

    retained_expiration(address, &database, &mut store, &owner, &stranger).await;
    membership_changes(address, &database, &owner, &stranger).await;
    snapshot_limits(address, &database).await;

    // The consumed authorization is durable across gateway restarts.
    let replay = authorization(&owner, &body);
    snapshot(
        http(address, "POST", HOST, &body, Some(&replay)).await,
        &descriptor,
        &owner,
    );
    stop.shutdown();
    timeout(Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let gateway = Gateway::start(config(&database_url)).await.unwrap();
    let address = gateway.local_addr();
    let stop = gateway.shutdown_handle();
    let server = tokio::spawn(gateway.run());
    refused(http(address, "POST", HOST, &body, Some(&replay)).await, 409);
    snapshot(query(address, &owner, &body).await, &descriptor, &owner);
    stop.shutdown();
    timeout(Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    corrupt_retained_rows(&database_url, &database, &owner, &replacement).await;
    drop(store);
    drop(database);
    driver.await.unwrap().unwrap();
}

async fn retained_expiration(
    address: SocketAddr,
    database: &Client,
    store: &mut Store,
    owner: &RelaySigner,
    stranger: &RelaySigner,
) {
    let descriptor = SnapshotDescriptor::new(COMMUNITY).unwrap();
    let body = request(owner);
    let baseline = snapshot(query(address, owner, &body).await, &descriptor, owner);
    let expiration = now() + 2;
    let event = owner.sign(
        now(),
        30078,
        vec![
            Tag::new(vec!["d".into(), "expiring-coordinate".into()]),
            Tag::new(vec!["expiration".into(), expiration.to_string()]),
        ],
        "retained state before the physical sweep".into(),
    );
    stored(store, &event).await;
    let before = snapshot(query(address, owner, &body).await, &descriptor, owner);
    assert!(before.events.contains(&event));
    timeout(Duration::from_secs(3), async {
        while now() < expiration {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();

    refused(query(address, owner, &body).await, 503);
    assert!(
        database
            .query_one(
                "SELECT EXISTS (SELECT 1 FROM nostr_event WHERE id = $1)",
                &[&event.id]
            )
            .await
            .unwrap()
            .get::<_, bool>(0),
        "the expired coordinate remains physically stored"
    );
    snapshot(
        query(address, stranger, &request(stranger)).await,
        &descriptor,
        stranger,
    );

    assert_eq!(store.delete_expired(now()).await.unwrap(), 1);
    let after = snapshot(query(address, owner, &body).await, &descriptor, owner);
    assert_eq!(after.events, baseline.events);
    assert_eq!(after.snapshot_id, baseline.snapshot_id);
}

async fn malformed_requests(address: SocketAddr, owner: &RelaySigner) {
    let filter = json!({"read_state_snapshot":1,"authors":[owner.pubkey()],"kinds":[30078]});
    for malformed in [
        json!([{"read_state_snapshot":false,"authors":[owner.pubkey()],"kinds":[30078]}]),
        json!([{"read_state_snapshot":2,"authors":[owner.pubkey()],"kinds":[30078]}]),
        json!([{"read_state_snapshot":1,"authors":[owner.pubkey()],"kinds":[30078],"limit":1}]),
        json!([{"read_state_snapshot":1,"authors":[owner.pubkey()],"kinds":[30078],"#t":["read-state"]}]),
        json!([{"read_state_snapshot":1,"authors":[owner.pubkey()],"kinds":[78]}]),
        json!([filter.clone(), filter.clone()]),
        json!([filter.clone(), {"kinds":[1]}]),
        filter,
    ] {
        refused(query(address, owner, &malformed.to_string()).await, 400);
    }
    let duplicate_key = format!(
        "[{{\"read_state_snapshot\":1,\"read_state_snapshot\":1,\"authors\":[\"{}\"],\"kinds\":[30078]}}]",
        owner.pubkey()
    );
    refused(query(address, owner, &duplicate_key).await, 400);
}

async fn corrupt_retained_rows(
    database_url: &str,
    database: &Client,
    owner: &RelaySigner,
    event: &Event,
) {
    let tags = serde_json::to_string(&event.tags).unwrap();
    let duplicate = state(
        owner,
        "cursor",
        event.created_at + 1,
        "conflicting retained head",
    );
    for defect in ["signature", "tags", "duplicate"] {
        match defect {
            "signature" => {
                database
                    .execute(
                        "UPDATE nostr_event SET sig = repeat('0',128) WHERE id = $1",
                        &[&event.id],
                    )
                    .await
                    .unwrap();
            }
            "tags" => {
                database
                    .execute(
                        "UPDATE nostr_event SET tags = '[1]'::jsonb WHERE id = $1",
                        &[&event.id],
                    )
                    .await
                    .unwrap();
            }
            _ => insert_raw(database, &duplicate, "cursor").await,
        }
        // A corrupt stored event deliberately stops the relay's database pool.
        // Each case gets its own gateway, and no invalid state is skipped.
        let gateway = Gateway::start(config(database_url)).await.unwrap();
        let address = gateway.local_addr();
        let stop = gateway.shutdown_handle();
        let server = tokio::spawn(gateway.run());
        refused(query(address, owner, &request(owner)).await, 503);
        if defect == "duplicate" {
            stop.shutdown();
            timeout(Duration::from_secs(5), server)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
        } else {
            let outcome = timeout(Duration::from_secs(5), server)
                .await
                .unwrap()
                .unwrap();
            assert!(
                matches!(outcome, Err(GatewayError::Internal(_))),
                "{outcome:?}"
            );
        }
        database
            .execute(
                "UPDATE nostr_event SET sig = $2, tags = $3::text::jsonb WHERE id = $1",
                &[&event.id, &event.sig, &tags],
            )
            .await
            .unwrap();
        database
            .execute("DELETE FROM nostr_event WHERE id = $1", &[&duplicate.id])
            .await
            .unwrap();
    }
}

async fn membership_changes(
    address: SocketAddr,
    database: &Client,
    owner: &RelaySigner,
    stranger: &RelaySigner,
) {
    let body = request(owner);
    database
        .execute(
            "UPDATE relay_policy SET closed_membership = TRUE WHERE singleton",
            &[],
        )
        .await
        .unwrap();
    refused(query(address, owner, &body).await, 403);
    database
        .execute(
            "INSERT INTO relay_member_pubkey(pubkey,note) VALUES ($1,'snapshot fixture')",
            &[&owner.pubkey()],
        )
        .await
        .unwrap();
    assert_eq!(query(address, owner, &body).await.status, 200);
    database
        .execute(
            "INSERT INTO relay_blocked_pubkey(pubkey,reason) VALUES ($1,'snapshot fixture')",
            &[&owner.pubkey()],
        )
        .await
        .unwrap();
    refused(query(address, owner, &body).await, 403);
    database
        .execute(
            "DELETE FROM relay_blocked_pubkey WHERE pubkey = $1",
            &[&owner.pubkey()],
        )
        .await
        .unwrap();
    database
        .execute(
            "INSERT INTO relay_allowed_pubkey(pubkey,reason) VALUES ($1,'snapshot fixture')",
            &[&stranger.pubkey()],
        )
        .await
        .unwrap();
    refused(query(address, owner, &body).await, 403);
    database
        .execute(
            "DELETE FROM relay_allowed_pubkey WHERE pubkey = $1",
            &[&stranger.pubkey()],
        )
        .await
        .unwrap();
    database
        .execute(
            "UPDATE relay_policy SET closed_membership = FALSE WHERE singleton",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(query(address, owner, &body).await.status, 200);
}

async fn snapshot_limits(address: SocketAddr, database: &Client) {
    let owner = signer(19);
    let body = request(&owner);
    // Count admission precedes event decoding. These invalid signatures must
    // produce a count-limit refusal, not an apparently complete truncated cut.
    database.execute(
        "INSERT INTO nostr_event (id,pubkey,created_at,kind,tags,content,sig,replacement_identifier) \
         SELECT md5('snapshot-a-'||n)||md5('snapshot-b-'||n),$1,0,30078, \
         jsonb_build_array(jsonb_build_array('d','limit-'||n)),'',repeat('0',128),'limit-'||n \
         FROM generate_series(1,4097) AS n",
        &[&owner.pubkey()],
    ).await.unwrap();
    refused(query(address, &owner, &body).await, 413);
    database
        .execute(
            "DELETE FROM nostr_event WHERE pubkey = $1",
            &[&owner.pubkey()],
        )
        .await
        .unwrap();

    for bytes in [8_388_609_usize, 8_388_508] {
        // The second event has a valid signature and fits the stored content/tag
        // budget. Its compact event array exceeds the separate encoded budget.
        let event = state(&owner, "large", now() - 60, &"x".repeat(bytes));
        insert_raw(database, &event, "large").await;
        refused(query(address, &owner, &body).await, 413);
        database
            .execute("DELETE FROM nostr_event WHERE id = $1", &[&event.id])
            .await
            .unwrap();
    }
    let empty = snapshot(
        query(address, &owner, &body).await,
        &SnapshotDescriptor::new(COMMUNITY).unwrap(),
        &owner,
    );
    assert!(
        empty.events.is_empty(),
        "empty complete state remains valid"
    );
}

fn config(database_url: &str) -> GatewayConfig {
    let mut config = GatewayConfig::new(database_url.into(), "127.0.0.1:0".parse().unwrap());
    config.relay_url = Some(format!("ws://{HOST}"));
    config.read_state_community = Some(COMMUNITY.into());
    config.db_connections = 2;
    config.limits.max_limit = 1;
    config.shutdown_grace = Duration::from_secs(2);
    // The expiration fixture invokes the real sweep explicitly after refusal.
    config.expiration_sweep = Duration::from_secs(86_400);
    assert!(config.relay_signer.is_none());
    config
}

fn signer(byte: u8) -> RelaySigner {
    RelaySigner::from_secret_hex(&format!("{byte:02x}").repeat(32)).unwrap()
}

fn state(signer: &RelaySigner, coordinate: &str, timestamp: u64, content: &str) -> Event {
    signer.sign(
        timestamp,
        30078,
        vec![Tag::new(vec!["d".into(), coordinate.into()])],
        content.into(),
    )
}

async fn stored(store: &mut Store, event: &Event) {
    let outcome = store.admit(event, now()).await.unwrap();
    assert!(
        matches!(outcome, AdmissionOutcome::Stored { .. }),
        "{outcome:?}"
    );
}

async fn insert_raw(database: &Client, event: &Event, coordinate: &str) {
    let tags = serde_json::to_string(&event.tags).unwrap();
    let created_at = i64::try_from(event.created_at).unwrap();
    database.execute(
        "INSERT INTO nostr_event (id,pubkey,created_at,kind,tags,content,sig,replacement_identifier) \
         VALUES ($1,$2,$3,30078,$4::text::jsonb,$5,$6,$7)",
        &[&event.id,&event.pubkey,&created_at,&tags,&event.content,&event.sig,&coordinate],
    ).await.unwrap();
}

fn request(signer: &RelaySigner) -> String {
    json!([{"read_state_snapshot":1,"authors":[signer.pubkey()],"kinds":[30078]}]).to_string()
}

fn authorization(signer: &RelaySigner, body: &str) -> String {
    let digest = Sha256::digest(body.as_bytes());
    let payload = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let event = signer.sign(
        now(),
        27235,
        vec![
            Tag::new(vec!["u".into(), format!("http://{HOST}/query")]),
            Tag::new(vec!["method".into(), "POST".into()]),
            Tag::new(vec!["payload".into(), payload]),
            Tag::new(vec![
                "nonce".into(),
                NONCE.fetch_add(1, Ordering::Relaxed).to_string(),
            ]),
        ],
        String::new(),
    );
    format!("Nostr {}", base64(&serde_json::to_vec(&event).unwrap()))
}

struct Response {
    status: u16,
    body: String,
}

impl Response {
    fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap()
    }
}

async fn query(address: SocketAddr, signer: &RelaySigner, body: &str) -> Response {
    http(
        address,
        "POST",
        HOST,
        body,
        Some(&authorization(signer, body)),
    )
    .await
}

async fn http(
    address: SocketAddr,
    method: &str,
    host: &str,
    body: &str,
    auth: Option<&str>,
) -> Response {
    timeout(Duration::from_secs(10), async {
        let path = if method == "GET" { "/" } else { "/query" };
        let mut stream = TcpStream::connect(address).await.unwrap();
        let authorization = auth.map(|auth| format!("Authorization: {auth}\r\n")).unwrap_or_default();
        let wire = format!(
            "{method} {path} HTTP/1.1\r\nHost: {host}\r\nAccept: application/nostr+json\r\nContent-Type: application/json\r\n{authorization}Content-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()
        );
        stream.write_all(wire.as_bytes()).await.unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).await.unwrap();
        let (head, body) = response.split_once("\r\n\r\n").unwrap();
        Response {status: head.split_whitespace().nth(1).unwrap().parse().unwrap(), body: body.into()}
    }).await.expect("HTTP snapshot request timed out")
}

fn snapshot(
    response: Response,
    descriptor: &SnapshotDescriptor,
    owner: &RelaySigner,
) -> ReadStateSnapshot {
    assert_eq!(response.status, 200, "{}", response.body);
    parse_snapshot(response.body.as_bytes(), descriptor, owner.pubkey()).unwrap()
}

fn refused(response: Response, status: u16) {
    assert_eq!(response.status, status, "{}", response.body);
    let body = response.json();
    assert!(body.get("error").is_some());
    assert!(body.get("complete").is_none());
    assert!(body.get("events").is_none());
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
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
            TABLE[usize::from((first & 3) << 4 | second >> 4)],
        ));
        output.push(if chunk.len() > 1 {
            char::from(TABLE[usize::from((second & 15) << 2 | third >> 6)])
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            char::from(TABLE[usize::from(third & 63)])
        } else {
            '='
        });
    }
    output
}
