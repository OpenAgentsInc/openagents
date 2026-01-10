//! Adjutant - The agent that DOES THE WORK.
//!
//! Named after StarCraft's command & control AI.
//!
//! Adjutant is not just a router - it directly uses tools to accomplish tasks.
//! For complex work, it can delegate to Claude Code.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                      AUTOPILOT CLI                           │
//! │  (user-facing: `autopilot run`, `autopilot issue claim`)     │
//! └─────────────────────────────────────────────────────────────┘
//!                               │
//!                               ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │                   OANIX (background)                         │
//! │  (discovers environment, reads .openagents/)                 │
//! └─────────────────────────────────────────────────────────────┘
//!                               │
//!                               ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │                       ADJUTANT                               │
//! │  The actual agent that DOES THE WORK                         │
//! │  - Prioritizes Claude (Pro/Max) via claude-agent-sdk         │
//! │  - Falls back to Cerebras TieredExecutor                     │
//! │  - Uses tools directly (Read, Edit, Bash, Glob, Grep)        │
//! │  - Delegates to Claude Code for very complex work            │
//! │  - Uses RLM for large context analysis                       │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Execution Priority
//!
//! 1. **Claude Pro/Max** - If Claude CLI is installed, use `claude-agent-sdk`
//! 2. **Cerebras TieredExecutor** - If CEREBRAS_API_KEY is set
//! 3. **Analysis-only** - If neither is available

pub mod auth;
pub mod autopilot_loop;
pub mod cli;
pub mod claude_executor;
pub mod delegate;
pub mod dspy;
pub mod dspy_orchestrator;
pub mod executor;
pub mod planner;
pub mod rlm_agent;
pub mod tiered;
pub mod tools;

use dsrs::LM;
use oanix::{OanixManifest, WorkspaceManifest};
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::mpsc;

pub use auth::{get_claude_path, has_claude_cli};
pub use claude_executor::ClaudeExecutor;
pub use executor::TaskResult;
pub use planner::{Complexity, TaskPlan};
pub use rlm_agent::{rlm_agent_definition, rlm_agent_with_write_access};
pub use tiered::TieredExecutor;
pub use tools::{Tool, ToolRegistry};
pub use autopilot_loop::{
    AcpChannelOutput, AutopilotConfig, AutopilotLoop, AutopilotOutput, AutopilotResult, ChannelOutput,
    CliOutput, DspyStage, TodoStatus, TodoTask, Verification, DSPY_META_KEY,
};
pub use dspy_orchestrator::{AssessmentResult, DspyOrchestrator};

/// Errors that can occur during Adjutant operations.
#[derive(Error, Debug)]
pub enum AdjutantError {
    #[error("No workspace found - run from a project directory with .openagents/")]
    NoWorkspace,

    #[error("Task planning failed: {0}")]
    PlanningFailed(String),

    #[error("Task execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Tool error: {0}")]
    ToolError(String),

    #[error("Claude Code delegation failed: {0}")]
    DelegationFailed(String),

    #[error("RLM error: {0}")]
    RlmError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// A task for Adjutant to execute.
#[derive(Debug, Clone)]
pub struct Task {
    /// Task ID (e.g., issue number)
    pub id: String,
    /// Task title
    pub title: String,
    /// Task description
    pub description: String,
    /// Files to consider (optional hints)
    pub files: Vec<PathBuf>,
    /// Acceptance criteria
    pub acceptance_criteria: Vec<String>,
}

impl Task {
    /// Create a new task.
    pub fn new(id: impl Into<String>, title: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: description.into(),
            files: Vec::new(),
            acceptance_criteria: Vec::new(),
        }
    }

    /// Create a task from an issue.
    pub fn from_issue(issue: &issues::Issue) -> Self {
        Self {
            id: format!("#{}", issue.number),
            title: issue.title.clone(),
            description: issue.description.clone().unwrap_or_default(),
            files: Vec::new(),
            acceptance_criteria: Vec::new(),
        }
    }

    /// Convert task to a prompt for Claude.
    pub fn to_prompt(&self) -> String {
        let mut prompt = format!("Task {}: {}\n\n{}", self.id, self.title, self.description);

        if !self.acceptance_criteria.is_empty() {
            prompt.push_str("\n\nAcceptance criteria:\n");
            for criterion in &self.acceptance_criteria {
                prompt.push_str(&format!("- {}\n", criterion));
            }
        }

        prompt
    }
}

