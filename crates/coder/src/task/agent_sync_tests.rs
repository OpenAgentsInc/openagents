use super::*;
use crate::task::agent_memory::{Author, MemoryKind};

const OWNER: [u8; 32] = [7; 32];
const NOW: u64 = 1_800_000_000;
const RELAY: &str = "wss://relay.example";

fn owner() -> SecretKey {
    SecretKey::from_byte_array(OWNER).unwrap()
}

fn screen() -> secret_screen::Screen {
    secret_screen::Screen::shapes()
}

/// Alice under `root`, with her own key, attested by [`OWNER`].
fn alice(root: &std::path::Path) -> Memory {
    let store = Store::new(root, "alice").unwrap();
    let record = store.open(root, 1).unwrap();
    let record = store.ensure_key(record, 1).unwrap();
    store
        .attest(record, &owner(), NOW + 300 * 86_400, NOW)
        .unwrap();
    Memory::new(store, screen())
}

/// Alice on a second computer: her record and key, nothing else.
fn second_device(first: &Memory, root: &std::path::Path) -> Memory {
    let dir = root.join("agents").join("alice");
    std::fs::create_dir_all(&dir).unwrap();
    for file in ["agent.json", "key"] {
        std::fs::copy(first.store().dir().join(file), dir.join(file)).unwrap();
    }
    Memory::new(Store::new(root, "alice").unwrap(), screen())
}

fn sync_on(memory: &Memory) {
    set_relays(memory.store(), &[RELAY.to_string()], NOW).unwrap();
}

fn note_text(memory: &Memory, now: u64, text: &str) -> u64 {
    memory
        .add(MemoryKind::Note, Author::Owner, text, Vec::new(), now)
        .unwrap()
}

fn texts(memory: &Memory) -> Vec<String> {
    memory
        .entries()
        .unwrap()
        .into_iter()
        .map(|e| e.text)
        .collect()
}

fn journal(memory: &Memory) -> Vec<String> {
    memory
        .store()
        .journal(500)
        .unwrap()
        .into_iter()
        .map(|e| e.text)
        .filter(|t| t.starts_with(NOTE_PREFIX))
        .collect()
}

/// A relay in memory with NIP-01 replacement: the newest event per
/// address, ties to the lowest ID.
#[derive(Default)]
struct Held {
    events: Vec<Event>,
    /// Each AUTH: the key and the NIP-AA tag it presented.
    auths: Vec<(String, Option<Tag>)>,
    /// Each event published, by ID.
    published: Vec<String>,
    /// Connections to these URLs fail.
    down: BTreeSet<String>,
    /// Stored just before the next engram publish lands: another writer.
    race: Option<Event>,
}

fn address(event: &Event) -> Option<(u16, String, String)> {
    let kind = event.kind;
    if kind == 0 || (10_000..20_000).contains(&kind) {
        return Some((kind, event.pubkey.clone(), String::new()));
    }
    if (30_000..40_000).contains(&kind) {
        let d = event
            .tags
            .iter()
            .find(|t| t.name() == Some("d"))
            .and_then(|t| t.value())
            .unwrap_or_default();
        return Some((kind, event.pubkey.clone(), d.to_string()));
    }
    None
}

impl Held {
    fn store(&mut self, event: Event) -> (bool, String) {
        if self.events.iter().any(|e| e.id == event.id) {
            return (true, "duplicate: already have this event".into());
        }
        if let Some(addr) = address(&event) {
            if let Some(i) = self
                .events
                .iter()
                .position(|e| address(e).as_ref() == Some(&addr))
            {
                let have = &self.events[i];
                let newer = event.created_at > have.created_at
                    || (event.created_at == have.created_at && event.id < have.id);
                if !newer {
                    return (
                        true,
                        "duplicate: newer replaceable event already stored".into(),
                    );
                }
                self.events.remove(i);
            }
        }
        self.events.push(event);
        (true, String::new())
    }

