use anyhow::{Result, anyhow};
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};
use tracing::{info, warn};

use crate::github::{GitHubClient, branch_name_for_issue};

/// GitHub workflow orchestrator for autopilot.
///
/// Core issue progression is decoupled from branch/PR side effects:
/// branch/PR mutations are enqueued as deferred integration exports.
pub struct GitHubWorkflow {
    client: Option<GitHubClient>,
    agent_identity: String,
    export_queue: Arc<Mutex<GitHubExportQueue>>,
}

impl GitHubWorkflow {
    /// Create a new GitHub workflow with authentication.
    pub fn new(token: &str, agent_identity: String) -> Result<Self> {
        let client = GitHubClient::new(token)?;
        Ok(Self {
            client: Some(client),
            agent_identity,
            export_queue: Arc::new(Mutex::new(GitHubExportQueue::default())),
        })
    }

    /// Create a workflow that can queue exports without an active GitHub client.
    pub fn without_client(agent_identity: String) -> Self {
        Self {
            client: None,
            agent_identity,
            export_queue: Arc::new(Mutex::new(GitHubExportQueue::default())),
        }
    }

    /// Core issue execution handshake: claim issue + queue branch export.
    pub async fn execute_issue(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
        issue_title: &str,
        _base_branch: &str,
        base_sha: &str,
    ) -> Result<IssueWorkflowResult> {
        info!(
            "Starting GitHub workflow for issue #{} in {}/{}",
            issue_number, owner, repo
        );

        let branch_name = branch_name_for_issue(issue_number, issue_title);

        let client = self.require_client()?;

        client
            .claim_issue(owner, repo, issue_number, &self.agent_identity)
            .await?;

        client
            .add_label(owner, repo, issue_number, "in-progress")
            .await?;

        let enqueue = self.enqueue_branch_export(
            owner,
            repo,
            issue_number,
            branch_name.clone(),
            base_sha.to_string(),
        );

        info!(
            "GitHub workflow initialized for issue #{} (branch export intent #{})",
            issue_number, enqueue.intent_id
        );

        Ok(IssueWorkflowResult {
            branch_name,
            issue_number,
            branch_export_intent_id: enqueue.intent_id,
        })
    }

    /// Queue a pull-request export after work is complete.
    pub async fn create_pr(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
        branch_name: &str,
        base_branch: &str,
        pr_title: &str,
        pr_body: &str,
    ) -> Result<GitHubExportEnqueueResult> {
        let enqueue = self.enqueue_pr_export(
            owner,
            repo,
            issue_number,
            branch_name.to_string(),
            base_branch.to_string(),
            pr_title.to_string(),
            pr_body.to_string(),
        );

        info!(
            "Queued PR export intent #{} for issue #{} in {}/{}",
            enqueue.intent_id, issue_number, owner, repo
        );

        Ok(enqueue)
    }

    /// Dispatch one queued/failed export intent.
    pub async fn dispatch_next_export(&self) -> Result<Option<GitHubExportDispatchResult>> {
        let Some(candidate) = self.mark_next_intent_dispatching() else {
            return Ok(None);
        };
        let client = self.require_client()?;

        let execution = match &candidate.payload {
            GitHubExportPayload::CreateBranch {
                branch_name,
                base_sha,
            } => client
                .create_branch(
                    &candidate.summary.owner,
                    &candidate.summary.repo,
                    branch_name,
                    base_sha,
                )
                .await
                .map(|_| GitHubExportExecutionResult { external_id: None }),
            GitHubExportPayload::CreatePullRequest {
                branch_name,
                base_branch,
                title,
                body,
            } => client
                .create_pull_request(
                    &candidate.summary.owner,
                    &candidate.summary.repo,
                    title,
                    body,
                    branch_name,
                    base_branch,
                )
                .await
                .map(|pr_number| GitHubExportExecutionResult {
                    external_id: Some(pr_number),
                }),
        };

        match execution {
            Ok(result) => {
                if candidate.summary.kind == GitHubExportKind::CreatePullRequest {
                    if let Err(error) = client
                        .add_label(
                            &candidate.summary.owner,
                            &candidate.summary.repo,
                            candidate.summary.issue_number,
                            "needs-review",
                        )
                        .await
                    {
                        warn!(
                            "PR export succeeded but needs-review label update failed for issue #{}: {}",
                            candidate.summary.issue_number, error
                        );
                    }
                }

                let snapshot =
                    self.complete_intent(candidate.summary.intent_id, result.external_id);
                Ok(Some(GitHubExportDispatchResult {
                    intent_id: snapshot.intent_id,
                    kind: snapshot.kind,
                    status: snapshot.status,
                    attempts: snapshot.attempts,
                    external_id: snapshot.external_id,
                    error: None,
                }))
            }
            Err(error) => {
                let error_text = error.to_string();
                let snapshot = self.fail_intent(candidate.summary.intent_id, error_text.clone());
                Ok(Some(GitHubExportDispatchResult {
                    intent_id: snapshot.intent_id,
                    kind: snapshot.kind,
                    status: snapshot.status,
                    attempts: snapshot.attempts,
                    external_id: snapshot.external_id,
                    error: Some(error_text),
                }))
            }
        }
    }

