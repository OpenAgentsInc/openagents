# Room overload: world damage and background requests

Issue #12841, tested part. Source: the preceding commits named `Reserve world
damage snapshots per seat` and `Isolate bounded background room requests`.
This does not close the remaining downstream-strike, CPU/admission, content
warming or integrated saturation gates. The work-class contract is
[`room-overload.md`](../verse/room-overload.md).

The world's full-tick damage aggregate has one reserved mailbox per seat.
Repeated delivery cannot consume other seats' space or bypass the due-heal
phase. The integrated regression first failed on the old immediate-overflow
path, then passes with 65 repeated deliveries and take-once resolution.
The mailbox authority kills all 22 root mutants, without equivalents.

Native background HTTP now has six independent lanes, one serial helper and
one Coax-owned identity per lane. Local request/response datagrams are bounded,
atomic and nonblocking. HTTP waits run outside the simulation; refused durable
offers return failure to their existing outbox. Identities, 15-second expiry,
retry spacing and publication generations are checked in Coax. An old world
response cannot apply after a newer publication request. The authority kills
139/139 root mutants, without equivalents.

## Actual socket/process receipts

The new Coax consumer starts its own loopback HTTP fixture and native server,
joins two rooms, walks in both, and sends area/pet cast requests in one while
all HTTP responses are stalled. It consumes actual gameplay wire snapshots.
The current room policy and native SDK are linked with the retained native
world artifacts from the #12838 qualification. This is an HTTP-transport
component receipt; it is not a current-world raid/fleet capacity measurement.

| Observation | Result |
| --- | --- |
| API responses stalled | 35 seconds |
| Active room frames | 1,051 |
| Other room frames | 1,051 |
| Other room's longest state gap | 48 ms (250 ms gate) |
| Health probes | 70 answered, zero failures |
| Most admitted background requests observed | 4 of 6 lanes |
| Busy cast refusals | 129 |
| Closed gameplay sockets | Zero |
| API resumes | Pending HTTP requests clear; both gameplay sockets remain open |

Cast requests are exercised here, not a proof that every pet/destruction
workload has been qualified. Those observability and saturation cases remain
part of #12841 and #12843.

The changed SDK also passes the actual scratch API/native character fault
suite, 35/35: kill before/after commit, lost responses, unavailable store,
obsolete process and process-group final-save drain. During the 38-second
outage it pauses inside the same 901-tick gameplay bound and restores the
accepted body. A successful final save exits 0; an unavailable final save
exits 1. The native startup-default check passes with zero fields wrong.
The complete room suite passes 589/589. `tools-check` passes with no new tool
missing a reason. Raw receipts are in
[`room-overload-background-2026-11-05/`](room-overload-background-2026-11-05/).
The later completed release worker build for that unchanged background part
is 1,022,517 / 1,060,000 bytes. The ability-reservation follow-up records its
newer worker separately.

The first current-world build generated its Coax units but failed at the SDK
link compile because Darwin's POSIX namespace lacks `MSG_DONTWAIT`. Both ends
now use `O_NONBLOCK` and ordinary socket flags, including the helper's response
send. The corrected SDK compile is warning-free. The build wrapper removed
its generated units on that failure; the retained-world component link above
avoids calling that failed build a passing current-world qualification. The
full current-world build remains required for the later gameplay gates.

## Rerun

From an isolated worktree, build the native room with
`nice -n 15 env COAX_JOBS=1 COAX_CHECK_JOBS=1 coax/web/room/build.sh /tmp/overload-room`.
Then run each bounded command separately:

```sh
nice -n 15 env COAX_JOBS=1 COAX_CHECK_JOBS=1 coax/web/tool.sh coax/web/room/background-check.coax /tmp/overload-room 35
nice -n 15 env COAX_JOBS=1 COAX_CHECK_JOBS=1 API_TESTS_ONLY=checkpoint2_checks.coax ROOM_BIN=/tmp/overload-room coax/web/api/test.sh
nice -n 15 env COAX_JOBS=1 COAX_CHECK_JOBS=1 coax/bin/coax test coax/web/room/room.coax
nice -n 15 env COAX_JOBS=1 coax/bin/coax mutate coax/web/room/hurt-budget.coax --root-only --jobs 1
nice -n 15 env COAX_JOBS=1 coax/bin/coax mutate coax/web/room/background-budget.coax --root-only --jobs 1
nice -n 15 env COAX_JOBS=1 coax/web/tool.sh tools-check
```

Development reused the already-passing unchanged API prelude and sourced the
normal scratch-Postgres section of `test.sh`, as recorded for #12838. Every
service/process/port belongs to the harness. No production call or deploy ran.
