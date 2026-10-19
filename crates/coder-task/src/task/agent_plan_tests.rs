use coder_host::access::day_plan::{By, Replanned};

use super::super::agent_jobs::{self, Job};
use super::super::agent_memory::{Author, MemoryEntry, MemoryKind, MemoryState};
use super::*;

// 2026-10-05 00:00 UTC, a Monday.
const MONDAY: u64 = 1_791_158_400;
const HOUR: u64 = 3600;

fn on(name: &str) -> Job {
    let mut job = agent_jobs::template(name, "", Some("o/r"), None, 0, MONDAY - 86_400).unwrap();
    job.enabled = true;
    job
}

fn insight(id: u64, text: &str) -> MemoryEntry {
    MemoryEntry {
        schema: super::super::agent_memory::SCHEMA.into(),
        v: 1,
        requires: Vec::new(),
        id,
        kind: MemoryKind::Insight,
        state: MemoryState::Active,
        author: Author::Agent,
        text: text.into(),
        at: MONDAY - HOUR,
        sources: vec!["journal:3".into()],
    }
}

struct Day {
    jobs: Vec<Job>,
    issues: Vec<(u64, String)>,
    queued: Vec<(String, String)>,
    memory: Vec<MemoryEntry>,
    known: Known,
}

impl Day {
    fn new() -> Self {
        Self {
            jobs: Vec::new(),
            issues: Vec::new(),
            queued: Vec::new(),
            memory: Vec::new(),
            known: Known::new("alice", world_tree::everglade()),
        }
    }

    fn inputs(&self, now: u64) -> Inputs<'_> {
        Inputs {
            agent: "alice",
            now,
            utc_offset: 0,
            jobs: &self.jobs,
            issues: &self.issues,
            queued: &self.queued,
            memory: &self.memory,
            tree: world_tree::everglade(),
            known: &self.known,
            bound: HOUSE,
        }
    }
}

fn screen() -> secret_screen::Screen {
    secret_screen::Screen::shapes()
}

const CONSOLE: &str = "everglade/knowledge-district/owners-house/great-room/console";
const LECTERN: &str = "everglade/knowledge-district/owners-house/great-room/lectern";

#[test]
fn local_dates_and_minutes_follow_the_offset() {
    assert_eq!(local(MONDAY + 7 * HOUR, 0), ("2026-10-05".into(), 420));
    assert_eq!(local(MONDAY + HOUR, -120), ("2026-10-04".into(), 23 * 60));
    assert_eq!(local(0, 0), ("1970-01-01".into(), 0));
}

#[test]
fn an_idle_day_stays_idle_with_no_model_call() {
    let day = Day::new();
    let mut writer = Scripted::default();
    let made = draft(&day.inputs(MONDAY + 7 * HOUR), &mut writer, &screen()).unwrap();
    assert!(!made.called && writer.prompts.is_empty());
    assert!(made.plan.idle());
    assert_eq!(
        made.plan.lines(),
        ["2026-10-05: no work today, idle at her desk"]
    );
    // An idle day's events change nothing but an owner's request.
    let mut plan = made.plan;
    let places = places(world_tree::everglade(), &day.known, HOUSE);
    let mut judge = Answers::default();
    judge.answers.push_back(Ok(Reaction::Continue));
    let event = Event::Observed {
        source: "job:keep-green".into(),
        text: "the default branch moved".into(),
    };
    let reacted = react(&mut plan, &event, MONDAY + 9 * HOUR, &places, &mut judge);
    assert!(!reacted.replanned && plan.idle());
}

