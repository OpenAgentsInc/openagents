//! Native finite automation; original principals and owning dispatchers stay explicit.
pub type Result<T> = std::result::Result<T, String>;
pub mod host;

pub mod cli;
mod controls;
pub mod evaluator;
mod execution;
mod goal_plans;
mod goals;
mod plans;
mod polls;
mod records;
mod schedule;
pub mod source;
mod source_plans;
#[cfg(test)]
mod tests;
pub mod transport;

mod recovery;
