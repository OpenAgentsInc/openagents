//! Apply-side scaffolding for client replication.
//!
//! In Phase 3, this will perform ECS mutations and buffer GPU upload work.

/// A tiny marker trait for types that apply replication messages to local state.
pub trait ReplicationApply {
    /// Apply a serialized message. Returns whether anything changed.
    fn apply_bytes(&mut self, _bytes: &[u8]) -> bool {
        false
    }
}
