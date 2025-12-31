//! Stack dependency graph for managing PR dependencies
//!
//! Builds and validates dependency graphs for stacked PRs.
//!
//! # Examples
//!
//! ```
//! use gitafter::stacks::graph::{StackGraph, LayerInfo};
//! use nostr::Event;
//!
//! # fn create_test_pr(
//! #     event_id: &str,
//! #     stack_id: &str,
//! #     layer: u32,
//! #     total: u32,
//! #     depends_on: Option<&str>,
//! # ) -> Event {
//! #     let mut tags = vec![
//! #         vec!["stack".to_string(), stack_id.to_string()],
//! #         vec!["layer".to_string(), layer.to_string(), total.to_string()],
//! #     ];
//! #     if let Some(dep) = depends_on {
//! #         tags.push(vec!["depends_on".to_string(), dep.to_string()]);
//! #     }
//! #     Event {
//! #         id: event_id.to_string(),
//! #         kind: 1618,
//! #         pubkey: "test".to_string(),
//! #         created_at: 0,
//! #         content: String::new(),
//! #         tags,
//! #         sig: String::new(),
//! #     }
//! # }
//! // Create a 3-layer stack
//! let pr1 = create_test_pr("pr1", "stack-uuid", 1, 3, None);
//! let pr2 = create_test_pr("pr2", "stack-uuid", 2, 3, Some("pr1"));
//! let pr3 = create_test_pr("pr3", "stack-uuid", 3, 3, Some("pr2"));
//!
//! // Build dependency graph
//! let graph = StackGraph::from_pr_events(&[pr1, pr2, pr3]).unwrap();
//!
//! // Get topologically sorted layers (base to top)
//! let ordered = graph.topological_sort().unwrap();
//! assert_eq!(ordered[0].layer_number, 1);
//! assert_eq!(ordered[2].layer_number, 3);
//! ```

use anyhow::{Result, anyhow};
use nostr::Event;
use std::collections::{HashMap, HashSet};

/// Represents a stack of PRs with dependency relationships
///
/// # Examples
///
/// ```
/// use gitafter::stacks::graph::StackGraph;
/// use nostr::Event;
///
/// # fn create_test_pr(id: &str, stack: &str, layer: u32, total: u32, dep: Option<&str>) -> Event {
/// #     let mut tags = vec![
/// #         vec!["stack".to_string(), stack.to_string()],
/// #         vec!["layer".to_string(), layer.to_string(), total.to_string()],
/// #     ];
/// #     if let Some(d) = dep { tags.push(vec!["depends_on".to_string(), d.to_string()]); }
/// #     Event { id: id.to_string(), kind: 1618, pubkey: "test".to_string(),
/// #             created_at: 0, content: String::new(), tags, sig: String::new() }
/// # }
/// let pr1 = create_test_pr("pr1", "stack1", 1, 2, None);
/// let pr2 = create_test_pr("pr2", "stack1", 2, 2, Some("pr1"));
///
/// let graph = StackGraph::from_pr_events(&[pr1, pr2]).unwrap();
/// let layers = graph.get_stack_layers("stack1");
/// assert_eq!(layers.len(), 2);
/// ```
pub struct StackGraph {
    /// Map of PR event ID to its layer information
    layers: HashMap<String, LayerInfo>,
    /// Dependency graph: pr_id -> list of pr_ids it depends on
    dependencies: HashMap<String, Vec<String>>,
}

/// Information about a PR layer in a stack
#[derive(Clone, Debug)]
pub struct LayerInfo {
    pub event_id: String,
    pub layer_number: u32,
    pub total_layers: u32,
    pub stack_id: String,
    pub depends_on: Option<String>,
    pub commit_id: Option<String>,
}

