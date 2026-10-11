# Resident gate and up projection scheduling

This scratch probe tests one scheduling change: omit the buffer barrier between
independent resident-f16 gate and up GEMMs. Both variants use the same production
kernel bytes, thread counts, matrix dimensions, buffers, and arithmetic. There
is no model run, quantized-weight conversion, or product source change.

The probe uses the previously verified Flash layer 0 gate, up, and down weights
from `../gemm-tile-probe`, with all three half-file hashes checked before use.
The weights were decoded independently from the pinned Q4_K artifact; this is
not a new validation of the production decoder. Inputs and RMS scale values are
deterministic synthetic data. The RMS scale is not a captured model parameter.

## Command sequence

One concurrent compute encoder records each command buffer. All buffers use
tracked shared storage. The serial baseline inserts a buffer barrier before
each GEMM and before SiLU. The overlap variant omits only the gate-to-up barrier.

1. RMS normalization reads the f32 residual and writes packed half rows into
   `act16`, with width 4096.
2. A buffer barrier precedes gate projection. Gate and up independently multiply
   `[N,4096]` inputs by separate `[12288,4096]` resident half weights and write
   distinct f32 `[N,12288]` buffers.
3. The `normalized_pair` scope ends after the up dispatch. It includes RMS and
   both projections; it is not a gate/up-only measurement.
4. The `full_ffn` scope keeps a buffer barrier before SiLU. This waits for both
   projections and protects `act16` from overwrite while either reads it. The
   production SiLU expression writes half `[N,12288]` activations into `act16`.
5. A final buffer barrier precedes the unchanged down projection. It multiplies
   `[N,12288]` by `[4096,12288]` half weights and adds the original f32 residual.

The GEMMs use the production 64×128, four-SIMD-group, full-K path with 128 threads.
RMS and SiLU use the production 256-thread dispatch. The existing production
`encode_ffn` uses this order. Its on-demand Q4 path reuses one `weight16` buffer,
so the proposed scheduling change excludes that path entirely. Any later product
change must require both gate and up weights to be independently resident f16.

## Fixed checks and timings

The only correctness sizes are 1, 17, 63, 64, 65, 512, 1024, and 2048 tokens. Each
runs the full serial FFN once, then the overlap variant once. Every serial output
is checked for finite values, copied into owned CPU storage, and hashed. Every
candidate output must match every byte of gate, up, half activation, and final
f32 residual. There is no numerical tolerance.

At 512, 1024, and 2048 tokens, five fixed ABBA rounds run each scope. A is serial;
B is overlap. Reset and validation occur outside the measured command interval.
Every timed output is compared with the immutable serial expectation. Pair runs
also verify that the residual input and unused activation suffix are unchanged.
Every command checks all redzones. Source weights, immutable expectations, and
reset copies are hashed again after each size.

These counts are fixed: **136 command buffers, 560 dispatches, 120 timing rows**.
No new trials, warmups, retries, or count increases are automatic. The command's
GPU timestamps and CPU encode/submit/wait duration are recorded separately.
All data resets, hashing, copying, and validation are excluded from those times.
The external and internal deadlines include that overhead.

For each size and scope, compute the ratio `(B1+B2)/(A0+A3)` per round, then take
the median of the five ratios. Qualification requires a full-FFN ratio of **0.90
or less at every timed size**. Complete correctness alone does not meet this
speed gate. Aggregate GPU command times cannot prove that dispatches overlapped
on the device, and no model latency gain follows from this microprobe alone.

## Memory and cleanup

At 2048 tokens, eight Metal buffers use 587,223,040 bytes including 256-byte guards
at both ends. Owned reset and output copies use 352,321,536 bytes. Their combined
payload is 939,544,576 bytes. The actual point-in-time device allocation plus all
owned CPU copies and transient weight staging must remain at or below 1.5 GiB;
explicit buffers must remain at or below 1 GiB. One case is live at a time.
Weight staging is sequential and counted; there is no full model load.

The build guard uses build and 4 GiB memory leases, a 16 GiB startup requirement,
a 12 GiB host memory floor, 180 seconds, and two Swift compiler threads. The GPU
guard uses GPU and 2 GiB memory leases, a 14 GiB startup requirement, the same
12 GiB floor, and 60 seconds. The probe also checks a 50-second internal limit.
The existing monitor samples each owned process's RSS and physical footprint.
It stops only the newly owned process group, and a failure discovered during
cleanup still fails the receipt. Missing memory samples or source identities
fail closed. No quiet lease or desktop, service, power, or priority control is
permitted. Retain the first failure; do not increase limits automatically.

## Review and execution

This directory is source-only until root reviews it and assigns a workload
window. The deployment-target investigation and CPU precision diagnostic have
priority. `source_check.py` and `summary_check.py` run without a compiler, model,
or GPU. The frozen manifest binds the source and existing guard dependencies.
The build records the exact binary and Metal library hashes; the run rechecks
those identities before launch and records all cleanup outcomes.

After explicit handoff, run from this directory:

```sh
openagents lease build --keep-target-dir --receipt build-lease-1.json -- \
  openagents lease memory --amount 4 --receipt build-memory-lease-1.json -- \
  python3 guard.py build --output build-1
openagents lease gpu --receipt gpu-lease-1.json -- \
  openagents lease memory --amount 2 --receipt gpu-memory-lease-1.json -- \
  python3 guard.py bench --output run-1
python3 summarize.py run-1
```

The summary refuses missing or duplicate keys, invalid times, incomplete output
coverage, changed command counts, failed cleanup, or a failed guard. Its exit
status is nonzero for any correctness failure or unmet fixed speed gate.
