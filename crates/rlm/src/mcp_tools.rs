//! RLM MCP tools for Codex integration.
//!
//! Exposes RLM capabilities as MCP tools that Codex can invoke:
//! - `rlm_query`: Run recursive analysis on a query
//! - `rlm_fanout`: Distribute query across swarm (if available)
//!
//! This implements Mode A of the RLM + Codex integration: Codex CALLS RLM.
//!
//! # Usage
//!
//! These tools are designed to be exposed via an MCP server. Codex can then
//! decide when to invoke them for deep analysis tasks.
//!
//! ```rust,ignore
//! use rlm::mcp_tools::rlm_tool_definitions;
//!
//! // Get MCP tool definitions for registration
//! let tools = rlm_tool_definitions();
//! ```

use serde::{Deserialize, Serialize};

/// Input for the `rlm_query` tool.
///
/// Deep recursive analysis using the prompt-execute-loop pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RlmQueryInput {
    /// The query to analyze
    pub query: String,
    /// Optional context (file content, data, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    /// Max iterations (default: 10)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_iterations: Option<u32>,
    /// Use orchestrated mode for large context (default: auto)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orchestrated: Option<bool>,
}

impl RlmQueryInput {
    /// Create a new RLM query input.
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            context: None,
            max_iterations: None,
            orchestrated: None,
        }
    }

    /// Set the context for analysis.
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    /// Set maximum iterations.
    pub fn with_max_iterations(mut self, max: u32) -> Self {
        self.max_iterations = Some(max);
        self
    }

    /// Enable orchestrated mode.
    pub fn orchestrated(mut self) -> Self {
        self.orchestrated = Some(true);
        self
    }
}

/// Output from the `rlm_query` tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RlmQueryOutput {
    /// The final answer
    pub answer: String,
    /// Number of iterations taken
    pub iterations: u32,
    /// Execution log entries (optional)
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub execution_log: Vec<String>,
}

/// Input for the `rlm_fanout` tool.
///
/// Distributes analysis across multiple workers (swarm, datacenter, or local).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RlmFanoutInput {
    /// The query to distribute
    pub query: String,
    /// Context to analyze
    pub context: String,
    /// Number of parallel workers (default: 3)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workers: Option<u32>,
    /// Venue: "local", "swarm", "datacenter"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub venue: Option<String>,
}

impl RlmFanoutInput {
    /// Create a new fanout input.
    pub fn new(query: impl Into<String>, context: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            context: context.into(),
            workers: None,
            venue: None,
        }
    }

    /// Set number of parallel workers.
    pub fn with_workers(mut self, n: u32) -> Self {
        self.workers = Some(n);
        self
    }

    /// Set execution venue.
    pub fn with_venue(mut self, venue: impl Into<String>) -> Self {
        self.venue = Some(venue.into());
        self
    }
}

/// Output from the `rlm_fanout` tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RlmFanoutOutput {
    /// The synthesized answer
    pub answer: String,
    /// Results from individual workers
    pub worker_results: Vec<WorkerResult>,
    /// Total cost in satoshis (if swarm execution)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_cost_sats: Option<u64>,
}

/// Result from an individual worker in fanout execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerResult {
    /// Worker identifier
    pub worker_id: String,
    /// The worker's answer
    pub answer: String,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f32,
}

/// Get MCP tool definitions for RLM tools.
///
/// Returns JSON schemas compatible with the MCP protocol's `tools/list` response.
///
/// # Tools Provided
///
/// - **rlm_query**: Deep recursive analysis using RLM. Use when you need to
///   analyze complex problems iteratively, execute code to verify hypotheses,
///   or process large documents.
///
/// - **rlm_fanout**: Distribute analysis across multiple workers. Use for
///   large documents that benefit from parallel analysis, or when you want
///   consensus from multiple providers.
pub fn rlm_tool_definitions() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "name": "rlm_query",
            "description": "Run deep recursive analysis using RLM (Recursive Language Model). \
                           Use when you need to analyze complex problems iteratively, \
                           execute code to verify hypotheses, or process large documents. \
                           The RLM will decompose the problem, write Python code to gather evidence, \
                           verify hypotheses, and synthesize findings into a final answer.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The analysis query - what you want to understand or find"
                    },
                    "context": {
                        "type": "string",
                        "description": "Optional context (file contents, code, data) to analyze"
                    },
                    "max_iterations": {
                        "type": "integer",
                        "description": "Maximum analysis iterations (default: 10)",
                        "minimum": 1,
                        "maximum": 30
                    },
                    "orchestrated": {
                        "type": "boolean",
                        "description": "Use orchestrated mode for large documents (auto-detected if not set)"
                    }
                },
                "required": ["query"]
            }
        }),
        serde_json::json!({
            "name": "rlm_fanout",
            "description": "Distribute analysis across multiple workers for parallel processing. \
                           Use for large documents that benefit from chunked parallel analysis, \
                           or when you want consensus from multiple inference providers. \
                           Workers can run locally, on the swarm network, or in datacenters.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The analysis query to distribute"
                    },
                    "context": {
                        "type": "string",
                        "description": "The context to analyze (will be chunked across workers)"
                    },
                    "workers": {
                        "type": "integer",
                        "description": "Number of parallel workers (default: 3)",
                        "minimum": 1,
                        "maximum": 10
                    },
                    "venue": {
                        "type": "string",
                        "enum": ["local", "swarm", "datacenter"],
                        "description": "Where to run workers: 'local' (same machine), 'swarm' (Nostr network), or 'datacenter' (cloud)"
                    }
                },
                "required": ["query", "context"]
            }
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rlm_query_input_builder() {
        let input = RlmQueryInput::new("Analyze this code")
            .with_context("fn main() { println!(\"Hello\"); }")
            .with_max_iterations(5)
            .orchestrated();

        assert_eq!(input.query, "Analyze this code");
        assert!(input.context.is_some());
        assert_eq!(input.max_iterations, Some(5));
        assert_eq!(input.orchestrated, Some(true));
    }

    #[test]
    fn test_rlm_fanout_input_builder() {
        let input = RlmFanoutInput::new("Find bugs", "code here")
            .with_workers(5)
            .with_venue("swarm");

        assert_eq!(input.query, "Find bugs");
        assert_eq!(input.context, "code here");
        assert_eq!(input.workers, Some(5));
        assert_eq!(input.venue, Some("swarm".to_string()));
    }

    #[test]
    fn test_tool_definitions_structure() {
        let tools = rlm_tool_definitions();

        assert_eq!(tools.len(), 2);

        // Check rlm_query tool
        assert_eq!(tools[0]["name"], "rlm_query");
        assert!(tools[0]["description"].as_str().unwrap().contains("RLM"));
        assert!(tools[0]["inputSchema"]["properties"]["query"].is_object());

        // Check rlm_fanout tool
        assert_eq!(tools[1]["name"], "rlm_fanout");
        assert!(
            tools[1]["description"]
                .as_str()
                .unwrap()
                .contains("parallel")
        );
        assert!(tools[1]["inputSchema"]["properties"]["venue"]["enum"].is_array());
    }

    #[test]
    fn test_serialization() {
        let input = RlmQueryInput::new("test query");
        let json = serde_json::to_string(&input).unwrap();
        let parsed: RlmQueryInput = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.query, "test query");
    }
}
