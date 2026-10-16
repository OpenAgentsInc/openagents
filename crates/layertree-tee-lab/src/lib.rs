//! LayerTree on today's Bitcoin with a TEE enforcing the rules: a sandbox.
//!
//! Spec: `docs/research/layertree/2026-10-15-mainnet-tee-spec.md`.
//! Written from LayerTree spec v0.15.2 and the paper; no upstream code is
//! copied (the upstream repository has no licence). Research only, outside
//! the monorepo's default build. Opt-in probes use real guest keys and
//! hardware evidence; transaction experiments use regtest or signet.
//!
//! * `tree`: the Merkle-sum tree, proofs, vouchers, path maintenance.
//! * `codec`: byte decoders for leaves, records, `enc64` and unroll packages.
//! * `output`: the tree output (root-committed key path, unroll leaf).
//! * `predicate`: pure transition and exact-payout checks, shared with Kani.
//! * `rules`: the Bitcoin and BIP-340 adapter for that predicate.
//! * `enclave`: the in-process co-signer; it signs only what passes, in two
//!   phases, with forced inclusion, mirror receipts and the `HALT` switch.
//! * `mirror`: record mirrors and the receipts the enclave waits for.
//! * `unroll`: the pre-signed, delayed exit that needs no enclave.
//! * `attest`: a fake report and the light client's check.
//! * `light`: a member following the tree from chain data and records.
//! * `follow`: that member on a real chain: BIP-158 filters, record
//!   sources, alarms.
//! * `events` (feature `events`): the `layertree.events.v1` run log.

pub mod attest;
pub mod codec;
pub mod enclave;
#[cfg(feature = "events")]
pub mod events;
pub mod follow;
#[cfg(all(unix, feature = "events"))]
pub mod guest;
pub mod light;
pub mod mirror;
pub mod output;
pub mod predicate;
#[cfg(kani)]
mod proofs;
pub mod rules;
pub mod tree;
pub mod unroll;
