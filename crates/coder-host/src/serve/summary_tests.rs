//! Activity summaries for studio decisions: a waiting task's headline from
//! host state, and a goal decision's own summary until it is answered.
use std::collections::BTreeMap;

use nostr::activity_summary::{self, Attention, Phase};

use super::{activity, goal_summaries};
use crate::tasks::{GoalDecision, Note, TaskRef};

fn host() -> String {
    coder_reach::pubkey(&secp256k1::SecretKey::from_byte_array([3; 32]).unwrap())
}

fn waiting(revision: u64) -> TaskRef {
    TaskRef {
        task: "a".repeat(64),
        revision,
        phase: Phase::Waiting,
    }
}

#[test]
fn a_studio_approval_raises_its_summary_with_the_seat_and_title() {
    let headline = "ada asks for approval: Parse the flag";
    let summary = activity(
        &host(),
        &waiting(7),
        Some(Note::Approval),
        Some(headline),
        1_790_000_000,
    )
    .unwrap();
    assert_eq!(
        (summary.phase, summary.attention, summary.sequence),
        (Phase::Waiting, Attention::Approval, 7)
    );
    assert_eq!(summary.headline, headline);
    // A question asks for input the same way.
    let asked = activity(
        &host(),
        &waiting(8),
        Some(Note::Question),
        Some("ada has a question: Parse the flag"),
        1_790_000_000,
    )
    .unwrap();
    assert_eq!(asked.attention, Attention::Input);
    // Without a studio headline, the note's generic one stands.
    let plain = activity(
        &host(),
        &waiting(7),
        Some(Note::Approval),
        None,
        1_790_000_000,
    )
    .unwrap();
    assert_eq!(plain.headline, "Coder asked for approval");
    // A studio headline never replaces the headline of a task that asks
    // for nothing.
    let running = TaskRef {
        phase: Phase::Running,
        ..waiting(9)
    };
    let summary = activity(&host(), &running, None, Some(headline), 1).unwrap();
    assert_eq!(summary.headline, "Task running");
    assert_eq!(summary.attention, Attention::None);
}

#[test]
fn a_goal_decision_raises_its_own_summary_until_it_is_answered() {
    let host = host();
    let decision = GoalDecision {
        subject: "b".repeat(64),
        sequence: 5,
        headline: "lead finished without a plan".into(),
    };
    let mut raised = BTreeMap::new();
    let open = goal_summaries(&host, &mut raised, std::slice::from_ref(&decision), 10);
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].subject, decision.subject);
    assert_eq!(
        (open[0].phase, open[0].attention, open[0].sequence),
        (Phase::Waiting, Attention::Input, 10)
    );
    assert_eq!(open[0].headline, "lead finished without a plan");
    // Raised once: the next sweep publishes nothing new.
    assert!(goal_summaries(&host, &mut raised, std::slice::from_ref(&decision), 11).is_empty());
    // Answered: a summary that asks for nothing supersedes it.
    let closed = goal_summaries(&host, &mut raised, &[], 12);
    assert_eq!(closed.len(), 1);
    assert_eq!(
        (closed[0].phase, closed[0].attention, closed[0].sequence),
        (Phase::Running, Attention::None, 11)
    );
    assert!(activity_summary::supersedes(&open[0], &closed[0]).unwrap());
    assert!(raised.is_empty());
    // A later decision of the same goal supersedes both.
    let later = GoalDecision {
        sequence: 6,
        ..decision
    };
    let reopened = goal_summaries(&host, &mut raised, std::slice::from_ref(&later), 13);
    assert_eq!(reopened[0].sequence, 12);
    assert!(activity_summary::supersedes(&closed[0], &reopened[0]).unwrap());
}
