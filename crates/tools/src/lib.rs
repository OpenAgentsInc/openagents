//! Core tools for OpenAgents
//!
//! Implements user stories TOOL-001 through TOOL-033:
//! - File reading (TOOL-001..004)
//! - File writing (TOOL-010..013)
//! - File search (TOOL-020..023)
//! - Shell execution (TOOL-030..033)
//!
//! # Example
//!
//! ```no_run
//! use tools::{ReadTool, WriteTool, EditTool, GrepTool, BashTool};
//!
//! // Read a file
//! let content = ReadTool::read("/path/to/file.rs", None, None).unwrap();
//! println!("{}", content.text);
//!
//! // Search for pattern
//! let matches = GrepTool::search("fn main", ".", None).unwrap();
//! for m in matches.matches {
//!     println!("{}:{}: {}", m.file, m.line, m.text);
//! }
//! ```

mod error;
mod read;
mod write;
mod edit;
mod grep;
mod find;
mod bash;

pub use error::*;
pub use read::*;
pub use write::*;
pub use edit::*;
pub use grep::*;
pub use find::*;
pub use bash::*;
