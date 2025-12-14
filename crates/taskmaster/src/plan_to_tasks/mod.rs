//! Plan-to-Tasks: Convert Claude plan files to taskmaster tasks
//!
//! This module provides functionality to:
//! 1. Discover plan files in ~/.claude/plans/
//! 2. Parse plans using Claude LLM to extract tasks
//! 3. Convert parsed tasks to taskmaster issues
//!
//! # Example
//!
//! ```no_run
//! use taskmaster::plan_to_tasks::{discover_plans, default_claude_dir, parse_plan_with_llm, convert_to_tasks};
//! use taskmaster::SqliteRepository;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Discover recent plans
//!     let plans = discover_plans(&default_claude_dir(), 3)?;
//!
//!     // Open taskmaster database
//!     let repo = SqliteRepository::open("taskmaster.db")?;
//!
//!     for plan in plans {
//!         // Parse with LLM
//!         let parsed = parse_plan_with_llm(&plan.content, &plan.name).await?;
//!
//!         // Convert to tasks
//!         let result = convert_to_tasks(&parsed, &plan.name, &repo, "tm", false)?;
//!
//!         println!("Created {} tasks from {}", result.created.len(), plan.name);
//!     }
//!
//!     Ok(())
//! }
//! ```

pub mod converter;
pub mod discovery;
pub mod parser;

pub use converter::{convert_to_tasks, print_summary, ConversionResult};
pub use discovery::{default_claude_dir, discover_plan_by_name, discover_plans, DiscoveryError, PlanFile};
pub use parser::{parse_plan_with_llm, ParsedPlan, ParsedTask, ParseError};
