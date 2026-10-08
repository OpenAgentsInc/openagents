//! Key custody, the record's identity fields, and her profile. Every test
//! keeps her key in a file under a temporary directory or in
//! [`MemoryKeys`]; none touches the real keychain.

use super::*;
use crate::task::agent::{Record, Store, verify_attestation};
use crate::task::agent_profile;

const NOW: u64 = 1_791_158_400;

fn owner() -> SecretKey {
    agent::parse_secret(&"07".repeat(32)).unwrap()
}

fn memory_store(dir: &tempfile::TempDir) -> (Store, Arc<MemoryKeys>) {
    let keychain = Arc::new(MemoryKeys::default());
    let keys: Arc<dyn KeyStore> = Arc::new(Migrating::new(keychain.clone()));
    let store = Store::with_keys(&dir.path().join("host"), "alice", keys).unwrap();
    (store, keychain)
}

fn journal(store: &Store) -> Vec<Entry> {
    store.journal(100).unwrap()
}

#[test]
fn the_file_backend_keeps_her_key_private_and_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::with_keys(&dir.path().join("host"), "alice", Arc::new(FileKeys)).unwrap();
    let record = store.open(dir.path(), NOW).unwrap();
    let record = store.ensure_key(record, NOW).unwrap();
    let key = store.key().unwrap().unwrap();
    assert_eq!(record.pubkey, Some(agent::public_hex(&key)));
    assert_eq!(store.custody_kind(), "file");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(store.dir().join("key"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    // Ensuring again keeps the same key.
    let again = store.ensure_key(record.clone(), NOW + 1).unwrap();
    assert_eq!(again.pubkey, record.pubkey);
    assert!(store.delete_key().unwrap());
    assert!(store.key().unwrap().is_none());
}

#[test]
fn a_file_key_moves_into_the_keychain_only_after_it_reads_back() {
    let dir = tempfile::tempdir().unwrap();
    // She was made on a host that kept her key in a file.
    let file = Store::with_keys(&dir.path().join("host"), "alice", Arc::new(FileKeys)).unwrap();
    let record = file.open(dir.path(), NOW).unwrap();
    let record = file.ensure_key(record, NOW).unwrap();
    let key = file.key().unwrap().unwrap();
    // The host now runs with its keychain.
    let (store, keychain) = memory_store(&dir);
    assert_eq!(store.key().unwrap(), Some(key));
    assert!(keychain.holds("agent:alice"));
    assert!(
        !store.dir().join("key").exists(),
        "the file goes once it moved"
    );
    assert!(journal(&store).iter().any(|e| {
        e.text
            .contains("moved from its file into the host's keychain")
    }));
    assert_eq!(store.key().unwrap(), Some(key));
    assert!(store.custody(&record).is_ok());
}

/// A keychain that says it kept a key and then reads nothing back.
#[derive(Debug, Default)]
struct Forgetful;

impl KeyStore for Forgetful {
    fn custody(&self) -> &'static str {
        "keychain"
    }
    fn load(&self, _: Slot<'_>) -> Result<Option<SecretKey>, String> {
        Ok(None)
    }
    fn store(&self, _: Slot<'_>, _: &SecretKey) -> Result<(), String> {
        Ok(())
    }
    fn delete(&self, _: Slot<'_>) -> Result<bool, String> {
        Ok(false)
    }
}

#[test]
fn a_keychain_that_does_not_read_back_leaves_the_file_key_in_place() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("host");
    let file = Store::with_keys(&root, "alice", Arc::new(FileKeys)).unwrap();
    let record = file.open(dir.path(), NOW).unwrap();
    file.ensure_key(record, NOW).unwrap();
    let key = file.key().unwrap().unwrap();
    let store = Store::with_keys(
        &root,
        "alice",
        Arc::new(Migrating::new(Arc::new(Forgetful))),
    )
    .unwrap();
    assert_eq!(
        store.key().unwrap(),
        Some(key),
        "she keeps running on her file"
    );
    assert!(store.dir().join("key").exists(), "the file stays");
    assert!(
        journal(&store)
            .iter()
            .any(|e| e.text.contains("stays in its file"))
    );
    // Storing a new key through it is refused, never half done.
    let other = SecretKey::new(&mut secp256k1::rand::rng());
    assert!(
        Migrating::new(Arc::new(Forgetful))
            .store(
                Slot {
                    name: "alice",
                    dir: store.dir()
                },
                &other
            )
            .is_err()
    );
}

