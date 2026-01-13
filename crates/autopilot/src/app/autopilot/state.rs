use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use tokio::sync::mpsc;

use crate::autopilot_loop::DspyStage;

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
        }
    }
}
