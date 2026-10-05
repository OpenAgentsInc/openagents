use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use coder_host::access::review::{Publication, PublishState};
use serde_json::json;

use super::super::super::publish::Reviewed;
use super::super::super::studio_sim::{BRANCH, Fixture, SimInbox};
use super::super::super::{Checks, Status, local, review};
use super::super::{
    GoalStatus, Inbox, MemoryKind, NewGoal, PLAN_SCHEMA, PlanEntry, Progress, Released, Repository,
    Role, Seat, SlotState, Studio, git, parse_route,
};
use super::*;

const LEAD: &str = "lead";
const WORKER: &str = "ada";

fn run_git(dir: &Path, args: &[&str]) -> String {
    let output = local::git().arg("-C").arg(dir).args(args).output().unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// Commit everything in `worktree` as the worker seat.
fn commit(worktree: &Path, message: &str) {
    let (name, email) = git::identity(WORKER);
    run_git(worktree, &["add", "-A"]);
    run_git(
        worktree,
        &[
            "-c",
            &format!("user.name={name}"),
            "-c",
            &format!("user.email={email}"),
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "-m",
            message,
        ],
    );
}

fn ids(released: &[Released]) -> Vec<String> {
    released.iter().map(|item| item.task_id.clone()).collect()
}

/// The README the fixture seeds, with its status line set to `status`.
fn readme(status: &str) -> String {
    format!("# Scratch\n\nStatus: {status}\n\nA scratch repository for the simulated team.\n")
}

/// A lead and one worker over the simulated team's scratch repository,
/// each task in a worktree of its own, with the simulated inbox standing
/// in for the engines: a test takes each turn as the scripted engine
/// would.
struct Bench {
    _dir: tempfile::TempDir,
    fixture: Fixture,
    inbox: SimInbox,
    studio: Studio,
    replies: BTreeMap<String, String>,
    clock: u64,
}

impl Bench {
    /// A goal its lead planned as `tasks`, with the lead's review on or
    /// off.
    fn new(lead_review: bool, tasks: serde_json::Value) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let fixture = Fixture::create(&dir.path().join("sim")).unwrap();
        let mut studio = Studio::open(&fixture.store)
            .unwrap()
            .with_worktrees(&fixture.worktrees);
        let route = parse_route("codex:studio-sim").unwrap();
        for (desk, (name, role)) in [(LEAD, Role::Lead), (WORKER, Role::Worker)]
            .into_iter()
            .enumerate()
        {
            studio
                .set_seat(Seat {
                    name: name.into(),
                    role,
                    route: route.clone(),
                    look: "default".into(),
                    desk: desk as u32,
                })
                .unwrap();
        }
        assert!(studio.lead_review(), "the lead reviews by default");
        studio.set_lead_review(lead_review).unwrap();
        let mut bench = Self {
            _dir: dir,
            inbox: SimInbox::default(),
            studio,
            replies: BTreeMap::new(),
            clock: 1_000,
            fixture,
        };
        let goal = NewGoal {
            text: "Change the greeting.".into(),
            repository: Repository {
                label: "scratch".into(),
                path: bench.fixture.checkout.to_string_lossy().into_owned(),
            },
            lead: None,
        };
        let (_, lead) = bench
            .studio
            .submit_goal(&mut bench.inbox, goal, bench.clock)
            .unwrap();
        let plan = json!({"schema": PLAN_SCHEMA, "tasks": tasks}).to_string();
        bench.turn(
            &lead.task_id,
            &format!("I read the repository.\n\n```json\n{plan}\n```\n"),
        );
        bench.reconcile();
        bench
    }

    fn reconcile(&mut self) -> Vec<Released> {
        self.clock += 60;
        let replies = &self.replies;
        let reply = |task: &str| replies.get(task).cloned();
        self.studio
            .reconcile(&mut self.inbox, self.clock, &reply)
            .unwrap()
    }

    /// One engine turn of `task`, ending with `reply`.
    fn turn(&mut self, task: &str, reply: &str) {
        self.inbox.start(task).unwrap();
        self.inbox.end(task, "model_finished");
        self.replies.insert(task.into(), reply.into());
    }

    fn entry(&self, id: &str) -> PlanEntry {
        self.studio.state().goals[0]
            .plan
            .iter()
            .find(|entry| entry.id == id)
            .unwrap()
            .clone()
    }

    fn task_id(&self, id: &str) -> String {
        self.entry(id).slot.task_id
    }

    fn flow(&self, id: &str) -> Flow {
        self.entry(id)
            .flow
            .expect("a task in a worktree has a flow")
    }

    fn task(&self, id: &str) -> super::super::super::Task {
        self.inbox.task(&self.task_id(id)).unwrap()
    }

    fn progress(&self, id: &str) -> Progress {
        self.studio.view(&self.inbox).goals[0]
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .unwrap()
            .progress
    }

    fn worktree(&self, id: &str) -> PathBuf {
        PathBuf::from(
            local::record(&self.fixture.store, &self.task_id(id))
                .unwrap()
                .worktree,
        )
    }

    /// The worker's turn on entry `id`: write `files`, commit them as its
    /// seat, end the turn, and record its check's `verdict` and `reason`.
    fn work(&mut self, id: &str, files: &[(&str, &str)], verdict: Checks, reason: &str) {
        let task = self.task_id(id);
        let worktree = self.worktree(id);
        for (path, text) in files {
            std::fs::write(worktree.join(path), text).unwrap();
        }
        if !files.is_empty() {
            commit(&worktree, "Work on the task");
        }
        self.turn(&task, "Done.");
        self.inbox.checked(&task, verdict, reason);
    }

    /// The lead's review of entry `id` ends with `verdict` and `notes`.
    /// Returns the review task.
    fn review(&mut self, id: &str, verdict: &str, notes: &str) -> String {
        let review = self.flow(id).review.expect("a review task");
        assert_eq!(review.state, SlotState::Submitted);
        let block = json!({"schema": REVIEW_SCHEMA, "verdict": verdict, "notes": notes});
        self.turn(
            &review.task_id,
            &format!("I read the diff.\n\n```json\n{block}\n```\n"),
        );
        review.task_id
    }

    /// The person merges entry `id` at its worktree's revisions now, as the
    /// host's publish does.
    fn merge(&mut self, id: &str) -> Publication {
        let task = self.task_id(id);
        let record = local::record(&self.fixture.store, &task).unwrap();
        let head = review::head(Path::new(&record.worktree)).unwrap();
        git::merge(
            &self.fixture.store,
            &task,
            &Reviewed {
                base: record.base,
                head_commit: head.commit,
                head: head.tree,
            },
        )
        .unwrap()
    }
}

