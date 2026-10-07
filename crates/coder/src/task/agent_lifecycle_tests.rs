use std::collections::BTreeMap;
use std::sync::Arc;

use super::*;
use crate::task::agent_key::{FileKeys, KeyStore, MemoryKeys};
use crate::task::agent_memory::{Author, MemoryKind};
use crate::task::agent_sync::tests::Fake;

const OWNER: [u8; 32] = [7; 32];
const OTHER: [u8; 32] = [9; 32];
const NOW: u64 = 1_800_000_000;
const RELAY: &str = "wss://relay.example";

fn owner() -> SecretKey {
    SecretKey::from_byte_array(OWNER).unwrap()
}

fn screen() -> secret_screen::Screen {
    secret_screen::Screen::shapes()
}

/// `name` under `root` with her key in `keys`, attested by [`OWNER`], who
/// remembers two things.
fn agent(root: &Path, name: &str, keys: Arc<dyn KeyStore>) -> Memory {
    let store = Store::with_keys(root, name, keys).unwrap();
    let record = store.open(root, 1).unwrap();
    let record = store.ensure_key(record, 1).unwrap();
    store
        .attest(record, &owner(), NOW + 300 * 86_400, NOW)
        .unwrap();
    let memory = Memory::new(store, screen());
    for (i, text) in ["the build runs on the 4080", "she prefers small commits"]
        .iter()
        .enumerate()
    {
        memory
            .add(
                MemoryKind::Note,
                Author::Owner,
                text,
                Vec::new(),
                NOW + i as u64,
            )
            .unwrap();
    }
    memory
}

/// What the owner reads with the owner key: each live slug and its body.
fn owner_bodies(store: &Store) -> BTreeMap<String, Body> {
    let view = agent_engrams::owner_read(store, &owner()).unwrap();
    assert!(view.problems.is_empty(), "{:?}", view.problems);
    view.heads
        .iter()
        .filter(|head| head.slug().as_str() != agent_engrams::PERSONA_SLUG)
        .map(|head| (head.slug().as_str().to_string(), head.body.clone()))
        .collect()
}

fn journal(store: &Store) -> Vec<String> {
    store
        .journal(500)
        .unwrap()
        .into_iter()
        .map(|e| e.text)
        .collect()
}

#[test]
fn rotation_keeps_every_memory_readable_under_a_new_key_with_signed_lineage() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("host");
    let keychain = Arc::new(MemoryKeys::default());
    let memory = agent(&root, "alice", keychain.clone());
    let store = memory.store();
    let before = owner_bodies(store);
    assert!(
        before.contains_key("core") && before.len() >= 3,
        "{before:?}"
    );
    let old = store.load().unwrap().unwrap().pubkey.unwrap();
    let old_key = store.key().unwrap().unwrap();

    let rotated = rotate(
        store,
        &screen(),
        &owner(),
        "a routine rotation",
        NOW + 365 * 86_400,
        NOW + 10,
    )
    .unwrap();
    assert_eq!(rotated.old, old);
    assert_ne!(rotated.new, old);
    assert!(rotated.archive.is_none(), "sync is off");

    // Her record names the new key, attested afresh, and the keychain
    // holds it in place of the old one; the next slot is gone.
    let record = store.load().unwrap().unwrap();
    assert_eq!(record.pubkey.as_deref(), Some(rotated.new.as_str()));
    let attestation = record.attestation.clone().unwrap();
    assert_eq!(attestation.owner, agent::public_hex(&owner()));
    agent::verify_attestation(&rotated.new, &attestation, NOW + 10).unwrap();
    let key = store.key().unwrap().unwrap();
    assert_ne!(key, old_key);
    assert_eq!(agent::public_hex(&key), rotated.new);
    assert!(!keychain.holds("agent:alice.next"));
    assert!(!store.dir().join("next").exists());
    assert!(!store.dir().join("engrams.next").exists());
    assert!(!store.dir().join("engrams.old").exists());

    // Every memory reads the same with the owner key, and with hers.
    assert_eq!(owner_bodies(store), before);
    let Opened::Ready(engrams) = EngramStore::read(store, &screen()) else {
        panic!("her new store reads");
    };
    assert_eq!(engrams.pair().agent().to_string(), rotated.new);
    assert_eq!(
        memory.entries().unwrap().len(),
        2,
        "her working memory is untouched"
    );

    // The owner's lineage record links the two keys and verifies.
    let lines = lineage(store).unwrap();
    assert_eq!(lines, vec![rotated.lineage.clone()]);
    lines[0].verify().unwrap();
    assert_eq!(
        (lines[0].old.as_str(), lines[0].new.as_str()),
        (old.as_str(), rotated.new.as_str())
    );
    assert_eq!(lines[0].reason, "a routine rotation");
    let mut forged = lines[0].clone();
    forged.new = old.clone();
    forged.old = rotated.new.clone();
    assert!(forged.verify().is_err());

    // Her profile carries the new attestation, and the journal says
    // grants don't carry over.
    let profile = crate::task::agent_profile::load(store).unwrap().unwrap();
    assert_eq!(profile.pubkey, rotated.new);
    assert!(crate::task::agent_profile::current(&record, &profile));
    let said = journal(store);
    assert!(
        said.iter()
            .any(|t| t.starts_with("the owner rotated her key"))
    );
    assert!(said.iter().any(|t| t.contains("don't carry over")));
}

