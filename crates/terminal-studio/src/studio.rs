//! Studio projections shared by the native window and the world mount.
use coder_access::{
    Right,
    studio::Snapshot,
    studio_intents::{self, Console},
};
use terminal_core::studio::{Prepared, View};

pub fn project(snapshot: &Snapshot, rights: &[Right]) -> Result<View, String> {
    if !rights.contains(&Right::Observe) {
        return Err("Studio observation is not admitted.".into());
    }
    snapshot.validate().map_err(|e| e.to_string())?;
    let view = &snapshot.view;
    let mut rows = vec!["WORKSPACES (/repo LABEL selects without submitting work)".into()];
    rows.extend(view.repositories.iter().map(|repo| repo.workspace.clone()));
    rows.push("GOALS".into());
    for goal in &view.goals {
        rows.push(format!(
            "{} {:?} {}/{} tasks {} {}",
            goal.goal,
            goal.status,
            goal.final_tasks,
            goal.total_tasks,
            goal.spend.label(),
            goal.text
        ));
    }
    rows.push("TASK WALL (linked run IDs; logs are not a terminal)".into());
    for task in &view.tasks {
        rows.push(format!(
            "{} {:?} @{} {} {}",
            task.task,
            task.status,
            task.seat,
            task.spend.label(),
            task.title
        ));
        rows.push(format!(
            "  goal {} entry {} waits on {}",
            task.goal,
            task.entry,
            if task.depends_on.is_empty() {
                "none".into()
            } else {
                task.depends_on.join(", ")
            }
        ));
        rows.push(format!(
            "  managed run {}; checks and artifacts require its run adapter",
            task.task
        ));
    }
    let decision_start = rows.len();
    rows.push("STUDIO QUESTIONS AND TOOL APPROVALS (separate from shell proposals)".into());
    for decision in &view.decisions {
        rows.push(format!(
            "{} {:?} seat {} task {} revision {}: {}",
            decision.decision,
            decision.kind,
            decision.seat.as_deref().unwrap_or("none"),
            decision.task.as_deref().unwrap_or("none"),
            decision.based_on,
            decision.text
        ));
        if let Some(approval) = &decision.approval {
            rows.push(format!(
                "  tool {} cwd {} {}",
                approval.tool,
                approval.cwd,
                approval.risk.label()
            ));
            rows.push(format!("  exact command: {}", approval.command));
            rows.push(format!("  reason: {}", approval.reason));
            rows.push(format!(
                "  standing rule: {}",
                approval.always.as_deref().unwrap_or("none")
            ));
        }
    }
    rows.push(
        "/answer DECISION TEXT answers the displayed revision; ENTER prepares, ENTER confirms."
            .into(),
    );
    let decisions = bounded_rows(rows[decision_start..].to_vec());
    rows.push("SEATS".into());
    for seat in &view.seats {
        rows.push(format!(
            "@{} {:?} route {} (served engine unknown) paused {} spend {} task {}",
            seat.seat,
            seat.activity,
            seat.route,
            seat.paused,
            seat.spend.label(),
            seat.task.as_deref().unwrap_or("none")
        ));
        for log in view.logs.iter().filter(|log| log.seat == seat.seat) {
            for line in &log.lines {
                rows.push(format!("  {} {:?} {}", line.at, line.activity, line.text));
            }
        }
    }
    rows.push("MEMORY (host entries; pinned plans first)".into());
    for memory in view
        .memory
        .iter()
        .filter(|m| m.pinned)
        .chain(view.memory.iter().filter(|m| !m.pinned))
    {
        rows.push(format!(
            "{} {:?} {} {}",
            memory.entry, memory.kind, memory.author, memory.text
        ));
    }
    rows.push(
        "Plain text submits a goal; @seat text messages; /pause, /resume, /stop @seat.".into(),
    );
    rows.push(
        "/task TASK cancel|retry|prioritize|reassign @seat. ENTER prepares; ENTER confirms.".into(),
    );
    Ok(View {
        source: serde_json::to_vec(snapshot).map_err(|e| e.to_string())?,
        stream: snapshot.stream.clone(),
        sequence: snapshot.sequence,
        rows: bounded_rows(rows),
        decisions,
        operate: rights.contains(&Right::Operate),
        review: rights.contains(&Right::Review),
        tasks: view.tasks.iter().map(|task| task.task.clone()).collect(),
        local_runs: Vec::new(),
        workspaces: view
            .repositories
            .iter()
            .map(|repo| repo.workspace.clone())
            .collect(),
    })
}

