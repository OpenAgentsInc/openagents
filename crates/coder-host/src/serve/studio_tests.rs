//! The host's merge decision over a task owner: the stale refusal, the
//! refusal of an unfinished task, each verdict's effect, a retried merge
//! after it published, and the owner's sentence carried with a refusal.
use std::sync::Mutex;

use coder_access::Code;
use coder_access::protocol::{CommandAction, TaskCommand, TaskCreate};
use coder_access::review::{Completeness, Landing, Publication, PublishState, TaskReview};
use coder_access::studio::{MergeDecision, Verdict};
use nostr::activity_summary::Phase;

use super::{studio_merge, studio_stream};
use crate::tasks::{Principal, Reviewed, Standing, TaskRef, Tasks};

const TASK: &str = "studio-g1-0011aabb-first";

fn rev(c: char) -> String {
    c.to_string().repeat(40)
}

/// A task owner whose one task's worktree is at `base`/`base`/`head`, and
/// which records each effect it is asked for.
#[derive(Default)]
struct Owner {
    head: Mutex<String>,
    publication: Mutex<Option<Publication>>,
    effects: Mutex<Vec<String>>,
    commands: Mutex<Vec<TaskCommand>>,
    /// The task's phase; `None` is completed.
    phase: Mutex<Option<Phase>>,
    /// A refusal the landing path answers, with its sentence.
    refuse: Mutex<Option<(Code, &'static str)>>,
}

impl Owner {
    fn at(head: &str) -> Self {
        let owner = Owner::default();
        *owner.head.lock().unwrap() = head.into();
        owner
    }
    fn effects(&self) -> Vec<String> {
        self.effects.lock().unwrap().clone()
    }
    fn in_phase(self, phase: Phase) -> Self {
        *self.phase.lock().unwrap() = Some(phase);
        self
    }
}

impl Tasks for Owner {
    fn create(&self, _: &str, _: &str, _: &TaskCreate) -> Result<TaskRef, Code> {
        Err(Code::Unsupported)
    }
    fn steer(&self, _: &str, _: &str, _: &str, _: u64, _: &str) -> Result<TaskRef, Code> {
        Err(Code::Unsupported)
    }
    fn cancel(&self, _: &str, _: &str, _: &str, _: u64, _: &str) -> Result<TaskRef, Code> {
        Err(Code::Unsupported)
    }
    fn current(&self) -> Vec<TaskRef> {
        vec![TaskRef {
            task: TASK.into(),
            revision: 7,
            phase: self.phase.lock().unwrap().unwrap_or(Phase::Completed),
        }]
    }
    fn review(&self, task: &str) -> Result<TaskReview, Code> {
        Ok(TaskReview {
            task: task.into(),
            base: rev('a'),
            head_commit: rev('a'),
            head: self.head.lock().unwrap().clone(),
            files: Vec::new(),
            files_total: 0,
            added: 0,
            removed: 0,
            uncounted: 0,
            diff: String::new(),
            completeness: Completeness::Complete,
            publication: self.publication.lock().unwrap().clone(),
        })
    }
    fn publish(
        &self,
        _principal: &Principal,
        task: &str,
        reviewed: &Reviewed,
    ) -> Result<Publication, Code> {
        if let Some((code, reason)) = *self.refuse.lock().unwrap() {
            return Err(crate::tasks::refuse(code, reason));
        }
        self.effects.lock().unwrap().push("publish".into());
        let publication = Publication {
            operation: "e".repeat(64),
            task: task.into(),
            base: reviewed.base.clone(),
            head_commit: reviewed.head_commit.clone(),
            head: reviewed.head.clone(),
            landing: Landing::Branch,
            state: PublishState::Published,
            branch: Some("main".into()),
            commit: Some(rev('f')),
            url: None,
            note: "Pushed onto main.".into(),
        };
        *self.publication.lock().unwrap() = Some(publication.clone());
        Ok(publication)
    }
    fn command(
        &self,
        _principal: &Principal,
        command: &TaskCommand,
        _standing: Standing<'_>,
    ) -> Result<TaskRef, Code> {
        self.effects.lock().unwrap().push("command".into());
        self.commands.lock().unwrap().push(command.clone());
        Ok(TaskRef {
            task: command.task.clone(),
            revision: 8,
            phase: Phase::Queued,
        })
    }
    fn studio_reject(
        &self,
        _principal: &Principal,
        task: &str,
        reviewed: &Reviewed,
        reason: &str,
    ) -> Result<(), Code> {
        self.effects
            .lock()
            .unwrap()
            .push(format!("reject {task} {} {reason}", reviewed.head));
        Ok(())
    }
}

fn decision(verdict: Verdict, head: &str, text: &str) -> MergeDecision {
    MergeDecision {
        task: TASK.into(),
        base: rev('a'),
        head_commit: rev('a'),
        head: head.into(),
        verdict,
        text: text.into(),
        command: "c".repeat(64),
        issued_at: 1_790_000_000,
    }
}

fn principal() -> Principal {
    Principal {
        device: "d".repeat(64),
        grant: Some("9".repeat(64)),
        epoch: Some(0),
    }
}

fn always(_: &Principal) -> bool {
    true
}

#[test]
fn a_decision_on_a_moved_worktree_is_stale_and_does_nothing() {
    let owner = Owner::at(&rev('c'));
    for (verdict, text) in [
        (Verdict::Merge, ""),
        (Verdict::RequestChanges, "Name the flag."),
        (Verdict::Reject, ""),
    ] {
        let refused = studio_merge(
            &owner,
            &principal(),
            &decision(verdict, &rev('b'), text),
            &always,
        )
        .unwrap_err();
        assert_eq!(refused.code, Code::Stale, "{verdict:?}");
        assert!(refused.reason.is_some(), "a stale refusal says why");
    }
    assert!(owner.effects().is_empty());
}

#[test]
fn a_merge_publishes_the_reviewed_revisions_once_and_a_retry_answers_it_again() {
    let owner = Owner::at(&rev('b'));
    let (merged, changed) = studio_merge(
        &owner,
        &principal(),
        &decision(Verdict::Merge, &rev('b'), ""),
        &always,
    )
    .unwrap();
    let publication = merged.publication.clone().unwrap();
    assert_eq!(publication.head, rev('b'));
    assert!(changed.is_none());
    merged.validate().unwrap();
    // The landing moved the worktree; a retry of the same merge still
    // answers, through the owner's publication keyed by the review.
    *owner.head.lock().unwrap() = rev('d');
    let (again, _) = studio_merge(
        &owner,
        &principal(),
        &decision(Verdict::Merge, &rev('b'), ""),
        &always,
    )
    .unwrap();
    assert_eq!(again.publication, Some(publication));
    // A request for changes at the old head is stale now.
    assert_eq!(
        studio_merge(
            &owner,
            &principal(),
            &decision(Verdict::RequestChanges, &rev('b'), "More tests."),
            &always,
        )
        .unwrap_err()
        .code,
        Code::Stale
    );
    assert_eq!(owner.effects(), ["publish", "publish"]);
}

#[test]
fn request_changes_is_the_same_seats_next_turn_and_reject_is_recorded() {
    let owner = Owner::at(&rev('b'));
    let (merged, changed) = studio_merge(
        &owner,
        &principal(),
        &decision(Verdict::RequestChanges, &rev('b'), "Name the flag."),
        &always,
    )
    .unwrap();
    assert_eq!(merged.verdict, Verdict::RequestChanges);
    assert!(merged.publication.is_none());
    assert_eq!(changed.unwrap().revision, 8);
    let commands = owner.commands.lock().unwrap().clone();
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].action, CommandAction::Send);
    assert_eq!(commands[0].command, "c".repeat(64));
    assert_eq!(commands[0].text, "Name the flag.");
    assert_eq!(commands[0].based_on, 7);
    let (merged, _) = studio_merge(
        &owner,
        &principal(),
        &decision(Verdict::Reject, &rev('b'), "Out of scope."),
        &always,
    )
    .unwrap();
    assert_eq!(merged.verdict, Verdict::Reject);
    assert_eq!(
        owner.effects(),
        [
            "command".to_owned(),
            format!("reject {TASK} {} Out of scope.", rev('b'))
        ]
    );
}

