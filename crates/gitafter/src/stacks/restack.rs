//! Restack operation for stacked diffs
//!
//! Handles rebasing all layers of a stack when the base changes.
//!
//! # Examples
//!
//! ```ignore
//! use gitafter::stacks::restack::restack_layers;
//! use std::path::Path;
//! use std::sync::Arc;
//!
//! async fn example(nostr_client: Arc<NostrClient>, identity: Arc<UnifiedIdentity>) -> anyhow::Result<()> {
//!     let pr_events = vec![/* PR events from Nostr */];
//!
//!     // Restack all layers after base branch update
//!     let result = restack_layers(
//!         Path::new("./repo"),
//!         "stack-uuid-123",
//!         &pr_events,
//!         nostr_client,
//!         identity,
//!         "30617:pubkey:repo-id",
//!     ).await?;
//!
//!     println!("Successfully restacked {} layers", result.succeeded.len());
//!     if !result.failed.is_empty() {
//!         println!("Failed to restack {} layers", result.failed.len());
//!     }
//!     Ok(())
//! }
//! ```

use anyhow::{Result, anyhow};
use nostr::Event;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::git::{abort_rebase, has_rebase_conflicts, rebase_commit};
use crate::nostr::NostrClient;
use crate::nostr::events::PullRequestBuilder;
use crate::stacks::graph::{LayerInfo, StackGraph};
use wallet::core::identity::UnifiedIdentity;

/// Result of a restack operation
///
/// # Examples
///
/// ```
/// use gitafter::stacks::restack::RestackResult;
/// use std::collections::HashMap;
///
/// let result = RestackResult {
///     updated_prs: HashMap::new(),
///     succeeded: vec!["pr1".to_string(), "pr2".to_string()],
///     failed: vec![],
/// };
///
/// assert_eq!(result.succeeded.len(), 2);
/// ```
pub struct RestackResult {
    /// Map of old PR event ID to new PR event ID
    pub updated_prs: HashMap<String, String>,
    /// Layers that were successfully restacked
    pub succeeded: Vec<String>,
    /// Layers that failed to restack
    pub failed: Vec<(String, String)>, // (pr_id, error_message)
}

/// Restack all layers in a stack onto a new base
///
/// This operation:
/// 1. Gets all PRs in the stack and sorts them by dependency order
/// 2. Rebases each layer's commit onto its new parent
/// 3. Publishes PR Update events (kind:1619) with new commit IDs
///
/// # Arguments
/// * `repo_path` - Path to the local git repository
/// * `stack_id` - The stack UUID to restack
/// * `pr_events` - All PR events in the stack
/// * `nostr_client` - Nostr client for publishing updates
/// * `identity` - Identity for signing events
/// * `repo_address` - Repository address tag (e.g., "30617:pubkey:repo-id")
///
/// # Examples
///
/// ```ignore
/// use gitafter::stacks::restack::restack_layers;
/// use std::path::Path;
/// use std::sync::Arc;
///
/// async fn example(nostr_client: Arc<NostrClient>, identity: Arc<UnifiedIdentity>) -> anyhow::Result<()> {
///     let pr_events = vec![/* PR events */];
///
///     let result = restack_layers(
///         Path::new("./my-repo"),
///         "stack-abc123",
///         &pr_events,
///         nostr_client,
///         identity,
///         "30617:pubkey:repo-id",
///     ).await?;
///
///     for pr_id in &result.succeeded {
///         println!("Restacked PR: {}", pr_id);
///     }
///     Ok(())
/// }
/// ```
pub async fn restack_layers(
    repo_path: &Path,
    stack_id: &str,
    pr_events: &[Event],
    nostr_client: Arc<NostrClient>,
    identity: Arc<UnifiedIdentity>,
    repo_address: &str,
) -> Result<RestackResult> {
    // Build stack graph
    let graph = StackGraph::from_pr_events(pr_events)?;

    // Validate stack structure
    graph.validate()?;

    // Get layers in dependency order (base to top)
    let layers = graph.topological_sort()?;

    // Filter to only layers in this stack
    let stack_layers: Vec<_> = layers
        .into_iter()
        .filter(|l| l.stack_id == stack_id)
        .collect();

    if stack_layers.is_empty() {
        return Err(anyhow!("No layers found for stack {}", stack_id));
    }

    let mut result = RestackResult {
        updated_prs: HashMap::new(),
        succeeded: Vec::new(),
        failed: Vec::new(),
    };

    // Track new commit IDs for each layer
    let mut new_commits: HashMap<String, String> = HashMap::new();

    // Rebase each layer in order
    for (idx, layer) in stack_layers.iter().enumerate() {
        match restack_layer(
            repo_path,
            layer,
            idx,
            &new_commits,
            &stack_layers,
            &nostr_client,
            &identity,
            repo_address,
        )
        .await
        {
            Ok((old_commit, new_commit, new_pr_event_id)) => {
                new_commits.insert(layer.event_id.clone(), new_commit.clone());
                result
                    .updated_prs
                    .insert(layer.event_id.clone(), new_pr_event_id.clone());
                result.succeeded.push(layer.event_id.clone());

                tracing::info!(
                    "Successfully restacked layer {}/{}: {} -> {}",
                    layer.layer_number,
                    layer.total_layers,
                    &old_commit[..8],
                    &new_commit[..8]
                );
            }
            Err(e) => {
                tracing::error!(
                    "Failed to restack layer {}/{}: {}",
                    layer.layer_number,
                    layer.total_layers,
                    e
                );

                result.failed.push((layer.event_id.clone(), e.to_string()));

                // Try to abort any in-progress rebase
                if let Err(abort_err) = abort_rebase(repo_path) {
                    tracing::warn!("Failed to abort rebase: {}", abort_err);
                }

                // Stop restacking remaining layers since they depend on this one
                break;
            }
        }
    }

    Ok(result)
}

