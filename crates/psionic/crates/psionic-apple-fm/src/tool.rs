use crate::contract::{AppleFmToolCallError, AppleFmToolDefinition};
use crate::structured::AppleFmGeneratedContent;

/// Reusable Rust-side tool contract for Apple FM sessions.
pub trait AppleFmTool: Send + Sync {
    /// Returns the stable tool definition registered with the bridge session.
    fn definition(&self) -> AppleFmToolDefinition;

    /// Executes the tool for one Apple FM tool invocation.
    fn call(&self, arguments: AppleFmGeneratedContent) -> Result<String, AppleFmToolCallError>;
}
