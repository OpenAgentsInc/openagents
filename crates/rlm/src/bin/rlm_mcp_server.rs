//! RLM MCP Server
//!
//! Exposes RLM tools via the MCP (Model Context Protocol) over stdio.
//!
//! # Usage
//!
//! Configure in Codex settings:
//! ```json
//! {
//!   "mcpServers": {
//!     "rlm": {
//!       "type": "stdio",
//!       "command": "rlm-mcp-server"
//!     }
//!   }
//! }
//! ```
//!
//! Or via command line:
//! ```bash
//! codex --mcp-server "rlm:stdio:rlm-mcp-server"
//! ```
//!
//! # Backend Selection
//!
//! Set `RLM_BACKEND` environment variable:
//! - `codex` - Use Codex via app-server (requires `codex` feature)
//! - `ollama` - Use Ollama at localhost:11434 (default)
//!
//! Example:
//! ```bash
//! RLM_BACKEND=codex rlm-mcp-server
//! ```

use lm_router::backends::OllamaBackend;
use lm_router::LmBackend;
use rlm::mcp_tools::{rlm_tool_definitions, RlmFanoutInput, RlmQueryInput};
use rlm::{Context, LmRouterClient, PythonExecutor, RlmConfig, RlmEngine};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::sync::Arc;

/// Backend type for RLM execution.
#[derive(Debug, Clone, Copy, PartialEq)]
enum RlmBackend {
    /// Use Ollama at localhost:11434
    Ollama,
    /// Use Codex via app-server (requires `codex` feature)
    Codex,
}

impl RlmBackend {
    fn from_env() -> Self {
        match std::env::var("RLM_BACKEND").as_deref() {
            Ok("codex") => RlmBackend::Codex,
            _ => RlmBackend::Ollama,
        }
    }
}

/// Create an LmRouter with auto-detected backends.
async fn create_router_with_backends() -> (Arc<lm_router::LmRouter>, String) {
    let mut builder = lm_router::LmRouter::builder();
    let mut default_model = "auto".to_string();

    // Try to detect Ollama
    let mut ollama = OllamaBackend::new();
    if ollama.is_available().await {
        if let Ok(()) = ollama.detect_models().await {
            let models: Vec<String> = ollama.supported_models();
            if let Some(first_model) = models.first() {
                default_model = first_model.to_string();
                eprintln!("[rlm-mcp-server] Detected Ollama with {} models", models.len());
            }
        }
        builder = builder.add_backend(ollama);
    } else {
        eprintln!("[rlm-mcp-server] Ollama not detected at localhost:11434");
    }

    let router = Arc::new(builder.build());

    // Get first available model from router if we have one
    if let Some(model) = router.available_models().first() {
        default_model = model.clone();
    }

    (router, default_model)
}

/// Handle MCP JSON-RPC requests
async fn handle_request(request: &Value) -> Value {
    let id = request.get("id").cloned().unwrap_or(json!(null));
    let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");

    let result = match method {
        "initialize" => handle_initialize(),
        "initialized" => json!({}), // Acknowledgment, no response needed
        "tools/list" => handle_tools_list(),
        "tools/call" => handle_tools_call(request).await,
        "shutdown" => json!({}),
        _ => json!({ "error": { "code": -32601, "message": format!("Unknown method: {}", method) } }),
    };

    // Don't wrap result if it's already an error response
    if result.get("error").is_some() {
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": result["error"]
        })
    } else {
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result
        })
    }
}

fn handle_initialize() -> Value {
    json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": "rlm-mcp-server",
            "version": env!("CARGO_PKG_VERSION")
        }
    })
}

fn handle_tools_list() -> Value {
    json!({
        "tools": rlm_tool_definitions()
    })
}

async fn handle_tools_call(request: &Value) -> Value {
    let empty_params = json!({});
    let params = request.get("params").unwrap_or(&empty_params);
    let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
    let empty_args = json!({});
    let args = params.get("arguments").unwrap_or(&empty_args);

    match tool_name {
        "rlm_query" => execute_rlm_query(args).await,
        "rlm_fanout" => execute_rlm_fanout(args).await,
        _ => json!({
            "error": { "code": -32602, "message": format!("Unknown tool: {}", tool_name) }
        }),
    }
}

