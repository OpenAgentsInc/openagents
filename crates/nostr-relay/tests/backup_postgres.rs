//! The database-and-media backup unit against a live relay: uploads and
//! deletions run while `deploy/backup/nostr-relay-backup` works, the unit
//! restores into an empty database and media root through
//! `deploy/backup/nostr-relay-restore`, and a relay serving the restore
//! answers for every blob the dump references. A media root that lost a
//! referenced blob produces no manifest.

use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use nostr_relay::{
    domain::{Event, RelaySigner, Tag},
    gateway::{Gateway, GatewayConfig, MediaConfig},
};
use secp256k1::{Keypair, Secp256k1, SecretKey};
use sha2::{Digest, Sha256};
use tokio::time::timeout;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn backup_unit_restores_whole_under_concurrent_mutation() {
    let Ok(database_url) = std::env::var("NOSTR_RELAY_TEST_DATABASE_URL") else {
        eprintln!("skipped: run scripts/test-postgres.sh");
        return;
    };
    let Ok(restore_database_url) = std::env::var("NOSTR_RELAY_TEST_RESTORE_DATABASE_URL") else {
        eprintln!("skipped: run scripts/test-postgres.sh");
        return;
    };
    if std::env::var("NOSTR_RELAY_TEST_ALLOW_DESTRUCTIVE").as_deref() != Ok("1") {
        eprintln!("skipped: live backup suite requires a disposable database guard");
        return;
    }

    let scratch = scratch_dir();
    let media_root = scratch.join("media");
    let backup_dir = scratch.join("backups");
    std::fs::create_dir_all(&media_root).unwrap();
    std::fs::create_dir_all(&backup_dir).unwrap();

    let gateway = Gateway::start(test_config(database_url.clone(), media_root.clone()))
        .await
        .unwrap();
    let address = gateway.local_addr();
    let stop = gateway.shutdown_handle();
    let server = tokio::spawn(gateway.run());

    // Blobs that exist before the dump and must survive into the restore,
    // one of which is deleted while the backup runs and so must ride along
    // under `.deleted/`.
    let kept = (0..4u8)
        .map(|index| upload(address, 40, &format!("kept blob {index}").repeat(3)))
        .collect::<Vec<_>>();
    let deleted_during = upload(address, 40, "deleted while the backup runs");

    let backup_done = Arc::new(AtomicBool::new(false));
    let churn_done = backup_done.clone();
    let churn_victim = deleted_during.clone();
    let churn = std::thread::spawn(move || {
        let mut created = Vec::new();
        let mut round = 0u32;
        while !churn_done.load(Ordering::Acquire) {
            let content = format!("churn {round}");
            created.push(upload(address, 41, &content));
            if round == 0 {
                delete(address, 40, &churn_victim);
            }
            if round % 3 == 2 {
                let victim = created.remove(0);
                delete(address, 41, &victim);
            }
            round += 1;
            std::thread::sleep(Duration::from_millis(10));
        }
        round
    });

    let backup = tokio::task::spawn_blocking({
        let backup_dir = backup_dir.clone();
        let media_root = media_root.clone();
        let database_url = database_url.clone();
        move || run_backup(&backup_dir, &media_root, &database_url)
    })
    .await
    .unwrap();
    backup_done.store(true, Ordering::Release);
    let rounds = churn.join().unwrap();
    assert!(
        rounds >= 1,
        "the churn ran {rounds} rounds during the backup"
    );
    assert!(
        backup.status.success(),
        "backup failed: {}",
        String::from_utf8_lossy(&backup.stderr)
    );
    let manifest = single_manifest(&backup_dir);
    let manifest_text = std::fs::read_to_string(&manifest).unwrap();
    eprintln!("churn rounds: {rounds}\n{manifest_text}");
    assert!(manifest_text.contains("\ndatabase\t"));
    assert!(manifest_text.contains("\nmedia\t"));

    // Restore into an empty database and media root; the script refuses
    // an incomplete pair and checks every referenced blob for bytes.
    let restore_root = scratch.join("restored-media");
    let restore = Command::new(script("nostr-relay-restore"))
        .arg(&manifest)
        .arg(&restore_database_url)
        .arg(&restore_root)
        .output()
        .unwrap();
    assert!(
        restore.status.success(),
        "restore failed: {}",
        String::from_utf8_lossy(&restore.stderr)
    );

    let restored = Gateway::start(test_config(restore_database_url, restore_root.clone()))
        .await
        .unwrap();
    let restored_address = restored.local_addr();
    let restored_stop = restored.shutdown_handle();
    let restored_server = tokio::spawn(restored.run());
    let kept_for_check = kept.clone();
    tokio::task::spawn_blocking(move || {
        for (sha256, body) in &kept_for_check {
            let response = get(restored_address, sha256);
            assert!(
                response.starts_with(b"HTTP/1.1 200 OK\r\n"),
                "kept blob {sha256} missing after restore"
            );
            assert_eq!(http_body(&response), body.as_slice());
        }
        // The blob deleted during the backup is either still referenced
        // by the dump, and then served whole from the bytes the archive
        // retained, or gone from both; never referenced without bytes.
        let response = get(restored_address, &deleted_during.0);
        if response.starts_with(b"HTTP/1.1 200 OK\r\n") {
            assert_eq!(http_body(&response), deleted_during.1.as_slice());
        } else {
            assert!(
                response.starts_with(b"HTTP/1.1 404 Not Found\r\n"),
                "deleted-during blob answered {:?}",
                String::from_utf8_lossy(&response[..response.len().min(40)])
            );
        }
    })
    .await
    .unwrap();
    restored_stop.shutdown();
    timeout(Duration::from_secs(5), restored_server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();

    // A media root that lost a referenced blob outside the relay's own
    // deletion path cannot produce a manifest, and leaves no dump or
    // archive that could pass for one.
    let (lost, _) = &kept[0];
    std::fs::remove_file(blob_path(&media_root, lost)).unwrap();
    let failing_dir = scratch.join("failing-backups");
    std::fs::create_dir_all(&failing_dir).unwrap();
    let failed = run_backup(&failing_dir, &media_root, &database_url);
    assert!(!failed.status.success());
    let stderr = String::from_utf8_lossy(&failed.stderr);
    assert!(stderr.contains(lost), "{stderr}");
    assert!(
        std::fs::read_dir(&failing_dir).unwrap().next().is_none(),
        "a failed backup left files behind"
    );

    stop.shutdown();
    timeout(Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    std::fs::remove_dir_all(scratch).unwrap();
}

fn run_backup(backup_dir: &Path, media_root: &Path, database_url: &str) -> std::process::Output {
    Command::new(script("nostr-relay-backup"))
        .env("NOSTR_RELAY_BACKUP_DIR", backup_dir)
        .env("NOSTR_RELAY_MEDIA_ROOT", media_root)
        .env("NOSTR_RELAY_BACKUP_DATABASE", database_url)
        .output()
        .unwrap()
}

fn script(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../deploy/backup")
        .join(name)
}

fn single_manifest(backup_dir: &Path) -> PathBuf {
    let mut manifests = std::fs::read_dir(backup_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "manifest"))
        .collect::<Vec<_>>();
    assert_eq!(manifests.len(), 1, "{manifests:?}");
    manifests.remove(0)
}

fn blob_path(media_root: &Path, sha256: &str) -> PathBuf {
    let dir = media_root.join(&sha256[..2]);
    std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(sha256))
        })
        .unwrap_or_else(|| panic!("no blob file for {sha256} in {}", dir.display()))
}