/// A turn in the conversation history.
#[derive(Debug, Clone)]
pub struct ConversationTurn {
    pub role: String,
    pub content: String,
}

/// Adjutant: The agent that DOES THE WORK.
pub struct Adjutant {
    /// Tool registry
    tools: ToolRegistry,
    /// OANIX manifest (compute, network, identity)
    manifest: OanixManifest,
    /// Workspace root
    workspace_root: PathBuf,
    /// Session ID for conversation continuity (used with Claude SDK)
    session_id: Option<String>,
    /// Conversation history for local LLMs (they don't have session resumption)
    conversation_history: Vec<ConversationTurn>,
    /// Cached LM for decision pipelines (lazily initialized)
    decision_lm: Option<Arc<LM>>,
    /// Training data collector for decision pipelines
    decision_training: Option<dspy::TrainingCollector>,
}

impl Adjutant {
    /// Create a new Adjutant from an OANIX manifest.
    pub fn new(manifest: OanixManifest) -> Result<Self, AdjutantError> {
        let workspace = manifest
            .workspace
            .as_ref()
            .ok_or(AdjutantError::NoWorkspace)?;

        let workspace_root = workspace.root.clone();
        let tools = ToolRegistry::new(&workspace_root);

        Ok(Self {
            tools,
            manifest,
            workspace_root,
            session_id: None,
            conversation_history: Vec::new(),
            decision_lm: None,
            decision_training: dspy::TrainingCollector::new(true).ok(),
        })
    }

    /// Get the workspace manifest.
    pub fn workspace(&self) -> Option<&WorkspaceManifest> {
        self.manifest.workspace.as_ref()
    }

    /// Get the full OANIX manifest.
    pub fn manifest(&self) -> &OanixManifest {
        &self.manifest
    }

    /// Get the tool registry.
    pub fn tools(&self) -> &ToolRegistry {
        &self.tools
    }

    /// Get the cached decision LM if available.
    pub fn decision_lm(&self) -> Option<Arc<LM>> {
        self.decision_lm.clone()
    }

    /// Set the session ID for conversation continuity.
    pub fn set_session_id(&mut self, id: String) {
        self.session_id = Some(id);
    }

    /// Get the current session ID.
    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    /// Set conversation history (for resuming with local LLMs).
    pub fn set_conversation_history(&mut self, history: Vec<ConversationTurn>) {
        self.conversation_history = history;
    }

    /// Get the conversation history.
    pub fn conversation_history(&self) -> &[ConversationTurn] {
        &self.conversation_history
    }

    /// Get or create the cached LM for decision pipelines.
    async fn get_or_create_decision_lm(&mut self) -> Option<Arc<LM>> {
        if self.decision_lm.is_none() {
            match dspy::lm_config::get_planning_lm().await {
                Ok(lm) => {
                    tracing::debug!("Decision LM initialized");
                    self.decision_lm = Some(lm);
                }
                Err(e) => {
                    tracing::debug!("No LM available for decisions: {}", e);
                }
            }
        }
        self.decision_lm.clone()
    }

