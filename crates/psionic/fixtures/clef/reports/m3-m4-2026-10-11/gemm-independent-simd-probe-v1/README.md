# Independent SIMD projection result

Reject product integration. The one fixed independent-SIMD candidate passed all numerical checks but failed the predeclared 10% paired speed target at every tested size.

| Tokens | Baseline full FFN median GPU ms | Candidate full FFN median GPU ms | Median paired candidate/baseline |
| --- | ---: | ---: | ---: |
| 512 | 3.09985 | 2.87460 | 1.03950 |
| 1024 | 6.40663 | 6.17560 | 1.02533 |
| 2048 | 11.48373 | 12.96325 | 1.12906 |

Paired projection-sum ratios were 1.07684, 1.01354 and 1.18605. Acceptance requires both full-FFN and projection-sum paired medians ≤0.90 at every size. The median of paired ratios differs from the ratio of marginal medians; the paired statistic controls the decision.

All 18 small checks and five real-weight FFN checks passed. Each small output had a full f64 oracle; weighted cases used 96 fixed f64 samples per projection plus complete output scans. Gate, up and half activation were bitwise identical to baseline. Accumulated down output differed by at most 1.430511474609375e-5; the largest candidate f64 sample error was 5.732717557549449e-6. The overwrite down case was bitwise equal. All 240 timed calls matched their own immutable expected bytes; source/weight/input/expectation hashes and buffer redzones stayed intact. The executed totals were 295 command buffers and 523 dispatches.

The numerical criterion was clarified before the first compilation or GPU run. F32 gate/up/down outputs and f64 samples retain absolute error ≤0.001. Half activation must be finite and satisfy absolute error ≤0.001 OR at most one Float16 ULP, with signed zeros sharing one ordered value. That allowance was not needed in this run because every weighted half activation was identical. [Version 2](criterion-revision-v2.json) records the pre-execution rationale and prior hashes. Model-free checks covered 63,488 finite half patterns plus 19 boundaries, including three nonfinite refusals. Timed repeats always remain bitwise exact. The full-model raw-logit 1e-3 gate is unchanged and was not run for this rejected candidate.

This is the first tested organization with four independent 32×32 SIMD products forming a 64×64 output tile, direct device inputs and cooperative f32 accumulation in K128 steps. The preserved baseline uses one four-SIMD 64×128 full-K operation for real Flash shapes. It is one architectural change, not an isolated test of only execution scope. The [initial plan](plan/PLAN.md) and [source audit](plan/source-audit.json) identify prior experiments and the public API reference; no guide code was copied.

The run used actual Flash layer-0 half weights and deterministic synthetic activations. It is not full-model latency or BF16 model-quality evidence. Five fixed ABBA rounds balance early/late positions, but substantial shared-machine timing variation remains; one-minute load average ranged 41.08–59.02. The result establishes no useful gain here and does not identify a thermal, frequency, bandwidth or scheduling cause. No retry, variant sweep, product edit or full-model run followed.

The first build passed in 9.2875 seconds; the single bench completed in 21.2372 seconds. Peak sampled owned-process accounting was 1,822,771,144 bytes, below the unchanged 2 GiB cap. Explicit Metal buffers peaked at 889,198,592 bytes; owned CPU expectations peaked at 603,979,776 bytes; their largest recorded combination with device.currentAllocatedSize was 1,494,089,728 bytes, below 1.5 GiB. The 14 GiB start requirement, 12 GiB available-memory floor and 60-second deadline remained unchanged. No memory/sampler/cleanup error occurred; the task-owned group and all leases were released. No quiet lease or desktop/system-service control was used.

[Complete ordered output](run-1/stdout.jsonl) preserves every check and all 240 timing records. [The summary](run-1/summary.json) preserves each paired ratio. [The build record](build-artifacts.json) retains executed source, executable and metallib hashes. The prepared source plan is preserved as [PREPARED_PLAN.md](PREPARED_PLAN.md); its initial unrun status describes preparation, not this completed result. The source audit and source-check records are historical snapshots.

Binaries, raw weights and memory timelines are excluded. To reproduce after explicit resource authorization, set CLEF_REPO to the checkout with the recorded guard dependencies, supply hash-matching assets in the documented sibling directory, and use guard.py build under build+4 GiB memory leases followed by guard.py bench under GPU+2 GiB memory leases. Use fresh output directory names and never a quiet lease. Run summarize.py afterward. Source-only checks can run without Metal; the published copy replaces its historical sibling-extract dependency with the same recorded hashes. Published source changes produce fresh build hashes; original receipt hash fields continue to identify the exact executed sources.

[The sanitization manifest](sanitization.json) maps every original file to its published bytes. Only this experiment's task-owned logs and lease receipts are included. No failed build or numerical run was discarded; both completed, while the failed performance decision is preserved.
