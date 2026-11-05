# Character recovery qualification — 2026-11-05

Issue #12838. Profile: [character checkpoint/resume 2](../protocol/character-checkpoint-v2.md).
Source is the preceding commits named `Define complete character resume
checkpoints` and `Persist and restore bounded character state`. These are
scratch-store and native-socket results, not a production
activation or browser-presentation measurement. The existing v1 receiver remains
additive and keeps its published meaning.

## Objectives and observed behavior

Every successful receipt follows a synchronous database commit. A lost response
does not repeat progression: the same owner/process/revision and complete body
return the original receipt. The saved XP chain cannot decrease; owner transfer
also takes the maximum legacy/v1/v2 XP floor. No unversioned report can write a
profile-2 character, from its first ticket onward. Account linking carries the
accepted body and advances both fences.

| Field | Restart from accepted capture | Before next acceptance |
| --- | --- | --- |
| HP/max/dead and temporary HP | Complete explicit body, including HP zero; actual-build bounds | At most 30 s authoritative gameplay plus one tick |
| Cumulative XP | Accepted cumulative floor; maximum reconciled across stores; no replay | Same bound on unaccepted gains |
| WANTED | Accepted authoritative value | Same bound |
| Room/place/yaw | Millimetre/radian capture; place restored only in the captured room; normal ground resolution | Same bound on movement |
| Class, slots and limited uses | Accepted class and remaining counts, constrained to build maxima | Same bound |
| Repair/cast cooldown and down timer | Accepted remaining ticks/elapsed down time; no offline credit | Same bound |
| World effects and active encounter | Conditions, concentration, summons, zones, in-flight casts, death-save rolls and contributions end with the lost world; ordinary fresh-town/raid rules | No world-state persistence promised by this character profile |

Same-process socket grace retains the live body. Restart uses the signed durable
capture once at hello, before placement. Initial play waits for acceptance.
During an outage the whole affected encounter pauses before its oldest accepted
capture reaches 30 s; movement queues and accrued time are cleared. Wall-clock
age can grow while paused. There is no finite restore-time objective during an
unavailable store. Normal capture cadence is 5 s.

The healthy restore objective is 60 s from replacement-process start through
startup, ticket issuance and first authoritative socket state, with matching
builds, at most 256 reservations, successful API responses within 100 ms and
the host being scheduled. It excludes fault detection and browser paint. A
successful planned drain has accepted its frozen final state; an unaccepted
final save returns failure instead of claiming success.

## Real process and database faults

The Coax harness starts native rooms, a scratch Postgres/API and a loopback fault
proxy. It consumes actual room sockets with the normal wire decoder. It issues
SIGKILL, process-group SIGTERM, STOP/CONT, HTTP failures and lost responses; these
are not simulated save receipts. The proxy can drop a reply after forwarding a
committed write. A separate SQL connection verifies committed bodies.

| Observation | Result |
| --- | --- |
| Kill before first acceptance | No gameplay frames; prior complete body survives |
| First healthy replacement | 112 ms total, 103 ms socket join |
| Kill after COMMIT before response | Committed changed place restores; 109 ms total, 100 ms socket join |
| Repeated lost real HTTP replies | Duplicate receipt preserves first acceptance and resumes gameplay |
| Save outage | 38,014 ms observed; 880 authoritative ticks, last frame at 29,303 ms; bound 901 ticks |
| Actual unaccepted movement | 34.878002 m rolled back to exactly the accepted place after kill |
| Outage replacement after store recovers | 108 ms total, 99 ms socket join |
| Obsolete stopped process resumes | Loses seat; cannot overwrite successor's checkpoint |
| Planned termination | Exit 0 only after final accepted position: x=4572 mm, z=-84000 mm |
| Final save unavailable | Exit 1; no successful-drain claim |
| Full save outbox | All 256 unsaved captures survive socket closure and grace expiry; character 257 refused despite a free simulation seat |

The capacity slice uses sixteen batches of sixteen sockets, each followed by
grace expiry. It checks the save reservation bound separately from connection
and simulation-seat limits. The failure suite has 35/35 checks; capacity has
17/17, including repeated API setup assertions. Raw final receipts are in
[`character-recovery-2026-11-05/`](character-recovery-2026-11-05/).

## Gates and artifacts

