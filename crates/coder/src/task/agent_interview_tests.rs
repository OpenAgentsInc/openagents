use std::collections::BTreeMap;

use gym::store::{ChainVerdict, verify_chain};

use super::*;

fn clock() -> String {
    "2026-10-06T12:00:00Z".into()
}

fn suite_and_fixture() -> (Suite, Fixture) {
    (
        gi::alice_v1_suite().expect("the suite loads"),
        Fixture::alice_v1().expect("the fixture loads and passes the screen"),
    )
}

#[test]
fn the_fixture_reads_as_the_agents_own_types_and_passes_the_screen() {
    let (_, fixture) = suite_and_fixture();
    assert_eq!(fixture.agent(), "alice");
    assert_eq!(fixture.journal.len(), fixture.gym.manifest.journal_rows);
    assert_eq!(fixture.memory.len(), fixture.gym.manifest.memory_entries);
    assert!(fixture.journal.windows(2).all(|w| w[0].at <= w[1].at));
    for kind in [
        super::super::agent::Kind::Request,
        super::super::agent::Kind::Ran,
        super::super::agent::Kind::Failed,
        super::super::agent::Kind::Task,
        super::super::agent::Kind::Rejected,
        super::super::agent::Kind::Memory,
        super::super::agent::Kind::Job,
        super::super::agent::Kind::Plan,
    ] {
        assert!(fixture.journal.iter().any(|e| e.kind == kind), "{kind:?}");
    }
    let states: BTreeSet<&str> = fixture.memory.iter().map(|e| e.state.word()).collect();
    assert_eq!(
        states,
        BTreeSet::from(["active", "candidate", "rejected"]),
        "accepted, candidate, and rejected preferences"
    );
    // A credential-shaped line is refused.
    let mut tainted = fixture.gym.clone();
    tainted.journal.push_str(
        "{\"schema\":\"openagents.agent-journal-entry.v1\",\"at\":1,\"kind\":\"ran\",\
         \"text\":\"export OPENAI_API_KEY=sk-proj-abcdefghijklmnopqrstuvwxyz0123456789\"}\n",
    );
    assert!(Fixture::parse(tainted).is_err());
}

#[test]
fn both_arms_run_through_the_scripted_answerer_into_a_verified_chain() {
    let (suite, fixture) = suite_and_fixture();
    let dir = tempfile::tempdir().unwrap();
    let store = gym::store::Store::at(dir.path().join("interviews.jsonl"));
    let mut items = suite.partition(Partition::Calibration).unwrap();
    items.extend(suite.partition(Partition::Development).unwrap());
    let mut summaries = BTreeMap::new();
    for name in ArmName::ALL {
        let mut arm = name.build(&dir.path().join("scratch"));
        let summary = run(
            &suite,
            &fixture,
            &items,
            arm.as_mut(),
            &mut FromBriefing,
            None,
            &store,
            &clock,
        )
        .unwrap();
        assert_eq!(summary.rows, items.len());
        assert_eq!(summary.checked, items.len());
        summaries.insert(name.as_str(), summary);
    }
    let none = &summaries["no-memory"];
    let overlap = &summaries["word-overlap"];
    assert_eq!(none.correct, 0, "an empty briefing recalls nothing");
    assert_eq!(none.evidence, 0);
    assert!(overlap.correct > 0, "{overlap:?}");
    assert!(overlap.evidence > 0, "{overlap:?}");
    assert!(
        overlap.correct < items.len(),
        "today's briefing misses the journal-only facts"
    );

    let rows = store.verified_rows().unwrap();
    assert_eq!(rows.len(), ArmName::ALL.len() * items.len());
    assert!(matches!(verify_chain(&rows), ChainVerdict::Ok { .. }));
    for row in &rows {
        assert_eq!(row["schema"], gi::ROW_SCHEMA);
        assert_eq!(row["suite_digest"], suite.digest.as_str());
        assert_eq!(row["fixture_digest"], fixture.gym.manifest.digest.as_str());
        assert!(row["correct"].is_boolean());
    }
    let carried: Vec<&serde_json::Value> = rows
        .iter()
        .filter(|r| r["arm"] == "word-overlap")
        .flat_map(|r| r["carried"].as_array().unwrap())
        .collect();
    assert!(
        carried
            .iter()
            .all(|c| c.as_str().unwrap().starts_with("memory:"))
    );

    // The same arm, answerer, and items again is a repeat, not a trial.
    let mut arm = ArmName::NoMemory.build(&dir.path().join("scratch"));
    let again = run(
        &suite,
        &fixture,
        &items[..1],
        arm.as_mut(),
        &mut FromBriefing,
        None,
        &store,
        &clock,
    );
    assert!(again.unwrap_err().contains("perturbation"));
    let trial = run(
        &suite,
        &fixture,
        &items[..1],
        arm.as_mut(),
        &mut FromBriefing,
        Some(2),
        &store,
        &clock,
    )
    .unwrap();
    assert_eq!(trial.rows, 1);
}

