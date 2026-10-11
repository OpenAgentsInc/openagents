# Independent SIMD projection probe

This is source preparation for one bounded experiment. It has not been compiled or run. It changes no product source and loads no complete model.

The candidate uses four independent 32×32 SIMD-group products arranged in a 2×2 threadgroup, direct half inputs, and f32 cooperative accumulators with a fixed K step of 128. It loads a nonzero residual before the first step in accumulate mode. Every thread reaches each scheduling barrier, including inactive output subtiles. Explicit tail extents preserve the original row strides and bound each K segment. The [source plan](plan/PLAN.md) identifies the public API basis and differences from earlier experiments. No guide code was copied.

The baseline is the unchanged production weight-shape planner. It selects 64×128 for the real Flash gate/up/down weights and 64×64 for these synthetic unknown shapes. Candidate output tiles are64×64. This is one architectural organization, not a test of execution scope alone.

## Correctness

Nine small shapes produce 18 checks across overwrite and nonzero-residual modes. Each compares every output at fixed absolute error ≤0.001 and checks every output against f64. Cases include real K=4096/12288, odd K tails, independent inactive row/column subtiles, zero/correlated/mixed-range/cancellation inputs, and token tails. Five complete FFN cases use unchanged real Flash half weights, full finite scans of gate/up/half-activation/down, and 96 fixed f64 outputs per projection. Failure stops before any timing.

Before each token-size timing phase, both complete outputs are computed, fully scanned, and copied into owned immutable CPU Data. A separate down-only expectation uses the common baseline half activation. Every timed dispatch or complete FFN must then equal its own frozen expectation byte for byte. Inputs, redzones, and expectation hashes are checked afterward. The timed down projection always uses the same baseline activation; complete FFNs use their respective activations. The half activation gate is finite and absolute difference ≤0.001 OR at most one Float16 ULP; signed zeros have distance zero. Both absolute and ULP errors are recorded. F32 projection and f64 sample gates remain 0.001, and ultimate full-model raw-logit acceptance remains 1e-3. This version 2 rule was frozen before the first compile or GPU run: different f32 reductions can straddle a half-rounding boundary whose step exceeds 0.001 above magnitude 2. The absolute branch preserves the existing allowance near zero. [The change record](criterion-revision-v2.json) preserves earlier source hashes. Timed repeats remain bitwise exact.

## Fixed timing and limits

There are five ABBA rounds at 512/1024/2048 tokens, with gate, up, down-accumulate and complete-FFN scopes: 240 timing rows. Including correctness and expectation setup, bench mode has 295 command buffers and 523 dispatches; check mode has 46 and 76. Summaries require exact unique coverage, finite positive GPU and wall times, successful cleanup, all numerical checks, and every repeat check. Material improvement requires a paired median ratio ≤0.90 for both complete FFN and projection sum at every size. Shared-machine drift remains possible; these are not full-model latency results.

The process guard is 2 GiB, with 14 GiB available before launch, a 12 GiB host floor, 60-second external and 50-second internal deadlines. There are no automatic retries, cap increases, or variants. No quiet lease is allowed. Compilation uses a 4 GiB guard, 16 GiB start, 12 GiB floor, 180-second deadline and two compiler threads.

Metal buffer allocations stay below 1 GiB. Metal currentAllocatedSize plus owned CPU expectations and any temporary mapped asset bytes must stay below 1.5 GiB, enforced before allocations and after commands. At 2048 tokens the largest explicit Metal allocation set is 889,198,592 bytes. Two full CPU expectations plus one down-only expectation use 603,979,776 bytes. Their overlap during capture is 1,493,178,368 bytes (1424.008 MiB); Metal driver overhead must also fit the unchanged 1.5 GiB limit. The initial Metal workspaces must be released before timing. Steady timing payloads total 1,258,295,808 bytes (1200.006 MiB). Residual values are written in place. The 100,663,296-byte mapped weight temporary exists only during initial loading. [The manifest](manifest.json) records exact component accounting.

## Files and reproduction

`candidate.metal` is the only new shader. `baseline.metal` composes the preserved production extracts. `probe.swift` owns validation and commands; `guard.py` owns only its launched process group. `build.py` pins source and artifact hashes. `source_check.py` checks mappings, counts, memory arithmetic, source identity and Python syntax without compiling or reading weight payloads. `summarize.py` rejects incomplete output. Real half assets remain in `../gemm-tile-probe`, pinned by `weights.json`; do not copy them into a publication bundle.

After explicit scheduling authorization, use the usual build+memory leases for `python3 guard.py build --output build-1`, then GPU+memory leases for `python3 guard.py bench --output run-1`. Run `python3 summarize.py run-1` afterward. Keep every failed receipt. A numerical or timing pass here still requires full-model logit validation before any product integration.
