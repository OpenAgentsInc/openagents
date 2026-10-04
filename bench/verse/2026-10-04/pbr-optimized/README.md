# Optimized PBR native profile

Captured with the shader optimization in commit `d3b8fe3963`, at 3456 × 2104 with 4× anti-aliasing on Apple M5 Max. The 100-second sequence completed 23 fireball casts, cultist respawn, and a live reload with zero dropped simulation time.

Work p95 is 18.642 ms and delivered p95 is 18.982 ms. The run fails the 16.667 ms work threshold. The preceding profile measured 26.066 ms work p95; concurrent compilation differed between runs, so this comparison does not isolate shader speedup. Another session compiled during loading and the early portion of this run.

`window/budget.json` retains the checker result, `window/frames.ndjson.gz` retains the complete profile, and the PNGs show the native scene before and after reload. No resolution, material channels, shadow filtering, or anti-aliasing settings were reduced. Performance acceptance remains incomplete.
