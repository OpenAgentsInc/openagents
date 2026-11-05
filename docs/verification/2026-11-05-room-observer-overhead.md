# Native observer overhead, paired workload

This measures the queue/save observer added in 2f848a16d0 for #12840 against
a frozen shared world object. A subsequent fresh emission exposes an older
Fire layout in that object. The source-closure comparison below does not join
the reused world C to those source trees. These results remain a paired
artifact/component observation, not current-source qualification. They do not
cover subsequent capacity or latency-trace additions. The earlier 0.028%
timed-span sample excludes inline counters and is not the overhead result.

The Coax `observer-overhead.coax` tool runs the same ten-socket, 75-second
mixed gameplay workload in A/B/B/A order. It calls the completed harness image
directly, so compiler/build time is excluded. Darwin `/usr/bin/time -lp`
records CPU and resource use for the harness and its waited children. Each
run has a 120-second deadline and must pass its actual gameplay, isolation,
outage and slow-consumer recovery gates. The legacy baseline must explicitly
match the control report; it cannot pass the exact-observer report check.

| Run | Profile | Elapsed s | User CPU s | System CPU s | Total CPU s |
| --- | --- | ---: | ---: | ---: | ---: |
| A1 | baseline | 83.49 | 2.82 | 2.78 | 5.60 |
| B2 | queue/save observer | 83.51 | 3.00 | 3.29 | 6.29 |
| B3 | queue/save observer | 83.76 | 3.99 | 4.75 | 8.74 |
| A4 | baseline | 83.52 | 3.46 | 3.60 | 7.06 |

Mean baseline CPU is 6.33 s; observed CPU is 7.515 s, a 1.185 CPU-second
increase (18.72% relative), approximately 1.42 percentage points of one CPU
over the elapsed run. The paired increases are 0.69 and 1.68 CPU seconds.
System CPU and the harness workload vary on this loaded Mac. Four runs do
not establish an isolated-server, confidence-interval, or fleet overhead
bound. The absolute values and scope should accompany the percentage.

All four runs have eight pets, four zones, 37 raiders, 1,040 submitted casts,
explicit busy refusals, 150/150 successful health probes, a slow consumer
which closes/rejoins and an independent healthy room. Both exact-observer
runs reject incomplete/saturated reports and retain actual ACK samples.
The accompanying raw logs contain the per-run tick distributions, bytes,
serialized rows and queue observations.

## Source and artifacts

Baseline source is 8c312f9649, before the queue/save observer, built in a
separate worktree. The observed room is the final host recorded in
`2026-11-05-room-vitals-boundaries.md`. Both use the same explicit compiler
0.48.0 room-component profile, O2/strict float flags, catalog and math.
Every source in all five shared world-unit dependency closures is compared
byte-for-byte between the baseline and observed source trees. However, that
comparison does not establish the source of the reused world C. Its retained
worker source key is `3acb5347bb4a8d4c079cd8a512a5f7c38e4ccc6bc79d1c8b6e2ea664b0ffface`;
fresh emission uses `ad4451901f5d89060fd3087ec4f8e623054c38d12841de3353854d38bc2098f8`
and includes the later Fire gate fields. Both variants in this record use the
same older C/header/object, so the paired comparison is artifact-consistent.
The baseline room unit and host are emitted/built separately. A fresh shared
world comparison in
[the capacity record](2026-11-05-room-capacity-vitals.md) supersedes this record
for current-source qualification.
This is not graphics or desktop compiler admission.

- Baseline native SHA-256:
  `5f741ab49df1e25eb03732475ddde527b3c0341e09f275ee8a406f34900f4450`.
- Observed native SHA-256:
  `572c3651cb0dffcf1093262b999a2cdb6356071682e5627199eba28298a02ed9`.
- Controller: 9/9 Coax checks. Shared harness: 233/233. All four bounded
  native integration runs pass. Full source keys, harness/room/compiler
  hashes and CPU output are in `room-observer-overhead-2026-11-05/`.

```sh
nice -n 15 env COAX_JOBS=1 COAX_CHECK_JOBS=1 \
  coax/web/tool.sh coax/web/room/observer-overhead.coax \
  "$HARNESS_BIN" "$BASELINE_ROOM_BIN" "$OBSERVED_ROOM_BIN" "$OUT_DIR"
```

The three binaries must be built first. `HARNESS_BIN` is the compiled
`background-check` tool, not `tool.sh`, which would include build time.
The controller preserves stdout/stderr for each run and fails on missing
CPU samples, timeout, truncation or a failed workload gate. No production
services or data are involved.
