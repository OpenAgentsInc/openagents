//! Which chat needs the person: one attention value over Coder task state.
//!
//! A chat with Coder work shows one [`Indicator`]: awaiting input, errored,
//! working, stale, completed and unseen, or idle. The desktop sidebar orders
//! by it ([`crate::chat_list::by_attention`]), and every other surface that
//! signals attention (sounds, notifications, the world's lamps) reads the
//! same value rather than deriving its own.
//!
//! A task that says it is working but has sent no event for
//! [`STALE_AFTER`] shows as [`Indicator::Stale`], never as working, so a
//! crashed engine cannot look busy forever. A question or an approval is
//! durable host state and never goes stale.
//!
//! Reimplemented from Zeron's `ChatIndicator`, `effective_indicator`, and
//! `attention_rank` (public MIT zeronsh/zeron at `9e1a1115`,
//! `crates/proto/src/entities.rs` and `crates/proto/src/view.rs`). Zeron
//! folds a stale session into idle and relies on a 45-second engine
//! heartbeat; Coder has no heartbeat, so stale is its own value and the
//! bound outlasts one command.

use nostr::activity_summary::{ActivitySummary, Attention, Phase};
use std::time::Duration;

/// How long a working task may go without an event before it shows as
/// stale. Coder emits no heartbeat, and one command may run for its full
/// 300-second deadline (`microcoder_loop::run::Limits::command_seconds`)
/// before its step is recorded, so the bound is twice that.
pub const STALE_AFTER: Duration = Duration::from_secs(600);

/// What a chat's row shows, most urgent first.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Indicator {
    /// A question or an approval waits for the person.
    AwaitingInput,
    /// The task failed, and the person has not seen it yet.
    Errored,
    /// The task is running and has sent an event within [`STALE_AFTER`].
    Working,
    /// The task says it is running but has been silent past
    /// [`STALE_AFTER`].
    Stale,
    /// The task finished, and the person has not seen it yet.
    Completed,
    /// Nothing needs the person.
    #[default]
    Idle,
}

impl Indicator {
    /// Where the indicator sorts in a list: lower needs the person sooner.
    #[must_use]
    pub fn rank(self) -> u8 {
        match self {
            Indicator::AwaitingInput => 0,
            Indicator::Errored => 1,
            Indicator::Working => 2,
            Indicator::Stale => 3,
            Indicator::Completed => 4,
            Indicator::Idle => 5,
        }
    }

    /// A short label for a row, or `None` when there is nothing to say.
    #[must_use]
    pub fn label(self) -> Option<&'static str> {
        match self {
            Indicator::AwaitingInput => Some("Needs you"),
            Indicator::Errored => Some("Failed"),
            Indicator::Working => Some("Working"),
            Indicator::Stale => Some("Stale"),
            Indicator::Completed => Some("Done"),
            Indicator::Idle => None,
        }
    }
}

/// Where a task is, as its source reports it, before the stale rule and
/// the seen marker apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Activity {
    /// Queued, starting, or running.
    Working,
    /// Waiting for an answer or an approval.
    AwaitingInput,
    /// The last turn failed.
    Failed,
    /// The last turn finished with a result.
    Completed,
    /// Stopped, cancelled, or unknown: nothing to show.
    Idle,
}

/// The indicator for `activity`, last heard from `silent` ago, where `seen`
/// says the person has seen the task's current ending.
#[must_use]
pub fn indicator(activity: Activity, silent: Duration, seen: bool) -> Indicator {
    match activity {
        Activity::AwaitingInput => Indicator::AwaitingInput,
        Activity::Working if silent > STALE_AFTER => Indicator::Stale,
        Activity::Working => Indicator::Working,
        Activity::Failed if !seen => Indicator::Errored,
        Activity::Completed if !seen => Indicator::Completed,
        Activity::Failed | Activity::Completed | Activity::Idle => Indicator::Idle,
    }
}

/// The activity a host's task summary reports.
#[must_use]
pub fn summary_activity(summary: &ActivitySummary) -> Activity {
    match (summary.phase, summary.attention) {
        (Phase::Waiting, Attention::Approval | Attention::Input) => Activity::AwaitingInput,
        (Phase::Queued | Phase::Running | Phase::Waiting, _) => Activity::Working,
        (Phase::Completed, _) => Activity::Completed,
        (Phase::Failed, _) => Activity::Failed,
        (Phase::Cancelled | Phase::Unknown, _) => Activity::Idle,
    }
}

