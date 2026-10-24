# Shared measurements

`measurement.mjs` owns the executable `aethyrforge.measurement.v1` contract and
the [JSON schema](../../../../../apps/aethyr/experiments/c-town/measurement.schema.json). Reports pin source changes, assets,
kernel, WGSL, quality, build, browser, OS, GPU, caches, warmup, and actual trial
order. Unavailable measurements and identities use `null`; a measured zero stays
zero. A fresh browser context does not imply a cold GPU, Wasm compiler, browser
process, driver, or OS cache.

Browser response sizes can be invalid, including negative values on a cached
reload. `summarizeResponseBodies` keeps the raw observation separate, counts
unavailable sizes, and reports a complete total as `null` when any size is
missing or invalid. Known partial bytes and measured zero remain distinct.

Distributions interpolate at `p × (n − 1)` on sorted observations. Frame summaries
separate callbacks, accepted submissions, skipped callbacks, and main-pass GPU
timestamps. Presentation remains unavailable. Kernel request time includes the
Rust JSON boundary; command encoding and render CPU time overlap nested work and
must not be added together. Dropped time is the last cumulative value minus the
first, within the observed window.

Simulation CPU, integrated rendering, startup, and editing remain separate phases.
`archiveEvidence` hashes exact prior JSON bytes and retains each report's schema
and limitations; it does not rewrite historical quantiles, timings, or claims.
The [player byte budgets](../../../../../apps/aethyr/experiments/kaigen/player-budgets.json) are enforced after build
dependency auditing. These local regression ceilings do not qualify the proposed
device, memory, or constrained-network targets.
