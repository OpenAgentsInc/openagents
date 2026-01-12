//! NostrBridge for publishing DSPy execution traces to Nostr relays.
//!
//! This module converts receiptable nodes in the execution DAG to Nostr events,
//! enabling distributed observability and audit trails.
//!
//! Only meaningful nodes are published:
//! - Predict nodes → kind:1 (text note) with dsrs tags
//! - Operator nodes (sandbox) → kind:1 (text note) with dsrs tags
//!
//! Root and Map nodes are internal and not published.

use crate::manifest::CompiledModuleManifest;
use crate::trace::{Graph, Node, NodeType};
use anyhow::Result;
use nostr::{Event, EventTemplate, Keypair, finalize_event, generate_secret_key};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Configuration for the Nostr bridge.
#[derive(Debug, Clone)]
pub struct NostrBridgeConfig {
    /// Relay URLs to publish to.
    pub relay_urls: Vec<String>,
    /// Whether to wait for confirmation from relays.
    pub wait_for_ok: bool,
    /// Tags to add to all events.
    pub default_tags: Vec<(String, String)>,
}

impl Default for NostrBridgeConfig {
    fn default() -> Self {
        Self {
            relay_urls: vec!["wss://nexus.openagents.com".to_string()],
            wait_for_ok: true,
            default_tags: Vec::new(),
        }
    }
}

/// Bridge for publishing DSPy traces to Nostr.
pub struct NostrBridge {
    /// Nostr keypair for signing events.
    keypair: Keypair,
    /// Configuration.
    config: NostrBridgeConfig,
}

impl NostrBridge {
    /// Create a new Nostr bridge with a keypair.
    pub fn new(keypair: Keypair) -> Self {
        Self {
            keypair,
            config: NostrBridgeConfig::default(),
        }
    }

    /// Create a new Nostr bridge with a random keypair.
    pub fn generate() -> Self {
        let secret_key = generate_secret_key();
        // Create keypair from secret key
        let keypair = Keypair {
            private_key: secret_key,
            public_key: nostr::get_public_key(&secret_key).expect("valid secret key"),
        };
        Self::new(keypair)
    }

    /// Set configuration.
    pub fn with_config(mut self, config: NostrBridgeConfig) -> Self {
        self.config = config;
        self
    }

    /// Convert a graph to publishable Nostr events.
    ///
    /// This does not publish the events, just creates them.
    pub fn graph_to_events(
        &self,
        graph: &Graph,
        manifest: Option<&CompiledModuleManifest>,
    ) -> Result<Vec<Event>> {
        let mut events = Vec::new();

        for node in &graph.nodes {
            if let Some(event) = self.node_to_event(node, manifest)? {
                events.push(event);
            }
        }

        Ok(events)
    }

    /// Convert a single node to a Nostr event (if applicable).
    fn node_to_event(
        &self,
        node: &Node,
        manifest: Option<&CompiledModuleManifest>,
    ) -> Result<Option<Event>> {
        match &node.node_type {
            NodeType::Root | NodeType::Map { .. } => {
                // Internal nodes, don't publish
                Ok(None)
            }
            NodeType::Predict { signature_name, .. } => {
                // Create a trace event for the prediction
                let trace_data = PredictTraceData {
                    node_id: node.id,
                    signature_name: signature_name.clone(),
                    compiled_id: manifest.and_then(|m| m.compiled_id.clone()),
                    has_output: node.output.is_some(),
                    token_usage: node.output.as_ref().map(|o| TokenUsage {
                        prompt_tokens: o.lm_usage.prompt_tokens,
                        completion_tokens: o.lm_usage.completion_tokens,
                        total_tokens: o.lm_usage.total_tokens,
                        cost_msats: o.lm_usage.cost_msats,
                    }),
                };

                let content = serde_json::to_string(&trace_data)?;

                // Build tags as Vec<Vec<String>>
                let mut tags = vec![
                    vec!["dsrs".to_string(), "predict".to_string()],
                    vec!["signature".to_string(), signature_name.clone()],
                ];

                // Add compiled_id tag if available
                if let Some(id) = manifest.and_then(|m| m.compiled_id.as_ref()) {
                    tags.push(vec!["compiled_id".to_string(), id.clone()]);
                }

                // Add default tags
                for (key, value) in &self.config.default_tags {
                    tags.push(vec![key.clone(), value.clone()]);
                }

                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                let template = EventTemplate {
                    created_at: now,
                    kind: 1, // text note
                    tags,
                    content,
                };

                let event = finalize_event(&template, &self.keypair.private_key)?;
                Ok(Some(event))
            }
            NodeType::Operator { name } => {
                // Create a trace event for the operator
                let trace_data = OperatorTraceData {
                    node_id: node.id,
                    operator_name: name.clone(),
                    compiled_id: manifest.and_then(|m| m.compiled_id.clone()),
                    has_output: node.output.is_some(),
                };

                let content = serde_json::to_string(&trace_data)?;

                let mut tags = vec![
                    vec!["dsrs".to_string(), "operator".to_string()],
                    vec!["operator".to_string(), name.clone()],
                ];

                // Add default tags
                for (key, value) in &self.config.default_tags {
                    tags.push(vec![key.clone(), value.clone()]);
                }

                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                let template = EventTemplate {
                    created_at: now,
                    kind: 1, // text note
                    tags,
                    content,
                };

                let event = finalize_event(&template, &self.keypair.private_key)?;
                Ok(Some(event))
            }
        }
    }

