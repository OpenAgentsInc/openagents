use std::path::Path;

use super::super::agent_interview::{Fixture, reflect_fixture};
use super::super::agent_jobs::{self, Edit, Facts, Jobs};
use super::super::agent_memory::MemoryState;
use super::super::issue_pick::{Open, Pull};
use super::*;

/// Replies in order, then nothing.
struct Replies(Vec<String>);

impl Writer for Replies {
    fn write(&mut self, _: &str, _: &str) -> Result<Reply, String> {
        if self.0.is_empty() {
            return Err("no reply left".into());
        }
        Ok(Reply {
            text: self.0.remove(0),
            model: "fake-writer".into(),
            usd: Some(0.01),
        })
    }
}

/// Jev's answers by insight text; an unknown insight fails.
struct Answers(BTreeMap<String, (f64, f64)>);

impl Verify for Answers {
    fn verify(&mut self, _: &str, insight: &str, _: &[&Record]) -> Result<Support, String> {
        let (supported, preference) = self.0.get(insight).copied().ok_or("the door is down")?;
        Ok(Support {
            supported,
            preference,
            model: "fake-jev".into(),
        })
    }
}

fn insight(id: u64, sources: &[&str]) -> MemoryEntry {
    MemoryEntry {
        schema: super::super::agent_memory::SCHEMA.into(),
        v: 1,
        requires: Vec::new(),
        id,
        kind: MemoryKind::Insight,
        state: super::super::agent_memory::MemoryState::Active,
        author: Author::Agent,
        text: format!("insight {id}"),
        at: 100 + id,
        sources: sources.iter().map(|s| (*s).to_string()).collect(),
    }
}

fn record(pos: usize, text: &str) -> Record {
    Record {
        reference: Ref::Journal(pos),
        body: Body::Journal(Entry::new(10 * pos as u64, Kind::Report, text)),
    }
}

fn written(text: &str, because: &[&str]) -> Written {
    Written {
        text: text.into(),
        because: because.iter().map(|s| (*s).to_string()).collect(),
    }
}

fn reason(insight: &Insight) -> &str {
    match &insight.verdict {
        Verdict::Dropped(why) => why,
        other => panic!("{other:?} for {}", insight.text),
    }
}

#[test]
fn fabricated_unshown_and_unsupported_insights_are_each_refused() {
    let journal: BTreeSet<usize> = [1, 2, 3].into();
    let entries = [insight(1, &["journal:1"])];
    let memory: HashMap<u64, &MemoryEntry> = entries.iter().map(|e| (e.id, e)).collect();
    let shown = [
        record(1, "the atif tests pass"),
        record(2, "the gym tests pass"),
    ];
    let screen = secret_screen::Screen::shapes();
    let context = Context {
        agent: "alice",
        journal: &journal,
        memory: &memory,
        shown: &shown,
        screen: &screen,
    };
    let token = format!("ghp_{}", "Ab1".repeat(12));
    let secret = format!("the gym token is {token}");
    let mut jev = Answers(BTreeMap::from([
        ("Both test suites pass.".to_string(), (0.9, 0.1)),
        (
            "The owner wants the tests run first.".to_string(),
            (0.8, 0.9),
        ),
        ("The gym tests always fail.".to_string(), (0.2, 0.0)),
        (secret.clone(), (0.9, 0.0)),
    ]));
    let mut check =
        |text: &str, because: &[&str]| check(&written(text, because), &context, &mut jev);

    let fabricated = check("Both test suites pass.", &["journal:1", "journal:99"]);
    assert_eq!(
        reason(&fabricated),
        "it cites journal:99, which doesn't exist"
    );
    let ghost = check("Both test suites pass.", &["memory:7"]);
    assert_eq!(reason(&ghost), "it cites memory:7, which doesn't exist");
    let unshown = check("Both test suites pass.", &["journal:1", "journal:3"]);
    assert_eq!(
        reason(&unshown),
        "it cites journal:3, which wasn't shown for this question"
    );
    let unshown_insight = check("Both test suites pass.", &["memory:1"]);
    assert!(reason(&unshown_insight).contains("wasn't shown"));
    let unsupported = check("The gym tests always fail.", &["journal:2"]);
    assert!(
        reason(&unsupported).starts_with("the cited records don't support it (Jev 0.20"),
        "{unsupported:?}"
    );
    assert_eq!(
        reason(&check("Both test suites pass.", &[])),
        "it cites no record"
    );
    assert!(reason(&check("Both test suites pass.", &["line 4"])).contains("isn't a record"));
    assert!(reason(&check(&secret, &["journal:1"])).starts_with("the secret screen refused it"));
    assert!(reason(&check("Nobody answers this.", &["journal:1"])).starts_with("Jev couldn't"));

    let stored = check(
        "Both test suites pass.",
        &["journal:1", "journal:2", "journal:1"],
    );
    assert_eq!(stored.verdict, Verdict::Stored { depth: 1 });
    let proposed = check("The owner wants the tests run first.", &["journal:2"]);
    assert_eq!(proposed.verdict, Verdict::Proposed { depth: 1 });
}

