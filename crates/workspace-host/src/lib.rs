//! Source-owned workspaces. Stored views never replace source admission.
pub mod artifact;
pub mod bridge;
pub mod checkpoint;
pub mod cli;
pub mod git;
pub mod host;
pub mod outbox;
pub mod projection;
pub type Result<T> = std::result::Result<T, String>;

#[cfg(test)]
mod tests;