    /// Retry a failed export intent by returning it to queued state.
    pub fn retry_export(&self, intent_id: u64) -> bool {
        let mut queue = self
            .export_queue
            .lock()
            .expect("export queue mutex poisoned");
        if let Some(intent) = queue
            .intents
            .iter_mut()
            .find(|intent| intent.summary.intent_id == intent_id)
        {
            if intent.summary.status == GitHubExportStatus::Failed {
                intent.summary.status = GitHubExportStatus::Queued;
                intent.summary.last_error = None;
                return true;
            }
        }
        false
    }

    /// Observable export queue state for diagnostics.
    pub fn export_queue_snapshot(&self) -> Vec<GitHubExportIntent> {
        let queue = self
            .export_queue
            .lock()
            .expect("export queue mutex poisoned");
        queue
            .intents
            .iter()
            .map(|intent| intent.summary.clone())
            .collect()
    }

    /// Post a receipt comment on the PR with execution details.
    pub async fn post_receipt(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        receipt: &WorkflowReceipt,
    ) -> Result<()> {
        let client = self.require_client()?;
        let comment = format_receipt_comment(receipt);

        client
            .comment_on_pr(owner, repo, pr_number, &comment)
            .await?;

        info!("Posted receipt comment on PR #{}", pr_number);

        Ok(())
    }

    fn enqueue_branch_export(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
        branch_name: String,
        base_sha: String,
    ) -> GitHubExportEnqueueResult {
        let idempotency_key = format!(
            "branch:{}:{}:{}:{}:{}",
            owner, repo, issue_number, branch_name, base_sha
        );
        self.enqueue_intent(
            owner,
            repo,
            issue_number,
            GitHubExportKind::CreateBranch,
            idempotency_key,
            GitHubExportPayload::CreateBranch {
                branch_name,
                base_sha,
            },
        )
    }

    fn enqueue_pr_export(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
        branch_name: String,
        base_branch: String,
        title: String,
        body: String,
    ) -> GitHubExportEnqueueResult {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        title.hash(&mut hasher);
        body.hash(&mut hasher);
        let payload_hash = hasher.finish();

        let idempotency_key = format!(
            "pr:{}:{}:{}:{}:{}:{:016x}",
            owner, repo, issue_number, branch_name, base_branch, payload_hash
        );
        self.enqueue_intent(
            owner,
            repo,
            issue_number,
            GitHubExportKind::CreatePullRequest,
            idempotency_key,
            GitHubExportPayload::CreatePullRequest {
                branch_name,
                base_branch,
                title,
                body,
            },
        )
    }

