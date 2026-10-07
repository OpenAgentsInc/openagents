//! The Agent Studio's signals (`docs/verse/agentcraft-parity.md`,
//! Signals): the goal the atrium and the HUD show, the count of decisions
//! waiting on the person, and the sounds and notices a change calls for.
//!
//! This decides *what* and *when*; [`deliver`] plays the sound and raises
//! the notice on desktop. A signal follows a change between two snapshots,
//! as `openagents_chat_app::cues` follows a change of a chat's activity:
//!
//! - A decision the studio has not shown before rings the [`Signal::Bell`].
//! - A task that comes to `done` plays the [`Signal::Chime`].
//! - A goal that comes to `done` plays the bigger [`Signal::Fanfare`].
//!
//! The first snapshot after the studio starts observing is only recorded,
//! so entering Everglade on finished work plays nothing, and a snapshot
//! that changes nothing plays nothing again.

use coder_access::studio::{GoalStatus, TaskStatus, View};
use std::collections::{BTreeMap, BTreeSet};

#[cfg(feature = "desktop")]
pub mod deliver;

/// Which sound a studio change plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Signal {
    /// A new decision waits on the person: the request cue's rising pair.
    Bell,
    /// A task finished: the done cue's settling pair.
    Chime,
    /// A goal finished: a rising four-note arpeggio, bigger than a task's.
    Fanfare,
}

impl Signal {
    /// Where the signal sorts when several arrive at once: lower needs the
    /// person sooner, then the bigger news.
    #[must_use]
    pub fn rank(self) -> u8 {
        match self {
            Signal::Bell => 0,
            Signal::Fanfare => 1,
            Signal::Chime => 2,
        }
    }

    /// The one signal to play for `signals` that arrived together, so a
    /// burst of changes plays one sound rather than a chord.
    #[must_use]
    pub fn most_urgent(signals: impl IntoIterator<Item = Signal>) -> Option<Signal> {
        signals.into_iter().min_by_key(|signal| signal.rank())
    }

    /// The notes the signal plays: (frequency in hertz, start in seconds).
    /// The bell and the chime are the desktop app's request and done cues,
    /// so a studio decision sounds like a chat's question.
    #[must_use]
    pub fn notes(self) -> &'static [(f32, f32)] {
        match self {
            // D5 rising to A5: the request cue.
            Signal::Bell => &[(587.33, 0.0), (880.0, 0.11)],
            // G5 settling to C5: the done cue.
            Signal::Chime => &[(783.99, 0.0), (523.25, 0.11)],
            // C5, E5, G5, and C6.
            Signal::Fanfare => &[(523.25, 0.0), (659.25, 0.1), (783.99, 0.2), (1046.5, 0.3)],
        }
    }
}

/// One change worth a signal, with what it is about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    pub signal: Signal,
    /// The decision's question, the task's title, or the goal's text.
    pub text: String,
}

impl Event {
    /// The desktop notice's title and body.
    #[must_use]
    pub fn notice(&self) -> (&'static str, String) {
        let title = match self.signal {
            Signal::Bell => "Agent Studio: a decision is waiting",
            Signal::Chime => "Agent Studio: a task finished",
            Signal::Fanfare => "Agent Studio: a goal finished",
        };
        (title, clip(&self.text, NOTICE_CHARS))
    }
}

/// The most events the studio holds for a host that has not taken them.
pub const MAX_PENDING: usize = 32;

/// The most characters of a notice's body.
const NOTICE_CHARS: usize = 160;

/// `text` cut to at most `max` characters, with an ellipsis when cut.
fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// What the studio last showed, for finding what changed.
#[derive(Debug, Default)]
pub struct Signals {
    /// Whether a snapshot has been recorded since the last reset.
    primed: bool,
    decisions: BTreeSet<String>,
    tasks: BTreeMap<String, TaskStatus>,
    goals: BTreeMap<String, GoalStatus>,
}

impl Signals {
    /// Forgets what the studio showed, so the next view is only recorded.
    /// The studio resets it when it stops observing.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Records `view` and returns the events its changes call for, in the
    /// order of the view's lists: new decisions, finished tasks, then
    /// finished goals. Nothing on the first view after a reset, and nothing
    /// for a decision, task, or goal first seen already finished.
    pub fn observe(&mut self, view: &View) -> Vec<Event> {
        let primed = std::mem::replace(&mut self.primed, true);
        let mut events = Vec::new();
        let decisions: BTreeSet<String> =
            view.decisions.iter().map(|d| d.decision.clone()).collect();
        let tasks: BTreeMap<String, TaskStatus> = view
            .tasks
            .iter()
            .map(|t| (t.task.clone(), t.status))
            .collect();
        let goals: BTreeMap<String, GoalStatus> = view
            .goals
            .iter()
            .map(|g| (g.goal.clone(), g.status))
            .collect();
        if primed {
            for decision in &view.decisions {
                if !self.decisions.contains(&decision.decision) {
                    events.push(Event {
                        signal: Signal::Bell,
                        text: decision.text.clone(),
                    });
                }
            }
            for task in &view.tasks {
                let before = self.tasks.get(&task.task).copied();
                if task.status == TaskStatus::Done && before.is_some_and(|b| b != TaskStatus::Done)
                {
                    events.push(Event {
                        signal: Signal::Chime,
                        text: task.title.clone(),
                    });
                }
            }
            for goal in &view.goals {
                let before = self.goals.get(&goal.goal).copied();
                if goal.status == GoalStatus::Done && before.is_some_and(|b| b != GoalStatus::Done)
                {
                    events.push(Event {
                        signal: Signal::Fanfare,
                        text: goal.text.clone(),
                    });
                }
            }
        }
        self.decisions = decisions;
        self.tasks = tasks;
        self.goals = goals;
        events
    }
}

