# Measured room work and startup preparation

Issue #12841, tested part. The native room and browser worker run the same
Coax budget and shared tick. This does not close downstream town-effect
reservation or the remaining pets/destruction/event-exhaustion qualification.

Each room earns 45 microseconds per monotonic millisecond, capped at 3 ms,
and needs 1.5 ms available to start a tick. The SDK charges the complete
simulation/serialization wall duration. An overrun is that room's debt. While
deferred, its simulation and rule clocks do not advance; queued movement is
counted/discarded and the movement earning epoch resets. It gains no deferred
movement credit. Started casts and accepted lifecycle work remain held.
New casts and seats are refused before cast order or ownership fencing when
credit is insufficient or the preceding tick exceeds the burst. Waiting cast
intents expire after 533 monotonic milliseconds with explicit busy refusal.
The room's measurements are exposed separately from background HTTP capacity.
Policy, SDK and harness source is the change `Budget measured room work before
admitting new actions`, on `882146be65`; subsequent concurrent audio changes
do not touch its room/page consumers.

This is amortized isolation, not preemption of an individual call or the OS.
Sixteen rooms have 720 ms/s aggregate refill, up to 3 ms stored credit each,
and each can overrun by its current uninterruptible call. Transport and
publication application retain separate measurements/bounds. The policy is
not an assertion that every workload sustains 30 Hz.

The first actual socket run exposed a startup problem: cold preparation
could spend the first room's credit and refuse a second player joining
immediately. Native startup now prepares all sixteen world slots before
Everglade readiness/admission. An unavailable initial publication returns
503, while already admitted gameplay retains its existing world during an
API outage. Other games retain their startup admission contract. The worker
continues preparing only its local world.

## Receipts

The policy kills 82/82 mutants, zero survivors and no equivalents. The
complete room suite passes 605/605. Integrated checks cover another room's
independent credit, no movement earned during a deferral, early cast/seat
refusal, waiting-intent expiry, clock reversal and debt/cost boundaries.
Startup defaults pass. The final release worker is 1,025,190 / 1,060,000 bytes.
An additional regression fails on the original startup wrapper: sharded relay
URLs bypassed it. The wrapper now uses the room's shrinking path normalizer.
The final native startup consumer stalls its initial API publication, observes
health without readiness, and verifies that unsharded, sharded and nested
sharded sockets all refuse admission. After releasing the publication it sees
readiness, connects on the sharded route and receives a state frame. The
complete background-harness suite passes 231/231.

The native consumer starts its own HTTP fixture and room server, maintains
one consumer in each of sixteen rooms, introduces rooms one per second, and
changes the publication while they run. A separate child owns the sixteenth
socket so the parent's fixed sixteen SDK handles retain a health-probe slot.
The first harness attempt exhausted those handles; its failed receipt is
retained, not presented as a passing health result.

| Final native startup/publication run, 35 seconds | Observation |
| --- | --- |
| Active rooms/consumers | 16, all streaming |
| First and healthy room frames | 1,050 each (30 Hz) |
| Healthy room longest gap | 38 ms, below the 250 ms gate |
| Health probes | 70 answered, zero failed |
| Full startup preparation | 315,700 us |
| Largest individual preparation | 21,079 us |
| Last publication over active worlds | 200 us |
| Deferred ticks / new-work refusals / retired rooms | 0 / 0 / 0 |
| Pending HTTP requests after recovery | 0 of 6 lanes |

The fixture publication is the committed 416-byte development world, not the
maximum authoring document. This is a startup/stream-isolation receipt, not
the final raid/fleet capacity envelope. The shared Mac was loaded by other
work; commands and child processes run niced. One observed native resident
snapshot after prewarming is 175,472 KiB, excluding its six small helpers;
it is not a measured peak or a deployment memory guarantee.
Hardware: Apple M5 Max, 18 CPU cores, 128 GiB RAM, macOS.

