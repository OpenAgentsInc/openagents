# Command encoding profile

Native 100-second movement, 23-fireball, respawn, and reload run from `b2479a2897`, at 3456 × 2104 and 4× anti-aliasing. Another Verse process and compilation overlapped this run. This is diagnostic evidence, not isolated performance acceptance.

The budget check failed: work p95 29.928 ms, delivered p95 30.645 ms, and 0.011885 seconds of dropped simulation time. `timings.json` separates preparation, command encoding, and queue submission. Command encoding dominates this run; these CPU timings are not GPU timestamps. The renderer currently executes many individual draw bundles, which is the next path to investigate.

The complete compressed profile, native before/after screenshots, reload receipt, and checker result are retained under `window/`.
