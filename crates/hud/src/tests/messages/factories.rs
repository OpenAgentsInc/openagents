//! Message factory functions for creating test HUD messages
//!
//! These factories generate valid HudMessage instances with sensible defaults,
//! making it easy to create test data without specifying every field.

use super::super::protocol::*;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn generate_id(prefix: &str) -> String {
    let count = COUNTER.fetch_add(1, Ordering::SeqCst);
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("{}-{}-{:x}", prefix, count, timestamp)
}

fn now_timestamp() -> String {
    chrono::Utc::now().to_rfc3339()
}

// ============================================================================
// Session Messages
// ============================================================================

/// Create a session_start message
pub fn session_start(session_id: Option<&str>) -> HudMessage {
    HudMessage::SessionStart {
        session_id: session_id
            .map(String::from)
            .unwrap_or_else(|| generate_id("session")),
        timestamp: now_timestamp(),
    }
}

/// Create a session_complete message
pub fn session_complete(success: bool, summary: &str) -> HudMessage {
    HudMessage::SessionComplete {
        success,
        summary: summary.to_string(),
    }
}

// ============================================================================
// Task Messages
// ============================================================================

/// Create a HudTaskInfo with defaults
pub fn task_info(id: Option<&str>, title: &str) -> HudTaskInfo {
    HudTaskInfo {
        id: id.map(String::from).unwrap_or_else(|| generate_id("oa")),
        title: title.to_string(),
        status: "in_progress".to_string(),
        priority: 1,
    }
}

/// Create a task_selected message
pub fn task_selected(task: HudTaskInfo) -> HudMessage {
    HudMessage::TaskSelected { task }
}

/// Convenience: Create a task_selected message with just id and title
pub fn task_selected_simple(id: &str, title: &str) -> HudMessage {
    task_selected(task_info(Some(id), title))
}

/// Create a HudSubtaskInfo with defaults
pub fn subtask_info(id: Option<&str>, description: &str) -> HudSubtaskInfo {
    HudSubtaskInfo {
        id: id.map(String::from).unwrap_or_else(|| generate_id("sub")),
        description: description.to_string(),
        status: SubtaskStatus::Pending,
    }
}

/// Create a task_decomposed message
pub fn task_decomposed(subtasks: Vec<HudSubtaskInfo>) -> HudMessage {
    HudMessage::TaskDecomposed { subtasks }
}

/// Convenience: Create a task_decomposed message with simple subtask definitions
pub fn task_decomposed_simple(subtasks: Vec<(&str, &str)>) -> HudMessage {
    let subtask_infos: Vec<HudSubtaskInfo> = subtasks
        .into_iter()
        .map(|(id, desc)| subtask_info(Some(id), desc))
        .collect();
    task_decomposed(subtask_infos)
}

/// Create a subtask_start message
pub fn subtask_start(subtask: HudSubtaskInfo) -> HudMessage {
    HudMessage::SubtaskStart {
        subtask: HudSubtaskInfo {
            status: SubtaskStatus::InProgress,
            ..subtask
        },
    }
}

/// Convenience: Create a subtask_start message with just id and description
pub fn subtask_start_simple(id: &str, description: &str) -> HudMessage {
    subtask_start(subtask_info(Some(id), description))
}

/// Create a subtask_complete message
pub fn subtask_complete(subtask: HudSubtaskInfo, result: Option<HudSubagentResult>) -> HudMessage {
    HudMessage::SubtaskComplete {
        subtask: HudSubtaskInfo {
            status: SubtaskStatus::Done,
            ..subtask
        },
        result: result.unwrap_or_default(),
    }
}

/// Convenience: Create a subtask_complete message with simple parameters
pub fn subtask_complete_simple(id: &str, description: &str, success: bool, files: Vec<String>) -> HudMessage {
    let result = if success {
        Some(HudSubagentResult {
            success: true,
            agent: Some(AgentType::ClaudeCode),
            files_modified: files,
            turns: 1,
            error: None,
        })
    } else {
        Some(HudSubagentResult {
            success: false,
            agent: Some(AgentType::ClaudeCode),
            files_modified: vec![],
            turns: 1,
            error: Some("Failed".to_string()),
        })
    };
    subtask_complete(subtask_info(Some(id), description), result)
}

/// Create a subtask_failed message
pub fn subtask_failed(subtask: HudSubtaskInfo, error: &str) -> HudMessage {
    HudMessage::SubtaskFailed {
        subtask: HudSubtaskInfo {
            status: SubtaskStatus::Failed,
            ..subtask
        },
        error: error.to_string(),
    }
}

// ============================================================================
// Verification Messages
// ============================================================================

/// Create a verification_start message
pub fn verification_start(command: &str) -> HudMessage {
    HudMessage::VerificationStart {
        command: command.to_string(),
    }
}

/// Create a verification_complete message
pub fn verification_complete(command: &str, passed: bool, output: Option<&str>) -> HudMessage {
    HudMessage::VerificationComplete {
        command: command.to_string(),
        passed,
        output: output.map(String::from),
    }
}

// ============================================================================
// Git Messages
// ============================================================================

