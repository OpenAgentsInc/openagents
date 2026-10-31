# Chapter 124: nanochat MuonAdamW, 2026-10-31

Task #68b5. Profile `cpu-muon-v1`, Apple clang arm64, Coax 0.40.0
(base 4b50a5c112). CPU only, one rank, binary64 throughout; no GPU, downloads
or model weights.

Reference: karpathy/nanochat `92d63d4e8bb4df75c3b71618f31ddde2378b2bcd`,
`nanochat/optim.py` (sha256 7c9b714c...afed8f), `nanochat/gpt.py`
(`GPT.setup_optimizer`, sha256 8cdedc2e...04e3) and `scripts/base_train.py`
(sha256 87395b80...fdcc), the hashes in
`psionic/fixtures/models/pins/nanochat.json`. Read only, never executed.

- `psionic/lib/optim/muon.coax` (`#authority`):
  - `mc_plan` reproduces setup_optimizer: six AdamW groups (head, wte, value
    embeddings, resid, x0, smear gate/smear/backout) with their rates, betas,
    eps, decay and the `(model_dim/768)^-0.5` scale; one Muon group per
    sorted distinct matrix shape (momentum 0.95, ns 5, beta2 0.9, the given
    decay); the flat layout and second-moment and scratch sizes. Empty groups
    are left out.
  - `mc_step` runs AdamW (chapter 124's `aw_cell`) on AdamW groups and
    nanochat's fused Muon update on each matrix: Nesterov momentum, MuonEq row
    equilibration, Polar Express (pinned coefficients, wide/tall branches),
    Muon+ renormalization, NorMuon variance reduction with the factored second
    moment, `sqrt(max(1, r/c))` rate scaling and cautious weight decay.
  - All-or-nothing admission (alias, shape, group, number, state, work,
    nonfinite, overflow), refuse or skip policy, more than five Polar steps
    refused (not silently capped), unused settings must be zero. Matrix
    overflow is detected on the sum of squares before any square root; past
    it every value is normalized.
- Coax debug and release: 11 check groups pass, plus the 9 loaded from
  adamw.coax. They cover the admission bounds and the planner tables; hand
  derivations (a 1x1 update, Polar Express on diagonal input equal to the
  scalar polynomial, the update's Frobenius norm and equal row/column norms,
  cautious decay when the update is exactly zero); and the planner
  reproducing both oracle layouts exactly and its refusals. Also both oracle
  replays and the sensitivity to every Muon setting, every group rate and to
  regrouping; resume at every step; refusals leaving every view unchanged;
  exact writes; Polar Express matching the oracle bit for bit on square, tall
  and wide input; and a self-check that every helper sees each single
  difference.
- Mutation (`coax mutate --root-only`, the AdamW dependency keeps its own
  284/284 record): **743 mutants, 743 killed, 0 survived**. No equivalents.
- Independent oracle (`psionic/tests/muon_oracle.py`, nanochat's own
  structure: named parameters, group dictionaries, per-name state, torch
  `lerp_` two-branch form, left-to-right sums). Case `nanochat`, a one-layer
  width-4 model (square 4x4 attention, wide 2x12 ve_gate, tall 16x4 and wide
  4x16 MLP, embedding/head/value-embedding/scalar/smear groups): 316
  parameters, 10 groups, 4 steps; a zero row and a 1e-9 row in its first
  gradient. Case `muon`: 68 parameters, 8 groups, momentum 0.85, beta2 0.95,
  decay 0.5, 3 Polar steps, 5 steps. Its committed values reproduce on every
  check. Declared tolerance 1e-12 relative to 1+|reference|; measured maximum
  1.46e-14 (Coax) and 1.29e-14 (native). Polar Express alone: exact.
- Native -O0/-O3, plain and ASan/UBSan: all four agree with the oracle on both
  cases, resume at all 11 cut points into fresh buffers, leave gradients
  untouched, refuse actual aliases (parameters/gradients, second moment over
  momentum, scratch over parameters or gradients), native NaN, infinity and
  matrix overflow, and skip without writing.
- Example: the planner sorts a two-layer width-8 inventory into 6 AdamW and 4
  Muon groups (the tall 32x8 rate doubles to 0.04); a 2x3 Muon matrix with an
  AdamW bias fits a linear map, loss 1.80 -> 8.8e-6 in 300 steps with a
  linear rate decay. Debug and release transcripts are identical.

Not claimed: multi-rank ZeRO-2/chunked ownership, fp32 or bf16 bit parity
with a torch run (hyperparameters there pass through fp32 tensors),
`torch.compile`, the fp16 GradScaler, base_train's rate, momentum and decay
schedules (the caller sets group fields per step), checkpoint serialization
of the Muon state, or GPU execution.
