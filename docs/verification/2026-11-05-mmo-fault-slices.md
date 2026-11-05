# MMO fault slices, first part of #12842

`coax/web/api/tests/room_checks.coax` exposes three individually selected
`mmo-fault-v1` slices. They reuse the actual room/API fault helpers rather
than a second simulation. Each selection requires a native room binary,
its source label and a room signing key; missing or malformed configuration
fails instead of silently skipping the native checks. The ordinary API
suite's selections and wall-clock raid seed remain unchanged.

The profile logs its slice, source, seed, injection boundary and elapsed
time. The default seed is 1284201; an explicit seed must be 1..2147483647.
This fixes the save raid's seed. Owner/travel use their existing state
barriers. These are real native event loops and wall clocks, not a claim
that the entire scheduler or failure timing is bitwise deterministic.

## Local results

Room source/artifact: `fae22e7101`, built for the interest-vitals check.
API and fault helper base: `fae22e7101` plus this harness patch. All runs
use nice 15, one Coax build/check worker, temporary Postgres databases,
loopback APIs and native socket consumers. The three slices run separately.
They neither deploy nor contact a production database.

| Slice | Injection and assertions | Result |
| --- | --- | --- |
| `save` | Four fighting druids; HTTP 500 for 30 s starting at checker +16 s, followed by two committed replies held for 7 s, exceeding the room's 5 s timeout. The seats earn XP, database XP exactly matches the heard rewards, lost replies cause duplicate retries without duplicate application, and SIGTERM drains with zero unreceipted players. | 5/5 pass; final bounded run 159,753 ms |
| `owner` | SIGSTOP g1 after its first save; advance the directory to g2 and save health 3; SIGCONT g1. The old owner loses its seat, g2's health stays, the old ticket is refused and a fresh ticket seats. | 6/6 pass; 44,783 ms |
| `travel` | SIGKILL source after its saved route; accept and save at the destination, then SIGKILL it and seat on its replacement. The next owners save the same HP and XP. | 5/5 pass; 40,461 ms |

The save checker now stops waiting after five minutes and kills only the
checker handle it started if it has not exited. Its existing proxy-start,
flush and shutdown deadlines are retained. The original save log said `raid+16s`;
the actual injection has always been **checker+16s**, ten seconds after
the harness requests the raid. The final log label corrects this.

The two new selector checks pass, including explicit empty/invalid variables.
Each selected scratch API run also completes the existing health, CLI,
anime and realm SQL/walkthrough tail without sanitizer diagnostics.
For iteration these runs resume `test.sh` at its scratch-Postgres stage;
the unchanged prelude had passed earlier in this work. This is not a full
unfiltered API-suite pass: the earlier unfiltered run was stopped in the
separate realm talk fixture, as recorded in the checkpoint-receiver record.

No room simulation source changes here. Reuse the unchanged **545/545**
room result and release-worker **1,004,062 / 1,060,000 bytes** from
[the interest-vitals record](2026-11-05-room-interest-vitals.md).
`tools-check` reports seven unrelated missing `Firm reason:` headers:
`coax/tests/nostr/relay_live_cutover.py`, `relay_rollback_rehearsal.py`, and
`verse/tools/humans/{face_rig,metahuman,outfits,pack_human,shape}.py`.
This part adds no non-Coax tool.

## Rerun

Build the desired native room with `coax/web/room/build.sh` and retain its
source identity. Run one slice per command from the repository root:

```
nice -n 15 env COAX_JOBS=1 COAX_CHECK_JOBS=1 API_TESTS_ONLY=room_checks.coax ROOM_FAULT_SLICE=save ROOM_FAULT_SEED=1284201 ROOM_FAULT_BUILD=SOURCE ROOM_BIN=/absolute/path/to/room coax/web/api/test.sh
nice -n 15 env COAX_JOBS=1 COAX_CHECK_JOBS=1 API_TESTS_ONLY=room_checks.coax ROOM_FAULT_SLICE=owner ROOM_FAULT_SEED=1284201 ROOM_FAULT_BUILD=SOURCE ROOM_BIN=/absolute/path/to/room coax/web/api/test.sh
nice -n 15 env COAX_JOBS=1 COAX_CHECK_JOBS=1 API_TESTS_ONLY=room_checks.coax ROOM_FAULT_SLICE=travel ROOM_FAULT_SEED=1284201 ROOM_FAULT_BUILD=SOURCE ROOM_BIN=/absolute/path/to/room coax/web/api/test.sh
```

The API wrapper creates a local test key and scratch store. Room/proxy ports
come from ephemeral loopback listeners, not fixed shared development ports.

## Remaining

#12842 remains open. This part exercises **legacy vitals**, not checkpoint
producer/restore from #12838. It does not prove checkpoint loss bounds,
kills immediately before/after checkpoint COMMIT, retry saturation, cast
loss/reordering, partial snapshots in the actual browser, cold/readiness
admission, rolling-deploy overlap or end-to-end recovery/presentation limits.
The room outcome-recovery work in #12902/#12903 and regional-store fault
work in #12857 retain their own evidence; this profile does not replace
them. Actual browser presentation/drift remains with #12778.