/// Create a commit_created message
pub fn commit_created(sha: &str, message: &str) -> HudMessage {
    HudMessage::CommitCreated {
        sha: sha.to_string(),
        message: message.to_string(),
    }
}

/// Create a push_complete message
pub fn push_complete(branch: &str) -> HudMessage {
    HudMessage::PushComplete {
        branch: branch.to_string(),
    }
}

// ============================================================================
// Phase and Error Messages
// ============================================================================

/// Create a phase_change message
pub fn phase_change(phase: OrchestratorPhase) -> HudMessage {
    HudMessage::PhaseChange { phase }
}

/// Create an error message
pub fn error(phase: OrchestratorPhase, error_msg: &str) -> HudMessage {
    HudMessage::Error {
        phase,
        error: error_msg.to_string(),
    }
}

/// Create an error message with phase as string (convenience)
pub fn error_in_phase(phase_name: &str, error_msg: &str) -> HudMessage {
    let phase = match phase_name {
        "init" => OrchestratorPhase::Init,
        "selecting" => OrchestratorPhase::Selecting,
        "decomposing" => OrchestratorPhase::Decomposing,
        "executing" => OrchestratorPhase::Executing,
        "verifying" => OrchestratorPhase::Verifying,
        "committing" => OrchestratorPhase::Committing,
        "pushing" => OrchestratorPhase::Pushing,
        "complete" => OrchestratorPhase::Complete,
        "failed" => OrchestratorPhase::Failed,
        _ => OrchestratorPhase::Failed,
    };
    error(phase, error_msg)
}

// ============================================================================
// APM Messages
// ============================================================================

/// Create an apm_update message
pub fn apm_update(session_apm: f64, total_actions: u64) -> HudMessage {
    HudMessage::ApmUpdate {
        session_id: generate_id("session"),
        session_apm,
        recent_apm: session_apm * 1.1,
        total_actions,
        duration_minutes: if session_apm > 0.0 {
            total_actions as f64 / session_apm
        } else {
            0.0
        },
    }
}

/// Create an apm_update with specific session_id
pub fn apm_update_for_session(session_id: &str, session_apm: f64, total_actions: u64) -> HudMessage {
    HudMessage::ApmUpdate {
        session_id: session_id.to_string(),
        session_apm,
        recent_apm: session_apm * 1.1,
        total_actions,
        duration_minutes: if session_apm > 0.0 {
            total_actions as f64 / session_apm
        } else {
            0.0
        },
    }
}

// ============================================================================
// Streaming Output Messages
// ============================================================================

/// Create a text_output message
pub fn text_output(text: &str, source: Option<AgentType>) -> HudMessage {
    HudMessage::TextOutput {
        text: text.to_string(),
        source,
    }
}

/// Create a tool_call message
pub fn tool_call(tool_name: &str, arguments: &str) -> HudMessage {
    HudMessage::ToolCall {
        tool_name: tool_name.to_string(),
        arguments: arguments.to_string(),
        call_id: Some(generate_id("call")),
    }
}

/// Create a tool_result message
pub fn tool_result(tool_name: &str, result: &str, is_error: bool) -> HudMessage {
    HudMessage::ToolResult {
        tool_name: tool_name.to_string(),
        result: result.to_string(),
        is_error,
        call_id: None,
    }
}

// ============================================================================
// Subagent Result Builders
// ============================================================================

/// Create a successful subagent result
pub fn subagent_result_success(files: Vec<&str>, turns: u32) -> HudSubagentResult {
    HudSubagentResult {
        success: true,
        agent: Some(AgentType::ClaudeCode),
        files_modified: files.into_iter().map(String::from).collect(),
        turns,
        error: None,
    }
}

/// Create a failed subagent result
pub fn subagent_result_failure(error: &str) -> HudSubagentResult {
    HudSubagentResult {
        success: false,
        agent: Some(AgentType::ClaudeCode),
        files_modified: Vec::new(),
        turns: 1,
        error: Some(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_start_generates_id() {
        let msg = session_start(None);
        if let HudMessage::SessionStart { session_id, .. } = msg {
            assert!(session_id.starts_with("session-"));
        } else {
            panic!("Expected SessionStart");
        }
    }

    #[test]
    fn test_session_start_uses_provided_id() {
        let msg = session_start(Some("my-session"));
        if let HudMessage::SessionStart { session_id, .. } = msg {
            assert_eq!(session_id, "my-session");
        } else {
            panic!("Expected SessionStart");
        }
    }

    #[test]
    fn test_task_info_defaults() {
        let task = task_info(None, "My Task");
        assert!(task.id.starts_with("oa-"));
        assert_eq!(task.title, "My Task");
        assert_eq!(task.status, "in_progress");
        assert_eq!(task.priority, 1);
    }

    #[test]
    fn test_apm_update_calculation() {
        let msg = apm_update(10.0, 100);
        if let HudMessage::ApmUpdate {
            session_apm,
            total_actions,
            duration_minutes,
            ..
        } = msg
        {
            assert_eq!(session_apm, 10.0);
            assert_eq!(total_actions, 100);
            assert_eq!(duration_minutes, 10.0); // 100 actions / 10 APM = 10 minutes
        } else {
            panic!("Expected ApmUpdate");
        }
    }
}
