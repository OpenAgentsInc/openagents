//! NIP-KB round trips: entry files to signed events, a sync's choices,
//! the cache, and trust. Keys are throwaway, derived from a label.

use std::path::{Path, PathBuf};

use nostr::domain::{Event, RelaySigner};
use nostr::kb;
use sha2::{Digest, Sha256};

use super::*;
use crate::lint::Corpus;
use crate::{Entry, Status};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("knowledge-remote-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn signer(label: &str) -> RelaySigner {
    let hex: String = Sha256::digest(label.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    RelaySigner::from_secret_hex(&hex).unwrap()
}

fn sign(signer: &RelaySigner, parts: kb::Unsigned, at: u64) -> Event {
    signer.sign(at, parts.kind, parts.tags, parts.content)
}

fn seed(id: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../knowledge/{id}.md"));
    std::fs::read_to_string(path).unwrap()
}

/// The seed entry `id` at `version`, with its body changed when the
/// version isn't 1.
fn version(id: &str, version: u32) -> String {
    let mut entry = Entry::parse(&seed(id)).unwrap();
    if version != 1 {
        entry.version = version;
        entry.body = format!("{}\n\nRevised in version {version}.", entry.body);
        return entry.render();
    }
    seed(id)
}

fn published(author: &RelaySigner, text: &str) -> Event {
    sign(author, entry_event(text).unwrap(), 1_790_000_000)
}

#[test]
fn an_entry_file_becomes_a_verified_event() {
    let author = signer("author");
    let event = published(&author, &seed("statistics.mmd-estimators"));
    let parsed = kb::parse_entry(&event).unwrap();
    assert_eq!(parsed.id, "statistics.mmd-estimators");
    assert_eq!(parsed.document, seed("statistics.mmd-estimators"));
    assert!(parsed.topics.contains(&"two-sample-test".to_string()));
    let sync = accept(&[event], &Corpus::default());
    assert_eq!(sync.accepted.len(), 1);
    assert!(sync.refused.is_empty(), "{:?}", sync.refused);
}

#[test]
fn a_sync_follows_heads_skips_withdrawals_and_refuses_conflicts() {
    let author = signer("author");
    let id = "shell.heredoc-quoting";
    let v1 = published(&author, &version(id, 1));
    let v2 = published(&author, &version(id, 2));
    // No head: the highest version.
    let sync = accept(&[v1.clone(), v2.clone()], &Corpus::default());
    assert_eq!(sync.accepted[0].version.version, 2);
    // A head naming version 1 wins.
    let head = sign(&author, kb::head(&v1).unwrap(), 1_790_000_100);
    let sync = accept(&[v1.clone(), v2.clone(), head], &Corpus::default());
    assert_eq!(sync.accepted[0].version.version, 1);
    // Version 2 withdrawn: back to version 1.
    let gone = sign(
        &author,
        kb::withdrawal(&v2, "wrong").unwrap(),
        1_790_000_200,
    );
    let sync = accept(&[v1.clone(), v2.clone(), gone.clone()], &Corpus::default());
    assert_eq!(sync.accepted[0].version.version, 1);
    // Both withdrawn: nothing, and the entry is reported withdrawn.
    let gone1 = sign(
        &author,
        kb::withdrawal(&v1, "wrong").unwrap(),
        1_790_000_300,
    );
    let sync = accept(&[v1.clone(), v2, gone, gone1], &Corpus::default());
    assert!(sync.accepted.is_empty());
    assert_eq!(
        sync.withdrawn,
        [(author.pubkey().to_string(), id.to_string())]
    );
    // Two documents for version 1: neither is picked.
    let other = version(id, 1).replace("## Details", "## Details\n\nEquivocated.");
    let twin = published(&author, &other);
    let sync = accept(&[v1, twin], &Corpus::default());
    assert!(sync.accepted.is_empty());
    assert!(
        sync.refused.iter().any(|r| r.contains("two documents")),
        "{:?}",
        sync.refused
    );
}

#[test]
fn a_sync_refuses_a_tampered_event_and_a_document_the_lint_refuses() {
    let author = signer("author");
    let mut tampered = published(&author, &seed("numerics.float-comparison"));
    tampered.content = tampered.content.replace("tolerance", "slack");
    let named = seed("numerics.cosine-zero-vector")
        .replace("## Details", "## Details\n\nSeen in drift-watch.");
    let sync = accept(
        &[tampered, published(&author, &named)],
        &Corpus {
            names: vec!["drift-watch".to_string()],
            ..Corpus::default()
        },
    );
    assert!(sync.accepted.is_empty());
    assert_eq!(sync.refused.len(), 2, "{:?}", sync.refused);
    assert!(sync.refused[1].contains("names the benchmark task"));
}

fn cache(dir: &Path, author: &RelaySigner, ids: &[&str]) {
    let events: Vec<Event> = ids.iter().map(|id| published(author, &seed(id))).collect();
    let sync = accept(&events, &Corpus::default());
    write_cache(dir, &sync).unwrap();
}

#[test]
fn the_cache_is_rechecked_when_it_loads() {
    let dir = scratch("cache");
    let author = signer("author");
    cache(&dir, &author, &["slip.one-fix-when-many-are-broken"]);
    let at = dir.join(author.pubkey());
    assert!(at.join("slip.one-fix-when-many-are-broken.md").exists());
    let (entries, problems) = read_cache(&dir);
    assert_eq!(entries.len(), 1);
    assert!(problems.is_empty());
    let file = at.join("slip.one-fix-when-many-are-broken.event.json");
    let text = std::fs::read_to_string(&file).unwrap();
    std::fs::write(&file, text.replace("admitted", "withdrawn")).unwrap();
    let (entries, problems) = read_cache(&dir);
    assert!(entries.is_empty());
    assert_eq!(problems.len(), 1);
}

#[test]
fn trust_decides_which_remote_entries_show_and_as_what() {
    let local = scratch("local");
    std::fs::write(
        local.join("shell.heredoc-quoting.md"),
        seed("shell.heredoc-quoting"),
    )
    .unwrap();
    let remote = scratch("remote-cache");
    let alice = signer("alice");
    let bob = signer("bob");
    // Both publish the seed entries, which are admitted; one shares a local ID.
    cache(
        &remote,
        &alice,
        &["statistics.psi-empty-bins", "shell.heredoc-quoting"],
    );
    cache(&remote, &bob, &["statistics.ks-two-sample"]);
    let ids = |base: &Base| -> Vec<(String, Status)> {
        base.entries
            .iter()
            .map(|e| (e.id.clone(), e.status))
            .collect()
    };
    let own = TrustConfig::default();
    // Own mode with no key of our own: local entries only.
    let (base, loaded) = load(&local, Some(&remote), &own, None, false).unwrap();
    assert_eq!(
        ids(&base),
        [("shell.heredoc-quoting".to_string(), Status::Admitted)]
    );
    assert!(loaded.remote.is_empty());
    // Alice is us: her entries keep their status; the local copy wins a clash.
    let (base, loaded) = load(&local, Some(&remote), &own, Some(alice.pubkey()), false).unwrap();
    assert_eq!(base.entries.len(), 2);
    assert_eq!(loaded.shadowed, 1);
    let psi = base.get("statistics.psi-empty-bins").unwrap();
    assert_eq!(psi.status, Status::Admitted);
    assert_eq!(psi.author, npub(alice.pubkey()));
    // Listed: Bob's entry keeps its status.
    let listed = TrustConfig {
        mode: Trust::Listed,
        authors: vec![bob.pubkey().to_string()],
    };
    let (base, _) = load(&local, Some(&remote), &listed, None, false).unwrap();
    assert_eq!(
        base.get("statistics.ks-two-sample").unwrap().status,
        Status::Admitted
    );
    assert!(base.get("statistics.psi-empty-bins").is_none());
    // All: unlisted authors are candidates, hidden unless candidates are asked for.
    let all = TrustConfig {
        mode: Trust::All,
        authors: Vec::new(),
    };
    let (base, _) = load(&local, Some(&remote), &all, None, false).unwrap();
    assert_eq!(base.entries.len(), 1);
    let (base, _) = load(&local, Some(&remote), &all, None, true).unwrap();
    assert_eq!(
        base.get("statistics.ks-two-sample").unwrap().status,
        Status::Candidate
    );
    assert_eq!(
        base.get("statistics.psi-empty-bins").unwrap().status,
        Status::Candidate
    );
}

#[test]
fn the_trust_file_and_author_keys_parse() {
    let dir = scratch("trust");
    let bob = signer("bob");
    let path = dir.join("trust.json");
    assert_eq!(TrustConfig::read(&path).unwrap(), TrustConfig::default());
    std::fs::write(
        &path,
        format!(
            r#"{{"mode": "listed", "authors": ["{}"]}}"#,
            npub(bob.pubkey())
        ),
    )
    .unwrap();
    let config = TrustConfig::read(&path).unwrap();
    assert_eq!(config.mode, Trust::Listed);
    assert_eq!(config.authors, [bob.pubkey().to_string()]);
    std::fs::write(&path, r#"{"mode": "everyone"}"#).unwrap();
    assert!(TrustConfig::read(&path).is_err());
    assert_eq!(parse_author("not a key"), None);
    let key = dir.join("knowledge-key");
    assert_eq!(own_pubkey(&key), None);
    let hex: String = Sha256::digest(b"bob")
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    std::fs::write(&key, format!("{hex}\n")).unwrap();
    assert_eq!(own_pubkey(&key).as_deref(), Some(bob.pubkey()));
}
