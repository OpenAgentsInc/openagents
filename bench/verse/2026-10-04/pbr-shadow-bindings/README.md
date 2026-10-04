# PBR shadow binding acceptance

Native binary from `a414a49eb3`, Apple M5 Max, 3456 × 2104, 4× anti-aliasing. The 100-second scripted movement sequence completed 23 fireballs, cultist respawn, and live renderer reload. The reload receipt confirms the world checkpoint stayed unchanged and the replacement catalog was presented.

The frame-budget checker passes: work p95 16.298 ms, delivered p95 16.596 ms, work max 26.700 ms, delivered max 26.991 ms, and zero dropped simulation time. The process snapshot during the run showed no other Verse, Cargo, or rustc process. This is measured acceptance on this adapter and workload, not a guarantee for every machine or future scene.

The native after-reload image was visually inspected: PBR props, character geometry, shadows, actor corpses, nameplates, and HUD remain present. Shadow materials bind only the base texture, sampler, and alpha uniform; full PBR channels remain bound in world draws. No resolution, AA, shadow filtering, or material quality was reduced. The earlier failed profiles remain retained.

The complete profile is losslessly compressed in `window/frames.ndjson.gz`; `timings.json` contains separate CPU phase statistics, not GPU timestamps. `window/budget.json` retains the passing checker result.
