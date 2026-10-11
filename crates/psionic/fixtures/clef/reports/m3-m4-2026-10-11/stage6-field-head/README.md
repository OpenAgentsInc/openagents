# Stage6 field-head validation

The opt-in fused field head passed the complete text comparison against the staged head for both Clef Flash and Clef 27B: 40 requests and 104 questions per model. Both modes used the same server, GGUF, head, requests, and trunk settings. This establishes agreement with the same quantized model; it does not establish the independent 27B BF16 accuracy gate.

| Check | Result |
| --- | --- |
| Flash head: 104 raw-logit and HTTP top comparisons | Pass; maximum absolute logit difference 9.5367431640625e-7 |
| 27B head: 104 raw-logit and HTTP top comparisons | Pass; maximum absolute logit difference 5.960464477539062e-7 |
| Stage6 27B staged and fused heads versus official BF16 text | Both fail: 93/104 top agreements; maximum probability difference about 0.217407 |
| Stage6 27B fused image acceptance | Pass: 6 cases, 14 top answers, 10 refusal checks; maximum probability difference 0.0017794304477648515 |
| Head logit limit | 1e-3, unchanged |
| Stage6 selected synthetic GPU checks | 21 backend and 7 serving tests passed |
| Earlier field-head backend CPU checks | 25 passed, 21 ignored |
| Earlier block16 backend checks | 24 CPU and 19 selected GPU tests passed |
| Stage6 build/check sequence | Server built; serving suite failed: 72 passed, 1 failed, 9 ignored |
| Block16 weighted standalone comparison | Failed; maximum absolute logit difference 1.08385086e-3 |
| Block16 weighted combined comparison | Failed; maximum absolute logit difference 1.35493279e-3 |

Block16 does not meet the strict 1e-3 weighted acceptance limit. Unchanged top answers and successful synthetic fixtures do not override either weighted failure. This bundle makes no claim that shorter blocks improve numerical accuracy or meet a performance target.

## Evidence map

- [Flash head comparison](field-head/flash/summary.json) and [27B head comparison](field-head/clef-27b/summary.json) contain every comparison. Each directory also includes all 40 request bodies, all 80 HTTP responses, startup/shutdown records, aggregate host samples, server logs, and both complete exported raw-logit metadata files.
- [Flash pooled-row hashes](field-head/flash/pooled-row-hashes.json) and [27B pooled-row hashes](field-head/clef-27b/pooled-row-hashes.json) record whole-file and all per-question hashes. Packaging rechecked byte equality against the original tensors. The approximately 21 MiB of raw pooled f16 data is omitted; the original source runs retain it.
- [Initial startup failure](field-head/failed-first-run/summary.json) and its [server log](field-head/failed-first-run/server-r0-staged.log) preserve the rejected `PSIONIC_CLEF_BATCH_REQUESTS=0` harness setting. [The correction](field-head/harness/harness-correction.json) sets it to `1`, the valid disabled-batching value. The original failed harness and corrected harness are both included. The initial failure produced no accepted model responses.
- [Original Stage6 checks](stage6-build/checks.json), [guard receipt](stage6-build/guard.json), and [serving log](stage6-build/serve.log) preserve the failed CPU assertion at a block32-era scratch threshold of 7,274 tokens. Compilation succeeded; the serving test failed. The build/check wrapper stopped before its later commands. Any corrected CPU retry is outside this immutable bundle.
- [Stage6 source manifest](stage6-build/source-manifest.json), [server identity](stage6-build/server-binary.json), and [test executable identities](stage6-build/test-binaries.json) identify the frozen source and artifacts. Earlier candidate manifests remain separate and are not relabeled as a later source state.
- [Field-head candidate checks](candidate-field-head/guard.json), [GPU receipt](candidate-field-head/gpu.json), [frozen sources](candidate-field-head/frozen-sources.json), and [CPU log](candidate-field-head/backend-build.log) preserve the preliminary field-head validation.
- [Earlier block16 checks](candidate-block16/small-gpu-checks.json) and [source description](candidate-block16/source.json) preserve the candidate tests before weighted acceptance.
- [Stage6 small GPU receipts](stage6-gpu/small-gpu-checks.json) and [weighted block16 failures](stage6-gpu/gpu-retry-checks.json) contain the final selected tests included here. Both weighted output logs retain the exact offending token/chunk/question/option coordinates.
- [Offline 27B BF16 text comparison](bf16-text-comparison/results.json) retains all 40 cases and 104 question comparisons for both staged and fused heads, including every mismatch, decision type, top-two margin, and probability difference. The [unchanged reference](bf16-text-comparison/reference-text-mps-bf16.json), [suite](bf16-text-comparison/requests.jsonl), and [offline comparison script](bf16-text-comparison/compare_saved.py) are included.
- [Stage6 fused image results](images/results.json), [owned server receipt](images/run.json), [config](images/config.json), and [unchanged image reference](images/reference-images-mps-bf16.json) preserve the completed image check and all ten HTTP refusals.
- [Offline packaging verification](verification.json) records coverage and rechecked identities. [The manifest](manifest.json) maps every copied input to its original SHA-256, published SHA-256, byte length, and sanitization changes.

