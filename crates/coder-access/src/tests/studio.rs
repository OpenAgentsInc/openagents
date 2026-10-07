//! Agent Studio over NIP-HOST: each intent's right and refusal code, the
//! stale merge refusal, and a missed update forcing a fresh snapshot.
use super::*;
use crate::studio::{
    Activity, Goal, GoalStatus, MergeDecision, Merged, Mirror, Role, Seat, Spend, Stream, Verdict,
    View,
};

/// The revisions the stub's worktree is at until a test moves its head.
const BASE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const HEAD: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

/// A host's studio for the access tests: a stream over a view a test
/// changes, and a worktree whose content tree a test can move.
#[derive(Clone)]
pub(in crate::tests) struct Stub {
    pub stream: Arc<Mutex<Stream>>,
    pub view: Arc<Mutex<View>>,
    pub head: Arc<Mutex<String>>,
}

impl Default for Stub {
    fn default() -> Self {
        Self {
            stream: Arc::new(Mutex::new(Stream::new("5d"))),
            view: Arc::new(Mutex::new(view())),
            head: Arc::new(Mutex::new(HEAD.into())),
        }
    }
}

impl Stub {
    pub fn snapshot(&self) -> crate::studio::Snapshot {
        let view = self.view.lock().unwrap().clone();
        self.stream.lock().unwrap().snapshot(view)
    }
    pub fn update(
        &self,
        stream: &str,
        since: u64,
    ) -> std::result::Result<crate::studio::Update, Code> {
        let view = self.view.lock().unwrap().clone();
        self.stream.lock().unwrap().update(view, stream, since)
    }
    /// A decision at the worktree's revisions is answered; any other is
    /// stale. This stub lands nothing, so a merge is unsupported.
    pub fn merge(&self, decision: &MergeDecision) -> std::result::Result<Merged, Code> {
        let head = self.head.lock().unwrap().clone();
        if decision.base != BASE || decision.head_commit != BASE || decision.head != head {
            return Err(Code::Stale);
        }
        if decision.verdict == Verdict::Merge {
            return Err(Code::Unsupported);
        }
        Ok(Merged {
            task: decision.task.clone(),
            base: decision.base.clone(),
            head_commit: decision.head_commit.clone(),
            head: decision.head.clone(),
            verdict: decision.verdict,
            publication: None,
        })
    }
}

fn view() -> View {
    View {
        goals: vec![Goal {
            goal: "g1-0011aabb".into(),
            text: "Add a dark mode".into(),
            workspace: "fixture".into(),
            lead: "planner".into(),
            status: GoalStatus::Planning,
            final_tasks: 0,
            total_tasks: 0,
            submitted_at: 1_790_000_000,
            spend: Spend::default(),
        }],
        seats: vec![Seat {
            seat: "planner".into(),
            role: Role::Lead,
            route: "codex:gpt-6".into(),
            look: "amber".into(),
            desk: 0,
            activity: Activity::Reading,
            station: Activity::Reading.station(),
            task: Some("studio-g1-0011aabb-lead".into()),
            paused: false,
            spend: Spend::default(),
        }],
        ..View::default()
    }
}

const TASK: &str = "studio-g1-0011aabb-first";

/// Every studio intent that changes the studio, each well formed.
fn intents() -> Vec<Operation> {
    vec![
        Operation::SubmitGoal {
            text: "Add a dark mode".into(),
            workspace: "fixture".into(),
            lead: None,
        },
        Operation::MessageSeat {
            seat: Some("planner".into()),
            text: "Keep the palette amber.".into(),
        },
        Operation::MessageSeat {
            seat: None,
            text: "Stand-up in five.".into(),
        },
        Operation::PauseSeat {
            seat: "builder".into(),
        },
        Operation::ResumeSeat {
            seat: "builder".into(),
        },
        Operation::StopSeat {
            seat: "builder".into(),
        },
        Operation::ReassignTask {
            task: TASK.into(),
            seat: "builder".into(),
        },
        Operation::CancelStudioTask { task: TASK.into() },
        Operation::RetryTask { task: TASK.into() },
        Operation::PrioritizeTask { task: TASK.into() },
        Operation::AnswerDecision {
            decision: TASK.into(),
            based_on: 3,
            text: "Use the system palette.".into(),
            command: "c".repeat(64),
            issued_at: now(),
        },
        Operation::AllowAlways {
            decision: TASK.into(),
            based_on: 3,
            rule: "builder may run shell `cargo test` in /work/repo without asking".into(),
            command: "d".repeat(64),
            issued_at: now(),
        },
    ]
}

