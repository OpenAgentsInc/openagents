//! Native coordination over admitted source owners and the existing CJ service.
pub mod book;
pub mod checks;
pub mod cli;
pub mod transport;
pub mod worker;
pub use operations::Limits as NativeLimits;
pub type Result<T> = std::result::Result<T, String>;
mod admin;
mod automation;
mod automation_source;
mod auxiliary;
mod background;
mod claims;
mod control;
mod dispatch;
mod findings;
mod integration;
mod operations;
mod proposals;
mod sources;
mod staging;
#[cfg(test)]
mod tests;
