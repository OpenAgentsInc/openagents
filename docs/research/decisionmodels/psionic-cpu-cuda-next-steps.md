# Psionic CPU and CUDA: proposed next steps

Date: 2026-10-15. Status: proposed; the work below has not started.

Continue improving native execution on the available Linux CPU and NVIDIA GPU.
None of these steps requires Metal or Apple hardware. Keep native and ONNX
Runtime (ORT) selectable, with shared preparation and answer processing.

The [existing execution roadmap](psionic-onnx-native-roadmap.md#current-measured-comparison)
records the completed implementation and benchmark evidence. Native beats ORT
on the tested Linux router and digit workloads. These are warm measurements
on an i7-14700K and RTX 4080, not a claim about every workload or deployment.
At exactly 512 tokens, native CPU latency is about 66 ms p50 and 74–79 ms p99;
both engines still miss the separate 20 ms p99 CPU target.

## Proposed order

| Priority | Work | Completion evidence |
| --- | --- | --- |
| 1 | Release server under sustained load | Paired native/ORT CPU and CUDA results from the actual server executable, including overload and recovery |
| 2 | Cold start and CUDA memory | Separate startup, first-request, unseen-shape, and warm timings; measured host RAM and device memory |
| 3 | Faster CPU execution | An explicitly labeled INT8 experiment with correctness, task quality, latency, and memory comparisons against float32 |
| 4 | CUDA throughput | Measured gains from batching, scheduling, or kernel changes without unacceptable tail latency or memory growth |
| 5 | Deployment packages | Reproducible native-only and ORT packages with clean installation, startup, dependency, and size checks |
| 6 | Router task evaluation | A separate report establishing whether the router makes useful decisions before routing promotion |

Start with priorities 1 and 2 to establish deployment baselines. Use those
results to choose the next CPU and CUDA optimizations. Router evaluation can
proceed independently; packaging follows the selected implementation changes.

## 1. Benchmark the release server

The current CUDA HTTP comparison exercises production routes inside an
integration-test executable with one serial loopback client. Extend coverage
to the actual release server executable, with both engines and devices.

- Measure concurrency 1, 2, 4, and 8, then increase load until throughput
  saturates. Include short inputs, exactly 512 tokens, and a fixed mixed corpus.
- Report throughput, p50, p95, and p99 latency, queue time, errors, and memory.
  Include a sustained run that exposes memory growth or performance drift.
- Exercise queue limits, request cancellation, overload responses, and recovery
  after load stops. Verify that request failures do not affect other answers.
- Compare answers with direct execution and record the actual engine, device,
  precision, CPU affinity, thread budget, and any ORT CPU fallback.

Completion means repeatable results and fixes for defects found in this scope.
Keep loopback measurements distinct from gateway, TLS, and remote-network costs.

## 2. Measure cold start and device memory

Current warm timing does not establish startup or first-shape behavior. Current
GPU memory comparisons report host resident memory, not peak VRAM.

- Separate process startup, artifact loading, readiness warmup, first response,
  and steady-state execution. Identify whether filesystem caches are warm.
- Measure a previously unseen sequence length, repeated lengths, and enough
  lengths to exercise CUDA graph cache eviction. Check correctness after reuse.
- Measure process host memory and GPU memory during loading and execution.
  Record the measurement method and sampling interval; distinguish sampled
  device peaks from allocator accounting and whole-device usage.
- Test repeated load/unload cycles and bounded cache behavior. Optimize buffers
  and graph retention only after identifying their latency and memory costs.

Run memory instrumentation separately when it distorts latency. Publish both
engines under the same protocol, including idle memory and peak observations.

## 3. Experiment with INT8 CPU execution

Keep the current float32 implementation and fixtures as the baseline. Investigate
INT8 projection weights and matrix multiplication as a separate precision variant.
Do not present quantized results as a same-precision improvement.

- Profile the current CPU path before selecting operations to quantize.
- Record quantization scales, calibration inputs where required, converter
  revision, and artifact hashes. Keep evaluation inputs separate from calibration.
- Compare raw output error, probabilities, and decision changes with float32.
  Establish acceptable task-quality loss before selecting a quantized variant.
- Measure short and 512-token inputs with one- and four-thread budgets,
  including packing cost, startup, package size, and resident memory.

The 20 ms p99 target remains a goal, not a promised outcome. Reject changes that
trade away required quality or worsen important workloads without a clear benefit.

## 4. Improve CUDA throughput

Use the release-server measurements to determine whether kernel execution,
launch overhead, transfers, or scheduling limits throughput.

- Evaluate bounded batching of compatible requests, including padding overhead
  and a maximum waiting time. Compare against unbatched execution at equal load.
- Evaluate concurrent execution only where it improves measured throughput.
  Bound buffers and in-flight work; preserve cancellation and overload behavior.
- Profile attention and projection operations before adding further fusion.
  Retain changes only when paired measurements show useful gains.
- Treat FP16 or other precision changes as separate experiments, with the same
  numerical and task-quality checks required for CPU quantization.

Report throughput together with tail latency and peak memory. A faster isolated
kernel alone does not establish better serving behavior.

## 5. Refresh deployment packages

Earlier package measurements predate the final native optimizations. Rebuild
native CPU, native CUDA, ORT CPU, and ORT CUDA packages from pinned revisions.

- Record executable, model, runtime, and total installed sizes separately.
- Verify native-only packages start without an ORT library. Verify each package
  installs and starts with only its declared dependencies and model artifacts.
- Record startup time, selected device, and actionable failures for missing or
  incompatible dependencies. Keep reduced-operator ORT results separate.

## 6. Evaluate whether the router is useful

Follow the [router experiment plan](s1-llm-auto-router.md). Build a versioned,
representative task set and evaluate routing accuracy, confidence calibration,
downstream answer quality, and estimated cost savings. Include uncertain and
out-of-distribution inputs. Numerical agreement with ORT validates execution;
it does not establish that a model should route application traffic.

Keep routing promotion separate from engine optimization. Respect the Coder
1.0 binary freeze; any integration must follow the
[repository contract](../../../AGENTS.md).

## Evidence and scope

Use the [existing benchmark protocol](psionic-onnx-native-roadmap.md#correctness-and-benchmark-protocol):
pin artifacts and software, alternate engine runs, retain individual timings,
and record shared-host load and limitations. Reuse existing harnesses and
report formats. Separate implementation fixes from optional precision variants.

This proposal does not reopen completed issues or claim Apple/mobile
qualification. It does not require a general native ONNX interpreter. Add issue
links as follow-up work starts, and keep measured outcomes in the execution
roadmap. This change records the proposal only.
