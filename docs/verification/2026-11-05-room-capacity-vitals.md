# Room capacity and reservation observations

This #12840 slice observes actual native capacity and completed API reservation
operations. It preserves placement, owner and durable-save policy. Client
presentation, clock uncertainty and the integrated latency split remain open.

## Boundaries

The native report separates world slots, live/unused worlds, seated connections,
grace holds and initial placement gates. Startup readiness is the exact `/ready`
condition, with draining reported separately. A draining process can remain
startup-ready while refusing placement.

The admin placement table reads configured directory caps and live reported
rooms/seats. It counts unexpired player and invite reservations separately,
excluding players already in a room's report. Readiness is credited only from
a report younger than 30 seconds bearing the directory's current process
generation. Missing, expired or superseded reports are unknown, not zero.
Directory age, report age, generation, drain and build remain visible together.

Reservation series classify the actual returned database operation: accepted,
redirected, full or failed; invites additionally distinguish absence from the
room. Retries count again. These are neither unique players nor successful HTTP
tickets. They do not claim durable gameplay acceptance. A separate transaction
writes the observation with a 50 ms PostgreSQL statement timeout and rolls back
on failure without changing the completed reservation. Lost observations are
logged. The server statement limit is not a database network deadline.

## Fresh source and artifacts

This build freshly emits the shared world unit, including the later Fire gate
layout. It supersedes the older frozen-world artifacts in the first vitals and
overhead records for current-source qualification. It uses the explicit 0.48.0
room-component compiler image, SHA-256
`cdaceb89e69c8d523e41242a940af34774e8b6b4f945998c2d3bf38486c357fb`.
It does not admit that image for graphics or desktop defaults.

- Native SHA-256:
  `11338370af66950d8b70b35e17bd9144572629d90ef5dc8758babe4f54bcf5e5`.
- Release worker: 1,034,562 / 1,060,000 bytes; SHA-256
  `16b8f8dc8d18fcfeb6f7236e5ca1d14cf2ae3350e4ff927466ac9a90d9466656`.
- Fresh shared-world dependency key:
  `ad4451901f5d89060fd3087ec4f8e623054c38d12841de3353854d38bc2098f8`.
- Native baseline rebuilt from 8c312f9649 with this same fresh world C/header/
  object: SHA-256
  `dd6c5cdc9a5719ddae5378bcb25f3d6ec126418626299eff2b30d9118ed55eb4`.

The retained source keys join the generated units to this room-component
profile. Other unchanged shared-unit objects are reused only after byte
comparison of their generated C and headers.

## Checks

- Full room suite: 644/644. Reservation observation authority: 58/58 mutants
  killed, zero survivors; no equivalents. Shared background harness: 234/234.
- Scratch API room checks: 161/161. These cover exact accepted/full/redirected
  request deltas, all invite outcomes, expired/duplicate reservations and
  current/superseded/expired readiness reports. A separate transaction locks
  the metrics row for two seconds: the ticket still succeeds within one
  second, its durable hold remains, and its failed observation changes no
  count. After the lock clears, the next ticket and observation succeed.
- A real 35-second service outage with sixteen warmed native rooms keeps every
  room streaming. During the run the report has 16 live worlds and connected
  seats, zero unused worlds/grace holds/initial gates, current generation and
  actual readiness. Health 70/70, independent-room maximum frame gap 38 ms.
  Startup world preparation is 285,278 us; worst room preparation 18,685 us.
- The fresh native checkpoint integration passes 37/37 checks, including the
  outage, accepted revision/capture metadata, stale-owner refusal and final
  drain success/failure. Successful transport alone never settles a save.
- The 75-second mixed workload has eight pets, four zones, 37 raiders and 1,040
  submitted casts, with 58 explicit busy refusals. Its independent healthy room
  has 2,251 frames and a maximum 51 ms gap. Health 150/150. The slow consumer
  closes/rejoins and catches up; maximum waiting output is 231,101 bytes.
  All exact-observer report gates pass, with no omitted lines.

## Paired fresh-world observer cost

The frozen harness SHA-256 is
`022c3bac835c91c2b0bf4d2001f6c0d64f77b42b8206615a9ab03c5c878ae5be`.
Both native variants share the same freshly emitted world C/header/object,
catalog and math. The baseline room unit and host come from 8c312f9649; the
observed variant includes queue/save and capacity observations. The controller
and its nine checks are unchanged from the first paired record.

| Run | Profile | Elapsed s | User CPU s | System CPU s | Total CPU s |
| --- | --- | ---: | ---: | ---: | ---: |
| A1 | baseline | 83.57 | 2.79 | 2.86 | 5.65 |
| B2 | queue/save/capacity | 83.39 | 2.80 | 2.79 | 5.59 |
| B3 | queue/save/capacity | 83.41 | 2.76 | 2.87 | 5.63 |
| A4 | baseline | 83.37 | 2.98 | 3.08 | 6.06 |

Baseline mean is 5.855 CPU seconds; observed mean is 5.610, a difference of
-0.245 CPU seconds (-4.18% relative). This loaded-host sample does not establish
a speedup, isolated-server cost or confidence/fleet bound. It measures the
complete harness and waited children, excludes compiler time, and does not
include API reservation-counter writes or future latency traces. Preserve the
absolute values and scope with the percentage. All four 75-second workloads
pass every report, gameplay, isolation, outage and slow-consumer recovery gate.

An earlier attempted pair stopped on its first observed run: three initial
joins were refused by the existing work budget, so ten sockets were not open.
That rejected run is retained and excluded from the comparison. Preflight now
allows at most six attempts per socket under a single ten-second deadline,
draining already-open sockets between attempts. Measurement still requires all
ten sockets and all original workload gates; no admission threshold changed.
The four final runs each needed zero retries. No automatic benchmark retry
exists after a workload gate fails.

Raw logs and source keys are in `room-capacity-vitals-2026-11-05/`. All API
checks use a scratch Postgres and private test processes/ports. Nothing is
deployed and no production data is used.

The native integrations are rerunnable through `background-check.coax warm`,
`background-check.coax stress`, and `checkpoint2-runtime.coax`; the API's
admission, invite and generation/freshness checks run through
`API_TESTS_ONLY=room_checks.coax coax/web/api/test.sh`. The retained binary
hashes and source keys define the artifact profile; use fresh builds for new
source rather than silently substituting an older cached world object.

Run the paired controller with the completed harness and the two completed
native binaries, as in `2026-11-05-room-observer-overhead.md`. Each round has a
120-second deadline; all four rounds take less than six minutes. Retained raw
timing and workload logs live in the `overhead/` subdirectory.