#[test]
fn the_depth_cap_holds_and_cycles_count_as_too_deep() {
    let entries = [
        insight(1, &["journal:1"]),
        insight(2, &["memory:1", "journal:2"]),
        insight(3, &["memory:2"]),
        insight(4, &["memory:5"]),
        insight(5, &["memory:4"]),
    ];
    let memory: HashMap<u64, &MemoryEntry> = entries.iter().map(|e| (e.id, e)).collect();
    assert_eq!(depth(Ref::Journal(1), &memory), 0);
    assert_eq!(depth(Ref::Memory(1), &memory), 1);
    assert_eq!(depth(Ref::Memory(3), &memory), 3);
    assert!(depth(Ref::Memory(4), &memory) > DEPTH_MAX, "a cycle");

    let journal: BTreeSet<usize> = [1, 2].into();
    let shown: Vec<Record> = entries
        .iter()
        .map(|e| Record {
            reference: Ref::Memory(e.id),
            body: Body::Memory(e.clone()),
        })
        .collect();
    let screen = secret_screen::Screen::shapes();
    let context = Context {
        agent: "alice",
        journal: &journal,
        memory: &memory,
        shown: &shown,
        screen: &screen,
    };
    let mut jev = Answers(BTreeMap::from([("A tree.".to_string(), (0.9, 0.0))]));
    let third = check(&written("A tree.", &["memory:2"]), &context, &mut jev);
    assert_eq!(third.verdict, Verdict::Stored { depth: 3 });
    let fourth = check(
        &written("A tree.", &["memory:3", "memory:1"]),
        &context,
        &mut jev,
    );
    assert_eq!(
        reason(&fourth),
        "its reflection depth 4 is over the cap of 3"
    );
}

#[test]
fn replies_parse_within_their_bounds() {
    let fenced = "```json\n{\"questions\": [\"a?\", \" \", \"b?\", \"c?\", \"d?\"]}\n```";
    assert_eq!(parse_questions(fenced).unwrap(), ["a?", "b?", "c?"]);
    assert!(parse_questions("{\"questions\": []}").is_err());
    assert!(parse_questions("no JSON here").is_err());
    let many: Vec<serde_json::Value> = (0..7)
        .map(|i| serde_json::json!({"text": format!("i{i}"), "because": ["journal:1"]}))
        .collect();
    let reply = serde_json::json!({ "insights": many }).to_string();
    assert_eq!(parse_insights(&reply).unwrap().len(), INSIGHTS);
    let set = support_set();
    assert_eq!(set.id, "openagents.insight-support.v1");
    assert_eq!(set.gate, SUPPORTED);
    assert!((threshold(SUPPORTED) - 0.7).abs() < 1e-9);
    assert!((threshold(PREFERENCE) - 0.5).abs() < 1e-9);
    let cited = record(4, "cargo test -p gym exited 101");
    assert!(verify_request("alice", "The gym tests failed once.", &[&cited]).is_ok());
    let state = verify_state("alice", "The gym tests failed once.", &[&cited]);
    assert_eq!(state["cited"][0]["reference"], "journal:4");
    assert_eq!(state["cited"][0]["kind"], "report");
}

fn store(dir: &tempfile::TempDir) -> Store {
    let store = Store::new(&dir.path().join("host"), "alice").unwrap();
    store.open(dir.path(), 1).unwrap();
    store
}

fn services(replies: Vec<String>, answers: &[(&str, f64, f64)]) -> Services {
    Services {
        writer: Box::new(Replies(replies)),
        verify: Box::new(Answers(
            answers
                .iter()
                .map(|(text, s, p)| ((*text).to_string(), (*s, *p)))
                .collect(),
        )),
        recall: agent_recall::Services::offline(),
    }
}

