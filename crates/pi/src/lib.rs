//! Pi Coding Agent
//!
//! A Rust port of the Pi coding agent from pi-mono, providing an autonomous
//! agent runtime with tool execution, streaming responses, and session management.
//!
//! # Overview
//!
//! The Pi agent executes tasks by:
//! 1. Receiving a user prompt
//! 2. Calling an LLM to generate responses and tool calls
//! 3. Executing tool calls (bash, read, write, edit, etc.)
//! 4. Continuing until the task is complete
//!
//! # Example
//!
//! ```ignore
//! use pi::{PiAgent, PiConfig};
//! use futures::StreamExt;
//!
//! #[tokio::main]
//! async fn main() {
//!     // Create agent with default configuration
//!     let config = PiConfig::new("claude-sonnet-4-20250514")
//!         .working_directory(".")
//!         .max_turns(20);
//!
//!     let mut agent = PiAgent::anthropic(config).unwrap();
//!
//!     // Run with a prompt
//!     let mut stream = agent.run("Create a hello world program");
//!
//!     while let Some(event) = stream.next().await {
//!         match event {
//!             Ok(AgentEvent::TextDelta { text }) => print!("{}", text),
//!             Ok(AgentEvent::Completed { total_cost_usd, .. }) => {
//!                 println!("\nCost: ${:.4}", total_cost_usd);
//!             }
//!             Err(e) => eprintln!("Error: {}", e),
//!             _ => {}
//!         }
//!     }
//! }
//! ```
//!
//! # Features
//!
//! - **Multi-provider LLM support**: Use any provider from the `llm` crate
//! - **Built-in tools**: bash, read, write, edit (using `tools` crate)
//! - **Streaming events**: Real-time updates for UI integration
//! - **Cancellation**: Abort operations via cancellation token
//! - **Cost tracking**: Per-turn and cumulative cost calculation
//!
//! # Architecture
//!
//! The agent follows this execution model:
//!
//! ```text
//! User Prompt
//!      │
//!      ▼
//! ┌─────────┐
//! │ Agent   │◄─────────────────────────────┐
//! │ Loop    │                              │
//! └────┬────┘                              │
//!      │                                   │
//!      ▼                                   │
//! ┌─────────┐    ┌─────────┐    ┌────────┐│
//! │ Stream  │───▶│ Process │───▶│Execute ││
//! │ LLM     │    │ Response│    │ Tools  ││
//! └─────────┘    └─────────┘    └───┬────┘│
//!                                   │     │
//!                                   │     │
//!                                   ▼     │
//!                              Has more   │
//!                              tool calls?│
//!                                   │     │
//!                              Yes──┘     │
//!                              No───▶ Done│
//! ```

pub mod agent;
pub mod config;
pub mod context;
pub mod cost;
pub mod error;
pub mod events;
pub mod hooks;
pub mod prompt;
pub mod state;
pub mod tool_executor;

// Re-export main types
pub use agent::PiAgent;
pub use config::{OverflowStrategy, PiConfig, RetryConfig, DEFAULT_TOOLS};
pub use context::{ContextManager, TokenBudget};
pub use cost::{get_pricing, CostTracker, ModelPricing};
pub use error::{PiError, PiResult};
pub use events::{AgentEvent, AgentOutcome, StopReason};
pub use hooks::{
    CostLimitHook, FileTrackingHook, HookRegistry, HookResult, LogLevel, LoggingHook, PiHook,
};
pub use prompt::{format_tool_descriptions, SystemPromptBuilder, DEFAULT_BASE_INSTRUCTIONS};
pub use state::AgentState;
pub use tool_executor::{PiTool, ToolDefinition, ToolOutput, ToolRegistry};
