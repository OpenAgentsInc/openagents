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

/// A runtime's exact public operation and attachment pins. Private operation
/// inputs are delivered only after the coordinator's durable intake.
#[derive(Clone, Copy)]
pub struct RuntimeDispatch<'a> {
    pub attachment: &'a str,
    pub fingerprint: &'a str,
    pub grant: &'a Value,
    pub controller: &'a str,
    pub executor: &'a str,
    pub engine: &'a Value,
    pub target: &'a Value,
    pub intake: &'a nostr::domain::Event,
}

/// A borrowed admission that exists only inside the allocator/coordinator
/// callback. It cannot be constructed, cloned, or retained by the runtime.
pub struct RuntimeFence<'a> {
    pub(crate) request: RuntimeUse<'a>,
    pub(crate) deadline: u64,
    pub(crate) admitted_at: u64,
    pub(crate) started: std::time::Instant,
}
impl RuntimeFence<'_> {
    /// Validate exact session-side pins without reacquiring either outer lock.
    pub fn with_current(
        &self,
        request: &RuntimeUse<'_>,
        now: u64,
        effect: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        let current = now.max(
            self.admitted_at
                .saturating_add(self.started.elapsed().as_secs()),
        );
        if current >= self.deadline {
            return Err("stale: runtime execution expired before native disclosure".into());
        }
        let actual = &self.request;
        if request.attachment != actual.attachment
            || request.fingerprint != actual.fingerprint
            || request.grant != actual.grant
            || request.controller != actual.controller
            || request.executor != actual.executor
            || request.lease != actual.lease
            || request.materialization != actual.materialization
            || request.engine != actual.engine
        {
            return Err("identity_mismatch: runtime use differs from the fenced execution".into());
        }
        effect()
    }
    pub fn lease(&self) -> &Value {
        self.request.lease
    }
    pub fn materialization(&self) -> &Value {
        self.request.materialization
    }
}

impl Allocator {
    /// Hold both authorities through disclosure and dispatch of one exact
    /// registered external operation. Success does not settle the worker job.
    pub fn with_runtime_execution(
        &self,
        request: &RuntimeDispatch<'_>,
        now: u64,
        effect: &mut dyn FnMut(
            &RuntimeFence<'_>,
            &coordination_host::worker::Intake,
            u64,
        ) -> Result<Value>,
    ) -> Result<Value> {
        self.runtime_execution(request, now, effect)
    }
}
