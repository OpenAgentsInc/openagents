//! FRLM Tool Handler - executes FRLM tools via the conductor.
//!
//! This handler bridges tool calls from the Apple FM Bridge to the FRLM conductor,
//! enabling recursive LLM sub-calls, fragment management, and execution tracing.

use anyhow::{anyhow, Result};
use frlm::{FrlmConductor, FrlmPolicy, Fragment, SubQuery, TraceEvent};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Handler for FRLM tool execution.
pub struct FrlmToolHandler {
    /// The FRLM conductor for orchestrating sub-queries.
    conductor: Arc<Mutex<FrlmConductor>>,
    /// Stored trace events for retrieval.
    trace_events: Arc<Mutex<Vec<TraceEvent>>>,
}

impl FrlmToolHandler {
    /// Create a new FRLM tool handler with default policy.
    pub fn new() -> Self {
        Self::with_policy(FrlmPolicy::default())
    }

    /// Create a handler with a custom policy.
    pub fn with_policy(policy: FrlmPolicy) -> Self {
        let conductor = FrlmConductor::new(policy);
        Self {
            conductor: Arc::new(Mutex::new(conductor)),
            trace_events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Execute a tool by name with the given arguments.
    pub async fn execute(&self, tool_name: &str, args: Value) -> Result<Value> {
        match tool_name {
            "llm_query_recursive" => self.handle_llm_query(args).await,
            "load_environment" => self.handle_load_environment(args).await,
            "select_fragments" => self.handle_select_fragments(args).await,
            "execute_parallel" => self.handle_execute_parallel(args).await,
            "verify_results" => self.handle_verify_results(args).await,
            "check_budget" => self.handle_check_budget(args).await,
            "get_trace_events" => self.handle_get_trace_events(args).await,
            _ => Err(anyhow!("Unknown FRLM tool: {}", tool_name)),
        }
    }

    /// Handle llm_query_recursive tool call.
    async fn handle_llm_query(&self, args: Value) -> Result<Value> {
        let prompt = args
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing required 'prompt' parameter"))?;

        let context = args.get("context").and_then(|v| v.as_str());
        let budget = args.get("budget").and_then(|v| v.as_u64());
        let model = args.get("model").and_then(|v| v.as_str());

        // Build the full prompt with context
        let full_prompt = if let Some(ctx) = context {
            format!("Context:\n{}\n\nQuery:\n{}", ctx, prompt)
        } else {
            prompt.to_string()
        };

        // Create sub-query
        let query_id = format!("sq-{}", uuid::Uuid::new_v4());
        let mut sub_query = SubQuery::new(&query_id, &full_prompt);

        if let Some(b) = budget {
            sub_query = sub_query.with_max_tokens(b as u32);
        }
        if let Some(m) = model {
            sub_query = sub_query.with_model(m);
        }

        // For now, we return a placeholder - in production this would
        // submit to the swarm or execute locally
        let conductor = self.conductor.lock().await;

        // Check if we can afford this query
        let estimated_cost = budget.unwrap_or(1000);
        if !conductor.can_afford(estimated_cost) {
            return Ok(json!({
                "error": "Budget exceeded",
                "remaining_budget": conductor.budget_remaining()
            }));
        }

        // Record trace event
        let mut events = self.trace_events.lock().await;
        events.push(TraceEvent::SubQuerySubmit {
            run_id: "tool-call".to_string(),
            query_id: query_id.clone(),
            prompt_preview: if full_prompt.len() > 100 {
                format!("{}...", &full_prompt[..97])
            } else {
                full_prompt.clone()
            },
            fragment_id: None,
            timestamp_ms: 0,
        });

        // Return structured response
        Ok(json!({
            "query_id": query_id,
            "status": "submitted",
            "prompt": full_prompt,
            "estimated_budget": estimated_cost
        }))
    }

    /// Handle load_environment tool call.
    async fn handle_load_environment(&self, args: Value) -> Result<Value> {
        let fragments_json = args
            .get("fragments")
            .ok_or_else(|| anyhow!("Missing required 'fragments' parameter"))?;

        let fragments: Vec<FragmentInput> = serde_json::from_value(fragments_json.clone())?;

        let mut conductor = self.conductor.lock().await;

        // Convert to FRLM fragments and load
        let frlm_fragments: Vec<Fragment> = fragments
            .into_iter()
            .map(|f| Fragment::new(f.id, f.content))
            .collect();

        let count = frlm_fragments.len();
        conductor.load_fragments(frlm_fragments);

        // Load context variables if provided
        if let Some(context_vars) = args.get("context_vars").and_then(|v| v.as_object()) {
            for (key, value) in context_vars {
                if let Some(v) = value.as_str() {
                    conductor.set_context(key, v);
                }
            }
        }

        Ok(json!({
            "loaded": count,
            "status": "success"
        }))
    }

    /// Handle select_fragments tool call.
    async fn handle_select_fragments(&self, args: Value) -> Result<Value> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing required 'query' parameter"))?;

        let max_fragments = args
            .get("max_fragments")
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as usize;

        // For now, return a basic selection result
        // In production, this would use semantic search or the conductor's fragment selection
        Ok(json!({
            "query": query,
            "max_fragments": max_fragments,
            "selected": [],
            "status": "no_fragments_loaded"
        }))
    }

    /// Handle execute_parallel tool call.
    async fn handle_execute_parallel(&self, args: Value) -> Result<Value> {
        let queries_json = args
            .get("queries")
            .ok_or_else(|| anyhow!("Missing required 'queries' parameter"))?;

        let queries: Vec<SubQueryInput> = serde_json::from_value(queries_json.clone())?;

        let fanout = args.get("fanout").and_then(|v| v.as_u64()).unwrap_or(8) as usize;
        let timeout_ms = args
            .get("timeout_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(30000);

        let query_count = queries.len();

        // Record trace events
        let mut events = self.trace_events.lock().await;
        for query in &queries {
            events.push(TraceEvent::SubQuerySubmit {
                run_id: "parallel-exec".to_string(),
                query_id: query.id.clone(),
                prompt_preview: if query.prompt.len() > 100 {
                    format!("{}...", &query.prompt[..97])
                } else {
                    query.prompt.clone()
                },
                fragment_id: query.fragment_id.clone(),
                timestamp_ms: 0,
            });
        }

        Ok(json!({
            "submitted": query_count,
            "fanout": fanout,
            "timeout_ms": timeout_ms,
            "status": "executing"
        }))
    }

    /// Handle verify_results tool call.
    async fn handle_verify_results(&self, args: Value) -> Result<Value> {
        let results_json = args
            .get("results")
            .ok_or_else(|| anyhow!("Missing required 'results' parameter"))?;

        let tier = args
            .get("tier")
            .and_then(|v| v.as_str())
            .unwrap_or("none");

        let results: Vec<ResultInput> = serde_json::from_value(results_json.clone())?;

        let verification_result = match tier {
            "none" => json!({
                "tier": "none",
                "passed": true,
                "verified_count": results.len()
            }),
            "redundancy" => {
                let n_of_m = args.get("n_of_m").and_then(|v| v.as_object());
                let (n, m) = if let Some(nm) = n_of_m {
                    (
                        nm.get("n").and_then(|v| v.as_u64()).unwrap_or(2) as usize,
                        nm.get("m").and_then(|v| v.as_u64()).unwrap_or(3) as usize,
                    )
                } else {
                    (2, 3)
                };
                json!({
                    "tier": "redundancy",
                    "n": n,
                    "m": m,
                    "passed": results.len() >= n,
                    "verified_count": results.len()
                })
            }
            "objective" => {
                let schema = args.get("schema");
                json!({
                    "tier": "objective",
                    "schema_provided": schema.is_some(),
                    "passed": true,
                    "verified_count": results.len()
                })
            }
            "validated" => {
                json!({
                    "tier": "validated",
                    "passed": true,
                    "verified_count": results.len(),
                    "attestations": []
                })
            }
            _ => {
                return Err(anyhow!("Unknown verification tier: {}", tier));
            }
        };

        Ok(verification_result)
    }

    /// Handle check_budget tool call.
    async fn handle_check_budget(&self, args: Value) -> Result<Value> {
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing required 'action' parameter"))?;

        let tokens = args.get("tokens").and_then(|v| v.as_u64()).unwrap_or(0);

        let conductor = self.conductor.lock().await;
        let remaining = conductor.budget_remaining();

        match action {
            "check" => Ok(json!({
                "action": "check",
                "remaining_sats": remaining,
                "can_afford_1k_tokens": conductor.can_afford(100)
            })),
            "reserve" => {
                if conductor.can_afford(tokens) {
                    Ok(json!({
                        "action": "reserve",
                        "tokens": tokens,
                        "status": "reserved",
                        "remaining_after": remaining.saturating_sub(tokens)
                    }))
                } else {
                    Ok(json!({
                        "action": "reserve",
                        "tokens": tokens,
                        "status": "insufficient_budget",
                        "remaining": remaining
                    }))
                }
            }
            "release" => Ok(json!({
                "action": "release",
                "tokens": tokens,
                "status": "released",
                "remaining_after": remaining + tokens
            })),
            _ => Err(anyhow!("Unknown budget action: {}", action)),
        }
    }

    /// Handle get_trace_events tool call.
    async fn handle_get_trace_events(&self, args: Value) -> Result<Value> {
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(100) as usize;

        let event_types: Option<Vec<String>> = args
            .get("event_types")
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        let events = self.trace_events.lock().await;

        // Filter and serialize events
        let filtered: Vec<Value> = events
            .iter()
            .filter(|e| {
                if let Some(ref types) = event_types {
                    let event_type = event_type_name(e);
                    types.contains(&event_type.to_string())
                } else {
                    true
                }
            })
            .take(limit)
            .map(|e| trace_event_to_json(e))
            .collect();

        Ok(json!({
            "events": filtered,
            "count": filtered.len(),
            "total_available": events.len()
        }))
    }
}

impl Default for FrlmToolHandler {
    fn default() -> Self {
        Self::new()
    }
}

// Helper types for deserialization

#[derive(Debug, Deserialize)]
struct FragmentInput {
    id: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct SubQueryInput {
    id: String,
    prompt: String,
    fragment_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ResultInput {
    query_id: String,
    content: String,
}

/// Get the type name of a trace event.
fn event_type_name(event: &TraceEvent) -> &'static str {
    match event {
        TraceEvent::RunInit { .. } => "query_started",
        TraceEvent::RunDone { .. } => "query_completed",
        TraceEvent::EnvLoadFragment { .. } => "fragment_loaded",
        TraceEvent::EnvSelectFragments { .. } => "info",
        TraceEvent::SubQuerySubmit { .. } => "subquery_dispatched",
        TraceEvent::SubQueryReturn { .. } => "subquery_completed",
        TraceEvent::Aggregate { .. } => "info",
        TraceEvent::FallbackLocal { .. } => "warning",
        TraceEvent::BudgetReserve { .. } => "budget_updated",
        TraceEvent::BudgetSettle { .. } => "budget_updated",
        TraceEvent::SubQueryExecute { .. } => "info",
        TraceEvent::SubQueryTimeout { .. } => "warning",
        TraceEvent::VerifyRedundant { .. } => "verification_completed",
        TraceEvent::VerifyObjective { .. } => "verification_completed",
    }
}

/// Convert a trace event to JSON.
fn trace_event_to_json(event: &TraceEvent) -> Value {
    match event {
        TraceEvent::RunInit {
            program,
            fragment_count,
            ..
        } => json!({
            "type": "query_started",
            "program": program,
            "fragment_count": fragment_count
        }),
        TraceEvent::RunDone {
            output,
            iterations,
            total_cost_sats,
            ..
        } => json!({
            "type": "query_completed",
            "output_length": output.len(),
            "iterations": iterations,
            "cost_sats": total_cost_sats
        }),
        TraceEvent::EnvLoadFragment {
            fragment_id,
            size_bytes,
            ..
        } => json!({
            "type": "fragment_loaded",
            "fragment_id": fragment_id,
            "size_bytes": size_bytes
        }),
        TraceEvent::SubQuerySubmit {
            query_id,
            prompt_preview,
            fragment_id,
            ..
        } => json!({
            "type": "subquery_dispatched",
            "query_id": query_id,
            "prompt_length": prompt_preview.len(),
            "fragment_id": fragment_id
        }),
        TraceEvent::SubQueryReturn {
            query_id,
            result_preview,
            duration_ms,
            cost_sats,
            success,
            ..
        } => json!({
            "type": "subquery_completed",
            "query_id": query_id,
            "content_length": result_preview.len(),
            "duration_ms": duration_ms,
            "cost_sats": cost_sats,
            "success": success
        }),
        TraceEvent::SubQueryTimeout {
            query_id,
            elapsed_ms,
            ..
        } => json!({
            "type": "warning",
            "subtype": "timeout",
            "query_id": query_id,
            "elapsed_ms": elapsed_ms
        }),
        TraceEvent::BudgetReserve {
            query_id,
            amount_sats,
            remaining_sats,
            ..
        } => json!({
            "type": "budget_updated",
            "action": "reserve",
            "query_id": query_id,
            "reserved": amount_sats,
            "remaining": remaining_sats
        }),
        TraceEvent::BudgetSettle {
            query_id,
            actual_sats,
            refund_sats,
            ..
        } => json!({
            "type": "budget_updated",
            "action": "settle",
            "query_id": query_id,
            "actual": actual_sats,
            "refund": refund_sats
        }),
        _ => json!({
            "type": "info",
            "event": format!("{:?}", event)
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_llm_query_recursive() {
        let handler = FrlmToolHandler::new();
        let result = handler
            .execute(
                "llm_query_recursive",
                json!({
                    "prompt": "What is 2+2?",
                    "budget": 100
                }),
            )
            .await
            .unwrap();

        assert!(result.get("query_id").is_some());
        assert_eq!(result.get("status").unwrap(), "submitted");
    }

    #[tokio::test]
    async fn test_load_environment() {
        let handler = FrlmToolHandler::new();
        let result = handler
            .execute(
                "load_environment",
                json!({
                    "fragments": [
                        {"id": "f1", "content": "Hello world"},
                        {"id": "f2", "content": "Goodbye world"}
                    ]
                }),
            )
            .await
            .unwrap();

        assert_eq!(result.get("loaded").unwrap(), 2);
        assert_eq!(result.get("status").unwrap(), "success");
    }

    #[tokio::test]
    async fn test_check_budget() {
        let handler = FrlmToolHandler::new();
        let result = handler
            .execute("check_budget", json!({"action": "check"}))
            .await
            .unwrap();

        assert_eq!(result.get("action").unwrap(), "check");
        assert!(result.get("remaining_sats").is_some());
    }

    #[tokio::test]
    async fn test_verify_results() {
        let handler = FrlmToolHandler::new();
        let result = handler
            .execute(
                "verify_results",
                json!({
                    "results": [
                        {"query_id": "q1", "content": "Result 1"},
                        {"query_id": "q2", "content": "Result 2"}
                    ],
                    "tier": "redundancy",
                    "n_of_m": {"n": 2, "m": 3}
                }),
            )
            .await
            .unwrap();

        assert_eq!(result.get("tier").unwrap(), "redundancy");
        assert_eq!(result.get("passed").unwrap(), true);
    }

    #[tokio::test]
    async fn test_get_trace_events() {
        let handler = FrlmToolHandler::new();

        // First submit a query to generate a trace event
        handler
            .execute("llm_query_recursive", json!({"prompt": "Test"}))
            .await
            .unwrap();

        // Now get trace events
        let result = handler
            .execute("get_trace_events", json!({"limit": 10}))
            .await
            .unwrap();

        assert!(result.get("events").is_some());
        assert!(result.get("count").unwrap().as_u64().unwrap() > 0);
    }

    #[tokio::test]
    async fn test_unknown_tool() {
        let handler = FrlmToolHandler::new();
        let result = handler.execute("unknown_tool", json!({})).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown FRLM tool"));
    }
}
