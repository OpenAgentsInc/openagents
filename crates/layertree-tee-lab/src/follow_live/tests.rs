use super::*;
use crate::{follow::Seen, light::LightError};
use nostr::domain::RelaySigner;
use std::{
    fs::OpenOptions,
    io::{Read, Write},
    net::TcpListener,
    sync::atomic::{AtomicU64, Ordering},
};
fn scratch(name: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "layertree-{name}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}
fn signer() -> RelaySigner {
    let key = bitcoin::secp256k1::SecretKey::new(&mut bitcoin::secp256k1::rand::thread_rng());
    RelaySigner::from_secret_hex(&hex(&key.secret_bytes())).unwrap()
}
#[test]
fn a_relay_cannot_relabel_the_author_kind_filename_or_body() {
    let author = signer();
    let name = "record-test.bin";
    let event = publish::note(&author, name, b"record bytes", now().unwrap());
    assert_eq!(
        mirror::decode_note(&event, name, &event.pubkey).unwrap(),
        b"record bytes"
    );
    assert!(mirror::decode_note(&event, "another-file", &event.pubkey).is_err());
    let other = publish::note(&signer(), name, b"record bytes", now().unwrap());
    assert!(mirror::decode_note(&other, name, &event.pubkey).is_err());
    let mut changed = event.clone();
    changed.kind = 30078;
    assert!(mirror::decode_note(&changed, name, &event.pubkey).is_err());
    let mut changed = event.clone();
    changed.content.push(' ');
    assert!(mirror::decode_note(&changed, name, &event.pubkey).is_err());
    let duplicate=author.sign(now().unwrap(),1,event.tags.clone(),format!("{{\"schema\":\"layertree.mirror.v1\",\"schema\":\"layertree.mirror.v1\",\"name\":\"{name}\",\"bytes\":\"00\"}}"));
    assert!(mirror::decode_note(&duplicate, name, &event.pubkey).is_err());
}
#[test]
fn mainnet_and_credential_bearing_endpoints_are_refused() {
    let cfg = scratch("bad-config").join("config.json");
    std::fs::write(&cfg,r#"{"schema":"layertree.follower.v1","network":"mainnet","rpc":"https://example.org","tree_open":"tree.jsonl","operator_key":"","unroll_key":"","unroll":"package.bin","member":0,"sources":{"directories":["mirror"]},"state":"state"}"#).unwrap();
    assert!(Config::load(&cfg).is_err());
    assert!(mirror::mirror_endpoint("http://10.128.0.119:18440", true).is_ok());
    assert!(mirror::mirror_endpoint("http://8.8.8.8", true).is_err());
    assert!(mirror::mirror_endpoint("http://private.example.org", true).is_err());
    assert!(chain::endpoint("https://user:password@example.org").is_err());
    assert!(chain::endpoint("https://example.org?token=unused").is_err());
    assert!(mirror::check_relay_url("wss://example.org?token=unused").is_err());
    assert!(mirror::check_relay_url("ws://example.org").is_err());
}
fn observation() -> Seen {
    Seen {
        height: 123,
        txid: None,
        result: Err(LightError::OutputMismatch),
    }
}
#[test]
fn unfinished_tail_and_duplicate_observations_recover_without_losing_an_alarm() {
    let dir = scratch("journal-tail");
    let mut journal =
        journal::Journal::open_configured(&dir, "regtest", "tree", 2, "binding", None).unwrap();
    journal.observe(&observation(), 123).unwrap();
    drop(journal);
    OpenOptions::new()
        .append(true)
        .open(dir.join("observations.jsonl"))
        .unwrap()
        .write_all(b"{unfinished")
        .unwrap();
    let mut journal =
        journal::Journal::open_configured(&dir, "regtest", "tree", 2, "binding", None).unwrap();
    assert_eq!(journal.pending(), 1);
    journal.observe(&observation(), 123).unwrap();
    drop(journal);
    assert_eq!(
        std::fs::read_to_string(dir.join("observations.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
    assert!(
        journal::Journal::open_configured(&dir, "regtest", "other-tree", 2, "other-binding", None)
            .is_err()
    );
}
#[test]
fn failed_webhooks_retry_after_restart_with_the_same_id_and_durable_ack() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = chain::endpoint(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
    let server = std::thread::spawn(move || {
        let mut ids = Vec::new();
        for code in [503, 200] {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut chunk = [0u8; 1024];
            let end = loop {
                let n = stream.read(&mut chunk).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&chunk[..n]);
                if let Some(p) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    break p + 4;
                }
            };
            let header = std::str::from_utf8(&bytes[..end])
                .unwrap()
                .to_ascii_lowercase();
            let len: usize = header
                .lines()
                .find_map(|l| l.strip_prefix("content-length:"))
                .unwrap()
                .trim()
                .parse()
                .unwrap();
            let id = header
                .lines()
                .find_map(|l| l.strip_prefix("idempotency-key:"))
                .unwrap()
                .trim()
                .to_owned();
            while bytes.len() < end + len {
                let n = stream.read(&mut chunk).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&chunk[..n]);
            }
            let payload: serde_json::Value =
                serde_json::from_slice(&bytes[end..end + len]).unwrap();
            assert_eq!(payload["id"], id);
            ids.push(id);
            write!(
                stream,
                "HTTP/1.1 {code} Test\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            )
            .unwrap();
        }
        ids
    });
    let dir = scratch("webhook");
    let mut journal = journal::Journal::open_configured(
        &dir,
        "regtest",
        "tree",
        2,
        "binding",
        Some(endpoint.clone()),
    )
    .unwrap();
    journal.observe(&observation(), 123).unwrap();
    journal.retry_notifications().unwrap();
    assert_eq!(journal.pending(), 1);
    drop(journal);
    let mut journal = journal::Journal::open_configured(
        &dir,
        "regtest",
        "tree",
        2,
        "binding",
        Some(endpoint.clone()),
    )
    .unwrap();
    journal.retry_notifications().unwrap();
    assert_eq!(journal.pending(), 0);
    drop(journal);
    let mut journal =
        journal::Journal::open_configured(&dir, "regtest", "tree", 2, "binding", Some(endpoint))
            .unwrap();
    journal.retry_notifications().unwrap();
    assert_eq!(journal.pending(), 0);
    let ids = server.join().unwrap();
    assert_eq!(ids.len(), 2);
    assert_eq!(ids[0], ids[1]);
}

#[test]
fn inline_files_bind_both_signatures_tree_record_hash_and_file_bytes() {
    use crate::{rules::Record, tree::Node};
    use base64::{Engine, engine::general_purpose::STANDARD};
    use bitcoin::hashes::{Hash, sha256};
    use nostr::domain::Tag;
    let author = signer();
    let tree = "42".repeat(32);
    let record = Record {
        prev: bitcoin::OutPoint::null(),
        changes: vec![],
        new_root: Node::EMPTY,
        next_unroll_key: None,
    };
    let payload = serde_json::to_vec(&json!({"v":"openagents.layertree.mirror.v1","tree_id":tree,"data":{"record":hex(&record.encode()),"unroll":null}})).unwrap();
    let tag = |name: &str, value: String| Tag::new(vec![name.into(), value]);
    let blob = author.sign(
        now().unwrap(),
        1064,
        vec![tag("type", "application/json".into())],
        STANDARD.encode(&payload),
    );
    let header = author.sign(
        now().unwrap(),
        1065,
        vec![
            tag("e", blob.id.clone()),
            tag("m", "application/json".into()),
            tag("x", sha256::Hash::hash(&payload).to_string()),
            tag("size", payload.len().to_string()),
            tag("t", format!("layertree:{tree}")),
            tag("r", hex(&record.hash())),
        ],
        "Signet tree recovery data.".into(),
    );
    let mut relay = mirror::Relay {
        inline_file: true,
        tree_id: Some(tree),
        auth_env: None,
        url: "ws://127.0.0.1:1".into(),
        publisher: blob.pubkey.clone(),
    };
    assert_eq!(
        inline_file::decode(&header, &blob, &relay, Some(record.hash())).unwrap(),
        (record.encode(), None)
    );
    assert!(inline_file::decode(&header, &blob, &relay, Some([0; 32])).is_err());
    relay.tree_id = Some("43".repeat(32));
    assert!(inline_file::decode(&header, &blob, &relay, Some(record.hash())).is_err());
    relay.tree_id = Some("42".repeat(32));
    let bad_blob = author.sign(
        now().unwrap(),
        1064,
        blob.tags.clone(),
        STANDARD.encode(b"another file"),
    );
    assert!(inline_file::decode(&header, &bad_blob, &relay, None).is_err());
    let mut bad_header = header.clone();
    bad_header.content.push('!');
    assert!(inline_file::decode(&bad_header, &blob, &relay, None).is_err());
    let bad_header = author.sign(
        now().unwrap(),
        1065,
        header
            .tags
            .iter()
            .cloned()
            .chain([tag("r", hex(&record.hash()))])
            .collect(),
        header.content.clone(),
    );
    assert!(inline_file::decode(&bad_header, &blob, &relay, None).is_err());
}
