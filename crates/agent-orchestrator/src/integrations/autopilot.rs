use crate::hooks::{Hook, HookResult, SessionEvent, ToolCall, ToolOutput};
use async_trait::async_trait;
use dsrs::{example, Predict, Prediction, Predictor, Signature, GLOBAL_SETTINGS};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, RwLock};

// ============================================================================
// DSPy Signatures
// ============================================================================

#[Signature]
struct IssueSelectionSignature {
    /// Select next issue to work on.

    /// JSON array of open issues
    #[input]
    open_issues: String,

    /// Agent capabilities or identifier
    #[input]
    agent_capabilities: String,

    /// Recent work summary
    #[input]
    recent_work: String,

    /// Selected issue ID
    #[output]
    selected_issue_id: String,

    /// Rationale for the selection
    #[output]
    reasoning: String,

    /// Estimated complexity
    #[output]
    estimated_complexity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueInfo {
    pub id: String,
    pub number: i32,
    pub title: String,
    pub status: IssueStatus,
    pub claimed_by: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IssueStatus {
    Open,
    InProgress,
    Done,
    Blocked,
}

pub trait IssueStore: Send + Sync {
    fn claim_issue(&self, issue_id: &str, run_id: &str) -> Result<bool, String>;
    fn complete_issue(&self, issue_id: &str) -> Result<bool, String>;
    fn block_issue(&self, issue_id: &str, reason: &str) -> Result<bool, String>;
    fn get_issue(&self, issue_id: &str) -> Option<IssueInfo>;
    fn get_next_ready(&self, agent: Option<&str>) -> Option<IssueInfo>;
}

#[derive(Default)]
pub struct InMemoryIssueStore {
    issues: RwLock<HashMap<String, IssueInfo>>,
}

impl InMemoryIssueStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_issue(&self, issue: IssueInfo) {
        let mut issues = self.issues.write().unwrap();
        issues.insert(issue.id.clone(), issue);
    }
}

impl IssueStore for InMemoryIssueStore {
    fn claim_issue(&self, issue_id: &str, run_id: &str) -> Result<bool, String> {
        let mut guard = self.issues.write().map_err(|e| e.to_string())?;
        if let Some(issue) = guard.get_mut(issue_id)
            && issue.status == IssueStatus::Open
        {
            issue.status = IssueStatus::InProgress;
            issue.claimed_by = Some(run_id.to_string());
            return Ok(true);
        }
        Ok(false)
    }

    fn complete_issue(&self, issue_id: &str) -> Result<bool, String> {
        let mut guard = self.issues.write().map_err(|e| e.to_string())?;
        if let Some(issue) = guard.get_mut(issue_id) {
            issue.status = IssueStatus::Done;
            issue.claimed_by = None;
            return Ok(true);
        }
        Ok(false)
    }

    fn block_issue(&self, issue_id: &str, _reason: &str) -> Result<bool, String> {
        let mut guard = self.issues.write().map_err(|e| e.to_string())?;
        if let Some(issue) = guard.get_mut(issue_id) {
            issue.status = IssueStatus::Blocked;
            issue.claimed_by = None;
            return Ok(true);
        }
        Ok(false)
    }

    fn get_issue(&self, issue_id: &str) -> Option<IssueInfo> {
        let guard = self.issues.read().ok()?;
        guard.get(issue_id).cloned()
    }

    fn get_next_ready(&self, agent: Option<&str>) -> Option<IssueInfo> {
        let guard = self.issues.read().ok()?;
        let mut open_issues: Vec<IssueInfo> = guard
            .values()
            .filter(|i| i.status == IssueStatus::Open)
            .cloned()
            .collect();

        if open_issues.is_empty() {
            return None;
        }

        if let Some(selected_id) = select_issue_dspy(&open_issues, agent, &guard) {
            if let Some(selected) = open_issues.iter().find(|i| i.id == selected_id) {
                return Some(selected.clone());
            }
        }

        open_issues.sort_by(|a, b| a.id.cmp(&b.id));
        open_issues.into_iter().next()
    }
}

pub struct IssueClaimHook {
    store: Arc<dyn IssueStore>,
    run_id: String,
}

impl IssueClaimHook {
    pub fn new(store: Arc<dyn IssueStore>, run_id: String) -> Self {
        Self { store, run_id }
    }
}

#[async_trait]
impl Hook for IssueClaimHook {
    fn name(&self) -> &str {
        "issue-claim"
    }