#[test]
fn a_preference_shaped_insight_waits_for_the_owner_and_the_run_is_journaled() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(&dir);
    let memory = Memory::new(store.clone(), secret_screen::Screen::shapes());
    for (at, kind, text) in [
        (100, Kind::Request, "commit the gym fix"),
        (
            110,
            Kind::Report,
            "I committed the gym fix with a short imperative message.",
        ),
        (120, Kind::Task, "task t-7 merged by the owner"),
        (
            130,
            Kind::Report,
            "I committed the atif fix with a short imperative message.",
        ),
    ] {
        store.append(&Entry::new(at, kind, text)).unwrap();
    }
    let positions: Vec<usize> = store
        .journal_rows()
        .unwrap()
        .iter()
        .map(|(p, _)| *p)
        .collect();
    let cite = |n: usize| format!("journal:{}", positions[n]);
    let preference = "The owner wants short imperative commit messages.";
    let fact = "Task t-7 merged after the gym fix was committed.";
    let reply = serde_json::json!({ "insights": [
        {"text": preference, "because": [cite(1), cite(3)]},
        {"text": fact, "because": [cite(1), cite(2)]},
        {"text": "The owner merges everything.", "because": [cite(2)]},
    ]})
    .to_string();
    let mut services = services(
        vec![
            "{\"questions\": [\"How does the owner want commits written?\"]}".into(),
            reply,
        ],
        &[
            (preference, 0.9, 0.95),
            (fact, 0.85, 0.05),
            ("The owner merges everything.", 0.1, 0.2),
        ],
    );
    let screen = secret_screen::Screen::shapes();
    let (reflection, applied) = memory
        .reflect(&mut services, &screen, "nightly", 200)
        .unwrap();
    assert_eq!(applied.stored.len(), 1);
    assert_eq!(applied.proposed.len(), 1);
    assert_eq!(applied.dropped, 1);
    assert_eq!(reflection.usd(), Some(0.02));
    assert_eq!(reflection.models, ["fake-writer"]);

    let entries = memory.entries().unwrap();
    let candidate = entries
        .iter()
        .find(|e| e.id == applied.proposed[0])
        .unwrap();
    assert_eq!(candidate.kind, MemoryKind::Preference);
    assert_eq!(candidate.state, MemoryState::Candidate);
    assert_eq!(candidate.sources, [cite(1), cite(3)]);
    let stored = entries.iter().find(|e| e.id == applied.stored[0]).unwrap();
    assert_eq!(stored.kind, MemoryKind::Insight);
    assert_eq!(stored.sources, [cite(1), cite(2)]);
    // The candidate shows where candidates show, at F2.
    let rows = memory.rows(None).unwrap();
    assert!(
        rows.iter()
            .any(|r| r.id == candidate.id && r.state == "candidate")
    );

    // No briefing carries the candidate; the stored insight is a record.
    let ask = "how should I write the commit message?";
    let recall = memory
        .recall(ask, "/w", 300, &mut agent_recall::Services::offline())
        .unwrap();
    assert!(!recall.text.contains(preference), "{}", recall.text);
    assert!(!recall.carried.contains(&Ref::Memory(candidate.id)));
    assert!(recall.carried.contains(&Ref::Memory(stored.id)));
    let (overlap, ids) = memory.briefing(ask, "/w").unwrap();
    assert!(!overlap.contains(preference) && !ids.contains(&candidate.id));
    memory.decide(candidate.id, true, 310).unwrap();
    let recall = memory
        .recall(ask, "/w", 320, &mut agent_recall::Services::offline())
        .unwrap();
    assert!(recall.carried.contains(&Ref::Memory(candidate.id)));

    // The journal holds the question with what it showed, each insight's
    // fate, and the run record.
    let journal = store.journal(100).unwrap();
    let texts: Vec<&str> = journal.iter().map(|e| e.text.as_str()).collect();
    let has = |needle: &str| texts.iter().any(|t| t.contains(needle));
    assert!(has(
        "reflection question 1: How does the owner want commits written?; shown "
    ));
    assert!(has(&format!(
        "insight entry {} (question 1, depth 1) cites {}, {}",
        stored.id,
        cite(1),
        cite(2)
    )));
    assert!(has("proposed from an insight and waiting for the owner"));
    assert!(has(
        "dropped an unverified insight (question 1): the cited records don't support it"
    ));
    let run = journal
        .iter()
        .rev()
        .find(|e| e.text.starts_with(RUN_PREFIX))
        .unwrap();
    assert_eq!(
        run.text,
        "reflection run (nightly): 1 questions from the 5 newest records, 15 shown each; model \
         fake-writer; cost $0.0200; stored 1, proposed 1, dropped 1"
    );
    assert_eq!(last_reflection(&store.journal_rows().unwrap()), Some(200));
}

