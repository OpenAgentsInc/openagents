//! Packet ABI host.
//!
//! A guest runs in a fresh instance with no ambient interface. Pure guests
//! receive only the packet. Snapshot-read guests may list and read handles
//! issued for that invocation.
//!
//! [`invoke_with_receipt`] also returns a digested [`InvocationReceipt`],
//! and [`replay`] reruns an invocation and compares it with one.

mod engine;
mod memory;
mod replay;
pub mod scope;
mod snapshot;

pub use engine::{
    BuildReceipt, Call, GuestValue, HostError, Limits, Profile, build_receipt, digest, invoke,
    representation,
};
pub use replay::{
    ENGINE, EXACT_REPLAY, InvocationReceipt, Outcome, RECEIPT_SCHEMA, Replay, canonical,
    invoke_with_receipt, replay,
};
pub use snapshot::{Entry, Snapshot, decode_base64, derivative, encode_base64};