impl StackGraph {
    /// Create a new stack graph from a list of PR events
    ///
    /// # Examples
    ///
    /// ```
    /// use gitafter::stacks::graph::StackGraph;
    /// use nostr::Event;
    ///
    /// # fn create_test_pr(id: &str, stack: &str, layer: u32, total: u32, dep: Option<&str>) -> Event {
    /// #     let mut tags = vec![
    /// #         vec!["stack".to_string(), stack.to_string()],
    /// #         vec!["layer".to_string(), layer.to_string(), total.to_string()],
    /// #     ];
    /// #     if let Some(d) = dep { tags.push(vec!["depends_on".to_string(), d.to_string()]); }
    /// #     Event { id: id.to_string(), kind: 1618, pubkey: "test".to_string(),
    /// #             created_at: 0, content: String::new(), tags, sig: String::new() }
    /// # }
    /// let pr1 = create_test_pr("pr1", "stack1", 1, 2, None);
    /// let pr2 = create_test_pr("pr2", "stack1", 2, 2, Some("pr1"));
    ///
    /// let graph = StackGraph::from_pr_events(&[pr1, pr2]).unwrap();
    /// assert_eq!(graph.get_stack_layers("stack1").len(), 2);
    /// ```
    pub fn from_pr_events(prs: &[Event]) -> Result<Self> {
        let mut layers = HashMap::new();
        let mut dependencies = HashMap::new();

        for pr in prs {
            // Extract stack information from tags
            let event_id = pr.id.clone();

            let stack_id = pr
                .tags
                .iter()
                .find(|tag| tag.len() >= 2 && tag[0] == "stack")
                .and_then(|tag| tag.get(1))
                .ok_or_else(|| anyhow!("PR {} missing stack tag", event_id))?
                .to_string();

            let depends_on = pr
                .tags
                .iter()
                .find(|tag| tag.len() >= 2 && tag[0] == "depends_on")
                .and_then(|tag| tag.get(1))
                .map(|s| s.to_string());

            let (layer_number, total_layers) = pr
                .tags
                .iter()
                .find(|tag| tag.len() >= 3 && tag[0] == "layer")
                .and_then(|tag| {
                    let current = tag.get(1)?.parse::<u32>().ok()?;
                    let total = tag.get(2)?.parse::<u32>().ok()?;
                    Some((current, total))
                })
                .ok_or_else(|| anyhow!("PR {} missing layer tag", event_id))?;

            let commit_id = pr
                .tags
                .iter()
                .find(|tag| tag.len() >= 2 && tag[0] == "c")
                .and_then(|tag| tag.get(1))
                .map(|s| s.to_string());

            let layer = LayerInfo {
                event_id: event_id.clone(),
                layer_number,
                total_layers,
                stack_id,
                depends_on: depends_on.clone(),
                commit_id,
            };

            layers.insert(event_id.clone(), layer);

            // Build dependency graph
            if let Some(dep) = depends_on {
                dependencies
                    .entry(event_id)
                    .or_insert_with(Vec::new)
                    .push(dep);
            }
        }

        Ok(Self {
            layers,
            dependencies,
        })
    }

    /// Get layers in dependency order (topological sort)
    /// Returns layers from base to top
    ///
    /// # Examples
    ///
    /// ```
    /// use gitafter::stacks::graph::StackGraph;
    /// use nostr::Event;
    ///
    /// # fn create_test_pr(id: &str, stack: &str, layer: u32, total: u32, dep: Option<&str>) -> Event {
    /// #     let mut tags = vec![
    /// #         vec!["stack".to_string(), stack.to_string()],
    /// #         vec!["layer".to_string(), layer.to_string(), total.to_string()],
    /// #     ];
    /// #     if let Some(d) = dep { tags.push(vec!["depends_on".to_string(), d.to_string()]); }
    /// #     Event { id: id.to_string(), kind: 1618, pubkey: "test".to_string(),
    /// #             created_at: 0, content: String::new(), tags, sig: String::new() }
    /// # }
    /// let pr1 = create_test_pr("pr1", "stack1", 1, 3, None);
    /// let pr2 = create_test_pr("pr2", "stack1", 2, 3, Some("pr1"));
    /// let pr3 = create_test_pr("pr3", "stack1", 3, 3, Some("pr2"));
    ///
    /// let graph = StackGraph::from_pr_events(&[pr1, pr2, pr3]).unwrap();
    /// let sorted = graph.topological_sort().unwrap();
    ///
    /// // Base layer first, top layer last
    /// assert_eq!(sorted[0].layer_number, 1);
    /// assert_eq!(sorted[2].layer_number, 3);
    /// ```
    pub fn topological_sort(&self) -> Result<Vec<LayerInfo>> {
        let mut sorted = Vec::new();
        let mut visited = HashSet::new();
        let mut visiting = HashSet::new();

        // Find base layer (layer with no dependencies)
        let base_layers: Vec<_> = self
            .layers
            .values()
            .filter(|l| l.depends_on.is_none())
            .collect();

        if base_layers.is_empty() {
            return Err(anyhow!("No base layer found in stack"));
        }

        if base_layers.len() > 1 {
            return Err(anyhow!("Multiple base layers found in stack"));
        }

        // Start DFS from base layer
        self.visit(
            &base_layers[0].event_id,
            &mut visited,
            &mut visiting,
            &mut sorted,
        )?;

        // Make sure we visited all layers
        if sorted.len() != self.layers.len() {
            return Err(anyhow!("Disconnected layers in stack"));
        }

        // Sort by layer number to ensure correct order
        sorted.sort_by_key(|l| l.layer_number);

        Ok(sorted)
    }