#[test]
fn a_failed_insight_call_is_recorded_and_unknown_cost_is_never_zero() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(&dir);
    store
        .append(&Entry::new(100, Kind::Report, "the atif tests pass"))
        .unwrap();
    let memory = Memory::new(store.clone(), secret_screen::Screen::shapes());
    let mut services = services(vec!["{\"questions\": [\"q1?\", \"q2?\"]}".into()], &[]);
    let screen = secret_screen::Screen::shapes();
    let (reflection, _) = memory
        .reflect(&mut services, &screen, "early", 200)
        .unwrap();
    assert_eq!(reflection.asked.len(), 2);
    assert!(reflection.asked.iter().all(|a| a.failed.is_some()));
    let mut unmetered = reflection.clone();
    unmetered.unmetered = 1;
    assert_eq!(unmetered.usd(), None);
    assert!(
        unmetered
            .run_record()
            .contains("cost unknown (1 calls reported none")
    );
    let journal = store.journal(50).unwrap();
    assert!(
        journal
            .iter()
            .any(|e| e.text == "reflection question 2 wrote no insights: no reply left")
    );
    // A reflection over nothing doesn't run.
    let empty = tempfile::tempdir().unwrap();
    let nothing = Memory::new(
        Store::new(&empty.path().join("host"), "bob").unwrap(),
        secret_screen::Screen::shapes(),
    );
    assert!(
        nothing
            .reflect(&mut services, &screen, "nightly", 5)
            .is_err()
    );
}

#[test]
fn a_recorded_reflection_over_the_phase_a_fixture_checks_every_insight() {
    let fixture = Fixture::alice_v1().unwrap();
    let script = Script::alice_v1().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let (reflection, insights) = reflect_fixture(&fixture, dir.path()).unwrap();
    assert_eq!(reflection.asked.len(), script.questions.len());
    assert_eq!(reflection.read, NEWEST);
    for (asked, scripted) in reflection.asked.iter().zip(&script.questions) {
        assert_eq!(asked.question, scripted.question);
        assert_eq!(asked.shown.len(), SHOWN);
        assert_eq!(asked.insights.len(), scripted.insights.len());
        for (insight, expected) in asked.insights.iter().zip(&scripted.insights) {
            match (&insight.verdict, expected.expect.as_str()) {
                (Verdict::Stored { depth: 1 }, "stored")
                | (Verdict::Proposed { depth: 1 }, "proposed") => {}
                (Verdict::Dropped(why), start) if why.starts_with(start) => {}
                (verdict, expect) => panic!("{}: {verdict:?}, expected {expect}", insight.text),
            }
        }
    }
    assert_eq!(insights.len(), 3, "{insights:?}");
    assert!(insights.iter().all(|e| e.id > 51 && e.at == script.at));
    assert!((reflection.usd().unwrap() - 0.048).abs() < 1e-9);
    let run = reflection.run_record();
    assert!(run.contains("model scripted-reflection-v1"), "{run}");
    assert!(run.ends_with("stored 3, proposed 1, dropped 3"), "{run}");
    let journal = std::fs::read_to_string(dir.path().join("agents/alice/journal.jsonl")).unwrap();
    assert!(journal.contains("dropped an unverified insight (question 1): it cites journal:999"));
    assert!(journal.contains("reflection run (recorded)"));
}

struct World;

impl Facts for World {
    fn head(&self, _: &Path) -> Option<String> {
        None
    }
    fn issues(&self, _: &str, _: &str) -> Result<(Vec<Open>, Vec<Pull>), String> {
        Ok((Vec::new(), Vec::new()))
    }
    fn capacity(&self) -> bool {
        true
    }
}

