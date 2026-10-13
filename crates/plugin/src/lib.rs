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
mod modules;
mod replay;
pub mod scope;
mod snapshot;

pub use engine::{
    BuildReceipt, Call, GuestValue, HostCalls, HostError, Limits, Profile, build_receipt, digest,
    invoke, invoke_with_host, representation,
};
pub use modules::{ModuleStats, module_stats};
pub use replay::{
    ENGINE, EXACT_REPLAY, InvocationReceipt, Outcome, RECEIPT_SCHEMA, Replay, canonical,
    invoke_with_receipt, invoke_with_receipt_and_host, replay, replay_with_host, snapshot_digest,
};
pub use snapshot::{
    Entry, SUPPLIED_ENTRIES, SUPPLIED_SNAPSHOT, SUPPLIED_VERSION, Snapshot, decode_base64,
    derivative, encode_base64,
};
