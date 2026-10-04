# Authored material evidence

The three 1920×1080 captures use the native GPU renderer with four-sample anti-aliasing. `materials.json` records the exact shader digest and retained material-map coverage. The scene declares normal, metallic/roughness, and emissive maps; it declares no occlusion maps.

`window/window-reload.json` records a 100-second 3456×2104 native run with 23 fireballs, live reload, unchanged authority at commit, and zero dropped simulation time. `window/budget.json` fails the required frame budget at 26.1 ms work p95. A concurrent release build overlapped the later part of the run; the earlier part also exceeded the budget, so this evidence does not establish that contention caused the failure.

The raw profile is losslessly retained as `window/frames.ndjson.gz`; decompress it before using `verse_play --check-profile`. These are CPU frame-work and window-delivery measurements, not GPU timestamps. Performance acceptance remains required for #10483.
