use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::{info, warn};

use crate::github::{GitHubClient, branch_name_for_issue};
use crate::paths::openagents_home;

/// GitHub workflow orchestrator for autopilot.
///
/// Core issue progression is decoupled from branch/PR side effects:
/// branch/PR mutations are enqueued as deferred integration exports.
pub struct GitHubWorkflow {
    client: Option<GitHubClient>,
    agent_identity: String,
    export_queue: Arc<Mutex<GitHubExportQueue>>,
    queue_store: GitHubExportQueueStore,
}

impl GitHubWorkflow {
    /// Create a new GitHub workflow with authentication.
    pub fn new(token: &str, agent_identity: String) -> Result<Self> {
        let client = GitHubClient::new(token)?;
        Ok(Self::new_with_store(
            Some(client),
            agent_identity,
            GitHubExportQueueStore::default(),
        ))
    }

    /// Create a workflow that can queue exports without an active GitHub client.
    pub fn without_client(agent_identity: String) -> Self {
        Self::new_with_store(None, agent_identity, GitHubExportQueueStore::default())
    }

    fn new_with_store(
        client: Option<GitHubClient>,
        agent_identity: String,
        queue_store: GitHubExportQueueStore,
    ) -> Self {
        let loaded_queue = match queue_store.load() {
            Ok(queue) => queue,
            Err(error) => {
                warn!(
                    "Failed to load persisted GitHub export queue from {}: {}",
                    queue_store.path.display(),
                    error
                );
                GitHubExportQueue::default()
            }
        };

        Self {
            client,
            agent_identity,
            export_queue: Arc::new(Mutex::new(loaded_queue)),
            queue_store,
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
        let branch_export_intent_id = match enqueue {
            Ok(result) => {
                info!(
                    "Queued branch export intent #{} for issue #{}",
                    result.intent_id, issue_number
                );
                Some(result.intent_id)
            }
            Err(error) => {
                warn!(
                    "Issue #{} core workflow progressed but branch export queueing failed: {}",
                    issue_number, error
                );
                None
            }
        };

        info!(
            "GitHub workflow initialized for issue #{} (branch export intent: {:?})",
            issue_number, branch_export_intent_id
        );

        Ok(IssueWorkflowResult {
            branch_name,
            issue_number,
            branch_export_intent_id,
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
        )?;

        info!(
            "Queued PR export intent #{} for issue #{} in {}/{}",
            enqueue.intent_id, issue_number, owner, repo
        );

        Ok(enqueue)
    }

    /// Dispatch one queued/failed export intent.
    pub async fn dispatch_next_export(&self) -> Result<Option<GitHubExportDispatchResult>> {
        let has_dispatchable = self.has_dispatchable_intent();
        if !has_dispatchable {
            return Ok(None);
        }
        let client = self.require_client()?;
        let Some(candidate) = self.mark_next_intent_dispatching()? else {
            return Ok(None);
        };

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
                    self.complete_intent(candidate.summary.intent_id, result.external_id)?;
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
                let duplicate = is_idempotent_duplicate_error(candidate.summary.kind, &error_text);
                let snapshot = if duplicate {
                    warn!(
                        "Export intent #{} observed duplicate adapter side effect; treating as completed: {}",
                        candidate.summary.intent_id, error_text
                    );
                    self.complete_intent(candidate.summary.intent_id, None)?
                } else {
                    self.fail_intent(candidate.summary.intent_id, error_text.clone())?
                };
                Ok(Some(GitHubExportDispatchResult {
                    intent_id: snapshot.intent_id,
                    kind: snapshot.kind,
                    status: snapshot.status,
                    attempts: snapshot.attempts,
                    external_id: snapshot.external_id,
                    error: if duplicate { None } else { Some(error_text) },
                }))
            }
        }
    }

