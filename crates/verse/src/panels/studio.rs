//! The Agent Studio's panels over Everglade (`docs/verse/agent-studio.md`,
//! "The studio in Verse"): the console at the notice board, a seat's panel
//! at its desk, the decisions at the podium, and the diff review at the
//! merge station.
//!
//! Each panel is a [`Panel`] of transcript rows in the chat palette, built
//! from the studio snapshot Everglade draws. The decisions read each
//! question or approval with the desktop app's question flow
//! (`openagents_chat_app::decision`), and the roster orders seats by the
//! app's attention value (`openagents_chat_app::attention`). The review is
//! the task's real diff in the changes tab.
//!
//! The panels observe. They send no studio intent: answering, steering,
//! and merging go through a host connection with the `operate` and `review`
//! rights, which Everglade does not open yet.

use super::{Intent, Panel, Tab, message, tool};
use crate::zones::everglade::studio::{PanelKind, word};
use coder_access::review::TaskReview;
use coder_access::studio::{Activity, DecisionKind, Seat, Task, TaskStatus, View};
use openagents_chat_app::{attention, decision};
use rust_native::{MessageRole, Node, ToolState};

/// The attention a seat doing `activity` shows in the roster: the desktop
/// app's indicator over the seat's activity. A seat whose task is over is
/// shown as seen, so only a waiting, failed, or working seat stands out.
#[must_use]
pub fn indicator(activity: Activity) -> attention::Indicator {
    let source = match activity {
        Activity::Waiting => attention::Activity::AwaitingInput,
        Activity::Failed | Activity::Blocked => attention::Activity::Failed,
        Activity::Reading
        | Activity::Editing
        | Activity::Running
        | Activity::Testing
        | Activity::Judging
        | Activity::Thinking => attention::Activity::Working,
        Activity::Done => attention::Activity::Completed,
        Activity::Idle | Activity::Paused => attention::Activity::Idle,
    };
    attention::indicator(source, std::time::Duration::ZERO, false)
}

/// The seat a panel of `kind` shows, if it shows one.
#[must_use]
pub fn seat<'a>(kind: &PanelKind, view: &'a View) -> Option<&'a Seat> {
    match kind {
        PanelKind::Seat(name) => view.seats.iter().find(|s| s.seat == *name),
        PanelKind::Desk(desk) => view.seats.iter().find(|s| s.desk == *desk),
        _ => None,
    }
}

/// The panel's title.
#[must_use]
pub fn title(kind: &PanelKind, view: Option<&View>) -> String {
    match kind {
        PanelKind::Console => "Console".into(),
        PanelKind::Seat(_) | PanelKind::Desk(_) => match view.and_then(|v| seat(kind, v)) {
            Some(seat) => format!("{} · {}", seat.seat, seat.route),
            None => match kind {
                PanelKind::Desk(desk) => format!("Desk {}", desk + 1),
                _ => "Seat".into(),
            },
        },
        PanelKind::Decisions => "Decisions".into(),
        PanelKind::Review => "Diff review".into(),
    }
}

/// When the goal of `task` was submitted, for ordering.
fn submitted(view: &View, goal: &str) -> u64 {
    view.goals
        .iter()
        .find(|g| g.goal == goal)
        .map_or(0, |g| g.submitted_at)
}

/// The tasks the merge station can review, newest first: plan tasks that
/// are done.
#[must_use]
pub fn reviewable(view: &View) -> Vec<&Task> {
    let mut tasks: Vec<&Task> = view
        .tasks
        .iter()
        .filter(|t| t.status == TaskStatus::Done && t.entry != "lead")
        .collect();
    tasks.sort_by(|a, b| {
        submitted(view, &b.goal)
            .cmp(&submitted(view, &a.goal))
            .then(b.position.cmp(&a.position))
    });
    tasks
}

fn note(key: &str, text: &str) -> Node<()> {
    message(key, MessageRole::System, text)
}

