# Ten-minute battle soak with concurrent reward-history syncs (#10559)

The same `battle_scale combined … 600` setup as
[`battle-soak-600`](../battle-soak-600/README.md) and
[`battle-soak-600-fixes`](../battle-soak-600-fixes/README.md): `coderos-4080`,
the licensed battle pack, 2560 × 1440, the delayed pipeline route, the
quiet lease. Run 2 adds `VERSE_GPU_TIMING=1`.

- `before-sync-profile-report.json.gz`: one GPU-timed run from `33238f064b`,
  the first with the slowest reward-history publications in the report. Its
  stall is one publication of 130 nodes (82 KB): 652 ms syncing the nodes
  one after another, 1.4 ms writing, 5 ms syncing the directory. It has one
  movement expiry and the stall's 1.0 m correction with 18 pending inputs.
- `run1-report.json.gz` and `run2-gpu-report.json.gz`: two runs from
  `3686a7d81b`, which syncs a publication's nodes on up to sixteen threads.
  Executable SHA-256
  `1444a54716fa286b6fad016fa8f5310c28c2e0a2cfd3fc7f20f133c4aa2e0919`; the
  load average at the start was 11.9 (the build had just finished).

| Gate | Fixes run 1 | Fixes run 2 | Run 1 | Run 2 |
| --- | --- | --- | --- | --- |
| Movement expiries | 1 | 2 | **0** | **0** |
| Players over the late p95 (0.25 m) | 2 | 2 | **0** | **0** (player 19 has no late windows) |
| Players over the 1 m maximum | 6 | 3 | 4 (1.06 to 1.39 m) | 1 (1.07 m) |
| Steady RSS growth (64 MiB bound) | 44 MiB | 35 MiB | 37 MiB | 38 MiB |
| Steady CPU / GPU frame p95 | 8.00 ms / not measured | 8.18 / 7.30 ms | 7.91 ms / not measured | 7.82 / 7.11 ms |
| Snapshot age p95 | 210 ms | 221 ms | 225 ms | 208 ms |
| Commit maximum | 0.69 s | 0.58 s | 0.092 s | 0.077 s |
| Slowest reward-history publication | 0.66 s | 0.56 s | 0.076 s (109 nodes, 68 ms syncing) | 0.024 s |

The stall is gone: no retained correction in either run has more than nine pending
inputs (the stall corrections had 15 to 21). Acceptance still fails on two things:

- **Single corrections just over 1 m.** Five corrections of 1.06 to 1.39 m,
  each with four or five pending inputs, all on open ground (support entity
  0). The estimate and the authority disagree about where another actor's
  capsule stopped the player, often along the arena's north wall at
  z = 16.15; the client sees other actors about 200 ms late.
- **Player 19 in run 2** (the frontline player) fails the 80 percent
  interval-movement share, as in the earlier run 2, so its late windows
  have no correction samples.

Run 1 also lacks the GPU measurement, by design (`VERSE_GPU_TIMING` is off).
