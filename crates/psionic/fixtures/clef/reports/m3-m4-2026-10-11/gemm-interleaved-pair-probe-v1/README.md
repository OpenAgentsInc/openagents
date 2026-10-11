# Interleaved gate/up projection result

Reject product integration. The single fixed candidate passed every correctness gate but missed the predeclared 10% paired speed improvement. No retry or product change followed.

| Tokens | Baseline full FFN median GPU ms | Candidate full FFN median GPU ms | Median paired candidate/baseline |
| --- | ---: | ---: | ---: |
|512|2.827854|2.964875|1.019823|
|1024|6.962313|6.485896|0.929939|
|2048|11.458312|12.324583|1.058406|

Paired projection-sum ratios were 1.017479, 0.955627, and 1.064982. Gate/up alone was 1.039980, 1.072301, and 1.096887. The fixed material-gain rule requires full-FFN and projection-sum paired ratios≤0.90 at every size. Standalone down deliberately uses identical code and activation, providing a control for timing variation. These are shared-machine measurements; the one-minute load average ranged 10.875–10.951. Do not infer a thermal, bandwidth, frequency, or scheduling cause.

The candidate packs corresponding 128-column gate and up tiles into one 256-column weight tile and performs one 64×256 full-K MPP operation with four SIMD groups. F32 intermediate output feeds a separate SiLU kernel with the production expression; down multiplication is unchanged. Packing is validated and reported separately from GPU timings. The packed persistent weight allocation is additional; this experiment does not claim reduced full-model memory.

All 18 small and five actual-weight checks passed. Every bounded small output was checked against an independent f64 calculation. Dense weighted cases used 96 fixed f64 samples per projection plus complete output comparisons, not a full dense f64 reference. Gate, up, half activation, and down matched the baseline bitwise. The largest candidate f64 error was 1.6751932889746968e-5. All 240 timed commands exactly matched immutable expected bytes. Ten packing records checked row identity and zero padding. Redzones and source/input/weight/expectation hashes stayed intact. Totals were 292 command buffers and 602 dispatches.

F32 and f64 absolute limits remained 0.001. The half activation criterion, frozen before compilation, was finite and absolute error≤0.001 OR distance≤one half ULP, with signed zeros sharing one value; no allowance was needed because activation was bitwise equal. This microprobe uses actual Flash layer 0 half weights with synthetic inputs. It establishes neither full-model latency nor independent BF16 answer quality. The unchanged full-model raw-logit 1e-3 and top-answer gates were not run for this rejected candidate.

The first build passed in 6.7246 seconds. The single bench completed in 9.7913 seconds. Peak sampled owned-process memory was1,853,147,056 bytes under the 2 GiB cap. Explicit buffers peaked at 855,642,624 bytes, owned expected bytes at 570,425,344, and their maximum combined with device.currentAllocatedSize at 1,426,931,712 bytes under 1.5 GiB. Start 14 GiB, host floor 12 GiB, and deadline 60 seconds were unchanged. All owned children and build/GPU/memory leases released; sampler and cleanup errors were absent. No quiet lease or desktop/system-service control was used.

[Ordered output](bench-1/stdout.jsonl) retains every check and timing. [Summary](bench-1/summary.json) retains all paired ratios and the failed material-gain decision. [Build artifacts](build-artifacts.json) preserve exact executed source, executable, and metallib hashes. The executable SHA256 is 2f4990fadaf5bd3c8cbc5a5f65ee6175c40e9467388eb3fb580d361331dd7feb; the metallib SHA256 is b48a0d212239823c84a2e7991f18ffc911ae16592db7c5d13ac38310c15d5246. Binaries, weights, and memory timelines are excluded. The original preparation document remains [PREPARED_PLAN.md](PREPARED_PLAN.md); its unrun status describes preparation only.

To reproduce after explicit resource authorization, set CLEF_REPO to a checkout matching recorded production and guard hashes, place matching assets in the sibling gemm-tile-probe directory, and run guard.py build under build+4 GiB memory leases, then guard.py bench under GPU+2 GiB memory leases with fresh output names. Run summarize.py afterward. Never use a quiet lease. Published path changes produce new source/build hashes; original receipt fields retain executed hashes unchanged. [Sanitization manifest](sanitization.json) maps original hashes to published bytes. No failed run was discarded; the failed speed decision is preserved.
