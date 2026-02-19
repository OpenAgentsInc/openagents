//! SwarmCompiler - Cost-efficient DSPy optimization
//!
//! Uses cheap Pylon swarm inference for bootstrap and premium models for validation.
//! Achieves ~96% cost reduction compared to premium-only approaches.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                    SwarmCompiler                         │
//! ├─────────────────────────────────────────────────────────┤
//! │  Phase 1: Bootstrap (Pylon swarm, ~10 msats/call)       │
//! │    - MIPROv2 candidate generation                        │
//! │    - Proxy metric evaluation                             │
//! ├─────────────────────────────────────────────────────────┤
//! │  Phase 2: Validate (Premium model, ~1000 msats/call)    │
//! │    - Full truth metric evaluation                        │
//! │    - Scorecard generation                                │
//! ├─────────────────────────────────────────────────────────┤
//! │  Phase 3: Promote (Gates)                                │
//! │    - PromotionManager evaluation                         │
//! │    - Manifest generation                                 │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Usage
//!
//! ```ignore
//! use dsrs::compiler::{SwarmCompiler, SwarmCompileConfig};
//! use dsrs::core::lm::PylonConfig;
//!
//! // Create LM providers
//! let bootstrap_lm = Arc::new(PylonLM::new(PylonConfig::swarm(...)));
//! let validation_lm = Arc::new(MockLM::new("codex"));
//!
//! // Create compiler
//! let mut compiler = SwarmCompiler::new(bootstrap_lm, validation_lm, scorer);
//!
//! // Compile a module
//! let result = compiler.compile(
//!     &mut my_module,
//!     trainset,
//!     &eval_tasks,
//!     SwarmCompileConfig::default(),
//! ).await?;
//!
//! println!("Compiled ID: {}", result.compiled_id());
//! println!("Total cost: {} msats", result.total_cost());
//! ```

mod budget;
mod provider;
mod result;
mod swarm_compiler;
mod trace_collector;

pub use budget::*;
pub use provider::*;
pub use result::*;
pub use swarm_compiler::*;
pub use trace_collector::*;
