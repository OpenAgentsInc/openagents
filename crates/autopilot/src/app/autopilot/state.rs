use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use tokio::sync::mpsc;

use crate::app::autopilot::PostCompletionResult;
use crate::autopilot_loop::DspyStage;

/// Issue pending validation after user selection.
#[derive(Debug, Clone)]
pub(crate) struct PendingValidation {
    pub issue_number: u32,
    pub title: String,
    pub description: Option<String>,
    pub blocked_reason: Option<String>,
}

pub(crate) struct AutopilotState {
    pub(crate) oanix_manifest: Option<adjutant::OanixManifest>,
    pub(crate) oanix_manifest_rx: Option<mpsc::UnboundedReceiver<adjutant::OanixManifest>>,
    pub(crate) autopilot_history: Vec<adjutant::ConversationTurn>,
    pub(crate) autopilot_history_rx:
        Option<mpsc::UnboundedReceiver<Vec<adjutant::ConversationTurn>>>,
    pub(crate) autopilot_interrupt_flag: Arc<AtomicBool>,
    pub(crate) autopilot_loop_iteration: usize,
    pub(crate) autopilot_max_iterations: usize,
    /// Issue suggestions computed after boot
    pub(crate) issue_suggestions: Option<DspyStage>,
    /// Channel for receiving issue suggestions from async task
    pub(crate) issue_suggestions_rx: Option<mpsc::UnboundedReceiver<DspyStage>>,
    /// Pending issue prompt to submit (set when user selects an issue from suggestions)
    pub(crate) pending_issue_prompt: Option<String>,

    // === Issue validation state ===
    /// Issue pending validation (after user selects but before work starts)
    pub(crate) pending_validation: Option<PendingValidation>,
    /// Channel for receiving validation results from async task
    pub(crate) validation_result_rx: Option<mpsc::UnboundedReceiver<adjutant::dspy::IssueValidationResult>>,

    // === Post-completion hook state ===
    /// Issue currently being worked on (UUID)
    pub(crate) current_issue_id: Option<String>,
    /// Issue number for display/prompts
    pub(crate) current_issue_number: Option<i32>,
    /// Issue title for retry prompts
    pub(crate) current_issue_title: Option<String>,
    /// Issue description for verification
    pub(crate) current_issue_description: Option<String>,
    /// Retry count for verification failures (max 1)
    pub(crate) current_issue_retry_count: u8,
    /// Whether to auto-start next issue after completion (autopilot continuous mode)
    pub(crate) autopilot_continuous_mode: bool,
    /// Channel for receiving post-completion results from async task
    pub(crate) post_completion_rx: Option<mpsc::UnboundedReceiver<PostCompletionResult>>,
}

impl AutopilotState {
    pub(crate) fn new(
        oanix_manifest_rx: Option<mpsc::UnboundedReceiver<adjutant::OanixManifest>>,
    ) -> Self {
        Self {
            oanix_manifest: None,
            oanix_manifest_rx,
            autopilot_history: Vec::new(),
            autopilot_history_rx: None,
            autopilot_interrupt_flag: Arc::new(AtomicBool::new(false)),
            autopilot_loop_iteration: 0,
            autopilot_max_iterations: 10,
            issue_suggestions: None,
            issue_suggestions_rx: None,
            pending_issue_prompt: None,
            // Issue validation state
            pending_validation: None,
            validation_result_rx: None,
            // Post-completion hook state
            current_issue_id: None,
            current_issue_number: None,
            current_issue_title: None,
            current_issue_description: None,
            current_issue_retry_count: 0,
            autopilot_continuous_mode: false,
            post_completion_rx: None,
        }
    }

    /// Clear current issue tracking state
    pub(crate) fn clear_current_issue(&mut self) {
        self.current_issue_id = None;
        self.current_issue_number = None;
        self.current_issue_title = None;
        self.current_issue_description = None;
        self.current_issue_retry_count = 0;
    }
}
