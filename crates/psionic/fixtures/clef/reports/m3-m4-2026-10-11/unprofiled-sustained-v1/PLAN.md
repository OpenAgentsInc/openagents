# Unprofiled FFN submission diagnostic

Status: proposed source plan only. No build or GPU execution is authorized by
this file. Root coordinates the next run after the separate BF16 pilot.

First establish whether the slowdown exists without a system profiler. Compare
one command in flight with at most four on the same queue. Both paths execute
the same fixed sequence of current weight-shape kernels and identical inputs.
Do not change queue priority, process QoS, host power settings, applications,
services, or other tasks.

## Existing evidence

The retained instrumented probe completed 522 FFNs in 13.016 seconds. Recomputed
GPU medians are 13.52 ms in the first second, 29.49 ms in seconds one through
three, and 24.56 ms afterward. Typical between-command GPU gaps remain
0.42–0.50 ms; a few larger gaps exist. See `prior-trace-reanalysis.json` for
values and the exact source hash. These observations put most of the measured
slowdown inside the command GPU interval. They cannot distinguish shader work,
preemption, memory behavior, or profiler effects.

The earlier synthetic `metal_sustained_gemm.swift` records only whole-command
intervals for 1, 4, 16, or 48 dispatches. It cannot locate a transition among the
dispatches inside one command. Its original comments prescribe a quiet lease;
this new experiment explicitly does not use one.

## Fixed first experiment

- Use the verified actual Flash layer-0 gate/up/down half weights and unchanged
  production weight-shape kernels at N=2,048, K=4,096, FFN width=12,288.
- Each command contains gate, up, SiLU, and down with float32 output. Use the
  nonaccumulating down form to match the earlier sustained trace. This is an
  isolated FFN projection test, not a full transformer layer or model request.
- Use one set of intermediate buffers and four separate final-output buffers.
  All are tracked resources on one queue. Pipeline dispatches retain their
  explicit buffer barriers. The pending command list never exceeds four.
- Before timing, run the small independent double-precision oracle and obtain
  the complete baseline FFN output. Verify finite values, guards, and hashes.
- Run exactly A/B/B/A, 128 FFNs per phase. A commits and waits after each call.
  B refills a four-command queue after retiring the oldest completed command.
  Do not force an idle interval or add a warmup that obscures the initial ramp.
- Keep kernels, buffers, per-command work, and input data identical. Commands
  overwrite their output without CPU resets. Hold each command and buffer until
  completion, and reuse a final-output slot only after its prior command ends.
- Store each command's CPU encode/commit/wait boundaries, driver kernel interval,
  and GPU start/end times in a fixed-size host record. Do not print, allocate
  buffers, hash data, or scan large outputs between timed calls. Emit the records
  after each phase drains. Collect full final-output comparisons and unchanged
  source hashes at the phase boundaries, outside recorded command intervals.
- Bound explicit allocations below 768 MiB; account for the baseline result,
  four final outputs, intermediate buffers, and one set of three weights. Verify
  the exact calculation before implementation. Do not stage additional weights.
- Run under GPU and 2 GiB memory leases, with the existing owned-process guard:
  14 GiB available before launch, 12 GiB available-memory floor, 60-second outer
  deadline, and a 50-second internal stop. A timeout is a retained incomplete
  result, not permission to increase the bound.

Retain every command. Report per-phase and relative-time-bin GPU durations,
between-command gaps, overlap if present, first/last intervals, CPU submission
cost, and total wall time. Use paired A/B phase order rather than selected minima.
The first A phase remains special because it starts first; do not treat this
single sequence as a causal power-policy experiment.

If only wall gaps change while GPU intervals remain stable, submission overhead
is implicated. If GPU intervals slow within both modes, pure host starvation is
insufficient to explain the effect. A difference in GPU intervals between modes
is evidence about workload submission, not proof of a thermal or frequency cause.
If the unprofiled run shows no repeatable slowdown, retain that result and stop.

## Optional follow-up, requiring a new review

Only if the first result leaves a material ambiguity, sample the owned compute
passes through `MTLCounterSampleBuffer`. The retained M5 capability inventory
supports stage-boundary timestamps and does not support dispatch-boundary
sampling. The installed SDK exposes start/end samples on
`MTLComputePassSampleBufferAttachmentDescriptor`; one encoder per measurement
unit gives an owned timestamp interval without a system trace. A small fixed
sample buffer is sufficient. Check resolved length, counter error values,
monotonicity, and time units before deriving intervals. Splitting encoders or
sampling can perturb execution, so keep it a separately labeled diagnostic.
There is no frequency counter in the observed inventory.

Primary sources:

- [Apple's counter sampling guide](https://developer.apple.com/documentation/metal/sampling-gpu-data-into-counter-sample-buffers)
- [Apple's start-of-compute-pass sample index](https://developer.apple.com/documentation/metal/mtlcomputepasssamplebufferattachmentdescriptor/startofencodersampleindex)
- [Apple's live GPU counter walkthrough](https://developer.apple.com/videos/play/tech-talks/10001/)

The SDK's `MTLComputePass.h` lines 33–49 define start/end encoder samples;
`MTLCounters.h` lines 185–195 define shared-buffer resolution and error values.
These APIs identify intervals, not the hardware cause of an interval changing.