/// The indicator for a host's task summary at Unix second `now`. A summary
/// dated after `now` counts as just heard.
#[must_use]
pub fn of_summary(summary: &ActivitySummary, now: u64, seen: bool) -> Indicator {
    let silent = Duration::from_secs(now.saturating_sub(summary.updated_at));
    indicator(summary_activity(summary), silent, seen)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::activity_summary::SubjectKind;

    const LIVE: Duration = Duration::from_secs(5);

    #[test]
    fn each_activity_has_its_indicator() {
        assert_eq!(
            indicator(Activity::AwaitingInput, LIVE, false),
            Indicator::AwaitingInput
        );
        assert_eq!(indicator(Activity::Failed, LIVE, false), Indicator::Errored);
        assert_eq!(
            indicator(Activity::Working, LIVE, false),
            Indicator::Working
        );
        assert_eq!(
            indicator(Activity::Completed, LIVE, false),
            Indicator::Completed
        );
        assert_eq!(indicator(Activity::Idle, LIVE, false), Indicator::Idle);
    }

    #[test]
    fn a_seen_ending_is_idle() {
        assert_eq!(indicator(Activity::Completed, LIVE, true), Indicator::Idle);
        assert_eq!(indicator(Activity::Failed, LIVE, true), Indicator::Idle);
        // A question still needs the person after they look at it.
        assert_eq!(
            indicator(Activity::AwaitingInput, LIVE, true),
            Indicator::AwaitingInput
        );
    }

    #[test]
    fn a_silent_working_task_is_stale_never_working() {
        assert_eq!(
            indicator(Activity::Working, STALE_AFTER, false),
            Indicator::Working,
            "the bound itself is still working"
        );
        let past = STALE_AFTER + Duration::from_secs(1);
        assert_eq!(indicator(Activity::Working, past, false), Indicator::Stale);
        assert_eq!(indicator(Activity::Working, past, true), Indicator::Stale);
        // Waiting is durable: an unanswered question never goes stale.
        assert_eq!(
            indicator(Activity::AwaitingInput, Duration::from_secs(86_400), false),
            Indicator::AwaitingInput
        );
    }

    #[test]
    fn ranks_put_the_person_first() {
        let mut all = [
            Indicator::Idle,
            Indicator::Completed,
            Indicator::Stale,
            Indicator::Working,
            Indicator::Errored,
            Indicator::AwaitingInput,
        ];
        all.sort_by_key(|indicator| indicator.rank());
        assert_eq!(
            all,
            [
                Indicator::AwaitingInput,
                Indicator::Errored,
                Indicator::Working,
                Indicator::Stale,
                Indicator::Completed,
                Indicator::Idle,
            ]
        );
        assert_eq!(Indicator::Idle.label(), None);
        assert_eq!(Indicator::Stale.label(), Some("Stale"));
    }

    fn summary(phase: Phase, attention: Attention, updated_at: u64) -> ActivitySummary {
        ActivitySummary {
            host: "a".repeat(64),
            subject_kind: SubjectKind::Task,
            subject: "b".repeat(64),
            sequence: 1,
            phase,
            headline: String::new(),
            attention,
            updated_at,
        }
    }

    #[test]
    fn task_summaries_map_to_indicators() {
        let now = 10_000;
        let cases = [
            (Phase::Waiting, Attention::Input, Indicator::AwaitingInput),
            (
                Phase::Waiting,
                Attention::Approval,
                Indicator::AwaitingInput,
            ),
            (Phase::Running, Attention::None, Indicator::Working),
            (Phase::Queued, Attention::None, Indicator::Working),
            (Phase::Completed, Attention::Completed, Indicator::Completed),
            (Phase::Failed, Attention::Failed, Indicator::Errored),
            (Phase::Cancelled, Attention::None, Indicator::Idle),
            (Phase::Unknown, Attention::None, Indicator::Idle),
        ];
        for (phase, attention, expected) in cases {
            assert_eq!(
                of_summary(&summary(phase, attention, now - 1), now, false),
                expected,
                "{phase:?}"
            );
        }
        let silent = summary(Phase::Running, Attention::None, now - 601);
        assert_eq!(of_summary(&silent, now, false), Indicator::Stale);
        let ahead = summary(Phase::Running, Attention::None, now + 30);
        assert_eq!(of_summary(&ahead, now, false), Indicator::Working);
    }
}
