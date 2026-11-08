# Everglade II's shared terrain: receipt (#13059)

Measured 2026-11-08 at about 03:15 Chicago time, on the owner's Mac (Apple Metal-3 through headless
Chrome), at 2400x1400 with DPR 2, with the cache off. The Mac was busy with other agents' builds (load
average 9 to 12). Each run is `coax/web/everglade2-terrain-check.coax` against a local static server.
"before" is the page and pack before #13059 (the glade's one tile, the path's ribbon, coloured ground
vertices); "after" is this change. The runs alternate before, after, before, after.

Columns in the TSVs: walk, load ms (to the scene), frames, rAF p50, p95, p99 and max (us), fps x10,
GPU pass p50 and p95 (us), Wasm memory before and after the walk (bytes), then the ground's facts:
route (1 the pack's record), tiles, publishes, refusals, first refusal, KiB resident, set and glade
revision words, reads past the tiles.

Run 1 caught a burst of load on the Mac and dropped frames on both builds. Run 2:

| walk  | load before | load after | fps before | fps after | GPU p50 before | GPU p50 after |
|-------|-------------|------------|------------|-----------|----------------|---------------|
| glade | 3377 ms     | 3859 ms    | 60.0       | 60.0      | 7226 us        | 7334 us       |
| path  | 3199 ms     | 3084 ms    | 60.0       | 60.1      | 12530 us       | 13014 us      |
| seam  | 3524 ms     | 3323 ms    | 51.1       | 60.1      | 15480 us       | 11808 us      |

After: route 1 (the pack's record), 21 tiles, one publish, no refusal, 1848 KiB resident, and Wasm
memory flat during every walk (124,846,080 bytes, 3.4 MB above before for the tile set and the
record's buffers).

The pack: `everglade2.pack` 46,921,596 bytes before, 47,791,796 after; the shipped `everglade2.vpk3`
26,024,836 bytes before, 26,246,017 after (+221,181). The ground vertices carry no colour now, but
the record (524,513 bytes, about 700 KB as base64 in the catalog) is larger than what that saves.

The golden views (`?golden`, `?golden&at=60`, 200, 330 and 470, 1600x900) differ from before by a
mean of 0.6 to 1.7 levels in 255, with under 0.6% of pixels more than 24 levels apart: the same frame.