    /// Execute a task - Adjutant does the work itself.
    ///
    /// This method determines the best execution strategy based on:
    /// - Task complexity (from planner)
    /// - Available backends (Claude CLI, Cerebras, etc.)
    /// - Context size (for RLM routing)
    /// - Task description keywords (analyze, recursive, etc.)
    ///
    /// Uses DSPy pipelines for intelligent routing decisions with legacy fallback.
    pub async fn execute(&mut self, task: &Task) -> Result<TaskResult, AdjutantError> {
        tracing::info!("Adjutant analyzing task: {}", task.title);

        // 1. Plan the task
        let mut plan = self.plan_task(task).await?;

        // 1b. Override complexity with DSPy classification (if available and confident)
        plan.complexity = self.determine_complexity_dspy(task, &plan).await;

        tracing::info!(
            "Plan: {} files, complexity {:?}",
            plan.files.len(),
            plan.complexity
        );

        // 2. Determine if RLM mode should be used (DSPy-first with fallback)
        let use_rlm = self.determine_use_rlm(task, &plan).await;

        // 3. Check which LM provider to use (respects priority: LlamaCpp > Claude > others)
        let provider = dspy::lm_config::detect_provider();
        tracing::info!("Adjutant: using LM provider: {:?}", provider);

        match provider {
            // Use local LlamaCpp/GPT-OSS - execute with local LM
            Some(dspy::lm_config::LmProvider::LlamaCpp) => {
                tracing::info!("Executing with local llama.cpp/GPT-OSS");
                return self.execute_with_local_lm(task, &plan).await;
            }
            // Use Claude SDK
            Some(dspy::lm_config::LmProvider::ClaudeSdk) => {
                let context = self.build_context(&plan).await?;
                let executor = ClaudeExecutor::new(&self.workspace_root);

                if use_rlm {
                    tracing::info!("Using Claude with RLM support for complex analysis");
                    let enable_rlm_tools = std::env::var("ADJUTANT_ENABLE_RLM")
                        .map(|v| v == "1" || v.to_lowercase() == "true")
                        .unwrap_or(true);
                    return executor.execute_with_rlm(task, &context, enable_rlm_tools).await;
                }

                tracing::info!("Using Claude standard execution");
                return executor.execute(task, &context, &mut self.tools).await;
            }
            // Other providers - use local tools
            _ => {}
        }

        // 4. Check delegation decision (DSPy-first with fallback)
        let delegation = self.determine_delegation(task, &plan).await;

        if delegation.should_delegate {
            match delegation.delegation_target.as_str() {
                "claude_code" => {
                    tracing::info!("DSPy: delegating to Claude Code (confidence: {:.2})", delegation.confidence);
                    return self.delegate_to_claude_code(task).await;
                }
                "rlm" => {
                    tracing::info!("DSPy: using RLM delegation (confidence: {:.2})", delegation.confidence);
                    return self.execute_with_rlm_delegate(task, &plan).await;
                }
                _ => {
                    // local_tools - fall through to execute_with_tools
                }
            }
        }

        // 5. Legacy fallback: Check complexity for delegation or RLM
        // (in case DSPy pipeline didn't recommend delegation but legacy rules would)
        if plan.complexity >= Complexity::High || plan.files.len() > 20 {
            tracing::info!("Legacy fallback: complexity high - delegating to Claude Code");
            return self.delegate_to_claude_code(task).await;
        }

        if plan.estimated_tokens > 100_000 {
            tracing::info!("Legacy fallback: context too large - using RLM");
            return self.execute_with_rlm_delegate(task, &plan).await;
        }

        // 6. Do the work myself using tools
        tracing::info!("Executing with local tools");
        self.execute_with_tools(task, &plan).await
    }

    /// Determine if RLM should be used (DSPy-first with legacy fallback).
    async fn determine_use_rlm(&mut self, task: &Task, plan: &TaskPlan) -> bool {
        let complexity_str = match plan.complexity {
            Complexity::Low => "Low",
            Complexity::Medium => "Medium",
            Complexity::High => "High",
            Complexity::VeryHigh => "VeryHigh",
        };

        let input = dspy::RlmTriggerInput {
            task_description: task.description.clone(),
            complexity: complexity_str.to_string(),
            estimated_tokens: plan.estimated_tokens,
        };

        // Get or create cached LM, create ephemeral pipeline
        let lm = self.get_or_create_decision_lm().await;
        let pipeline = match lm {
            Some(lm) => dspy::RlmTriggerPipeline::with_lm(lm),
            None => dspy::RlmTriggerPipeline::new(),
        };

        // Try DSPy pipeline first
        match pipeline.should_trigger(&input).await {
            Ok(result) if result.confidence > 0.7 => {
                tracing::debug!(
                    "DSPy RLM decision: use_rlm={}, confidence={:.2}, reasoning={}",
                    result.use_rlm,
                    result.confidence,
                    result.reasoning
                );

                // Record training example
                if let Some(ref mut collector) = self.decision_training {
                    let _ = collector.record_rlm_trigger(dspy::RlmTriggerTrainingExample {
                        task_description: task.description.clone(),
                        complexity: complexity_str.to_string(),
                        estimated_tokens: plan.estimated_tokens,
                        use_rlm: result.use_rlm,
                        confidence: result.confidence,
                    });
                }

                result.use_rlm
            }
            Ok(result) => {
                tracing::debug!(
                    "DSPy RLM confidence too low ({:.2}), using legacy fallback",
                    result.confidence
                );
                self.should_use_rlm(task, plan)
            }
            Err(e) => {
                tracing::debug!("DSPy RLM pipeline failed: {}, using legacy fallback", e);
                self.should_use_rlm(task, plan)
            }
        }
    }

