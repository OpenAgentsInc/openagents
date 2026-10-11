# Reuse the prepared DeltaNet diagonal

Status: source preparation only. Nothing in this directory has been compiled or
executed. The product worktree is unchanged. This is a numerical-alignment
hypothesis, not an accepted optimization.

`candidate.patch` contains four proposed source changes. `baseline/` and
`candidate/` preserve their complete before/after files, with SHA-256 values in
`manifest.json`. The standalone probe uses `baseline.metal`, `candidate.metal`,
and `support.metal`. The last file contains the exact production scalar
reduction, normalization, and sequential scan, plus a fixture-only dot wrapper.

## The arithmetic difference

For one block, let `S0` be its initial state, `G[t]` the product of decays through
token `t`, and `D(t,j)` the product from `j+1` through `t`. The block scan solves

```text
L[t,j] = beta[t] D(t,j) K[t]·K[j]     for j < t
(I + L) U = diag(beta) (V - diag(G) K S0^T)
Y = diag(G) Q S0^T + M U
M[t,j] = D(t,j) Q[t]·K[j]            for j <= t
```

The current block implementation computes the diagonal of `M` with TensorOps.
The sequential implementation instead uses the scalar `kq` already produced by
`clef_delta_prep`, then writes `sq + delta * kq` after the carried-state dot.
These reductions and accumulation orders can round differently.

The candidate splits off the diagonal:

```text
M_lower[t,j] = D(t,j) Q[t]·K[j]      for j < t; zero otherwise
Y[t] = (diag(G) Q S0^T + M_lower U)[t] + U[t] * kq[t]
```

The production `kq` already includes normalized Q, normalized K, and the query's
`1/sqrt(128)` scale. `U` already contains beta. `D(t,t)` is one, so the added
term must not multiply beta or decay again. The output gate and normalization
remain downstream of this operation.

This preserves real-number algebra while aligning one f32 reduction and its
final addition with the sequential scan. It does not make the off-diagonal
TensorOps products or carried-state updates identical to sequential f32. No
full-model quality improvement is established by source inspection.

## Interface and memory

- Add the existing `key_dot_query` buffer after beta in `delta_scan_block`.
  Validate `query_key_bytes / dim`; the shape planner has already required
  `dim == 128` and checked the product. Empty input remains a no-op.
- Keep preparation buffers 0 through 5 unchanged, bind `kq` at 6, and move
  preparation arguments from 6 to 7. Scan bindings remain unchanged.
- Store 32 diagonal values at workspace floats `[1056, 1088)`, between beta
  `[1024, 1056)` and the value region starting at 4096. Workspace stays 9280
  floats per block/value head; threadgroup storage stays 24 KiB.
- Use the existing grouped/reordered key-head mapping. No new allocation,
  dispatch, or GPU wait is required. The existing preparation/scan device
  barrier publishes the diagonal with the other workspace data.
- Add `U*kq` before the existing multiplication of `U` by the final decay.
  The triangular solve and final-state update are otherwise unchanged.

The patch updates the one serving call and all three existing test calls.
The short-buffer test covers the additional input. The new ignored hardware
test runs real production preparation at one token with zero initial state,
two key heads and six value heads in both head orders. It requires bitwise
output equality to sequential and checks output/state against the independent
dense f64 recurrence. `Guarded::read()` checks both redzones before returning
block output and state.

## Bounded standalone check

`probe.swift` compares production sequential, unchanged block32, and the
candidate. It loads no model or weight file. Before timing it checks:

- The real production-prepared one-token diagonal in both head orders, with
  exact output equality to sequential from zero initial state.
- Tails at 1, 15, 31, 32, 33, and 65 tokens; two key heads and six value heads.
- Zero decay on each side of a block boundary, cumulative underflow, decay
  one, beta zero/one, nonzero initial state, and nonaligned chunk schedules.
- Correlated keys across 155 and 2032 tokens, including 257-token chunks.
- Every small-fixture output and final state against the independent dense
  f64 oracle. The existing bound stays `2e-5 + 2e-4*abs(reference)`.
- Candidate final state bitwise equal to unchanged block32. Candidate output
  differences remain visible. Buffer redzones and all input bytes stay fixed.

For each 1024/2048-token Flash (16/32 heads) and 27B (16/48 heads) shape, every
output and state is also checked against sequential with the same bound. These
large shapes use synthetic correlated inputs, not model activations. Their
independent f64 coverage is supplied by the smaller/long-carry fixtures, not
claimed for every large-shape value.

Timing measures sequential scan versus each block's preparation plus scan.
The common normalization and `kq` preparation are excluded from all three.
There are six fixed rounds per shape; each implementation occupies each order
position twice. Raw GPU start/end times, GPU duration, host duration, and load
average are preserved. Outputs are poisoned before each correctness dispatch;
every timed dispatch starts with the same state. The final timed result must
match its previously checked result bitwise.

`guard.py` requires build+memory or GPU+memory leases and refuses a quiet lease.
GPU execution is bounded at 2 GiB, a 14 GiB available-memory start, a 12 GiB
floor, and 60 seconds. The child has its own 55-second deadline. Fixture
accounting is bounded at 768 MiB. Compilation is bounded at 4 GiB, a 16 GiB
start, the same floor, and 180 seconds. Only the newly launched child group can
be stopped. The parent must explicitly release the current build/GPU hold
before either command is scheduled.

After that handoff, the lease-owned commands are `python3 guard.py build
--output build-1` and `python3 guard.py bench --output run-1`. Both refuse to
overwrite an existing result directory. `python3 summarize.py run-1` preserves
all checks, failures and paired timing samples; in benchmark mode it requires exactly 26 checks
and all 72 timing rows. Check-only mode requires 22 checks and no timing rows. A failed gate stops the probe rather than weakening a
bound. Neither a synthetic pass nor a timing win substitutes for the unchanged
weighted `1e-3` logit gate and top-answer checks.

## Existing performance evidence

The Stage 5 weighted test logs establish maximum logit differences of
`8.87095928e-4` alone and `1.40523911e-3` with other optimizations. Their 27.81 s
and 23.87 s test durations include loading and both implementations. They are
not paired scan timings and do not establish a block32 speedup.

The earlier register-column probe measured unchanged block16 preparation plus
scan at 0.975 ms for 1024 tokens and 1.989 ms for 2048, but compared it only with
a slower block16 register variant. It did not run sequential scan. The old
scalar WY experiment was slower than sequential, but it is a different kernel.

The earlier loaded 4k profile attributed about 0.27 of 4.4 device seconds to
DeltaNet, versus about 3.4 seconds to matrix multiplication. Those measurements
are not a current phase profile. Even a successful narrow DeltaNet change
cannot, from that evidence alone, explain or close the full M3 latency gap.

No further block-size sweep is proposed: block16 already made standalone
weighted parity worse. Compensated solves and replayed state updates would add
work without a demonstrated error source. First establish whether block32 has
a useful paired speed advantage and whether this isolated diagonal change
reduces the existing error, retaining failures if either hypothesis fails.
