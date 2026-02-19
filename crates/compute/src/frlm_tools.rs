//! FRLM tool definitions for Apple FM integration.
//!
//! Defines the 7 core FRLM tools that enable recursive LLM sub-calls:
//! - llm_query_recursive: Make recursive sub-LM calls
//! - load_environment: Load fragments into context
//! - select_fragments: Select relevant fragments
//! - execute_parallel: Execute sub-queries in parallel
//! - verify_results: Verify sub-query results (4 tiers)
//! - check_budget: Check/update token budget
//! - get_trace_events: Get execution trace

use crate::backends::FmToolDefinition;
use serde_json::json;

/// Create all 7 FRLM tool definitions with JSON schemas.
pub fn create_frlm_tools() -> Vec<FmToolDefinition> {
    vec![
        // Core recursive sub-LM call
        FmToolDefinition {
            name: "llm_query_recursive".to_string(),
            description: "Make a recursive sub-LM call. Returns the LLM response.".to_string(),
            parameters: Some(json!({
                "type": "object",
                "properties": {
                    "prompt": {
                        "type": "string",
                        "description": "The prompt to send to the sub-LM"
                    },
                    "context": {
                        "type": "string",
                        "description": "Optional context to include with the prompt"
                    },
                    "budget": {
                        "type": "integer",
                        "description": "Maximum tokens for this sub-query"
                    },
                    "verification": {
                        "type": "string",
                        "enum": ["none", "redundancy", "objective", "validated"],
                        "description": "Verification tier for the result"
                    },
                    "model": {
                        "type": "string",
                        "description": "Optional model preference"
                    }
                },
                "required": ["prompt"]
            })),
        },
        // Load fragments into context
        FmToolDefinition {
            name: "load_environment".to_string(),
            description: "Load fragments into the execution context for processing.".to_string(),
            parameters: Some(json!({
                "type": "object",
                "properties": {
                    "fragments": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string" },
                                "content": { "type": "string" }
                            },
                            "required": ["id", "content"]
                        },
                        "description": "Fragments to load"
                    },
                    "context_vars": {
                        "type": "object",
                        "additionalProperties": { "type": "string" },
                        "description": "Context variables to set"
                    }
                },
                "required": ["fragments"]
            })),
        },
        // Select relevant fragments
        FmToolDefinition {
            name: "select_fragments".to_string(),
            description: "Select relevant fragments from loaded environment based on criteria."
                .to_string(),
            parameters: Some(json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Query to match fragments against"
                    },
                    "max_fragments": {
                        "type": "integer",
                        "description": "Maximum number of fragments to return"
                    },
                    "filter": {
                        "type": "object",
                        "additionalProperties": { "type": "string" },
                        "description": "Metadata filter (key-value pairs)"
                    }
                },
                "required": ["query"]
            })),
        },
        // Execute sub-queries in parallel
        FmToolDefinition {
            name: "execute_parallel".to_string(),
            description: "Execute multiple sub-queries in parallel using the FRLM scheduler."
                .to_string(),
            parameters: Some(json!({
                "type": "object",
                "properties": {
                    "queries": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string" },
                                "prompt": { "type": "string" },
                                "fragment_id": { "type": "string" }
                            },
                            "required": ["id", "prompt"]
                        },
                        "description": "Sub-queries to execute in parallel"
                    },
                    "fanout": {
                        "type": "integer",
                        "description": "Maximum concurrent executions"
                    },
                    "timeout_ms": {
                        "type": "integer",
                        "description": "Timeout per query in milliseconds"
                    }
                },
                "required": ["queries"]
            })),
        },
        // Verify results
        FmToolDefinition {
            name: "verify_results".to_string(),
            description: "Verify sub-query results using specified verification tier.".to_string(),
            parameters: Some(json!({
                "type": "object",
                "properties": {
                    "results": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "query_id": { "type": "string" },
                                "content": { "type": "string" }
                            },
                            "required": ["query_id", "content"]
                        },
                        "description": "Results to verify"
                    },
                    "tier": {
                        "type": "string",
                        "enum": ["none", "redundancy", "objective", "validated"],
                        "description": "Verification tier: none (skip), redundancy (N-of-M), objective (schema), validated (attestation)"
                    },
                    "schema": {
                        "type": "object",
                        "description": "JSON schema for objective verification"
                    },
                    "n_of_m": {
                        "type": "object",
                        "properties": {
                            "n": { "type": "integer" },
                            "m": { "type": "integer" }
                        },
                        "description": "N-of-M consensus parameters for redundancy verification"
                    }
                },
                "required": ["results", "tier"]
            })),
        },
        // Check budget
        FmToolDefinition {
            name: "check_budget".to_string(),
            description: "Check remaining token budget or update budget allocation.".to_string(),
            parameters: Some(json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["check", "reserve", "release"],
                        "description": "Budget action: check (get remaining), reserve (allocate), release (return)"
                    },
                    "tokens": {
                        "type": "integer",
                        "description": "Number of tokens to reserve or release"
                    }
                },
                "required": ["action"]
            })),
        },
        // Get trace events
        FmToolDefinition {
            name: "get_trace_events".to_string(),
            description: "Get execution trace events for debugging and observability.".to_string(),
            parameters: Some(json!({
                "type": "object",
                "properties": {
                    "run_id": {
                        "type": "string",
                        "description": "Optional run ID to filter events"
                    },
                    "event_types": {
                        "type": "array",
                        "items": {
                            "type": "string",
                            "enum": [
                                "query_started",
                                "query_completed",
                                "fragment_loaded",
                                "subquery_dispatched",
                                "subquery_completed",
                                "verification_started",
                                "verification_completed",
                                "budget_updated",
                                "error",
                                "warning",
                                "info",
                                "debug"
                            ]
                        },
                        "description": "Filter by event types"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of events to return"
                    }
                }
            })),
        },
    ]
}

/// Get the names of all FRLM tools.
pub fn frlm_tool_names() -> Vec<String> {
    vec![
        "llm_query_recursive".to_string(),
        "load_environment".to_string(),
        "select_fragments".to_string(),
        "execute_parallel".to_string(),
        "verify_results".to_string(),
        "check_budget".to_string(),
        "get_trace_events".to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_frlm_tools() {
        let tools = create_frlm_tools();
        assert_eq!(tools.len(), 7);

        // Verify tool names
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"llm_query_recursive"));
        assert!(names.contains(&"load_environment"));
        assert!(names.contains(&"select_fragments"));
        assert!(names.contains(&"execute_parallel"));
        assert!(names.contains(&"verify_results"));
        assert!(names.contains(&"check_budget"));
        assert!(names.contains(&"get_trace_events"));
    }

    #[test]
    fn test_tool_names() {
        let names = frlm_tool_names();
        assert_eq!(names.len(), 7);
    }

    #[test]
    fn test_tools_have_descriptions() {
        let tools = create_frlm_tools();
        for tool in tools {
            assert!(
                !tool.description.is_empty(),
                "Tool {} has no description",
                tool.name
            );
        }
    }

    #[test]
    fn test_tools_have_parameters() {
        let tools = create_frlm_tools();
        for tool in tools {
            // All FRLM tools should have parameters
            assert!(
                tool.parameters.is_some(),
                "Tool {} has no parameters",
                tool.name
            );
        }
    }
}
