# Chapter 78: AdamW and explicit precision, 2026-10-31

Task #5d0b. Profile `cpu-adamw-v1`, Apple clang arm64, Coax 0.38.0
(575b776fca). CPU only; no GPU, downloads or model weights.

- `psionic/lib/optim/adamw.coax` (`#authority`): AdamW with bias correction,
  decoupled decay applied to the incoming value before the moment step, and up
  to 16 parameter groups covering the parameters exactly once. The master copy
  (parameters, both moments) is binary64; compute precision is declared as
  binary64 or binary32, a binary32 compute gradient must be a binary32 value,
  and a binary32 compute copy is written. Nothing is written until the whole
  step is admitted; `nonfinite` and `overflow` refuse or, under the skip
  policy, return `skipped` and advance only the skip counter.
- Coax debug and release: 9 check groups pass: exact admission bounds, two
  hand-derived steps fixing the decay order and bias correction, three
  independent multi-step references, resume at every step of the two-group
  case, every refusal leaving all views unchanged, non-finite precedence across
  groups, exact writes, and a self-check that every test helper sees each
  single difference.
- Mutation: **284 mutants, 284 killed, 0 survived**. No equivalents.
- Independent oracle (`psionic/tests/adamw_oracle.py`, per-group dictionaries,
  Python floats, struct-packed binary32): two groups over a 3x4 matrix for 5
  steps, a binary32-compute vector for 3, a scalar with beta1 = 0 for 4. Its
  committed values reproduce on every check. Coax and native results match
  every step within 1e-15 relative; on this host the maximum difference is 0.
- Native -O0/-O3, plain and ASan/UBSan: all four agree with the oracle on all
  cases, resume at every step into fresh buffers, refuse actual aliases
  (parameters/gradients, moments, compute copy) and native NaN/infinity, and
  skip without writing.
- Example: a two-group regression (decayed weight, undecayed bias), loss
  1.875 -> 1.3346e-5 in 400 steps, step 200's NaN gradient skipped: 399
  applied, 1 skipped. Debug and release transcripts are identical.

Not claimed: Muon, nanochat optimizer parity, schedulers, checkpoint
serialization (task #039b) or GPU execution.
