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

## Update 2026-06-20 — market dispatch encoder

Advances `blocker.product_promises.agentic_kernel_optimization_market_dispatch_missing`
by turning the paper kernel-optimization WORK DEFINITION into a concrete,
dispatchable job on the already-green verified-work rail
(`labor.forum_work_requests.v1`).

New artifact (in `apps/openagents.com/workers/api`):

- `src/kernel-optimization-work-dispatch.ts` —
  `buildKernelOptimizationWorkRequest(spec)` maps a `KernelOptimizationJobSpec`
  (target model + device + hardware, named-baseline tok/s record, kernel/op,
  independent validator device, budget, deadline) to the exact
  `CreateForumWorkRequestBody` the forum work-request route already accepts. It
  round-trips the body through `decodeCreateForumWorkRequestBody`, so any value
  it returns is guaranteed dispatch-valid. The `verificationCommandRef` binds
  the parity verdict: it names `KERNEL_OPTIMIZATION_PARITY_CLASS_ID`, the
  baseline tok/s floor (`min_tok_s=`), the named-baseline record, and the
  validator device — so acceptance stays mechanically checkable (faster AND
  still correct), not operator judgment. Jobs require both a
  `capability.kernel_optimization.*` provider capability and the green
  `capability.tassadar_poc.numeric_model_executor` (parity replay) capability.
- `src/kernel-optimization-work-dispatch.test.ts` — 7 tests: dispatch-valid
  round-trip, parity/baseline binding, required capabilities, route-valid slug,
  and rejection of non-positive baseline tok/s, non-integer budget, and empty
  kernel ref. Anchored on the historical 217.md baseline (328 tok/s).

This is the encoder only: it constructs the dispatch payload but does not POST
it, settle escrow, or run measurement. Money still moves through the existing
forum work-request route + labor escrow; this only produces the request body.

### What still remains for market dispatch (blocker NOT cleared)

- Actually POSTing a built request through the forum work-request route and
  driving its lifecycle (offer -> accept -> result -> release) for a real
  kernel job.
- Feeding the throughput-parity verifier from a live worker tok/s measurement
  + a real optimized-kernel output trace replayed on the validator device,
  rather than caller-supplied numbers.
- An at-scale, across-the-mesh run with settlement receipts
  (`blocker.*_at_scale_run_missing`).

## Update 2026-06-20 — at-scale campaign fan-out + settlement aggregation

Advances `blocker.product_promises.agentic_kernel_optimization_at_scale_run_missing`
by composing the two pieces above (the market-dispatch encoder and the
throughput-parity verdict) into the across-the-mesh shape the promise names:
ONE campaign that fans out across MANY targets, and a settlement ledger that
reduces the MANY verdicts back into ONE campaign result.

New artifact (in `apps/openagents.com/workers/api`):

- `src/kernel-optimization-campaign.ts` —
  - `buildKernelOptimizationCampaign(spec)` fans one campaign (a non-empty set
    of `KernelOptimizationJobSpec`s, e.g. the four smallest Qwen 3.5 models from
    217.md) into one dispatch-valid `CreateForumWorkRequestBody` per job. It
    guards the at-scale invariants the single-job encoder cannot: non-empty
    campaign, no duplicate `(kernel, model, device, hardware)` target, and — since
    the encoder truncates slugs to 80 chars — no two jobs collapsing to the same
    requested slug. Returns the request set + total escrow `totalBudgetSats`.
  - `summarizeKernelOptimizationCampaignSettlement(campaignRef, items)` reduces
    the per-job `{ budgetSats, KernelOptimizationVerdict }` pairs into a
    `KernelOptimizationCampaignSettlement`: accepted/rejected counts, payout owed
    (sum of accepted budgets), refund owed (sum of rejected budgets), rejection
    reason histogram, and accepted-job speedup min/max/mean. Acceptance comes
    straight from each verdict's `outcome` (already "faster AND still correct"),
    so a `parity_rejected` job never accrues payout.
- `src/kernel-optimization-campaign.test.ts` — 7 vitest tests: 4-target
  dispatch-valid + slug-unique fan-out, empty-campaign / duplicate-target /
  blank-campaignRef rejection, the mixed-outcome settlement (accept, faster-but-
  wrong, no-improvement), the null-speedup all-rejected case, and non-integer
  budget rejection. Anchored on the 217.md numbers (328 -> 523 tok/s).

This moves no money, posts no request, and runs no kernel. It produces the
dispatch set + the settlement ledger the verified-work rail would execute.

### What still remains for the at-scale run (blocker NOT cleared)

- Actually dispatching the campaign requests through the live forum work-request
  route, driving every job's lifecycle across many real worker agents.
- Live tok/s measurement + real optimized-kernel output traces replayed on
  independent validator devices feeding the verdicts (still caller-supplied).
- Real escrow settled from the ledger, producing dereferenceable settlement
  receipts + owner sign-off. The March 2026 result stays historical-demo only.