    async fn before_tool(&self, call: &mut ToolCall) -> HookResult {
        if call.name == "issue_claim"
            && let Some(issue_id) = call.parameters.get("issue_id").and_then(|v| v.as_str())
        {
            match self.store.claim_issue(issue_id, &self.run_id) {
                Ok(true) => {
                    tracing::info!("Claimed issue: {}", issue_id);
                }
                Ok(false) => {
                    return HookResult::Block {
                        message: format!("Issue {} is not available for claiming", issue_id),
                    };
                }
                Err(e) => {
                    return HookResult::Block {
                        message: format!("Failed to claim issue: {}", e),
                    };
                }
            }
        }
        HookResult::Continue
    }
}

pub struct IssueCompleteHook {
    store: Arc<dyn IssueStore>,
}

impl IssueCompleteHook {
    pub fn new(store: Arc<dyn IssueStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Hook for IssueCompleteHook {
    fn name(&self) -> &str {
        "issue-complete"
    }

    async fn after_tool(&self, call: &ToolCall, _output: &mut ToolOutput) -> HookResult {
        if call.name == "issue_complete"
            && let Some(issue_id) = call.parameters.get("issue_id").and_then(|v| v.as_str())
            && let Err(e) = self.store.complete_issue(issue_id)
        {
            tracing::warn!("Failed to complete issue {}: {}", issue_id, e);
        }
        HookResult::Continue
    }

    async fn on_session(&self, event: &SessionEvent) -> HookResult {
        if let SessionEvent::Error { error, .. } = event {
            tracing::warn!("Session error, issues may need unblocking: {}", error);
        }
        HookResult::Continue
    }
}

pub struct AutopilotIntegration {
    store: Arc<dyn IssueStore>,
    run_id: String,
    current_issue: RwLock<Option<String>>,
    completed_count: RwLock<u32>,
}

impl AutopilotIntegration {
    pub fn new(store: Arc<dyn IssueStore>, run_id: String) -> Self {
        Self {
            store,
            run_id,
            current_issue: RwLock::new(None),
            completed_count: RwLock::new(0),
        }
    }

    pub fn claim_next(&self, agent: Option<&str>) -> Option<IssueInfo> {
        let issue = self.store.get_next_ready(agent)?;

        if self.store.claim_issue(&issue.id, &self.run_id).ok()? {
            if let Ok(mut guard) = self.current_issue.write() {
                *guard = Some(issue.id.clone());
            }
            Some(issue)
        } else {
            None
        }
    }

    pub fn complete_current(&self) -> bool {
        let issue_id = self
            .current_issue
            .read()
            .ok()
            .and_then(|guard| guard.clone());

        if let Some(id) = issue_id
            && self.store.complete_issue(&id).unwrap_or(false)
        {
            if let Ok(mut guard) = self.current_issue.write() {
                *guard = None;
            }
            if let Ok(mut count) = self.completed_count.write() {
                *count += 1;
            }
            return true;
        }
        false
    }

    pub fn block_current(&self, reason: &str) -> bool {
        let issue_id = self
            .current_issue
            .read()
            .ok()
            .and_then(|guard| guard.clone());

        if let Some(id) = issue_id
            && self.store.block_issue(&id, reason).unwrap_or(false)
        {
            if let Ok(mut guard) = self.current_issue.write() {
                *guard = None;
            }
            return true;
        }
        false
    }

    pub fn current_issue_id(&self) -> Option<String> {
        self.current_issue
            .read()
            .ok()
            .and_then(|guard| guard.clone())
    }

    pub fn completed_count(&self) -> u32 {
        self.completed_count.read().map(|guard| *guard).unwrap_or(0)
    }

    pub fn create_hooks(&self) -> (IssueClaimHook, IssueCompleteHook) {
        (
            IssueClaimHook::new(self.store.clone(), self.run_id.clone()),
            IssueCompleteHook::new(self.store.clone()),
        )
    }
}

fn dspy_ready() -> bool {
    GLOBAL_SETTINGS.read().unwrap().is_some()
}

fn run_prediction<F>(future: F) -> Option<Prediction>
where
    F: Future<Output = std::result::Result<Prediction, anyhow::Error>>,
{
    if !dspy_ready() {
        return None;
    }

    let result = if let Ok(handle) = tokio::runtime::Handle::try_current() {
        catch_unwind(AssertUnwindSafe(|| {
            tokio::task::block_in_place(|| handle.block_on(future))
        }))
    } else if let Ok(runtime) = tokio::runtime::Runtime::new() {
        catch_unwind(AssertUnwindSafe(|| runtime.block_on(future)))
    } else {
        return None;
    };

    match result {
        Ok(Ok(prediction)) => Some(prediction),
        _ => None,
    }
}

fn get_string(prediction: &Prediction, key: &str) -> String {
    let val = prediction.get(key, None);
    if let Some(s) = val.as_str() {
        s.to_string()
    } else {
        val.to_string().trim_matches('"').to_string()
    }
}

fn summarize_recent_work(issues: &HashMap<String, IssueInfo>) -> String {
    let mut summaries = Vec::new();
    for issue in issues.values() {
        if issue.status != IssueStatus::Open {
            summaries.push(format!("{} ({:?})", issue.id, issue.status));
        }
    }

    if summaries.is_empty() {
        "None".to_string()
    } else {
        summaries.join(", ")
    }
}

fn select_issue_dspy(
    open_issues: &[IssueInfo],
    agent: Option<&str>,
    all_issues: &HashMap<String, IssueInfo>,
) -> Option<String> {
    if !dspy_ready() {
        return None;
    }

    let open_issues_json = serde_json::to_string(open_issues).ok()?;
    let recent_work = summarize_recent_work(all_issues);
    let capabilities = agent
        .map(|id| format!("agent_id: {}", id))
        .unwrap_or_else(|| "unknown".to_string());

    let selector = Predict::new(IssueSelectionSignature::new());
    let example = example! {
        "open_issues": "input" => open_issues_json,
        "agent_capabilities": "input" => capabilities,
        "recent_work": "input" => recent_work,
    };

    let prediction = run_prediction(selector.forward(example))?;
    let selected_id = get_string(&prediction, "selected_issue_id");

    if selected_id.is_empty() {
        None
    } else {
        Some(selected_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_issue(id: &str, number: i32, title: &str) -> IssueInfo {
        IssueInfo {
            id: id.to_string(),
            number,
            title: title.to_string(),
            status: IssueStatus::Open,
            claimed_by: None,
        }
    }

    #[test]
    fn test_in_memory_store_add_and_get() {
        let store = InMemoryIssueStore::new();
        let issue = create_test_issue("issue-1", 1, "Test Issue");
        store.add_issue(issue);

        let retrieved = store.get_issue("issue-1");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().title, "Test Issue");
    }

    #[test]
    fn test_claim_issue() {
        let store = InMemoryIssueStore::new();
        store.add_issue(create_test_issue("issue-1", 1, "Test"));

        let result = store.claim_issue("issue-1", "run-123");
        assert!(result.is_ok());
        assert!(result.unwrap());

        let issue = store.get_issue("issue-1").unwrap();
        assert_eq!(issue.status, IssueStatus::InProgress);
        assert_eq!(issue.claimed_by, Some("run-123".to_string()));
    }

    #[test]
    fn test_claim_already_claimed() {
        let store = InMemoryIssueStore::new();
        let mut issue = create_test_issue("issue-1", 1, "Test");
        issue.status = IssueStatus::InProgress;
        issue.claimed_by = Some("run-other".to_string());
        store.add_issue(issue);

        let result = store.claim_issue("issue-1", "run-123");
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_complete_issue() {
        let store = InMemoryIssueStore::new();
        let mut issue = create_test_issue("issue-1", 1, "Test");
        issue.status = IssueStatus::InProgress;
        store.add_issue(issue);

        let result = store.complete_issue("issue-1");
        assert!(result.is_ok());

        let issue = store.get_issue("issue-1").unwrap();
        assert_eq!(issue.status, IssueStatus::Done);
    }

    #[test]
    fn test_block_issue() {
        let store = InMemoryIssueStore::new();
        store.add_issue(create_test_issue("issue-1", 1, "Test"));

        let result = store.block_issue("issue-1", "Needs review");
        assert!(result.is_ok());

        let issue = store.get_issue("issue-1").unwrap();
        assert_eq!(issue.status, IssueStatus::Blocked);
    }

    #[test]
    fn test_get_next_ready() {
        let store = InMemoryIssueStore::new();
        store.add_issue(create_test_issue("issue-1", 1, "First"));

        let next = store.get_next_ready(None);
        assert!(next.is_some());
        assert_eq!(next.unwrap().id, "issue-1");
    }

    #[test]
    fn test_autopilot_integration_workflow() {
        let store = Arc::new(InMemoryIssueStore::new());
        store.add_issue(create_test_issue("issue-1", 1, "Task 1"));
        store.add_issue(create_test_issue("issue-2", 2, "Task 2"));

        let integration = AutopilotIntegration::new(store, "run-test".to_string());

        let first_issue = integration.claim_next(None);
        assert!(first_issue.is_some());
        let first_id = first_issue.unwrap().id;
        assert!(first_id == "issue-1" || first_id == "issue-2");
        assert_eq!(integration.current_issue_id(), Some(first_id.clone()));

        assert!(integration.complete_current());
        assert!(integration.current_issue_id().is_none());
        assert_eq!(integration.completed_count(), 1);

        let second_issue = integration.claim_next(None);
        assert!(second_issue.is_some());
        let second_id = second_issue.unwrap().id;
        assert_ne!(second_id, first_id);
    }

    #[test]
    fn test_autopilot_block_current() {
        let store = Arc::new(InMemoryIssueStore::new());
        store.add_issue(create_test_issue("issue-1", 1, "Task"));

        let integration = AutopilotIntegration::new(store.clone(), "run-test".to_string());

        integration.claim_next(None);
        assert!(integration.block_current("Needs review"));

        let issue = store.get_issue("issue-1").unwrap();
        assert_eq!(issue.status, IssueStatus::Blocked);
        assert!(integration.current_issue_id().is_none());
    }

    #[tokio::test]
    async fn test_issue_claim_hook() {
        let store = Arc::new(InMemoryIssueStore::new());
        store.add_issue(create_test_issue("issue-1", 1, "Task"));

        let hook = IssueClaimHook::new(store.clone(), "run-test".to_string());

        let mut params = HashMap::new();
        params.insert("issue_id".to_string(), serde_json::json!("issue-1"));

        let mut call = ToolCall {
            name: "issue_claim".to_string(),
            parameters: params,
            session_id: "session-1".to_string(),
        };

        let result = hook.before_tool(&mut call).await;
        assert!(matches!(result, HookResult::Continue));
    }

    #[tokio::test]
    async fn test_issue_claim_hook_unavailable() {
        let store = Arc::new(InMemoryIssueStore::new());
        let mut issue = create_test_issue("issue-1", 1, "Task");
        issue.status = IssueStatus::InProgress;
        store.add_issue(issue);

        let hook = IssueClaimHook::new(store, "run-test".to_string());

        let mut params = HashMap::new();
        params.insert("issue_id".to_string(), serde_json::json!("issue-1"));

        let mut call = ToolCall {
            name: "issue_claim".to_string(),
            parameters: params,
            session_id: "session-1".to_string(),
        };

        let result = hook.before_tool(&mut call).await;
        assert!(matches!(result, HookResult::Block { .. }));
    }
}