    /// Get all event IDs from events.
    pub fn get_event_ids(events: &[Event]) -> Vec<String> {
        events.iter().map(|e| e.id.clone()).collect()
    }

    /// Create a summary event for the entire trace.
    pub fn create_summary_event(
        &self,
        graph: &Graph,
        manifest: Option<&CompiledModuleManifest>,
    ) -> Result<Event> {
        let summary = NostrTraceSummary {
            node_count: graph.nodes.len(),
            predict_count: graph
                .nodes
                .iter()
                .filter(|n| matches!(n.node_type, NodeType::Predict { .. }))
                .count(),
            operator_count: graph
                .nodes
                .iter()
                .filter(|n| matches!(n.node_type, NodeType::Operator { .. }))
                .count(),
            total_tokens: graph
                .nodes
                .iter()
                .filter_map(|n| n.output.as_ref())
                .map(|o| o.lm_usage.total_tokens)
                .sum(),
            total_cost_msats: graph
                .nodes
                .iter()
                .filter_map(|n| n.output.as_ref())
                .map(|o| o.lm_usage.cost_msats)
                .sum(),
            compiled_id: manifest.and_then(|m| m.compiled_id.clone()),
            signature_name: manifest.map(|m| m.signature_name.clone()),
        };

        let content = serde_json::to_string(&summary)?;

        let mut tags = vec![vec!["dsrs".to_string(), "trace_summary".to_string()]];

        if let Some(id) = &summary.compiled_id {
            tags.push(vec!["compiled_id".to_string(), id.clone()]);
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let template = EventTemplate {
            created_at: now,
            kind: 1, // text note
            tags,
            content,
        };

        let event = finalize_event(&template, &self.keypair.private_key)?;
        Ok(event)
    }

    /// Get the public key of this bridge as hex.
    pub fn public_key_hex(&self) -> String {
        self.keypair.public_key_hex()
    }

    /// Create a generic event with specified kind, content, and tags.
    ///
    /// This is useful for creating custom events outside of the standard trace flow.
    pub fn create_event(&self, kind: u16, content: &str, tags: Vec<Vec<String>>) -> Result<Event> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let template = EventTemplate {
            created_at: now,
            kind,
            tags,
            content: content.to_string(),
        };

        let event = finalize_event(&template, &self.keypair.private_key)?;
        Ok(event)
    }

    /// Get the public key bytes.
    pub fn public_key(&self) -> [u8; 32] {
        self.keypair.public_key
    }
}

/// Trace data for a Predict node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictTraceData {
    /// Node ID in the graph.
    pub node_id: usize,
    /// Name of the signature.
    pub signature_name: String,
    /// Compiled module ID (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compiled_id: Option<String>,
    /// Whether the node produced output.
    pub has_output: bool,
    /// Token usage (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_usage: Option<TokenUsage>,
}

