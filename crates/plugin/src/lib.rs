//! Packet ABI host.
//!
//! A guest runs in a fresh instance with no ambient interface. Pure guests
//! receive only the packet. Snapshot-read guests may list and read handles
//! issued for that invocation.

mod engine;
mod memory;
mod snapshot;

pub use engine::{
    BuildReceipt, Call, GuestValue, HostError, Limits, Profile, build_receipt, digest, invoke,
    representation,
};
pub use snapshot::{Entry, Snapshot, decode_base64, derivative, encode_base64};