#[test]
fn rotation_refuses_another_owner_and_an_unreadable_store_and_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("host");
    let memory = agent(&root, "alice", Arc::new(FileKeys));
    let store = memory.store();
    let old = store.load().unwrap().unwrap();
    let old_key = store.key().unwrap().unwrap();

    let other = SecretKey::from_byte_array(OTHER).unwrap();
    let why = rotate(store, &screen(), &other, "", NOW + 86_400, NOW).unwrap_err();
    assert!(why.contains("isn't the one that attested her"), "{why}");

    // A head that doesn't verify: the store fails closed, so no rotation.
    let engrams = agent_engrams::dir_of(store);
    let head = std::fs::read_dir(&engrams)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.file_name().unwrap() != agent_engrams::INDEX)
        .unwrap();
    let good = std::fs::read(&head).unwrap();
    let mut event: Event = serde_json::from_slice(&good).unwrap();
    event.content.push('A');
    std::fs::write(&head, serde_json::to_vec(&event).unwrap()).unwrap();
    let why = rotate(store, &screen(), &owner(), "", NOW + 86_400, NOW).unwrap_err();
    assert!(why.contains("can't be read"), "{why}");

    assert_eq!(store.load().unwrap().unwrap(), old);
    assert_eq!(store.key().unwrap().unwrap(), old_key);
    assert!(!store.dir().join("next").exists());
    assert!(!store.dir().join("engrams.next").exists());
    assert!(lineage(store).unwrap().is_empty());
    std::fs::write(&head, good).unwrap();
}