/// Execute RLM query using Ollama backend.
async fn execute_rlm_query_ollama(
    input: &RlmQueryInput,
    config: RlmConfig,
) -> Result<rlm::RlmResult, rlm::RlmError> {
    // Create LmRouter with auto-detected backends
    let (router, model) = create_router_with_backends().await;

    let client = LmRouterClient::new(router, &model);
    let executor = PythonExecutor::new();

    let mut engine = RlmEngine::with_config(client, executor, config);

    // Set context if provided
    if let Some(ref ctx) = input.context {
        engine.set_context(Context::from_text(ctx.clone()));
    }

    // Choose execution mode
    if input.orchestrated.unwrap_or(false)
        || input
            .context
            .as_ref()
            .map(|c| c.len() > 50_000)
            .unwrap_or(false)
    {
        engine.run_orchestrated(&input.query).await
    } else {
        engine.run(&input.query).await
    }
}

/// Execute RLM query using Codex backend (requires `codex` feature).
#[cfg(feature = "codex")]
async fn execute_rlm_query_codex(
    input: &RlmQueryInput,
    config: RlmConfig,
) -> Result<rlm::RlmResult, rlm::RlmError> {
    use rlm::CodexLlmClient;

    // Get workspace root from current directory
    let workspace_root = std::env::current_dir().unwrap_or_else(|_| "/tmp".into());

    let client = CodexLlmClient::new(workspace_root);
    let executor = PythonExecutor::new();

    let mut engine = RlmEngine::with_config(client, executor, config);

    // Set context if provided
    if let Some(ref ctx) = input.context {
        engine.set_context(Context::from_text(ctx.clone()));
    }

    // Choose execution mode
    if input.orchestrated.unwrap_or(false)
        || input
            .context
            .as_ref()
            .map(|c| c.len() > 50_000)
            .unwrap_or(false)
    {
        engine.run_orchestrated(&input.query).await
    } else {
        engine.run(&input.query).await
    }
}

async fn execute_rlm_query(args: &Value) -> Value {
    // Parse input
    let input: RlmQueryInput = match serde_json::from_value(args.clone()) {
        Ok(i) => i,
        Err(e) => {
            return json!({
                "content": [{
                    "type": "text",
                    "text": format!("Failed to parse rlm_query input: {}", e)
                }],
                "isError": true
            });
        }
    };

    let backend = RlmBackend::from_env();
    eprintln!("[rlm-mcp-server] Using backend: {:?}", backend);

    // Build config
    let mut config = RlmConfig::default();
    if let Some(max_iter) = input.max_iterations {
        config.max_iterations = max_iter;
    }

    // Execute with appropriate backend
    let result = match backend {
        #[cfg(feature = "codex")]
        RlmBackend::Codex => {
            execute_rlm_query_codex(&input, config).await
        }
        #[cfg(not(feature = "codex"))]
        RlmBackend::Codex => {
            return json!({
                "content": [{
                    "type": "text",
                    "text": "Codex backend requested but 'codex' feature not enabled. \
                            Rebuild with: cargo build -p rlm --features codex"
                }],
                "isError": true
            });
        }
        RlmBackend::Ollama => {
            execute_rlm_query_ollama(&input, config).await
        }
    };

    match result {
        Ok(rlm_result) => {
            json!({
                "content": [{
                    "type": "text",
                    "text": rlm_result.output
                }],
                "_meta": {
                    "iterations": rlm_result.iterations,
                    "execution_log_length": rlm_result.execution_log.len()
                }
            })
        }
        Err(e) => {
            json!({
                "content": [{
                    "type": "text",
                    "text": format!("RLM execution failed: {}", e)
                }],
                "isError": true
            })
        }
    }
}

