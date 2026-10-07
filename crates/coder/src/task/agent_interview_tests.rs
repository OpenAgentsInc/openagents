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
    // The full arm carries the insights the recorded reflection stored;
    // the no-reflection arm has none to carry.
    let insights = |arm: &str| {
        rows.iter()
            .filter(|r| r["arm"] == arm)
            .flat_map(|r| r["carried"].as_array().unwrap())
            .filter_map(|c| c.as_str()?.strip_prefix("memory:")?.parse::<u64>().ok())
            .filter(|id| *id as usize > fixture.memory.len())
            .count()
    };
    assert_eq!(insights("no-reflection"), 0);
    assert!(insights("full") > 0);

    // The same arm, answerer, and items again is a repeat, not a trial.
    let mut arm = ArmName::NoMemory.build(&dir.path().join("scratch"));
    let again = run(
        &suite,
        &fixture,
        &items[..1],
        arm.as_mut(),
        &mut FromBriefing,
        None,
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
        None,
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
    fn respond(&mut self, system: &str, prompt: &str) -> Result<Response, String> {
        self.0.push(format!("{system}\n{prompt}"));
        Ok(("Task t-108 merged.".into(), "fake-model".into(), Some(0.01)))
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

fn v2() -> (Suite, Fixture) {
    (
        gi::alice_v2_suite().expect("the five-category suite loads"),
        Fixture::alice_v1().expect("the fixture loads"),
    )
}

#[test]
fn the_day_plan_reads_standing_jobs_and_waiting_tasks() {
    let (_, fixture) = v2();
    let ask = Ask {
        item_id: "plan/x",
        question: "What will you do at 2 AM tonight?",
        as_of: fixture.gym.manifest.as_of,
    };
    let plan = StandingJobs.plan(&fixture, &ask).unwrap();
    for line in [
        "- 02:00 nightly-check (Nightly checks): Run the nightly checks",
        "- 03:00 keep-green (Keep supervise green): Keep it green: cargo test -p supervise.",
        "- 08:00 watch-issues (Watch issues): Look at open issues labeled agent",
        "task t-110 waits at the Merge station",
    ] {
        assert!(plan.text.contains(line), "{line}\n{}", plan.text);
    }
    // Merged and rejected tasks no longer wait.
    assert!(!plan.text.contains("t-131"), "{}", plan.text);
    assert!(plan.carried.iter().all(|c| c.starts_with("journal:")));
    assert!(plan.carried.len() >= 4);
    // Before any job was added there's no plan.
    let early = Ask {
        as_of: fixture.journal[0].at + 60,
        ..ask
    };
    assert_eq!(
        StandingJobs.plan(&fixture, &early).unwrap(),
        Briefing::default()
    );
    // The no-reflection arm carries the plan; the arm without it doesn't.
    let scratch = tempfile::tempdir().unwrap();
    let with = ArmName::NoReflection
        .build(scratch.path())
        .brief(&fixture, &ask)
        .unwrap();
    let without = ArmName::NoReflectionOrPlan
        .build(scratch.path())
        .brief(&fixture, &ask)
        .unwrap();
    assert!(with.text.contains("Your plan for 2026-09-28"));
    assert!(!without.text.contains("Your plan for"));
}

#[test]
fn the_scripted_judge_reads_support_and_embellishment() {
    let (suite, fixture) = v2();
    let item = suite
        .items
        .iter()
        .find(|i| i.id == "reflection/commits")
        .unwrap();
    let evidence = judge::evidence(&fixture.gym, item).unwrap();
    let briefing = Briefing::default();
    let mut read = |answer: &str| {
        ScriptedJudge
            .judge(&judge::Case {
                agent: "alice",
                item,
                evidence: &evidence,
                briefing: &briefing,
                answer,
            })
            .unwrap()
    };
    let truth = read(&item.truth);
    assert!(truth.is_supported() && !truth.is_embellished(), "{truth:?}");
    let blank = read(DONT_REMEMBER);
    assert!(
        !blank.is_supported() && !blank.is_embellished(),
        "{blank:?}"
    );
    let invented = read("The owner wants every commit signed with a hardware key on Fridays.");
    assert!(invented.is_embellished(), "{invented:?}");
    assert_eq!(invented.judge, "scripted");
    assert_eq!(invented.set, "openagents.interview-answer.v1");

    // Jev's request for the same case fits the set's bound and names both
    // questions; nothing is sent.
    let request = judge::request(&judge::Case {
        agent: "alice",
        item,
        evidence: &evidence,
        briefing: &Briefing {
            text: "x".repeat(judge::BRIEFING_MAX * 2),
            carried: Vec::new(),
        },
        answer: &item.truth,
    })
    .unwrap();
    let text = format!("{request:?}");
    assert!(text.contains(judge::SUPPORTED) && text.contains(judge::EMBELLISHED));
    assert!(text.contains("reference_answer"));
}

#[test]
fn a_scripted_round_runs_all_five_arms_and_evaluates_the_gate() {
    let (suite, fixture) = v2();
    let dir = tempfile::tempdir().unwrap();
    let store = gym::store::Store::at(dir.path().join("interviews.jsonl"));
    let marks = gym::store::Store::at(dir.path().join("marks.jsonl"));
    let items = suite.partition(Partition::Development).unwrap();
    let scratch = dir.path().join("scratch");
    let spec = Round {
        group: "alice-interview-v2 development".into(),
        seed_base: 0,
        blocks: 3,
        scratch: &scratch,
        clock: &clock,
        marks: Some(&marks),
    };
    let report = round(
        &suite,
        &fixture,
        &items,
        &mut FromBriefing,
        &mut ScriptedJudge,
        &spec,
        &store,
    )
    .unwrap();

    assert_eq!(report.blocks, vec![0, 1, 2]);
    assert_eq!(report.rows, ArmName::ALL.len() * items.len() * 3);
    let arms: Vec<&str> = report.arms.iter().map(|a| a.arm.as_str()).collect();
    for name in ArmName::ALL {
        assert!(arms.contains(&name.as_str()), "{arms:?}");
    }
    for arm in &report.arms {
        assert_eq!(arm.total.rows, items.len() * 3, "{}", arm.arm);
        assert_eq!(arm.blocks, vec![0, 1, 2]);
        assert_eq!(arm.categories.len(), 5, "{}", arm.arm);
        assert!(arm.total.judged > 0, "{}", arm.arm);
    }
    let rows = store.verified_rows().unwrap();
    assert!(matches!(verify_chain(&rows), ChainVerdict::Ok { .. }));
    assert_eq!(
        report.head.as_deref(),
        rows.last().unwrap()["receipt"].as_str()
    );
    // Code checks memory and plans; the judge reads the rest.
    for row in gi::rows_of(&rows) {
        let checked = gi::Category::parse(&row.family).unwrap().code_checked();
        assert_eq!(row.correct.is_some(), checked, "{}", row.item_id);
        assert_eq!(row.judgment.is_some(), !checked, "{}", row.item_id);
    }
    // gym::ab compared the two arms on the code-checked families, and the
    // unmeasured block spread leaves it undecided.
    assert_eq!(report.ab.control, "arm:word-overlap");
    assert_eq!(report.ab.candidate, "arm:full");
    assert_eq!(report.ab.screening_round.drawn(), vec![0, 1, 2]);
    assert_eq!(report.ab.decision, gym::ab::Decision::Undecided);
    // The gate judged, and with no marks the judge is uncalibrated.
    assert_eq!(report.gate.gate_id, gi::GATE);
    assert_eq!(report.gate.gate_digest, gi::gate().unwrap().digest());
    assert_eq!(report.comparison.blocks, 3);
    let agreement = report
        .gate
        .criteria
        .iter()
        .find(|c| c.name == "judge_agrees_with_the_owners_marks")
        .unwrap();
    assert_eq!(agreement.verdict, gym::gate::Verdict::Unverifiable);
    assert!(render_round(&report).contains("gate interview-v1"));

    // The owner marks a sample the way the judge read it; the judge's
    // agreement then counts.
    let all = gi::rows_of(&rows);
    for row in gi::sample(&all, 5) {
        let judgment = row.judgment.as_ref().unwrap();
        marks
            .append(&gi::Mark::on(
                row,
                judgment.is_supported(),
                judgment.is_embellished(),
                "",
                clock(),
            ))
            .unwrap();
    }
    let measured = gi::agreement(&all, &gi::marks_of(&marks.verified_rows().unwrap()));
    assert_eq!(measured.matched, 5);
    assert_eq!(measured.rate(), Some(1.0));
    let outcome = gi::gate()
        .unwrap()
        .judge_interview(&gi::comparison("g", &all, Some(&measured)));
    let agreement = outcome
        .criteria
        .iter()
        .find(|c| c.name == "judge_agrees_with_the_owners_marks")
        .unwrap();
    assert_eq!(agreement.verdict, gym::gate::Verdict::Passed);
    // Marking the same answer again is a repeat.
    let again = gi::sample(&all, 1)[0];
    assert!(
        marks
            .append(&gi::Mark::on(again, true, false, "", clock()))
            .is_err()
    );
}

#[test]
fn the_live_answerer_stops_at_its_cost_cap() {
    let mut live = Live::new(Fake(Vec::new())).capped(0.015);
    let ask = Ask {
        item_id: "memory/x",
        question: "Which task merged?",
        as_of: 1_790_586_000,
    };
    let briefing = Briefing::default();
    assert!(live.answer("alice", &briefing, &ask).is_ok());
    assert!(live.answer("alice", &briefing, &ask).is_ok());
    let refused = live.answer("alice", &briefing, &ask).unwrap_err();
    assert!(refused.contains("cost cap"), "{refused}");
    assert_eq!(live.model.0.len(), 2);
}

#[test]
fn the_day_plan_drafts_from_insights_only_when_reflection_stored_them() {
    let fixture = Fixture::alice_v1().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let made = DayPlans::new(Some(scratch.path().join("reflect")))
        .made(&fixture)
        .unwrap();
    assert!(made.called);
    let sources: Vec<&str> = made.plan.blocks.iter().map(|b| b.source.as_str()).collect();
    assert_eq!(sources, ["memory:55"], "{:?}", made.rejected);
    assert_eq!(made.rejected.len(), 1);
    assert!(made.rejected[0].1.contains("wasn't offered"));
    // Without reflection there is nothing to draft from, and no call.
    let bare = DayPlans::new(None).made(&fixture).unwrap();
    assert!(!bare.called && bare.plan.idle());
    let ask = Ask {
        item_id: "plan/ten-am",
        question: "What will you do at 10 AM?",
        as_of: 1_790_586_000,
    };
    let full = ArmName::Full
        .build(scratch.path())
        .brief(&fixture, &ask)
        .unwrap();
    assert!(
        full.text.contains(
            "- 10:00-11:00 Rerun the verse release tests with a longer command bound [memory:55]"
        ),
        "{}",
        full.text
    );
    assert!(full.carried.iter().any(|c| c == "memory:55"));
    let no_reflection = ArmName::NoReflection
        .build(scratch.path())
        .brief(&fixture, &ask)
        .unwrap();
    assert!(no_reflection.text.contains("Your plan for 2026-09-28"));
    assert!(!no_reflection.text.contains("Your other blocks"));
}
