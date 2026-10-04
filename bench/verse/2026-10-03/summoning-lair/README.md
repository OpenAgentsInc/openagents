# Summoning lair evidence

The captures come from the engine’s GPU passes. The source receipt records the CC0 Fantasy Props MegaKit subset and the separately admitted local Bestiary model. `rejected-overbright.png` retains the rejected initial lighting treatment. The combat video records the full encounter at 1280×720; it predates the native-resolution renderer changes.

`performance/` and `performance-spatial/` retain rejected performance attempts: neither completed enough successful movement/fireball casts to pass acceptance. `performance-hd-cold/` retains the first Retina run, which failed the frame budget and dropped startup simulation time. `performance-hd-interrupted/` ended before sustained acceptance and has no passing result. `performance-hd-shadow-bundles/` completed 23 casts without dropping simulation time but exceeded the CPU frame-work budget. `performance-hd-world-bundles/` also exceeded the frame budget and recorded an extra queued redraw after the final capture; redraws now stop when the event loop exits. `performance-hd-zero-lights/` retained zero dropped simulation time but still failed the frame budget and did not prove presentation of the new catalog. These results remain failed attempts.

The final `performance-hd/` run uses the physical Retina viewport, four-sample scene anti-aliasing, a three-times-density font atlas, filtered texture mipmaps, and reusable world and shadow commands. Its frame measurements and budget report record performance; the before and after captures and window reload receipt verify GPU presentation across an asset reload.

The accepted 100-second run renders at 3456×2104 on Apple M5 Max, completes 23 fireballs, and drops no simulation time. Frame-work p95 is 14.676 ms; delivered-frame p95 is 14.974 ms, with a maximum of 26.461 ms. `resize-hd/` separately verifies a smaller viewport, restoration, and live presentation after reload.

Rejected attempts retain lossless gzip-compressed frame records. Decompress `frames.ndjson.gz` to inspect or rerun the profile checker; accepted performance and resize records remain plain NDJSON.
