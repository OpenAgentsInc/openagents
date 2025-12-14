//! Wrappers for existing tools crate implementations.
//!
//! These wrappers adapt the synchronous, static tool implementations
//! from the `tools` crate to the async `Tool` trait.

mod bash;
mod edit;
mod find;
mod grep;
mod read;
mod write;

pub use bash::BashToolWrapper;
pub use edit::EditToolWrapper;
pub use find::FindToolWrapper;
pub use grep::GrepToolWrapper;
pub use read::ReadToolWrapper;
pub use write::WriteToolWrapper;

use crate::{BoxedTool, IntoBoxedTool};

/// Get all standard tool wrappers as boxed dynamic tools.
pub fn standard_tools() -> Vec<BoxedTool> {
    vec![
        BashToolWrapper.into_boxed(),
        ReadToolWrapper.into_boxed(),
        WriteToolWrapper.into_boxed(),
        EditToolWrapper.into_boxed(),
        GrepToolWrapper.into_boxed(),
        FindToolWrapper.into_boxed(),
    ]
}
