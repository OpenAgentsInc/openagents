# Current Mac Everglade worker qualification — #12981

The current admitted worker passes the unchanged 600 µs all-sample p95 gate,
with the complete paired gameplay replay passing at local/100/250 ms. No
simulation, worker optimization profile, timing limit, movement tolerance or
NPC-condition window changes in this follow-up. The implementation changes
repair the Coax test fixture and its position observer.

Source base: `70966abbb1`. Apple M5 Max, headless muted hardware Chrome/Metal,
normal process priority (nice 0), two background players, no raids as requested
by the issue's command. Builds finish before timing; unrelated processes are
not stopped. The local and delayed runs have identical input hashes, and each
receipt reports `source_fence_unchanged: true`, `passed: true`.

| Profile | Ticks | p50 µs | p95 µs | p99 µs |
| --- | ---: | ---: | ---: | ---: |
| local | 1,922 | 300 | 471 | 576 |
| 100 ms + jitter/loss | 1,922 | 310 | 440 | 566 |
| 250 ms + jitter/loss | 1,923 | 311 | 420 | 550 |

All gameplay verdicts pass, including actual movement/jump, gravity affecting
and dropping an NPC, the same uniquely observed Hold Person target, per-person
condition bits, camera, corrections and NPC animation. There are zero worker
stalls and zero dropped beat milliseconds. These are measured qualifications
on this target, not a promise under arbitrary host contention or Linux timing.

## Causes and corrections

The original 250 ms mismatch already has a documented cause/fix at
`docs/verification/worker-qualification-2026-11-06/`: independent nearest-person
holds selected person 38 offline and 40 online, both with condition 256. The
existing named-target fixture fixes that without widening timing tolerances.
Its unchanged 24/41/33-mutant evidence is reused.

The fresh baseline here meets the timing limit (546/540/390 µs) and matches the
named target, but correctly fails two extra fixture preconditions:

1. The current initial camera heading sends the old walk away from the patrols.
   The fixture now faces north through ordinary CDP pointer input before its
   start mark. It strafes west, clears the solid gate-post front, crosses back
   to the open road and enters the arch. It never teleports a body or writes
   camera state. The existing 14.740 m conservative gravity placement check
   still contains both complete patrols, and actual NPC effects remain required.
   Intermediate post-only approaches are retained as failed observations.
2. A multipart full snapshot clears its mirror at part zero. Publishing that
   partial mirror made a known page disappear briefly and falsely tripped the
   32 m clearance guard. The bot observer now publishes only a complete,
   acknowledged final snapshot. A final snapshot with a missing preceding part
   still refuses clearance; the actual 32 m distance test is unchanged.

The qualifier now fences the bot, proxy and snapshot-policy sources as well as
its previous inputs. `local/` and `network/` hold the passing receipts and raw
samples; `baseline/`, `camera-only/` and `west-post/` retain failures. Original
issue observations remain in their existing historical records.

## Checks and identities

The multipart regression failed on the old observer, then passes, including
an actually unsafe completed position and a skipped packet part. The isolated
`drift-snapshot.coax` authority has 3 passing checks and kills all 10 mutants,
zero survivors. Heading checks cover the normalized input range and geometry
checks retain the rejected post positions. The physical DOM input check passes.
Unchanged named-target mutation evidence is reused from the prior record.

`build-identities.json` pins compiler, builder, simulation sources and native
SDK sources. Both live receipts pin actual native room, page, worker, bridges,
observer and executable hashes. Worker: 1,068,379 bytes, SHA256
`113124abdca5f0b78ff7bd91f357cffa39112331175dfa2b9e8978b9acf81286`.
The pre-existing product budget is 1,122,000 bytes after the Ocarina movement
addition in `dde4d40aa4`; this follow-up changes no byte budget. The historical
Linux size issue #12980 is separate and is not closed by these Mac results.

Large executables, replay facts and captures are retained under
`~/.openagents/aethyrforge/worker-12981/2026-11-07/`. No production deployment or
owner action is needed.

```sh
coax/bin/coax test coax/web/drift-snapshot.coax --library
coax/bin/coax mutate coax/web/drift-snapshot.coax --library --root-only
coax/bin/coax test coax/web/room/drift-bots.coax --only 'background clearance'
coax/bin/coax test coax/web/everglade-drift.coax --only 'explicitly faces'
coax/bin/coax test coax/web/everglade-drift.coax --only 'gravity placement'
```

Build the qualifier through `tool.sh` with an intentionally invalid limit, then
run its prebuilt image directly at nice 0. The passing current command is:

```sh
coax/web/build/tools/everglade-drift-23dfc150fa0d21dd199b2da382b9c2dd0762e8183e3f576657fe1341d327117e --profiles local --bots 2 --port 9350 --no-raid --tick-us 600 --out /tmp/issue12981-local-arch
coax/web/build/tools/everglade-drift-23dfc150fa0d21dd199b2da382b9c2dd0762e8183e3f576657fe1341d327117e --profiles 100,250 --bots 2 --port 9350 --no-raid --tick-us 600 --out /tmp/issue12981-network-final
```