/// The goal the atrium and the HUD's goal bar show, with the count of
/// decisions waiting on the person.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    /// The goal as submitted.
    pub text: String,
    pub status: GoalStatus,
    /// Plan tasks that are over, of `total`.
    pub done: u32,
    pub total: u32,
    /// Open decisions across every goal.
    pub waiting: usize,
}

impl Summary {
    /// The goal to show from `view`: the newest goal still under way.
    /// `None` when every goal is done, so a finished goal's header clears,
    /// or for a studio with no goal.
    #[must_use]
    pub fn of(view: &View) -> Option<Self> {
        let goal = view
            .goals
            .iter()
            .filter(|g| g.status != GoalStatus::Done)
            .max_by_key(|g| g.submitted_at)?;
        Some(Self {
            text: goal.text.clone(),
            status: goal.status,
            done: goal.final_tasks.min(goal.total_tasks),
            total: goal.total_tasks,
            waiting: view.decisions.len(),
        })
    }

    /// The share of the goal's plan tasks that are over, `0..=1`. A goal
    /// with no plan yet is at zero, and a finished goal is full.
    #[must_use]
    pub fn progress(&self) -> f32 {
        if self.status == GoalStatus::Done {
            return 1.0;
        }
        if self.total == 0 {
            return 0.0;
        }
        (self.done as f32 / self.total as f32).clamp(0.0, 1.0)
    }

    /// The task count, such as `3/5 tasks`, or the goal's state while it
    /// has no plan.
    #[must_use]
    pub fn counts(&self) -> String {
        if self.total == 0 {
            return match self.status {
                GoalStatus::Planning => "planning".into(),
                GoalStatus::Decision => "waiting on you".into(),
                GoalStatus::Running => "running".into(),
                GoalStatus::Done => "done".into(),
            };
        }
        format!("{}/{} tasks", self.done, self.total)
    }
}