/// Restack a single layer
///
/// Returns (old_commit_id, new_commit_id, new_pr_event_id)
async fn restack_layer(
    repo_path: &Path,
    layer: &LayerInfo,
    layer_idx: usize,
    new_commits: &HashMap<String, String>,
    all_layers: &[LayerInfo],
    nostr_client: &NostrClient,
    identity: &UnifiedIdentity,
    repo_address: &str,
) -> Result<(String, String, String)> {
    let old_commit = layer
        .commit_id
        .as_ref()
        .ok_or_else(|| anyhow!("Layer {} missing commit ID", layer.event_id))?;

    // Determine the new base commit
    let new_base_commit = if layer_idx == 0 {
        // First layer - rebase onto the original base branch
        // For now, we'll assume this is "main" or the commit's parent
        // In a full implementation, we'd need to pass the base branch/commit
        return Err(anyhow!("Base layer rebase not yet implemented"));
    } else {
        // Not the first layer - rebase onto the previous layer's new commit
        let prev_layer = &all_layers[layer_idx - 1];
        new_commits
            .get(&prev_layer.event_id)
            .ok_or_else(|| anyhow!("Previous layer not yet rebased"))?
            .clone()
    };

    // Perform the rebase
    let new_commit_id = rebase_commit(repo_path, old_commit, &new_base_commit)?;

    // Check for conflicts
    if has_rebase_conflicts(repo_path)? {
        return Err(anyhow!("Rebase conflicts detected"));
    }

    // Publish PR Update event (kind:1619) with new commit ID
    let pr_update_event_id =
        publish_pr_update(layer, &new_commit_id, nostr_client, identity, repo_address).await?;

    Ok((old_commit.clone(), new_commit_id, pr_update_event_id))
}

/// Publish a PR Update event (kind:1619) after restacking
async fn publish_pr_update(
    layer: &LayerInfo,
    new_commit_id: &str,
    nostr_client: &NostrClient,
    identity: &UnifiedIdentity,
    repo_address: &str,
) -> Result<String> {
    // Build PR Update event
    // kind:1619 events reference the original PR and include updated information
    let update_content = format!(
        "Restacked layer {}/{}: updated commit to {}",
        layer.layer_number,
        layer.total_layers,
        &new_commit_id[..8]
    );

    // For PR updates, we use a standard text note (kind:1) with special tags
    // or we could define a custom event structure
    // For now, we'll use PullRequestBuilder to create an updated PR event
    let pr_update = PullRequestBuilder::new(
        repo_address,
        &format!(
            "Layer {}/{} (restacked)",
            layer.layer_number, layer.total_layers
        ),
        &update_content,
    )
    .commit(new_commit_id)
    .stack(&layer.stack_id)
    .layer(layer.layer_number, layer.total_layers);

    // Add dependency if not base layer
    let pr_update = if let Some(dep) = &layer.depends_on {
        pr_update.depends_on(dep)
    } else {
        pr_update
    };

    let event_template = pr_update.build();

    // Sign the event
    let signed_event = identity
        .sign_event(event_template)
        .map_err(|e| anyhow!("Failed to sign PR update event: {}", e))?;

    let event_id = signed_event.id.clone();

    // Publish to relays
    nostr_client.publish_event(signed_event).await?;

    Ok(event_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Full integration tests would require:
    // - A real git repository
    // - Mock Nostr client
    // - Mock identity
    // These tests are placeholders for the structure

    #[test]
    fn test_restack_result_creation() {
        let result = RestackResult {
            updated_prs: HashMap::new(),
            succeeded: Vec::new(),
            failed: Vec::new(),
        };

        assert_eq!(result.succeeded.len(), 0);
        assert_eq!(result.failed.len(), 0);
    }
}
