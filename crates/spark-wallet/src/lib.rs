//! The person's Spark wallet (Breez SDK), shared by the phone and computers.
//!
//! The phone's Wallet tab (`crates/openagents-mobile`) and `openagents
//! wallet` on computers run the same code and the same seed, so the person
//! has one balance everywhere (owner decision, 2026-10-02, in
//! `docs/breez/README.md`).
//!
//! - [`model`]: the [`Node`](model::Node) trait and the values it returns.
//! - [`spark`]: [`SparkNode`](spark::SparkNode), the real wallet, over any
//!   Breez storage.
//! - [`seed`]: BIP39 entropy and its recovery words.
//! - [`store`]: Breez's records as one JSON file, for computers, whose
//!   workspace cannot link Breez's SQLite store beside `ldk-node`'s.
//! - [`computer`]: where a computer keeps the wallet (`~/.openagents/spark`)
//!   and its seed (a 0600 file), and how it opens it on mainnet.
//! - [`link`]: the sealed envelope that carries the seed from the phone to a
//!   computer the owner approved.

pub mod computer;
pub mod link;
pub mod model;
pub mod seed;
pub mod spark;
pub mod store;

pub use breez_sdk_spark::Network;
