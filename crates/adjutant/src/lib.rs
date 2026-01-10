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
pub mod cli;
pub mod claude_executor;
pub mod delegate;
pub mod dspy;
pub mod executor;
pub mod planner;
pub mod rlm_agent;
pub mod tiered;
pub mod tools;

use futures::StreamExt;
use oanix::{OanixManifest, WorkspaceManifest};
use std::path::PathBuf;
use thiserror::Error;
use tokio::sync::mpsc;

pub use auth::{get_claude_path, has_claude_cli};
pub use claude_executor::ClaudeExecutor;
pub use executor::TaskResult;
pub use planner::{Complexity, TaskPlan};
pub use rlm_agent::{rlm_agent_definition, rlm_agent_with_write_access};
pub use tiered::TieredExecutor;
pub use tools::{Tool, ToolRegistry};

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
    /// DSPy pipeline for complexity classification
    complexity_pipeline: dspy::ComplexityPipeline,
    /// DSPy pipeline for delegation decisions
    delegation_pipeline: dspy::DelegationPipeline,
    /// DSPy pipeline for RLM trigger decisions
    rlm_pipeline: dspy::RlmTriggerPipeline,
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
            complexity_pipeline: dspy::ComplexityPipeline::new(),
            delegation_pipeline: dspy::DelegationPipeline::new(),
            rlm_pipeline: dspy::RlmTriggerPipeline::new(),
        })
    }

    /// Get the workspace manifest.
    pub fn workspace(&self) -> Option<&WorkspaceManifest> {
        self.manifest.workspace.as_ref()
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
        let plan = self.plan_task(task).await?;
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
    async fn determine_use_rlm(&self, task: &Task, plan: &TaskPlan) -> bool {
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

        // Try DSPy pipeline first
        match self.rlm_pipeline.should_trigger(&input).await {
            Ok(result) if result.confidence > 0.7 => {
                tracing::debug!(
                    "DSPy RLM decision: use_rlm={}, confidence={:.2}, reasoning={}",
                    result.use_rlm,
                    result.confidence,
                    result.reasoning
                );
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
    async fn determine_delegation(&self, task: &Task, plan: &TaskPlan) -> dspy::DelegationResult {
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

        // Try DSPy pipeline first
        match self.delegation_pipeline.decide(&input).await {
            Ok(result) if result.confidence > 0.7 => {
                tracing::debug!(
                    "DSPy delegation decision: should_delegate={}, target={}, confidence={:.2}",
                    result.should_delegate,
                    result.delegation_target,
                    result.confidence
                );
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

    /// Stream response from local LM (llama.cpp/GPT-OSS).
    async fn stream_with_local_lm(
        &mut self,
        task: &Task,
        plan: &TaskPlan,
        token_tx: mpsc::UnboundedSender<String>,
    ) -> Result<TaskResult, AdjutantError> {
        // Build context from relevant files
        let context = self.build_context(plan).await?;

        // Build the current user prompt
        let user_prompt = format!(
            "## Task\n{}\n\n\
             ## Context\n{}\n\n\
             Provide a clear, helpful response.",
            task.to_prompt(),
            context
        );

        // Get llama.cpp endpoint
        let base_url = std::env::var("LLAMACPP_URL").unwrap_or_else(|_| {
            for port in [8080, 8000] {
                if std::net::TcpStream::connect_timeout(
                    &format!("127.0.0.1:{}", port).parse().unwrap(),
                    std::time::Duration::from_millis(100),
                )
                .is_ok()
                {
                    return format!("http://127.0.0.1:{}/v1", port);
                }
            }
            "http://127.0.0.1:8080/v1".to_string()
        });

        // Build OpenAI-compatible chat completion request with streaming
        let client = reqwest::Client::new();
        let system_prompt = "You are an AI coding assistant. Format your responses using proper markdown:\n\
            - Use blank lines between paragraphs and sections\n\
            - Use ## for headers, followed by a blank line\n\
            - Use proper list formatting with blank lines before and after lists\n\
            - Keep responses clear and well-structured";

        // Build messages array with conversation history
        let mut messages = vec![
            serde_json::json!({"role": "system", "content": system_prompt}),
        ];

        // Add conversation history for multi-turn context
        for turn in &self.conversation_history {
            messages.push(serde_json::json!({
                "role": turn.role,
                "content": turn.content
            }));
        }

        // Add current user message
        messages.push(serde_json::json!({"role": "user", "content": user_prompt}));

        let request_body = serde_json::json!({
            "model": "local",
            "messages": messages,
            "stream": true,
            "max_tokens": 4000,
            "temperature": 0.7
        });

        let response = client
            .post(format!("{}/chat/completions", base_url))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| AdjutantError::ExecutionFailed(format!("HTTP request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AdjutantError::ExecutionFailed(format!(
                "LLM request failed with status {}: {}",
                status, body
            )));
        }

        // Stream the response
        let mut full_response = String::new();
        let mut stream = response.bytes_stream();
        let mut buffer = String::new(); // Buffer for incomplete SSE lines

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| {
                AdjutantError::ExecutionFailed(format!("Stream read error: {}", e))
            })?;

            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);

            // Process complete lines from buffer
            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer[..newline_pos].trim();

                // Parse SSE events (data: {...})
                if let Some(data) = line.strip_prefix("data: ") {
                    if data != "[DONE]" {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                            if let Some(content) = json["choices"][0]["delta"]["content"].as_str() {
                                full_response.push_str(content);
                                let _ = token_tx.send(content.to_string());
                            }
                        }
                    }
                }

                // Remove processed line from buffer
                buffer = buffer[newline_pos + 1..].to_string();
            }
        }

        // Process any remaining data in buffer
        if !buffer.trim().is_empty() {
            if let Some(data) = buffer.trim().strip_prefix("data: ") {
                if data != "[DONE]" {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                        if let Some(content) = json["choices"][0]["delta"]["content"].as_str() {
                            full_response.push_str(content);
                            let _ = token_tx.send(content.to_string());
                        }
                    }
                }
            }
        }

        // Save this turn to conversation history for future context
        // Use a shorter version of the prompt (just the task) for history to save tokens
        self.conversation_history.push(ConversationTurn {
            role: "user".to_string(),
            content: task.to_prompt(),
        });
        self.conversation_history.push(ConversationTurn {
            role: "assistant".to_string(),
            content: full_response.clone(),
        });

        Ok(TaskResult {
            success: true,
            summary: full_response,
            modified_files: Vec::new(),
            commit_hash: None,
            error: None,
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
}
