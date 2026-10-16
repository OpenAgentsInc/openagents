# layertree-tee-lab

A research sandbox. It holds LayerTree's off-chain tree and transition
rules, enforced by a modelled TEE co-signer on today's Bitcoin, with a
pre-signed, timelocked unroll that needs no enclave. The spec is
[`docs/research/layertree/2026-10-15-mainnet-tee-spec.md`](../../docs/research/layertree/2026-10-15-mainnet-tee-spec.md).

- **Outside the default build.** The crate has its own `[workspace]`, and
  the root `Cargo.toml` lists it under `exclude`. Nothing ships in a Coder
  binary.
- **No upstream code.** It is written from LayerTree spec v0.15.2, read at
  `lightninglabs/layertree` commit `f6c4d29`, because the repository has no
  licence. One opt-in test reads upstream's published vectors from a local
  reference clone at test time.
- **No network, no real keys, no money.**

```sh
cargo test --manifest-path crates/layertree-tee-lab/Cargo.toml
# the opt-in cross-check against upstream's vectors:
LAYERTREE_VECTORS=~/work/projects/repos/layertree/spec/test-vectors.json \
  cargo test --manifest-path crates/layertree-tee-lab/Cargo.toml -- --ignored
```

| Module | What |
| --- | --- |
| `tree` | leaf, Merkle-sum nodes, dense tree, proofs, vouchers, path maintenance |
| `output` | the tree output: `Q + H_state(r)·G` key path, the unroll tapleaf, `TREE_SPK_CCV` |
| `rules` | the predicate the enclave enforces (spec 7.1 to 7.5), with one `Reject` per rule |
| `enclave` | the in-process co-signer: its key never leaves, and it signs only what passes |
| `unroll` | the delayed exit with deleted intermediate keys, checked with public data only |
| `attest` | a fake hardware report and the light client's attestation check |
| `light` | a member following the tree from chain data and anchored records |

`tests/lean_mirrors.rs` has property tests named after upstream's Lean
theorems (`ExitK_no_inflation`, `Steps_sum`, `Reachable_ledger`, `payable`,
`exit_available`, `voucher_no_replay`, `path_maintenance` and others).

No `bitcoind` was available when this was written. The regtest harness is
the next step of P0 (see the spec, section 11).
