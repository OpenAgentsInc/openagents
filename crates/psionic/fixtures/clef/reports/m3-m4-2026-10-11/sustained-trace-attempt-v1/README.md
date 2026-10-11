# Bounded Metal trace attempt

The owned Flash FFN probe completed 522 calls in 13.016 seconds. Its seven-output
f64 oracle passed, sampled output bits stayed identical, output guards stayed
unchanged, and all 8,388,608 final outputs were finite. This was one FFN layer,
not a model request or an acceptance latency benchmark.

The profiler failed during finalization after the probe exited. Its sampled
owned-group RSS jumped from about 375 MB to 7.84 GB, overshooting the 2 GiB sampled
cap; the raw trace also crossed the 1 GiB output bound. The guard stopped only its
owned process group. Cleanup passed and all leases were released. The lowest
recorded host available memory was 98.79 GB. Neither bound was raised, and there
was no retry, fallback, permission change, quiet lease, or desktop/service control.

The partial trace contains 1,239,660,656 bytes and was not finalized or exported.
It remains outside Git because a system trace can contain unrelated process
metadata. This directory contains only owned samples with relative timestamps,
aggregate facts, and hashes of small original evidence files. No raw trace table,
process ID, absolute path, wall-clock time, weight tensor, or model input is copied.

The first second had a median GPU interval of 13.52 ms. Seconds one and two rose
to 29.01 and 29.88 ms, then the intervals settled near 24–25 ms. Most wall time
outside the GPU interval remained about 0.27–0.34 ms. The first dispatch also had
12.28 ms of CPU driver scheduling. These are instrumented timings; do not combine
them with unprofiled latency results.

Thermal state remained fair and low-power mode remained off. The public Metal
counter inventory exposed only timestamps: stage-boundary sampling was supported,
dispatch-boundary sampling was not. No GPU frequency or performance-state result
is available. The samples reproduce a slowdown but do not distinguish power
policy, scheduling, memory behavior, or profiling overhead, and do not establish
a thermal cause.
