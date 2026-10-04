//! Solana-specific primitives used by the cross-chain claim path.
//!
//! - [`ata`] derives the Associated Token Account address for a
//!   `(owner, mint)` pair so we can check whether it already exists.
//! - [`rpc`] is a minimal JSON-RPC client implementing just
//!   `getAccountInfo`, used by that existence check.

pub mod ata;
pub mod rpc;