    /// Determine delegation decision (DSPy-first with legacy fallback).
    async fn determine_delegation(&mut self, task: &Task, plan: &TaskPlan) -> dspy::DelegationResult {
        let complexity_str = match plan.complexity {
            Complexity::Low => "Low",
            Complexity::Medium => "Medium",
            Complexity::High => "High",
            Complexity::VeryHigh => "VeryHigh",
        };

        let input = dspy::DelegationInput {
            task_description: task.description.clone(),
            complexity: complexity_str.to_string(),
            file_count: plan.files.len() as u32,
            estimated_tokens: plan.estimated_tokens,
        };

        // Get or create cached LM, create ephemeral pipeline
        let lm = self.get_or_create_decision_lm().await;
        let pipeline = match lm {
            Some(lm) => dspy::DelegationPipeline::with_lm(lm),
            None => dspy::DelegationPipeline::new(),
        };

        // Try DSPy pipeline first
        match pipeline.decide(&input).await {
            Ok(result) if result.confidence > 0.7 => {
                tracing::debug!(
                    "DSPy delegation decision: should_delegate={}, target={}, confidence={:.2}",
                    result.should_delegate,
                    result.delegation_target,
                    result.confidence
                );

                // Record training example
                if let Some(ref mut collector) = self.decision_training {
                    let _ = collector.record_delegation(dspy::DelegationTrainingExample {
                        task_description: task.description.clone(),
                        complexity: complexity_str.to_string(),
                        file_count: plan.files.len() as u32,
                        estimated_tokens: plan.estimated_tokens,
                        should_delegate: result.should_delegate,
                        delegation_target: result.delegation_target.clone(),
                        confidence: result.confidence,
                    });
                }

                result
            }
            Ok(result) => {
                tracing::debug!(
                    "DSPy delegation confidence too low ({:.2}), using legacy fallback",
                    result.confidence
                );
                // Return default (no delegation) - legacy rules will be checked
                dspy::DelegationResult::default()
            }
            Err(e) => {
                tracing::debug!("DSPy delegation pipeline failed: {}, using legacy fallback", e);
                // Return default (no delegation) - legacy rules will be checked
                dspy::DelegationResult::default()
            }
        }
    }

    /// Execute a task with streaming support for local LM.
    ///
    /// This method streams tokens through the provided channel for real-time display.
    /// Returns the complete result when done.
    pub async fn execute_streaming(
        &mut self,
        task: &Task,
        token_tx: mpsc::UnboundedSender<String>,
    ) -> Result<TaskResult, AdjutantError> {
        tracing::info!("Adjutant streaming task: {}", task.title);

        // 1. Plan the task
        let plan = self.plan_task(task).await?;

        // 2. Check which LM provider to use
        let provider = dspy::lm_config::detect_provider();

        match provider {
            // Use local LlamaCpp/GPT-OSS with streaming
            Some(dspy::lm_config::LmProvider::LlamaCpp) => {
                tracing::info!("Streaming with local llama.cpp/GPT-OSS");
                return self.stream_with_local_lm(task, &plan, token_tx).await;
            }
            // For other providers, fall back to non-streaming
            _ => {
                let result = self.execute(task).await?;
                // Send the complete result as a single chunk
                let _ = token_tx.send(result.summary.clone());
                return Ok(result);
            }
        }
    }

