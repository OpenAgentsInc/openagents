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

## Update 2026-06-20 — campaign↔settlement escrow-conservation reconciliation

Further advances `blocker.product_promises.agentic_kernel_optimization_at_scale_run_missing`
by binding the two halves that previously existed independently: the dispatched
campaign (`buildKernelOptimizationCampaign`) and its settlement ledger
(`summarizeKernelOptimizationCampaignSettlement`). Nothing yet asserted they
described the SAME campaign — at scale the dangerous drift is accounting, not
correctness (correctness is already caught by the parity verdict).

New artifact (in `apps/openagents.com/workers/api`):

- `reconcileKernelOptimizationCampaignSettlement(campaign, settlement)` (added to
  `src/kernel-optimization-campaign.ts`) returns a
  `KernelOptimizationCampaignReconciliation` report whose `ok` gate must hold
  before the verified-work rail releases any payout/refund. It mechanically
  catches the three at-scale accounting failure modes: (1) **campaignRef drift**
  — settling one run's verdicts against another's dispatch set; (2) **job-count
  drift** — a dispatched job never settled or an extra settlement no job backs;
  (3) **escrow drift** — `payout + refund != escrow locked` (sats created or
  destroyed in settlement). It moves no money and never throws; a mismatch is a
  listed `discrepancy`, not an exception.
- `src/kernel-optimization-campaign.test.ts` — 4 new tests (11 total): the
  complete escrow-conserving four-job reconciliation, dropped-job drift, escrow
  drift with a matching job count, and reconciling mismatched campaigns.

### What still remains for the at-scale run (blocker NOT cleared)

- The reconciliation is totals/count-level (escrow conservation + job count); it
  does not yet match each settlement item to its specific dispatched job, because
  the parity verdict carries no `kernelRef`. Per-target reconciliation remains.
  (Addressed by the 2026-06-20 update below.)
- Everything from the prior update still stands: live dispatch through the forum
  route, real worker tok/s + replayed output traces, and real escrow settled
  into dereferenceable receipts + owner sign-off.

## Update 2026-06-20 — per-job (per-target) settlement reconciliation

Further advances `blocker.product_promises.agentic_kernel_optimization_at_scale_run_missing`
by closing the gap the prior update explicitly named: the existing
reconciliation was totals-level (job count + escrow conservation) and could not
say WHICH job drifted, nor catch offsetting errors that net to zero. The parity
verdict carries the target model/device/hardware but not the dispatched job's
identity, so matching is keyed instead on the campaign's already-guaranteed-
unique `requestedSlug`.

New artifact (in `apps/openagents.com/workers/api`):

- `reconcileKernelOptimizationCampaignPerJob(campaign, items)` (added to
  `src/kernel-optimization-campaign.ts`) matches each
  `KernelOptimizationKeyedSettlementItem` (a settlement item plus the
  `requestedSlug` it settles) to its specific dispatched request. It returns a
  `KernelOptimizationCampaignPerJobReconciliation` whose `ok` gate must hold
  before the rail releases any per-job payout/refund, naming: `matchedSlugs`
  (settled exactly once with the dispatched escrow), `unsettledSlugs`
  (dispatched but never settled), `unexpectedSlugs` (settled but never
  dispatched by this campaign), `duplicateSlugs` (settled more than once), and
  `budgetMismatches` (per-job settled escrow != dispatched escrow). It moves no
  money and never throws.
- `src/kernel-optimization-campaign.test.ts` — 5 new tests (16 total),
  including a case that double-settles one job and drops another so the
  totals-level reconciler still passes (job count 4, escrow 200_000 both match)
  while the per-job reconciler correctly fails it.

### What still remains for the at-scale run (blocker NOT cleared)

- Matching is by `requestedSlug` (the on-rail job id), not by the parity
  verdict's own target — wiring the verdict's target back to the dispatched job
  end-to-end remains. (Addressed by the 2026-06-20 update below.)
- Everything from the prior updates still stands: live dispatch through the
  forum route, real worker tok/s + replayed output traces, and real escrow
  settled into dereferenceable receipts + owner sign-off. The March 2026 result
  stays historical-demo only. No promise state changed; no blocker dropped.

## Update 2026-06-20 — verdict-target ↔ dispatched-job binding (target-swap guard)

Further advances `blocker.product_promises.agentic_kernel_optimization_at_scale_run_missing`
by closing the gap every prior update explicitly named: the slug-keyed and
totals-level reconcilers trust the `requestedSlug` a settlement is *filed under*
but never inspect the **parity verdict** that settlement carries. At scale this
leaves a hole that conserves escrow and passes both existing reconcilers: a
settlement filed under job A's slug, with job A's exact budget, but carrying a
verdict that optimizes a DIFFERENT target B (e.g. a verified accept on a cheaper
model). The accounting balances; the wrong work gets paid.

New artifact (in `apps/openagents.com/workers/api`):

