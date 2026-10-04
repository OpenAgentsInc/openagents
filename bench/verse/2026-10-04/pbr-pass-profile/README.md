# Per-pass command profile

Native run from `cb1f379cef`, 3456 × 2104, 4× anti-aliasing, 100 seconds, 23 fireballs, respawn, and live reload. Compilation from another session overlapped loading; this is diagnostic evidence rather than an exclusive machine benchmark.

The budget fails: work p95 34.520 ms, delivered p95 35.340 ms, zero dropped simulation time. `timings.json` shows command finalization dominates, rather than the calls that record individual passes. These are CPU wall-clock timings, not GPU timestamps; finalization can include backend command replay and driver synchronization. This evidence does not establish shader execution cost.

The next investigation is resource tracking and command replay during finalization, including redundant material texture bindings in shadow draws. No rendering quality settings were reduced. The full profile, checker result, native images, and reload receipt are retained.