pub fn prepare(
    review: Option<&terminal_core::studio::Review>,
    snapshot: &Snapshot,
    rights: &[Right],
    line: &str,
    workspace: Option<&str>,
) -> Result<Prepared, String> {
    project(snapshot, rights)?;
    if line.len() > 4096 {
        return Err("Studio command exceeds its bound.".into());
    }
    let action =
        if line == "/merge" || line.starts_with("/changes ") || line.starts_with("/reject ") {
            let displayed = review.ok_or("Open /review TASK before deciding.")?;
            if displayed.stream != snapshot.stream {
                return Err("Review belongs to another host stream.".into());
            }
            let review: coder_access::review::TaskReview =
                serde_json::from_slice(&displayed.source).map_err(|e| e.to_string())?;
            review.validate().map_err(|e| e.to_string())?;
            if review.task != displayed.task
                || !snapshot
                    .view
                    .tasks
                    .iter()
                    .any(|task| task.task == review.task)
            {
                return Err("Review task is not admitted.".into());
            }
            let (verdict, text) = if line == "/merge" {
                (coder_access::studio::Verdict::Merge, String::new())
            } else if let Some(text) = line.strip_prefix("/changes ") {
                (
                    coder_access::studio::Verdict::RequestChanges,
                    text.to_owned(),
                )
            } else {
                (
                    coder_access::studio::Verdict::Reject,
                    line.strip_prefix("/reject ").unwrap().to_owned(),
                )
            };
            studio_intents::Action::Decide {
                review: Box::new(review),
                verdict,
                text,
            }
        } else if let Some(id) = line.strip_prefix("/always ") {
            let decision = snapshot
                .view
                .decisions
                .iter()
                .find(|decision| decision.decision == id)
                .ok_or("That decision is not in the displayed snapshot.")?;
            let rule = decision
                .approval
                .as_ref()
                .and_then(|approval| approval.always.clone())
                .ok_or("The host offers no standing rule for that approval.")?;
            studio_intents::Action::AllowAlways {
                decision: id.to_owned(),
                based_on: decision.based_on,
                rule,
            }
        } else {
            match studio_intents::console(line, &snapshot.view, workspace) {
                Console::Act(action) => action,
                Console::Refused(error) => return Err(error),
                _ => {
                    return Err(
                        "Use a goal, seat message, answer, or studio command on this page.".into(),
                    );
                }
            }
        };
    if !rights.contains(&action.right()) {
        return Err("Studio operation is not admitted.".into());
    }
    let operation = action.operation(studio_intents::now());
    operation.validate().map_err(|e| e.to_string())?;
    Ok(Prepared {
        request: studio_intents::mint(),
        review: action.right() == Right::Review,
        stream: snapshot.stream.clone(),
        description: match &action {
            studio_intents::Action::Answer {
                decision, based_on, ..
            }
            | studio_intents::Action::AllowAlways {
                decision, based_on, ..
            } => {
                let shown = snapshot
                    .view
                    .decisions
                    .iter()
                    .find(|shown| shown.decision == *decision)
                    .ok_or("Decision is not in the displayed snapshot.")?;
                format!(
                    "STUDIO {:?} {} revision {}: {} approval {:?}; answer {}",
                    shown.kind, decision, based_on, shown.text, shown.approval, line
                )
            }
            studio_intents::Action::Decide {
                review, verdict, ..
            } => format!(
                "STUDIO REVIEW {:?} {} base {} HEAD {} tree {}; {}",
                verdict, review.task, review.base, review.head_commit, review.head, line
            ),
            _ => format!("{}: {}", action.describe(), line),
        },
        bytes: serde_json::to_vec(&operation).map_err(|e| e.to_string())?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_access::studio::{Activity, Log, LogLine, Role, Seat, Station, View as StudioView};
    fn snapshot() -> Snapshot {
        Snapshot {
            stream: "ab".into(),
            sequence: 1,
            view: StudioView {
                seats: vec![Seat {
                    seat: "ada".into(),
                    role: Role::Worker,
                    route: "codex:test".into(),
                    look: "default".into(),
                    desk: 0,
                    activity: Activity::Testing,
                    station: Station::ProvingGround,
                    task: None,
                    paused: false,
                    spend: Default::default(),
                }],
                logs: vec![Log {
                    seat: "ada".into(),
                    task: None,
                    lines: vec![LogLine {
                        at: 42,
                        activity: Activity::Testing,
                        text: "cargo test".into(),
                    }],
                }],
                ..StudioView::default()
            },
        }
    }
    #[test]
    fn projection_preserves_source_facts_and_never_claims_log_input() {
        let snapshot = snapshot();
        let before = snapshot.clone();
        let display = project(&snapshot, &[Right::Observe]).unwrap();
        assert_eq!(snapshot, before);
        assert!(!display.operate);
        let text = display.rows.join("\n");
        assert!(
            text.contains("codex:test")
                && text.contains("cargo test")
                && text.contains("logs are not a terminal")
        );
        assert!(project(&snapshot, &[Right::World]).is_err());
    }
    #[test]
    fn shared_parser_prepares_existing_operation_and_denied_never_prepares() {
        let snapshot = snapshot();
        let prepared = prepare(
            None,
            &snapshot,
            &[Right::Observe, Right::Operate],
            "/pause @ada",
            None,
        )
        .unwrap();
        let operation: coder_access::Operation = serde_json::from_slice(&prepared.bytes).unwrap();
        assert!(matches!(operation, coder_access::Operation::PauseSeat { seat } if seat == "ada"));
        assert!(prepare(None, &snapshot, &[Right::Observe], "/pause @ada", None).is_err());
        assert!(
            prepare(
                None,
                &snapshot,
                &[Right::World, Right::Operate],
                "/pause @ada",
                None
            )
            .is_err()
        );
    }
    #[test]
    fn approval_keeps_displayed_revision_and_exact_standing_rule() {
        use coder_access::studio::{Approval, Decision, DecisionKind, Risk};
        let mut shown = snapshot();
        shown.view.decisions.push(Decision {
            decision: "studio-g-a".into(),
            goal: "g".into(),
            task: Some("studio-g-a".into()),
            seat: Some("ada".into()),
            kind: DecisionKind::Approval,
            text: "Run this?".into(),
            based_on: 7,
            approval: Some(Approval {
                tool: "shell".into(),
                command: "cargo test".into(),
                cwd: "/scratch".into(),
                reason: "Check it".into(),
                risk: Risk::Low,
                always: Some("cargo test in /scratch".into()),
            }),
        });
        let rights = [Right::Observe, Right::Operate];
        let displayed = project(&shown, &rights).unwrap();
        let frozen: Snapshot = serde_json::from_slice(&displayed.source).unwrap();
        shown.view.decisions[0].based_on = 8;
        let prepared = prepare(None, &frozen, &rights, "/answer studio-g-a yes", None).unwrap();
        let operation: coder_access::Operation = serde_json::from_slice(&prepared.bytes).unwrap();
        assert!(matches!(
            operation,
            coder_access::Operation::AnswerDecision { based_on: 7, .. }
        ));
        assert!(
            prepared.description.contains("cargo test")
                && prepared.description.contains("/scratch")
        );
        let standing = prepare(None, &frozen, &rights, "/always studio-g-a", None).unwrap();
        let operation: coder_access::Operation = serde_json::from_slice(&standing.bytes).unwrap();
        assert!(
            matches!(operation, coder_access::Operation::AllowAlways { based_on: 7, rule, .. }
            if rule == "cargo test in /scratch")
        );
        assert!(
            prepare(
                None,
                &frozen,
                &[Right::Observe, Right::Review],
                "/answer studio-g-a yes",
                None
            )
            .is_err()
        );
    }

    #[test]
    fn review_decisions_keep_all_three_revisions_and_require_review() {
        use coder_access::review::{Completeness, TaskReview};
        use coder_access::studio::{Task, TaskStatus};
        let mut snapshot = snapshot();
        snapshot.view.tasks.push(Task {
            task: "studio-g-a".into(),
            goal: "g".into(),
            entry: "a".into(),
            position: 1,
            title: "Change".into(),
            seat: "ada".into(),
            depends_on: Vec::new(),
            status: TaskStatus::Done,
            spend: Default::default(),
        });
        let review = TaskReview {
            task: "studio-g-a".into(),
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
        let shown = project_review(&snapshot.stream, &review).unwrap();
        for line in ["/merge", "/changes add a test", "/reject wrong change"] {
            let prepared = prepare(
                Some(&shown),
                &snapshot,
                &[Right::Observe, Right::Review],
                line,
                None,
            )
            .unwrap();
            assert!(prepared.review);
            let operation: coder_access::Operation =
                serde_json::from_slice(&prepared.bytes).unwrap();
            let coder_access::Operation::DecideMerge { decision } = operation else {
                panic!()
            };
            assert_eq!(
                (decision.base, decision.head_commit, decision.head),
                (
                    review.base.clone(),
                    review.head_commit.clone(),
                    review.head.clone()
                )
            );
            assert!(
                prepare(
                    Some(&shown),
                    &snapshot,
                    &[Right::Observe, Right::Operate],
                    line,
                    None
                )
                .is_err()
            );
        }
        let mut other = shown;
        other.stream = "cd".into();
        assert!(
            prepare(
                Some(&other),
                &snapshot,
                &[Right::Observe, Right::Review],
                "/merge",
                None
            )
            .is_err()
        );
    }
}

/// Display an exact host review without interpreting the diff as Markdown.
pub fn project_review(
    stream: &str,
    review: &coder_access::review::TaskReview,
) -> Result<terminal_core::studio::Review, String> {
    review.validate().map_err(|e| e.to_string())?;
    let mut rows = vec![
        format!("EXACT-REVISION REVIEW {}", review.task),
        format!("base {}", review.base),
        format!("HEAD {}", review.head_commit),
        format!("tree {}", review.head),
        format!(
            "diff {:?}; {} files +{} -{} uncounted {}",
            review.completeness, review.files_total, review.added, review.removed, review.uncounted
        ),
        "Independent checks and lead evidence: unknown in this review; inspect the managed run."
            .into(),
        "A studio merge is local and pushes nothing. Host admission decides each verdict.".into(),
    ];
    for file in &review.files {
        rows.push(format!(
            "{:?} {} +{:?} -{:?}",
            file.status, file.path, file.added, file.removed
        ));
    }
    for line in review.diff.lines() {
        // Split long lines for the bounded sheet while preserving the original
        // reviewed bytes in the opaque source used for the decision.
        let mut row = String::new();
        for character in line.chars() {
            if row.len() + character.len_utf8() > 4096 {
                rows.push(std::mem::take(&mut row));
            }
            row.push(character);
        }
        rows.push(row);
    }
    Ok(terminal_core::studio::Review {
        stream: stream.to_owned(),
        task: review.task.clone(),
        rows,
        source: serde_json::to_vec(review).map_err(|e| e.to_string())?,
    })
}

fn bounded_rows(rows: Vec<String>) -> Vec<String> {
    let mut display = Vec::new();
    for line in rows {
        let mut row = String::new();
        for character in line.chars() {
            if row.len() + character.len_utf8() > 4096 {
                display.push(std::mem::take(&mut row));
            }
            row.push(character);
        }
        display.push(row);
    }
    display
}
