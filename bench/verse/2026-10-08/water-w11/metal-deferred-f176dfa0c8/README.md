# Deferred Metal query measurements

All 15 Apple M5 Max fixed views return 96 valid GPU samples out of 96 with
query resolution deferred until render submission completion. Each case
holds its own GPU and quiet lease. The prior failed experiments remain in
`../metal-timer-diagnosis/`; their thresholds were not weakened.

The executable was compiled from `f9ad77960a` plus the exact patch in its
receipt, committed as `f176dfa0c8`. Budget calibration and the RTX 4080
regression checks remain pending. Images stay in private scratch.