#[test]
fn a_standing_job_lands_in_its_slot_without_a_model_call() {
    let mut day = Day::new();
    day.jobs = vec![on("nightly-check"), on("reflect"), on("plan")];
    // A job that is off isn't work.
    let mut off = on("keep-green");
    off.enabled = false;
    day.jobs.push(off);
    // The script holds no reply: a call would fail the draft.
    let mut writer = Scripted::default();
    let made = draft(&day.inputs(MONDAY + 7 * HOUR), &mut writer, &screen()).unwrap();
    assert!(!made.called);
    let blocks = &made.plan.blocks;
    assert_eq!(blocks.len(), 2, "{blocks:?}");
    assert_eq!(
        (blocks[0].start, blocks[0].source.as_str(), blocks[0].by),
        (120, "job:nightly-check", By::Code)
    );
    assert_eq!(blocks[0].node, CONSOLE, "a check runs commands");
    assert_eq!(
        (blocks[1].start, blocks[1].source.as_str()),
        (180, "job:reflect")
    );
    assert_eq!(blocks[1].node, DESK);
}

#[test]
fn every_block_names_a_real_source_and_a_known_place() {
    let mut day = Day::new();
    day.jobs = vec![on("nightly-check")];
    day.issues = vec![
        (7, "Fix the relay timeout".into()),
        (8, "Docs".into()),
        (9, "Flaky test".into()),
        (10, "Lint".into()),
    ];
    day.queued = vec![("request:1".into(), "update the changelog".into())];
    day.memory = vec![insight(12, "The owner reviews changes after lunch.")];
    let reply = serde_json::json!({"blocks": [
        {"source": "issue:7", "node": DESK, "start": "09:00", "minutes": 90, "title": "Work issue 7"},
        {"source": "request:1", "node": CONSOLE, "start": "11:00", "minutes": 30, "title": "Update the changelog"},
        {"source": "memory:12", "node": LECTERN, "start": "13:00", "minutes": 30, "title": "Bring changes for review"},
        // Invented work, a stranger's place, a duplicate, an overlap, the
        // past, and a scheduled job the model may not move.
        {"source": "issue:99", "node": DESK, "start": "14:00", "minutes": 30, "title": "Invented"},
        {"source": "issue:8", "node": "everglade/commons/workshop-hall/hall/desk-1", "start": "15:00", "minutes": 30, "title": "Elsewhere"},
        {"source": "issue:7", "node": DESK, "start": "16:00", "minutes": 30, "title": "Again"},
        {"source": "issue:9", "node": DESK, "start": "09:30", "minutes": 30, "title": "Overlap"},
        {"source": "issue:10", "node": DESK, "start": "05:00", "minutes": 30, "title": "Past"},
        {"source": "job:nightly-check", "node": CONSOLE, "start": "17:00", "minutes": 30, "title": "Moved job"},
    ]})
    .to_string();
    let mut writer = Scripted::new([reply]);
    let inputs = day.inputs(MONDAY + 7 * HOUR);
    let made = draft(&inputs, &mut writer, &screen()).unwrap();
    assert!(made.called);
    let prompt = &writer.prompts[0];
    for id in ["[issue:7]", "[request:1]", "[memory:12]"] {
        assert!(prompt.contains(id), "{prompt}");
    }
    let (listed, fixed) = prompt.split_once("These blocks are fixed").unwrap();
    assert!(!listed.contains("[job:nightly-check]"), "jobs are code's");
    assert!(fixed.contains("02:00-02:30 Nightly check [job:nightly-check]"));
    let ids = source_ids(&sources(&inputs));
    let plan = &made.plan;
    assert_eq!(plan.blocks.len(), 4, "{:?}", made.rejected);
    assert!(plan.blocks.iter().all(|b| ids.contains(&b.source)));
    assert!(plan.blocks.windows(2).all(|w| w[0].end <= w[1].start));
    assert_eq!(made.rejected.len(), 6);
    let why: Vec<&str> = made.rejected.iter().map(|(_, w)| w.as_str()).collect();
    assert!(why[0].contains("wasn't offered"));
    assert!(why[1].contains("within her bound"));
    assert!(why[2].contains("planned already"));
    assert!(why[3].contains("overlaps"));
    assert!(why[4].contains("before now"));
    assert!(
        why[5].contains("wasn't offered"),
        "a scheduled job is code's"
    );
    assert!(plan.validate().is_ok());
}

