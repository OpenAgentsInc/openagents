# Compiler Replay Reference

`psionic-compiler` now has a checked-in replay gate for lowering and
program-identity drift.

The gate lives in:

- `crates/psionic/psionic-compiler/tests/process_replay.rs`
- `crates/psionic/psionic-compiler/tests/fixtures/`
- `scripts/lint/psionic-compiler-replay-gate.sh`

## What It Checks

The current fixture set intentionally stays small and reviewable:

- `matmul_add`
  - baseline linear algebra lowering over one matmul-plus-bias graph
- `attention_backend_extension_tensor_sharded`
  - backend-extension-heavy attention lowering plus explicit tensor-sharded
    topology identity

Each fixture records:

- graph digest and canonical graph-signature lines
- lowered plan digest and canonical plan-signature lines
- human-readable plan debug output
- compiled-program digest and topology-bound signature lines
- serialized topology payload when the case carries explicit topology

## How To Run It

Use either entrypoint:

```bash
cargo test -p psionic-compiler --test process_replay -- --nocapture
scripts/lint/psionic-compiler-replay-gate.sh
```

## Drift Policy

Treat replay drift as a regression by default.

If a change is supposed to preserve lowering behavior, the fixture must not
move. If the fixture moves anyway, reviewers should treat that as evidence of
unexpected compiler drift until proven otherwise.

If a compiler change is intentionally supposed to change lowering or topology
identity:

1. inspect the fixture diff line by line
2. explain why the new lowering is expected
3. update the fixture in the same change that changes the compiler
4. keep the issue, PR, or commit message explicit about the approved drift

This keeps "expected change" separate from silent replay breakage.

## Why It Exists

Benchmarks alone do not protect compiler truth.

Psionic already depends on deterministic digests, replay-safe receipts, and
inspectable execution identity. The compiler needs the same discipline. This
gate gives ordinary development a small, deterministic regression harness for
plan and topology identity before performance or product-level behavior changes
start hiding the drift.
