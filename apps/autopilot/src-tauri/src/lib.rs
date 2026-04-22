pub mod control;
mod pylon;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct AutopilotStatus {
    product: &'static str,
    shell: &'static str,
    rust_authority: &'static str,
    runtime_lane: &'static str,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkbenchWorkspace {
    id: String,
    name: String,
    path: String,
    branch: String,
    trust: &'static str,
    policy: &'static str,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkbenchSession {
    id: &'static str,
    title: &'static str,
    goal: &'static str,
    state: &'static str,
    permission_mode: &'static str,
    resume_state: &'static str,
    engine: &'static str,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkbenchTimelineEvent {
    id: &'static str,
    time: &'static str,
    state: &'static str,
    label: &'static str,
    detail: &'static str,
    owner: &'static str,
    evidence: &'static str,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkbenchApproval {
    id: &'static str,
    state: &'static str,
    risk: &'static str,
    request: &'static str,
    policy: &'static str,
    paths: Vec<&'static str>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkbenchDiff {
    id: &'static str,
    state: &'static str,
    file: &'static str,
    summary: &'static str,
    additions: u32,
    deletions: u32,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkbenchVerification {
    id: &'static str,
    state: &'static str,
    command: &'static str,
    elapsed_ms: u32,
    detail: &'static str,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkbenchEvidence {
    id: &'static str,
    kind: &'static str,
    state: &'static str,
    location: &'static str,
    owner: &'static str,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkbenchScorecard {
    first_tool_event_seconds: u32,
    verified_diff_minutes: u32,
    recovery_state: &'static str,
    human_interventions: u32,
    sats_earned_today: u32,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkbenchSnapshot {
    product: &'static str,
    visible_surface: &'static str,
    generated_at_unix_ms: u64,
    workspace: WorkbenchWorkspace,
    session: WorkbenchSession,
    timeline: Vec<WorkbenchTimelineEvent>,
    approvals: Vec<WorkbenchApproval>,
    diffs: Vec<WorkbenchDiff>,
    verification: Vec<WorkbenchVerification>,
    evidence: Vec<WorkbenchEvidence>,
    scorecard: WorkbenchScorecard,
}

#[tauri::command]
fn autopilot_status() -> AutopilotStatus {
    AutopilotStatus {
        product: "Autopilot",
        shell: "Tauri",
        rust_authority: "online",
        runtime_lane: "prototype",
    }
}

#[tauri::command]
fn autopilot_workbench_snapshot() -> WorkbenchSnapshot {
    let workspace_path = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let workspace_name = workspace_path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("workspace")
        .to_string();
    let workspace_branch =
        current_git_branch(workspace_path.as_path()).unwrap_or_else(|| "unknown".to_string());

    WorkbenchSnapshot {
        product: "Autopilot",
        visible_surface: "workbench",
        generated_at_unix_ms: unix_time_ms(),
        workspace: WorkbenchWorkspace {
            id: format!("workspace.local.{workspace_name}"),
            name: workspace_name,
            path: workspace_path.display().to_string(),
            branch: workspace_branch,
            trust: "local user workspace",
            policy: "approval required for writes and process control",
        },
        session: WorkbenchSession {
            id: "session.clean-room.autopilot.001",
            title: "Clean-room Autopilot workbench rebuild",
            goal: "Repo -> session -> tools -> approvals -> diff -> verification -> resume",
            state: "designing",
            permission_mode: "default approval",
            resume_state: "recoverable from Tauri snapshot",
            engine: "Autopilot agent engine contract",
        },
        timeline: vec![
            WorkbenchTimelineEvent {
                id: "event.freeze-target-loop",
                time: "T+00:00",
                state: "done",
                label: "Target loop frozen",
                detail: "One visual workbench owns the repo, session, timeline, approvals, diffs, verification, and resume state.",
                owner: "Autopilot",
                evidence: "docs/autopilot-clean-room-rebuild-decisions.md",
            },
            WorkbenchTimelineEvent {
                id: "event.contract-first",
                time: "T+00:07",
                state: "running",
                label: "Workbench contract exposed",
                detail: "Rust/Tauri now provides the workbench snapshot that React renders.",
                owner: "Autopilot core",
                evidence: "autopilot_workbench_snapshot",
            },
            WorkbenchTimelineEvent {
                id: "event.engine-next",
                time: "T+next",
                state: "queued",
                label: "Attach real agent engine",
                detail: "Replace demo session data with persisted turn/tool/approval events from the engine boundary.",
                owner: "Agent engine",
                evidence: "pending contract implementation",
            },
        ],
        approvals: vec![
            WorkbenchApproval {
                id: "approval.write-policy.001",
                state: "required",
                risk: "medium",
                request: "Allow controlled file edits through the engine boundary.",
                policy: "UI may request; Rust authority enforces.",
                paths: vec!["apps/autopilot/src", "apps/autopilot/src-tauri/src"],
            },
            WorkbenchApproval {
                id: "approval.process-control.001",
                state: "held",
                risk: "high",
                request: "Launch or attach a long-running agent process.",
                policy: "Explicit user approval until trust rules exist.",
                paths: vec!["local process table", "workspace files"],
            },
        ],
        diffs: vec![
            WorkbenchDiff {
                id: "diff.workbench-shell",
                state: "draft",
                file: "apps/autopilot/src/App.tsx",
                summary: "Default surface becomes a visual workbench instead of command-only console.",
                additions: 210,
                deletions: 8,
            },
            WorkbenchDiff {
                id: "diff.tauri-contract",
                state: "draft",
                file: "apps/autopilot/src-tauri/src/lib.rs",
                summary: "Adds Rust-owned snapshot contract for Autopilot workbench state.",
                additions: 150,
                deletions: 0,
            },
        ],
        verification: vec![
            WorkbenchVerification {
                id: "verify.frontend-build",
                state: "pending",
                command: "cd apps/autopilot && bun run build",
                elapsed_ms: 0,
                detail: "Confirms the clean-room workbench compiles.",
            },
            WorkbenchVerification {
                id: "verify.rust-check",
                state: "pending",
                command: "cargo check -p autopilot",
                elapsed_ms: 0,
                detail: "Confirms the Tauri contract compiles.",
            },
        ],
        evidence: vec![
            WorkbenchEvidence {
                id: "evidence.decision-log",
                kind: "decision_log",
                state: "tracked",
                location: "docs/autopilot-clean-room-rebuild-decisions.md",
                owner: "Autopilot",
            },
            WorkbenchEvidence {
                id: "evidence.snapshot-contract",
                kind: "tauri_command",
                state: "tracked",
                location: "autopilot_workbench_snapshot",
                owner: "Autopilot core",
            },
        ],
        scorecard: WorkbenchScorecard {
            first_tool_event_seconds: 0,
            verified_diff_minutes: 0,
            recovery_state: "contract defined, persistence next",
            human_interventions: 0,
            sats_earned_today: 0,
        },
    }
}

fn current_git_branch(path: &std::path::Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["-C", path.to_str()?, "rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8(output.stdout)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn unix_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(pylon::PylonManager::default())
        .setup(|app| {
            control::start_control_plane(app.handle().clone()).map_err(|error| error.into())
        })
        .invoke_handler(tauri::generate_handler![
            autopilot_status,
            autopilot_workbench_snapshot,
            pylon::pylon_detect,
            pylon::pylon_get_status,
            pylon::pylon_start,
            pylon::pylon_stop,
            pylon::pylon_restart,
            pylon::pylon_set_mode,
            pylon::pylon_open_logs,
            pylon::proof_run,
            pylon::proof_get,
            pylon::proof_doctor,
            pylon::proof_stop,
            pylon::proof_reset,
            pylon::proof_open_artifacts,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::autopilot_workbench_snapshot;

    #[test]
    fn workbench_snapshot_keeps_autopilot_as_visible_product() {
        let snapshot = autopilot_workbench_snapshot();

        assert_eq!(snapshot.product, "Autopilot");
        assert_eq!(snapshot.visible_surface, "workbench");
        assert!(snapshot.session.engine.contains("Autopilot"));
    }
}
