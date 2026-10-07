use super::*;
use crate::task::agent_memory::{Author, MemoryKind, MemoryState};
use crate::task::agent_recall::By;

const OWNER: [u8; 32] = [7; 32];

fn owner() -> SecretKey {
    SecretKey::from_byte_array(OWNER).unwrap()
}

/// Alice with her own key, attested by [`OWNER`].
fn attested(dir: &tempfile::TempDir) -> Memory {
    let store = Store::new(&dir.path().join("host"), "alice").unwrap();
    let record = store.open(dir.path(), 1).unwrap();
    let record = store.ensure_key(record, 1).unwrap();
    store.attest(record, &owner(), 1_000_000, 1).unwrap();
    Memory::new(store, secret_screen::Screen::shapes())
}

fn ready(memory: &Memory, now: u64) -> EngramStore {
    match EngramStore::open(memory.store(), memory.screen(), now) {
        Opened::Ready(engrams) => engrams,
        other => panic!("not ready: {other:?}"),
    }
}

fn row(record: &str, importance: f64, at: u64) -> ScoreRow {
    ScoreRow {
        schema: SCORE_SCHEMA.into(),
        v: 1,
        record: record.into(),
        digest: "d".into(),
        importance,
        by: By::Rule,
        set: None,
        set_digest: None,
        probabilities: None,
        model: None,
        at,
    }
}

fn engram_notes(memory: &Memory) -> Vec<String> {
    memory
        .store()
        .journal(500)
        .unwrap()
        .into_iter()
        .filter(|e| e.text.starts_with(NOTE_PREFIX))
        .map(|e| e.text)
        .collect()
}

#[test]
fn memory_writes_go_through_as_signed_private_heads() {
    let dir = tempfile::tempdir().unwrap();
    let memory = attested(&dir);
    let id = memory
        .add(
            MemoryKind::Preference,
            Author::Agent,
            "Owner wants small commits.",
            vec!["journal:3".into()],
            100,
        )
        .unwrap();
    let engrams = ready(&memory, 101);
    let value = engrams.value(&entry_slug(id)).unwrap();
    let entry: MemoryEntry = serde_json::from_str(value).unwrap();
    assert_eq!(entry.state, MemoryState::Candidate);
    let head = engrams.head(&entry_slug(id)).unwrap();
    assert_eq!(
        head.body.extra()["schema"],
        super::super::agent_memory::SCHEMA
    );
    assert_eq!(head.body.extra()["v"], 1);
    // Core is seeded from her record, and her persona holds no secret.
    let core = engrams.core().unwrap();
    assert!(
        core.contains("alice") && core.contains("My charter:"),
        "{core}"
    );
    let persona = engrams.value(&Slug::parse(PERSONA_SLUG).unwrap()).unwrap();
    let key = memory.store().key().unwrap().unwrap();
    let secret_hex: String = key
        .secret_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert!(
        persona.contains("\"name\":\"alice\"") && persona.contains(&agent::public_hex(&owner()))
    );
    assert!(!persona.contains(&secret_hex));
    for path in std::fs::read_dir(engrams.dir()).unwrap() {
        let text = std::fs::read_to_string(path.unwrap().path()).unwrap();
        assert!(!text.contains(&secret_hex) && !text.contains("small commits"));
    }
    // The owner's acceptance goes through too.
    memory.decide(id, true, 200).unwrap();
    let engrams = ready(&memory, 201);
    let entry: MemoryEntry = serde_json::from_str(engrams.value(&entry_slug(id)).unwrap()).unwrap();
    assert_eq!(entry.state, MemoryState::Active);
    assert!(engrams.head(&entry_slug(id)).unwrap().created_at >= 200);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(engrams.dir()), 0o700);
        let file = engrams
            .dir()
            .join(format!("{}.json", engrams.pair().d_tag(&entry_slug(id))));
        assert_eq!(mode(&file), 0o600);
        assert_eq!(mode(&engrams.dir().join(INDEX)), 0o600);
    }
}

