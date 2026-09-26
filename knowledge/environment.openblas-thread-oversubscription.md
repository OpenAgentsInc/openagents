---
id: environment.openblas-thread-oversubscription
version: 1
kind: environment
title: Control OpenBLAS thread oversubscription in CPU workloads
summary: >-
  OpenBLAS may make a small CPU workload slower when it creates more worker
  threads than useful; benchmark thread counts and set the runtime limit for
  both timing and training.
tags: [openblas, blas, cpu, performance, threads, containers]
applies_when: >-
  Running CPU numerical workloads linked against OpenBLAS, especially inside
  containers with a small CPU quota.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - caffe-cifar-10
  cites:
    - OpenBLAS, Frequently Asked Questions, How can I use OpenBLAS in multithreading applications?
    - OpenBLAS, Runtime variables, OPENBLAS_NUM_THREADS
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

The BLAS library's thread pool can oversubscribe a constrained environment or add synchronization overhead for workloads whose operations are too small to benefit from parallel BLAS. Do not infer performance from the reported host CPU count alone. Benchmark representative work with candidate `OPENBLAS_NUM_THREADS` values, then use the best observed setting consistently for the actual run. This environment variable is documented by OpenBLAS as its runtime thread-count control.

Citations: OpenBLAS, *Frequently Asked Questions*, “How can I use OpenBLAS in multithreading applications?”; OpenBLAS, *Runtime variables*, `OPENBLAS_NUM_THREADS`.

## How to check

```sh
for n in 1 2 4; do
  OPENBLAS_NUM_THREADS=$n timeout 120 ./your-program benchmark-arguments
 done
```

Compare elapsed time or workload throughput on identical inputs and avoid changing numerical configuration between benchmark and production execution. For training, verify completion and iteration count in the captured log after choosing the setting.