#[test]
fn a_wider_bound_offers_the_places_she_knows_there() {
    let tree = world_tree::everglade();
    let mut known = Known::new("alice", tree);
    let library = "everglade/commons/workshop-hall/hall/library";
    assert!(
        !places(tree, &known, "everglade")
            .iter()
            .any(|n| n.id == library)
    );
    known.enter(tree, "everglade/commons/workshop-hall/hall");
    assert!(
        places(tree, &known, "everglade")
            .iter()
            .any(|n| n.id == library)
    );
    // The house bound never offers it.
    assert!(!places(tree, &known, HOUSE).iter().any(|n| n.id == library));
}

fn morning() -> (Day, DayPlan) {
    let mut day = Day::new();
    day.jobs = vec![on("nightly-check")];
    day.jobs[0].trigger = agent_jobs::Trigger::Schedule {
        at: "12:00".into(),
        weekday: None,
        utc_offset: 0,
    };
    day.issues = vec![(7, "Fix the relay timeout".into()), (8, "Docs".into())];
    let reply = serde_json::json!({"blocks": [
        {"source": "issue:7", "node": DESK, "start": "09:00", "minutes": 120, "title": "Work issue 7"},
        {"source": "issue:8", "node": DESK, "start": "13:00", "minutes": 60, "title": "Work issue 8"},
    ]})
    .to_string();
    let mut writer = Scripted::new([reply]);
    let plan = draft(&day.inputs(MONDAY + 7 * HOUR), &mut writer, &screen())
        .unwrap()
        .plan;
    (day, plan)
}

#[test]
fn only_the_block_under_way_is_decomposed_when_it_starts() {
    let (_, mut plan) = morning();
    assert_eq!(begin(&mut plan, 8 * 60), None, "nothing at 08:00");
    assert_eq!(begin(&mut plan, 9 * 60 + 5), Some(0));
    assert_eq!(begin(&mut plan, 9 * 60 + 30), None, "it started already");
    let reply = serde_json::json!({"steps": [
        {"start": "09:05", "minutes": 10, "text": "Read issue 7 and its thread"},
        {"start": "09:15", "minutes": 15, "text": "Reproduce the timeout"},
        {"start": "09:20", "minutes": 10, "text": "Overlaps, dropped"},
        {"start": "09:30", "minutes": 40, "text": "Too long, dropped"},
        {"start": "09:30", "minutes": 15, "text": "Write the failing test"},
    ]})
    .to_string();
    let mut writer = Scripted::new([reply]);
    let (steps, _) = decompose(
        &plan,
        "Fix the relay timeout",
        9 * 60 + 5,
        &mut writer,
        &screen(),
    )
    .unwrap();
    assert_eq!(writer.prompts.len(), 1);
    assert!(
        writer.prompts[0].contains("09:05-10:05"),
        "{}",
        writer.prompts[0]
    );
    let texts: Vec<&str> = steps.iter().map(|s| s.text.as_str()).collect();
    assert_eq!(
        texts,
        [
            "Read issue 7 and its thread",
            "Reproduce the timeout",
            "Write the failing test"
        ]
    );
    plan.steps = steps;
    // The next block starting drops the last one's steps.
    assert_eq!(begin(&mut plan, 12 * 60), Some(1));
    assert!(plan.steps.is_empty());
}

