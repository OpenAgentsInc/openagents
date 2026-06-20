# compute.agentic_kernel_optimization_at_scale.v1 — vertex-fleet note

State: **red** (unchanged — no flip).

## What this change adds

Advances **`blocker.product_promises.agentic_kernel_optimization_at_scale_run_missing`**
by closing a real wiring gap in the at-scale campaign settlement layer
(`apps/openagents.com/workers/api/src/kernel-optimization-campaign.ts`).

The campaign module already had four independent reconcilers — totals
(job-count + escrow conservation), per-job (slug + escrow), verdict-target
(model/device/hardware), and verdict-op — each guarding a different at-scale
drift, none subsuming another. But there was **no single gate composing them**:
a caller had to remember to run all four and AND the results by hand. Forgetting
any one (most easily the op reconciler, which only bites when one target hosts
several ops) silently re-opens exactly the drift that gate exists to catch.

Added `evaluateKernelOptimizationCampaignRelease(spec, items)` +
`KernelOptimizationCampaignReleaseGate`: it derives the campaign fan-out and the
settlement ledger ONCE (so all four gates provably evaluate the same campaign +
settlement), runs all four reconcilers, and is `ok` iff every constituent report
is `ok`. It also surfaces the settlement ledger (payout/refund totals) and a
single gate-prefixed discrepancy list, so an operator gets the atomic
"safe to release payout/refund at scale" verdict from one call. It moves no
money and never throws on a reconciliation finding — only on a structurally
invalid spec (same contract the campaign builder enforces).

Tests (`kernel-optimization-campaign.test.ts`, +4): all-gates-hold ok path with
the settlement surfaced; a same-target op swap that ONLY the op gate catches
(totals/per-job/target all pass) proving the composite is non-redundant; a
double-settled job the per-job gate catches while totals stay blind; and an
invalid-spec throw.

## What remains for this blocker (still red)

This is still DEFINED-not-EXECUTED plumbing. Green still needs: a real
market-dispatched agent-authored kernel, a live tok/s improvement vs a named
baseline on declared hardware, a `Verified` independent-device output-parity
verdict, an at-scale across-the-mesh run with these gates run on REAL
settlements, and dereferenceable accepted-work + settlement receipts plus owner
sign-off. The March 2026 Psionic/Qwen 3.5 result stays historical-demo evidence
only (`docs/transcripts/217.md`). No promise state was changed; no blocker was
dropped from the registry.

## Pointers

- Code: `apps/openagents.com/workers/api/src/kernel-optimization-campaign.ts`
- Tests: `apps/openagents.com/workers/api/src/kernel-optimization-campaign.test.ts`
- Protocol: `docs/tassadar/2026-06-19-agentic-kernel-optimization-work-definition-and-parity-protocol.md`
- Parity engine: `packages/tassadar-executor/src/kernel-optimization-parity.ts`