fn merge(verdict: Verdict, head: &str) -> Operation {
    Operation::DecideMerge {
        decision: Box::new(MergeDecision {
            task: TASK.into(),
            base: BASE.into(),
            head_commit: BASE.into(),
            head: head.into(),
            verdict,
            text: match verdict {
                Verdict::RequestChanges => "Name the setting.".into(),
                _ => String::new(),
            },
            command: random_id(),
            issued_at: now(),
        }),
    }
}

#[test]
fn studio_operations_name_their_one_right() {
    for op in intents() {
        assert!(op.name().starts_with("studio."), "{}", op.name());
        assert_eq!(op.required(), Some(Right::Operate), "{}", op.name());
        assert!(op.studio_intent() && !op.reads_only() && op.retains_reply());
        op.validate().unwrap();
    }
    for op in [
        Operation::StudioSnapshot {},
        Operation::StudioUpdate {
            stream: "5d".into(),
            since: 1,
        },
        Operation::OpenReview { task: TASK.into() },
    ] {
        assert_eq!(op.required(), Some(Right::Observe), "{}", op.name());
        assert!(op.reads_only() && !op.retains_reply(), "{}", op.name());
    }
    let decide = merge(Verdict::Merge, HEAD);
    assert_eq!(decide.required(), Some(Right::Review));
    assert!(!decide.reads_only() && decide.retains_reply());
    // Names and wire tags agree.
    for op in intents() {
        let wire = serde_json::to_value(&op).unwrap();
        assert_eq!(wire["kind"], op.name());
    }
}

#[test]
fn malformed_studio_intents_are_refused_before_any_effect() {
    let refused = |op: Operation| op.validate().unwrap_err().code;
    assert_eq!(
        refused(Operation::SubmitGoal {
            text: " ".into(),
            workspace: "fixture".into(),
            lead: None,
        }),
        Code::Bounds
    );
    assert_eq!(
        refused(Operation::SubmitGoal {
            text: "Goal".into(),
            workspace: "fixture".into(),
            lead: Some("Lead Seat".into()),
        }),
        Code::Malformed
    );
    assert_eq!(
        refused(Operation::MessageSeat {
            seat: None,
            text: "x".repeat(crate::studio::MAX_MESSAGE + 1),
        }),
        Code::Bounds
    );
    for op in [
        Operation::PauseSeat { seat: "".into() },
        Operation::ResumeSeat {
            seat: "-lead".into(),
        },
        Operation::StopSeat {
            seat: "a".repeat(33),
        },
        Operation::ReassignTask {
            task: "../escape".into(),
            seat: "builder".into(),
        },
        Operation::CancelStudioTask { task: "a/b".into() },
        Operation::RetryTask { task: "".into() },
        Operation::PrioritizeTask {
            task: "x".repeat(129),
        },
        Operation::OpenReview {
            task: ".hidden".into(),
        },
        Operation::StudioUpdate {
            stream: "XYZ".into(),
            since: 1,
        },
        Operation::AnswerDecision {
            decision: TASK.into(),
            based_on: 1,
            text: "Yes".into(),
            command: "short".into(),
            issued_at: 1,
        },
        Operation::AllowAlways {
            decision: TASK.into(),
            based_on: 1,
            rule: "builder may run shell `ls` in /work".into(),
            command: "short".into(),
            issued_at: 1,
        },
    ] {
        assert_eq!(refused(op.clone()), Code::Malformed, "{}", op.name());
    }
    assert_eq!(
        refused(Operation::AnswerDecision {
            decision: TASK.into(),
            based_on: 1,
            text: "x".repeat(crate::studio::MAX_ANSWER + 1),
            command: "c".repeat(64),
            issued_at: 1,
        }),
        Code::Bounds
    );
    assert_eq!(
        refused(Operation::AllowAlways {
            decision: TASK.into(),
            based_on: 1,
            rule: " ".into(),
            command: "c".repeat(64),
            issued_at: 1,
        }),
        Code::Bounds
    );
    assert_eq!(refused(merge(Verdict::Merge, "main")), Code::Malformed);
}