#[test]
fn verification_speaks_the_shared_vocabulary() {
    assert_eq!(Verification::of(Checks::Running), None);
    assert_eq!(Verification::of(Checks::Passed), Some(Verification::Passed));
    assert_eq!(
        Verification::of(Checks::Disputed),
        Some(Verification::Unverifiable)
    );
    assert_eq!(
        Verification::of(Checks::Unavailable),
        Some(Verification::Unverifiable)
    );
    assert_eq!(
        Verification::of(Checks::NotRun).map(Verification::shared),
        Some(nostr::contracts::Verification::NotRun)
    );
    assert_eq!(
        serde_json::to_string(&Verification::NotRun).unwrap(),
        "\"not_run\""
    );
    assert_eq!(Verification::Failed.word(), "failed");
}

#[test]
fn a_review_reply_ends_with_its_verdict() {
    let reply = format!(
        "Looks fine.\n\n```json\n{{\"schema\":\"{REVIEW_SCHEMA}\",\"verdict\":\"changes\",\"notes\":\"Add a test.\"}}\n```\n"
    );
    let verdict = review_in_reply(&reply).unwrap();
    assert_eq!(verdict.verdict, Verdict::Changes);
    assert_eq!(verdict.notes, "Add a test.");
    assert!(review_in_reply("No verdict here.").is_none());
    let unknown = format!("{{\"schema\":\"{REVIEW_SCHEMA}\",\"verdict\":\"ship\"}}");
    assert!(review_in_reply(&unknown).is_none());
}

