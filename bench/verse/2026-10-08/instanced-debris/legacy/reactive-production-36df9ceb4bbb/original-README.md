# Instanced debris evidence

The current production kit passes the 16.7 ms p99 target for
[#10937](https://github.com/OpenAgentsInc/openagents/issues/10937) in both
development and release builds. Source `36df9ceb4bbb7fb0c7be30bd1c3538e0be78ab3e`
renders High at 1920×1080 on Apple M5 Max / Metal with TAA, settled lighting,
and 960 simulation frames. Both runs retain every timing sample, including
startup, selected image readbacks, and the final drain.

`current-dev-frame-ledger.jsonl` and `current-release-frame-ledger.jsonl`
export the exact 960 rows from each raw report. All four selected PNGs from
each run remain retained, including establishing and ground smoke views.

| Current profile | Swarm p99, ms | Aftermath p99, ms | Swarm maximum, ms |
| --- | ---: | ---: | ---: |
| Development | 14.257 | 12.369 | 18.240 |
| Release | 12.498 | 6.872 | 15.267 |

These ordinary `--live --settle-light --no-video --seconds 16` runs save
impact frame 469 and smoke frame 600 without extra numbered captures. At most
two submissions remain pending. Actual adjacent-start host intervals include
simulation, mesh preparation, submission, admission waits, and selected
readbacks; the final interval includes the drain. Submitted and completed
indices are exactly 0–959. PNG writes and progress logs follow the timed run.
This measures offscreen throughput cadence, with completion latency reported
separately. It does not simulate presentation, vsync, or a compositor, and it
does not replace elapsed time with a CPU/GPU overlap estimate.

The production kit `c5599554…` produces 509 chunks; the historical DAE kit
produces 700. The configured ceiling remains 700. Source, asset, profile, and
timing policy differences prevent a controlled historical speedup claim.
Development builds retain the repository's package optimization overrides;
release builds use the production profile. GPU timestamp diagnostics are
disabled in these ordinary captures, so no GPU-duration claim is made.

The prior accepted production runs on `46cf41188b` remain in
`legacy/pre-reactive-production/`, with their exact reports, ledgers, images,
commands, executable hashes, leases, and check results. Their development
swarm p99 is 12.958 ms, and their release swarm p99 is 11.071 ms. Current
acceptance uses the reactive source above.

All earlier serial reports and seven newer DAE failures remain retained.
Their acceptance results stay unchanged under `historical_serial_final`,
`runs`, and `intermediate_runs` in `verification.json`.

| Historical source | Profile | Swarm p99, ms | Result |
| --- | --- | ---: | --- |
| `eb6992520c` | dev | 19.390 | Failed |
| `94a1d864c5` | dev | 21.254 | Failed |
| `254bc815bf` | dev | 23.500 | Failed |
| `58991a3264` | dev | 24.439 | Failed |
| `d2657acead` | release | 18.143 | Failed |
| `8e13f4b01c` | dev | 30.343 | Failed |
| `8e13f4b01c` | release | 29.371 | Failed |

The original serial runs wait for the current frame; their pixel policies
differ. Newer bounded reports retain all 960 ledger rows and separate callback
observation latency. No startup frame or outlier is removed. The query
release report's 5,124.976 ms startup maximum remains recorded.

`current-command-manifest.json`, build logs, and quiet/GPU receipts bind the
current reports to executable digests and commands. Scoped formatting and
current CPU checks pass. `current-checks.sh` records the exact check and build
commands. The failed E0502 and E0061 check attempts remain in
`historical-reactive-checks/`.

Four [native motion and reactive tests](../temporal-aa/legacy/reactive-center-only/proof/native-manifest.json)
pass on fixture source `df355b0cd1b45843b54aea40a6e6f3d665dfa237`. The first
reactive test attempt retains its
[shader parse failure](../temporal-aa/legacy/reactive-center-only/proof/native-attempt0/10936-reactive-native-2.log).
The fixture fed GLES and native conditional shader variants together; the
fixture-only fix selects native WGSL. The production capture binaries remain
the `36df9ceb4b` builds. Earlier B3 light-upload tests remain supplementary
evidence in the archived production check record.

The impact and aftermath stills show textured rubble, fire, smoke, and the
standing damaged neighbor. No missing chunk surface or detached light patch
is observed in these views. Stills do not establish motion quality; the
[temporal evidence](../temporal-aa/README.md) retains the paired sequences.

Run `python3 check.py` to verify both debris and relighting retained artifacts,
source and lease bindings, and every current and archived production ledger
row. `SHA256SUMS` covers every retained file except itself.
