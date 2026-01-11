#[derive(Clone, Debug)]
pub(crate) struct PermissionHistoryEntry {
    pub(crate) tool_name: String,
    pub(crate) decision: String,
    pub(crate) timestamp: u64,
    pub(crate) detail: Option<String>,
}