#[test]
fn forgetting_writes_a_tombstone_and_journals_no_content() {
    let dir = tempfile::tempdir().unwrap();
    let memory = attested(&dir);
    let id = memory
        .add(
            MemoryKind::Note,
            Author::Owner,
            "codename falcon",
            vec![],
            100,
        )
        .unwrap();
    memory.forget(id, 100).unwrap();
    let engrams = ready(&memory, 101);
    let head = engrams.head(&entry_slug(id)).unwrap();
    assert!(head.is_tombstone());
    // Monotonic: the tombstone is newer than the entry it covers.
    assert!(head.created_at > 100);
    assert!(engrams.live().iter().all(|h| h.slug != entry_slug(id)));
    for note in engram_notes(&memory) {
        assert!(!note.contains("falcon"), "{note}");
    }
    // Reconciling keeps it forgotten.
    reconcile(&memory, 102).unwrap();
    assert!(memory.entries().unwrap().is_empty());
}

#[test]
fn a_newer_engram_head_wins_and_working_rows_migrate() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(&dir.path().join("host"), "alice").unwrap();
    store.open(dir.path(), 1).unwrap();
    // An agent made before engrams: memory and scores, no key yet.
    let plain = Memory::new(store.clone(), secret_screen::Screen::shapes());
    let kept = plain
        .add(MemoryKind::Note, Author::Owner, "kept note", vec![], 10)
        .unwrap();
    let edited = plain
        .add(MemoryKind::Note, Author::Owner, "old words", vec![], 11)
        .unwrap();
    let gone = plain
        .add(MemoryKind::Note, Author::Owner, "to be removed", vec![], 12)
        .unwrap();
    Scores::of(&store)
        .append(&[row("journal:2", 4.0, 13)])
        .unwrap();
    assert!(!dir_of(&store).exists());
    let record = store.load().unwrap().unwrap();
    let record = store.ensure_key(record, 20).unwrap();
    store.attest(record, &owner(), 1_000_000, 20).unwrap();
    let memory = Memory::new(store.clone(), secret_screen::Screen::shapes());
    let done = reconcile(&memory, 30).unwrap();
    assert_eq!(done.pulled, 0);
    assert_eq!(done.pushed, 4, "three entries and one score");
    let mut engrams = ready(&memory, 31);
    assert_eq!(
        engrams.live().len(),
        5,
        "three entries, a score, her persona"
    );
    // Another device: a newer edit, a newer tombstone, a new entry, and a
    // newer score.
    let mut changed = memory.entries().unwrap()[1].clone();
    assert_eq!(changed.id, edited);
    changed.text = "new words".into();
    engrams.put_entry(&changed, 40).unwrap();
    engrams.forget_entry(gone, 40).unwrap();
    let mut arrived = changed.clone();
    arrived.id = 9;
    arrived.text = "from the phone".into();
    engrams.put_entry(&arrived, 40).unwrap();
    engrams.put_score(&row("journal:2", 9.0, 40), 41).unwrap();
    let done = reconcile(&memory, 50).unwrap();
    assert_eq!(
        done,
        Reconciled {
            pulled: 4,
            pushed: 0
        }
    );
    let texts: Vec<(u64, String)> = memory
        .entries()
        .unwrap()
        .into_iter()
        .map(|e| (e.id, e.text))
        .collect();
    assert_eq!(
        texts,
        vec![
            (kept, "kept note".to_string()),
            (edited, "new words".to_string()),
            (9, "from the phone".to_string()),
        ]
    );
    assert_eq!(
        Scores::of(&store).load().unwrap()["journal:2"].importance,
        9.0
    );
    // An older engram never overwrites a newer working row.
    assert_eq!(reconcile(&memory, 60).unwrap(), Reconciled::default());
}

#[test]
fn score_rows_go_through() {
    let dir = tempfile::tempdir().unwrap();
    let memory = attested(&dir);
    write_through_scores(&memory, &[row("memory:3", 6.0, 70)], 70);
    let engrams = ready(&memory, 71);
    let value = engrams
        .value(&Slug::parse("mem/score/memory/3").unwrap())
        .unwrap();
    let back: ScoreRow = serde_json::from_str(value).unwrap();
    assert_eq!(back, row("memory:3", 6.0, 70));
    assert!(score_slug("nonsense").is_none());
}