## Comparison scope and limits

The server SHA-256 is `77c74a2cf4c737ae0b92278e7f8384a213dc53f1567963fb6b8c6d0dc7157991`. Flash used chunk 2,048 and its GGUF head. Clef 27B used chunk 512, the official BF16 joint-head artifact, and on-demand Q4 FFN weights. Both head comparisons enabled fused TensorOps attention and shape-selected GEMM, disabled prefix reuse, used one request slot, and compared staged versus fused field heads. Delta remained on its default sequential path in these head comparisons.

The checks require finite raw logits, complete request/question coverage, matching option coordinates, exact input-token counts, identical pooled f16 rows, unchanged model metadata, and matching raw and HTTP top answers. Native token IDs are not exported. Their equality is inferred from identical request bytes and the same encoder, not directly observed or proved by matching counts alone.

These were correctness runs on a shared Mac. CPU compilation overlapped the head comparisons. HTTP timings and host-load records are preserved for context, but they are not clean latency measurements. No quiet lease or desktop/system-service changes were used. Process guard receipts show sampled accounting and cleanup for owned groups; sampled peaks do not prove a hard allocation ceiling, especially for short tests.

The Flash `cpu-golden-comparison.json` is an additional comparison against an existing Flash CPU golden, identified by its hash. It is not a new independent 27B BF16 validation. The 27B accuracy shortfall documented in earlier evidence is not resolved by these head checks; the current strict result is recorded below.

## Current 27B accuracy gate

An offline comparison of the saved Stage6 responses against the unchanged official BF16 text reference gives 93 of 104 matching top answers for both staged and fused heads. All 40 token counts match. Maximum probability differences are 0.2174068876822337 for staged and 0.21740690247160716 for fused fields. Both fail the original M1 gate of 100% top agreement and maximum probability difference at most 0.02. The script calls the existing `image_parity` response-validation, top-answer, margin, and summary functions and verifies the original numeric request encoding, saved HTTP status, request/response hashes, finite reference logits, and complete coverage. It launches no model or server.

These Stage6 results supersede no earlier evidence: the earlier Stage4 snapshot had 94 of 104 matching tops. The current head change preserves its same-Q4 baseline, but the independent text accuracy gate still fails. The bundle does not attribute the change from 94 to 93 to a particular optimization.

The separate image run passes the issue's image acceptance: six cases, 14 matching tops, matching token counts, and ten typed HTTP refusals. Its maximum probability difference, 0.0017794304477648515, is reported as a diagnostic; the image gate is not a text numerical acceptance gate. Images use the fused field head, fused attention, shape-selected GEMM, on-demand Q4 FFN weights, and sequential Delta. This image success does not cancel the text failures.

## Reproduction and publication

The archived scripts and configs preserve their executed settings with private paths replaced by labels. They are records, not immediately runnable commands. `reproducer/compare_heads.py` imports the adjacent helper snapshots and accepts a local config with actual model, server, suite, and new output paths. These helper snapshots were copied at packaging time; their hashes are in the manifest, but the Stage6 source manifest did not capture Python helpers. The corrected harness hash in the run summaries identifies the executed comparison logic.

Use the pinned server/source and artifact hashes, resolve the config labels locally, and keep the recorded GPU and memory leases, owned-process guards, bounds, and CPU thread limits. No quiet lease is required or requested by this experiment. Re-running model checks is separate from verifying this bundle and requires an available resource slot. The offline BF16 comparison can be repeated with `python3 bf16-text-comparison/compare_saved.py --reference bf16-text-comparison/reference-text-mps-bf16.json --run field-head/clef-27b --suite bf16-text-comparison/requests.jsonl --rules reproducer/tools/image_parity.py --output new-comparison.json` from this bundle. Because the reference checkpoint path was sanitized, that rerun records the published reference file hash; the manifest retains the original hash and the numerical comparisons are unchanged.

Publication substitutes the task scratch root with `run-artifacts`, the session root with `session-artifacts`, the checkout with `workspace/openagents`, and the private home with `user-home`. Numeric logits, probabilities, request bodies, failure output, source hashes, and guard decisions are unchanged. Binaries, weights, raw pooled tensors, and full memory timelines are excluded; receipt references to omitted files remain as original record labels. No unrelated process command lines are included.