#[test]
fn rotation_with_sync_on_archives_the_old_key_and_republishes_under_the_new_one() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("host");
    let memory = agent(&root, "alice", Arc::new(FileKeys));
    let store = memory.store();
    agent_sync::set_relays(store, &[RELAY.to_string()], NOW).unwrap();
    let fake = Fake::default();
    let first = agent_sync::sync(store, &screen(), &fake, NOW + 1);
    assert_eq!(first.error, None, "{first:?}");
    let before = owner_bodies(store);

    let rotated = rotate(store, &screen(), &owner(), "", NOW + 86_400, NOW + 20).unwrap();
    let request = rotated.archive.clone().expect("sync is on");
    // The owner signs it, with a request-borne credential for the old key.
    let parsed = nostr::domain::parse_identity_archive_request(&request, NOW + 20).unwrap();
    assert!(parsed.archive);
    assert_eq!(parsed.target, rotated.old);
    assert_eq!(parsed.replaced_by.as_deref(), Some(rotated.new.as_str()));
    assert_eq!(parsed.reason.as_deref(), Some("rotated"));
    assert_eq!(request.pubkey, agent::public_hex(&owner()));
    let auth = request
        .tags
        .iter()
        .find(|t| t.name() == Some("auth"))
        .unwrap();
    let credential = agent::Attestation {
        owner: auth.0[1].clone(),
        conditions: auth.0[2].clone(),
        signature: auth.0[3].clone(),
    };
    assert_eq!(credential.owner, request.pubkey);
    agent::verify_attestation(&rotated.old, &credential, request.created_at).unwrap();

    let sent = send_archive(store, &owner(), &request, &rotated.relays, &fake, NOW + 20);
    assert!(sent.iter().all(|s| s.accepted), "{sent:?}");
    assert_eq!(
        fake.held().auths.last().unwrap(),
        &(request.pubkey.clone(), None),
        "the owner publishes it as the owner"
    );

    // The next pass publishes her heads, profile, and relay list as the
    // new key, and the owner reads every memory from the relay.
    assert!(agent_sync::due(store, NOW + 21));
    let pass = agent_sync::sync(store, &screen(), &fake, NOW + 21);
    assert_eq!(pass.error, None, "{pass:?}");
    assert!(
        pass.relays[0].profile && pass.relays[0].relay_list,
        "{pass:?}"
    );
    let new_x: XOnlyPublicKey = rotated.new.parse().unwrap();
    let view = agent_sync::owner_read(&new_x, &owner(), &[RELAY.to_string()], &fake).unwrap();
    let from_relay: BTreeMap<String, Body> = view
        .heads
        .iter()
        .filter(|head| head.slug().as_str() != agent_engrams::PERSONA_SLUG)
        .map(|head| (head.slug().as_str().to_string(), head.body.clone()))
        .collect();
    assert_eq!(from_relay, before);
}

#[test]
fn retirement_deletes_her_key_from_the_keychain_and_the_owner_still_reads_her_memory() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("host");
    let keychain = Arc::new(MemoryKeys::default());
    let memory = agent(&root, "alice", keychain.clone());
    let store = memory.store();
    agent_sync::set_relays(store, &[RELAY.to_string()], NOW).unwrap();
    let before = owner_bodies(store);
    let pubkey = store.load().unwrap().unwrap().pubkey.unwrap();
    assert!(keychain.holds("agent:alice"));

    let retired = retire(store, Some(&owner()), NOW + 5).unwrap();
    assert!(retired.key_deleted);
    assert!(!keychain.holds("agent:alice"));
    let request = retired
        .archive
        .expect("sync is on and the owner key is here");
    let parsed = nostr::domain::parse_identity_archive_request(&request, NOW + 5).unwrap();
    assert_eq!(
        (
            parsed.target.as_str(),
            parsed.reason.as_deref(),
            parsed.replaced_by
        ),
        (pubkey.as_str(), Some("retired"), None)
    );

    let record = store.load().unwrap().unwrap();
    assert_eq!(record.state, State::Retired);
    assert_eq!(record.pubkey.as_deref(), Some(pubkey.as_str()));
    assert!(store.custody(&record).is_ok());
    assert_eq!(owner_bodies(store), before);
    assert_eq!(memory.entries().unwrap().len(), 2);
    assert!(
        journal(store)
            .iter()
            .any(|t| t.starts_with("retired by the owner"))
    );
    // A sync pass refuses her now, and retiring again deletes nothing.
    let pass = agent_sync::sync(store, &screen(), &Fake::default(), NOW + 6);
    assert_eq!(pass.error.as_deref(), Some("she is retired"));
    let again = retire(store, Some(&owner()), NOW + 7).unwrap();
    assert!(!again.key_deleted && again.archive.is_none());
}