#[test]
fn a_merge_or_request_for_changes_of_an_unfinished_task_is_refused_with_a_reason() {
    for phase in [
        Phase::Queued,
        Phase::Running,
        Phase::Waiting,
        Phase::Failed,
        Phase::Cancelled,
    ] {
        let owner = Owner::at(&rev('b')).in_phase(phase);
        for (verdict, text) in [
            (Verdict::Merge, ""),
            (Verdict::RequestChanges, "Name the flag."),
        ] {
            let refused = studio_merge(
                &owner,
                &principal(),
                &decision(verdict, &rev('b'), text),
                &always,
            )
            .unwrap_err();
            assert_eq!(refused.code, Code::Conflict, "{phase:?} {verdict:?}");
            let reason = refused.reason.expect("the refusal says why");
            assert!(
                reason.contains("waiting for review or done"),
                "{phase:?}: {reason}"
            );
        }
        // Nothing landed and no turn was sent.
        assert!(owner.effects().is_empty(), "{phase:?}");
    }
    let queued = Owner::at(&rev('b')).in_phase(Phase::Queued);
    let refused = studio_merge(
        &queued,
        &principal(),
        &decision(Verdict::Merge, &rev('b'), ""),
        &always,
    )
    .unwrap_err();
    assert!(
        refused
            .reason
            .unwrap()
            .contains("is queued and has not run"),
        "the sentence says where the task is"
    );
    // **Reject** still closes an unfinished task.
    let (merged, _) = studio_merge(
        &queued,
        &principal(),
        &decision(Verdict::Reject, &rev('b'), "Not needed."),
        &always,
    )
    .unwrap();
    assert_eq!(merged.verdict, Verdict::Reject);
    assert_eq!(
        queued.effects(),
        [format!("reject {TASK} {} Not needed.", rev('b'))]
    );
}

#[test]
fn an_owner_refusal_carries_the_owners_sentence() {
    let owner = Owner::at(&rev('b'));
    *owner.refuse.lock().unwrap() = Some((
        Code::Conflict,
        "The studio's checkout has uncommitted changes.",
    ));
    let refused = studio_merge(
        &owner,
        &principal(),
        &decision(Verdict::Merge, &rev('b'), ""),
        &always,
    )
    .unwrap_err();
    assert_eq!(refused.code, Code::Conflict);
    assert_eq!(
        refused.reason.as_deref(),
        Some("The studio's checkout has uncommitted changes.")
    );
    // A sentence noted for another code is not carried.
    let _ = crate::tasks::refuse(Code::Bounds, "Too many.");
    assert_eq!(crate::tasks::take_reason(Code::Conflict), None);
    assert_eq!(crate::tasks::take_reason(Code::Bounds), None, "taken once");
}

#[test]
fn each_process_is_its_own_studio_stream() {
    let stream = studio_stream();
    coder_access::studio::stream_id(stream.id()).unwrap();
    assert_eq!(stream.sequence(), 0);
}