The final native proxy cast regression answers 30/30 casts under 250 ms RTT,
30 ms jitter and 2% loss: 24 taken, six expected refusals, slowest 1,165 ms,
zero past 2,000 ms. All six target-gone interrupts occur; no other interrupt
does. The local, no-raid headless browser drift consumer holds against the
new worker/native artifacts. Its record includes player movement, ability
fields, held NPCs, lifecycle/plate/status probes and presentation corrections;
it does not substitute for the raid or 100/250 ms presentation slices.

The real scratch API/native checkpoint fault slice passes 35/35 with the
changed SDK: the 38,015 ms outage permits 883 active ticks against the
901-tick loss bound, rolls back 34.659 m, and restores in 415 ms total in that
case. Planned final-save termination succeeds only after acceptance;
unavailable final save exits with failure. The unchanged API prelude is
reused from the #12838 record; this is not an unfiltered API-suite pass.

Raw receipts and the generated-unit source keys are in
[`room-work-2026-11-05/`](room-work-2026-11-05/). Browser rows, key traces and
screenshots for this invocation remain at `/tmp/overload-12841/drift-work/`.

The final native artifact, including the sharded-startup regression, is
`5eadf336518c506617769b3e8b9bb2fb5aeb477b6945ab980ef76af37db7123d`;
its worker is `b477cb391d4ab81c48da8111e7fd4ddfc603a488b9b5ad722aa7d56103cd71e6`.
The unchanged warm/cast/checkpoint/drift paths above were qualified immediately
before that wrapper correction with native artifact
`da70c06dd23b60c013b0f6f3488374eabd1794f078547a2316d7c051c80654b2`
and worker `6af6dcd6293933cdeb7e7a462b2ad03f9900850d185c207c5f48fde6e069dbb2`
(1,025,168 bytes); they were not repeated to confirm unchanged results.
Compiler: `cdaceb89e69c8d523e41242a940af34774e8b6b4f945998c2d3bf38486c357fb`
(0.48.0). The worker tool retains a compiler/source-key manifest plus the
exact six generated C/header units with `COAX_ROOM_KEEP_C`; native compilation
uses the normal build's strict float flags and SDKs. Unchanged native objects
are reused only after their generated C and header compare byte-for-byte.
No production request, deployment or owner-visible browser runs.

## Rerun

Run each bounded command separately in an isolated worktree:

```sh
nice -n 15 env COAX_JOBS=1 COAX_CHECK_JOBS=1 coax/bin/coax test coax/web/room/room.coax
nice -n 15 env COAX_JOBS=1 coax/bin/coax mutate coax/web/room/work-budget.coax --jobs 1
nice -n 15 env COAX_JOBS=1 COAX_CHECK_JOBS=1 coax/web/room/build.sh /tmp/room-work
nice -n 15 env COAX_JOBS=1 COAX_CHECK_JOBS=1 coax/web/tool.sh coax/web/room/background-check.coax startup /tmp/room-work
nice -n 15 env COAX_JOBS=1 COAX_CHECK_JOBS=1 coax/web/tool.sh coax/web/room/background-check.coax warm /tmp/room-work 35
nice -n 15 env COAX_JOBS=1 COAX_CHECK_JOBS=1 coax/web/tool.sh coax/web/room/room-proof.coax cast /tmp/room-work 49771 250 30 20 6
nice -n 15 env COAX_JOBS=1 COAX_CHECK_JOBS=1 API_TESTS_ONLY=checkpoint2_checks.coax ROOM_BIN=/tmp/room-work coax/web/api/test.sh
nice -n 15 env COAX_JOBS=1 COAX_CHECK_JOBS=1 coax/web/tool.sh everglade-drift --profiles local --bots 3 --port 49811 --no-raid --offline worker --out /tmp/room-work-drift
```

For retained C without repeating Coax generation before a native SDK link:

```sh
COAX_ROOM_KEEP_C=/tmp/room-native-c COAX_ROOM_C_ONLY=1 nice -n 15 coax/web/tool.sh coax/web/room/worker.coax
```

The cast proof now prints a failing consumer's stderr before releasing its
process, so a failed handshake retains the actual diagnostic.