    /// Retry a failed export intent by returning it to queued state.
    pub fn retry_export(&self, intent_id: u64) -> Result<bool> {
        let mut queue = self
            .export_queue
            .lock()
            .expect("export queue mutex poisoned");
        let updated = if let Some(intent) = queue
            .intents
            .iter_mut()
            .find(|intent| intent.summary.intent_id == intent_id)
        {
            if intent.summary.status == GitHubExportStatus::Failed {
                intent.summary.status = GitHubExportStatus::Queued;
                intent.summary.last_error = None;
                queue.checkpoint.last_retried_intent_id = Some(intent_id);
                true
            } else {
                false
            }
        } else {
            false
        };
        if updated {
            self.persist_locked_queue(&queue)?;
        }
        Ok(updated)
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

    /// Observable checkpoint/watermark state for adapter replay.
    pub fn export_checkpoint_snapshot(&self) -> GitHubExportCheckpointSnapshot {
        let queue = self
            .export_queue
            .lock()
            .expect("export queue mutex poisoned");
        GitHubExportCheckpointSnapshot {
            last_enqueued_intent_id: queue.checkpoint.last_enqueued_intent_id,
            last_dispatched_intent_id: queue.checkpoint.last_dispatched_intent_id,
            last_completed_intent_id: queue.checkpoint.last_completed_intent_id,
            last_failed_intent_id: queue.checkpoint.last_failed_intent_id,
            last_retried_intent_id: queue.checkpoint.last_retried_intent_id,
        }
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
    ) -> Result<GitHubExportEnqueueResult> {
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
    ) -> Result<GitHubExportEnqueueResult> {
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
    ) -> Result<GitHubExportEnqueueResult> {
        let mut queue = self
            .export_queue
            .lock()
            .expect("export queue mutex poisoned");

        if let Some(existing) = queue
            .intents
            .iter()
            .find(|intent| intent.summary.idempotency_key == idempotency_key)
        {
            return Ok(GitHubExportEnqueueResult {
                intent_id: existing.summary.intent_id,
                idempotency_key: existing.summary.idempotency_key.clone(),
                status: existing.summary.status,
                deduplicated: true,
            });
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
        queue.checkpoint.last_enqueued_intent_id = Some(intent_id);
        self.persist_locked_queue(&queue)?;

        Ok(GitHubExportEnqueueResult {
            intent_id,
            idempotency_key,
            status: GitHubExportStatus::Queued,
            deduplicated: false,
        })
    }

    fn has_dispatchable_intent(&self) -> bool {
        let queue = self
            .export_queue
            .lock()
            .expect("export queue mutex poisoned");
        queue.intents.iter().any(|intent| {
            matches!(
                intent.summary.status,
                GitHubExportStatus::Queued | GitHubExportStatus::Failed
            )
        })
    }

    fn mark_next_intent_dispatching(&self) -> Result<Option<QueuedGitHubExportIntent>> {
        let mut queue = self
            .export_queue
            .lock()
            .expect("export queue mutex poisoned");
        let Some(intent) = queue
            .intents
            .iter()
            .find(|intent| {
                matches!(
                    intent.summary.status,
                    GitHubExportStatus::Queued | GitHubExportStatus::Failed
                )
            })
            .cloned()
        else {
            return Ok(None);
        };

        if let Some(entry) = queue
            .intents
            .iter_mut()
            .find(|entry| entry.summary.intent_id == intent.summary.intent_id)
        {
            entry.summary.status = GitHubExportStatus::Dispatching;
            entry.summary.attempts = entry.summary.attempts.saturating_add(1);
            entry.summary.last_error = None;
            let snapshot = entry.clone();
            let dispatched_id = entry.summary.intent_id;
            queue.checkpoint.last_dispatched_intent_id = Some(dispatched_id);
            self.persist_locked_queue(&queue)?;
            return Ok(Some(snapshot));
        }

        Ok(None)
    }

    fn complete_intent(
        &self,
        intent_id: u64,
        external_id: Option<u64>,
    ) -> Result<GitHubExportIntent> {
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
        let snapshot = intent.summary.clone();
        queue.checkpoint.last_completed_intent_id = Some(intent_id);
        self.persist_locked_queue(&queue)?;
        Ok(snapshot)
    }

    fn fail_intent(&self, intent_id: u64, error: String) -> Result<GitHubExportIntent> {
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
        let snapshot = intent.summary.clone();
        queue.checkpoint.last_failed_intent_id = Some(intent_id);
        self.persist_locked_queue(&queue)?;
        Ok(snapshot)
    }

    fn persist_locked_queue(&self, queue: &GitHubExportQueue) -> Result<()> {
        self.queue_store.save(queue)
    }

    fn require_client(&self) -> Result<&GitHubClient> {
        self.client
            .as_ref()
            .ok_or_else(|| anyhow!("GitHub integration client unavailable"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitHubExportKind {
    CreateBranch,
    CreatePullRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitHubExportStatus {
    Queued,
    Dispatching,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GitHubExportCheckpointSnapshot {
    pub last_enqueued_intent_id: Option<u64>,
    pub last_dispatched_intent_id: Option<u64>,
    pub last_completed_intent_id: Option<u64>,
    pub last_failed_intent_id: Option<u64>,
    pub last_retried_intent_id: Option<u64>,
}

#[derive(Debug, Clone)]
struct GitHubExportExecutionResult {
    external_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct QueuedGitHubExportIntent {
    summary: GitHubExportIntent,
    payload: GitHubExportPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct GitHubExportQueueCheckpoint {
    #[serde(default)]
    last_enqueued_intent_id: Option<u64>,
    #[serde(default)]
    last_dispatched_intent_id: Option<u64>,
    #[serde(default)]
    last_completed_intent_id: Option<u64>,
    #[serde(default)]
    last_failed_intent_id: Option<u64>,
    #[serde(default)]
    last_retried_intent_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GitHubExportQueue {
    #[serde(default = "default_export_queue_version")]
    version: u32,
    next_intent_id: u64,
    #[serde(default)]
    checkpoint: GitHubExportQueueCheckpoint,
    #[serde(default)]
    intents: VecDeque<QueuedGitHubExportIntent>,
}

impl Default for GitHubExportQueue {
    fn default() -> Self {
        Self {
            version: default_export_queue_version(),
            next_intent_id: 0,
            checkpoint: GitHubExportQueueCheckpoint::default(),
            intents: VecDeque::new(),
        }
    }
}

/// Result of initializing an issue workflow.
#[derive(Debug, Clone)]
pub struct IssueWorkflowResult {
    pub branch_name: String,
    pub issue_number: u64,
    pub branch_export_intent_id: Option<u64>,
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

const EXPORT_QUEUE_VERSION: u32 = 1;

fn default_export_queue_version() -> u32 {
    EXPORT_QUEUE_VERSION
}

fn is_idempotent_duplicate_error(kind: GitHubExportKind, error: &str) -> bool {
    let normalized = error.to_ascii_lowercase();
    let contains_duplicate = normalized.contains("already exists")
        || normalized.contains("reference already exists")
        || normalized.contains("name already exists");
    if !contains_duplicate {
        return false;
    }
    match kind {
        GitHubExportKind::CreateBranch => true,
        GitHubExportKind::CreatePullRequest => {
            normalized.contains("pull request") || normalized.contains("a pull request")
        }
    }
}

#[derive(Debug, Clone)]
struct GitHubExportQueueStore {
    path: PathBuf,
}

impl Default for GitHubExportQueueStore {
    fn default() -> Self {
        Self {
            path: openagents_home()
                .join("workflow")
                .join("github-export-queue.json"),
        }
    }
}

impl GitHubExportQueueStore {
    #[cfg(test)]
    fn with_path(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    fn load(&self) -> Result<GitHubExportQueue> {
        if !self.path.exists() {
            return Ok(GitHubExportQueue::default());
        }
        let raw = fs::read_to_string(&self.path).with_context(|| {
            format!(
                "failed to read GitHub export queue at {}",
                self.path.display()
            )
        })?;
        let queue = serde_json::from_str::<GitHubExportQueue>(&raw).with_context(|| {
            format!(
                "failed to parse GitHub export queue at {}",
                self.path.display()
            )
        })?;
        Ok(normalize_loaded_queue(queue))
    }

    fn save(&self, queue: &GitHubExportQueue) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create GitHub export queue directory {}",
                    parent.display()
                )
            })?;
        }
        let mut persisted = queue.clone();
        persisted.version = default_export_queue_version();
        let json = serde_json::to_string_pretty(&persisted)
            .context("failed to serialize GitHub export queue")?;

        let tmp_path = temporary_queue_path(&self.path);
        fs::write(&tmp_path, json).with_context(|| {
            format!(
                "failed to write temporary GitHub export queue {}",
                tmp_path.display()
            )
        })?;
        fs::rename(&tmp_path, &self.path).with_context(|| {
            format!(
                "failed to commit GitHub export queue {}",
                self.path.display()
            )
        })?;

        Ok(())
    }
}

fn temporary_queue_path(path: &Path) -> PathBuf {
    let mut tmp = path.to_path_buf();
    let extension = path
        .extension()
        .map(|ext| format!("{}.tmp", ext.to_string_lossy()))
        .unwrap_or_else(|| "tmp".to_string());
    tmp.set_extension(extension);
    tmp
}

fn normalize_loaded_queue(mut queue: GitHubExportQueue) -> GitHubExportQueue {
    if queue.version == 0 {
        queue.version = default_export_queue_version();
    }

    for intent in queue.intents.iter_mut() {
        if intent.summary.status == GitHubExportStatus::Dispatching {
            intent.summary.status = GitHubExportStatus::Failed;
            if intent.summary.last_error.is_none() {
                intent.summary.last_error =
                    Some("Recovered interrupted dispatch; safe to retry.".to_string());
            }
        }
    }

    if let Some(max_id) = queue
        .intents
        .iter()
        .map(|intent| intent.summary.intent_id)
        .max()
    {
        queue.next_intent_id = queue.next_intent_id.max(max_id);
    }
    queue
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn workflow_without_client_for_tests() -> (GitHubWorkflow, TempDir) {
        let tmp = tempfile::tempdir().expect("tempdir should be created");
        let queue_path = tmp.path().join("github-export-queue.json");
        let workflow = GitHubWorkflow::new_with_store(
            None,
            "autopilot-agent".to_string(),
            GitHubExportQueueStore::with_path(queue_path),
        );
        (workflow, tmp)
    }

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
        let (workflow, _tmp) = workflow_without_client_for_tests();

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
        let checkpoint = workflow.export_checkpoint_snapshot();
        assert_eq!(checkpoint.last_enqueued_intent_id, Some(first.intent_id));
    }

    #[tokio::test]
    async fn dispatch_next_export_returns_none_when_queue_is_empty() {
        let (workflow, _tmp) = workflow_without_client_for_tests();

        let dispatched = workflow
            .dispatch_next_export()
            .await
            .expect("dispatch should succeed");
        assert!(dispatched.is_none());
    }

    #[tokio::test]
    async fn export_intents_are_persisted_and_reloaded() {
        let tmp = tempfile::tempdir().expect("tempdir should be created");
        let queue_path = tmp.path().join("github-export-queue.json");

        let workflow_one = GitHubWorkflow::new_with_store(
            None,
            "autopilot-agent".to_string(),
            GitHubExportQueueStore::with_path(queue_path.clone()),
        );
        let first = workflow_one
            .create_pr(
                "openagents",
                "autopilot",
                77,
                "autopilot/77-feature",
                "main",
                "Feature",
                "Body",
            )
            .await
            .expect("enqueue should succeed");
        drop(workflow_one);

        let workflow_two = GitHubWorkflow::new_with_store(
            None,
            "autopilot-agent".to_string(),
            GitHubExportQueueStore::with_path(queue_path),
        );
        let snapshot = workflow_two.export_queue_snapshot();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].intent_id, first.intent_id);
        assert_eq!(snapshot[0].status, GitHubExportStatus::Queued);
    }

    #[test]
    fn loading_queue_recovers_dispatching_intents() {
        let tmp = tempfile::tempdir().expect("tempdir should be created");
        let queue_path = tmp.path().join("github-export-queue.json");

        let mut intents = VecDeque::new();
        intents.push_back(QueuedGitHubExportIntent {
            summary: GitHubExportIntent {
                intent_id: 1,
                idempotency_key: "branch:openagents:autopilot:1:autopilot/1:abc".to_string(),
                kind: GitHubExportKind::CreateBranch,
                owner: "openagents".to_string(),
                repo: "autopilot".to_string(),
                issue_number: 1,
                status: GitHubExportStatus::Dispatching,
                attempts: 1,
                external_id: None,
                last_error: None,
            },
            payload: GitHubExportPayload::CreateBranch {
                branch_name: "autopilot/1".to_string(),
                base_sha: "abc".to_string(),
            },
        });
        let queue = GitHubExportQueue {
            version: 1,
            next_intent_id: 1,
            checkpoint: GitHubExportQueueCheckpoint::default(),
            intents,
        };
        std::fs::write(
            &queue_path,
            serde_json::to_string_pretty(&queue).expect("serialize queue"),
        )
        .expect("write queue");

        let workflow = GitHubWorkflow::new_with_store(
            None,
            "autopilot-agent".to_string(),
            GitHubExportQueueStore::with_path(queue_path),
        );
        let snapshot = workflow.export_queue_snapshot();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].status, GitHubExportStatus::Failed);
        assert!(
            snapshot[0]
                .last_error
                .as_deref()
                .unwrap_or_default()
                .contains("Recovered interrupted dispatch")
        );
    }
}
