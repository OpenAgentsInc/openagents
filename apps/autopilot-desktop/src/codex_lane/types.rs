#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexSkillListEntry {
    pub cwd: String,
    pub skills: Vec<CodexSkillSummary>,
    pub errors: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexSkillSummary {
    pub name: String,
    pub path: String,
    pub scope: String,
    pub enabled: bool,
    pub interface_display_name: Option<String>,
    pub dependency_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexThreadListEntry {
    pub thread_id: String,
    pub thread_name: Option<String>,
    pub preview: String,
    pub status: Option<String>,
    pub loaded: bool,
    pub cwd: Option<String>,
    pub path: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexThreadTranscriptRole {
    User,
    Codex,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexThreadTranscriptMessage {
    pub role: CodexThreadTranscriptRole,
    pub content: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexThreadPlanArtifact {
    pub turn_id: String,
    pub text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexThreadReviewArtifact {
    pub turn_id: String,
    pub review: String,
    pub completed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexThreadCompactionArtifact {
    pub turn_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexModelCatalogEntry {
    pub model: String,
    pub display_name: String,
    pub description: String,
    pub hidden: bool,
    pub is_default: bool,
    pub default_reasoning_effort: String,
    pub supported_reasoning_efforts: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexMcpServerStatusEntry {
    pub name: String,
    pub auth_status: String,
    pub tool_count: usize,
    pub resource_count: usize,
    pub template_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexAppEntry {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub is_accessible: bool,
    pub is_enabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexRemoteSkillEntry {
    pub id: String,
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexTurnPlanStep {
    pub step: String,
    pub status: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexCommandApprovalRequest {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub reason: Option<String>,
    pub command: Option<String>,
    pub cwd: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexFileChangeApprovalRequest {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub reason: Option<String>,
    pub grant_root: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexToolCallRequest {
    pub thread_id: String,
    pub turn_id: String,
    pub call_id: String,
    pub tool: String,
    pub arguments: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexToolUserInputQuestion {
    pub id: String,
    pub header: String,
    pub question: String,
    pub options: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexToolUserInputRequest {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub questions: Vec<CodexToolUserInputQuestion>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexAuthTokensRefreshRequest {
    pub reason: String,
    pub previous_account_id: Option<String>,
}
