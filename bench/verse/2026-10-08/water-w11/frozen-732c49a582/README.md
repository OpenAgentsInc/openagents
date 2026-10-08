# Frozen water measurements

These 2026-10-08 measurements use application source
`732c49a582474fa6e669716637d1e0b194d5e8ba`. The native artifact includes
harness-only failure retention at `ba2deeb24fef40279638d3920361e7314795a9f4`.
The browser artifact is a test-compiled, export-only benchmark derivative;
production must use the normal release build.

`receipt.json` records exact artifacts, inputs, commands, leases, raw-data
digests, and limitations. Each `.json.gz` decompresses to the original raw
record bytes. `native-summary.json` reports sample counts, transitions,
and peak declared residency. `browser-visual-audit.json` distinguishes
successful startup from four black WebGPU captures; these cases do not
pass visual acceptance. WebGL2 captures render correctly. Captures remain
in scratch because the browser scenes include licensed kit content.

Acceptance remains incomplete. Low timestamps use an intrusive split-pass
probe, supported timestamp observations are sparse or absent, negative
fence differences are indeterminate, and the budgets are not calibrated.
No missing GPU observation is replaced with zero. Browser main-thread
intervals are elapsed time; its waves run inline, and it exposes no thread
CPU clock. Native main-thread and worker values use thread CPU clocks.

The read-only dry/wet WebGPU observer proves that fresh blended passes
omit scene bind group 0, invalidating every submission. Timestamp maps
succeed but return all-zero counters. `webgpu-diagnosis-summary.json`
reports the errors; the compressed record retains complete device,
Log, network, shader, canvas, and query observations. The executed observer
and its exact hash are retained. No source fix or decoder relaxation is
included in this frozen evidence.
