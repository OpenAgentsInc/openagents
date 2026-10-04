# Native render graph acceptance

Built from `78478008d8`. The native renderer executes the admitted chamber plan for static shadow refresh, cache copy, dynamic shadow draws, multisample world/resolve, overlays, and readback. Plan admission occurs before GPU buffer writes.

The 100-second Apple M5 Max run at 3456 × 2104 with 4× anti-aliasing passes the existing frame-budget checker: work p95 16.310 ms, delivered p95 16.617 ms, work max 27.093 ms, delivered max 27.412 ms, 23 fireballs, and zero dropped simulation time. It includes cultist respawn and live asset reload. The receipt confirms the unchanged world checkpoint and presentation of the replacement catalog.

The after-reload native image was visually inspected: lit props, shadows, actors/corpses, nameplates, and HUD remain present. No image grading, reduced resolution, or reduced AA was used. This proves the measured adapter/workload, not every platform or future scene. These timings measure CPU work and delivered intervals, not GPU timestamps. The full profile is losslessly compressed in `window/frames.ndjson.gz`.
