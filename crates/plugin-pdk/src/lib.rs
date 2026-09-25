//! Packet types for `openagents.plugin-packet.v1`.
//!
//! A guest and the host serialize the same request and response. The
//! packed return value is the guest output pointer in the high 32 bits
//! and the byte length in the low 32 bits.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[cfg(feature = "guest")]
pub mod guest;

/// Packet version string.
pub const VERSION: &str = "openagents.plugin-packet.v1";

/// The request the host writes into guest memory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    /// Packet version.
    pub v: String,
    /// Required features. Empty for this version.
    pub requires: Vec<Value>,
    /// Host-generated invocation id.
    pub invocation: String,
    /// Declared operation slug.
    pub operation: String,
    /// Operation input.
    pub input: Value,
    /// Logical handle names mapped to opaque tokens.
    pub handles: Map<String, Value>,
}

/// The response a guest returns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    /// Packet version.
    pub v: String,
    /// Required features. Empty for this version.
    pub requires: Vec<Value>,
    /// The invocation the request carried.
    pub invocation: String,
    /// `ok`, `unsupported_input`, or `refused`.
    pub status: String,
    /// Schema-valid value, or null when status is not `ok`.
    pub value: Value,
    /// Bounded refusal text, or null.
    pub reason: Option<String>,
}

/// Pack a guest pointer and length into the `oa_handle` return value.
#[must_use]
pub fn pack(ptr: u32, len: u32) -> i64 {
    (((u64::from(ptr)) << 32) | u64::from(len)) as i64
}

/// Split a packed return into pointer and length.
#[must_use]
pub fn unpack(packed: i64) -> (u32, u32) {
    let bits = packed as u64;
    ((bits >> 32) as u32, bits as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_round_trips_pointer_and_length() {
        assert_eq!(unpack(pack(100, 4)), (100, 4));
        assert_eq!(unpack(pack(0, 0)), (0, 0));
    }
}
