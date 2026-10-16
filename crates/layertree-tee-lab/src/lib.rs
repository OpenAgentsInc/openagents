//! LayerTree on today's Bitcoin with a TEE enforcing the rules: a sandbox.
//!
//! Spec: `docs/research/layertree/2026-10-15-mainnet-tee-spec.md`.
//! Written from LayerTree spec v0.15.2 and the paper; no upstream code is
//! copied (the upstream repository has no licence). Research only: no
//! network, no real keys, no money. Outside the monorepo's default build.
//!
//! * `tree`: the Merkle-sum tree, proofs, vouchers, path maintenance.
//! * `codec`: byte decoders for leaves, records, `enc64` and unroll packages.
//! * `output`: the tree output (root-committed key path, unroll leaf).
//! * `rules`: the predicate the enclave enforces.
//! * `enclave`: the in-process co-signer; it signs only what passes.
//! * `unroll`: the pre-signed, delayed exit that needs no enclave.
//! * `attest`: a fake report and the light client's check.
//! * `light`: a member following the tree from chain data and records.
//! * `events` (feature `events`): the `layertree.events.v1` run log.

pub mod attest;
pub mod codec;
pub mod enclave;
#[cfg(feature = "events")]
pub mod events;
pub mod light;
pub mod output;
pub mod rules;
pub mod tree;
pub mod unroll;