#[test]
fn a_key_she_had_that_cannot_be_read_fails_closed_and_is_never_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let (store, keychain) = memory_store(&dir);
    let record = store.open(dir.path(), NOW).unwrap();
    let record = store.ensure_key(record, NOW).unwrap();
    let key = store.key().unwrap().unwrap();
    assert_eq!(store.custody_kind(), "keychain");
    assert!(
        !store.dir().join("key").exists(),
        "a new key goes to the keychain"
    );

    // A locked keychain: she can't run, and nothing makes a new key.
    keychain.fail(true);
    let why = store.custody(&record).unwrap_err();
    assert!(why.contains("can't be read"), "{why}");
    assert!(store.ensure_key(record.clone(), NOW + 1).is_err());
    keychain.fail(false);
    assert_eq!(store.key().unwrap(), Some(key), "the same key, untouched");

    // A missing item: refused the same way, and no replacement appears.
    keychain
        .delete(Slot {
            name: "alice",
            dir: store.dir(),
        })
        .unwrap();
    let why = store.custody(&record).unwrap_err();
    assert!(why.contains("missing"), "{why}");
    let refused = store.ensure_key(record.clone(), NOW + 2).unwrap_err();
    assert!(refused.contains("won't make her a new one"), "{refused}");
    assert!(!keychain.holds("agent:alice"));
    assert!(store.key().unwrap().is_none());
    assert_eq!(store.load().unwrap().unwrap().pubkey, record.pubkey);

    // Another key in her place is refused too.
    let other = SecretKey::new(&mut secp256k1::rand::rng());
    keychain
        .store(
            Slot {
                name: "alice",
                dir: store.dir(),
            },
            &other,
        )
        .unwrap();
    assert!(store.custody(&record).unwrap_err().contains("another key"));
    assert!(store.ensure_key(record, NOW + 3).is_err());
}

#[test]
fn her_loop_refuses_requests_without_her_key_and_journals_why() {
    use crate::task::agent_host::Agents;
    use coder_host::Principal;
    use coder_host::access::agent::Mode;
    use coder_host::access::protocol::Operation;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("host");
    let store = Store::new(&root, "alice").unwrap();
    let record = store.open(dir.path(), NOW).unwrap();
    store.ensure_key(record, NOW).unwrap();
    std::fs::remove_file(store.dir().join("key")).unwrap();
    let agents = Agents::new(&root, dir.path().join("tasks"), Default::default());
    let asked = agents.answer(
        "k1",
        &Principal {
            device: "owner".into(),
            grant: None,
            epoch: None,
        },
        &Operation::AskAgent {
            agent: "alice".into(),
            text: "run the atif tests".into(),
            workspace: None,
            context: String::new(),
            mode: Mode::Terminal,
            typist: false,
            computer: None,
        },
    );
    assert!(asked.is_err());
    let refused: Vec<Entry> = journal(&store)
        .into_iter()
        .filter(|e| e.kind == Kind::Refused)
        .collect();
    assert_eq!(refused.len(), 1, "{refused:?}");
    assert!(refused[0].text.contains("missing from the file"));
    assert!(
        store.key().unwrap().is_none(),
        "no key was made in its place"
    );
}

