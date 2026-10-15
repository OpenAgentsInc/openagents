//! One work/Issue authority. Assignment never dispatches an execution.
pub mod authority;
pub mod cli;
pub mod domain;
pub mod execution;
pub mod github;
pub mod reader;
pub mod views;
pub type Result<T> = std::result::Result<T, String>;

mod access;

#[cfg(test)]
mod tests;

pub mod checks;

pub mod market;