/// A device with `observe` reads the studio and its reviews, and is
/// refused every intent as `missing_right` naming `operate`, and a merge
/// decision naming `review`, before the task owner sees any of them.
/// `operate` dispatches each intent; it does not decide a merge.
#[tokio::test]
async fn studio_intents_need_operate_and_merges_need_review() {
    let f = Fixture::served(0, false).await;
    let (_, observer) = f.enroll("observe").await;
    for op in intents() {
        let error = observer.call(op.clone()).await.unwrap_err();
        assert_eq!(
            (error.code, error.missing),
            (Code::MissingRight, Some(Right::Operate)),
            "{}",
            op.name()
        );
    }
    let error = observer
        .call(merge(Verdict::Reject, HEAD))
        .await
        .unwrap_err();
    assert_eq!(
        (error.code, error.missing),
        (Code::MissingRight, Some(Right::Review))
    );
    assert_eq!(f.recorder.count(), 0);
    let Outcome::Studio { snapshot } = observer.call(Operation::StudioSnapshot {}).await.unwrap()
    else {
        panic!("a snapshot expected");
    };
    assert_eq!(snapshot.view, view());
    // The review read passes the grant check; this host reviews nothing.
    let error = observer
        .call(Operation::OpenReview { task: TASK.into() })
        .await
        .unwrap_err();
    assert_eq!(error.code, Code::Unsupported);

    let (_, operator) = f.enroll("observe,operate").await;
    for op in intents() {
        let Outcome::Dispatched { receipt } = operator.call(op.clone()).await.unwrap() else {
            panic!("{} dispatched expected", op.name());
        };
        assert_eq!(receipt.operation, op.name());
    }
    assert_eq!(f.recorder.count(), intents().len());
    let error = operator
        .call(merge(Verdict::Reject, HEAD))
        .await
        .unwrap_err();
    assert_eq!(
        (error.code, error.missing),
        (Code::MissingRight, Some(Right::Review))
    );
    assert_eq!(f.recorder.count(), intents().len());
}

/// A merge decision binds to the review's three revisions: once the
/// worktree moves, the same decision is refused as `stale`, so the client
/// reloads the review.
#[tokio::test]
async fn a_merge_decision_on_a_moved_worktree_is_stale() {
    let f = Fixture::served(0, false).await;
    let (_, reviewer) = f.enroll("observe,review").await;
    let Outcome::Merged { merged } = reviewer.call(merge(Verdict::Reject, HEAD)).await.unwrap()
    else {
        panic!("a merge record expected");
    };
    assert_eq!(
        (merged.verdict, merged.head.as_str()),
        (Verdict::Reject, HEAD)
    );
    *f.recorder.2.head.lock().unwrap() = "d".repeat(40);
    for verdict in [Verdict::Merge, Verdict::RequestChanges, Verdict::Reject] {
        let error = reviewer.call(merge(verdict, HEAD)).await.unwrap_err();
        assert_eq!(error.code, Code::Stale, "{verdict:?}");
    }
    // A decision at the new head is current again.
    let Outcome::Merged { merged } = reviewer
        .call(merge(Verdict::RequestChanges, &"d".repeat(40)))
        .await
        .unwrap()
    else {
        panic!("a merge record expected");
    };
    assert_eq!(merged.verdict, Verdict::RequestChanges);
    assert_eq!(f.recorder.count(), 2);
}

/// A client keeps the studio with updates; an update it did not ask for,
/// or a host that started again (a new stream), forces a fresh snapshot.
#[tokio::test]
async fn a_missed_studio_update_forces_a_fresh_snapshot() {
    let f = Fixture::served(0, false).await;
    let (_, observer) = f.enroll("observe").await;
    let mut mirror = Mirror::default();
    let answer = observer.call(mirror.next()).await.unwrap();
    assert!(mirror.accept(&answer).unwrap());
    // The studio changes; the client hears the update from its sequence.
    f.recorder.2.view.lock().unwrap().seats[0].activity = Activity::Editing;
    let answer = observer.call(mirror.next()).await.unwrap();
    assert!(mirror.accept(&answer).unwrap());
    assert_eq!(
        mirror.snapshot().unwrap().view.seats[0].activity,
        Activity::Editing
    );
    // An update from another point than the mirror's is a missed one: the
    // copy is dropped and the next request is a snapshot.
    let held = mirror.snapshot().unwrap().sequence;
    f.recorder.2.view.lock().unwrap().goals[0].status = GoalStatus::Running;
    let first = f.recorder.2.update("5d", held).unwrap();
    f.recorder.2.view.lock().unwrap().goals[0].status = GoalStatus::Done;
    let second = f.recorder.2.update("5d", first.sequence).unwrap();
    let error = mirror
        .accept(&Outcome::StudioUpdate {
            update: Box::new(second),
        })
        .unwrap_err();
    assert_eq!(error.code, Code::Stale);
    assert_eq!(mirror.next(), Operation::StudioSnapshot {});
    let answer = observer.call(mirror.next()).await.unwrap();
    mirror.accept(&answer).unwrap();
    assert_eq!(
        mirror.snapshot().unwrap().view.goals[0].status,
        GoalStatus::Done
    );
    // The host starts again: its stream is new, so the client's sequence
    // means nothing there and the host refuses the update as `stale`.
    *f.recorder.2.stream.lock().unwrap() = Stream::new("6e");
    let error = observer.call(mirror.next()).await.unwrap_err();
    assert_eq!(error.code, Code::Stale);
    mirror.refused(&error);
    let answer = observer.call(mirror.next()).await.unwrap();
    mirror.accept(&answer).unwrap();
    assert_eq!(mirror.snapshot().unwrap().stream, "6e");
}

