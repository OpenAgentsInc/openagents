use std::cell::RefCell;
use std::rc::Rc;

use super::super::agent::Store;
use super::super::agent_memory::Author;
use super::*;

const HOUR: u64 = 3600;
const T0: u64 = 1_790_000_000;

fn row(at: u64, kind: Kind, text: &str) -> Entry {
    Entry::new(at, kind, text)
}

fn ran(at: u64, text: &str, status: i32) -> Entry {
    let mut entry = row(at, Kind::Ran, text);
    entry.status = Some(status);
    entry
}

fn note(id: u64, at: u64, kind: MemoryKind, state: MemoryState, text: &str) -> MemoryEntry {
    MemoryEntry {
        schema: super::super::agent_memory::SCHEMA.into(),
        v: 1,
        requires: Vec::new(),
        id,
        kind,
        state,
        author: Author::Owner,
        text: text.into(),
        at,
        sources: Vec::new(),
    }
}

fn record(reference: Ref, entry: Entry) -> Record {
    Record {
        reference,
        body: Body::Journal(entry),
    }
}

fn positioned(rows: Vec<Entry>) -> Vec<(usize, Entry)> {
    rows.into_iter()
        .enumerate()
        .map(|(i, e)| (i + 1, e))
        .collect()
}

#[test]
fn the_rule_table_scores_known_cases_and_leaves_the_rest_to_jev() {
    let journal = |entry| record(Ref::Journal(1), entry);
    assert_eq!(rule(&journal(ran(T0, "ls crates", 0))), Some(1.0));
    assert_eq!(rule(&journal(ran(T0, "cargo test -p gym", 101))), None);
    assert_eq!(
        rule(&journal(row(T0, Kind::Control, "the owner stopped alice"))),
        Some(10.0)
    );
    assert_eq!(
        rule(&journal(row(T0, Kind::Control, "the owner retired alice"))),
        Some(10.0)
    );
    assert_eq!(
        rule(&journal(row(
            T0,
            Kind::Job,
            "job nightly-check fired, occurrence 3 of 30"
        ))),
        Some(1.0)
    );
    assert_eq!(
        rule(&journal(row(T0, Kind::Keyed, "made alice's key"))),
        Some(1.0)
    );
    for kind in [
        Kind::Request,
        Kind::Report,
        Kind::Failed,
        Kind::Rejected,
        Kind::Task,
    ] {
        assert_eq!(rule(&journal(row(T0, kind, "something"))), None, "{kind:?}");
    }
    let memory = |kind, text: &str| Record {
        reference: Ref::Memory(1),
        body: Body::Memory(note(1, T0, kind, MemoryState::Active, text)),
    };
    assert_eq!(
        rule(&memory(MemoryKind::Note, "x")),
        Some(8.0),
        "an owner's note"
    );
    assert_eq!(rule(&memory(MemoryKind::Preference, "x")), Some(8.0));
    assert_eq!(
        rule(&memory(MemoryKind::Outcome, "1: run the tests (ok exit 0)")),
        Some(1.0)
    );
    assert_eq!(rule(&memory(MemoryKind::Project, "x")), None);
    // Priors stay inside the scale and rank failures over routine rows.
    let failed = journal(row(T0, Kind::Failed, "the run failed"));
    let report = journal(row(T0, Kind::Report, "done"));
    assert!(prior(&failed) > prior(&report));
}

#[test]
fn importance_maps_the_weighted_level_onto_one_to_ten() {
    let levels = |pairs: &[(u32, f64)]| pairs.iter().copied().collect::<BTreeMap<_, _>>();
    assert!((importance_from(&levels(&[(0, 1.0)]), 0.0, 4) - 1.0).abs() < 1e-9);
    assert!((importance_from(&levels(&[(3, 1.0)]), 0.0, 4) - 10.0).abs() < 1e-9);
    let split = importance_from(&levels(&[(1, 0.5), (2, 0.5)]), 0.0, 4);
    assert!((split - 5.5).abs() < 1e-9, "{split}");
    // No probabilities: the reported position.
    assert!((importance_from(&BTreeMap::new(), 2.0, 4) - 7.0).abs() < 1e-9);
    // The set is valid, names its measurement, and the request carries
    // the record and nothing about its neighbors.
    let set = importance_set();
    assert_eq!(set.id, "openagents.memory-importance.v1");
    assert!(set.policy.evidence.iter().all(|path| {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(path)
            .is_file()
    }));
    let request = judge_request(
        "alice",
        &record(
            Ref::Journal(4),
            row(T0, Kind::Task, "task t-101 merged by the owner"),
        ),
    )
    .unwrap();
    let state = format!("{:?}", request.state);
    assert!(state.contains("task t-101 merged by the owner"));
    assert!(state.contains("journal row"));
}

