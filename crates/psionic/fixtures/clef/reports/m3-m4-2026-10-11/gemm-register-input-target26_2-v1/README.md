# Register-input GEMM with a macOS 26.2 deployment target

The candidate passed its fixed numerical checks but failed the speed gate. Do not integrate it. At 512, 1,024, and 2,048 token rows, its paired full-FFN GPU times were 2.5874, 2.4366, and 2.6351 times the production baseline.

| Token rows | Baseline full FFN median (ms) | Candidate full FFN median (ms) | Paired full-FFN ratio | Paired projection-sum ratio |
| --- | ---: | ---: | ---: | ---: |
| 512 | 2.7832 | 7.1071 | 2.5874 | 2.6437 |
| 1,024 | 5.9039 | 14.6040 | 2.4366 | 2.6326 |
| 2,048 | 11.3431 | 29.7234 | 2.6351 | 2.6380 |

Ratios are the median of the fixed pairs, not the ratio of the two medians. The frozen speed gate required both ratios to be at most 0.90 at every size. All 240 timing records remain in the ordered [JSONL parts](run-1/stdout.jsonl/part-00.jsonl), including outliers. [The summary](run-1/summary.json) contains every paired ratio. Shared-machine load and temporal drift limit interpretation; these are projection microbenchmarks, not model latency or evidence of a specific hardware cause.

## What passed

The one build and one bench attempt completed. The bench checked all 12 exact cooperative-input and residual roundtrips before GEMM, 24 small cases, and five real-Flash-weight cases. It completed 319 command buffers and 547 dispatches. Source inputs, redzones, immutable expectations, and repeated outputs passed their checks.

Small cases compare every output to an independent f64 oracle. The maximum candidate error was 0.0000139642; the maximum baseline/candidate difference was 0.00000977516. Real-weight cases compare every gate, up, activation, and down output and sample 96 f64 oracle coordinates per projection. Their largest output difference was 0.0000143051, and largest sampled candidate f64 error was 0.00000573272. The weighted oracle does not cover every output. The predeclared half-intermediate gate was absolute error at most 0.001 **or** at most one half ULP; f32 checks remained at 0.001. No gate changed after execution.

The [build receipt](build-1/receipt.json) reports 6.813 seconds. The [bench receipt](run-1/receipt.json) reports 11.371 seconds, a peak sampled accounted footprint of 1,632,896,752 bytes under the 2 GiB process cap, no monitor or cleanup error, and the owned process group stopped. The GPU plus owned-expectation peak was 1,259,159,552 bytes under the 1.5 GiB allocation bound. The smallest sampled available host memory was 96,953,008,128 bytes, above the 12 GiB floor. Both build and GPU lease receipts are included. The lease inventory was empty after completion. No other task-owned heavy workload overlapped the timing window; unrelated desktop activity was not controlled.

## Why this is a separate attempt

The previous `gemm-register-input-probe` failed its first input roundtrip before any GEMM. A separate coordinate export then established that the SDK's default Metal 4.0 deployment minimum selected destination-only helpers for input cooperative tensors. An unchanged shader built for macOS 26.2 selected operand-aware helpers and passed both exact coordinate exports. Those earlier failures and the proof remain in their separate immutable bundles.

This attempt changes only the deployment target of both shader builds, guard dependencies, and source bookkeeping. The candidate shader, Swift host, arithmetic, cases, timings, and acceptance gates are unchanged. [Source-delta records](source-delta.json) cover all 17 source files; 13 are byte-identical. [The patch](source-changes.patch) contains the remaining changes. Both shaders use `-std=metal4.0 -mmacosx-version-min=26.2`.

The resulting host executable SHA-256 is `4ab065220c3254324d8cf6dc6c5d7b8906092909d3a3957a94fdbb6ba3820a4b`; the metallib SHA-256 is `20ef7ef48565421742b7aa4c57b036d29c364167fdfbceaf50a4794154e9cd82`. [Build records](build-artifacts.json) include the exact sources and SDK header hashes. No executable, shader library, or model weights are published here.

This establishes an offline compiler-target requirement for this experiment. It does not establish the deployment behavior of the product's runtime source compiler, nor full-model accuracy. Product code is unchanged.

## Reproduction and file handling

[The frozen plan](source/README.md), [host](source/probe.swift), [shader](source/candidate.metal), [build wrapper](source/build.py), [guard](source/guard.py), and [analyzer](source/summarize.py) are included. Their source path labels must be replaced with local paths before execution. The original recorded hashes identify what ran; the published-file hashes identify the scrubbed copies. Do not treat a scrubbed file as the exact executed file.

Run `python3 verify_bundle.py` to verify published hashes and reconstruct the complete bench JSONL as `run-1/stdout-restored.jsonl`. Reassembly concatenates every line without dropping or reordering records. [The sanitization manifest](sanitization.json) maps original files and hashes to their published copies. The memory timelines are small and included in full. No failed speed results or original attempt outcomes were removed.
