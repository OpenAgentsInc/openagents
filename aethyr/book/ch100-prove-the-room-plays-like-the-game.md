# 100. Prove the room plays like the game

> **Status: partly implemented.** The drift check plays one scripted session
> offline and in a local room, side by side, and compares what the two pages
> draw frame by frame, on local, 100 ms and 250 ms networks with 30 ms of
> jitter and 2% loss, with bots in the room
> ([coax/web/everglade-drift.coax](../../coax/web/everglade-drift.coax),
> #12778). `coax/web/room/deploy.sh` runs it before every deploy, and a
> failure stops the deploy. A local room's fixed dice
> and bots walking clear of your abilities removed three causes of drift
> (commit f54bb0f001). The 2026-11-05 handoff records later complete
> passes and probe corrections (#12946); keep revision-specific receipts. Raid bots
> fight (`raid-check.coax`); the load test's bots walk and cast but don't
> fight (`load.coax`, #12711).

Every earlier chapter in this part has its own checks. This chapter adds
the checks that would have caught the failure the part was written after:
the same game, offline and in a room, drawn the same; and load tests whose
bots play the game.

## What this chapter teaches

- The difference between checking state and checking what is drawn.
- A drift test: one session, two hosts, per-frame probes, under shaped
  networks.
- Load tests that play: move, cast, fight, crowd.
- Reading a drift table.

## The counterexample

On 2146dc01d9 the server-authoritative room was closed as done. Its checks
measured latency, ticks and presence. The owner then played a local room and
found five problems in minutes (chapter 95). The root-cause report found:

1. **Two paths.** Offline and the room ran as two games. The page branched
   on room state about 65 times across 8 files, and each feature was rewired
   by hand. Only movement was wired completely.
2. **Two worlds.** The room loaded the published world (12 placed things);
   the local page got a 404. A crate stood only on the server (chapter 87).
3. **Lost defaults.** The C host zero-filled the server, so the level cap
   read 0 (chapter 86).
4. **Events as state.** Reverse Gravity was applied twice and turned itself
   off (chapter 94).
5. **Tests checked state, not pictures.** The two-browser smoke checked that
   Reverse Gravity's cylinder existed and that you rose, never what was
   drawn.

No check compared the two paths, so nothing failed. The fix for the class of
bug is a check that compares them.

## The drift test

Two headless Chromes play the same scripted session at the same moments: one
offline, one in a local room through a shaped network. The steps cover the
crate, strafing, walking, a jump, Reverse Gravity on and off, Wall of Stone,
Wind Wall, Hold Person on you, the town's people, and then a raid where both
pages' stables must fall and the room page must climb from level 1 to 2.

Each page records a row of 28 columns every frame (position, camera heading,
lift height, corrections, particles, each spell shown, the bar, conditions,
status text, townsfolk walking in place). A frame of one page fails when no
frame of the other within a window (250 ms plus the round trip) has the same
value within the probe's tolerance. A probe drifts when frames fail in a
row:

<!-- source: coax/web/everglade-drift.coax drift-bounds -->
```jai
DR_RUN :: 6;              // frames failing in a row that make a drift
DR_YAW_JUMP :: 300;       // mrad the camera may turn in one frame
DR_TURN :: 175;           // mrad (10 degrees) the camera may turn in DR_TURN_MS with no look (a pointer move): only
DR_TURN_MS :: 1000;       // your look turns it (aethyr/cam/camera.coax verse_orbit_follow), so this is rounding room
DR_FIX_RATE :: 1;         // corrections over 5 cm a second, at most
DR_STALL :: 15;           // percent more frames of the town's people walking in place than offline
```

Some probes are judged on each page alone: no frame may turn the camera more
than 300 mrad, the room page may make at most one correction over 5 cm a
second and none over 0.5 m, and the room's townsfolk may walk in place on at
most 15% more of their frames than offline.

## Run it

```shell
coax/web/room/build.sh
apps/openagents-web/build.sh
coax/web/tool.sh everglade-drift --no-raid
```

The output is one line per probe and profile, a table, and an exit status of
0 or 1. On main at `cbb9a1ce9b`, 8 of 16 probes drifted at every profile:
position, lift height, Reverse Gravity, Wall of Stone's panels, conditions
on you, HELD lines, corrections and townsfolk walking in place. Wall of
Stone itself drifted at 250 ms (#12718). Later commits
fixed the world source and the lasting abilities (`a941197cf1`).

## Load tests that play

Idle bots holding sockets measure nothing useful (research doc, section 15).

- `raid-check.coax`: bots walk to the square and cast at the nearest raider
  by its target code, as druids, warlocks or a mix.
- `drift-bots.coax`: bots walk rings around the drift page's player, and one
  casts Hold Person on cue.
- `load.coax`: up to 256 bots, each sending a walking input every step, a
  cast every 5 s and pings; it checks state at 28 Hz or more, every seat
  seeing every other, and the acknowledgement time.
- `net-proxy.coax`: the shaped network: latency, jitter and lost reads that
  arrive after a TCP-style resend delay of at least 200 ms.

## What is left

- Keep the drift check passing on every probe and network profile after
  each simulation or presentation change. The handoff records a full pass
  at `e00925f958`; that receipt does not qualify later changes.
- The 64-player load test, with bots that fight (#12711).
- Failure injection at the hard points: during a deploy (#12797), with old
  and new versions overlapping (#12788), and after a save commits but before
  it is acknowledged (#12793).

## Acceptance checks

- `coax/web/tool.sh everglade-drift` exits 0 at local, 100 ms and 250 ms, and
  a failure stops `coax/web/room/deploy.sh` (no switch skips it).
- A raid with 16 bots and 60 raiders holds tick p95 under its bound
  (chapter 99).

## Dependencies

Chapters 5 (replay), 90 (prediction), 93 (interpolation), 94 (effects) and
95 (offline as a local server).