/// Token usage information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub cost_msats: u64,
}

/// Trace data for an Operator node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorTraceData {
    /// Node ID in the graph.
    pub node_id: usize,
    /// Name of the operator.
    pub operator_name: String,
    /// Compiled module ID (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compiled_id: Option<String>,
    /// Whether the node produced output.
    pub has_output: bool,
}

/// Summary of an entire trace for Nostr publishing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NostrTraceSummary {
    /// Total nodes in graph.
    pub node_count: usize,
    /// Number of Predict nodes.
    pub predict_count: usize,
    /// Number of Operator nodes.
    pub operator_count: usize,
    /// Total tokens used.
    pub total_tokens: u64,
    /// Total cost in millisatoshis.
    pub total_cost_msats: u64,
    /// Compiled module ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compiled_id: Option<String>,
    /// Signature name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature_name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::signature::DummySignature;
    use std::sync::Arc;

    #[test]
    fn test_bridge_creation() {
        let bridge = NostrBridge::generate();
        // Should have a valid public key
        assert_eq!(bridge.public_key().len(), 32);
    }

    #[test]
    fn test_graph_to_events_empty() {
        let bridge = NostrBridge::generate();
        let graph = Graph::new();

        let events = bridge.graph_to_events(&graph, None).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_graph_to_events_with_nodes() {
        let bridge = NostrBridge::generate();

        let mut graph = Graph::new();
        // Root node (not published)
        graph.add_node(NodeType::Root, vec![], None);
        // Predict node (published)
        graph.add_node(
            NodeType::Predict {
                signature_name: "TestSig".to_string(),
                signature: Arc::new(DummySignature::new()),
            },
            vec![0],
            None,
        );
        // Map node (not published)
        graph.add_node(
            NodeType::Map {
                mapping: vec![("out".to_string(), (1, "in".to_string()))],
            },
            vec![1],
            None,
        );

        let events = bridge.graph_to_events(&graph, None).unwrap();

        // Only predict node should be published
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_create_summary_event() {
        let bridge = NostrBridge::generate();

        let mut graph = Graph::new();
        graph.add_node(NodeType::Root, vec![], None);
        graph.add_node(
            NodeType::Predict {
                signature_name: "Sig1".to_string(),
                signature: Arc::new(DummySignature::new()),
            },
            vec![0],
            None,
        );

        let event = bridge.create_summary_event(&graph, None).unwrap();

        // Verify event content
        let summary: NostrTraceSummary = serde_json::from_str(&event.content).unwrap();
        assert_eq!(summary.node_count, 2);
        assert_eq!(summary.predict_count, 1);
    }

    #[test]
    fn test_predict_trace_data_serde() {
        let data = PredictTraceData {
            node_id: 1,
            signature_name: "TestSig".to_string(),
            compiled_id: Some("abc123".to_string()),
            has_output: true,
            token_usage: Some(TokenUsage {
                prompt_tokens: 100,
                completion_tokens: 50,
                total_tokens: 150,
                cost_msats: 75,
            }),
        };

        let json = serde_json::to_string(&data).unwrap();
        let parsed: PredictTraceData = serde_json::from_str(&json).unwrap();

        assert_eq!(data.node_id, parsed.node_id);
        assert_eq!(data.signature_name, parsed.signature_name);
    }

    #[test]
    fn test_config_with_default_tags() {
        let config = NostrBridgeConfig {
            relay_urls: vec!["wss://relay.example.com".to_string()],
            wait_for_ok: false,
            default_tags: vec![("app".to_string(), "dsrs".to_string())],
        };

        let bridge = NostrBridge::generate().with_config(config);

        let mut graph = Graph::new();
        graph.add_node(
            NodeType::Predict {
                signature_name: "Sig".to_string(),
                signature: Arc::new(DummySignature::new()),
            },
            vec![],
            None,
        );

        let events = bridge.graph_to_events(&graph, None).unwrap();
        assert_eq!(events.len(), 1);

        // Check that default tag was added
        let has_app_tag = events[0]
            .tags
            .iter()
            .any(|t| t.first().map(|s| s == "app").unwrap_or(false));
        assert!(has_app_tag);
    }
}
