//! Shared OpenAgents v1 contracts.
//!
//! This module is the pure checker for `nips/openagents/contracts.md`:
//! encoding, references, locks, effects, evidence, outcomes, and the
//! private artifact envelope. It does not store bytes, open a socket, or
//! follow a locator. A passing check establishes shape and identity. It
//! does not grant authority or prove that an effect occurred.
//!
//! Machine-readable schemas live in `nips/openagents/schemas/`. A schema
//! that needs a vocabulary [`schema`] does not implement is refused
//! before execution.

mod body;
mod error;
mod json;
mod schema;

pub use body::{
    ARTIFACT_ENVELOPE_KIND, ARTIFACT_MARKER, ArtifactEnvelope, ArtifactRef, Assurance, Bound,
    BoundAssignment, Capture, Consistency, ContextEntry, ContextManifest, Coverage, DefinitionRef,
    Digest, Effects, EnvelopeRoute, EventRef, Evidence, ExecutionReceipt, Integration, Lock,
    LockEntry, Observation, Outcome, Reservation, ResolveLimits, ResolvedBytes, SCHEMA_MEDIA_TYPE,
    SchemaRef, SourceHint, SourceKind, Usage, Verification, admit_child, authenticate_event,
    check_artifact_bytes, check_derivation, check_enforcement, envelope_route, may_read,
    parse_artifact, parse_bound, parse_context, parse_definition, parse_effects,
    parse_envelope_body, parse_evidence, parse_lock, parse_observation, parse_receipt,
    parse_reservation, parse_schema_ref, resolve_lock,
};
pub use error::{ContractError, RefusalCode};
pub use json::{
    MAX_BODY_BYTES, MAX_DEPTH, SAFE_INTEGER, digest_bytes, digest_value, jcs, parse_strict,
    parse_strict_bounded,
};
pub use schema::{SchemaClosure, prepare_closure, validate_instance};

#[cfg(test)]
mod tests;
