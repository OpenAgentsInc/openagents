//! The task owner a host hands admitted task operations to.
//!
//! The host checks the device's grant and the `operate` right first; the
//! owner then records the effect durably. Every call carries the NIP-HOST
//! request ID as its idempotency key, so a retry after an uncertain save
//! repeats the same logical operation rather than minting a new one.
//!
//! Creating a task records intent only. It grants no execution authority:
//! the local task owner still needs its own explicit execution grant before
//! anything runs. Steering and cancelling follow the CTRL semantics of the
//! local owner: a steer records a replacement instruction and supersedes a
//! running context, and a cancel requests a stop.

use coder_access::Code;
use coder_access::protocol::TaskCreate;
use nostr::activity_summary::Phase;

/// A task after an accepted operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskRef {
    /// The host-issued task ID: 64 lowercase hexadecimal characters.
    pub task: String,
    /// The task's revision after the operation.
    pub revision: u64,
    /// The phase an activity summary reports.
    pub phase: Phase,
}

/// Where admitted task operations go.
///
/// Implementations return promptly and never call back into the host.
pub trait Tasks: Send + Sync {
    /// Record a new task. `key` is the idempotency key.
    ///
    /// # Errors
    /// Returns the NIP-HOST refusal code the device receives.
    fn create(&self, key: &str, device: &str, task: &TaskCreate) -> Result<TaskRef, Code>;

    /// Replace a task's instructions at the revision the device last read.
    ///
    /// # Errors
    /// Returns the NIP-HOST refusal code the device receives.
    fn steer(
        &self,
        key: &str,
        device: &str,
        task: &str,
        revision: u64,
        prompt: &str,
    ) -> Result<TaskRef, Code>;

    /// Request a task's cancellation at the revision the device last read.
    ///
    /// # Errors
    /// Returns the NIP-HOST refusal code the device receives.
    fn cancel(
        &self,
        key: &str,
        device: &str,
        task: &str,
        revision: u64,
        reason: &str,
    ) -> Result<TaskRef, Code>;
}

/// A host without a task owner. Every task operation refuses as
/// `unavailable`.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoTasks;

impl Tasks for NoTasks {
    fn create(&self, _: &str, _: &str, _: &TaskCreate) -> Result<TaskRef, Code> {
        Err(Code::Unavailable)
    }
    fn steer(&self, _: &str, _: &str, _: &str, _: u64, _: &str) -> Result<TaskRef, Code> {
        Err(Code::Unavailable)
    }
    fn cancel(&self, _: &str, _: &str, _: &str, _: u64, _: &str) -> Result<TaskRef, Code> {
        Err(Code::Unavailable)
    }
}