    fn enqueue_intent(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
        kind: GitHubExportKind,
        idempotency_key: String,
        payload: GitHubExportPayload,
    ) -> GitHubExportEnqueueResult {
        let mut queue = self
            .export_queue
            .lock()
            .expect("export queue mutex poisoned");

        if let Some(existing) = queue
            .intents
            .iter()
            .find(|intent| intent.summary.idempotency_key == idempotency_key)
        {
            return GitHubExportEnqueueResult {
                intent_id: existing.summary.intent_id,
                idempotency_key: existing.summary.idempotency_key.clone(),
                status: existing.summary.status,
                deduplicated: true,
            };
        }

        queue.next_intent_id = queue.next_intent_id.saturating_add(1);
        let intent_id = queue.next_intent_id;
        let summary = GitHubExportIntent {
            intent_id,
            idempotency_key: idempotency_key.clone(),
            kind,
            owner: owner.to_string(),
            repo: repo.to_string(),
            issue_number,
            status: GitHubExportStatus::Queued,
            attempts: 0,
            external_id: None,
            last_error: None,
        };

        queue
            .intents
            .push_back(QueuedGitHubExportIntent { summary, payload });

        GitHubExportEnqueueResult {
            intent_id,
            idempotency_key,
            status: GitHubExportStatus::Queued,
            deduplicated: false,
        }
    }

    fn mark_next_intent_dispatching(&self) -> Option<QueuedGitHubExportIntent> {
        let mut queue = self
            .export_queue
            .lock()
            .expect("export queue mutex poisoned");
        let intent = queue
            .intents
            .iter_mut()
            .find(|intent| {
                matches!(
                    intent.summary.status,
                    GitHubExportStatus::Queued | GitHubExportStatus::Failed
                )
            })?
            .clone();

        if let Some(entry) = queue
            .intents
            .iter_mut()
            .find(|entry| entry.summary.intent_id == intent.summary.intent_id)
        {
            entry.summary.status = GitHubExportStatus::Dispatching;
            entry.summary.attempts = entry.summary.attempts.saturating_add(1);
            entry.summary.last_error = None;
            return Some(entry.clone());
        }

        None
    }

    fn complete_intent(&self, intent_id: u64, external_id: Option<u64>) -> GitHubExportIntent {
        let mut queue = self
            .export_queue
            .lock()
            .expect("export queue mutex poisoned");
        let intent = queue
            .intents
            .iter_mut()
            .find(|intent| intent.summary.intent_id == intent_id)
            .expect("intent must exist when completing");
        intent.summary.status = GitHubExportStatus::Completed;
        intent.summary.external_id = external_id;
        intent.summary.last_error = None;
        intent.summary.clone()
    }

    fn fail_intent(&self, intent_id: u64, error: String) -> GitHubExportIntent {
        let mut queue = self
            .export_queue
            .lock()
            .expect("export queue mutex poisoned");
        let intent = queue
            .intents
            .iter_mut()
            .find(|intent| intent.summary.intent_id == intent_id)
            .expect("intent must exist when failing");
        intent.summary.status = GitHubExportStatus::Failed;
        intent.summary.last_error = Some(error);
        intent.summary.clone()
    }

