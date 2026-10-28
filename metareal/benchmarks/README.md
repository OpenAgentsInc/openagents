# R0: real engine kernels and the live Metal path

This harness measures the current `metareal/cast.coax` player pose against a
pinned historical native evaluator using the actual app's `Cast.bin`. It also
measures the six robots' poses/body capsules and player movement/collision
through the real generated C ABI. It makes no full-engine equivalence claim.

```sh
python3 metareal/benchmarks/run.py --output /tmp/coax-r0 --repeats 9
python3 metareal/benchmarks/run.py --output /tmp/coax-r0-asan --repeats 1 --sanitize
# Proposed performance gate (all three profiles):
python3 metareal/benchmarks/run.py --output /tmp/coax-r0-budget --enforce-budget
```

Python is measurement tooling only. Product workload selection, sample
validation, quantiles and dispersion are in `metareal/benchmarks.coax`, an
authority unit with checks and a zero-survivor mutation gate. Native code
observes clocks, storage and allocation calls. The Python host independently
recomputes ranks/medians and validates reference/input hashes.

## Frozen reference and assumptions

`contract.json` pins source commit, three original file hashes, reader adapter
hash, actual corpus hash, palette tolerance, workload and a **proposed** 5%
median regression limit. The original files in `reference/` remain byte-exact
copies from commit `47f431ba99524a219faa2e11d2adee20388440b6` of the historical
engine repository. No historical build scripts are restored or run.

At build time `reader-f32.inc` replaces only `Data::skin`: the current MRC1
asset stores already decoded/normalized f32 transforms, while the historical
asset used half-float texture rows. The pinned `evaluateAll`, clip frame
selection and blending execute unchanged. A separate native MRC1 reader
constructs its own offsets/joints/clips. Native placement rotates basis
vectors to produce the palette; it does not copy the Coax matrix expression.

The animation state supplied to both evaluators comes from Coax. This tests
the pose calculation, **not independent animation-state-machine parity**.
The reader adaptation, full-body mask and palette tolerance are visible
assumptions requiring independent review before an adoption sign-off.
The original evaluator's temporary vector remains in the reference: actual
allocations are counted, never silently removed to improve a result.

## Workload and profiles

The actual cast has 65 player joints and 402 robot joints. After 180 warm-up
frames, each repeat records 540 frames over idle, walk, run, airborne,
idle and dance phases. Pose calls alternate order to reduce ordering bias.
The fixture also drives real robot body construction and player collision
against those capsules, with no headless terminal walls. The live app keeps
its actual world geometry. Inputs, not recorded frame durations, are shared.

| Mode | Generated Coax arithmetic | C/C++ optimization |
| --- | --- | --- |
| `debug` | Checked | `-O0` |
| `app` | Checked, as the current iOS generator emits | `-O2` |
| `release` | Runtime integer wrapping via `--release` | `-O2` |

Both sides receive the same optimization, `-g` and `-ffp-contract=off` flags.
No LTO is used. Sanitized runs add ASan/UBSan and frame pointers and are
correctness evidence, not performance baselines. Generation includes source
mapping; results retain first/repeated codegen, compiler phases, C compile,
link and raw kernel samples. OS caches and unrelated system load are not
controlled. Repeated invocations are not a resident/incremental compiler.

Every player palette element is compared with absolute tolerance `0.0002`.
Each timer includes monotonic-clock overhead; empty-clock samples are reported
and not subtracted. Median, nearest-rank p95, mean and population coefficient
of variation are recorded. Inspect tails and variance before acting on a
ratio, particularly for operations of a few microseconds.

Allocation instrumentation intercepts malloc/calloc/realloc/free in generated
C and ordinary C++ new/new[] in the reference on the measured thread. It
requires zero observed Coax pose/robot/player allocations. It does not claim
to observe system allocator internals, aligned allocations, other threads,
Swift/Metal allocations or total process memory. Caller buffers are allocated
outside the kernel timers; value copies and stack use still cost time.

The default command fails on correctness, input/hash or allocation violations.
It records a missed proposed timing budget without pretending success means
speed parity. `--enforce-budget` additionally exits nonzero on that miss.
The latest retained nine-repeat run passes this unchanged median gate in all
three profiles; [the pose follow-up](../../docs/verification/coax-r0-pose-2026-10-28.md)
records the rewrite, raw samples, stack tradeoff and remaining tail limits.
A result on one Mac is not a universal performance guarantee.

## Real iOS frames

Debug builds accept the explicit `-OABenchmark` launch argument. Start the
Grid normally; the Coax fixture then supplies movement input to the actual
controller, cast and Metal renderer. No mock rendering or benchmark screen is
introduced. Release builds compile the capture adapter out.

```sh
cd apps/openagents-ios
xcodegen generate
xcodebuild -project OpenAgents.xcodeproj -scheme OpenAgents \
  -destination 'platform=iOS Simulator,id=YOUR_SIMULATOR_UUID' \
  -derivedDataPath build CODE_SIGNING_ALLOWED=NO \
  -parallel-testing-enabled NO -only-testing:OpenAgentsUITests/R0UITests test
xcrun simctl get_app_container YOUR_SIMULATOR_UUID com.openagents.app data
```

The data container's `Documents/R0-frame.json` retains raw CPU samples and
completed command-buffer GPU clocks when available. A missing clock is absent,
not zero. The capture reports completed commands and GPU errors. Frame CPU,
robots/bodies, player update, pose **plus palette copy**, buffer copy, encode
and GPU durations have different scopes; CPU submission does not wait on GPU
execution, and their medians must not be added as a frame total. Upload timing
here measures host buffer copies, not a universal PCIe/DMA transfer latency.

The UI test waits for 540 requested frames to be recorded. Inspect the report
for all counts and errors too. Statistics/file serialization happens after
capture. Simulator results establish the real software path, not physical
iPhone speed, memory pressure, power use or GPU performance. Device evidence,
independent reference review and agreed tail budgets remain R0 work.

See [the retained evidence](../../docs/verification/coax-r0-2026-10-28.md).
