//! Native finite automation; original principals and owning dispatchers stay explicit.
pub type Result<T> = std::result::Result<T, String>;
pub mod host;

pub mod cli;
mod controls;
mod execution;
mod plans;
mod records;
mod schedule;
#[cfg(test)]
mod tests;
