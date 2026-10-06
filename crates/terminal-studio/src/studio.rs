//! Studio projections shared by the native window and the world mount.
use coder_access::{
    Right,
    studio::Snapshot,
    studio_intents::{self, Console},
};
use terminal_core::studio::{Prepared, View};

pub fn receipt(bytes: &[u8], operation: &str) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|_| "Studio outcome unknown; no automatic replay.".to_owned())?;
    if let Some(error) = value["error"].as_str() {
        let code = value["code"].as_str().unwrap_or("refused");
        return Err(if matches!(code, "unavailable" | "transport") {
            format!("Studio outcome unknown ({code}): {error}; no automatic replay.")
        } else {
            format!("{code}: {error}")
        });
    }
    if value["operation"].as_str() != Some(operation) {
        return Err("Studio receipt names another operation; no automatic replay.".into());
    }
    let reference = value["reference"]
        .as_str()
        .filter(|r| !r.is_empty() && r.len() <= 128)
        .ok_or("Studio receipt has no bounded reference; no automatic replay.")?;
    Ok(format!("Host receipt: {operation} {reference}"))
}

/// Maps only the existing studio steering operations to their existing
/// CLI adapters. Each adapter keeps its request identity on a lost reply.
pub fn arguments(operation: coder_access::Operation) -> Result<Vec<String>, String> {
    use coder_access::Operation;
    let mut args = vec!["--json".into(), "studio".into()];
    let words = match operation {
        Operation::SubmitGoal {
            text,
            workspace,
            lead: None,
        } => {
            args.extend(["--workspace".into(), workspace]);
            vec!["goal".into(), "submit".into(), text]
        }
        Operation::MessageSeat { seat, text } => vec![
            "message".into(),
            seat.unwrap_or_else(|| "everyone".into()),
            text,
        ],
        Operation::PauseSeat { seat } => vec!["seat".into(), "pause".into(), seat],
        Operation::ResumeSeat { seat } => vec!["seat".into(), "resume".into(), seat],
        Operation::StopSeat { seat } => vec!["seat".into(), "stop".into(), seat],
        Operation::RetryTask { task } => vec!["task".into(), "retry".into(), task],
        Operation::PrioritizeTask { task } => vec!["task".into(), "prioritize".into(), task],
        Operation::CancelStudioTask { task } => vec!["task".into(), "cancel".into(), task],
        Operation::ReassignTask { task, seat } => {
            vec!["task".into(), "reassign".into(), task, seat]
        }
        _ => return Err("This is not a studio steering operation.".into()),
    };
    args.push("--".into());
    args.extend(words);
    Ok(args)
}

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
        stream: snapshot.stream.clone(),
        sequence: snapshot.sequence,
        rows,
        operate: rights.contains(&Right::Operate),
        local_runs: Vec::new(),
        workspaces: view
            .repositories
            .iter()
            .map(|repo| repo.workspace.clone())
            .collect(),
    })
}

pub fn prepare(
    snapshot: &Snapshot,
    rights: &[Right],
    line: &str,
    workspace: Option<&str>,
) -> Result<Prepared, String> {
    project(snapshot, rights)?;
    if line.len() > 4096 {
        return Err("Studio command exceeds its bound.".into());
    }
    let action = match studio_intents::console(line, &snapshot.view, workspace) {
        Console::Act(action) => action,
        Console::Refused(error) => return Err(error),
        _ => return Err("Use a goal, seat message, or task/seat command on this page.".into()),
    };
    if !rights.contains(&action.right()) {
        return Err("Studio operation is not admitted.".into());
    }
    if matches!(
        action,
        studio_intents::Action::Answer { .. }
            | studio_intents::Action::AllowAlways { .. }
            | studio_intents::Action::Decide { .. }
    ) {
        return Err("Open the approvals or exact-revision review adapter for this action.".into());
    }
    let operation = action.operation(studio_intents::now());
    operation.validate().map_err(|e| e.to_string())?;
    Ok(Prepared {
        stream: snapshot.stream.clone(),
        description: format!("{}: {}", action.describe(), line),
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
            &snapshot,
            &[Right::Observe, Right::Operate],
            "/pause @ada",
            None,
        )
        .unwrap();
        let operation: coder_access::Operation = serde_json::from_slice(&prepared.bytes).unwrap();
        assert!(matches!(operation, coder_access::Operation::PauseSeat { seat } if seat == "ada"));
        assert!(prepare(&snapshot, &[Right::Observe], "/pause @ada", None).is_err());
        assert!(
            prepare(
                &snapshot,
                &[Right::World, Right::Operate],
                "/pause @ada",
                None
            )
            .is_err()
        );
    }

    #[test]
    fn helper_arguments_keep_option_like_text_literal_and_reject_other_operations() {
        let args = arguments(coder_access::Operation::MessageSeat {
            seat: Some("ada".into()),
            text: "--control-socket".into(),
        })
        .unwrap();
        assert_eq!(
            args,
            [
                "--json",
                "studio",
                "--",
                "message",
                "ada",
                "--control-socket"
            ]
        );
        let args = arguments(coder_access::Operation::SubmitGoal {
            text: "--workspace".into(),
            workspace: "scratch".into(),
            lead: None,
        })
        .unwrap();
        assert_eq!(
            args,
            [
                "--json",
                "studio",
                "--workspace",
                "scratch",
                "--",
                "goal",
                "submit",
                "--workspace"
            ]
        );
        assert!(arguments(coder_access::Operation::StudioSnapshot {}).is_err());
    }

    #[test]
    fn host_receipts_and_refusals_remain_distinct_from_unknown_outcomes() {
        assert!(
            receipt(
                br#"{"operation":"studio.seat.pause","reference":"ada"}"#,
                "studio.seat.pause"
            )
            .unwrap()
            .contains("ada")
        );
        assert!(
            receipt(
                br#"{"code":"revoked","error":"grant revoked"}"#,
                "studio.seat.pause"
            )
            .unwrap_err()
            .contains("revoked")
        );
        assert!(
            receipt(
                br#"{"operation":"studio.seat.stop","reference":"ada"}"#,
                "studio.seat.pause"
            )
            .is_err()
        );
        assert!(receipt(b"{}", "studio.seat.pause").is_err());
    }
}
