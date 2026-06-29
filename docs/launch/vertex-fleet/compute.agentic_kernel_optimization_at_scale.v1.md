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

---

## 2026-06-20 update — market dispatch boundary (blocker: market_dispatch_missing)

State: **red** (unchanged — no flip).

Advances **`blocker.product_promises.agentic_kernel_optimization_market_dispatch_missing`**
by binding the already-built throughput-parity acceptance verdict
(`packages/tassadar-executor/src/kernel-optimization-parity.ts`) to the
verified-work labor rail, so the two market boundaries from the work definition
are mechanically checkable instead of paper-only:

- `packages/tassadar-executor/src/kernel-optimization-dispatch.ts`
  - `buildKernelOptimizationWorkRequest(spec)` — turns a named target + named
    baseline tok/s record into a **public, public-safe kernel-optimization work
    request** shaped like the green labor market (`labor.forum_work_requests.v1`):
    it names the target model+device, the baseline throughput record, the
    required parity capability, the independent validator device, and the **dual
    acceptance criteria** (throughput improvement vs the named baseline AND
    output parity via exact-trace-replay). Refuses target/op mismatch against the
    baseline, non-positive budget, and any embedded secret/path/credential
    material (mirrors the green `apps/pylon/src/work-requester.ts` guard).
  - `buildKernelOptimizationSettlementClaim(verdict)` — turns a kernel
    optimization acceptance verdict into a **born-verified settlement claim** for
    the verified-work rail (`labor.nostr_negotiation_market.v1`). A claim is
    produced ONLY for an `accepted` verdict with a verified parity outcome and a
    finite positive speedup — a faster-but-wrong or non-improving kernel never
    yields a payable claim. Correctness/throughput dominate at the money
    boundary, not just at verification.
- Tests: `packages/tassadar-executor/src/kernel-optimization-dispatch.test.ts`
  (8 tests: dispatch happy path; baseline target/op mismatch refusals; budget
  refusal; unsafe-material refusal; born-verified settlement; rejected-verdict
  refusal; missing-speedup refusal). Exported from the package index.

Moves no money and creates no serving claim — it produces the public request
body a dispatcher would post and the settlement claim a verified-work clear
would consume.

### What remains (still red)

A live network run (real agents authoring kernels and posting these requests to
the live rail), real tok/s records vs named baselines on declared hardware with
independent-device `Verified` parity and **settled** receipts, the at-scale
across-the-mesh run (`agentic_kernel_optimization_at_scale_run_missing`), and
owner sign-off. The March-2026 Psionic/Qwen 3.5 result stays historical-demo
evidence only. No promise state changed; no blocker dropped from the registry.
