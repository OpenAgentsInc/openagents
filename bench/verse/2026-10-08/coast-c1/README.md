# Coast C1 captures

The procedural bay at high tide, rendered at 960 × 540 on each quality tier.
The records retain the fixed camera, shared tick, adapter, and PNG SHA-256.
High-tier estuary and low/high-tide pool views also show the inland bodies
and the shelf channel. These images contain no licensed assets and use no
lighting bake.

Run the `verse` example's ignored `capture_coast` test under a GPU lease,
with `COAST_CAPTURE_OUTPUT` and `VERSE_QUALITY` set. The example also captures
the harbor, estuary, and pools at both tides. Warm-up uses image readbacks;
these records make no GPU timing claim.

Validation: eight coast tests; entry, return, and plaza clearance tests;
58 CPU water tests; shared shader validation; and three RTX 4080 water
parity tests. Shelter texture interpolation differs from the analytic mask
by at most 0.003077 in gain on Low and Medium, and 0.000697 on High.
The browser-target and native CLI checks pass. Physical device qualification remains
with C6 in `NEEDS_OWNER.md`.