- `reconcileKernelOptimizationCampaignTargets(spec, items)` (added to
  `src/kernel-optimization-campaign.ts`) binds each settlement's verdict back to
  the specific dispatched job by **target**. It recomputes each dispatched job's
  requested slug from the campaign spec via the same dispatch encoder (so it
  cannot drift from the real slug), learns that slug's true `(model, device,
  hardware)` target, and checks the settled verdict's own `target` matches it
  (trim + case-normalized). It returns a
  `KernelOptimizationCampaignTargetReconciliation` whose `ok` gate must hold
  before any payout/refund release, naming `matchedSlugs`, `unmatchedSlugs`
  (settled under a slug this campaign never dispatched), and `targetMismatches`
  (the slug, its dispatched target, and the settled verdict's target). It takes
  the campaign SPEC (not the built campaign) because only the spec carries the
  structured target; it moves no money and never throws.
- `src/kernel-optimization-campaign.test.ts` — 4 new tests (20 total): the clean
  four-job pass, the target-swap case that the per-job reconciler explicitly
  still passes while this one fails, an unmatched-slug settlement, and spec
  validation reuse (empty campaign rejected).

This is the end-to-end "wire the verdict's target back to the dispatched job"
piece the prior update deferred — now done on the model/device/hardware the
verdict already carries (no change to the green parity engine).

### What still remains for the at-scale run (blocker NOT cleared)

- Binding is on model/device/hardware; the op-level kernel ref is still not in
  the parity verdict (baseline and optimized records intentionally name
  different kernel *implementations*), so op-granular target binding would need
  the verdict to carry the optimized *op* distinctly. Spec-side slug recompute
  covers op implicitly (the slug encodes it) but the verdict does not assert it.
  (Addressed by the 2026-06-20 update below.)
- Everything from the prior updates still stands: live dispatch through the
  forum route, real worker tok/s + replayed output traces, and real escrow
  settled into dereferenceable receipts + owner sign-off. The March 2026 result
  stays historical-demo only. No promise state changed; no blocker dropped.

## Update 2026-06-20 — verdict-op ↔ dispatched-job binding (op-swap guard)

Further advances `blocker.product_promises.agentic_kernel_optimization_at_scale_run_missing`
by closing the gap the prior update explicitly named: the model/device/hardware
target reconciler binds a settlement's verdict to its dispatched job by
`(model, device, hardware)`, but a single campaign can dispatch SEVERAL ops
against the SAME target — e.g. `rmsnorm` and `attention.flash` both on
`qwen-3.5-0.5b`/`cuda`/`a10g`. At scale this leaves a hole that conserves
escrow and passes BOTH the per-job (slug + escrow) and the model/device/hardware
target reconcilers: a settlement filed under the `rmsnorm` job's slug, with that
job's exact budget, but carrying a verdict that actually optimized
`attention.flash`. The accounting balances and the coarse target matches; the
wrong op gets paid.

New artifacts:

- `packages/tassadar-executor/src/kernel-optimization-parity.ts` — the parity
  verdict now carries `optimizedOpRef`, the op the job targets (matching the
  dispatched job's `kernelRef`), surfaced (trimmed) from a new required verifier
  input field. It is distinct from the throughput records' `kernelRef`, which
  name the baseline vs optimized *implementations* of that op. No new rejection
  reason and no change to the green exact-trace-replay engine.
- `apps/openagents.com/workers/api/src/kernel-optimization-campaign.ts` —
  `reconcileKernelOptimizationCampaignOps(spec, items)` binds each settlement's
  verdict back to the specific dispatched job by OP. It recomputes each
  dispatched job's requested slug from the campaign spec via the same dispatch
  encoder (so it cannot drift from the real slug), learns that slug's true op
  (`kernelRef`), and checks the settled verdict's own `optimizedOpRef` matches it
  (trim + case-normalized; a blank verdict op never matches). It returns a
  `KernelOptimizationCampaignOpReconciliation` whose `ok` gate must hold before
  any per-op payout/refund release, naming `matchedSlugs`, `unmatchedSlugs`, and
  `opMismatches` (the slug, its dispatched op, and the settled verdict's op). It
  moves no money and never throws.
- Tests: `kernel-optimization-parity.test.ts` gains the op-surfaced/trim
  assertions (6 tests); `kernel-optimization-campaign.test.ts` gains a 5-test
  op-reconciliation block (25 total) including a two-ops-on-one-target campaign
  whose op-swap passes BOTH the per-job and model/device/hardware reconcilers
  while this one fails it, plus blank-op, unmatched-slug, and spec-validation
  reuse cases.

This is the op-granular "wire the verdict's op back to the dispatched job" piece
the prior update deferred — now done on the op the verdict carries explicitly.

### What still remains for the at-scale run (blocker NOT cleared)

- Op binding compares the dispatched job's single `kernelRef` (op) to the
  verdict's `optimizedOpRef`; it does not yet assert the baseline/optimized
  throughput records both pertain to that op (they intentionally name different
  implementations). Cross-checking record provenance against the op remains.
- Everything from the prior updates still stands: live dispatch through the
  forum route, real worker tok/s + replayed output traces, and real escrow
  settled into dereferenceable receipts + owner sign-off. The March 2026 result
  stays historical-demo only. No promise state changed; no blocker dropped.

## Update 2026-06-20 — throughput-record op-provenance gate (apples-to-oranges guard)

Advances `blocker.product_promises.agentic_kernel_optimization_throughput_parity_verification_missing`
by closing the gap the prior op-binding update explicitly named: the verifier
took the baseline/optimized throughput records on trust and never asserted they
both *measure the op the job claims to optimize*. The records carry `kernelRef`
(the kernel *implementation* — baseline vs optimized intentionally differ), so
nothing stopped a settlement pairing a `rmsnorm` baseline (328 tok/s) against an
`attention.flash` optimized record (a higher, unrelated tok/s) and reading the
ratio as a "speedup" of `rmsnorm`. The accounting balanced and every campaign
reconciler passed; the throughput comparison was apples-to-oranges.

New artifact (in `packages/tassadar-executor`, the green parity engine):

- `src/kernel-optimization-parity.ts` — `KernelThroughputRecord` now carries an
  `opRef` (the op the record's kernel implements, distinct from `kernelRef` the
  implementation). `verifyKernelOptimizationParity` gains an `op_mismatch`
  rejection: both records' `opRef` AND the job's claimed `optimizedOpRef` must
  normalize (trim + lowercase) to the same non-empty op, else the tok/s numbers
  are not comparable and the kernel is rejected. The gate sits in the structural
  block (after `target_mismatch`, before `invalid_throughput`/parity/throughput),
  so op provenance — like target — is checked before "faster but wrong" can ever
  matter. No change to the exact-trace-replay engine; no new money path.
- `src/kernel-optimization-parity.test.ts` — 6 new tests (11 total): case-
  insensitive record-op match accepts, a different-op optimized record rejects,
  a claimed op that disagrees with both records rejects, a blank claimed op
  rejects, and op-mismatch wins over a faster-but-wrong parity verdict.

This is the "cross-checking record provenance against the op" piece the prior
update deferred — now the verifier itself proves the two tok/s records and the
claimed op are the same op before producing any acceptance verdict.

### What still remains (blocker NOT cleared)

- Op provenance is now asserted on the records' declared `opRef`; it is still a
  declared field, not derived from the measured kernel's own trace. Binding the
  record's op to the replayed output trace's graph remains.
- Everything from the prior updates still stands: live dispatch through the
  forum route, real worker tok/s + replayed output traces feeding the verdict
  (numbers are still caller-supplied), and real escrow settled into
  dereferenceable receipts + owner sign-off. The March 2026 result stays
  historical-demo only. No promise state changed; no blocker dropped.

## Update 2026-06-20 — delivered-kernel gate (same-kernel / no-deliverable guard)

Advances `blocker.product_promises.agentic_kernel_optimization_throughput_parity_verification_missing`
by closing a hole the op-provenance gate left open: the verifier asserted the
baseline and optimized throughput records measure the SAME op, but never that the
optimized record names a kernel implementation actually DISTINCT from the
baseline. The work definition's deliverable is "an agent-authored optimized
kernel" — a new kernel, not the baseline remeasured. Nothing stopped a settlement
pairing the baseline kernel against ITSELF (`kernelRef` equal) and reading
remeasurement noise — or a cherry-picked high run of the same implementation — as
a "speedup". Every campaign reconciler passed and the parity verdict was
`verified`; a non-optimization got paid.

New artifact (in `packages/tassadar-executor`, the green parity engine):

- `src/kernel-optimization-parity.ts` — `verifyKernelOptimizationParity` gains a
  `kernel_not_optimized` rejection. After the op-provenance gate and before the
  throughput/parity gates (structural, like target and op), it requires the
  optimized record's `kernelRef` to (a) be non-empty and (b) normalize (trim +
  lowercase) to something different from the baseline record's `kernelRef`. If
  the optimized record names the same implementation as the baseline (or names
  none), no new kernel was delivered, so the tok/s delta is not an optimization
  and the kernel is rejected regardless of throughput or parity. No change to the
  exact-trace-replay engine; no new money path.
- `src/kernel-optimization-parity.test.ts` — 4 new tests (15 total): same-kernel
  rejection, trim/case-insensitive same-kernel detection, blank optimized
  `kernelRef` rejection, and the deliverable gate winning over a faster-but-wrong
  parity verdict.

### What still remains (blocker NOT cleared)

- The gate proves the optimized record names a DIFFERENT, non-empty kernel
  implementation than the baseline; it does not yet prove that implementation is
  the one whose output trace was replayed on the validator device (the
  `kernelRef` is a declared label, not derived from the replayed graph). Binding
  the optimized `kernelRef` to the parity trace's graph digest remains.
- Everything from the prior updates still stands: live dispatch through the
  forum route, real worker tok/s + replayed output traces feeding the verdict
  (numbers are still caller-supplied), and real escrow settled into
  dereferenceable receipts + owner sign-off. The March 2026 result stays
  historical-demo only. No promise state changed; no blocker dropped.