#[test]
fn the_index_is_a_cache_rebuilt_from_the_events() {
    let dir = tempfile::tempdir().unwrap();
    let memory = attested(&dir);
    memory
        .add(MemoryKind::Note, Author::Owner, "one", vec![], 100)
        .unwrap();
    let engrams = ready(&memory, 101);
    let index = read_index(engrams.dir()).unwrap().unwrap();
    assert_eq!(index, engrams.index());
    assert_eq!(index.heads.len(), 3, "core, persona, and one entry");
    std::fs::remove_file(engrams.dir().join(INDEX)).unwrap();
    let engrams = ready(&memory, 102);
    assert_eq!(read_index(engrams.dir()).unwrap().unwrap(), index);
    std::fs::write(engrams.dir().join(INDEX), "{not json").unwrap();
    let engrams = ready(&memory, 103);
    assert_eq!(read_index(engrams.dir()).unwrap().unwrap(), index);
}

#[test]
fn an_unreadable_store_carries_nothing_and_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let memory = attested(&dir);
    memory
        .add(MemoryKind::Note, Author::Owner, "one", vec![], 100)
        .unwrap();
    let engrams = ready(&memory, 101);
    let core_path = engrams
        .dir()
        .join(format!("{}.json", engrams.pair().d_tag(&Slug::core())));
    let core_before = std::fs::read(&core_path).unwrap();
    // Tamper with one head: its signature no longer verifies.
    let entry_path = engrams
        .dir()
        .join(format!("{}.json", engrams.pair().d_tag(&entry_slug(1))));
    let mut event: Event =
        serde_json::from_str(&std::fs::read_to_string(&entry_path).unwrap()).unwrap();
    event.created_at += 1;
    std::fs::write(&entry_path, serde_json::to_string(&event).unwrap()).unwrap();
    let opened = EngramStore::open(memory.store(), memory.screen(), 102);
    assert!(matches!(opened, Opened::Unreadable(_)));
    assert!(opened.carried_core().is_err());
    // A memory write still lands in the working file; the store is untouched.
    let before: Vec<_> = std::fs::read_dir(engrams.dir()).unwrap().collect();
    memory
        .add(MemoryKind::Note, Author::Owner, "two", vec![], 110)
        .unwrap();
    memory
        .add(MemoryKind::Note, Author::Owner, "three", vec![], 111)
        .unwrap();
    assert_eq!(memory.entries().unwrap().len(), 3);
    let after: Vec<_> = std::fs::read_dir(engrams.dir()).unwrap().collect();
    assert_eq!(before.len(), after.len());
    assert_eq!(std::fs::read(&core_path).unwrap(), core_before);
    assert!(reconcile(&memory, 112).is_err());
    assert_eq!(std::fs::read(&core_path).unwrap(), core_before);
    let notes = engram_notes(&memory);
    assert_eq!(
        notes.iter().filter(|n| n.contains("unreadable")).count(),
        1,
        "journaled once: {notes:?}"
    );
}

#[test]
fn the_secret_screen_and_the_core_limit_refuse_writes() {
    let dir = tempfile::tempdir().unwrap();
    let memory = attested(&dir);
    let mut engrams = ready(&memory, 100);
    let heads = engrams.heads().len();
    let token = format!("ghp_{}", "Ab1".repeat(12));
    let body = Body::memory(
        Slug::parse("mem/note/x").unwrap(),
        format!("the token is {token}"),
    )
    .unwrap();
    let refused = engrams.put(body, 101).unwrap_err();
    assert!(refused.contains("secret screen"), "{refused}");
    let refused = engrams
        .put(Body::core("a".repeat(CORE_MAX + 1)), 101)
        .unwrap_err();
    assert!(refused.contains("at most"), "{refused}");
    assert_eq!(ready(&memory, 102).heads().len(), heads);
    for path in std::fs::read_dir(engrams.dir()).unwrap() {
        let text = std::fs::read_to_string(path.unwrap().path()).unwrap();
        assert!(!text.contains(&token));
    }
}

#[test]
fn a_poisoned_clock_is_a_conflict_not_a_write() {
    let dir = tempfile::tempdir().unwrap();
    let memory = attested(&dir);
    let mut engrams = ready(&memory, 100);
    let slug = Slug::parse("mem/note/x").unwrap();
    engrams
        .put(Body::memory(slug.clone(), "far").unwrap(), 100_000)
        .unwrap();
    let conflict = engrams
        .put(Body::memory(slug, "now").unwrap(), 101)
        .unwrap_err();
    assert!(conflict.contains("conflict"), "{conflict}");
}

