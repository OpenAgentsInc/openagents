# Consolidated draw-bundle native profile

The native binary includes `ddc038cfd5`. This 100-second run at 3456 × 2104 with 4× anti-aliasing completed 23 fireballs, cultist respawn, and a successful reload, with zero dropped simulation time. The retained native screenshots were visually inspected: the scene, actor corpses, nameplates, lighting, and HUD remain present after reload.

The frame budget still fails: work p95 18.715 ms, delivered p95 19.104 ms. This is essentially unchanged from the earlier optimized-shader run (18.642 ms work p95); the measurement does not demonstrate a speedup from consolidating bundle calls. No other Verse process appeared in the process snapshot during loading, but this was not an exclusive machine benchmark.

`timings.json` retains separate CPU preparation, command encoding, and queue submission statistics. These are not GPU timestamps. Performance acceptance remains incomplete. The complete profile is losslessly compressed in `window/frames.ndjson.gz`.