    fn matches(filter: &Value, event: &Event) -> bool {
        let list = |name: &str| {
            filter[name].as_array().map(|a| {
                a.iter()
                    .map(|v| v.to_string().trim_matches('"').to_string())
                    .collect::<Vec<_>>()
            })
        };
        if list("kinds").is_some_and(|k| !k.contains(&event.kind.to_string())) {
            return false;
        }
        if list("authors").is_some_and(|a| !a.contains(&event.pubkey)) {
            return false;
        }
        for tag in ["p", "d"] {
            if let Some(values) = list(&format!("#{tag}")) {
                let hit = event.tags.iter().any(|t| {
                    t.name() == Some(tag)
                        && t.value().is_some_and(|v| values.iter().any(|w| w == v))
                });
                if !hit {
                    return false;
                }
            }
        }
        true
    }
}

#[derive(Clone, Default)]
struct Fake(Arc<Mutex<Held>>);

struct FakeRelay(Arc<Mutex<Held>>);

impl Fake {
    fn held(&self) -> std::sync::MutexGuard<'_, Held> {
        self.0.lock().unwrap()
    }
}

impl Connector for Fake {
    fn connect(
        &self,
        url: &str,
        key: &SecretKey,
        auth: Option<&Tag>,
    ) -> Result<Box<dyn Relay>, String> {
        let mut held = self.held();
        if held.down.contains(url) {
            return Err("connection refused".into());
        }
        held.auths.push((agent::public_hex(key), auth.cloned()));
        Ok(Box::new(FakeRelay(self.0.clone())))
    }
}

impl Relay for FakeRelay {
    fn publish(&mut self, event: &Event) -> Result<(bool, String), String> {
        let mut held = self.0.lock().unwrap();
        held.published.push(event.id.clone());
        if event.kind == ENGRAM_KIND
            && let Some(other) = held.race.take()
        {
            held.store(other);
        }
        Ok(held.store(event.clone()))
    }

    fn query(&mut self, filter: &Value) -> Result<Vec<Event>, String> {
        let held = self.0.lock().unwrap();
        let mut found: Vec<Event> = held
            .events
            .iter()
            .filter(|e| Held::matches(filter, e))
            .cloned()
            .collect();
        found.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        let limit = filter["limit"].as_u64().map_or(usize::MAX, |l| l as usize);
        found.truncate(limit);
        Ok(found)
    }
}