#[test]
fn receipts_round_trip_and_recency_comes_from_the_latest() {
    let carried = [Ref::Memory(1), Ref::Memory(3), Ref::Journal(22)];
    let text = receipt(&carried).unwrap();
    assert_eq!(
        text,
        "the briefing carried memory entries 1, 3; journal rows 22"
    );
    assert_eq!(parse_receipt(&text), carried.to_vec());
    assert_eq!(
        receipt(&[Ref::Memory(1), Ref::Memory(3)]).unwrap(),
        "the briefing carried memory entries 1, 3",
        "a memory-only receipt reads as it always has"
    );
    assert_eq!(receipt(&[]), None);
    assert_eq!(
        parse_receipt("the briefing carried journal rows 5, 9"),
        vec![Ref::Journal(5), Ref::Journal(9)]
    );
    assert!(parse_receipt("wrote note entry 1: x").is_empty());

    let journal = positioned(vec![
        row(T0, Kind::Report, "the old report"),
        row(T0, Kind::Report, "the other old report"),
        row(
            T0 + HOUR,
            Kind::Memory,
            "the briefing carried journal rows 1",
        ),
        row(
            T0 + 50 * HOUR,
            Kind::Memory,
            "the briefing carried journal rows 1",
        ),
        row(
            T0 + 90 * HOUR,
            Kind::Memory,
            "the briefing carried journal rows 2",
        ),
    ]);
    let last = last_carried(&journal, T0 + 60 * HOUR);
    assert_eq!(last.get(&Ref::Journal(1)), Some(&(T0 + 50 * HOUR)));
    assert_eq!(
        last.get(&Ref::Journal(2)),
        None,
        "after the briefing's time"
    );

    // Two otherwise equal rows: the one a briefing carried lately is the
    // more recent.
    let recall = recall(
        &Inputs {
            agent: "alice",
            request: "anything",
            workspace: "/w",
            now: T0 + 60 * HOUR,
            journal: &journal,
            memory: &[],
        },
        &HashMap::new(),
        &mut Services::offline(),
    );
    assert_eq!(recall.carried[0], Ref::Journal(1), "{recall:?}");
}

#[test]
fn candidates_leave_out_memory_rows_and_what_waits_for_the_owner() {
    let journal = positioned(vec![
        row(T0, Kind::Request, "remember that the relay listens on 7447"),
        row(
            T0,
            Kind::Memory,
            "wrote note entry 1: remember that the relay listens on 7447",
        ),
        row(T0, Kind::Request, "always squash before you push"),
        row(T0, Kind::Request, "run the gym tests"),
        ran(T0, "cargo test -p gym", 0),
    ]);
    let memory = vec![
        note(
            1,
            T0,
            MemoryKind::Note,
            MemoryState::Active,
            "remember that the relay listens on 7447",
        ),
        note(
            2,
            T0,
            MemoryKind::Preference,
            MemoryState::Candidate,
            "The owner said: always squash before you push",
        ),
        note(
            3,
            T0,
            MemoryKind::Preference,
            MemoryState::Rejected,
            "never test",
        ),
        note(
            4,
            T0,
            MemoryKind::Outcome,
            MemoryState::Active,
            "t-1 merged",
        ),
    ];
    let refs: Vec<Ref> = candidates(&journal, &memory)
        .iter()
        .map(|r| r.reference)
        .collect();
    assert_eq!(
        refs,
        vec![
            Ref::Memory(1),
            Ref::Memory(4),
            Ref::Journal(4),
            Ref::Journal(5)
        ]
    );
    // The note stands first even when nothing about it matches.
    let recall = recall(
        &Inputs {
            agent: "alice",
            request: "run the gym tests",
            workspace: "/w",
            now: T0 + HOUR,
            journal: &journal,
            memory: &memory,
        },
        &HashMap::new(),
        &mut Services::offline(),
    );
    assert_eq!(recall.carried[0], Ref::Memory(1));
    assert!(!recall.text.contains("squash"));
    assert!(recall.text.contains("(ran, exit 0, "), "{}", recall.text);
    assert_eq!(recall.basis, "bm25");
}

/// Jev's stand-in: an importance per text, and every record it was asked.
#[derive(Clone, Default)]
struct FakeJudge {
    by_text: BTreeMap<String, f64>,
    asked: Rc<RefCell<Vec<String>>>,
}

impl Judge for FakeJudge {
    fn judge(&mut self, _: &str, records: &[&Record]) -> Vec<Result<Judged, String>> {
        records
            .iter()
            .map(|record| {
                self.asked.borrow_mut().push(record.reference.to_string());
                let importance = self.by_text.get(record.text()).copied().unwrap_or(2.0);
                Ok(Judged {
                    importance,
                    probabilities: BTreeMap::from([(0, 0.5), (3, 0.5)]),
                    model: "fake-jev".into(),
                })
            })
            .collect()
    }
}

fn memory_in(dir: &tempfile::TempDir) -> Memory {
    let store = Store::new(&dir.path().join("host"), "alice").unwrap();
    store.open(dir.path(), T0).unwrap();
    Memory::new(store, secret_screen::Screen::shapes())
}