/// The waiting badge's text, such as `2 waiting · press J`, or `None` when
/// nothing waits. `J` opens the decisions panel.
#[must_use]
pub fn badge(waiting: usize) -> Option<String> {
    (waiting > 0).then(|| format!("{waiting} waiting · press J"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_access::studio::{Decision, DecisionKind, Goal, Task};

    fn goal(id: &str, status: GoalStatus, done: u32, total: u32, at: u64) -> Goal {
        Goal {
            spend: Default::default(),
            goal: id.into(),
            text: format!("goal {id}"),
            workspace: "repo".into(),
            lead: "lead".into(),
            status,
            final_tasks: done,
            total_tasks: total,
            submitted_at: at,
        }
    }

    fn task(id: &str, status: TaskStatus) -> Task {
        Task {
            spend: Default::default(),
            task: id.into(),
            goal: "g1".into(),
            entry: id.into(),
            position: 0,
            title: format!("task {id}"),
            seat: "ada".into(),
            depends_on: Vec::new(),
            status,
        }
    }

    fn decision(id: &str) -> Decision {
        Decision {
            decision: id.into(),
            goal: "g1".into(),
            task: None,
            seat: None,
            kind: DecisionKind::Question,
            text: format!("question {id}"),
            based_on: 0,
            approval: None,
        }
    }

    fn view(goals: Vec<Goal>, tasks: Vec<Task>, decisions: Vec<Decision>) -> View {
        View {
            goals,
            tasks,
            decisions,
            ..View::default()
        }
    }

    fn signals(events: Vec<Event>) -> Vec<Signal> {
        events.into_iter().map(|e| e.signal).collect()
    }

    #[test]
    fn each_change_maps_to_its_signal() {
        let mut tracker = Signals::default();
        let running = || goal("g1", GoalStatus::Running, 0, 2, 1);
        let start = view(
            vec![running()],
            vec![
                task("t1", TaskStatus::Running),
                task("t2", TaskStatus::Queued),
            ],
            vec![],
        );
        assert!(tracker.observe(&start).is_empty());
        let asked = view(
            vec![running()],
            vec![
                task("t1", TaskStatus::Waiting),
                task("t2", TaskStatus::Queued),
            ],
            vec![decision("d1")],
        );
        let events = tracker.observe(&asked);
        assert_eq!(signals(events.clone()), [Signal::Bell]);
        assert_eq!(events[0].text, "question d1");
        let done = view(
            vec![goal("g1", GoalStatus::Running, 1, 2, 1)],
            vec![
                task("t1", TaskStatus::Done),
                task("t2", TaskStatus::Running),
            ],
            vec![],
        );
        assert_eq!(signals(tracker.observe(&done)), [Signal::Chime]);
        let finished = view(
            vec![goal("g1", GoalStatus::Done, 2, 2, 1)],
            vec![task("t1", TaskStatus::Done), task("t2", TaskStatus::Done)],
            vec![],
        );
        let events = tracker.observe(&finished);
        assert_eq!(signals(events.clone()), [Signal::Chime, Signal::Fanfare]);
        assert_eq!(
            Signal::most_urgent(events.iter().map(|e| e.signal)),
            Some(Signal::Fanfare)
        );
        // A failed or cancelled task is not a chime.
        let mut tracker = Signals::default();
        tracker.observe(&view(vec![], vec![task("t1", TaskStatus::Running)], vec![]));
        for status in [
            TaskStatus::Failed,
            TaskStatus::Cancelled,
            TaskStatus::Blocked,
        ] {
            assert!(
                tracker
                    .observe(&view(vec![], vec![task("t1", status)], vec![]))
                    .is_empty(),
                "{status:?}"
            );
        }
    }

    #[test]
    fn a_signal_plays_once_per_change() {
        let mut tracker = Signals::default();
        let open = view(
            vec![],
            vec![task("t1", TaskStatus::Running)],
            vec![decision("d1")],
        );
        // Entering on an open decision plays nothing.
        assert!(tracker.observe(&open).is_empty());
        assert!(tracker.observe(&open).is_empty());
        let two = view(
            vec![],
            vec![task("t1", TaskStatus::Running)],
            vec![decision("d1"), decision("d2")],
        );
        assert_eq!(signals(tracker.observe(&two)), [Signal::Bell]);
        assert!(tracker.observe(&two).is_empty());
        let done = view(
            vec![],
            vec![task("t1", TaskStatus::Done)],
            vec![decision("d2")],
        );
        assert_eq!(signals(tracker.observe(&done)), [Signal::Chime]);
        assert!(tracker.observe(&done).is_empty());
        // A task first seen already done, and a reset, play nothing.
        let more = view(
            vec![],
            vec![task("t1", TaskStatus::Done), task("t9", TaskStatus::Done)],
            vec![decision("d2")],
        );
        assert!(tracker.observe(&more).is_empty());
        tracker.reset();
        let fresh = view(vec![], vec![], vec![decision("d3")]);
        assert!(tracker.observe(&fresh).is_empty());
        assert!(tracker.observe(&fresh).is_empty());
    }

    #[test]
    fn a_burst_plays_the_most_urgent_signal() {
        assert_eq!(
            Signal::most_urgent([Signal::Chime, Signal::Fanfare, Signal::Bell]),
            Some(Signal::Bell)
        );
        assert_eq!(
            Signal::most_urgent([Signal::Chime, Signal::Fanfare]),
            Some(Signal::Fanfare)
        );
        assert_eq!(Signal::most_urgent(std::iter::empty()), None);
        // The goal's fanfare has more notes than a task's chime.
        assert!(Signal::Fanfare.notes().len() > Signal::Chime.notes().len());
        assert_ne!(Signal::Bell.notes(), Signal::Chime.notes());
    }

    #[test]
    fn the_summary_shows_the_newest_goal_under_way() {
        assert_eq!(Summary::of(&View::default()), None);
        let studio = view(
            vec![
                goal("old", GoalStatus::Running, 1, 4, 1),
                goal("new", GoalStatus::Done, 3, 3, 9),
            ],
            vec![],
            vec![decision("d1"), decision("d2")],
        );
        let summary = Summary::of(&studio).unwrap();
        assert_eq!(summary.text, "goal old");
        assert_eq!(summary.counts(), "1/4 tasks");
        assert_eq!(summary.waiting, 2);
        assert!((summary.progress() - 0.25).abs() < 1e-6);
        // A finished goal's header clears.
        let finished = view(vec![goal("new", GoalStatus::Done, 3, 3, 9)], vec![], vec![]);
        assert_eq!(Summary::of(&finished), None);
        let planning = view(
            vec![goal("p", GoalStatus::Planning, 0, 0, 2)],
            vec![],
            vec![],
        );
        let summary = Summary::of(&planning).unwrap();
        assert_eq!(summary.progress(), 0.0);
        assert_eq!(summary.counts(), "planning");
    }

    #[test]
    fn the_badge_names_the_waiting_count_and_the_key() {
        assert_eq!(badge(0), None);
        assert_eq!(badge(3).as_deref(), Some("3 waiting · press J"));
    }

    #[test]
    fn a_notice_names_the_change_and_clips_its_text() {
        let event = Event {
            signal: Signal::Bell,
            text: "x".repeat(400),
        };
        let (title, body) = event.notice();
        assert!(title.contains("decision"));
        assert_eq!(body.chars().count(), NOTICE_CHARS);
        assert!(body.ends_with('…'));
    }
}
