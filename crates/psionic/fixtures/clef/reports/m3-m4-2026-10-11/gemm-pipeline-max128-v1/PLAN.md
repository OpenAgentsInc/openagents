# Fixed 128-thread GEMM pipeline limit

This scratch probe tests one compiler input: set
`MTLComputePipelineDescriptor.maxTotalThreadsPerThreadgroup` to 128 for the
unchanged overwrite and accumulate GEMM functions. Every dispatch already uses
128 threads. The baseline uses `makeComputePipelineState(function:)`, matching
production. The candidate uses the same function from the same Metal library
through a fresh descriptor, with only that limit set. It keeps the default
`threadGroupSizeIsMultipleOfThreadExecutionWidth` value (false). The probe
checks descriptor defaults and returned pipeline limits before any dispatch.
RMS and SiLU share their unchanged baseline pipelines in both variants.

## Evidence and scope

The retained resident-overlap run reports a maximum of 1,024 threads for both
GEMM pipelines, although their actual threadgroup size is 128. Source inspection
of the earlier `gemm-*` probes found capacity checks, but no descriptor assignment
to this limit. Apple describes the limit as an input to compiler optimization
and register spilling in
[Metal Compute on MacBook Pro](https://developer.apple.com/videos/play/tech-talks/10580/)
and documents the
[pipeline property](https://developer.apple.com/documentation/metal/mtlcomputepipelinedescriptor/maxtotalthreadsperthreadgroup).
Those sources motivate this experiment; they do not show that this kernel spills
or predict a speedup on M5. Returned thread limits and static threadgroup memory
are recorded for all six pipelines. They do not measure register allocation or
NAX occupancy.

The shaders remain byte-identical to production. Compilation uses the same
Metal 4.0 defaults for both variants; no deployment-target, precision, tile,
weight-layout, dispatch-size, or source change is mixed into this comparison.
The probe uses independently decoded, previously verified real Flash layer 0
half weights from `../gemm-tile-probe`. Deterministic residuals and RMS scales
are synthetic. No model runs and no weight conversion occur.

## Fixed execution and correctness

Both variants use one concurrent compute encoder with an identical sequence:
RMS normalization to `act16`, buffer barrier, gate GEMM, buffer barrier, up GEMM.
The `normalized_pair` scope stops there. The `full_ffn` scope adds a buffer
barrier, production SiLU into reused `act16`, another buffer barrier, and the
down GEMM accumulating the original f32 residual. Gate and up are separate
resident half weights. All buffers use tracked shared storage.

Correctness sizes are exactly 1, 17, 63, 64, 65, 512, 1024, and 2048 tokens.
For each size, the full default FFN runs first. Every output must be finite;
owned CPU copies preserve its gate, up, half activation, and final f32 residual.
The candidate must match every output byte before that size's timings start.
This intentionally requires bitwise equality despite a possible compiler
lowering change; no tolerance is introduced after a failure.

At 512, 1024, and 2048 tokens, each of five rounds executes both scopes in fixed
ABBA order: default, max128, max128, default. Every timed output must match the
immutable default expectation. Reset and full byte comparisons occur outside
the command timestamps. Pair-only calls additionally check that the residual
and unused activation suffix remain unchanged. Every command checks redzones;
weights, reset copies, and expectations are hashed after every size.

Counts are fixed at **136 command buffers, 560 dispatches, and 120 timing rows**.
The per-round ratio is `(B1+B2)/(A0+A3)`. Qualification requires the median of
five full-FFN ratios to be **at most 0.90 at each size**. Pair-scope ratios are
reported separately. There is no automatic retry, cap increase, extra size,
second descriptor flag, or variant search. Shared-machine temporal drift remains
a limitation; a microprobe win would still need full-model checks.

## Memory and execution limits

The largest case has 587,223,040 bytes of guarded Metal buffers and 352,321,536
bytes of owned CPU copies. Combined payload is 939,544,576 bytes. Actual device
allocation plus owned copies and transient weight staging must stay below
1.5 GiB; explicit buffers stay below 1 GiB. One case is live at a time. Pipeline
compiler allocations are additionally covered by the process-group guard.

The reviewed guard uses build plus 4 GiB memory leases, 16 GiB available memory
at startup, a 12 GiB floor, 180 seconds, and two compiler threads. The single
bench uses GPU plus 2 GiB memory leases, 14 GiB at startup, the same floor, and
60 seconds externally, with an internal 50-second deadline. RSS and physical
footprint are monitored for the owned group. Any sampling, cleanup, hash,
allocation, correctness, or deadline failure is retained and stops the attempt.
No quiet lease, desktop app, service, power, priority, or system setting change
is permitted. Missing source identities fail closed.

## Review and reproduction

Source preparation is authorized; compilation and execution require root's
separate workload handoff. `source_check.py` and `summary_check.py` are small,
model-free Python checks. The frozen manifest records every source and guard
hash. The build records library and executable hashes, which the run rechecks.
The baseline/candidate source delta from the reviewed overlap harness is saved
in `source-delta.json` and `probe-origin.patch`; that earlier probe is unchanged.

After review and explicit handoff, the fixed commands are:

```sh
openagents lease build --keep-target-dir --receipt build-lease-1.json -- \
  openagents lease memory --amount 4 --receipt build-memory-lease-1.json -- \
  python3 guard.py build --output build-1
openagents lease gpu --receipt gpu-lease-1.json -- \
  openagents lease memory --amount 2 --receipt gpu-memory-lease-1.json -- \
  python3 guard.py bench --output run-1
python3 summarize.py run-1
```

An incomplete run or unmet speed gate returns a nonzero summary result. Preserve
all original outputs, including failures; do not infer kernel speed from a failed
pipeline compilation or incomplete correctness run.