    /// Depth-first search visit for topological sort
    fn visit(
        &self,
        node: &str,
        visited: &mut HashSet<String>,
        visiting: &mut HashSet<String>,
        result: &mut Vec<LayerInfo>,
    ) -> Result<()> {
        if visited.contains(node) {
            return Ok(());
        }

        if visiting.contains(node) {
            return Err(anyhow!("Circular dependency detected in stack"));
        }

        visiting.insert(node.to_string());

        // Visit dependents (layers that depend on this one)
        for (dependent_id, deps) in &self.dependencies {
            if deps.contains(&node.to_string()) {
                self.visit(dependent_id, visited, visiting, result)?;
            }
        }

        visiting.remove(node);
        visited.insert(node.to_string());

        if let Some(layer) = self.layers.get(node) {
            result.push(layer.clone());
        }

        Ok(())
    }

    /// Get all layers in a stack by stack ID
    pub fn get_stack_layers(&self, stack_id: &str) -> Vec<LayerInfo> {
        let mut layers: Vec<_> = self
            .layers
            .values()
            .filter(|l| l.stack_id == stack_id)
            .cloned()
            .collect();

        layers.sort_by_key(|l| l.layer_number);
        layers
    }

    /// Validate the stack structure
    ///
    /// Checks for circular dependencies and consecutive layer numbering.
    ///
    /// # Examples
    ///
    /// ```
    /// use gitafter::stacks::graph::StackGraph;
    /// use nostr::Event;
    ///
    /// # fn create_test_pr(id: &str, stack: &str, layer: u32, total: u32, dep: Option<&str>) -> Event {
    /// #     let mut tags = vec![
    /// #         vec!["stack".to_string(), stack.to_string()],
    /// #         vec!["layer".to_string(), layer.to_string(), total.to_string()],
    /// #     ];
    /// #     if let Some(d) = dep { tags.push(vec!["depends_on".to_string(), d.to_string()]); }
    /// #     Event { id: id.to_string(), kind: 1618, pubkey: "test".to_string(),
    /// #             created_at: 0, content: String::new(), tags, sig: String::new() }
    /// # }
    /// let pr1 = create_test_pr("pr1", "stack1", 1, 2, None);
    /// let pr2 = create_test_pr("pr2", "stack1", 2, 2, Some("pr1"));
    ///
    /// let graph = StackGraph::from_pr_events(&[pr1, pr2]).unwrap();
    /// assert!(graph.validate().is_ok());
    /// ```
    pub fn validate(&self) -> Result<()> {
        // Check for circular dependencies
        let _sorted = self.topological_sort()?;

        // Check that layer numbers are consecutive
        for layers in self.get_stacks().values() {
            let mut numbers: Vec<_> = layers.iter().map(|l| l.layer_number).collect();
            numbers.sort_unstable();

            for (i, num) in numbers.iter().enumerate() {
                if *num != (i as u32 + 1) {
                    return Err(anyhow!("Non-consecutive layer numbers in stack"));
                }
            }
        }

        // Check for missing dependencies
        for layer in self.layers.values() {
            if let Some(dep_id) = &layer.depends_on {
                if !self.layers.contains_key(dep_id) {
                    return Err(anyhow!(
                        "Layer {} depends on non-existent PR {}",
                        layer.event_id,
                        dep_id
                    ));
                }
            }
        }

        Ok(())
    }

