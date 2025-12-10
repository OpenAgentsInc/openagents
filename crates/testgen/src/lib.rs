//! TestGen - Test Generation and Evolution System
//!
//! A Rust implementation of the TestGen flow for Terminal-Bench tasks.
//! Generates comprehensive test suites using iterative LLM-based generation
//! with category-based organization and quality evolution.
//!
//! # Architecture
//!
//! - **types**: Core domain types (TestGenConfig, GeneratedTest, etc.)
//! - **error**: Error types using thiserror
//! - **environment**: Environment context for test generation
//! - **analyzer**: Analysis functions for quality metrics
//! - **scoring**: 0-1000 scoring system
//! - **meta_reasoner**: Config evolution with guardrails
//! - **store**: SQLite persistence layer
//! - **generator**: Iterative test generation engine
//!
//! # Example
//!
//! ```no_run
//! use testgen::{TestGenerator, TestGenStore, IterationConfig};
//! use fm_bridge::FMClient;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let client = FMClient::new();
//!     let store = TestGenStore::open("testgen.db")?;
//!     let generator = TestGenerator::new(client);
//!
//!     // Generate tests for a task
//!     // let result = generator.generate_iteratively(...).await?;
//!
//!     Ok(())
//! }
//! ```

pub mod analyzer;
pub mod environment;
pub mod error;
pub mod formatter;
pub mod generator;
pub mod meta_reasoner;
pub mod scoring;
pub mod store;
pub mod types;

// Re-export main types
pub use analyzer::{
    analyze_category_distribution, analyze_testgen_run, calculate_category_balance,
    TestGenTrajectory,
};
pub use environment::{
    EnvironmentInfo, FileInfo, FilesInfo, PlatformInfo, ProhibitedTool, ToolsInfo,
};
pub use error::{Result, TestGenError};
pub use generator::{IterationConfig, TestGenEmitter, TestGenerator};
pub use meta_reasoner::{apply_config_change, validate_config_change, Guardrails};
pub use scoring::{compute_overall_score, score_testgen_run};
pub use store::TestGenStore;
pub use types::{
    generate_run_id, generate_session_id, ConfigChangeType, GeneratedTest, ModelType,
    ReflectionAction, ReflectionEntry, TestCategory, TestGenAnalysis, TestGenBestConfig,
    TestGenConfig, TestGenConfigChange, TestGenConfigInput, TestGenContext, TestGenEvolution,
    TestGenRun, TestGenRunInput, TestGenStats, TestGenTaskStats,
};

// Re-export formatter for pytest generation
pub use formatter::format_as_pytest;