    /// Stream response from local LM (llama.cpp/GPT-OSS) with tool calling support.
    ///
    /// This uses the gptoss Responses API which supports tool calling, enabling
    /// the LLM to actually read files, edit files, run commands, etc.
    async fn stream_with_local_lm(
        &mut self,
        task: &Task,
        plan: &TaskPlan,
        token_tx: mpsc::UnboundedSender<String>,
    ) -> Result<TaskResult, AdjutantError> {
        use gpt_oss::{
            GptOssClient, GptOssResponsesRequest, GptOssToolDefinition, GptOssToolFunction,
            GptOssToolChoice,
        };
        use std::path::Path;

        // Build initial context from relevant files
        let mut context = self.build_context(plan).await?;

        // Truncate context to fit in local LLM context window
        const MAX_CONTEXT_CHARS: usize = 16_000;
        if context.len() > MAX_CONTEXT_CHARS {
            tracing::warn!(
                "Context too large ({} chars), truncating to {}",
                context.len(),
                MAX_CONTEXT_CHARS
            );
            context.truncate(MAX_CONTEXT_CHARS);
            context.push_str("\n\n[... context truncated ...]");
        }

        // Define available tools for the LLM
        let tools = vec![
            GptOssToolDefinition {
                tool_type: "function".to_string(),
                function: GptOssToolFunction {
                    name: "read_file".to_string(),
                    description: Some("Read the contents of a file".to_string()),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "Path to the file to read (relative to workspace root)"
                            }
                        },
                        "required": ["path"]
                    }),
                },
            },
            GptOssToolDefinition {
                tool_type: "function".to_string(),
                function: GptOssToolFunction {
                    name: "edit_file".to_string(),
                    description: Some("Edit a file by replacing old_string with new_string. The old_string must be unique in the file.".to_string()),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "Path to the file to edit"
                            },
                            "old_string": {
                                "type": "string",
                                "description": "The exact string to find and replace (must be unique)"
                            },
                            "new_string": {
                                "type": "string",
                                "description": "The string to replace it with"
                            }
                        },
                        "required": ["path", "old_string", "new_string"]
                    }),
                },
            },
            GptOssToolDefinition {
                tool_type: "function".to_string(),
                function: GptOssToolFunction {
                    name: "write_file".to_string(),
                    description: Some("Write content to a new file (creates parent directories if needed)".to_string()),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "Path to the file to write"
                            },
                            "content": {
                                "type": "string",
                                "description": "The content to write to the file"
                            }
                        },
                        "required": ["path", "content"]
                    }),
                },
            },
            GptOssToolDefinition {
                tool_type: "function".to_string(),
                function: GptOssToolFunction {
                    name: "bash".to_string(),
                    description: Some("Execute a bash command in the workspace directory".to_string()),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "command": {
                                "type": "string",
                                "description": "The bash command to execute"
                            }
                        },
                        "required": ["command"]
                    }),
                },
            },
            GptOssToolDefinition {
                tool_type: "function".to_string(),
                function: GptOssToolFunction {
                    name: "glob".to_string(),
                    description: Some("Find files matching a glob pattern (e.g., '**/*.rs', 'src/*.ts')".to_string()),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "pattern": {
                                "type": "string",
                                "description": "The glob pattern to match files"
                            }
                        },
                        "required": ["pattern"]
                    }),
                },
            },
            GptOssToolDefinition {
                tool_type: "function".to_string(),
                function: GptOssToolFunction {
                    name: "grep".to_string(),
                    description: Some("Search for a pattern in files using ripgrep".to_string()),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "pattern": {
                                "type": "string",
                                "description": "The regex pattern to search for"
                            },
                            "path": {
                                "type": "string",
                                "description": "Optional path to search in (defaults to workspace root)"
                            }
                        },
                        "required": ["pattern"]
                    }),
                },
            },
            GptOssToolDefinition {
                tool_type: "function".to_string(),
                function: GptOssToolFunction {
                    name: "task_complete".to_string(),
                    description: Some("Signal that the task is complete. Call this when you have finished the requested work.".to_string()),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "summary": {
                                "type": "string",
                                "description": "A summary of what was done"
                            },
                            "success": {
                                "type": "boolean",
                                "description": "Whether the task was completed successfully"
                            }
                        },
                        "required": ["summary", "success"]
                    }),
                },
            },
        ];

        // Build initial prompt
        let system_prompt = format!(
            "You are an AI coding assistant with access to tools. Use the tools to complete the task.\n\n\
             Workspace root: {}\n\n\
             Available tools:\n\
             - read_file: Read file contents\n\
             - edit_file: Edit a file (old_string must be unique)\n\
             - write_file: Create a new file\n\
             - bash: Run bash commands\n\
             - glob: Find files by pattern\n\
             - grep: Search file contents\n\
             - task_complete: Signal when done\n\n\
             IMPORTANT: When you've completed the task, call task_complete with a summary.\n\
             If you can't complete the task, still call task_complete with success=false.",
            self.workspace_root.display()
        );

        let user_prompt = format!(
            "## Task\n{}\n\n## Initial Context\n{}",
            task.to_prompt(),
            context
        );

        // Get gptoss endpoint (same as llama.cpp)
        let base_url = std::env::var("GPT_OSS_URL")
            .or_else(|_| std::env::var("LLAMACPP_URL"))
            .unwrap_or_else(|_| {
                for port in [8080, 8000] {
                    if std::net::TcpStream::connect_timeout(
                        &format!("127.0.0.1:{}", port).parse().unwrap(),
                        std::time::Duration::from_millis(100),
                    )
                    .is_ok()
                    {
                        return format!("http://127.0.0.1:{}", port);
                    }
                }
                "http://127.0.0.1:8000".to_string()
            });

        let client = GptOssClient::with_base_url(&base_url)
            .map_err(|e| AdjutantError::ExecutionFailed(format!("Failed to create gptoss client: {}", e)))?;

        // Build conversation as input
        let mut messages: Vec<serde_json::Value> = vec![
            serde_json::json!({"role": "system", "content": system_prompt}),
        ];

        // Add any prior conversation history
        for turn in &self.conversation_history {
            messages.push(serde_json::json!({
                "role": turn.role,
                "content": turn.content
            }));
        }

        messages.push(serde_json::json!({"role": "user", "content": user_prompt}));

        // Track state
        let mut modified_files: Vec<String> = Vec::new();
        let mut full_response = String::new();
        let mut task_completed = false;
        let mut task_success = false;
        let mut iteration = 0;
        const MAX_TOOL_ITERATIONS: usize = 20;

        // Tool calling loop
        while !task_completed && iteration < MAX_TOOL_ITERATIONS {
            iteration += 1;
            tracing::debug!("Tool iteration {}/{}", iteration, MAX_TOOL_ITERATIONS);

            // Create request with tools
            let request = GptOssResponsesRequest {
                model: "local".to_string(),
                input: serde_json::json!(messages),
                tools: Some(tools.clone()),
                tool_choice: Some(GptOssToolChoice::Mode("auto".to_string())),
                reasoning: None,
                max_output_tokens: Some(4000),
                temperature: Some(0.7),
                top_p: None,
                stop: None,
                stream: false,
                extra: std::collections::HashMap::new(),
            };

            // Call the Responses API
            let response = client.responses(request).await.map_err(|e| {
                AdjutantError::ExecutionFailed(format!("gptoss responses API failed: {}", e))
            })?;

            // Extract text output
            let text = response.output_text();
            if !text.is_empty() {
                let _ = token_tx.send(text.clone());
                full_response.push_str(&text);
                full_response.push('\n');
            }

            // Extract tool calls
            let tool_calls = response.tool_calls();

            if tool_calls.is_empty() {
                // No tool calls - LLM is done generating
                tracing::debug!("No tool calls, LLM finished generating");
                break;
            }

            // Execute each tool call
            for tool_call in tool_calls {
                let tool_name = &tool_call.name;
                let args = &tool_call.arguments;

                let _ = token_tx.send(format!("\n**[Tool: {}]**\n", tool_name));

                let tool_result = match tool_name.as_str() {
                    "read_file" => {
                        let path = args["path"].as_str().unwrap_or("");
                        let _ = token_tx.send(format!("Reading: {}\n", path));
                        self.tools.read(Path::new(path)).await?
                    }
                    "edit_file" => {
                        let path = args["path"].as_str().unwrap_or("");
                        let old_string = args["old_string"].as_str().unwrap_or("");
                        let new_string = args["new_string"].as_str().unwrap_or("");
                        let _ = token_tx.send(format!("Editing: {}\n", path));
                        let result = self.tools.edit(Path::new(path), old_string, new_string).await?;
                        if result.success {
                            modified_files.push(path.to_string());
                        }
                        result
                    }
                    "write_file" => {
                        let path = args["path"].as_str().unwrap_or("");
                        let content = args["content"].as_str().unwrap_or("");
                        let _ = token_tx.send(format!("Writing: {}\n", path));
                        let result = self.tools.write(Path::new(path), content).await?;
                        if result.success {
                            modified_files.push(path.to_string());
                        }
                        result
                    }
                    "bash" => {
                        let command = args["command"].as_str().unwrap_or("");
                        let _ = token_tx.send(format!("Running: {}\n", command));
                        self.tools.bash(command).await?
                    }
                    "glob" => {
                        let pattern = args["pattern"].as_str().unwrap_or("");
                        let _ = token_tx.send(format!("Globbing: {}\n", pattern));
                        self.tools.glob(pattern).await?
                    }
                    "grep" => {
                        let pattern = args["pattern"].as_str().unwrap_or("");
                        let path = args.get("path").and_then(|p| p.as_str());
                        let _ = token_tx.send(format!("Searching: {}\n", pattern));
                        self.tools.grep(pattern, path.map(Path::new)).await?
                    }
                    "task_complete" => {
                        let summary = args["summary"].as_str().unwrap_or("Task completed");
                        task_success = args["success"].as_bool().unwrap_or(false);
                        task_completed = true;
                        let _ = token_tx.send(format!("\n**Task Complete**: {}\n", summary));
                        full_response.push_str(&format!("\n\n## Summary\n{}", summary));
                        tools::ToolOutput::success(summary)
                    }
                    _ => {
                        tools::ToolOutput::failure(format!("Unknown tool: {}", tool_name))
                    }
                };

                // Show tool result (truncated)
                let result_preview = if tool_result.content.len() > 500 {
                    format!("{}...[truncated]", &tool_result.content[..500])
                } else {
                    tool_result.content.clone()
                };

                if tool_result.success {
                    let _ = token_tx.send(format!("Result: {}\n", result_preview));
                } else {
                    let _ = token_tx.send(format!(
                        "Error: {}\n",
                        tool_result.error.as_deref().unwrap_or("Unknown error")
                    ));
                }

                // Add tool result to messages for next iteration
                let tool_result_msg = if tool_result.success {
                    serde_json::json!({
                        "role": "tool",
                        "tool_call_id": tool_call.id,
                        "content": tool_result.content
                    })
                } else {
                    serde_json::json!({
                        "role": "tool",
                        "tool_call_id": tool_call.id,
                        "content": format!("Error: {}", tool_result.error.unwrap_or_default())
                    })
                };
                messages.push(tool_result_msg);
            }
        }

        if iteration >= MAX_TOOL_ITERATIONS && !task_completed {
            let _ = token_tx.send("\n**Max tool iterations reached**\n".to_string());
        }

        // Save conversation history
        self.conversation_history.push(ConversationTurn {
            role: "user".to_string(),
            content: task.to_prompt(),
        });
        self.conversation_history.push(ConversationTurn {
            role: "assistant".to_string(),
            content: full_response.clone(),
        });

        // Determine success: task_complete called with success=true, OR files were modified
        let success = task_success || !modified_files.is_empty();

        Ok(TaskResult {
            success,
            summary: full_response,
            modified_files,
            commit_hash: None,
            error: if !success && !task_completed {
                Some("Task did not complete - no files modified and task_complete not called".to_string())
            } else {
                None
            },
            session_id: self.session_id.clone(),
        })
    }

    /// Execute task using local LM (llama.cpp/GPT-OSS).
    async fn execute_with_local_lm(
        &mut self,
        task: &Task,
        plan: &TaskPlan,
    ) -> Result<TaskResult, AdjutantError> {
        // Build context from relevant files
        let context = self.build_context(plan).await?;

        // Create LM and execute
        let lm = dspy::lm_config::create_lm(&dspy::lm_config::LmProvider::LlamaCpp)
            .await
            .map_err(|e| AdjutantError::ExecutionFailed(format!("Failed to create LM: {}", e)))?;

        // Build prompt with context
        let prompt = format!(
            "You are an AI coding assistant. Complete the following task.\n\n\
             ## Task\n{}\n\n\
             ## Context\n{}\n\n\
             Provide a clear, helpful response.",
            task.to_prompt(),
            context
        );

        // Execute with local LM using Chat API
        use dsrs::{Chat, Message};
        let chat = Chat::new(vec![
            Message::system("You are an AI coding assistant."),
            Message::user(&prompt),
        ]);

        let response = lm.call(chat, vec![])
            .await
            .map_err(|e| AdjutantError::ExecutionFailed(format!("LM call failed: {}", e)))?;

        // Extract text from response
        let summary = response.output.content().to_string();

        Ok(TaskResult {
            success: true,
            summary,
            modified_files: Vec::new(),
            commit_hash: None,
            error: None,
            session_id: self.session_id.clone(),
        })
    }

    /// Determine if RLM mode should be used for a task.
    fn should_use_rlm(&self, task: &Task, plan: &TaskPlan) -> bool {
        // High complexity tasks benefit from RLM
        if plan.complexity >= Complexity::High {
            return true;
        }

        // Large context benefits from RLM's orchestrated analysis
        if plan.estimated_tokens > 50_000 {
            return true;
        }

        // Check task description for RLM-friendly keywords
        let description_lower = task.description.to_lowercase();
        let rlm_keywords = [
            "analyze",
            "recursive",
            "investigate",
            "find all",
            "security",
            "audit",
            "review",
            "deep dive",
            "comprehensive",
        ];

        for keyword in &rlm_keywords {
            if description_lower.contains(keyword) {
                return true;
            }
        }

        false
    }

    /// Build context string from planned files.
    async fn build_context(&mut self, plan: &TaskPlan) -> Result<String, AdjutantError> {
        let mut context = String::new();
        for file in &plan.files {
            let result = self.tools.read(file).await.map_err(|e| {
                AdjutantError::ToolError(format!("Failed to read {}: {}", file.display(), e))
            })?;
            if result.success {
                context.push_str(&format!(
                    "\n--- {} ---\n{}\n",
                    file.display(),
                    result.content
                ));
            }
        }
        Ok(context)
    }

    /// Plan a task - analyze what needs to be done.
    async fn plan_task(&self, task: &Task) -> Result<TaskPlan, AdjutantError> {
        planner::plan_task(&self.tools, &self.workspace_root, task).await
    }

    /// Execute task using local tools.
    async fn execute_with_tools(
        &mut self,
        task: &Task,
        plan: &TaskPlan,
    ) -> Result<TaskResult, AdjutantError> {
        executor::execute_with_tools(&mut self.tools, &self.workspace_root, task, plan).await
    }

    /// Delegate complex work to Claude Code.
    async fn delegate_to_claude_code(&self, task: &Task) -> Result<TaskResult, AdjutantError> {
        delegate::delegate_to_claude_code(&self.workspace_root, task).await
    }

    /// Execute task using RLM delegate for large context (fallback when Claude CLI not available).
    async fn execute_with_rlm_delegate(
        &self,
        task: &Task,
        plan: &TaskPlan,
    ) -> Result<TaskResult, AdjutantError> {
        delegate::execute_with_rlm(&self.workspace_root, task, plan).await
    }

    /// Determine complexity using DSPy pipeline (with legacy fallback).
    async fn determine_complexity_dspy(&mut self, task: &Task, plan: &TaskPlan) -> Complexity {
        let keywords = extract_complexity_keywords(&task.description);
        let input = dspy::ComplexityInput {
            task_description: task.description.clone(),
            file_count: plan.files.len() as u32,
            estimated_tokens: plan.estimated_tokens,
            keywords: keywords.clone(),
        };

        // Get or create cached LM, create ephemeral pipeline
        let lm = self.get_or_create_decision_lm().await;
        let pipeline = match lm {
            Some(lm) => dspy::ComplexityPipeline::with_lm(lm),
            None => dspy::ComplexityPipeline::new(),
        };

        // Try DSPy pipeline first
        match pipeline.classify(&input).await {
            Ok(result) if result.confidence > 0.7 => {
                tracing::debug!(
                    "DSPy complexity decision: complexity={}, confidence={:.2}, reasoning={}",
                    result.complexity,
                    result.confidence,
                    result.reasoning
                );

                // Record training example
                if let Some(ref mut collector) = self.decision_training {
                    let _ = collector.record_complexity(dspy::ComplexityTrainingExample {
                        task_description: task.description.clone(),
                        file_count: plan.files.len() as u32,
                        estimated_tokens: plan.estimated_tokens,
                        keywords,
                        expected_complexity: result.complexity.clone(),
                        confidence: result.confidence,
                    });
                }

                parse_complexity(&result.complexity)
            }
            Ok(result) => {
                tracing::debug!(
                    "DSPy complexity confidence too low ({:.2}), using legacy fallback",
                    result.confidence
                );
                planner::determine_complexity(&plan.files, plan.estimated_tokens, &task.description)
            }
            Err(e) => {
                tracing::debug!("DSPy complexity pipeline failed: {}, using legacy fallback", e);
                planner::determine_complexity(&plan.files, plan.estimated_tokens, &task.description)
            }
        }
    }
}

/// Parse complexity string from DSPy output to Complexity enum.
fn parse_complexity(s: &str) -> Complexity {
    match s.to_lowercase().as_str() {
        "low" => Complexity::Low,
        "medium" => Complexity::Medium,
        "high" => Complexity::High,
        "veryhigh" | "very_high" | "very high" => Complexity::VeryHigh,
        _ => Complexity::Medium, // Default fallback
    }
}

/// Extract complexity-relevant keywords from task description.
fn extract_complexity_keywords(description: &str) -> Vec<String> {
    let keywords = [
        "refactor", "rewrite", "migrate", "architect", "security", "audit",
    ];
    let lower = description.to_lowercase();
    keywords
        .iter()
        .filter(|k| lower.contains(*k))
        .map(|k| k.to_string())
        .collect()
}