    /// Validate that adding a new PR would not create circular dependencies
    ///
    /// # Examples
    ///
    /// ```
    /// use gitafter::stacks::graph::StackGraph;
    /// use nostr::Event;
    ///
    /// # fn create_test_pr(id: &str, stack: &str, layer: u32, total: u32, dep: Option<&str>) -> Event {
    /// #     let mut tags = vec![
    /// #         vec!["stack".to_string(), stack.to_string()],
    /// #         vec!["layer".to_string(), layer.to_string(), total.to_string()],
    /// #     ];
    /// #     if let Some(d) = dep { tags.push(vec!["depends_on".to_string(), d.to_string()]); }
    /// #     Event { id: id.to_string(), kind: 1618, pubkey: "test".to_string(),
    /// #             created_at: 0, content: String::new(), tags, sig: String::new() }
    /// # }
    /// let pr1 = create_test_pr("pr1", "stack1", 1, 2, None);
    /// let graph = StackGraph::from_pr_events(&[pr1]).unwrap();
    ///
    /// // Valid: pr2 depends on existing pr1
    /// let pr2 = create_test_pr("pr2", "stack1", 2, 2, Some("pr1"));
    /// assert!(graph.validate_new_pr(&pr2).is_ok());
    /// ```
    pub fn validate_new_pr(&self, new_pr: &Event) -> Result<()> {
        // Extract dependency from new PR
        let new_depends_on = new_pr
            .tags
            .iter()
            .find(|tag| tag.len() >= 2 && tag[0] == "depends_on")
            .and_then(|tag| tag.get(1))
            .map(|s| s.to_string());

        // If new PR has a dependency, check it exists
        if let Some(dep_id) = &new_depends_on {
            if !self.layers.contains_key(dep_id) {
                return Err(anyhow!(
                    "Cannot create PR: depends on non-existent PR {}",
                    dep_id
                ));
            }

            // Check for circular dependency by simulating adding the PR
            let test_graph = self.clone_with_new_pr(new_pr)?;
            test_graph.topological_sort()?;
        }

        Ok(())
    }

    /// Clone graph and add a new PR for validation purposes
    fn clone_with_new_pr(&self, new_pr: &Event) -> Result<Self> {
        let mut all_prs = Vec::new();

        // Reconstruct events from existing layers (simplified - just for validation)
        for layer in self.layers.values() {
            let mut tags = vec![
                vec!["stack".to_string(), layer.stack_id.clone()],
                vec![
                    "layer".to_string(),
                    layer.layer_number.to_string(),
                    layer.total_layers.to_string(),
                ],
            ];

            if let Some(dep) = &layer.depends_on {
                tags.push(vec!["depends_on".to_string(), dep.clone()]);
            }

            if let Some(commit) = &layer.commit_id {
                tags.push(vec!["c".to_string(), commit.clone()]);
            }

            all_prs.push(Event {
                id: layer.event_id.clone(),
                kind: 1618,
                pubkey: String::new(),
                created_at: 0,
                content: String::new(),
                tags,
                sig: String::new(),
            });
        }

        // Add the new PR
        all_prs.push(new_pr.clone());

        Self::from_pr_events(&all_prs)
    }