#[test]
fn the_owners_request_interrupts_and_replans_from_the_current_block() {
    let (day, mut plan) = morning();
    begin(&mut plan, 9 * 60 + 30);
    let places = places(world_tree::everglade(), &day.known, HOUSE);
    // The judge is never asked.
    let mut judge = Answers::default();
    let event = Event::Owner {
        source: "request:1".into(),
        text: "look at the failing deploy".into(),
    };
    let reacted = react(&mut plan, &event, MONDAY + 10 * HOUR, &places, &mut judge);
    assert_eq!(
        reacted,
        Reacted {
            reaction: Reaction::ReactNow,
            by: "code".into(),
            replanned: true
        }
    );
    assert!(judge.asked.is_empty());
    let current = plan.current_block().unwrap();
    assert_eq!(
        (current.start, current.source.as_str(), current.by),
        (600, "request:1", By::Owner)
    );
    let at = |source: &str| -> Vec<(u32, u32)> {
        plan.blocks
            .iter()
            .filter(|b| b.source == source)
            .map(|b| (b.start, b.end))
            .collect()
    };
    // Issue 7 stopped at 10:00 and its last hour follows the request.
    assert_eq!(at("issue:7"), [(540, 600), (630, 690)]);
    // The scheduled job keeps its slot; issue 8 keeps its time.
    assert_eq!(at("job:nightly-check"), [(720, 750)]);
    assert_eq!(at("issue:8"), [(780, 840)]);
    assert_eq!(plan.replans.len(), 1);
    assert_eq!(plan.replans[0].kind, Replanned::Interrupt);
    assert!(plan.validate().is_ok());
}

#[test]
fn a_scheduled_job_fires_in_its_slot_and_jev_decides_the_rest() {
    let (day, mut plan) = morning();
    begin(&mut plan, 9 * 60 + 30);
    let places = places(world_tree::everglade(), &day.known, HOUSE);
    let mut judge = Answers::default();
    let fired = Event::Job {
        job: "nightly-check".into(),
        text: "Nightly check".into(),
    };
    let reacted = react(&mut plan, &fired, MONDAY + 12 * HOUR, &places, &mut judge);
    assert_eq!(reacted.by, "code");
    assert!(!reacted.replanned && judge.asked.is_empty());
    assert_eq!(plan.current_block().unwrap().source, "job:nightly-check");
    // Jev defers a watched issue to after the current block.
    judge.answers.push_back(Ok(Reaction::Defer));
    let observed = Event::Observed {
        source: "issue:9".into(),
        text: "Issue #9 came free: flaky test".into(),
    };
    let reacted = react(
        &mut plan,
        &observed,
        MONDAY + 12 * HOUR + 600,
        &places,
        &mut judge,
    );
    assert_eq!(
        (reacted.reaction, reacted.by.as_str()),
        (Reaction::Defer, "jev")
    );
    assert_eq!(judge.asked[0]["current"]["source"], "job:nightly-check");
    assert_eq!(judge.asked[0]["event"]["source"], "issue:9");
    let deferred = plan.blocks.iter().find(|b| b.source == "issue:9").unwrap();
    assert_eq!((deferred.start, deferred.by), (750, By::Reaction));
    assert_eq!(plan.current_block().unwrap().source, "job:nightly-check");
    // Issue 8 moved past it.
    let eight = plan.blocks.iter().find(|b| b.source == "issue:8").unwrap();
    assert_eq!(eight.start, 780);
    // With no answer, she continues and nothing changes.
    let before = plan.blocks.clone();
    let reacted = react(
        &mut plan,
        &observed,
        MONDAY + 12 * HOUR + 900,
        &places,
        &mut judge,
    );
    assert_eq!(reacted.reaction, Reaction::Continue);
    assert!(reacted.by.starts_with("code ("));
    assert_eq!(plan.blocks, before);
    assert_eq!(plan.replans.len(), 1);
}

#[test]
fn a_replan_drops_what_no_longer_fits_and_says_so() {
    let (_, mut plan) = morning();
    let late = Block {
        start: 23 * 60,
        end: 23 * 60 + 55,
        title: "Late".into(),
        source: "issue:8".into(),
        node: DESK.into(),
        by: By::Model,
    };
    plan.blocks.retain(|b| b.source != "issue:8");
    plan.blocks.push(late);
    let inserted = Block {
        start: 22 * 60 + 40,
        end: 23 * 60 + 10,
        title: "Urgent".into(),
        source: "request:2".into(),
        node: DESK.into(),
        by: By::Owner,
    };
    replan(
        &mut plan,
        MONDAY + 22 * HOUR + 40 * 60,
        22 * 60 + 40,
        inserted,
        Replanned::Interrupt,
        "the owner asked",
    );
    assert!(!plan.blocks.iter().any(|b| b.title == "Late"));
    assert_eq!(plan.replans[0].dropped, ["Late"]);
}