fn console(view: &View) -> Vec<Node<()>> {
    let mut rows = Vec::new();
    let mut goals: Vec<_> = view.goals.iter().collect();
    goals.sort_by(|a, b| b.submitted_at.cmp(&a.submitted_at));
    for goal in goals {
        let status = match goal.status {
            coder_access::studio::GoalStatus::Planning => "planning",
            coder_access::studio::GoalStatus::Decision => "waiting on a decision",
            coder_access::studio::GoalStatus::Running => "running",
            coder_access::studio::GoalStatus::Done => "done",
        };
        rows.push(message(
            &format!("goal-{}", goal.goal),
            MessageRole::User,
            &format!(
                "**Goal** · {status}\n\n{}\n\n{} of {} tasks over · lead {} · {}",
                goal.text, goal.final_tasks, goal.total_tasks, goal.lead, goal.workspace
            ),
        ));
    }
    if view.goals.is_empty() {
        rows.push(note("no-goals", "No goal yet."));
    }
    let mut seats: Vec<&Seat> = view.seats.iter().collect();
    seats.sort_by_key(|s| indicator(s.activity).rank());
    for seat in seats {
        let state = match indicator(seat.activity) {
            attention::Indicator::Working | attention::Indicator::AwaitingInput => {
                ToolState::Running
            }
            attention::Indicator::Errored => ToolState::Failed,
            _ => ToolState::Done,
        };
        let label = indicator(seat.activity).label().unwrap_or("Idle");
        rows.push(tool(
            &format!("roster-{}", seat.seat),
            &seat.seat,
            &format!("{label} · {} · {}", word(seat.activity), seat.route),
            "",
            state,
        ));
    }
    let open = view.decisions.len();
    if open > 0 {
        rows.push(note(
            "decisions-open",
            &match open {
                1 => "1 decision waits at the podium.".to_owned(),
                n => format!("{n} decisions wait at the podium."),
            },
        ));
    }
    rows.push(note(
        "console-help",
        "Plain text starts a goal; `@seat text` messages a seat; `/answer`, `/diff`, \
         `/status`, `/pause`, `/resume`, `/stop`, and `/repos` mirror the studio's \
         intents. This view observes: sending needs a host connection with the \
         `operate` right.",
    ));
    rows
}

fn seat_rows(kind: &PanelKind, view: &View) -> Vec<Node<()>> {
    let Some(seat) = seat(kind, view) else {
        return vec![note("no-seat", "No seat sits at this desk.")];
    };
    let mut rows = Vec::new();
    let role = match seat.role {
        coder_access::studio::Role::Lead => "lead",
        coder_access::studio::Role::Worker => "worker",
    };
    let task = seat
        .task
        .as_deref()
        .and_then(|id| view.tasks.iter().find(|t| t.task == id));
    let mut body = format!(
        "**{}** · {role} · {}\n\n{}{}",
        seat.seat,
        seat.route,
        word(seat.activity),
        if seat.paused { " · paused" } else { "" }
    );
    if let Some(task) = task {
        body.push_str(&format!("\n\nTask: {}", task.title));
    }
    rows.push(message("seat", MessageRole::Assistant, &body));
    let lines = view
        .logs
        .iter()
        .find(|log| log.seat == seat.seat)
        .map_or(&[][..], |log| log.lines.as_slice());
    let working = indicator(seat.activity) == attention::Indicator::Working;
    for (i, line) in lines.iter().enumerate() {
        let state = if matches!(line.activity, Activity::Failed | Activity::Blocked) {
            ToolState::Failed
        } else if working && i + 1 == lines.len() {
            ToolState::Running
        } else {
            ToolState::Done
        };
        rows.push(tool(
            &format!("log-{i}"),
            word(line.activity),
            &line.text,
            "",
            state,
        ));
    }
    if lines.is_empty() {
        rows.push(note("no-log", "Nothing in this seat's log yet."));
    }
    rows
}

fn decision_rows(view: &View) -> Vec<Node<()>> {
    let position = |task: Option<&str>| {
        task.and_then(|id| view.tasks.iter().find(|t| t.task == id))
            .map_or(0, |t| t.position)
    };
    let mut decisions: Vec<_> = view.decisions.iter().collect();
    decisions.sort_by(|a, b| {
        submitted(view, &a.goal)
            .cmp(&submitted(view, &b.goal))
            .then(position(a.task.as_deref()).cmp(&position(b.task.as_deref())))
    });
    let mut rows = Vec::new();
    for (i, open) in decisions.into_iter().enumerate() {
        let who = open.seat.as_deref().unwrap_or("The studio");
        let (flow, heading) = match open.kind {
            DecisionKind::Approval => (
                decision::Flow::approval(&open.text),
                format!("**{who}** asks to go ahead"),
            ),
            DecisionKind::Question => (
                decision::Flow::question(&open.text),
                format!("**{who}** asks"),
            ),
            DecisionKind::InvalidPlan => (
                decision::Flow::question(&open.text),
                "**The plan failed validation**: answer with a corrected plan".to_owned(),
            ),
            DecisionKind::NoPlan => (
                decision::Flow::question(&open.text),
                "**The lead finished without a plan**: answer with a plan".to_owned(),
            ),
            DecisionKind::LeadFailed => (
                decision::Flow::question(&open.text),
                "**The lead failed**: answer with a plan, or retry the lead".to_owned(),
            ),
            DecisionKind::DependencyFailed => (
                decision::Flow::question(&open.text),
                "**A dependency did not finish**: retry or cancel the task".to_owned(),
            ),
        };
        let pages = flow.pages();
        let mut body = heading;
        for (n, page) in pages.iter().enumerate() {
            if pages.len() > 1 {
                body.push_str(&format!("\n\n*{} of {}*", n + 1, pages.len()));
            }
            if !page.prompt.is_empty() {
                body.push_str("\n\n");
                body.push_str(&page.prompt);
            }
            for (k, option) in page.options.iter().enumerate() {
                body.push_str(&format!("\n{}. {option}", k + 1));
            }
        }
        rows.push(message(
            &format!("decision-{i}-{}", open.decision),
            MessageRole::Assistant,
            &body,
        ));
    }
    if rows.is_empty() {
        rows.push(note("no-decisions", "No decision waits on you."));
    }
    rows
}

