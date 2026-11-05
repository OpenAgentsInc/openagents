# Room vitals at their actual boundaries

This is a tested part of #12840, following the pick gauges in fae22e7101.
It qualifies native frame accounting and durable-save observations. It does
not qualify end-to-end browser presentation, clock-offset uncertainty, the
complete reservation/capacity view, or total instrumentation overhead.

Source-scope correction: the reused world unit in these component artifacts
has an older Fire layout than a later fresh emission. Its retained source key
is listed below; the native and worker artifacts agree with each other, but
this record does not qualify the current integrated world source. Queue/save
boundary unit checks and the explicitly pinned artifact observations remain
separate evidence. The subsequent
[capacity build](2026-11-05-room-capacity-vitals.md) emits the world afresh and
reruns the affected native checks.

The older native counter sampled every fourth D frame and multiplied its
entries by four. It also counted intended recipients after a socket queue
refused the frame. `frame-observe.coax` now accounts for every accepted copy.
The full D frame, including WebSocket framing, equals its header plus entry
classes. Inbound frames include the client mask. Kernel writes, refused
queues, and queued bytes discarded on close are separate observations.
Serialized row text and deferrals describe the production v3 serializer
before queue admission. Picks, serialized rows, queue acceptance, transport,
and display are distinct boundaries.

The separate server-queued ACK histogram ends only when the addressed final
D frame enters the socket queue. Multipart prefixes and duplicate or stale
ACKs cannot consume that sample. The historical input-to-ACK chart retains
its prior RTT estimate. Neither histogram measures client presentation.

Profile 2 save observations come from the exact matching durable receipt,
including owner, process, revision and API acceptance timestamp. Capture age
and receipt age use the local monotonic clock. The latest pending capture
stays separate. Legacy a/d receipts have their own durable marker: expiry,
eviction, refusal, stale results and a bare historical success cannot invent
an accepted revision. Legacy ages retain their declared wall-clock scope.

The existing admin view gains replication/save/capacity series, named worst
selection and serialization seats, exact queued ACKs, accepted saves and
delivery details. Its section capacity is 128, covering eight reporting
shards. Report omissions and saturated counters explicitly mark incomplete
observation. The launch runbook explains scopes and response thresholds.

## Verification

All processes and services use private temporary state and their own ports.
No production service or database is involved. Commands run niced with one
Coax job. Raw results are in `room-vitals-boundaries-2026-11-05/`.

- Full room suite: 643/643. Exact-frame authority mutation: 168/168 killed,
  zero survivors, root-only with dependency checks retained, no equivalents.
- Admin Coax suite: 47/47, including the regression which failed with the
  previous 32-section limit. Background fixture suite: 233/233, including
  the explicit legacy-control/observer distinction for paired qualification.
- Full `coax/web/api/test.sh`: passes, including 151 room checks, the store
  checkpoint checks, 98 CLI checks and the other API suites. This run
  precedes the final queued-ACK/worst-interest dashboard assertion; the
  selected runs verify the new assertions: 193/193 with real native room
  faults, and 153/153 for the final dashboard including omission/overhead
  numbers beside their scopes. Both selected API commands finish their CLI,
  anime and SQL checks successfully.
- Scratch-store native checkpoint slice: 37/37. It loses HTTP replies,
  refuses writes for more than 38 seconds, pauses the old process, replaces
  its owner and resumes it. The new observations are checked against the
  independently committed Postgres body. During the outage: 16 failed
  attempts, 15 retries, one pending capture, one paused encounter, accepted
  revision 5 and latest revision 15. The stale process reports one fenced
  receipt. The healthy replacement restores in 420 ms in this run.
- Native mixed stress, 75 seconds: eight pets, four zones, 37 raiders,
  1,040 casts, 58 explicit busy outcomes. Independent room: 2,249 frames,
  maximum 85 ms gap. All 150 health probes pass through the 35-second HTTP
  outage; background lanes recover. The slow consumer closes and rejoins;
  no other consumer closes. The final report has 2,845 actual queued-ACK
  samples (p50 18 ms, p95 34 ms), 9,573.2 serialized rows/s, no omitted lines
  and no saturated counters. Tick p50/p95/p99: 0.67/1.43/1.61 ms.
- Native final-host warm check: all sixteen worlds stream, publication
  completes, 60/60 health probes pass, and the independent room's maximum
  frame gap is 38 ms.
- `tools-check`: zero new files missing a reason.

The mixed-stress report takes 60 us and its clocked observer spans plus report
occupy 0.028% of elapsed reporting time. This includes descheduling and excludes
inline serializer, input and transport counters. It is neither CPU utilization
nor a measured bound on total instrumentation overhead. A representative
paired qualification remains open.

## Artifacts and reproduction

These are room-component artifacts using compiler 0.48.0, SHA-256
`cdaceb89e69c8d523e41242a940af34774e8b6b4f945998c2d3bf38486c357fb`.
The compiler reports that its source tree is newer. This explicit component
profile does not confer graphics or desktop compiler admission. The retained
six-unit source-key manifest is included with the raw results.

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| release room worker | 1,030,024 / 1,060,000 | `9d7e47bbac5783c71213ec073defa9b68b97c61464df42b68ba4d508c2851742` |
| admin worker | 53,923 | `252d16dbcd0612082b102f1d625aaf42102eaa3e8c4dd84245d54efc222f6182` |
| native mixed-stress/checkpoint image | — | `5aedf15c4ea079939d6f644500ff95796aad23c011778bdeb8fb327c01960deb` |
| native final host | — | `572c3651cb0dffcf1093262b999a2cdb6356071682e5627199eba28298a02ed9` |

The final host adds the explicit observation-timing scope and reserves space
for that report tail; its Coax units are identical to the stress image.
Unchanged C/header unit pairs reuse their prior O2 object only after byte
comparison. Both images pass `--start-check`; strict floating-point flags and
`room-math.c` are retained.

```sh
nice -n 15 env COAX_JOBS=1 COAX_CHECK_JOBS=1 coax/bin/coax test coax/web/room/room.coax
nice -n 15 coax/bin/coax mutate coax/web/room/frame-observe.coax --root-only --jobs 1
nice -n 15 coax/bin/coax test apps/openagents-web/admin.coax
nice -n 15 coax/web/tool.sh coax/web/room/background-check.coax stress "$ROOM_BIN" 75
nice -n 15 env API_TESTS_ONLY=checkpoint2_checks.coax ROOM_BIN="$ROOM_BIN" coax/web/api/test.sh
nice -n 15 env API_TESTS_ONLY=room_checks.coax ROOM_BIN="$ROOM_BIN" coax/web/api/test.sh
```

Run bounded slices instead of repeating unchanged passing gates. A native room
build is required first; `ROOM_BIN` names that completed image. This record is
not a claim of current-build browser/frame or fleet qualification.