#[test]
fn a_seat_plan_is_its_task_queue_with_no_model() {
    use coder_host::access::studio::{Task, TaskStatus, View};
    let task = |id: &str, seat: &str, position, status| Task {
        task: id.into(),
        goal: "g".into(),
        entry: id.into(),
        position,
        title: format!("Task {id}"),
        seat: seat.into(),
        depends_on: Vec::new(),
        status,
        spend: Default::default(),
    };
    let view = View {
        tasks: vec![
            task("t3", "ada", 3, TaskStatus::Queued),
            task("t1", "ada", 1, TaskStatus::Done),
            task("t2", "ada", 2, TaskStatus::Running),
            task("t4", "bob", 1, TaskStatus::Queued),
        ],
        ..View::default()
    };
    let desk = "everglade/commons/workshop-hall/hall/desk-1";
    let plan = seat_plan(&view, "ada", desk, MONDAY + 9 * HOUR, 0);
    let sources: Vec<&str> = plan.blocks.iter().map(|b| b.source.as_str()).collect();
    assert_eq!(sources, ["task:t2", "task:t3"]);
    assert!(
        plan.blocks
            .iter()
            .all(|b| b.by == By::Studio && b.node == desk)
    );
    assert_eq!(plan.current, Some(0));
    assert!(plan.validate().is_ok());
}

#[test]
fn a_plan_persists_and_its_job_fires_once_a_morning() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(&dir.path().join("host"), "alice").unwrap();
    let record = store.open(dir.path(), 1).unwrap();
    let (_, plan) = morning();
    assert_eq!(load(&store).unwrap(), None);
    save(&store, &plan).unwrap();
    assert_eq!(load(&store).unwrap(), Some(plan));
    let jobs = agent_jobs::Jobs::new(store.clone());
    let job = agent_jobs::template("plan", "", None, None, 0, MONDAY).unwrap();
    assert!(!job.enabled, "off until the owner turns it on");
    jobs.add(job, MONDAY).unwrap();
    jobs.edit("plan", agent_jobs::Edit::On, MONDAY).unwrap();
    struct World;
    impl agent_jobs::Facts for World {
        fn head(&self, _: &std::path::Path) -> Option<String> {
            None
        }
        fn issues(
            &self,
            _: &str,
            _: &str,
        ) -> Result<
            (
                Vec<super::super::issue_pick::Open>,
                Vec<super::super::issue_pick::Pull>,
            ),
            String,
        > {
            Ok((Vec::new(), Vec::new()))
        }
        fn capacity(&self) -> bool {
            true
        }
    }
    let tick = |now| agent_jobs::tick(&store, &record, dir.path(), &World, now).unwrap();
    assert!(tick(MONDAY + 6 * HOUR).is_empty());
    let fired = tick(MONDAY + 7 * HOUR + 60);
    assert_eq!(fired.len(), 1);
    assert!(fired[0].plan && fired[0].reflect.is_none());
    assert!(tick(MONDAY + 9 * HOUR).is_empty());
}

