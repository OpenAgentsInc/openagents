# compute.agentic_kernel_optimization_at_scale.v1 — vertex-fleet note

Promise state: **red** (unchanged — no flip).

## What this change builds

Advances `blocker.product_promises.agentic_kernel_optimization_throughput_parity_verification_missing`
by turning the paper throughput-parity protocol
(`docs/tassadar/2026-06-19-agentic-kernel-optimization-work-definition-and-parity-protocol.md`)
into an **executable verifier**.

New artifact (in `packages/tassadar-executor`, alongside the green exact-trace-replay engine):

- `src/kernel-optimization-parity.ts` — `verifyKernelOptimizationParity(...)`
  combines a named-baseline tok/s record, the optimized-kernel tok/s record, and
  the independent-device exact-trace-replay parity verdict (`TassadarReplayVerdict`)
  into a single `KernelOptimizationVerdict`. A kernel is `accepted` iff, on the
  SAME declared target (model/device/hardware), the parity verdict is `verified`
  AND the optimized tok/s beats the baseline. Correctness dominates speed: a
  `parity_rejected` (faster-but-wrong) kernel is rejected regardless of throughput.
  Other rejection reasons: `target_mismatch`, `invalid_throughput`,
  `no_throughput_improvement`.
- `src/kernel-optimization-parity.test.ts` — 5 tests covering accept, the
  faster-but-wrong rejection, no-improvement, target mismatch, and invalid/NaN
  throughput. Anchored on the historical 217.md numbers (328 -> 523 tok/s).
- exported from `src/index.ts`.

The verifier moves no money and makes no serving claim; it only produces the
acceptance verdict that the verified-work rail (`labor.nostr_negotiation_market.v1`)
would settle on.

## What genuinely remains (still red)

- A real agent-authored optimized kernel **dispatched through the market**
  (`blocker.*_market_dispatch_missing`).
- A real tok/s record against a named baseline on declared hardware feeding this
  verifier from live measurement (here the numbers are caller-supplied).
- The parity verdict still uses the bounded-workload exact-trace-replay engine;
  wiring a real kernel's output trace into it remains.
- An **at-scale, across-the-mesh** run with settlement receipts
  (`blocker.*_at_scale_run_missing`, `blocker.*_settlement_receipts_missing`).

No promise state changed; no blocker dropped (this partially advances the
throughput-parity-verification blocker — verdict logic now exists and is tested,
but it is not yet fed by live dispatched measurements).
