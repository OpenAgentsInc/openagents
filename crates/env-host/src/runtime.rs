//! Runtime-side admission uses the allocator's own grant and current authority.
use crate::{Result, allocator::Allocator};
use serde_json::Value;
/// Exact facts the runtime host must bind to its admitted operation. This is
/// not a transferable grant and does not authorize a different CJ target.
#[derive(Clone, Copy)]
pub struct RuntimeUse<'a> {
    pub attachment: &'a str,
    pub fingerprint: &'a str,
    pub grant: &'a Value,
    pub controller: &'a str,
    pub executor: &'a str,
    pub lease: &'a Value,
    pub materialization: &'a Value,
    pub engine: &'a Value,
}
impl Allocator {
    /// Hold the allocator and COORD fences through the runtime's disclosure.
    /// Operation-specific capability, input, and budget checks remain required.
    pub fn with_runtime_use(
        &self,
        request: &RuntimeUse<'_>,
        now: u64,
        effect: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        self.runtime_use(request, now, effect)
    }
}
