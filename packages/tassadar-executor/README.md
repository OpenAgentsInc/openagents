# @openagentsinc/tassadar-executor

TypeScript executor and exact-replay verdicts for the psionic Tassadar
ALM numeric-model format (`TassadarAlmNumericModel` v1) — the portable,
digest-pinned artifact produced by the psionic executor-compiler
(psionic #1113). Used by the `compute.tassadar_executor_poc.v1` proof of
concept: Pylons execute the workload, validators replay it on a separate
device, and verdicts are a digest comparison.

The load-bearing tests reproduce the Rust executor's trace digest
byte-for-byte on the committed fixtures:

- `fixtures/tassadar-poc-loop-sum-v1.json` — the original
  backward-branch loop `TassadarProgram`; regeneration recipe is the
  ignored `dump_poc_fixture` test in psionic
  `crates/psionic-compiler/src/tassadar_alm_numeric.rs`.
- `fixtures/tassadar-compiled-program-corpus-v1.json` — the C1
  run-facing corpus of four distinct psionic-derived programs
  (loop-sum, arithmetic, memory roundtrip, factorial state machine);
  regeneration recipe is the ignored
  `dump_numeric_program_corpus_fixture` test in the same psionic file.

Replay CLI:

```
bun run replay fixtures/tassadar-poc-loop-sum-v1.json \
  --validator-device device.example [--claimed-digest <hex>] [--tamper-step 10]
```

Claim boundary: faithful re-execution of digest-pinned compiled
workloads only — no softmax, no learning, no serving, no performance
claim against conventional CPUs. The promise's unsafeCopy governs all
public copy about this lane.
