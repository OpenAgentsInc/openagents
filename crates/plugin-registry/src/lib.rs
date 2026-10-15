//! NIP-REG recommendations. Selection never installs, enables, or executes.

pub mod catalog;
pub mod cli;
pub mod publish;
pub mod source;
pub mod store;

pub type Result<T> = std::result::Result<T, String>;
pub const SCHEMA: &str = "openagents.registry.v1";
pub const CATALOG_LIMIT: usize = 1_048_576;
pub const SCHEMA_BYTES: &[u8] = include_bytes!("../../../nips/openagents/schemas/registry.v1.json");

#[cfg(test)]
mod tests;