async fn execute_rlm_fanout(args: &Value) -> Value {
    // Parse input
    let input: RlmFanoutInput = match serde_json::from_value(args.clone()) {
        Ok(i) => i,
        Err(e) => {
            return json!({
                "content": [{
                    "type": "text",
                    "text": format!("Failed to parse rlm_fanout input: {}", e)
                }],
                "isError": true
            });
        }
    };

    let workers = input.workers.unwrap_or(3);
    let venue = input.venue.as_deref().unwrap_or("local");

    // For now, implement simple local fanout by chunking the context
    // Full swarm integration would use frlm::FrlmConductor
    match venue {
        "local" => execute_local_fanout(&input.query, &input.context, workers).await,
        "swarm" => {
            // TODO: Integrate with frlm::FrlmConductor for swarm execution
            json!({
                "content": [{
                    "type": "text",
                    "text": "Swarm fanout not yet implemented. Use venue='local' for now."
                }],
                "isError": true
            })
        }
        "datacenter" => {
            // TODO: Integrate with datacenter execution
            json!({
                "content": [{
                    "type": "text",
                    "text": "Datacenter fanout not yet implemented. Use venue='local' for now."
                }],
                "isError": true
            })
        }
        _ => {
            json!({
                "content": [{
                    "type": "text",
                    "text": format!("Unknown venue: {}. Use 'local', 'swarm', or 'datacenter'.", venue)
                }],
                "isError": true
            })
        }
    }
}

async fn execute_local_fanout(query: &str, context: &str, workers: u32) -> Value {
    use rlm::chunking::{chunk_by_structure, detect_structure};

    // Chunk the context
    let structure = detect_structure(context);
    let chunks = chunk_by_structure(context, &structure, 8000, 200); // overlap of 200 chars

    if chunks.is_empty() {
        return json!({
            "content": [{
                "type": "text",
                "text": "No chunks generated from context"
            }],
            "isError": true
        });
    }

    // Limit to requested number of workers
    let chunks_to_process: Vec<_> = chunks.into_iter().take(workers as usize).collect();

    // Create router with auto-detected backends
    let (router, model) = create_router_with_backends().await;

    // Process chunks sequentially (could be parallelized with tokio::spawn)
    let mut worker_results = Vec::new();
    let mut all_findings = Vec::new();

    for (i, chunk) in chunks_to_process.iter().enumerate() {
        let client = LmRouterClient::new(router.clone(), &model);
        let executor = PythonExecutor::new();

        let mut engine = RlmEngine::new(client, executor);
        engine.set_context(Context::from_text(chunk.content.clone()));

        let section_name = chunk
            .section_context
            .as_deref()
            .unwrap_or("(untitled section)");

        let chunk_query = format!(
            "{}\n\nAnalyzing chunk {} of {}: {}",
            query,
            i + 1,
            chunks_to_process.len(),
            section_name
        );

        match engine.run(&chunk_query).await {
            Ok(result) => {
                worker_results.push(json!({
                    "worker_id": format!("worker_{}", i),
                    "answer": result.output.clone(),
                    "confidence": 0.8
                }));
                all_findings.push(format!(
                    "## Chunk {} ({})\n{}",
                    i + 1,
                    section_name,
                    result.output
                ));
            }
            Err(e) => {
                worker_results.push(json!({
                    "worker_id": format!("worker_{}", i),
                    "answer": format!("Error: {}", e),
                    "confidence": 0.0
                }));
            }
        }
    }

    // Synthesize results
    let synthesis = if all_findings.is_empty() {
        "No findings from any worker.".to_string()
    } else {
        format!(
            "# Analysis Results\n\nAnalyzed {} chunks for query: {}\n\n{}",
            worker_results.len(),
            query,
            all_findings.join("\n\n---\n\n")
        )
    };

    json!({
        "content": [{
            "type": "text",
            "text": synthesis
        }],
        "_meta": {
            "worker_results": worker_results,
            "total_workers": worker_results.len()
        }
    })
}

#[tokio::main]
async fn main() {
    // Initialize logging to stderr
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::WARN.into()),
        )
        .with_writer(std::io::stderr)
        .init();

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    // Read JSON-RPC requests line by line
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("Failed to read line: {}", e);
                continue;
            }
        };

        // Skip empty lines
        if line.trim().is_empty() {
            continue;
        }

        // Parse JSON request
        let request: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let error_response = json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": { "code": -32700, "message": format!("Parse error: {}", e) }
                });
                writeln!(stdout, "{}", error_response).ok();
                stdout.flush().ok();
                continue;
            }
        };

        // Handle the request
        let response = handle_request(&request).await;

        // Write response
        if let Err(e) = writeln!(stdout, "{}", response) {
            eprintln!("Failed to write response: {}", e);
        }
        stdout.flush().ok();
    }
}
