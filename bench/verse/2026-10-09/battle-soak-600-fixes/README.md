# Ten-minute battle soak after the October 9 fixes (#10559)

The same two `battle_scale combined … 600` runs as
[`battle-soak-600`](../battle-soak-600/README.md), on `coderos-4080`, the
licensed battle pack, 2560 × 1440, and the delayed pipeline route. The
executable is built from `0db6a09302` (main), SHA-256
`4008492dcf0ec61fe0684966869353f8be9563ef95a4141c606978f567595a87`. Both
ran under the quiet lease; the load average at the start was 7.40 (two
artifact-queue builds had just finished). Run 2 adds `VERSE_GPU_TIMING=1`.

What changed since the earlier soak:

- `1d3f1724db`: corpse and prop blocker boxes take their own collider shape,
  so a support is never carried by another shape's pose frame.
- `2e0d8d24c7`: staged reward-history nodes publish in three passes; the
  soak rows record the allocator's in-use bytes.
- `768d9b761a`: movement confirmations carry loose props' committed poses
  (wire 34); the harness trims the shared allocator after each segment.

| Gate | Earlier run 1 | Earlier run 2 | Run 1 | Run 2 |
| --- | --- | --- | --- | --- |
| Movement expiries | 0 | 1 | 1 | 2 |
| Players over the late p95 (0.25 m) | 0 | 1 | 2 | 2 |
| Players over the 1 m maximum | 4 | 10 | 6 | 3 |
| Steady RSS growth (64 MiB bound) | 76 MiB | 70 MiB | **44 MiB** | **35 MiB** |
| Steady CPU / GPU frame p95 | pass | 7.97 / 7.28 ms | 8.00 ms / not measured | 8.18 / 7.30 ms |
| Snapshot age p95 | 223 ms | 206 ms | 210 ms | 221 ms |
| Commit maximum | 0.64 s | 0.70 s | 0.69 s | 0.58 s |
| Reward-history sync maximum | 0.61 s | 0.69 s | 0.66 s | 0.56 s |

- **Memory passes.** RSS no longer steps at the midpoint reconnection; the
  allocator's in-use bytes stay within about 32 MiB of their first sample.
- **No spell-collider step-ups.** No retained correction trace in either
  run has a baseline or estimate standing on a loose prop.
- **The authority still stalls once a run.** Every correction over 1.3 m
  has 15 to 21 pending inputs, between 240 and 273 s. The expiries are at
  authority ticks 10,092 (run 1) and 5,330 and 7,626 (run 2), at 30 ticks a
  second; only the last falls in that window. The commit maximum still equals the reward-history
  sync maximum, so writing the nodes in three passes did not remove it; the
  time is not in per-node syncs. The next step is to record what that one
  sync writes (node count, bytes, and when) before changing it.
- A remaining single correction shows another open case: an
  applied confirmation standing on a corpse box (`shape 1`) that the
  client's last scene snapshot does not have yet (run 2, player 7, 1.22 m
  at 40.6 s).
