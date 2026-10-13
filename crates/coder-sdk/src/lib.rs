//! The Coder SDK's harness-independent layer
//! (`docs/coder/design/coder-sdk.md`): one normalized hook event set, the
//! [`HookOutcome`] a hook returns, a [`Registry`] of hooks, the fixed
//! rules that combine several hooks on one event ([`Runner`]), the support
//! tier each backend gives each event, the hook set's digest in the route's
//! adapter digest, and a [`ScriptedSession`] that replays a recorded session
//! through the hooks with no login. It also holds the one Coder tool
//! server every harness that takes MCP gets ([`ToolServer`], in process or
//! over stdio), with the `decide` tool behind the disclosure check
//! ([`Admission`]).
//!
//! The Claude Code backend is [`claude`]; the Coder loop, Codex and ACP
//! backends build on this crate elsewhere.

pub mod admission;
pub mod bridge;
pub mod claude;
pub mod command;
pub mod compose;
pub mod decide;
pub mod decision;
pub mod digest;
pub mod events;
pub mod hooks;
pub mod outcome;
pub mod registry;
pub mod scripted;
pub mod tools;

pub use admission::{Admission, DisclosureRefused};
pub use coder_delegate::session::{HookBackend, HookTier, hook_tier, hook_tiers};
pub use compose::{
    Applied, Budget, Dropped, Failure, FailureKind, Refused, Runner, check_backend, check_session,
    check_start,
};
pub use decision::{Acted, Costs, DecisionHook, Ruled, Timing};
pub use digest::{adapter_digest, hook_set, hook_set_digest};
pub use events::{Call, Event};
pub use hooks::{Asked, Asker, Hooks};
pub use outcome::{Context, HookOutcome, Permission, Stop};
pub use registry::{Disclosure, Hook, Registry, Source, Versions};
pub use scripted::{RECORDED_SESSION_SCHEMA, Recorded, Replay, ScriptedSession};
pub use tools::{Tool, ToolServer, serve, serve_stdio};
