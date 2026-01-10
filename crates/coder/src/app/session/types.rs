use serde::{Deserialize, Serialize};

/// Session info from SystemInit.
#[derive(Default)]
pub(crate) struct SessionInfo {
    pub(crate) model: String,
    pub(crate) permission_mode: String,
    pub(crate) session_id: String,
    pub(crate) tool_count: usize,
    pub(crate) tools: Vec<String>,
    #[allow(dead_code)]
    pub(crate) output_style: String,
    #[allow(dead_code)]
    pub(crate) slash_commands: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct SessionEntry {
    pub(crate) id: String,
    pub(crate) created_at: u64,
    pub(crate) updated_at: u64,
    pub(crate) last_message: String,
    pub(crate) message_count: usize,
    pub(crate) model: String,
}

#[derive(Clone, Debug)]
pub(crate) struct CheckpointEntry {
    pub(crate) user_message_id: String,
    pub(crate) label: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct StoredMessage {
    pub(crate) role: String,
    pub(crate) content: String,
    #[serde(default)]
    pub(crate) uuid: Option<String>,
}
