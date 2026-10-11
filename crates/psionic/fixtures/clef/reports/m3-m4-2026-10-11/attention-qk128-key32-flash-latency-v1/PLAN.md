# Flash attention latency comparison

This plan measures the incremental effect of QK128 scoring with unchanged
key32 updates. It compares the newly validated candidate with frozen Stage6
and a separately owned Ollama server. It has not run. Root review and a
scheduled window after the approved CPU layer35 replay are required.

The candidate already passed 25 CPU tests, four focused GPU tests, all Flash
and 27B corpus/long-history comparisons with exactly equal logits, and the
six-image/ten-refusal suite. These timing runs do not repeat a quality gate
or infer a model speedup from the attention microbenchmark.

## Fixed design

Use the existing `mac_quiet_bench.py` server, host monitoring, nonce generation,
cache validation, and request loop. Two rounds reverse the order:

1. Stage6 key32, candidate QK128/key32, Ollama.
2. Ollama, candidate QK128/key32, Stage6 key32.

Each owned server receives the same three retained request bodies, targeting
1k, 4k, and 16k tokens. Each workload has one warmup and five measured requests
per round: 108 requests total, comprising 18 warmups and 90 measured requests.
A fresh leading state nonce makes each sample unique. All three variants use
identical bytes and token counts for each paired sample. Preserve every raw
sample, including warmups, outliers, failures, queue time, and host load.

Both native variants use chunk 2048, resident-f16 weights, fused-tensorops
attention, weight-shapes GEMM, fused-fields head, sequential Delta, batch
requests 1, prefix-cache bytes 0, and two CPU threads. Profiling and export
are absent. They differ only through their frozen binaries. All native
responses must report zero cached tokens and no cached timing. Ollama uses
its existing isolated server setup, fresh port and scratch HOME, one loaded
model, one parallel request, no cloud, and no model pruning. Its internal
prefix reuse is not directly observable when it omits cached-token counts;
unique leading nonces prevent reuse of complete request answers.

Stage6 server SHA-256:
`77c74a2cf4c737ae0b92278e7f8384a213dc53f1567963fb6b8c6d0dc7157991`.
Candidate SHA-256:
`f4def388b4b6622fef9023c2616639c215829d7a519359eb64d1dbca9fdfa31f`.
`provenance.json` pins all binaries, harness dependencies, exact input files,
native GGUF, source freeze, accepted correctness receipts, Ollama manifest,
and the declared hashes/sizes of its 893 blobs. Before timing, the wrapper
verifies these files by streaming, with a separate 120-second hash deadline.
No model or blob is downloaded or converted.

## Bounds and scheduling

All six server groups use the existing 36 GiB sampled process-group cap,
48 GiB startup requirement, 12 GiB available-memory floor, 180-second server
startup limit, and 300-second request limit. The full measurement has a fixed
1,800-second deadline after preflight. It uses GPU and memory36 leases and
explicitly refuses a quiet lease. No host app, service, power setting, or
unrelated process is controlled. The caller must ensure no task-owned build,
CPU model, or second GPU workload overlaps the timing window.

Shared-machine load limits remain 150 at start and 180 during the run, matching
the retained comparison. Load and total GPU activity are recorded; background
interference is possible. Any failure stops this attempt and preserves all
receipts. No retry, threshold change, or larger resource reservation is
automatic. No 27B or 76 GiB workload is included or authorized by this plan.

## Analysis

The analysis requires exactly 108 unique coordinates, the six expected owned
server starts and clean shutdowns, valid finite times, complete typed answers,
and matching request bytes and actual token counts across every paired sample.
It rejects missing/duplicate samples, cache hits, truncated native state,
changed artifacts, malformed phase times, or failed resource/cleanup checks.
Actual token ranges are bounded at 950–1,200, 3,500–4,500, and 15,000–17,000;
exact observed counts remain in the output.

Report every paired ratio, per-round medians, combined medians, and ordinary
encode/queue/prefill/head times separately. Compare the candidate with Stage6
and Ollama. Check the M3 upper latency targets of 0.35, 1.3, and 7.5 seconds
against candidate HTTP medians, labeled with actual token counts and shared
conditions. These data do not establish idle-machine performance or identify
a hardware cause. Warmups are excluded only from the reported medians and
remain in the raw and analyzed records.

After approval, run `run.py` under the GPU and memory36 leases, recording both
lease receipts in this directory. Do not launch the benchmark from this plan
without the explicit scheduling handoff. `run-receipt.json` and `run-1/` must
not already exist. The current preparation contains neither.
