//! An injected observer of explicitly admitted project supervisor records.

use coder_access::Code;
use coder_access::project::{Chunk, List, OriginalQuery, Page, Query};

/// Reading a graph never registers, claims, schedules, or dispatches its work.
/// Providers independently admit the device, workspace, and project aliases.
pub trait Projects: Send + Sync + std::fmt::Debug {
    fn list(&self, device: &str, workspace: &str) -> Result<List, Code>;
    fn read(&self, device: &str, query: &Query) -> Result<Page, Code>;
    fn original(&self, device: &str, query: &OriginalQuery) -> Result<Chunk, Code>;
}
