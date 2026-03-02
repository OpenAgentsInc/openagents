//! App-owned state modules extracted from `app_state.rs` for local ownership clarity.

pub mod alerts_recovery;
pub mod autopilot_goals;
pub mod earnings_gate;
pub mod goal_conditions;
pub mod goal_loop_executor;
pub mod goal_skill_resolver;
pub mod job_inbox;
pub mod operations;
pub mod provider_runtime;
pub mod swap_contract;
pub mod swap_quote_adapter;
