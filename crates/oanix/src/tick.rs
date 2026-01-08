//! OANIX tick loop - autonomous execution.
//!
//! The tick loop is the core of OANIX's autonomous operation.
//! It periodically wakes, assesses the situation, and takes action.

use crate::manifest::IssueSummary;
use crate::state::{OanixMode, OanixState};
use std::time::Duration;
use tracing::{debug, info, warn};

/// Result of a single tick.
#[derive(Debug)]
pub enum TickResult {
    /// Continue with work
    Continue(WorkItem),
    /// Perform a user-requested action
    UserAction(UserAction),
    /// Idle for a duration
    Idle(Duration),
    /// Shutdown requested
    Shutdown,
    /// Error occurred
    Error(String),
}

/// A unit of work to be performed.
#[derive(Debug, Clone)]
pub enum WorkItem {
    /// Work on an issue from the workspace
    Issue(IssueWork),
    /// Process a job from the swarm
    Job(JobWork),
    /// Continue existing task
    ContinueTask { task_id: String },
    /// Housekeeping (refresh manifest, sync state, etc.)
    Housekeeping(HousekeepingTask),
}

/// Issue work item.
#[derive(Debug, Clone)]
pub struct IssueWork {
    /// Issue number
    pub number: u32,
    /// Issue title
    pub title: String,
    /// Priority
    pub priority: String,
}

impl From<&IssueSummary> for IssueWork {
    fn from(issue: &IssueSummary) -> Self {
        Self {
            number: issue.number,
            title: issue.title.clone(),
            priority: issue.priority.clone(),
        }
    }
}

/// Job work from the swarm.
#[derive(Debug, Clone)]
pub struct JobWork {
    /// Job ID (from NIP-90)
    pub job_id: String,
    /// Job kind
    pub kind: u32,
    /// Query or task description
    pub query: String,
    /// Budget in sats
    pub budget_sats: Option<u64>,
}

/// Housekeeping task.
#[derive(Debug, Clone)]
pub enum HousekeepingTask {
    /// Refresh the environment manifest
    RefreshManifest,
    /// Sync state to disk
    SyncState,
    /// Check for updates
    CheckUpdates,
}

/// User-requested action.
#[derive(Debug, Clone)]
pub enum UserAction {
    /// Pause execution
    Pause,
    /// Resume execution
    Resume,
    /// Work on specific issue
    WorkOnIssue(u32),
    /// Start provider mode
    StartProvider,
    /// Stop provider mode
    StopProvider,
    /// Quit
    Quit,
}

/// Tick configuration.
#[derive(Debug, Clone)]
pub struct TickConfig {
    /// How often to refresh the manifest (seconds)
    pub manifest_refresh_interval: u64,
    /// How long to idle when nothing to do (seconds)
    pub idle_duration: u64,
    /// Whether to auto-pick issues
    pub auto_pick_issues: bool,
    /// Whether to accept swarm jobs
    pub accept_swarm_jobs: bool,
}

impl Default for TickConfig {
    fn default() -> Self {
        Self {
            manifest_refresh_interval: 60,
            idle_duration: 5,
            auto_pick_issues: false,
            accept_swarm_jobs: false,
        }
    }
}

/// Execute a single tick of the OANIX loop.
///
/// The tick loop follows this priority order:
/// 1. Check if paused -> return Idle
/// 2. Refresh stale manifest
/// 3. Check for user input (not implemented here, handled externally)
/// 4. Check for incoming jobs (if provider mode)
/// 5. Continue existing task
/// 6. Find next actionable issue (if auto-pick enabled)
/// 7. Housekeeping
/// 8. Idle
pub async fn oanix_tick(state: &mut OanixState, config: &TickConfig) -> TickResult {
    // 1. If paused, stay idle
    if state.is_paused() {
        debug!("Tick: paused, staying idle");
        return TickResult::Idle(Duration::from_secs(config.idle_duration));
    }

    // 2. Refresh manifest if stale
    if state.needs_refresh(config.manifest_refresh_interval) {
        info!("Tick: manifest refresh needed");
        if let Err(e) = state.refresh_manifest().await {
            warn!("Failed to refresh manifest: {}", e);
            // Continue with stale data rather than failing
        }
        return TickResult::Continue(WorkItem::Housekeeping(HousekeepingTask::RefreshManifest));
    }

    // 3. User input handled externally via channels

    // 4. Check for swarm jobs (if provider mode)
    if state.is_provider_mode() && config.accept_swarm_jobs {
        // TODO: Poll for NIP-90 jobs from relay
        // For now, this is a placeholder
        debug!("Tick: provider mode, would poll for jobs");
    }

    // 5. Continue existing task
    if let Some(task) = &state.active_task {
        debug!("Tick: continuing task {}", task.id);
        return TickResult::Continue(WorkItem::ContinueTask {
            task_id: task.id.clone(),
        });
    }

    // 6. Find next actionable issue
    if config.auto_pick_issues {
        if let Some(issue) = state.next_actionable_issue() {
            info!("Tick: found actionable issue #{}", issue.number);
            return TickResult::Continue(WorkItem::Issue(IssueWork::from(issue)));
        }
    }

    // 7. Periodic housekeeping
    // (sync state, check updates, etc.)

    // 8. Nothing to do, idle
    debug!("Tick: nothing to do, idling for {}s", config.idle_duration);
    TickResult::Idle(Duration::from_secs(config.idle_duration))
}