#[test]
fn an_important_old_record_outranks_a_fresh_mundane_one_and_scores_once() {
    let dir = tempfile::tempdir().unwrap();
    let memory = memory_in(&dir);
    let store = memory.store().clone();
    let old = "task t-7 rejected by the owner at the Merge station";
    for entry in [
        row(T0 - 200 * HOUR, Kind::Report, "the docs build passed"),
        row(T0, Kind::Task, old),
        ran(T0 + 70 * HOUR, "ls crates", 0),
    ] {
        store.append(&entry).unwrap();
    }
    let judge = FakeJudge {
        by_text: BTreeMap::from([(old.to_string(), 10.0)]),
        ..FakeJudge::default()
    };
    let asked = judge.asked.clone();
    let mut services = Services {
        judge: Some(Box::new(judge.clone())),
        relevance: Box::new(Lexical),
    };
    let now = T0 + 72 * HOUR;
    let recall = memory
        .recall("what is the weather", "/w", now, &mut services)
        .unwrap();
    let first_scored = recall
        .carried
        .iter()
        .position(|r| matches!(r, Ref::Journal(_)))
        .unwrap();
    let task_pos = store
        .journal_rows()
        .unwrap()
        .iter()
        .find(|(_, e)| e.text == old)
        .unwrap()
        .0;
    assert_eq!(
        recall.carried[first_scored],
        Ref::Journal(task_pos),
        "{}",
        recall.text
    );
    let ls = recall.text.find("ls crates").unwrap();
    assert!(recall.text.find(old).unwrap() < ls, "{}", recall.text);

    // The sidecar holds a rule row for the command and a Jev row for the
    // task, with the set and its digest; no text.
    let rows = Scores::of(&store).load().unwrap();
    let task = &rows[&format!("journal:{task_pos}")];
    assert_eq!(task.by, By::Jev);
    assert_eq!(task.set.as_deref(), Some("openagents.memory-importance.v1"));
    assert_eq!(
        task.set_digest.as_deref(),
        Some(importance_set().digest().as_str())
    );
    assert_eq!(task.model.as_deref(), Some("fake-jev"));
    assert!(
        rows.values()
            .any(|r| r.by == By::Rule && r.importance == 1.0)
    );
    let sidecar = std::fs::read_to_string(store.dir().join("scores.jsonl")).unwrap();
    assert!(!sidecar.contains("Merge station") && !sidecar.contains("ls crates"));
    assert!(sidecar.contains(SCORE_SCHEMA));

    // A second briefing reuses the scores: Jev isn't asked again.
    let before = asked.borrow().len();
    memory
        .recall("what is the weather", "/w", now + HOUR, &mut services)
        .unwrap();
    assert_eq!(asked.borrow().len(), before);
    // memory.jsonl and journal.jsonl keep their shape: the briefing wrote
    // neither.
    assert!(!store.dir().join("memory.jsonl").exists());
    assert_eq!(store.journal_rows().unwrap().len(), 4);
}

/// Embeddings' stand-in: letter counts, so texts that share letters are
/// near. It counts the texts it embedded and can fail on demand.
struct FakeEmbed {
    embedded: Rc<RefCell<usize>>,
    fail: bool,
}

impl knowledge::search::Embed for FakeEmbed {
    fn model(&self) -> &str {
        "fake/letters"
    }

    async fn embed(
        &self,
        inputs: Vec<String>,
    ) -> Result<(Vec<Vec<f32>>, Option<f64>), knowledge::search::EmbedError> {
        if self.fail {
            return Err("the provider refused".into());
        }
        *self.embedded.borrow_mut() += inputs.len();
        Ok((
            inputs
                .iter()
                .map(|text| {
                    let mut v = vec![0f32; 26];
                    for b in text.to_ascii_lowercase().bytes() {
                        if b.is_ascii_lowercase() {
                            v[usize::from(b - b'a')] += 1.0;
                        }
                    }
                    v
                })
                .collect(),
            None,
        ))
    }
}

#[test]
fn embedded_relevance_caches_vectors_and_falls_back_to_bm25() {
    let dir = tempfile::tempdir().unwrap();
    let embedded = Rc::new(RefCell::new(0));
    let mut relevance = Embedded::new(
        FakeEmbed {
            embedded: embedded.clone(),
            fail: false,
        },
        dir.path(),
    );
    let texts = vec!["zzz zzz".to_string(), "abc abc".to_string()];
    let scores = relevance.relevance("abc", &texts);
    assert!(scores[1] > scores[0], "{scores:?}");
    assert_eq!(*embedded.borrow(), 3, "two documents and the query");
    assert!(dir.path().join("embeddings-fake-letters.bin").is_file());
    assert_eq!(relevance.basis(), "cosine fake/letters");
    // The cache holds the documents: the next ranking embeds the query.
    let mut again = Embedded::new(
        FakeEmbed {
            embedded: embedded.clone(),
            fail: false,
        },
        dir.path(),
    );
    assert_eq!(again.relevance("zzz", &texts).len(), 2);
    assert_eq!(*embedded.borrow(), 4);

    let mut failing = Embedded::new(
        FakeEmbed {
            embedded,
            fail: true,
        },
        dir.path(),
    );
    let lexical = failing.relevance("abc", &texts);
    assert_eq!(lexical, knowledge::search::bm25_texts(&texts, "abc"));
    assert!(failing.basis().starts_with("bm25"), "{}", failing.basis());
}
