mod generation;
mod parsing;
#[cfg(test)]
mod tests;
mod types;

pub use generation::generate_changes;
pub use parsing::parse_search_replace;
pub use types::{ChangeBlock, ChangeResponse};
