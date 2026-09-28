//! Model-provider capacity: which providers a host can reach, and which of
//! them refused work for a usage or rate limit, until when.
//!
//! The book lives in the Microcoder loop crate
//! ([`microcoder_loop::capacity`]), because the loop's failover writes it
//! and `coder` runs the loop in its delegate door. This module re-exports
//! it under the task store's name, so the auto-start policy, the usage
//! probes, and the loop read one `capacity.json`.

pub use microcoder_loop::capacity::*;