#[test]
fn the_canned_truths_grade_right_and_the_briefing_sees_only_the_past() {
    let (suite, fixture) = suite_and_fixture();
    let dir = tempfile::tempdir().unwrap();
    let store = gym::store::Store::at(dir.path().join("rows.jsonl"));
    let items = suite.partition(Partition::Development).unwrap();
    let mut canned = Canned(
        items
            .iter()
            .map(|item| (item.id.clone(), item.truth.clone()))
            .collect(),
    );
    let mut arm = NoMemory;
    let summary = run(
        &suite,
        &fixture,
        &items,
        &mut arm,
        &mut canned,
        None,
        &store,
        &clock,
    )
    .unwrap();
    assert_eq!(summary.correct, items.len());

    let mut overlap = ArmName::WordOverlap.build(dir.path());
    let early = Ask {
        item_id: "x",
        question: "What did the owner ask you to remember about Jev thresholds?",
        as_of: fixture.journal[0].at + 1,
    };
    let briefing = overlap.brief(&fixture, &early).unwrap();
    assert!(briefing.text.is_empty(), "{}", briefing.text);
    let later = Ask {
        as_of: fixture.gym.manifest.as_of,
        ..early
    };
    let briefing = overlap.brief(&fixture, &later).unwrap();
    assert!(briefing.text.contains("measurement document"));
    // Candidates and rejected preferences never reach a briefing.
    assert!(!briefing.text.contains("cargo fmt"));
    assert!(!briefing.text.contains("full workspace"));
}

#[test]
fn locked_items_need_a_recorded_spend() {
    let (suite, _) = suite_and_fixture();
    assert!(items(&suite, Partition::Locked, None).is_err());
    let dir = tempfile::tempdir().unwrap();
    let ledger = LockedLedger::at(dir.path().join("ledger.jsonl"));
    let spend = Spend {
        subject: "test",
        reason: "a test of the read",
        at: "2026-10-06T12:00:00Z",
        adapter: "",
    };
    let locked = items(&suite, Partition::Locked, Some((&ledger, &spend))).unwrap();
    assert!(!locked.is_empty());
    assert_eq!(ledger.reads().unwrap().len(), 1);
    assert!(items(&suite, Partition::Locked, Some((&ledger, &spend))).is_err());
}

struct Fake(Vec<String>);

impl Respond for Fake {
    fn respond(&mut self, system: &str, prompt: &str) -> Result<(String, String), String> {
        self.0.push(format!("{system}\n{prompt}"));
        Ok(("Task t-108 merged.".into(), "fake-model".into()))
    }
}

#[test]
fn the_live_answerer_prompts_with_the_briefing_and_records_its_model() {
    let mut live = Live::new(Fake(Vec::new()));
    let briefing = Briefing {
        text: "- (outcome) task t-108 merged by the owner at the Merge station\n".into(),
        carried: vec!["memory:9".into()],
    };
    let ask = Ask {
        item_id: "memory/x",
        question: "Which task merged?",
        as_of: 1_790_586_000,
    };
    let answer = live.answer("alice", &briefing, &ask).unwrap();
    assert_eq!(answer, "Task t-108 merged.");
    assert_eq!(live.identity().model, "fake-model");
    let seen = &live.model.0[0];
    assert!(seen.contains("You are alice"));
    assert!(seen.contains("task t-108 merged by the owner"));
    assert!(seen.contains("Question: Which task merged?"));
    assert!(seen.contains("2026-09-28T09:00:00Z"));
    let (_, empty) = prompt("alice", &Briefing::default(), &ask);
    assert!(empty.contains("(empty)"));
}