#[test]
fn a_red_check_goes_back_once_then_the_lead_reviews_and_the_person_merges() {
    let mut bench = Bench::new(
        true,
        json!([
            {"id": "greet", "title": "Change the greeting", "seat": WORKER},
            {"id": "docs", "title": "Document the greeting", "depends_on": ["greet"], "seat": WORKER},
        ]),
    );
    assert_eq!(bench.flow("greet").stage, Stage::Work);

    // Red: the failure goes back to the same task, once.
    bench.work(
        "greet",
        &[("greeting.txt", "Hello\n")],
        Checks::Failed,
        "greeting_test: expected Hello, studio",
    );
    assert_eq!(bench.progress("greet"), Progress::Review);
    assert!(bench.reconcile().is_empty());
    let task = bench.task("greet");
    assert_eq!(task.status, Status::Queued);
    assert_eq!(task.turn(), 2);
    let told = &task.follow_ups.last().unwrap().prompt;
    assert!(
        told.contains("greeting_test: expected Hello, studio"),
        "{told}"
    );
    let flow = bench.flow("greet");
    assert_eq!(flow.fixes, 1);
    assert_eq!(flow.verification, Some(Verification::Failed));
    assert!(flow.command.is_none());
    assert_eq!(bench.progress("greet"), Progress::Queued);
    // Another pass sends nothing more.
    bench.reconcile();
    assert_eq!(bench.task("greet").follow_ups.len(), 1);

    // Green: the lead reviews the diff, the check, and the history in a
    // task and worktree of its own.
    bench.work(
        "greet",
        &[("greeting.txt", "Hello, studio\n")],
        Checks::Passed,
        "",
    );
    let released = bench.reconcile();
    let flow = bench.flow("greet");
    assert_eq!(flow.stage, Stage::Review);
    assert_eq!(flow.verification, Some(Verification::Passed));
    let review = flow.review.clone().unwrap();
    assert_eq!(review.seat, LEAD);
    assert_eq!(review.state, SlotState::Submitted);
    assert_eq!(ids(&released), vec![review.task_id.clone()]);
    let asked = bench.inbox.task(&review.task_id).unwrap();
    let prompt = &asked.intent.prompt;
    assert!(prompt.contains("Checks: passed"), "{prompt}");
    assert!(prompt.contains("greeting.txt"), "{prompt}");
    assert!(prompt.contains("+Hello, studio"), "{prompt}");
    assert!(
        prompt.contains("greeting_test"),
        "the history carries the fix round: {prompt}"
    );
    assert!(prompt.contains(REVIEW_SCHEMA), "{prompt}");
    assert_ne!(
        PathBuf::from(&asked.intent.workspace.path),
        bench.worktree("greet"),
        "the review never works in the worker's tree"
    );
    assert_eq!(bench.progress("greet"), Progress::Review);
    // The lead's seat is busy with the review.
    let lead = bench.studio.view(&bench.inbox);
    let lead = lead
        .seats
        .iter()
        .find(|seat| seat.seat.name == LEAD)
        .unwrap();
    assert_eq!(lead.task_id.as_deref(), Some(review.task_id.as_str()));

    // The person cannot merge while the lead reviews.
    let early = bench.merge("greet");
    assert_eq!(early.state, PublishState::Refused);
    assert!(early.note.contains("still reviewing"), "{}", early.note);

    // Approved: the change waits on the person, and so does its dependent.
    bench.review("greet", "approve", "Small and tested.");
    bench.reconcile();
    let flow = bench.flow("greet");
    assert_eq!(flow.stage, Stage::Merge);
    assert!(
        flow.notes
            .iter()
            .any(|note| note.contains("Small and tested.")),
        "{:?}",
        flow.notes
    );
    assert_eq!(bench.progress("greet"), Progress::Merge);
    assert_eq!(bench.entry("docs").slot.state, SlotState::Held);
    assert_eq!(
        bench.studio.view(&bench.inbox).goals[0].status,
        GoalStatus::Running
    );

    // The merge lands; the flow ends and the dependent starts.
    let merged = bench.merge("greet");
    assert_eq!(merged.state, PublishState::Published, "{}", merged.note);
    assert_eq!(
        std::fs::read_to_string(bench.fixture.checkout.join("greeting.txt")).unwrap(),
        "Hello, studio\n"
    );
    let released = bench.reconcile();
    assert_eq!(bench.flow("greet").stage, Stage::Merged);
    assert_eq!(bench.progress("greet"), Progress::Done);
    assert_eq!(ids(&released), vec![bench.task_id("docs")]);
}