/// A sample day with a re-plan, as `D-plan.txt` shows it. With
/// `AGENT_PLAN_DUMP` set to a directory, it writes the text and the plan's
/// JSON there for the capture.
#[test]
fn a_sample_day_reads_as_text_after_two_replans() {
    let wednesday = MONDAY + 2 * 86_400;
    let mut day = Day::new();
    let mut nightly = on("nightly-check");
    nightly.title = "Nightly checks".into();
    day.jobs = vec![nightly, on("reflect"), on("keep-green"), on("plan")];
    day.issues = vec![
        (812, "Bound the relay's replay buffer".into()),
        (815, "Name the plan board in the Verse docs".into()),
    ];
    day.queued = vec![("request:1".into(), "run the coder-pty tests".into())];
    day.memory = vec![insight(
        12,
        "cargo test -p verse --release needs a longer command bound.",
    )];
    let reply = serde_json::json!({"blocks": [
        {"source": "issue:812", "node": DESK, "start": "09:00", "minutes": 120, "title": "Work issue 812: bound the relay's replay buffer"},
        {"source": "request:1", "node": CONSOLE, "start": "11:15", "minutes": 30, "title": "Run the coder-pty tests"},
        {"source": "memory:12", "node": CONSOLE, "start": "13:00", "minutes": 60, "title": "Rerun the verse release tests with a longer bound"},
        {"source": "issue:815", "node": DESK, "start": "14:30", "minutes": 60, "title": "Work issue 815: name the plan board in the docs"},
        {"source": "chore:tidy", "node": DESK, "start": "16:00", "minutes": 30, "title": "Tidy the desk"},
    ]})
    .to_string();
    let steps = serde_json::json!({"steps": [
        {"start": "09:00", "minutes": 10, "text": "Read issue 812 and the replay code it names"},
        {"start": "09:10", "minutes": 15, "text": "Write a test that overflows the replay buffer"},
        {"start": "09:25", "minutes": 15, "text": "Cap the buffer and drop the oldest frames"},
        {"start": "09:40", "minutes": 10, "text": "Run cargo test -p coder-pty"},
        {"start": "09:50", "minutes": 10, "text": "Bring the change to the Merge station"},
    ]})
    .to_string();
    let mut writer = Scripted::new([reply, steps]);
    let made = draft(&day.inputs(wednesday + 7 * HOUR), &mut writer, &screen()).unwrap();
    assert_eq!(made.rejected.len(), 1, "the invented chore");
    let refused = format!(
        "refused at the draft: {} ({})\n",
        made.rejected[0].0, made.rejected[0].1
    );
    let mut plan = made.plan;
    assert_eq!(begin(&mut plan, 9 * 60), Some(2));
    let source = &plan.current_block().unwrap().title.clone();
    plan.steps = decompose(&plan, source, 9 * 60, &mut writer, &screen())
        .unwrap()
        .0;
    let places = places(world_tree::everglade(), &day.known, HOUSE);
    let mut judge = Answers::default();
    react(
        &mut plan,
        &Event::Owner {
            source: "request:2".into(),
            text: "look at why the deploy failed".into(),
        },
        wednesday + 10 * HOUR,
        &places,
        &mut judge,
    );
    begin(&mut plan, 13 * 60 + 20);
    judge.answers.push_back(Ok(Reaction::ReactNow));
    let reacted = react(
        &mut plan,
        &Event::Observed {
            source: "job:keep-green".into(),
            text: "The default branch moved and cargo test -p supervise failed".into(),
        },
        wednesday + 13 * HOUR + 20 * 60,
        &places,
        &mut judge,
    );
    assert!(reacted.replanned);
    assert!(plan.validate().is_ok());
    let text = text(&plan);
    assert!(text.starts_with("alice for 2026-10-07, within "), "{text}");
    assert!(text.contains("re-planned from 10:00 (Interrupt, request:2)"));
    assert!(text.contains("re-planned from 13:20 (React, job:keep-green)"));
    let ids = source_ids(&sources(&day.inputs(wednesday + 7 * HOUR)));
    assert!(
        plan.blocks
            .iter()
            .all(|b| ids.contains(&b.source) || b.by != By::Model)
    );
    if let Some(dir) = std::env::var_os("AGENT_PLAN_DUMP") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::write(dir.join("D-plan.txt"), format!("{text}{refused}")).unwrap();
        std::fs::write(
            dir.join("D-plan.json"),
            serde_json::to_string_pretty(&plan).unwrap(),
        )
        .unwrap();
    }
}