fn review_rows(view: &View, review: Option<&TaskReview>) -> Vec<Node<()>> {
    let Some(review) = review else {
        return vec![note(
            "no-review",
            "No review is open. A done task's review loads here when its seat finishes.",
        )];
    };
    let title = view
        .tasks
        .iter()
        .find(|t| t.task == review.task)
        .map_or(review.task.as_str(), |t| t.title.as_str());
    let mut body = format!(
        "**What changed** · {title}\n\n{} {} · +{} −{}",
        review.files_total,
        if review.files_total == 1 {
            "file"
        } else {
            "files"
        },
        review.added,
        review.removed
    );
    for file in &review.files {
        body.push_str(&format!(
            "\n- `{}` +{} −{}",
            file.path,
            file.added.unwrap_or(0),
            file.removed.unwrap_or(0)
        ));
    }
    vec![
        message("review", MessageRole::Assistant, &body),
        note(
            "review-help",
            "**Merge**, **Request changes**, and **Reject** need a host connection with \
             the `review` right.",
        ),
    ]
}

/// The rows a panel of `kind` shows for `view`, and for the review panel,
/// `review`.
#[must_use]
pub fn rows(kind: &PanelKind, view: Option<&View>, review: Option<&TaskReview>) -> Vec<Node<()>> {
    let Some(view) = view else {
        return vec![note(
            "not-loaded",
            "The studio has not loaded. It loads while you are in Everglade with a studio \
             source.",
        )];
    };
    match kind {
        PanelKind::Console => console(view),
        PanelKind::Seat(_) | PanelKind::Desk(_) => seat_rows(kind, view),
        PanelKind::Decisions => decision_rows(view),
        PanelKind::Review => review_rows(view, review),
    }
}

/// Shows `kind` for `view` in `panel`: its title, rows, and for the review,
/// the diff in the changes tab.
pub fn fill(panel: &mut Panel, kind: &PanelKind, view: Option<&View>, review: Option<&TaskReview>) {
    panel.set_title(&title(kind, view));
    panel.set_rows(rows(kind, view, review));
    if *kind == PanelKind::Review {
        let diff = review.map_or("", |r| r.diff.as_str());
        if panel.diff_source() != Some(diff) {
            panel.set_diff(diff);
        }
    }
}

/// A new panel showing `kind`; the review opens on its changes tab.
#[must_use]
pub fn open(kind: &PanelKind, view: Option<&View>, review: Option<&TaskReview>) -> Panel {
    let mut panel = Panel::new(&title(kind, view));
    fill(&mut panel, kind, view, review);
    if *kind == PanelKind::Review && review.is_some() {
        panel.apply(Intent::Show(Tab::Changes));
    }
    panel
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zones::everglade::studio::Attention;

    #[test]
    fn the_roster_and_the_lamps_rank_seats_alike() {
        let all = [
            Activity::Idle,
            Activity::Reading,
            Activity::Editing,
            Activity::Running,
            Activity::Testing,
            Activity::Judging,
            Activity::Thinking,
            Activity::Waiting,
            Activity::Blocked,
            Activity::Paused,
            Activity::Done,
            Activity::Failed,
        ];
        let lamp_rank = |a: Attention| match a {
            Attention::AwaitingInput => 0,
            Attention::Errored => 1,
            Attention::Working => 2,
            Attention::Completed => 4,
            Attention::Idle => 5,
        };
        for activity in all {
            assert_eq!(
                indicator(activity).rank(),
                lamp_rank(Attention::of(activity)),
                "{activity:?}"
            );
        }
    }

    #[test]
    fn an_unloaded_studio_says_so_in_every_panel() {
        for kind in [
            PanelKind::Console,
            PanelKind::Desk(0),
            PanelKind::Decisions,
            PanelKind::Review,
        ] {
            let rows = rows(&kind, None, None);
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].key, "not-loaded");
        }
    }
}
