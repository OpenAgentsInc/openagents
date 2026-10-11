# Flash latency after batching attention scores

The single planned shared-machine comparison completed all 108 requests:
90 measured requests and 18 warmups. Exact request bytes and actual token
counts match across Stage6, the QK128/key32 candidate, and Ollama. All six
owned servers stopped, with no resource, sampling, or cleanup errors.
However, the all-engine uncached gate fails: Ollama reported prefix reuse
on 31 of 36 requests. Native-only uncached validation passes. The candidate
does not establish a consistent whole-model speed improvement, and none of
the three target medians is met.

| Workload | Actual tokens | Stage6 median (s) | Candidate median (s) | Ollama median (s) | Median paired candidate / Stage6 | Median paired candidate / Ollama |
| --- | --- | --- | --- | --- | --- | --- |
| len1k | 1079, 1080, 1081, 1082, 1083 | 0.674974 | 0.511324 | 0.817950 | 0.8990 | 0.6493 |
| len4k | 3915, 3916, 3918, 3919, 3921 | 2.256425 | 2.374397 | 2.817055 | 1.0418 | 0.8363 |
| len16k | 15506, 15508, 15510, 15511, 15512, 15513, 15515 | 10.002488 | 10.325572 | 11.756560 | 1.0385 | 0.8795 |

Ratios below 1 favor the candidate. Each workload has ten measured pairs,
five in each of two reversed server-order rounds. `run-1/analysis-v3.json`
keeps both rounds, every paired ratio, warmup samples, and separate native
encode, queue, prefill, and head timings. The total-time columns are HTTP
latency, not prefill or kernel timings. Per-round results matter because
host conditions changed during this shared-machine run.

The candidate meets 0 of the three fixed HTTP median targets
(0.35 s, 1.3 s, and 7.5 s respectively). These are observations from this
run, not idle-machine results or evidence for a particular hardware cause.
Correctness was checked separately before timing: Flash and 27B had exactly
equal raw logits for 104 corpus answers and six long-history answers each.
This bundle makes no new 27B latency claim.

## Fixed experiment

Both native servers use chunk size 2,048, resident-f16 FFN weights,
fused-tensorops attention, weight-shapes GEMM, fused-fields scoring, and
sequential DeltaNet. Batching is disabled with `PSIONIC_CLEF_BATCH_REQUESTS=1`;
prefix reuse is disabled with `PSIONIC_CLEF_PREFIX_CACHE_BYTES=0`. Native
cached token counts are zero, and no state is truncated. Every paired
request has a unique leading nonce, with the same body sent to all three
variants. Full responses and all raw samples are retained.

Ollama runs as a separate task-owned server on a fresh local port, with one
parallel request and one loaded model. Its pinned existing model directory
is used without changing a running service. Unique leading nonces prevent
identical full-request reuse. Ollama's top-level `prompt_eval_cached_count`
explicitly reports
0 tokens on five requests, 37 on 14, 38 on 14, and 39 on three. These are
skipped prefix tokens in the pinned release source. The nonce prevents
identical full-answer reuse but leaves a shared template prefix. Therefore,
Ollama timing columns are descriptive and have a known cache asymmetry;
they do not pass the intended uncached-comparison gate.

No task-owned builds, CPU model runs, or other GPU model runs overlapped this
window. Other desktop activity was left running. Host load and memory were
sampled throughout. No quiet lease, desktop application, service, power
setting, or process priority was changed.

## Offline analyzer corrections and cache audit

The original `run-receipt.json` remains failed and unchanged. Its post-run
analyzer first failed because the configured Ollama symlink resolved to a
different path spelling from the pinned hash map. Separately named v2
resolves unique binary identities and rechecks their hashes. It then
correctly preserved a second failure: the reused native image validator
expected top-probability confidence for Ollama.

[Ollama v0.40.2 source](https://github.com/ollama/ollama/blob/b061384d90ff455462bc32745dd0de479de717a3/decision/systemone.go)
defines confidence as normalized distribution concentration. All 72 Ollama
choice/score confidence fields match that formula within 2.22e-16. V3 checks
that actual formula to 1e-12, while native validation remains unchanged.
Typed answers, probability coverage, top choices, score expectations,
actual token counts, exact paired bodies, all samples, and cleanup pass.
Fifteen offline checks cover valid and malformed responses, canonical binary
identity, and the retained cache failure. All 36 paired native answer
objects are exactly equal. No response was rewritten and no model reran.

V3 records every top-level cached count and returns `passed: false` for the
all-engine uncached comparison. The frozen plan and earlier analyzers are
retained as written, including their incorrect assumption that Ollama cache
telemetry was unobserved. This README and `cache-source-audit.json` correct
that interpretation. The scoped public-source review found no supported
cache-disable request or environment option in the inspected MLX scoring
path; no claim is made about uninspected or future controls.

`retrospective-cache-audit.json` also records unchanged earlier evidence:
Flash Stage6 has counts 0×4, 37×11, 38×19, 39×2 across 36 requests; 27B
Stage6 has 0×4, 37×7, 38×6, 39×1 across 18. Their earlier unobserved-telemetry
claims are incorrect too. Original file hashes and every sample count are
included. `proposed-shared-cache-check.patch` is a source-only proposal to
reject nonzero or malformed top-level cached counts; it was not applied.

## Identity and resources

Candidate server SHA-256:
`f4def388b4b6622fef9023c2616639c215829d7a519359eb64d1dbca9fdfa31f`.
Stage6 server SHA-256:
`77c74a2cf4c737ae0b92278e7f8384a213dc53f1567963fb6b8c6d0dc7157991`.
The exact native model, all 893 Ollama blobs, binaries, harness sources,
requests, and prior correctness receipts were verified before measurement.
The hash pass took 9.408 seconds and was
outside the timed samples. The whole attempt took 583.905
seconds, including startup, cleanup, and verification.

Each server had a 36 GiB owned-memory cap, a 48 GiB startup requirement, and
a 12 GiB available-memory floor. Startup was limited to 180 seconds and
each request to 300 seconds. The hash pass had a 120-second limit; the
measurement had a fixed 1,800-second limit. Peak accounted owned memory was
23,052,756,760 bytes; the lowest available memory was
68,548,001,792 bytes. The GPU and memory leases
were released after all six server groups stopped. There was one attempt,
with no retry or expanded bound.

## Reproduction and retained data

`PLAN.md`, `config.json`, `run.py`, and `analyze.py` preserve the original
fixed order and gates, including the two analyzer defects. The separately
named v2/v3 analyzers, diffs, failed receipts, and offline checks document
the corrections without replacing the original evidence. The reusable
harness and original request bodies are included.
`provenance.json` and `preparation-manifest.json` bind their original hashes.
`publication-manifest.json` maps original and scrubbed hashes. Local roots
are replaced by `<LATENCY>`, `<SCRATCH>`, `<WORKTREE>`, and `<OWNER_HOME>`;
restore those paths before reproducing. Logs larger than the publication
limit are split at complete lines. No binaries, model weights, or raw tensors
are included. Full answers, timings, host samples, memory checks, server
logs, and cleanup receipts are retained.