#[test]
fn the_owner_decrypts_with_the_owner_key() {
    let dir = tempfile::tempdir().unwrap();
    let memory = attested(&dir);
    let id = memory
        .add(
            MemoryKind::Note,
            Author::Owner,
            "owner reads this",
            vec![],
            100,
        )
        .unwrap();
    let view = owner_read(memory.store(), &owner()).unwrap();
    assert!(view.problems.is_empty(), "{:?}", view.problems);
    assert!(view.heads.iter().any(|h| h.slug().is_core()));
    let entry = view
        .heads
        .iter()
        .find(|h| h.slug() == entry_slug(id))
        .unwrap();
    let Body::Memory {
        value: Some(value), ..
    } = &entry.body
    else {
        panic!("not an entry");
    };
    assert!(value.contains("owner reads this"));
    // Another key reads nothing.
    let stranger = SecretKey::from_byte_array([9; 32]).unwrap();
    let view = owner_read(memory.store(), &stranger).unwrap();
    assert!(view.heads.is_empty());
    assert_eq!(view.problems.len(), 3);
}

#[test]
fn an_agent_without_an_owner_keeps_her_memory_and_no_engrams() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(&dir.path().join("host"), "alice").unwrap();
    let record = store.open(dir.path(), 1).unwrap();
    store.ensure_key(record, 1).unwrap();
    let memory = Memory::new(store.clone(), secret_screen::Screen::shapes());
    memory
        .add(MemoryKind::Note, Author::Owner, "one", vec![], 10)
        .unwrap();
    memory
        .add(MemoryKind::Note, Author::Owner, "two", vec![], 11)
        .unwrap();
    assert_eq!(memory.entries().unwrap().len(), 2);
    let opened = EngramStore::open(&store, memory.screen(), 12);
    assert!(matches!(opened, Opened::Skipped(_)));
    assert_eq!(opened.carried_core(), Ok(None));
    assert!(!dir_of(&store).exists());
    let notes = engram_notes(&memory);
    assert_eq!(notes.len(), 1, "journaled once: {notes:?}");
    assert!(notes[0].contains("no owner attestation"));
}

#[test]
fn insights_live_at_their_own_slug_and_old_ones_move_there() {
    let dir = tempfile::tempdir().unwrap();
    let memory = attested(&dir);
    let insight = memory
        .add(
            MemoryKind::Insight,
            Author::Agent,
            "Small changes merge the same day.",
            vec!["journal:1".into(), "memory:2".into()],
            10,
        )
        .unwrap();
    let engrams = ready(&memory, 11);
    let value = engrams.value(&insight_slug(insight)).unwrap();
    let entry: MemoryEntry = serde_json::from_str(value).unwrap();
    assert_eq!(
        entry.sources,
        vec!["journal:1", "memory:2"],
        "citations kept"
    );
    assert!(engrams.head(&entry_slug(insight)).is_none(), "written once");

    // An older host wrote this insight at `mem/entry/ID`.
    let mut engrams = ready(&memory, 12);
    engrams.forget_entry(insight, 12).unwrap();
    let body = Body::memory(entry_slug(insight), serde_json::to_string(&entry).unwrap())
        .unwrap()
        .with_extra("schema", entry.schema.clone().into())
        .unwrap();
    engrams.put(body, 13).unwrap();
    assert!(engrams.value(&insight_slug(insight)).is_none());
    reconcile(&memory, 20).unwrap();
    let engrams = ready(&memory, 21);
    assert!(engrams.value(&insight_slug(insight)).is_some(), "moved");
    assert!(engrams.head(&entry_slug(insight)).unwrap().is_tombstone());
    assert_eq!(memory.entries().unwrap(), vec![entry]);

    // Forgetting tombstones the slug it lives at.
    memory.forget(insight, 30).unwrap();
    let engrams = ready(&memory, 31);
    assert!(engrams.head(&insight_slug(insight)).unwrap().is_tombstone());
    reconcile(&memory, 32).unwrap();
    assert!(memory.entries().unwrap().is_empty());
}

