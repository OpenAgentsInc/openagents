# Kev 4B: one retained Metal evaluation pass

A completed `support-v2-three-way` pass on Apple M5 Max retained all 157
calibration and development items: 117 correct, no refusals, and no harness
loss. Nearest-rank latency was **109.299042 ms p50** and **127.842208 ms p95**.
The locked partition was not read. This is one evaluation pass, not an
eight-block latency sweep or a measured noise floor.

The remaining latency work was deferred by the operator. This record supplies
partial evidence for [#9393](https://github.com/OpenAgentsInc/openagents/issues/9393)
and [#9382](https://github.com/OpenAgentsInc/openagents/issues/9382); it does not
close either issue or replace their published gates. The shared quiet store is
unchanged. Raw results are retained independently in
[`rows.jsonl`](2026-09-20-kev-4b-metal-pass/rows.jsonl).

## Conditions and scope

The controller ran from 2026-09-20T21:20:21Z to 21:21:05Z, including startup and
cleanup. Evaluation began at 21:20:41Z. Host: Apple M5 Max, macOS 26.4, build
25E246. Source checkout: `5301b2141e9a24704fdc3bd6e0180361ebb60a78`;
the only reported untracked path was `.claude/`.

Load averages before were 2.337, 3.498, and 3.962; afterward they were 2.334,
3.345, and 3.882. Five-second samples and process snapshots are retained.
The coordinator observed another Kev process, PID 94153, resident and idle at
launch. A later Kev process, PID 5131, was observed after the pass. Those
observations do not establish an exclusively occupied host or continuously
prove that every other process was idle. Treat this as a pass with recorded
host conditions, not proof of an uncontended latency distribution.

This Apple Metal pass and the earlier Linux CPU small-Kev passes differ in
host and backend. Their timings do not isolate checkpoint size or establish a
controlled speed comparison. Local cost remains `unmetered_local_lane`.

## Exact measured identity

- Loaded-content digest:
  `sha256:2559d7a66cd0f4077d7f47b5460eedaf8211a43b917588638cc2da1f459628f9`.
- Base revision: `906bfd4b4dc7f14ee4320094d8b41684abff8539`.
- Execution: Metal, `f32` backbone and head, `eager-block-causal-v1`,
  `fp32-before-cast-v1` LoRA merge, option isolation `false`, and state/branch
  token bounds of 8192 each.
- Suite digest:
  `54fbf4137c3de538f2dea07d47ca1ee835c09eb25aa26a320441679129f618f9`.
- Question digest:
  `9745b1d9a0f3828888762a33ab36dc1b5093708b79febdd1ec7af2d7571ce0ec`.

Every retained v3 row names the same content digest and execution settings.
The model card preserves each loaded file's digest and size. Historical rows
without this identity are not backfilled or rewritten.

## Commands and artifacts

The exact absolute commands and binary hashes are in
[`run.json`](2026-09-20-kev-4b-metal-pass/run.json). In abbreviated form:

```sh
kev-serve --adapter-dir ~/work/kev-artifacts/kev-4b \
  --base-dir ~/work/kev-artifacts/qwen3-4b --default kev-4b \
  --device metal --dtype fp32 --port 11459
gym eval --suite crates/gym/suites/support-v2-three-way.json \
  --door kev-4b=http://127.0.0.1:11459 --timeout 300 \
  --record /tmp/openagents-apple-handoff/quiet-kev-4b.jsonl --fit
```

The evaluation returned zero and cleanup reported no errors. The controller
validated 157 unique expected open items, all timings, zero harness loss,
and matching content and execution identities. `--fit` printed calibration
comparisons; no `--records` argument wrote or installed calibration maps.

Raw-row SHA-256:
`98b877c5917cda74633415040cee5dcf454e02d552533e56911a68f10b855cf0`.
The [artifact hash inventory](2026-09-20-kev-4b-metal-pass/sha256.json)
covers the unchanged rows, run metadata, model card, eval/server logs, load
samples, and before/after process snapshots. No additional model calls were
made to prepare this record.