#[test]
fn retirement_without_the_owner_key_journals_that_no_archive_request_went() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("host");
    let memory = agent(&root, "alice", Arc::new(FileKeys));
    agent_sync::set_relays(memory.store(), &[RELAY.to_string()], NOW).unwrap();
    let retired = retire(memory.store(), None, NOW + 5).unwrap();
    assert!(retired.key_deleted && retired.archive.is_none());
    assert!(!memory.store().dir().join("key").exists());
    assert!(
        journal(memory.store())
            .iter()
            .any(|t| t.contains("no owner key was at hand"))
    );
}

#[test]
fn a_moved_agent_names_her_new_controller_and_runs_nothing_here() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("host");
    let memory = agent(&root, "alice", Arc::new(FileKeys));
    let store = memory.store();
    agent_sync::set_relays(store, &[RELAY.to_string()], NOW).unwrap();
    assert!(mark_moved(store, "not-a-key", NOW).is_err());
    let other = "ab".repeat(32);
    let record = mark_moved(store, &other, NOW + 5).unwrap();
    assert_eq!(record.state, State::Moved);
    assert_eq!(record.state.word(), "moved");
    let roles = record.roles.unwrap();
    assert_eq!(
        (roles.controller.as_str(), roles.custodian.as_str()),
        (other.as_str(), other.as_str())
    );
    let pass = agent_sync::sync(store, &screen(), &Fake::default(), NOW + 6);
    assert!(pass.error.unwrap().contains("moved"));
    let why = rotate(store, &screen(), &owner(), "", NOW + 86_400, NOW + 7).unwrap_err();
    assert!(why.contains("moved"), "{why}");
    // Her record still reads in older terms: the key stays for the owner.
    assert!(store.key().unwrap().is_some());
}

#[test]
fn a_snapshot_never_holds_her_key_and_an_import_gets_a_new_one() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("host");
    let memory = agent(&root, "alice", Arc::new(FileKeys));
    let store = memory.store();
    let key = store.key().unwrap().unwrap();
    let secret_hex: String = key
        .secret_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();

    let none = export(store, &screen(), MemoryChoice::None, None, NOW).unwrap();
    assert!(none.core.is_none() && none.entries.is_empty());
    let core = export(store, &screen(), MemoryChoice::Core, None, NOW).unwrap();
    assert!(core.core.as_deref().unwrap().contains("I am alice"));
    assert!(core.entries.is_empty());
    let all = export(store, &screen(), MemoryChoice::All, Some(&owner()), NOW).unwrap();
    assert_eq!(all.core, core.core);
    assert_eq!(all.entries.len(), 2);
    for snapshot in [&none, &core, &all] {
        let text = serde_json::to_string(snapshot).unwrap();
        assert!(!text.contains(&secret_hex));
        let back: Snapshot = serde_json::from_str(&text).unwrap();
        assert_eq!(&back, snapshot);
    }

    // Import makes a new agent with a new key, her core, and her memory
    // encrypted under it.
    let bob = Store::with_keys(&root, "bob", Arc::new(FileKeys)).unwrap();
    let record = import(
        &bob,
        &screen(),
        &all,
        dir.path(),
        Some(&owner()),
        NOW + 86_400,
        NOW + 10,
    )
    .unwrap();
    assert_ne!(record.pubkey, all.pubkey);
    assert_eq!(record.charter, all.charter);
    assert_eq!(record.definition.as_ref(), Some(&all.definition));
    let bodies = owner_bodies(&bob);
    assert!(
        bodies["core"].to_json().contains("I am alice"),
        "her core came with her: {bodies:?}"
    );
    let entries = bodies
        .keys()
        .filter(|s| s.starts_with("mem/entry/"))
        .count();
    assert_eq!(entries, 2);
    let lines = lineage(&bob).unwrap();
    assert_eq!(lines.len(), 1);
    lines[0].verify().unwrap();
    assert_eq!(Some(&lines[0].old), all.pubkey.as_ref());
    // Importing over an agent that exists is refused.
    assert!(
        import(
            &bob,
            &screen(),
            &all,
            dir.path(),
            None,
            NOW + 86_400,
            NOW + 11
        )
        .is_err()
    );
}