/// Run the OANIX tick loop.
///
/// This runs until shutdown is requested.
pub async fn run_tick_loop(
    mut state: OanixState,
    config: TickConfig,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
) -> anyhow::Result<()> {
    info!("Starting OANIX tick loop");

    loop {
        // Check for shutdown signal
        if *shutdown_rx.borrow() {
            info!("Shutdown signal received");
            state.save().await?;
            return Ok(());
        }

        // Execute tick
        let result = oanix_tick(&mut state, &config).await;

        match result {
            TickResult::Continue(work) => {
                info!("Tick result: work item {:?}", work);
                // TODO: Actually execute the work
                // For now, just log and continue
            }
            TickResult::UserAction(action) => {
                info!("Tick result: user action {:?}", action);
                match action {
                    UserAction::Pause => state.set_mode(OanixMode::Paused),
                    UserAction::Resume => state.set_mode(OanixMode::Idle),
                    UserAction::Quit => {
                        state.save().await?;
                        return Ok(());
                    }
                    UserAction::StartProvider => state.set_mode(OanixMode::Provider),
                    UserAction::StopProvider => state.set_mode(OanixMode::Idle),
                    UserAction::WorkOnIssue(num) => {
                        state.start_task(
                            format!("issue-{}", num),
                            format!("Issue #{}", num),
                            "issue".to_string(),
                        );
                    }
                }
            }
            TickResult::Idle(duration) => {
                debug!("Tick result: idle for {:?}", duration);
                tokio::select! {
                    _ = tokio::time::sleep(duration) => {}
                    _ = shutdown_rx.changed() => {
                        if *shutdown_rx.borrow() {
                            info!("Shutdown during idle");
                            state.save().await?;
                            return Ok(());
                        }
                    }
                }
            }
            TickResult::Shutdown => {
                info!("Tick result: shutdown");
                state.save().await?;
                return Ok(());
            }
            TickResult::Error(e) => {
                warn!("Tick error: {}", e);
                // Continue anyway, errors are not fatal
            }
        }

        // Save state periodically
        if let Err(e) = state.save().await {
            warn!("Failed to save state: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{
        ComputeManifest, HardwareManifest, IdentityManifest, NetworkManifest, OanixManifest,
    };
    use std::time::Instant;

    fn mock_manifest() -> OanixManifest {
        OanixManifest {
            hardware: HardwareManifest::unknown(),
            compute: ComputeManifest::empty(),
            network: NetworkManifest::offline(),
            identity: IdentityManifest::unknown(),
            workspace: None,
            discovered_at: Instant::now(),
        }
    }

    #[tokio::test]
    async fn test_tick_idle_when_paused() {
        let manifest = mock_manifest();
        let mut state = OanixState::new(manifest);
        state.set_mode(OanixMode::Paused);

        let config = TickConfig::default();
        let result = oanix_tick(&mut state, &config).await;

        assert!(matches!(result, TickResult::Idle(_)));
    }

    #[tokio::test]
    async fn test_tick_continues_existing_task() {
        let manifest = mock_manifest();
        let mut state = OanixState::new(manifest);
        state.start_task("task-1".into(), "Test task".into(), "test".into());

        let config = TickConfig::default();
        let result = oanix_tick(&mut state, &config).await;

        assert!(matches!(
            result,
            TickResult::Continue(WorkItem::ContinueTask { .. })
        ));
    }

    #[tokio::test]
    async fn test_tick_idles_when_nothing_to_do() {
        let manifest = mock_manifest();
        let mut state = OanixState::new(manifest);

        let config = TickConfig::default();
        let result = oanix_tick(&mut state, &config).await;

        assert!(matches!(result, TickResult::Idle(_)));
    }
}
