# Original gpt-oss fixtures and evidence

`original/pins.json` identifies every original artifact at separately pinned HF
revisions. Original configs, dtype metadata, shard index and raw headers are
retained; full original weights stay outside Git in `~/.cache/psionic/`.
`original/notices/` retains the upstream Apache license and usage policy.
Downloaded code was never executed. The fixed runtime offsets derive from the
complete original headers, not a converted checkpoint.

`tiny/` is our deterministic owned two-layer BF16/MXFP4 model, with independent
dense full-prefix scalar references, routes, intermediate residuals and cache
entries. `gptoss_fixture.py` authors/checks it. No external weights are needed
for default checks.

`original/{20b,120b}/qualification/` contains compressed independent full-model
NumPy reference vectors, exact candidate digests and the fixed tolerance verdict
for two prefixes. `qualification/{20b,120b}/` binds final original forward/replay
runs to generated C, compiler/native identities and source hashes. Its generation
log/IDs are earlier completed same-profile smoke observations, not the final
build's latency verdict. `qualification/lab-build.json` identifies the exact
library translation/executable used by the original speculation/tool observations;
it explicitly records which harness build inputs were not archived.

Those earlier original runs are CPU wide with f32 storage, not upstream BF16 activation
parity. Sparse original smoke requests do not establish general model quality,
long-context conformance or production throughput. Simultaneous work affected
latencies. Final broad mutation runs were stopped at the owner's request;
focused changed-code checks and native sanitizers are the final gate.

`decode-products.coax` freezes the old single-token lane/fold program for
address-optimization parity. `qualification/decode-speed-2026-10-29/` separately
records paired original-20b CUDA speed runs: one resident weight allocation,
alternating baseline/candidate modules, exact complete-state and generated-ID
agreement, plus an end-to-end Responses smoke benchmark. These records do not
change the earlier CPU numerical/latency verdicts.
