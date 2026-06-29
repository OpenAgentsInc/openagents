# Agentic kernel optimization: work definition + throughput-parity protocol — 2026-06-19

Weekend promise assault, DE-10. This defines the **public kernel-optimization
work definition** and the **throughput-parity verification protocol** that
`compute.agentic_kernel_optimization_at_scale.v1` (red) requires, by binding the
already-green exact-trace-replay parity engine to a named-baseline throughput
record dispatched through the already-green verified-work market.

It does NOT flip the promise. `compute.agentic_kernel_optimization_at_scale.v1`
STAYS **red**. This is the protocol + correctness-anchor binding, not a live
at-scale run with settled receipts. Per the hard rule, green needs an at-scale,
across-the-mesh run with throughput-parity verdicts and settled receipts + owner
sign-off.

## The honest baseline today

The only demonstrated kernel-optimization result is historical: a March 2026
build-series video (docs/transcripts/217.md) where an agent's custom CUDA kernels
took Psionic to ~523 tok/s vs a leading local runtime's ~328 tok/s on Qwen 3.5
0.5B (and Psionic beat that runtime on the four smallest Qwen 3.5 models). That
is a single historical development result, NOT a live dereferenceable receipt,
and NOT a market-dispatched job. Per `proof.demand_provenance.v1`, first-party
optimization is plumbing proof, not market proof.

## Work definition (a kernel-optimization unit of accepted work)

A kernel-optimization job is an accepted-work unit (same shape as the green labor
market, `labor.forum_work_requests.v1`) with:

1. **Target**: a named open model + named device (e.g. Qwen 3.5 0.5B on a named
   GPU), and the kernel/op to optimize.
2. **Baseline**: a public throughput (tok/s) record for an unoptimized or
   prior-best kernel on that exact target, on declared hardware.
3. **Deliverable**: an agent-authored optimized kernel.
4. **Acceptance criteria**: (a) a public throughput improvement vs the named
   baseline on the declared hardware, AND (b) an **output-parity verdict** — the
   optimized kernel must produce identical outputs to the baseline.

## Throughput-parity verification protocol

The correctness anchor reuses the **exact-trace-replay** verification already
proven green for bounded workloads under `compute.tassadar_executor_poc.v1`,
implemented in `packages/tassadar-executor` (`src/replay.ts`, digest-pinned
fixtures, `bun run replay ... --validator-device`):

1. The optimized kernel runs on the worker device; its output trace is captured
   and committed as a content-addressed digest.
2. An **independent validator device** replays the SAME inputs and recomputes the
   digest.
3. **Parity verdict**: `Verified` iff the worker digest matches the validator
   digest byte-for-byte; `Rejected` on any mismatch (a faster-but-wrong kernel
   fails acceptance — speed never overrides correctness).
4. Only a `Verified` job with a throughput improvement is accepted; payment then
   clears through the verified-work rail (`labor.nostr_negotiation_market.v1`),
   born-verified, under RL-2/RL-3.

This makes "faster" and "still correct" both mechanically checkable: the
throughput record is a public number on declared hardware, and parity is an
independent-device byte-for-byte replay verdict — not operator judgment.

## What this changes

- `compute.agentic_kernel_optimization_at_scale.v1` — **red → red.** Supplies the
  public work definition + the throughput-parity protocol bound to the existing
  exact-trace-replay engine and verified-work rail (two of the named gates,
  defined not executed). Still missing for green: an agent-authored optimized
  kernel actually dispatched through the market, a real throughput record vs a
  named baseline with a `Verified` parity verdict, an at-scale across-the-mesh
  run, and accepted-work + settlement receipts. The March 2026 result stays
  historical-demo evidence only. No state flip; receipt-first + owner sign-off
  remain required.

## References

- `docs/transcripts/217.md` — historical Psionic/Qwen 3.5 kernel result (demo).
- `packages/tassadar-executor/src/replay.ts` — exact-trace-replay parity engine.
- `promise:compute.tassadar_executor_poc.v1` — green parity correctness anchor.
- `promise:labor.forum_work_requests.v1`, `promise:labor.nostr_negotiation_market.v1`
  — green verified-work dispatch + settlement rail.
- `triton`, `flashinfer` (projects/repos) — kernel-DSL / kernel-generator refs.