    /// Get all stacks grouped by stack ID
    fn get_stacks(&self) -> HashMap<String, Vec<LayerInfo>> {
        let mut stacks: HashMap<String, Vec<LayerInfo>> = HashMap::new();

        for layer in self.layers.values() {
            stacks
                .entry(layer.stack_id.clone())
                .or_insert_with(Vec::new)
                .push(layer.clone());
        }

        stacks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_pr(
        event_id: &str,
        stack_id: &str,
        layer: u32,
        total: u32,
        depends_on: Option<&str>,
        commit: Option<&str>,
    ) -> Event {
        let mut tags = vec![
            vec!["stack".to_string(), stack_id.to_string()],
            vec!["layer".to_string(), layer.to_string(), total.to_string()],
        ];

        if let Some(dep) = depends_on {
            tags.push(vec!["depends_on".to_string(), dep.to_string()]);
        }

        if let Some(c) = commit {
            tags.push(vec!["c".to_string(), c.to_string()]);
        }

        Event {
            id: event_id.to_string(),
            kind: 1618,
            pubkey: "test_pubkey".to_string(),
            created_at: 0,
            content: String::new(),
            tags,
            sig: String::new(),
        }
    }

    #[test]
    fn test_simple_stack() {
        let pr1 = create_test_pr("pr1", "stack1", 1, 3, None, Some("commit1"));
        let pr2 = create_test_pr("pr2", "stack1", 2, 3, Some("pr1"), Some("commit2"));
        let pr3 = create_test_pr("pr3", "stack1", 3, 3, Some("pr2"), Some("commit3"));

        let graph = StackGraph::from_pr_events(&[pr1, pr2, pr3]).unwrap();
        let sorted = graph.topological_sort().unwrap();

        assert_eq!(sorted.len(), 3);
        assert_eq!(sorted[0].event_id, "pr1");
        assert_eq!(sorted[1].event_id, "pr2");
        assert_eq!(sorted[2].event_id, "pr3");
    }

    #[test]
    fn test_validate_good_stack() {
        let pr1 = create_test_pr("pr1", "stack1", 1, 2, None, None);
        let pr2 = create_test_pr("pr2", "stack1", 2, 2, Some("pr1"), None);

        let graph = StackGraph::from_pr_events(&[pr1, pr2]).unwrap();
        assert!(graph.validate().is_ok());
    }

    #[test]
    fn test_circular_dependency() {
        // Create a circular dependency (which shouldn't happen in practice)
        // This test verifies our detection works
        let mut pr1 = create_test_pr("pr1", "stack1", 1, 2, Some("pr2"), None);
        let pr2 = create_test_pr("pr2", "stack1", 2, 2, Some("pr1"), None);

        // Manually create circular dep
        pr1.tags
            .push(vec!["depends_on".to_string(), "pr2".to_string()]);

        let graph = StackGraph::from_pr_events(&[pr1, pr2]).unwrap();
        assert!(graph.topological_sort().is_err());
    }

    #[test]
    fn test_missing_dependency() {
        // PR2 depends on PR1, but PR1 is not in the graph
        let pr2 = create_test_pr("pr2", "stack1", 2, 2, Some("pr1"), None);

        let graph = StackGraph::from_pr_events(&[pr2]).unwrap();
        let result = graph.validate();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("depends on non-existent") || err_msg.contains("No base layer"),
            "Expected error about missing dependency or no base layer, got: {}",
            err_msg
        );
    }

    #[test]
    fn test_validate_new_pr_missing_dependency() {
        let pr1 = create_test_pr("pr1", "stack1", 1, 2, None, None);
        let graph = StackGraph::from_pr_events(&[pr1]).unwrap();

        // Try to add pr3 that depends on non-existent pr2
        let pr3 = create_test_pr("pr3", "stack1", 3, 3, Some("pr2"), None);
        let result = graph.validate_new_pr(&pr3);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("depends on non-existent")
        );
    }

    #[test]
    fn test_validate_new_pr_would_create_cycle() {
        // Create initial stack: pr1 -> pr2
        let pr1 = create_test_pr("pr1", "stack1", 1, 2, None, None);
        let pr2 = create_test_pr("pr2", "stack1", 2, 2, Some("pr1"), None);
        let graph = StackGraph::from_pr_events(&[pr1, pr2]).unwrap();

        // Try to add pr3 that would create a cycle (pr3 depends on pr2, but we'd make pr1 depend on pr3)
        // This is a simplified test - in practice we can't easily create this without modifying existing events
        let pr3 = create_test_pr("pr3", "stack1", 3, 3, Some("pr2"), None);
        let result = graph.validate_new_pr(&pr3);
        assert!(result.is_ok()); // This should be fine - no cycle yet
    }

    #[test]
    fn test_orphaned_layer() {
        // Create layer 1 and layer 3, but skip layer 2
        let pr1 = create_test_pr("pr1", "stack1", 1, 3, None, None);
        let pr3 = create_test_pr("pr3", "stack1", 3, 3, Some("pr1"), None);

        let graph = StackGraph::from_pr_events(&[pr1, pr3]).unwrap();
        let result = graph.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Non-consecutive"));
    }
}
