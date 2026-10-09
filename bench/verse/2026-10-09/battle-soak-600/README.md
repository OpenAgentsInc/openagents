# Ten-minute battle soak on main (#10559)

Two `battle_scale combined … 600` runs on `coderos-4080` (i7-14700K,
RTX 4080, Linux), source `1f16fad522` (main, after `f548ba0229` and
`aa2ae109b8`), the licensed battle pack, 2560 × 1440, the delayed
pipeline route (40 ms ± 20 ms a chunk). The executable's SHA-256 is in
`executable.sha256`. The reports are gzipped. Run 2 adds
`VERSE_GPU_TIMING=1` so the GPU gate is measured.

| Gate | Oct 6 trace run | Run 1 | Run 2 |
| --- | --- | --- | --- |
| Movement expiries | 11 | 0 | 1 |
| Players over the late p95 (0.25 m) | 10 | 0 | 1 (0.267 m) |
| Players over the 1 m maximum | 1 | 4 (1.03 to 3.60 m) | 10 (1.05 to 4.27 m) |
| Steady RSS growth (64 MiB bound) | 93 MiB | 76 MiB | 70 MiB |
| Steady CPU frame p95 | pass | pass | 7.97 ms |
| Steady GPU scene p95 | pass | not measured | 7.28 ms |
| Snapshot age p95 | pass | 223 ms | 206 ms |

Run 1 also has player 19 (the frontline) passing. In run 2, player 19
fails the 80% interval-movement share, so its late windows have no
correction samples.

## What the remaining failures show

- **Memory is a step, not a slope.** The resident set rises over the
  first two minutes, jumps about 26 MiB when all twenty clients reconnect
  at the midpoint (300 s), and then stays flat (under 6 MiB over the last
  300 s in both runs). The 64 MiB bound fails on the reconnection step.
  That step is either retained per-connection state or allocator high
  water; these runs do not tell which.
- **Corrections over 1 m fall into two kinds.**
  1. Simultaneous corrections of 2.0 to 4.3 m for players 3, 5, and 7 at
     232.5 to 232.8 s in run 2, each with 18 to 21 pending inputs, about
     160 ms of unconfirmed movement. That moment matches run 2's single
     movement expiry. The authority stalled for several ticks, and the
     confirmations that followed moved every waiting player at once.
  2. Single corrections of 0.9 to 1.6 m (players 1, 6, and 14 in run 2;
     0, 2, and 19 in run 1). The prediction is about 0.5 m up, standing on
     a spell's collider (entity `4294967296`, the spell range), while the
     authority keeps the player on the ground. The predicted estimate's
     support is the spell solid; the authority's is the ground (entity 0).
     The prediction steps up onto spell geometry that the authority does
     not treat the same way at that moment.
- Run 1's 3.6 m correction (player 15) starts from a baseline whose
  support is actor 240's second life at 0.24 m. The prediction's
  estimate moved 3.47 m with zero input, which is the actor's position
  added once: the replay carried the character by the support's whole
  pose. This needs a test before it changes.