#[test]
fn a_check_that_fails_again_goes_on_with_the_failure_noted() {
    let mut bench = Bench::new(
        false,
        json!([{"id": "greet", "title": "Change the greeting", "seat": WORKER}]),
    );
    bench.work(
        "greet",
        &[("greeting.txt", "Hello\n")],
        Checks::Failed,
        "first failure",
    );
    bench.reconcile();
    bench.work(
        "greet",
        &[("greeting.txt", "Hello!\n")],
        Checks::Failed,
        "second failure",
    );
    bench.reconcile();
    let flow = bench.flow("greet");
    assert_eq!(flow.stage, Stage::Merge);
    assert_eq!(flow.fixes, 1);
    assert!(
        flow.notes
            .iter()
            .any(|note| note.contains("second failure")),
        "{:?}",
        flow.notes
    );
    assert_eq!(bench.task("greet").follow_ups.len(), 1);
    let view = bench.studio.view(&bench.inbox);
    let entry = &view.goals[0].entries[0];
    assert_eq!(entry.progress, Progress::Merge);
    assert_eq!(entry.stage, Some(Stage::Merge));
    assert_eq!(entry.verification, Some(Verification::Failed));
}

#[test]
fn the_lead_sends_changes_back_and_reviews_the_new_turn() {
    let mut bench = Bench::new(
        true,
        json!([{"id": "greet", "title": "Change the greeting", "seat": WORKER}]),
    );
    bench.work(
        "greet",
        &[("greeting.txt", "Hello, studio\n")],
        Checks::Passed,
        "",
    );
    bench.reconcile();
    let first = bench.review("greet", "changes", "Also say it in the README.");
    bench.reconcile();

    // The notes go back to the same task as its next turn.
    let task = bench.task("greet");
    assert_eq!(task.status, Status::Queued);
    let told = &task.follow_ups.last().unwrap().prompt;
    assert!(told.contains("Also say it in the README."), "{told}");
    let flow = bench.flow("greet");
    assert_eq!((flow.stage, flow.changes), (Stage::Work, 1));
    assert!(flow.review.is_none());
    assert_eq!(bench.progress("greet"), Progress::Queued);
    assert!(bench.studio.state().memory.iter().any(|entry| {
        entry.kind == MemoryKind::Decision && entry.text.contains("Also say it in the README.")
    }));

    // The new turn goes through the checks and a new review.
    bench.work(
        "greet",
        &[("README.md", &readme("Hello, studio"))],
        Checks::Passed,
        "",
    );
    bench.reconcile();
    let flow = bench.flow("greet");
    assert_eq!(flow.stage, Stage::Review);
    let second = flow.review.unwrap().task_id;
    assert_ne!(second, first);
    let prompt = bench.inbox.task(&second).unwrap().intent.prompt;
    assert!(prompt.contains("README.md"), "{prompt}");
    assert!(
        prompt.contains("Also say it in the README."),
        "the history carries the request: {prompt}"
    );
    bench.review("greet", "approve", "");
    bench.reconcile();
    assert_eq!(bench.progress("greet"), Progress::Merge);
}

