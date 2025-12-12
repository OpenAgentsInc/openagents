//! Repository module for issue storage
//!
//! This module contains the repository trait and error types.

mod error;
mod r#trait;

pub use error::{Result, TaskmasterError};
pub use r#trait::IssueRepository;
