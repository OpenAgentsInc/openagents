use std::sync::Arc;

use super::*;
use crate::task::agent_key::FileKeys;

const OWNER: [u8; 32] = [7; 32];
const NOW: u64 = 1_800_000_000;

fn owner() -> SecretKey {
    SecretKey::from_byte_array(OWNER).unwrap()
}

fn run(repo: &Path, args: &[&str]) -> String {
    let output = super::super::local::git()
        .arg("-C")
        .arg(repo)
        .args(args)
        .env("GIT_AUTHOR_DATE", "1800000100 +0000")
        .env("GIT_COMMITTER_DATE", "1800000100 +0000")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// A repository under `dir` with one unsigned commit; returns its ID.
fn repository(dir: &Path) -> (std::path::PathBuf, String) {
    let repo = dir.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    run(&repo, &["init", "-q", "-b", "main"]);
    run(&repo, &["config", "user.name", "Studio Alice"]);
    run(&repo, &["config", "user.email", "alice@example.invalid"]);
    run(&repo, &["config", "commit.gpgsign", "false"]);
    std::fs::write(repo.join("notes.txt"), "alice\n").unwrap();
    run(&repo, &["add", "-A"]);
    run(&repo, &["commit", "-q", "-m", "Add notes\n\nWith a body."]);
    let head = run(&repo, &["rev-parse", "HEAD"]);
    (repo, head)
}

/// Alice under `root` with her own key, attested by [`OWNER`].
fn alice(root: &Path) -> Store {
    let store = Store::with_keys(root, "alice", Arc::new(FileKeys)).unwrap();
    let record = store.open(root, 1).unwrap();
    let record = store.ensure_key(record, 1).unwrap();
    store
        .attest(record, &owner(), NOW + 300 * 86_400, NOW)
        .unwrap();
    store
}

#[test]
fn a_signed_commit_verifies_with_her_key_and_the_owners_attestation() {
    let dir = tempfile::tempdir().unwrap();
    let (repo, head) = repository(dir.path());
    let store = alice(&dir.path().join("host"));
    let key = store.key().unwrap().unwrap();
    let record = store.load().unwrap().unwrap();

    let signed = sign_commit(&repo, &head, &key, record.attestation.as_ref()).unwrap();
    assert_ne!(signed, head);
    // The same tree, parents, and message; Git reads it as a commit.
    assert_eq!(
        run(&repo, &["rev-parse", &format!("{signed}^{{tree}}")]),
        run(&repo, &["rev-parse", &format!("{head}^{{tree}}")])
    );
    assert_eq!(
        run(&repo, &["log", "-1", "--format=%B", &signed]),
        "Add notes\n\nWith a body."
    );
    let verified = verify_commit(&repo, &signed).unwrap();
    assert_eq!(verified.pubkey, agent::public_hex(&key));
    assert_eq!(verified.timestamp, 1_800_000_100);
    assert_eq!(verified.owner_authorized, Some(true));
    // Signing again gives the same object, and a signed commit stays.
    assert_eq!(
        sign_commit(&repo, &head, &key, record.attestation.as_ref()).unwrap(),
        signed
    );
    assert_eq!(sign_commit(&repo, &signed, &key, None).unwrap(), signed);
    assert!(
        verify_commit(&repo, &head).is_err(),
        "the original is unsigned"
    );
}

#[test]
fn the_seat_signer_signs_only_for_an_agent_who_turned_it_on() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("host");
    let (repo, head) = repository(dir.path());
    let store = alice(&root);
    let signer = seat_signer(&root);
    assert!(signer("alice", &repo, &head).is_none(), "off by default");
    assert!(signer("bob", &repo, &head).is_none(), "not an agent");

    set(&store, true, NOW).unwrap();
    assert!(Settings::load(&store).unwrap().worktree_commits);
    let signed = signer("alice", &repo, &head).unwrap().unwrap();
    let verified = verify_commit(&repo, &signed).unwrap();
    assert_eq!(Some(verified.pubkey), store.load().unwrap().unwrap().pubkey);
    assert_eq!(verified.owner_authorized, Some(true));
    assert!(
        store
            .journal(20)
            .unwrap()
            .iter()
            .any(|e| e.text.starts_with("NIP-GS: signed her commit"))
    );

    // Without her key she can't sign, and the merge is refused.
    std::fs::remove_file(store.dir().join("key")).unwrap();
    assert!(signer("alice", &repo, &head).unwrap().is_err());
    set(&store, false, NOW + 1).unwrap();
    assert!(signer("alice", &repo, &head).is_none());
}
