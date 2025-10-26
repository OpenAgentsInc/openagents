//! Control message schema and parser for WS payloads.
//!
//! The mobile/desktop clients send small JSON control messages over the same
//! WebSocket used for Codex stream broadcasting. This module defines the
//! supported control verbs and a tolerant parser that extracts them from the
//! inbound payload.

#[derive(Debug)]
pub enum ControlCommand {
    Interrupt,
    Projects,
    Skills,
    BridgeStatus,
    ConvexStatus,
    ConvexCreateDemo,
    ConvexCreateThreads,
    ConvexCreateDemoThread,
    ConvexBackfill,
    ProjectSave { project: crate::projects::Project },
    ProjectDelete { id: String },
    RunSubmit { thread_doc_id: String, text: String, project_id: Option<String>, resume_id: Option<String> },
}

/// Parse a control command from a raw JSON string. Returns None on errors.
pub fn parse_control_command(payload: &str) -> Option<ControlCommand> {
    let v: serde_json::Value = serde_json::from_str(payload).ok()?;
    let ty = v.get("control").and_then(|x| x.as_str())?;
    match ty {
        "interrupt" => Some(ControlCommand::Interrupt),
        "projects" => Some(ControlCommand::Projects),
        "skills" => Some(ControlCommand::Skills),
        "bridge.status" => Some(ControlCommand::BridgeStatus),
        "convex.status" => Some(ControlCommand::ConvexStatus),
        "convex.create_demo" => Some(ControlCommand::ConvexCreateDemo),
        "convex.create_threads" => Some(ControlCommand::ConvexCreateThreads),
        "convex.create_demo_thread" => Some(ControlCommand::ConvexCreateDemoThread),
        "convex.backfill" => Some(ControlCommand::ConvexBackfill),
        "project.save" => {
            let proj: crate::projects::Project = serde_json::from_value(v.get("project")?.clone()).ok()?;
            Some(ControlCommand::ProjectSave { project: proj })
        }
        "project.delete" => v.get("id").and_then(|x| x.as_str()).map(|id| ControlCommand::ProjectDelete { id: id.to_string() }),
        "run.submit" => {
            let thread_doc_id = v.get("threadDocId").and_then(|x| x.as_str()).map(|s| s.to_string())?;
            let text = v.get("text").and_then(|x| x.as_str()).map(|s| s.to_string()).unwrap_or_default();
            let project_id = v.get("projectId").and_then(|x| x.as_str()).map(|s| s.to_string());
            let resume_id = v.get("resumeId").and_then(|x| x.as_str()).map(|s| s.to_string());
            Some(ControlCommand::RunSubmit { thread_doc_id, text, project_id, resume_id })
        }
        _ => None,
    }
}