#[test]
fn the_reflect_job_fires_nightly_and_early_within_its_daily_bound() {
    // 2026-10-05 00:00 UTC.
    const MONDAY: u64 = 1_791_158_400;
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(&dir.path().join("host"), "alice").unwrap();
    let record = store.open(dir.path(), MONDAY).unwrap();
    let jobs = Jobs::new(store.clone());
    let job = agent_jobs::template("reflect", "", None, None, 0, MONDAY).unwrap();
    assert_eq!(job.trigger.word(), "reflect");
    jobs.add(job, MONDAY).unwrap();
    jobs.edit("reflect", Edit::On, MONDAY).unwrap();
    let tick = |at: u64| agent_jobs::tick(&store, &record, dir.path(), &World, at).unwrap();

    // 01:00: nothing has happened since it was turned on.
    assert!(tick(MONDAY + 3600).is_empty());
    // 03:00: the nightly slot.
    let fired = tick(MONDAY + 3 * 3600 + 60);
    assert_eq!(fired.len(), 1);
    assert_eq!(fired[0].reflect.as_deref(), Some("nightly"));
    // A record the nightly reflection saw.
    store
        .append(&Entry::new(
            MONDAY + 3 * 3600 + 30,
            Kind::Control,
            "the owner stopped alice",
        ))
        .unwrap();
    store
        .append(&Entry::new(
            MONDAY + 3 * 3600 + 90,
            Kind::Memory,
            &format!("{RUN_PREFIX} (nightly): test"),
        ))
        .unwrap();
    assert!(tick(MONDAY + 4 * 3600).is_empty());
    // Sixteen stops after it: 160 importance, over 150.
    for i in 0..16 {
        store
            .append(&Entry::new(
                MONDAY + 5 * 3600 + i,
                Kind::Control,
                "the owner stopped alice",
            ))
            .unwrap();
    }
    let early = tick(MONDAY + 6 * 3600);
    assert_eq!(early.len(), 1);
    assert!(
        early[0]
            .reflect
            .as_deref()
            .unwrap()
            .starts_with("early: importance 160")
    );
    // The importance is still there, but two early ones a day is the bound.
    assert_eq!(tick(MONDAY + 7 * 3600).len(), 1);
    assert!(tick(MONDAY + 8 * 3600).is_empty());
    // The next day's slot fires, and the count starts again.
    let next = tick(MONDAY + 86_400 + 3 * 3600 + 60);
    assert_eq!(next[0].reflect.as_deref(), Some("nightly"));
    assert_eq!(tick(MONDAY + 86_400 + 4 * 3600).len(), 1, "early again");

    let loaded = &jobs.load().unwrap()[0];
    assert_eq!(loaded.occurrences, 5);
    assert_eq!(loaded.budget.unmetered, 5);
    jobs.meter("reflect", Some(0.05)).unwrap();
    jobs.meter("reflect", None).unwrap();
    let metered = &jobs.load().unwrap()[0];
    assert_eq!(metered.budget.unmetered, 4);
    assert!((metered.budget.spent - 0.05).abs() < 1e-9);
}

/// The scripted reflection over the phase A fixture, as a reader sees it:
/// `cargo test -p coder --lib agent_reflect::tests::trace -- --ignored
/// --nocapture`.
#[test]
#[ignore = "prints the recorded reflection's trace"]
fn trace() {
    let fixture = Fixture::alice_v1().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let (reflection, _) = reflect_fixture(&fixture, dir.path()).unwrap();
    let store = Store::new(dir.path(), "alice").unwrap();
    let rows: BTreeMap<String, Record> = agent_recall::candidates(
        &store.journal_rows().unwrap(),
        &Memory::new(store.clone(), secret_screen::Screen::shapes())
            .entries()
            .unwrap(),
    )
    .into_iter()
    .map(|r| (r.reference.to_string(), r))
    .collect();
    println!(
        "Reflection at {} ({}), {} newest records read",
        reflection.at, reflection.trigger, reflection.read
    );
    for (n, asked) in reflection.asked.iter().enumerate() {
        println!("\nQuestion {}: {}\nShown:", n + 1, asked.question);
        for reference in &asked.shown {
            if let Some(record) = rows.get(&reference.to_string()) {
                print!("  {}", shown_line(record));
            }
        }
        println!("Insights:");
        for insight in &asked.insights {
            let verdict = match &insight.verdict {
                Verdict::Stored { depth } => format!("STORED as an insight, depth {depth}"),
                Verdict::Proposed { depth } => {
                    format!("PROPOSED as a preference candidate, depth {depth}")
                }
                Verdict::Dropped(why) => format!("DROPPED as unverified: {why}"),
            };
            let support = insight.support.as_ref().map_or_else(String::new, |s| {
                format!(
                    " [Jev supported {:.2}, preference {:.2}]",
                    s.supported, s.preference
                )
            });
            println!(
                "  - {}\n    because {}\n    {verdict}{support}",
                insight.text,
                insight.because.join(", ")
            );
        }
    }
    println!("\nRun record: {}", reflection.run_record());
    let journal = std::fs::read_to_string(dir.path().join("agents/alice/journal.jsonl")).unwrap();
    println!("\nJournal rows the reflection wrote:");
    for line in journal.lines().skip(fixture.journal.len()) {
        let entry: Entry = serde_json::from_str(line).unwrap();
        println!("  [{:?}] {}", entry.kind, entry.text);
    }
}