Apple M5 Max (Mac17,6), 128 GiB, macOS 26.4 (25E246), Coax 0.48.0. Native build
uses the normal strict-math host sources. New authorities have no surviving
mutants and no equivalent annotations:

| Authority | Root mutants killed |
| --- | ---: |
| Complete checkpoint codec | 187/187 |
| Bounded save/receipt flow | 51/51 |
| Signed resume envelope | 117/117 |
| Resume prefix/payload | 75/75 |
| API activation/profile policy | 12/12 |
| Total | 442/442 |

The full room suite passes 571/571, the page's network slice 29/29, and the
scratch legacy room/API regression 149/149. That regression also passes the
98 CLI caller checks and the remaining selected API tail under sanitizers.
The release room worker is 1,019,909 bytes against its unchanged 1,060,000-byte
budget. `tools-check` passes with zero new files missing a reason.

The unfiltered API run was attempted and failed the unrelated existing
`chat: an expired token says so and is marked expired` fixture: it observed a
queued row. This is not a full unfiltered API pass. Its preceding NIP-98 suites
and 33,286 differential cases passed. No passing unchanged gates were repeated
just for confirmation. Earlier development runs exposed an incorrectly parsed
proxy Content-Length, malformed harness input spacing, and a SIGTERM-killed
save helper; the final receipts above follow those fixes. The final outage
test asserts nonzero actual movement, avoiding a stationary rollback proof.

Artifact SHA-256:

```text
native room: 022ec060918c411e3c528e12d76c841489a37dcc34ee948ac8a04e2c3a31dae3
release worker: d61c4779675db2d0d5262de6a5d6b72a204f3563a562496618dcd694083a6f53
```

## Rerun

Use an isolated worktree and the published compiler. Every invocation below
is bounded separately; the fault and capacity slices each use a fresh scratch
database and their own ephemeral ports. No production credentials are needed.

```sh
nice -n 15 env COAX_JOBS=1 COAX_CHECK_JOBS=1 coax/web/room/build.sh /tmp/room-recovery
nice -n 15 env COAX_JOBS=1 COAX_CHECK_JOBS=1 coax/bin/coax test coax/web/room/room.coax
nice -n 15 env COAX_JOBS=1 COAX_CHECK_JOBS=1 coax/bin/coax test apps/openagents-web/everglade-net.coax --only 'net:'
nice -n 15 env COAX_JOBS=1 COAX_CHECK_JOBS=1 API_TESTS_ONLY=checkpoint2_checks.coax ROOM_BIN=/tmp/room-recovery coax/web/api/test.sh
nice -n 15 env COAX_JOBS=1 COAX_CHECK_JOBS=1 API_TESTS_ONLY=checkpoint2_checks.coax C2_FAULT_SLICE=capacity ROOM_BIN=/tmp/room-recovery coax/web/api/test.sh
nice -n 15 env COAX_JOBS=1 COAX_CHECK_JOBS=1 API_TESTS_ONLY=room_checks.coax coax/web/api/test.sh
nice -n 15 env COAX_JOBS=1 coax/bin/coax mutate aethyr/session/character-checkpoint.coax --root-only --jobs 1
nice -n 15 env COAX_JOBS=1 coax/bin/coax mutate aethyr/session/checkpoint-flow.coax --root-only --jobs 1
nice -n 15 env COAX_JOBS=1 coax/bin/coax mutate aethyr/session/resume-envelope.coax --root-only --jobs 1
nice -n 15 env COAX_JOBS=1 coax/bin/coax mutate aethyr/session/resume-payload.coax --root-only --jobs 1
nice -n 15 env COAX_JOBS=1 coax/bin/coax mutate coax/web/api/checkpoint-resume-policy.coax --root-only --jobs 1
nice -n 15 env COAX_JOBS=1 coax/web/tool.sh tools-check
```

For development iterations this run sourced the unchanged scratch-Postgres
section of `test.sh` after the already-passing compiler/NIP-98 prelude, avoiding
repeating those unchanged results. The commands above run the complete normal
entry point. Split an invocation further if its prelude would exceed 15 minutes.

Production activation is owner-only: migration 0135/API first with the flag off,
then all compatible room destinations and the new page, then explicitly enable
`OA_ROOM_CHECKPOINTS=1`. Checkpoint profile membership is sticky. Rollback must
retain a compatible reader and accepted captures. The workspace's
`NEEDS_OWNER.md` contains the concrete release request; no deploy ran here.