#[test]
fn an_old_record_gains_a_definition_and_roles_when_opened() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::with_keys(&dir.path().join("host"), "alice", Arc::new(FileKeys)).unwrap();
    agent::private_dir(store.dir()).unwrap();
    let old = serde_json::json!({
        "schema": agent::RECORD_SCHEMA,
        "v": 1,
        "requires": [],
        "name": "alice",
        "charter": "read only",
        "workspace": "/tmp",
        "look": "alice",
        "created_at": 1,
        "desk": 3,
    });
    std::fs::write(store.dir().join("agent.json"), old.to_string()).unwrap();
    let loaded = store.load().unwrap().unwrap();
    assert_eq!(loaded.definition, None, "reading alone changes nothing");
    let record = store.open(dir.path(), NOW).unwrap();
    let definition = record.definition.clone().unwrap();
    assert_eq!(definition.display_name, "Alice");
    assert_eq!(definition.respond_to, agent::RESPOND_TO_OWNER);
    assert!(definition.system_prompt.is_empty());
    let roles = record.roles.clone().unwrap();
    assert_eq!(
        (roles.authority.as_str(), roles.controller.as_str()),
        ("", "host")
    );
    assert_eq!(store.load().unwrap().unwrap(), record, "saved");
    // Opening again migrates nothing more.
    store.open(dir.path(), NOW + 1).unwrap();
    let migrated = journal(&store)
        .iter()
        .filter(|e| e.kind == Kind::Migrated)
        .count();
    assert_eq!(migrated, 1);
    // The owner's attestation names the authority.
    let record = store.ensure_key(record, NOW).unwrap();
    let record = store.attest(record, &owner(), NOW + 86_400, NOW).unwrap();
    assert_eq!(record.roles.unwrap().authority, agent::public_hex(&owner()));
    // Unknown fields in the definition are refused, as in the rest.
    let text = std::fs::read_to_string(store.dir().join("agent.json"))
        .unwrap()
        .replace("\"respond_to\"", "\"surprise\": 1, \"respond_to\"");
    std::fs::write(store.dir().join("agent.json"), text).unwrap();
    assert!(store.load().is_err());
}

#[test]
fn a_system_prompt_in_her_definition_feeds_her_planner() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::with_keys(&dir.path().join("host"), "alice", Arc::new(FileKeys)).unwrap();
    let mut record: Record = store.open(dir.path(), NOW).unwrap();
    let default = crate::task::agent_steer::definition(&record);
    assert!(default.starts_with("You are alice, the owner's workshop agent"));
    record.definition = None;
    assert_eq!(crate::task::agent_steer::definition(&record), default);
    let mut defined = record.definition();
    defined.system_prompt = "You are Alice, a careful release engineer.".into();
    record.definition = Some(defined);
    let prompt = crate::task::agent_steer::definition(&record);
    assert!(prompt.starts_with("You are Alice, a careful release engineer."));
    assert!(prompt.ends_with(&record.charter), "her charter still binds");
}

#[test]
fn her_profile_is_signed_by_her_key_with_the_owner_attestation() {
    let dir = tempfile::tempdir().unwrap();
    let (store, _keychain) = memory_store(&dir);
    let record = store.open(dir.path(), NOW).unwrap();
    let record = store.ensure_key(record, NOW).unwrap();
    assert!(
        agent_profile::refresh(&store, &record, NOW)
            .unwrap()
            .is_none()
    );
    let record = store
        .attest(record, &owner(), NOW + 30 * 86_400, NOW)
        .unwrap();
    let profile = agent_profile::load(&store).unwrap().unwrap();
    assert_eq!(profile.kind, 0);
    assert_eq!(Some(profile.pubkey.clone()), record.pubkey);
    let attested = nostr::domain::verify_owner_attestation(&profile)
        .unwrap()
        .unwrap();
    assert_eq!(attested.owner_pubkey, agent::public_hex(&owner()));
    let content: serde_json::Value = serde_json::from_str(&profile.content).unwrap();
    assert_eq!(content["name"], "Alice");
    assert_eq!(content["bot"], true);
    assert!(agent_profile::current(&record, &profile));
    // Current: nothing to sign again.
    assert!(
        agent_profile::refresh(&store, &record, NOW + 1)
            .unwrap()
            .is_none()
    );
    // A renewal signs her profile again with the new attestation.
    let renewed = store
        .attest(record, &owner(), NOW + 300 * 86_400, NOW + 2)
        .unwrap();
    let again = agent_profile::load(&store).unwrap().unwrap();
    assert_ne!(again.id, profile.id);
    assert!(agent_profile::current(&renewed, &again));
    let until = verify_attestation(
        renewed.pubkey.as_deref().unwrap(),
        renewed.attestation.as_ref().unwrap(),
        NOW,
    )
    .unwrap();
    assert_eq!(until, NOW + 300 * 86_400);
    assert!(
        nostr::domain::verify_owner_attestation(&again)
            .unwrap()
            .is_some()
    );
    // Her secret key is in neither the profile nor her record.
    let secret: String = store
        .key()
        .unwrap()
        .unwrap()
        .secret_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let files = [
        std::fs::read_to_string(agent_profile::path(&store)).unwrap(),
        std::fs::read_to_string(store.dir().join("agent.json")).unwrap(),
    ];
    assert!(files.iter().all(|f| !f.contains(&secret)));
}