#[test]
fn urls_compare_in_canonical_form_and_dedupe() {
    assert_eq!(
        canonical("WSS://Relay.Example:443/").unwrap(),
        "wss://relay.example"
    );
    assert_eq!(
        canonical("ws://Relay.Example:80").unwrap(),
        "ws://relay.example"
    );
    assert_eq!(
        canonical("ws://relay.example:7777/").unwrap(),
        "ws://relay.example:7777"
    );
    assert_eq!(
        canonical("wss://relay.example/Path/").unwrap(),
        "wss://relay.example/Path/"
    );
    assert_eq!(canonical("wss://[::1]:443").unwrap(), "wss://[::1]");
    for bad in [
        "https://relay.example",
        "wss://",
        "relay.example",
        "wss://a@b",
        "wss://h:x",
    ] {
        assert!(canonical(bad).is_err(), "{bad}");
    }
    let urls: Vec<String> = [
        "wss://Relay.example:443",
        "wss://relay.example/",
        "ftp://x",
        "ws://other",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    assert_eq!(
        configured(&urls),
        vec!["wss://Relay.example:443", "ws://other"]
    );
}

#[test]
fn sync_is_off_by_default_and_the_owner_turns_it_on_and_off() {
    let dir = tempfile::tempdir().unwrap();
    let memory = alice(&dir.path().join("host"));
    let store = memory.store();
    assert!(!Settings::load(store).unwrap().on());
    assert!(!due(store, NOW));
    let fake = Fake::default();
    let status = sync(store, &screen(), &fake, NOW);
    assert_eq!(status.error.as_deref(), Some("relay sync is off"));
    assert!(fake.held().auths.is_empty());
    assert!(set_relays(store, &["https://x".into()], NOW).is_err());
    sync_on(&memory);
    assert!(due(store, NOW));
    set_relays(store, &[], NOW).unwrap();
    assert!(!due(store, NOW));
    assert!(request(store, NOW).is_err());
}

#[test]
fn a_pass_authenticates_with_the_auth_tag_and_publishes_heads_profile_and_relay_list() {
    let dir = tempfile::tempdir().unwrap();
    let memory = alice(&dir.path().join("host"));
    note_text(&memory, NOW, "the build runs on the 4080");
    sync_on(&memory);
    let fake = Fake::default();
    let status = sync(memory.store(), &screen(), &fake, NOW);
    assert_eq!(status.error, None, "{status:?}");
    assert!(status.conflicts.is_empty(), "{status:?}");
    let record = memory.store().load().unwrap().unwrap();
    let attestation = record.attestation.unwrap();
    {
        let held = fake.held();
        let (key, tag) = &held.auths[0];
        assert_eq!(Some(key), record.pubkey.as_ref());
        let tag = tag.as_ref().expect("the AUTH carries the auth tag");
        assert_eq!(
            tag.0,
            [
                "auth",
                &attestation.owner,
                &attestation.conditions,
                &attestation.signature
            ]
        );
        let kinds: BTreeSet<u16> = held.events.iter().map(|e| e.kind).collect();
        assert_eq!(kinds, BTreeSet::from([0, RELAY_LIST_KIND, ENGRAM_KIND]));
        let list = held
            .events
            .iter()
            .find(|e| e.kind == RELAY_LIST_KIND)
            .unwrap();
        assert_eq!(write_relays(list), vec![RELAY.to_string()]);
        assert_eq!(list.tags[0].0, ["r", RELAY, "write"]);
        let profile = held.events.iter().find(|e| e.kind == 0).unwrap();
        assert!(
            nostr::domain::verify_owner_attestation(profile)
                .unwrap()
                .is_some()
        );
    }
    // Core, persona, and the note.
    assert!(status.pushed >= 3, "{status:?}");
    assert_eq!(status.relays[0].published, status.pushed);
    assert!(status.relays[0].profile && status.relays[0].relay_list);
    let saved = Status::load(memory.store()).unwrap().unwrap();
    assert_eq!(saved, status);
    assert!(!due(memory.store(), NOW + 10));
    assert!(due(memory.store(), NOW + INTERVAL));
    request(memory.store(), NOW + 20).unwrap();
    assert!(due(memory.store(), NOW + 20));

    // A second pass has nothing to send.
    let before = fake.held().published.len();
    let again = sync(memory.store(), &screen(), &fake, NOW + 30);
    assert_eq!((again.pushed, again.pulled), (0, 0), "{again:?}");
    assert_eq!(fake.held().published.len(), before);
}

#[test]
fn an_edit_on_one_computer_reaches_the_other_through_the_relay() {
    let dir = tempfile::tempdir().unwrap();
    let first = alice(&dir.path().join("one"));
    note_text(&first, NOW, "first computer's note");
    sync_on(&first);
    let fake = Fake::default();
    assert_eq!(sync(first.store(), &screen(), &fake, NOW).error, None);

    let second = second_device(&first, &dir.path().join("two"));
    sync_on(&second);
    let status = sync(second.store(), &screen(), &fake, NOW + 10);
    assert_eq!(status.error, None, "{status:?}");
    assert!(status.pulled >= 1, "{status:?}");
    assert_eq!(texts(&second), vec!["first computer's note"]);

    let id = note_text(&second, NOW + 20, "second computer's note");
    second.forget(1, NOW + 21).unwrap();
    assert_eq!(sync(second.store(), &screen(), &fake, NOW + 30).error, None);
    let back = sync(first.store(), &screen(), &fake, NOW + 40);
    assert_eq!(back.error, None, "{back:?}");
    assert!(back.conflicts.is_empty(), "{back:?}");
    assert_eq!(texts(&first), vec!["second computer's note"]);
    assert_eq!(first.entries().unwrap()[0].id, id);
}

#[test]
fn a_concurrent_writer_is_a_conflict_that_is_journaled_and_never_retried() {
    let dir = tempfile::tempdir().unwrap();
    let first = alice(&dir.path().join("one"));
    sync_on(&first);
    let fake = Fake::default();
    assert_eq!(sync(first.store(), &screen(), &fake, NOW).error, None);
    let second = second_device(&first, &dir.path().join("two"));
    sync_on(&second);
    assert_eq!(sync(second.store(), &screen(), &fake, NOW + 1).error, None);

    // Both write entry 1; the second computer's lands on the relay while
    // the first one's publish is on the way.
    note_text(&first, NOW + 10, "the first writer");
    note_text(&second, NOW + 11, "the second writer");
    let theirs = match EngramStore::read(second.store(), &screen()) {
        Opened::Ready(engrams) => {
            let d = engrams.pair().d_tag(&agent_engrams::entry_slug(1));
            engrams.event(&d).unwrap().unwrap()
        }
        other => panic!("{other:?}"),
    };
    fake.held().race = Some(theirs.clone());
    let published = fake.held().published.len();
    let status = sync(first.store(), &screen(), &fake, NOW + 10);
    assert_eq!(status.error, None, "{status:?}");
    assert_eq!(status.conflicts.len(), 1, "{status:?}");
    assert!(status.conflicts[0].starts_with("mem/entry/1 on wss://relay.example"));
    assert!(
        journal(&first)
            .iter()
            .any(|t| t.contains("conflict: mem/entry/1"))
    );
    // One publish of the entry, no retry.
    let sent: Vec<String> = fake.held().published[published..].to_vec();
    assert_eq!(sent.iter().filter(|id| **id == theirs.id).count(), 0);
    assert_eq!(sent.len(), 1, "{sent:?}");

    // The next pass takes the newer head instead of fighting it.
    let next = sync(first.store(), &screen(), &fake, NOW + 20);
    assert!(next.conflicts.is_empty(), "{next:?}");
    assert_eq!(texts(&first), vec!["the second writer"]);
}

#[test]
fn a_clock_poisoned_head_is_a_conflict_and_never_taken() {
    let dir = tempfile::tempdir().unwrap();
    let first = alice(&dir.path().join("one"));
    sync_on(&first);
    let fake = Fake::default();
    let second = second_device(&first, &dir.path().join("two"));
    // The second computer's clock runs a day ahead.
    note_text(&second, NOW + 86_400, "from the future");
    sync_on(&second);
    assert_eq!(
        sync(second.store(), &screen(), &fake, NOW + 86_400).error,
        None
    );
    let status = sync(first.store(), &screen(), &fake, NOW);
    assert!(
        status
            .conflicts
            .iter()
            .any(|c| c.contains("ahead of this clock")),
        "{status:?}"
    );
    assert!(texts(&first).is_empty());
}

#[test]
fn a_full_answer_is_reported_as_truncated() {
    let dir = tempfile::tempdir().unwrap();
    let memory = alice(&dir.path().join("host"));
    sync_on(&memory);
    let fake = Fake::default();
    assert_eq!(sync(memory.store(), &screen(), &fake, NOW).error, None);
    assert!(!Status::load(memory.store()).unwrap().unwrap().truncated());
    // A relay that returns as many events as the limit may hold more.
    let filler: Vec<Event> = fake
        .held()
        .events
        .iter()
        .filter(|e| e.kind == ENGRAM_KIND)
        .cloned()
        .collect();
    {
        let mut held = fake.held();
        while held.events.len() < QUERY_LIMIT + 5 {
            let mut copy = filler[0].clone();
            copy.id = format!("{:064x}", held.events.len());
            held.events.push(copy);
        }
    }
    let limited = sync(memory.store(), &screen(), &fake, NOW + 2);
    assert!(limited.truncated(), "{limited:?}");
    assert!(Status::load(memory.store()).unwrap().unwrap().truncated());
}

#[test]
fn a_pass_fails_closed_without_her_key() {
    let dir = tempfile::tempdir().unwrap();
    let memory = alice(&dir.path().join("host"));
    sync_on(&memory);
    std::fs::remove_file(memory.store().dir().join("key")).unwrap();
    let fake = Fake::default();
    let status = sync(memory.store(), &screen(), &fake, NOW);
    assert!(status.error.unwrap().contains("missing"));
    assert!(fake.held().auths.is_empty());
}

#[test]
fn an_unreachable_relay_is_reported_and_the_others_still_sync() {
    let dir = tempfile::tempdir().unwrap();
    let memory = alice(&dir.path().join("host"));
    let down = "wss://down.example".to_string();
    set_relays(memory.store(), &[down.clone(), RELAY.to_string()], NOW).unwrap();
    let fake = Fake::default();
    fake.held().down.insert(down.clone());
    let status = sync(memory.store(), &screen(), &fake, NOW);
    assert_eq!(status.error, None);
    assert!(status.relays[0].error.is_some());
    assert!(status.relays[1].published > 0);
}

#[test]
fn the_owner_reads_her_memory_from_relays_with_the_owner_key_alone() {
    let dir = tempfile::tempdir().unwrap();
    let memory = alice(&dir.path().join("host"));
    note_text(&memory, NOW, "remember the staging relay");
    let forgotten = note_text(&memory, NOW, "this one goes");
    memory.forget(forgotten, NOW + 1).unwrap();
    sync_on(&memory);
    let fake = Fake::default();
    assert_eq!(sync(memory.store(), &screen(), &fake, NOW + 2).error, None);
    let agent: XOnlyPublicKey = memory
        .store()
        .load()
        .unwrap()
        .unwrap()
        .pubkey
        .unwrap()
        .parse()
        .unwrap();
    // Named under another spelling; her list names the relay she writes to.
    let view = owner_read(
        &agent,
        &owner(),
        &["wss://RELAY.example:443/".into()],
        &fake,
    )
    .unwrap();
    assert_eq!(view.relays, vec![RELAY.to_string()]);
    assert!(view.problems.is_empty(), "{view:?}");
    assert_eq!(view.forgotten, 1);
    let slugs: Vec<String> = view
        .heads
        .iter()
        .map(|h| h.slug().as_str().to_string())
        .collect();
    assert!(slugs.contains(&"core".to_string()), "{slugs:?}");
    assert!(
        view.heads
            .iter()
            .any(|h| h.body.to_json().contains("remember the staging relay"))
    );
    // The owner authenticated as the owner, without an auth tag.
    let held = fake.held();
    let (key, tag) = held.auths.last().unwrap();
    assert_eq!(*key, agent::public_hex(&owner()));
    assert!(tag.is_none());
    drop(held);
    // Another owner decrypts nothing.
    let stranger = SecretKey::from_byte_array([9; 32]).unwrap();
    let view = owner_read(&agent, &stranger, &[RELAY.into()], &fake).unwrap();
    assert!(view.heads.is_empty());
    assert!(owner_read(&agent, &owner(), &[], &fake).is_err());
}

#[test]
fn the_sweeper_runs_one_pass_when_due() {
    let dir = tempfile::tempdir().unwrap();
    let memory = alice(&dir.path().join("host"));
    let fake = Fake::default();
    let sweeper = Sweeper::new(Arc::new(fake.clone()));
    assert!(sweeper.sweep(memory.store(), &screen(), NOW).is_none());
    sync_on(&memory);
    sweeper
        .sweep(memory.store(), &screen(), NOW)
        .unwrap()
        .join()
        .unwrap();
    assert!(Status::load(memory.store()).unwrap().is_some());
    assert!(sweeper.sweep(memory.store(), &screen(), NOW + 1).is_none());
}
