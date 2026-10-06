//! Everglade navigation into the shared terminal; no studio intent is sent.
use crate::terminal::opening::{Host, Kind, Opening, ResourceRef, Revision, StudioPart};
use crate::zones::everglade::studio::PanelKind;
use coder_access::{Right, review::TaskReview, studio::Snapshot};
use sha2::{Digest, Sha256};

/// References from an admitted snapshot. The local host process is named
/// by its source-issued stream; a restart therefore cannot reuse context.
/// World presence never supplies observation admission.
pub fn context(
    snapshot: &Snapshot,
    rights: &[Right],
    target: &PanelKind,
    review: Option<&TaskReview>,
) -> Result<Opening, String> {
    if !rights.contains(&Right::Observe) {
        return Err("studio observation is not admitted".into());
    }
    snapshot.validate().map_err(|e| e.to_string())?;
    if let Some(review) = review {
        review.validate().map_err(|e| e.to_string())?;
    }
    let host = Host::Local {
        instance: format!("{:x}", Sha256::digest(snapshot.stream.as_bytes())),
    };
    let view = &snapshot.view;
    let selected_seat = match target {
        PanelKind::Seat(name) => view.seats.iter().find(|s| &s.seat == name),
        PanelKind::Desk(desk) => view.seats.iter().find(|s| &s.desk == desk),
        _ => None,
    };
    let selected_task = match target {
        PanelKind::Task(id) => view.tasks.iter().find(|t| &t.task == id),
        PanelKind::Review => review.and_then(|r| view.tasks.iter().find(|t| t.task == r.task)),
        _ => selected_seat
            .and_then(|s| s.task.as_ref())
            .and_then(|id| view.tasks.iter().find(|t| &t.task == id)),
    };
    if matches!(target, PanelKind::Seat(_) | PanelKind::Desk(_)) && selected_seat.is_none()
        || matches!(target, PanelKind::Task(_)) && selected_task.is_none()
    {
        return Err("the selected studio record is unavailable".into());
    }
    if matches!(target, PanelKind::Review) && review.is_some() && selected_task.is_none() {
        return Err("the reviewed task is no longer in this studio".into());
    }
    if selected_seat.is_some_and(|seat| seat.task.is_some()) && selected_task.is_none() {
        return Err("the seat's task is unavailable in this snapshot".into());
    }
    let goal = selected_task
        .and_then(|t| view.goals.iter().find(|g| g.goal == t.goal))
        .or_else(|| {
            if selected_seat.is_none() && selected_task.is_none() {
                view.goals.first()
            } else {
                None
            }
        });
    let seat = selected_seat
        .or_else(|| selected_task.and_then(|t| view.seats.iter().find(|s| s.seat == t.seat)));
    let reference = |part, id: &str| {
        ResourceRef::new(Kind::Studio, host.clone(), id)
            .studio(part)
            .with_revision(Revision::Counter(snapshot.sequence))
    };
    let exact_review = review
        .filter(|r| selected_task.is_some_and(|t| t.task == r.task))
        .map(|r| {
            let identities = serde_json::to_vec(&(&r.task, &r.base, &r.head_commit, &r.head))
                .unwrap_or_default();
            ResourceRef::new(Kind::Studio, host.clone(), &r.task)
                .studio(StudioPart::Review)
                .with_revision(Revision::Sha256(format!(
                    "{:x}",
                    Sha256::digest(identities)
                )))
        });
    let opening = Opening {
        v: crate::terminal::opening::OPENING.into(),
        host: host.clone(),
        stream: snapshot.stream.clone(),
        workspace: goal.map(|g| g.workspace.clone()),
        goal: goal.map(|g| reference(StudioPart::Goal, &g.goal)),
        seat: seat.map(|s| reference(StudioPart::Seat, &s.seat)),
        task: selected_task.map(|t| reference(StudioPart::Task, &t.task)),
        // NIP-HOST studio snapshots do not advertise a chat thread identity.
        thread: None,
        review: exact_review,
    };
    opening.check()?;
    Ok(opening)
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_access::studio::*;

    fn snapshot() -> Snapshot {
        let mut view = View {
            goals: vec![Goal {
                goal: "g1".into(),
                text: "check a patch".into(),
                workspace: "app".into(),
                lead: "lead".into(),
                status: GoalStatus::Running,
                final_tasks: 0,
                total_tasks: 1,
                submitted_at: 1,
                spend: Default::default(),
            }],
            seats: vec![Seat {
                seat: "lead".into(),
                role: Role::Lead,
                route: "codex:sim".into(),
                look: "default".into(),
                desk: 0,
                activity: Activity::Running,
                station: Station::Desk,
                task: Some("t1".into()),
                paused: false,
                spend: Default::default(),
            }],
            tasks: vec![Task {
                task: "t1".into(),
                goal: "g1".into(),
                entry: "lead".into(),
                position: 0,
                title: "check".into(),
                seat: "lead".into(),
                depends_on: Vec::new(),
                status: TaskStatus::Running,
                spend: Default::default(),
            }],
            ..View::default()
        };
        view.canonicalize();
        Snapshot {
            stream: "ab".into(),
            sequence: 3,
            view,
        }
    }

    #[test]
    fn station_and_keyboard_selection_keep_the_same_existing_records() {
        let snapshot = snapshot();
        let before = snapshot.clone();
        let desk = context(&snapshot, &[Right::Observe], &PanelKind::Desk(0), None).unwrap();
        let seat = context(
            &snapshot,
            &[Right::Observe],
            &PanelKind::Seat("lead".into()),
            None,
        )
        .unwrap();
        assert_eq!(desk, seat);
        assert_eq!(desk.goal.as_ref().unwrap().id, "g1");
        assert_eq!(desk.task.as_ref().unwrap().id, "t1");
        assert_eq!(desk.workspace.as_deref(), Some("app"));
        assert_eq!(snapshot, before);
        assert!(desk.thread.is_none());
        let encoded = serde_json::to_vec(&desk).unwrap();
        assert_eq!(serde_json::from_slice::<Opening>(&encoded).unwrap(), desk);
    }

    #[test]
    fn world_only_and_stale_selections_do_not_disclose_context() {
        let snapshot = snapshot();
        assert!(context(&snapshot, &[Right::World], &PanelKind::Desk(0), None).is_err());
        assert!(
            context(
                &snapshot,
                &[Right::Observe],
                &PanelKind::Seat("missing".into()),
                None
            )
            .is_err()
        );
        let before = context(&snapshot, &[Right::Observe], &PanelKind::Desk(0), None).unwrap();
        let mut restarted = snapshot;
        restarted.stream = "cd".into();
        let after = context(&restarted, &[Right::Observe], &PanelKind::Desk(0), None).unwrap();
        assert_ne!(before.host, after.host);
    }
    #[test]
    fn review_openings_bind_the_exact_base_commit_and_tree() {
        use coder_access::review::Completeness;
        let snapshot = snapshot();
        let review = TaskReview {
            task: "t1".into(),
            base: "a".repeat(40),
            head_commit: "b".repeat(40),
            head: "c".repeat(40),
            files: Vec::new(),
            files_total: 0,
            added: 0,
            removed: 0,
            uncounted: 0,
            diff: String::new(),
            completeness: Completeness::Complete,
            publication: None,
        };
        let first = context(
            &snapshot,
            &[Right::Observe],
            &PanelKind::Review,
            Some(&review),
        )
        .unwrap();
        let mut changed = review;
        changed.head = "d".repeat(40);
        let second = context(
            &snapshot,
            &[Right::Observe],
            &PanelKind::Review,
            Some(&changed),
        )
        .unwrap();
        assert_ne!(first.review, second.review);
        assert_eq!(first.task, second.task);
    }
}