#[test]
fn deleted_working_files_rebuild_from_engrams_identically() {
    let dir = tempfile::tempdir().unwrap();
    let memory = attested(&dir);
    memory
        .add(MemoryKind::Note, Author::Owner, "first", vec![], 10)
        .unwrap();
    let pref = memory
        .add(
            MemoryKind::Preference,
            Author::Agent,
            "Owner wants tests.",
            vec![],
            11,
        )
        .unwrap();
    memory.decide(pref, true, 12).unwrap();
    memory
        .add(
            MemoryKind::Insight,
            Author::Agent,
            "An insight.",
            vec!["journal:1".into()],
            13,
        )
        .unwrap();
    let gone = memory
        .add(MemoryKind::Note, Author::Owner, "forgotten", vec![], 14)
        .unwrap();
    memory.forget(gone, 15).unwrap();
    let store = memory.store().clone();
    let rows = [row("journal:2", 4.0, 16), row("memory:1", 2.0, 16)];
    Scores::of(&store).append(&rows).unwrap();
    write_through_scores(&memory, &rows, 16);
    let working = std::fs::read(store.dir().join("memory.jsonl")).unwrap();
    let scores = Scores::of(&store).load().unwrap();

    std::fs::remove_file(store.dir().join("memory.jsonl")).unwrap();
    std::fs::remove_file(store.dir().join("scores.jsonl")).unwrap();
    // Any read of the memory rebuilds both files from the heads.
    assert_eq!(memory.entries().unwrap().len(), 3);
    assert_eq!(
        std::fs::read(store.dir().join("memory.jsonl")).unwrap(),
        working
    );
    assert_eq!(Scores::of(&store).load().unwrap(), scores);
    assert!(
        engram_notes(&memory)
            .iter()
            .any(|n| n.contains("rebuilt the working files from engrams: 3 entries, 2 score rows"))
    );
    // An unreadable store rebuilds nothing.
    std::fs::remove_file(store.dir().join("memory.jsonl")).unwrap();
    let engrams = ready(&memory, 20);
    let file = engrams
        .dir()
        .join(format!("{}.json", engrams.pair().d_tag(&entry_slug(1))));
    std::fs::write(&file, "{ not an event").unwrap();
    assert!(rebuild_from_engrams(&memory).is_err());
    assert!(memory.entries().unwrap().is_empty());
}

#[test]
fn reach_lists_orphans_and_dangling_links_from_core() {
    let dir = tempfile::tempdir().unwrap();
    let memory = attested(&dir);
    let linked = memory
        .add(MemoryKind::Note, Author::Owner, "linked note", vec![], 10)
        .unwrap();
    let orphan = memory
        .add(MemoryKind::Note, Author::Owner, "orphan note", vec![], 11)
        .unwrap();
    let mut engrams = ready(&memory, 12);
    // A memory reached through another memory, and links to nothing.
    let chained = Body::memory(
        Slug::parse("mem/topic/merges").unwrap(),
        format!("see [[mem/entry/{linked}]] and [[mem/entry/404]]"),
    )
    .unwrap();
    engrams.put(chained, 12).unwrap();
    engrams
        .put(
            Body::core("I am alice. [[mem/topic/merges]] [[mem/gone]] [[core]]"),
            13,
        )
        .unwrap();
    let reach = engrams.reach();
    assert!(reach.core);
    assert_eq!(
        reach.reachable,
        vec![
            format!("mem/entry/{linked}"),
            "mem/topic/merges".to_string()
        ]
    );
    assert_eq!(reach.orphans, vec![format!("mem/entry/{orphan}")]);
    assert_eq!(
        reach.dangling,
        vec![
            Dangling {
                from: "core".into(),
                to: "mem/gone".into()
            },
            Dangling {
                from: "mem/topic/merges".into(),
                to: "mem/entry/404".into()
            },
        ]
    );
    // A forgotten target dangles; nothing deletes the orphan.
    memory.forget(linked, 20).unwrap();
    let engrams = ready(&memory, 21);
    let reach = engrams.reach();
    assert!(
        reach
            .dangling
            .iter()
            .any(|d| d.to == format!("mem/entry/{linked}"))
    );
    assert!(engrams.value(&entry_slug(orphan)).is_some());
    // The owner key reads the same graph.
    let view = owner_read(memory.store(), &owner()).unwrap();
    assert_eq!(super::reach(&view.heads), reach);
}