#[test]
fn a_conflicting_merge_goes_back_to_the_worker_who_merges_the_branch_in() {
    let mut bench = Bench::new(
        false,
        json!([{"id": "docs", "title": "Document the greeting", "seat": WORKER}]),
    );
    bench.work(
        "docs",
        &[("README.md", &readme("documented"))],
        Checks::Passed,
        "",
    );
    bench.reconcile();
    let flow = bench.flow("docs");
    assert_eq!(
        flow.stage,
        Stage::Merge,
        "with the lead's review off the change goes to the person"
    );
    assert!(flow.review.is_none());

    // Meanwhile the person's branch changed the same line.
    let checkout = bench.fixture.checkout.clone();
    std::fs::write(checkout.join("README.md"), readme("greets people")).unwrap();
    run_git(&checkout, &["commit", "-q", "-am", "Greet people"]);
    let refused = bench.merge("docs");
    assert_eq!(refused.state, PublishState::Refused);
    assert!(refused.note.contains("README.md"), "{}", refused.note);

    // The next pass sends the task back to its worker to merge the branch
    // in and resolve it.
    bench.reconcile();
    let task = bench.task("docs");
    assert_eq!(task.status, Status::Queued);
    let told = &task.follow_ups.last().unwrap().prompt;
    assert!(told.contains(&format!("has merged `{BRANCH}`")), "{told}");
    assert!(told.contains("README.md"), "{told}");
    let flow = bench.flow("docs");
    assert_eq!((flow.stage, flow.conflicts), (Stage::Work, 1));
    assert!(flow.conflict.is_none());
    assert_eq!(bench.progress("docs"), Progress::Queued);
    // The conflict is read once.
    bench.reconcile();
    assert_eq!(bench.task("docs").follow_ups.len(), 1);

    // The worker merges the branch in, resolves the conflict, and commits.
    let worktree = bench.worktree("docs");
    let merging = local::git()
        .arg("-C")
        .arg(&worktree)
        .args(["merge", BRANCH])
        .output()
        .unwrap();
    assert!(!merging.status.success(), "the merge conflicts");
    bench.work(
        "docs",
        &[("README.md", &readme("greets people, documented"))],
        Checks::Passed,
        "",
    );
    bench.reconcile();
    assert_eq!(bench.flow("docs").stage, Stage::Merge);

    // Now the merge lands, and the goal is done.
    let merged = bench.merge("docs");
    assert_eq!(merged.state, PublishState::Published, "{}", merged.note);
    assert!(
        std::fs::read_to_string(checkout.join("README.md"))
            .unwrap()
            .contains("Status: greets people, documented")
    );
    bench.reconcile();
    assert_eq!(bench.flow("docs").stage, Stage::Merged);
    assert_eq!(
        bench.studio.view(&bench.inbox).goals[0].status,
        GoalStatus::Done
    );
}

#[test]
fn a_task_that_changed_nothing_is_done_without_a_merge_decision() {
    let mut bench = Bench::new(
        true,
        json!([
            {"id": "look", "title": "Investigate the greeting", "seat": WORKER},
            {"id": "next", "title": "Act on it", "depends_on": ["look"], "seat": WORKER},
        ]),
    );
    bench.work("look", &[], Checks::NotRun, "");
    let released = bench.reconcile();
    let flow = bench.flow("look");
    assert_eq!(flow.stage, Stage::Unchanged);
    assert!(flow.review.is_none(), "no review and no merge decision");
    assert_eq!(bench.progress("look"), Progress::Done);
    assert_eq!(ids(&released), vec![bench.task_id("next")]);
}

#[test]
fn a_merged_change_is_done_and_its_dependent_starts() {
    let mut bench = Bench::new(
        false,
        json!([
            {"id": "greet", "title": "Change the greeting", "seat": WORKER},
            {"id": "docs", "title": "Document the greeting", "depends_on": ["greet"], "seat": WORKER},
        ]),
    );
    // The change lands while its task still sits in the queue, as a
    // `task.publish` of a studio task can make it land.
    let task = bench.task_id("greet");
    let worktree = bench.worktree("greet");
    std::fs::write(worktree.join("greeting.txt"), "Hello, studio\n").unwrap();
    commit(&worktree, "Change the greeting");
    assert_eq!(bench.task("greet").status, Status::Queued);
    let merged = bench.merge("greet");
    assert_eq!(merged.state, PublishState::Published, "{}", merged.note);
    assert_eq!(bench.entry("docs").slot.state, SlotState::Held);

    // The host marks it merged: the entry is done whatever its task does
    // next, and the next pass starts its dependent.
    assert!(bench.studio.note_merged(&task).unwrap());
    assert!(!bench.studio.note_merged(&task).unwrap(), "marked once");
    assert_eq!(bench.flow("greet").stage, Stage::Merged);
    assert_eq!(bench.progress("greet"), Progress::Done);
    let released = bench.reconcile();
    assert_eq!(ids(&released), vec![bench.task_id("docs")]);
    assert_eq!(bench.flow("greet").stage, Stage::Merged);
    assert_eq!(bench.progress("greet"), Progress::Done);

    // A task no plan entry holds changes nothing.
    assert!(!bench.studio.note_merged(&"0".repeat(64)).unwrap());
}
