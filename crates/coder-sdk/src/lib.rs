//! The Coder SDK's harness-independent layer
//! (`docs/coder/design/coder-sdk.md`): one normalized hook event set, the
//! [`HookOutcome`] a hook returns, a [`Registry`] of hooks, the fixed
//! rules that combine several hooks on one event ([`Runner`]), the support
//! tier each backend gives each event, the hook set's digest in the route's
//! adapter digest, and a [`ScriptedSession`] that replays a recorded session
//! through the hooks with no login.
//!
//! The Claude Code backend is [`claude`]; the Coder loop, Codex and ACP
//! backends build on this crate elsewhere.

pub mod claude;
pub mod compose;
pub mod digest;
pub mod events;
pub mod outcome;
pub mod registry;
pub mod scripted;

pub use coder_delegate::session::{HookBackend, HookTier, hook_tier, hook_tiers};
pub use compose::{
    Applied, Budget, Dropped, Failure, FailureKind, Refused, Runner, check_backend, check_session,
    check_start,
};
pub use digest::{adapter_digest, hook_set, hook_set_digest};
pub use events::{Call, Event};
pub use outcome::{Context, HookOutcome, Permission, Stop};
pub use registry::{Disclosure, Hook, Registry, Source, Versions};
pub use scripted::{RECORDED_SESSION_SCHEMA, Recorded, Replay, ScriptedSession};
