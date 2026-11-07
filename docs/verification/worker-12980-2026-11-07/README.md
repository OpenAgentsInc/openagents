# Supported Linux Everglade worker — #12980

Source base: `fed5b35f95`. The ordinary supported Linux builder produces the
canonical `apps/openagents-web/build/everglade-room.wasm` at **1,052,572 bytes**,
SHA256 `b11eeba4157d922a57536942627477b9fb693e578b486e240b7492a5e34d6a57`.
The additional check against this issue's original **1,060,000-byte** limit
exits 0. No byte budget changes in this follow-up. The existing product limit
is 1,122,000 after the owner's Ocarina addition in `dde4d40aa4`.

## Build change

The admitted Linux Coax 0.52 compiler, LLVM 21.1.8 and Binaryen 129 remain
unchanged. River keeps C `-O3`, with `-fno-unroll-loops` on Linux release only.
Other C profiles, LTO, final Binaryen `-O3`, fixed room math and strict floating
point flags remain unchanged. Mac release and development builds receive no
new flag. The isolated profile authority passes its check and kills 4/4 root
mutants, zero survivors.

The current unmodified Linux build is 1,064,349 bytes and fails the historical
limit. It and the original 1,062,884-byte failing artifact are retained with
their exact identities. Binaryen convergence/size experiments remained over
the historical limit; reducing river to C `-O2` fit, but its replay attempts
failed. Those are rejected observations, not accepted qualifications.

## Replay fixture corrections

The normal-priority paired replay uses actual native room and Wasm Worker
hosts, two background players, headless muted hardware Chrome/Metal on Apple
M5 Max, and local/100/250 ms profiles. Delayed profiles retain their existing
30 ms jitter and 2% read loss. The 600 µs all-sample p95 gate and existing
movement, condition and timing tolerances are unchanged.

Intermediate failures identified three fixture problems:

1. Shift+1 used an 80 ms tap without the processed-step wait used by other
   spell taps. It now shares that wait. Command/refusal/status observations
   are retained for future failures.
2. The native room received the complete replay control answer, while the
   worker received only fixed dice; PvP consequently differed. Both now
   receive the same full answer before rehearsal. Raid setup keeps its
   separate existing controls.
3. The old gravity position grazed loose stable props and stood beneath the
   gate lintel. A read-only diagnostic against the exact generated wasm32 ABI
   identified piece 4511 / mesh 31 (barrel) at (12.40, -64.80), and piece 4556 /
   mesh 32 (wooden crate) at (7.82, -69.00). Both had `DP_LOOSE`, with zero
   authoritative break events. These are the stable counter's loose props,
   not structural walls. The temporary raw-memory observer is removed.
   Ordinary W/D input now continues onto the open road. Its new precondition
   bounds x to [0.5, 2.5] m and z to [-59.5, -58] m, beyond the lintel and well
   inside the field for both props. The four-corner check retains the original
   conservative 14.740 m radius and the complete NPC-patrol check. The isolated
   placement authority kills 11/11 mutants, zero survivors.

No simulation, probe meaning, server hold reach, comparison tolerance or
performance limit changes. Earlier failed replays remain under `failed/`;
the unchanged Mac worker's passing control is under `mac-control/`.

## Passing qualification

The final `passing/` receipt reports `passed: true` and
`source_fence_unchanged: true`, including both builder/profile sources and the
new placement authority. Every gameplay verdict passes on all three profiles:
actual movement/jump, gravity affecting and dropping an NPC, all three wall/
gravity effects, the same scripted Hold Person target, per-person conditions,
stables, camera, corrections and NPC animation. Transmitted movement inputs are
identical by session step. There are zero worker stalls and zero dropped beat
milliseconds. No raids were requested by this bounded qualification command;
the previous raid evidence is retained in the #12980/#12981 historical record.

| Profile | Measured ticks | p50 µs | p95 µs | p99 µs |
| --- | ---: | ---: | ---: | ---: |
| local | 1,955 | 321 | 525 | 640 |
| 100 ms + jitter/loss | 1,951 | 330 | 505 | 635 |
| 250 ms + jitter/loss | 1,961 | 336 | 566 | 656 |

These are actual target measurements; no arbitrary host-contention guarantee
is inferred. Raw all-tick samples, command/refusal observations, input streams,
vitals and the unchanged-input receipt accompany the replay log. The profile,
placement and four-corner geometry checks pass; the two new authorities have
15 mutants killed in total and zero survivors. `tools-check` passes with zero
new tool files missing a firm reason.

## Reproduction and retained inputs

`build-identities.json` records compiler/toolchain identities and flags;
`linux-identities.txt` pins the builder, profile, simulation roots and original,
current oversized and final artifacts. The build logs and silent successful
historical-budget log are retained beside the actual byte counts. Generated
C/header banks, source export and binaries live under
`~/.openagents/aethyrforge/worker-12980/2026-11-07/` on `coderos-4080`.
Raw browser captures and executable inputs are retained under the same path
on the Mac. No source asset or executable binary is committed.

```sh
# Linux, ordinary supported builder; no qualification filename bypass
COAX_ROOM_KEEP_C=../candidate-generated-c coax/web/tool.sh coax/web/room/worker.coax
coax/web/tool.sh wasm-size budget ../original-budget.txt everglade-room apps/openagents-web/build/everglade-room.wasm

# Mac, build the qualifier first, then run its prebuilt image at nice 0
coax/web/build/tools/everglade-drift-23dfc150fa0d21dd199b2da382b9c2dd0762e8183e3f576657fe1341d327117e --profiles local,100,250 --bots 2 --port 9350 --no-raid --tick-us 600 --out /tmp/issue12980-open-road
```

The historical budget fixture is exactly `everglade-room 1060000\n`.
The runtime target here is the Linux-produced portable Wasm artifact on the
Mac browser, not a Linux browser performance claim. The unchanged native room
and page artifacts reuse the build identities recorded for #12981.