    fn require_client(&self) -> Result<&GitHubClient> {
        self.client
            .as_ref()
            .ok_or_else(|| anyhow!("GitHub integration client unavailable"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitHubExportKind {
    CreateBranch,
    CreatePullRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitHubExportStatus {
    Queued,
    Dispatching,
    Completed,
    Failed,
}

#[derive(Debug, Clone)]
pub struct GitHubExportIntent {
    pub intent_id: u64,
    pub idempotency_key: String,
    pub kind: GitHubExportKind,
    pub owner: String,
    pub repo: String,
    pub issue_number: u64,
    pub status: GitHubExportStatus,
    pub attempts: u32,
    pub external_id: Option<u64>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GitHubExportEnqueueResult {
    pub intent_id: u64,
    pub idempotency_key: String,
    pub status: GitHubExportStatus,
    pub deduplicated: bool,
}

#[derive(Debug, Clone)]
pub struct GitHubExportDispatchResult {
    pub intent_id: u64,
    pub kind: GitHubExportKind,
    pub status: GitHubExportStatus,
    pub attempts: u32,
    pub external_id: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
struct GitHubExportExecutionResult {
    external_id: Option<u64>,
}

#[derive(Debug, Clone)]
enum GitHubExportPayload {
    CreateBranch {
        branch_name: String,
        base_sha: String,
    },
    CreatePullRequest {
        branch_name: String,
        base_branch: String,
        title: String,
        body: String,
    },
}

#[derive(Debug, Clone)]
struct QueuedGitHubExportIntent {
    summary: GitHubExportIntent,
    payload: GitHubExportPayload,
}

#[derive(Debug, Default)]
struct GitHubExportQueue {
    next_intent_id: u64,
    intents: VecDeque<QueuedGitHubExportIntent>,
}

/// Result of initializing an issue workflow.
#[derive(Debug, Clone)]
pub struct IssueWorkflowResult {
    pub branch_name: String,
    pub issue_number: u64,
    pub branch_export_intent_id: u64,
}

/// Execution receipt for transparency.
#[derive(Debug, Clone)]
pub struct WorkflowReceipt {
    pub model: String,
    pub duration_seconds: u64,
    pub tokens_input: u64,
    pub tokens_output: u64,
    pub cost_usd: f64,
    pub files_changed: usize,
    pub tests_run: Option<usize>,
    pub tests_passed: Option<usize>,
    pub ci_status: Option<String>,
    pub replay_url: Option<String>,
}

fn format_receipt_comment(receipt: &WorkflowReceipt) -> String {
    let mut comment = String::from("## 🤖 Autopilot Receipt\n\n");

    comment.push_str(&format!("**Model**: {}\n", receipt.model));
    comment.push_str(&format!(
        "**Duration**: {} seconds\n",
        receipt.duration_seconds
    ));
    comment.push_str(&format!(
        "**Tokens**: {} in / {} out\n",
        receipt.tokens_input, receipt.tokens_output
    ));
    comment.push_str(&format!("**Cost**: ${:.4}\n", receipt.cost_usd));
    comment.push_str(&format!("**Files Changed**: {}\n", receipt.files_changed));

    if let (Some(run), Some(passed)) = (receipt.tests_run, receipt.tests_passed) {
        comment.push_str(&format!("**Tests**: {} / {} passed\n", passed, run));
    }

    if let Some(ref status) = receipt.ci_status {
        comment.push_str(&format!("**CI Status**: {}\n", status));
    }

    if let Some(ref url) = receipt.replay_url {
        comment.push_str(&format!("\n[View Full Replay]({})\n", url));
    }

    comment.push_str("\n---\n");
    comment.push_str("_Generated by [OpenAgents Autopilot](https://openagents.com)_");

    comment
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_receipt() {
        let receipt = WorkflowReceipt {
            model: "codex-sonnet-4-5-20250929".to_string(),
            duration_seconds: 480,
            tokens_input: 150000,
            tokens_output: 8000,
            cost_usd: 0.42,
            files_changed: 3,
            tests_run: Some(42),
            tests_passed: Some(42),
            ci_status: Some("passed".to_string()),
            replay_url: Some("https://openagents.com/replays/abc123".to_string()),
        };

        let comment = format_receipt_comment(&receipt);
        assert!(comment.contains("codex-sonnet-4-5-20250929"));
        assert!(comment.contains("480 seconds"));
        assert!(comment.contains("$0.4200"));
        assert!(comment.contains("42 / 42 passed"));
    }

    #[tokio::test]
    async fn create_pr_queues_deferred_export_and_deduplicates() {
        let workflow = GitHubWorkflow::without_client("autopilot-agent".to_string());

        let first = workflow
            .create_pr(
                "openagents",
                "autopilot",
                42,
                "autopilot/42-fix-login",
                "main",
                "Fix login",
                "Body",
            )
            .await
            .expect("enqueue should succeed");
        let second = workflow
            .create_pr(
                "openagents",
                "autopilot",
                42,
                "autopilot/42-fix-login",
                "main",
                "Fix login",
                "Body",
            )
            .await
            .expect("dedupe enqueue should succeed");

        assert_eq!(first.intent_id, second.intent_id);
        assert!(!first.deduplicated);
        assert!(second.deduplicated);

        let snapshot = workflow.export_queue_snapshot();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].kind, GitHubExportKind::CreatePullRequest);
        assert_eq!(snapshot[0].status, GitHubExportStatus::Queued);
        assert_eq!(snapshot[0].attempts, 0);
    }

    #[tokio::test]
    async fn dispatch_next_export_returns_none_when_queue_is_empty() {
        let workflow = GitHubWorkflow::without_client("autopilot-agent".to_string());

        let dispatched = workflow
            .dispatch_next_export()
            .await
            .expect("dispatch should succeed");
        assert!(dispatched.is_none());
    }
}
