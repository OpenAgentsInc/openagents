# Aethyrforge offline room: native and Worker

Issue #12866, measured 2026-11-05 on the owner's M5 Max MacBook Pro (18 CPU
cores, 128 GB; macOS 26.4, 25E246). Runtime artifacts were built from
`dfc7c0b1f2` plus this change; their SHA-256 digests are in `artifacts.sha256`.
Both hosts use the same Coax room, room-tick.c, world document, one local-1
seat, strict floating-point flags and room-math.c. No model, microphone,
provider session or network service is needed for this play path.

| Observation | Native room | Wasm Worker |
| --- | ---: | ---: |
| First rendered frame, seconds | 1.80 | 3.01 |
| Recorded interval, seconds | 15.36 | 15.65 |
| Actual Workers created | 0 | 1 |
| Room frames received | 468 | 476 |
| World callbacks | 1,738 | 1,850 |
| Final tick p50 / p95 / p99, milliseconds | 0.25 / 0.33 / 0.43 | 0.60 / 0.70 / 0.74 |
| App CPU, % of one core | 3.39 | 4.47 |
| Web process CPU, % of one core | 16.21 | 32.71 |
| Native room CPU, % of one core | 1.11 | 0.27 |

These are observed costs of each platform's host, not an instruction-level
benchmark. Native vitals report 10-second histograms in 10 microsecond bins;
the Worker reports individual tick quantiles over each second, including
flush/postMessage work. The Worker observation is its last complete bucket,
not a percentile computed by pooling bucket percentiles. Both complete raw
reports, including every per-second observation, are committed. The unused
native server remains alive in the Worker comparison to serve the same world
API. Other agents were building on this Mac, and the drift check was running
in muted headless Chrome during these samples. These numbers do not qualify
maximum throughput or display latency. World callbacks can exceed submitted
GPU frames under the existing frame budget.

## Repeat

Build the room, Worker and client in separate commands (each under 15 minutes):

```sh
COAX_CHECK_JOBS=2 COAX_JOBS=2 coax/web/room/build.sh
COAX_CHECK_JOBS=2 COAX_JOBS=2 coax/web/room/worker.sh
apps/aethyrforge-desktop/build.sh
coax/web/tool.sh apps/aethyrforge-desktop/offline-check.coax \
  "$PWD/apps/aethyrforge-desktop/build/Aethyrforge.app" /tmp/forge-offline
coax/web/tool.sh everglade-drift --offline native --profiles local --no-raid \
  --port 8980 --out /tmp/forge-offline-drift
```

The native desktop checks open a visible, muted window, then quit it. They
require connected seat zero, the expected host, actual Worker counts, more
than 100 rendered callbacks and received messages, and nonzero tick timings.
The drift check uses headless, muted Chrome. Its marks now use the same
future input step on both pages, as its key presses already did. The earlier
wall-clock marks produced a false movement drift: both players stood at
exactly (-15,700, -56,700) mm, but their separately sampled origins differed
by 381 mm. No probe tolerance was increased.

The corrected native-local versus networked-room run passes all 25 probes
(`drift-native.log`), including movement, spells, conditions, town state,
camera continuity, corrections and NPC animation speed.
The browser Worker regression also passes all 25 probes (`drift-worker.log`).

The admission policy kills all 8 mutants. Integration checks reject native
local enablement on a wildcard bind, refuse a second local seat, refuse
ticketless ordinary room joins, and still admit a valid signed ordinary-room
ticket. The native executable also rejects `--native-local --listen 0.0.0.0`.
The broad pre-existing relay mutation run was stopped before 15 minutes;
it is not claimed as a passing result. The focused policy and join evidence
is committed. The tools scan reports the same 25 unrelated existing files
without Firm reason headers recorded in #12865.
