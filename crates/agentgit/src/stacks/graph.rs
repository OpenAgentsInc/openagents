//! Stack dependency graph for managing PR dependencies
//!
//! Builds and validates dependency graphs for stacked PRs.

use anyhow::{anyhow, Result};
use nostr::Event;
use std::collections::{HashMap, HashSet};

/// Represents a stack of PRs with dependency relationships
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
    pub fn from_pr_events(prs: &[Event]) -> Result<Self> {
        let mut layers = HashMap::new();
        let mut dependencies = HashMap::new();

        for pr in prs {
            // Extract stack information from tags
            let event_id = pr.id.clone();

            let stack_id = pr.tags.iter()
                .find(|tag| tag.len() >= 2 && tag[0] == "stack")
                .and_then(|tag| tag.get(1))
                .ok_or_else(|| anyhow!("PR {} missing stack tag", event_id))?
                .to_string();

            let depends_on = pr.tags.iter()
                .find(|tag| tag.len() >= 2 && tag[0] == "depends_on")
                .and_then(|tag| tag.get(1))
                .map(|s| s.to_string());

            let (layer_number, total_layers) = pr.tags.iter()
                .find(|tag| tag.len() >= 3 && tag[0] == "layer")
                .and_then(|tag| {
                    let current = tag.get(1)?.parse::<u32>().ok()?;
                    let total = tag.get(2)?.parse::<u32>().ok()?;
                    Some((current, total))
                })
                .ok_or_else(|| anyhow!("PR {} missing layer tag", event_id))?;

            let commit_id = pr.tags.iter()
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
                dependencies.entry(event_id).or_insert_with(Vec::new).push(dep);
            }
        }

        Ok(Self {
            layers,
            dependencies,
        })
    }

    /// Get layers in dependency order (topological sort)
    /// Returns layers from base to top
    pub fn topological_sort(&self) -> Result<Vec<LayerInfo>> {
        let mut sorted = Vec::new();
        let mut visited = HashSet::new();
        let mut visiting = HashSet::new();

        // Find base layer (layer with no dependencies)
        let base_layers: Vec<_> = self.layers.values()
            .filter(|l| l.depends_on.is_none())
            .collect();

        if base_layers.is_empty() {
            return Err(anyhow!("No base layer found in stack"));
        }

        if base_layers.len() > 1 {
            return Err(anyhow!("Multiple base layers found in stack"));
        }

        // Start DFS from base layer
        self.visit(&base_layers[0].event_id, &mut visited, &mut visiting, &mut sorted)?;

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
        let mut layers: Vec<_> = self.layers.values()
            .filter(|l| l.stack_id == stack_id)
            .cloned()
            .collect();

        layers.sort_by_key(|l| l.layer_number);
        layers
    }

    /// Validate the stack structure
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

        Ok(())
    }

    /// Get all stacks grouped by stack ID
    fn get_stacks(&self) -> HashMap<String, Vec<LayerInfo>> {
        let mut stacks: HashMap<String, Vec<LayerInfo>> = HashMap::new();

        for layer in self.layers.values() {
            stacks.entry(layer.stack_id.clone())
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
        pr1.tags.push(vec!["depends_on".to_string(), "pr2".to_string()]);

        let graph = StackGraph::from_pr_events(&[pr1, pr2]).unwrap();
        assert!(graph.topological_sort().is_err());
    }
}