/// Every `studio.agent.*` operation, as the owner's devices send them.
fn agent_operations() -> Vec<Operation> {
    use crate::agent::{JobEdit, MemoryEdit, Mode, Ran};
    let agent = || "alice".to_string();
    vec![
        Operation::ListAgents {},
        Operation::AskAgent {
            agent: agent(),
            text: "run the atif tests".into(),
            workspace: None,
            context: String::new(),
            mode: Mode::Auto,
            typist: true,
        },
        Operation::AnswerAgent {
            agent: agent(),
            step: 1,
            confirm: true,
        },
        Operation::AgentRan {
            agent: agent(),
            step: 1,
            ran: Ran {
                status: Some(0),
                ..Ran::default()
            },
        },
        Operation::StopAgent {
            agent: agent(),
            reason: "enough".into(),
        },
        Operation::ListAgentMemory {
            agent: agent(),
            after: None,
        },
        Operation::EditAgentMemory {
            agent: agent(),
            edit: MemoryEdit::Note {
                text: "a note".into(),
            },
        },
        Operation::ListAgentJobs { agent: agent() },
        Operation::EditAgentJobs {
            agent: agent(),
            edit: JobEdit::Pause {
                job: "nightly-check".into(),
            },
        },
        Operation::AgentLog {
            agent: agent(),
            after: None,
        },
        Operation::ListAgentWorkspaces {},
        Operation::NewAgent {
            agent: agent(),
            workspace: "/Users/me/code/app".into(),
        },
    ]
}

/// Only the owner talks to the workshop agent: the owner's own key and a
/// device the owner granted `observe` and `operate` reach her, and a
/// device that may only be in the world with her, such as a guest in a
/// shared Everglade, is refused every `studio.agent.*` operation at the
/// host, before the task owner sees it.
#[tokio::test]
async fn only_the_owner_and_owner_granted_devices_reach_the_workshop_agent() {
    let f = Fixture::served(0, false).await;
    let (_, guest) = f.enroll("world").await;
    for op in agent_operations() {
        let error = guest.call(op.clone()).await.unwrap_err();
        assert_eq!(error.code, Code::MissingRight, "{}", op.name());
    }
    assert_eq!(f.recorder.count(), 0, "nothing reached her");
    // A device with `observe` only reads; it cannot command her.
    let (_, watcher) = f.enroll("observe").await;
    for op in agent_operations() {
        let answer = watcher.call(op.clone()).await;
        if op.required() == Some(Right::Operate) {
            assert_eq!(
                answer.unwrap_err().code,
                Code::MissingRight,
                "{}",
                op.name()
            );
        } else {
            assert!(answer.is_ok(), "{}", op.name());
        }
    }
    // The owner's key, and a device the owner granted `operate`, reach
    // her with every operation.
    let before = f.recorder.count();
    let (_, phone) = f.enroll("observe,operate").await;
    for client in [f.owner_client(), phone] {
        for op in agent_operations() {
            let Outcome::Agent { agent } = client.call(op.clone()).await.unwrap() else {
                panic!("{} answers as the agent", op.name());
            };
            assert_eq!(agent["dispatched"], "alice");
        }
    }
    assert_eq!(f.recorder.count(), before + 2 * agent_operations().len());
}
