//! Recursive Language Model (RLM) execution engine.
//!
//! RLMs enable LLMs to programmatically examine, decompose, and recursively
//! call themselves over input data. This replaces the canonical `llm.complete()`
//! call with an iterative prompt-execute-loop pattern.
//!
//! # Architecture
//!
//! ```text
//! User Query → RlmEngine → FM Bridge (LLM) → Parse Commands → Executor
//!                 ↑                                              ↓
//!                 └──────────── Execution Result ←───────────────┘
//!                               (loop until FINAL)
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use rlm::{RlmEngine, MockExecutor};
//! use fm_bridge::FMClient;
//!
//! let client = FMClient::new()?;
//! let executor = MockExecutor::new();
//! let engine = RlmEngine::new(client, executor);
//!
//! let result = engine.run("What is 2 + 2?").await?;
//! ```

mod command;
mod engine;
mod error;
mod executor;
mod mock_executor;
mod prompts;

pub use command::{Command, RunArgs};
pub use engine::{RlmConfig, RlmEngine, RlmResult};
pub use error::RlmError;
pub use executor::{ExecutionEnvironment, ExecutionResult};
pub use mock_executor::MockExecutor;
pub use prompts::SYSTEM_PROMPT;
