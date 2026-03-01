use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{CadError, CadResult};

/// Stable identifier for a feature node.
pub type FeatureNodeId = String;

/// Feature node contract with explicit dependency edges.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FeatureNode {
    pub id: FeatureNodeId,
    pub name: String,
    pub operation_key: String,
    pub depends_on: Vec<FeatureNodeId>,
    pub params: BTreeMap<String, String>,
}

/// Deterministic feature graph container.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct FeatureGraph {
    pub nodes: Vec<FeatureNode>,
}

impl FeatureGraph {
    /// Validate ID uniqueness and dependency references.
    pub fn validate(&self) -> CadResult<()> {
        let mut ids = BTreeSet::new();
        for node in &self.nodes {
            if node.id.trim().is_empty() {
                return Err(CadError::InvalidFeatureGraph {
                    reason: "feature node id must not be empty".to_string(),
                });
            }
            if !ids.insert(node.id.clone()) {
                return Err(CadError::InvalidFeatureGraph {
                    reason: format!("duplicate feature node id: {}", node.id),
                });
            }
        }

        for node in &self.nodes {
            for dep in &node.depends_on {
                if dep == &node.id {
                    return Err(CadError::InvalidFeatureGraph {
                        reason: format!("feature {} cannot depend on itself", node.id),
                    });
                }
                if !ids.contains(dep) {
                    return Err(CadError::InvalidFeatureGraph {
                        reason: format!(
                            "feature {} depends on missing node {}",
                            node.id, dep
                        ),
                    });
                }
            }
        }

        Ok(())
    }

    /// Compute deterministic topological order.
    ///
    /// Tie-breaking is lexical by feature node id.
    pub fn deterministic_topo_order(&self) -> CadResult<Vec<FeatureNodeId>> {
        self.validate()?;

        let mut indegree: BTreeMap<FeatureNodeId, usize> = BTreeMap::new();
        let mut outgoing: BTreeMap<FeatureNodeId, BTreeSet<FeatureNodeId>> = BTreeMap::new();

        for node in &self.nodes {
            indegree.insert(node.id.clone(), node.depends_on.len());
            outgoing.entry(node.id.clone()).or_default();
        }

        for node in &self.nodes {
            for dep in &node.depends_on {
                outgoing
                    .entry(dep.clone())
                    .or_default()
                    .insert(node.id.clone());
            }
        }

        let mut ready: BTreeSet<FeatureNodeId> = indegree
            .iter()
            .filter_map(|(id, degree)| (*degree == 0).then_some(id.clone()))
            .collect();

        let mut ordered = Vec::with_capacity(self.nodes.len());

        while let Some(next) = ready.pop_first() {
            ordered.push(next.clone());

            if let Some(children) = outgoing.get(&next) {
                for child in children {
                    if let Some(value) = indegree.get_mut(child) {
                        *value = value.saturating_sub(1);
                        if *value == 0 {
                            ready.insert(child.clone());
                        }
                    }
                }
            }
        }

        if ordered.len() != self.nodes.len() {
            return Err(CadError::InvalidFeatureGraph {
                reason: "cycle detected in feature graph".to_string(),
            });
        }

        Ok(ordered)
    }
}

#[cfg(test)]
mod tests {
    use super::{FeatureGraph, FeatureNode};
    use std::collections::BTreeMap;

    fn node(id: &str, depends_on: &[&str]) -> FeatureNode {
        FeatureNode {
            id: id.to_string(),
            name: id.to_string(),
            operation_key: "noop".to_string(),
            depends_on: depends_on.iter().map(|value| (*value).to_string()).collect(),
            params: BTreeMap::new(),
        }
    }

    #[test]
    fn deterministic_topo_order_is_stable_across_insertion_orders() {
        let graph_a = FeatureGraph {
            nodes: vec![
                node("feature.vent", &["feature.base"]),
                node("feature.base", &[]),
                node("feature.mount_holes", &["feature.base"]),
            ],
        };
        let graph_b = FeatureGraph {
            nodes: vec![
                node("feature.base", &[]),
                node("feature.mount_holes", &["feature.base"]),
                node("feature.vent", &["feature.base"]),
            ],
        };

        let order_a = graph_a.deterministic_topo_order();
        let order_b = graph_b.deterministic_topo_order();
        assert!(order_a.is_ok(), "graph_a topo order should succeed");
        assert!(order_b.is_ok(), "graph_b topo order should succeed");

        if let (Ok(left), Ok(right)) = (order_a, order_b) {
            assert_eq!(left, right);
            assert_eq!(
                left,
                vec![
                    "feature.base".to_string(),
                    "feature.mount_holes".to_string(),
                    "feature.vent".to_string()
                ]
            );
        }
    }

    #[test]
    fn missing_dependency_is_rejected() {
        let graph = FeatureGraph {
            nodes: vec![node("feature.vent", &["feature.base"])],
        };
        let result = graph.validate();
        assert!(result.is_err(), "missing dependency should return error");
    }

    #[test]
    fn cycle_is_rejected() {
        let graph = FeatureGraph {
            nodes: vec![
                node("feature.a", &["feature.b"]),
                node("feature.b", &["feature.a"]),
            ],
        };
        let result = graph.deterministic_topo_order();
        assert!(result.is_err(), "cycle should return error");
    }
}