fn test_config(database_url: String, media_root: PathBuf) -> GatewayConfig {
    let mut config = GatewayConfig::new(database_url, "127.0.0.1:0".parse().unwrap());
    config.relay_url = Some("ws://relay.test".to_owned());
    config.db_connections = 2;
    config.shutdown_grace = Duration::from_secs(2);
    config.relay_signer = Some(RelaySigner::from_secret_hex(&hex(&[90; 32])).unwrap());
    config.media = Some(MediaConfig {
        root: media_root,
        cloud_base_url: None,
        max_blob_bytes: 4_096,
        max_bytes_per_pubkey: 1 << 20,
    });
    config.limits.media_per_minute_ip = 10_000;
    config.limits.media_per_minute_pubkey = 10_000;
    config.limits.max_connections_per_ip = 100;
    config
}

fn upload(address: SocketAddr, secret_byte: u8, content: &str) -> (String, Vec<u8>) {
    let body = content.as_bytes().to_vec();
    let sha256 = hex(&Sha256::digest(&body));
    let auth = signed_event(
        secret_byte,
        now(),
        27_235,
        vec![
            Tag::new(vec!["u".into(), "http://relay.test/upload".into()]),
            Tag::new(vec!["method".into(), "PUT".into()]),
            Tag::new(vec!["payload".into(), sha256.clone()]),
        ],
        content,
    );
    let authorization = base64(&serde_json::to_vec(&auth).unwrap());
    let response = raw_http(
        address,
        &format!(
            "PUT /upload HTTP/1.1\r\nHost: relay.test\r\nContent-Type: text/plain\r\nAuthorization: Nostr {authorization}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        ),
        &body,
    );
    assert!(
        response.starts_with(b"HTTP/1.1 201 Created\r\n")
            || response.starts_with(b"HTTP/1.1 200 OK\r\n"),
        "{}",
        String::from_utf8_lossy(&response)
    );
    (sha256, body)
}

fn delete(address: SocketAddr, secret_byte: u8, blob: &(String, Vec<u8>)) {
    let path = format!("/{}", blob.0);
    let auth = signed_event(
        secret_byte,
        now(),
        27_235,
        vec![
            Tag::new(vec!["u".into(), format!("http://relay.test{path}")]),
            Tag::new(vec!["method".into(), "DELETE".into()]),
        ],
        &blob.0,
    );
    let authorization = base64(&serde_json::to_vec(&auth).unwrap());
    let response = raw_http(
        address,
        &format!(
            "DELETE {path} HTTP/1.1\r\nHost: relay.test\r\nAuthorization: Nostr {authorization}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        ),
        &[],
    );
    assert!(
        response.starts_with(b"HTTP/1.1 200 OK\r\n"),
        "{}",
        String::from_utf8_lossy(&response)
    );
}

fn get(address: SocketAddr, sha256: &str) -> Vec<u8> {
    raw_http(
        address,
        &format!("GET /{sha256} HTTP/1.1\r\nHost: relay.test\r\nConnection: close\r\n\r\n"),
        &[],
    )
}

fn raw_http(address: SocketAddr, head: &str, body: &[u8]) -> Vec<u8> {
    let mut stream = TcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream.write_all(head.as_bytes()).unwrap();
    stream.write_all(body).unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    response
}

fn http_body(response: &[u8]) -> &[u8] {
    let boundary = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .unwrap();
    &response[boundary + 4..]
}

fn scratch_dir() -> PathBuf {
    std::env::temp_dir().join(format!(
        "nostr-relay-backup-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
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

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
