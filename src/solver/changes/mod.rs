mod generation;
mod parsing;
mod types;
#[cfg(test)]
mod tests;

pub use generation::generate_changes;
pub use parsing::parse_search_replace;
pub use types::{ChangeResponse, ChangeBlock};