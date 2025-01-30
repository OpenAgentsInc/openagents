pub mod generation;
pub mod parsing;
pub mod tests;
pub mod types;

pub use generation::generate_changes;
pub use parsing::parse_search_replace;
pub use types::{ChangeBlock, ChangeResponse};