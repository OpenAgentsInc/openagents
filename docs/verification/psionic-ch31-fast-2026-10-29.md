# Chapter 31: Elephant fast on Metal (issue #12608), 2026-10-29

Same model, machines and request as the [chapter 31 record](psionic-ch31-2026-10-29.md):
the pinned original Elephant (`model.safetensors` SHA-256
`682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba`), a
MacBook Pro with an Apple M5 Max (128 GB, macOS 26), coderos-4080's RTX 4080,
and chapter 10's 86-token sample with a 49-token prefix. The Lab request is
the desktop app's five-question `LAB_TEST_BODY`.

Commits: `e23facf54a` (group kernels: threadgroup memory and barriers in the
Coax compute subset, Metal and CUDA), `1a2d3af3e1` (Metal 4 tensor-op tiles
for Elephant's projections), `fce09c894f` (forks: one prefix pass per
request, on Metal and CUDA). Session open was made fast separately by
`ab6211a70b` (verified-file records, chapter 15); this work only fixed its
macOS build (`st_mtimespec`).

## Group kernels

`psionic/tests/run_group_kernels.sh`: a 16x16-tiled matrix product that
stages tiles in group memory between barriers, compiled by `coax metal-compute`
and `coax cuda`, is bit-identical to the same Coax compiled to C on the M5 Max
and on the RTX 4080 (the 4080 borrowed through the GPU switch, gpt-oss-20b
restored afterwards). Admission refuses a group size that is not a multiple
of 32 or is over 1024 threads, and group memory shared other than through
writes each thread owns and barriers (coax/README.md, INVARIANTS.md).

## Tensor-op tiles (`psionic/tests/run_metal_kernels.sh tiles`)

`am_matmul_bf16` (Metal Performance Primitives `matmul2d`, bfloat x float ->
float). Bound frozen before measuring: 2^-20 of `sum |w x|` per output.

| 86-token shape | `dw_project` (warp) | tile 16x64 (16x32 for a/b) | rate |
| --- | --- | --- | --- |
| qkv 8192x4096 | 8.46 ms | 0.69 ms | |
| gate/up 12288x4096 | 10.5 ms | 0.73 ms | up to 11.9 TFLOP/s |
| down 4096x12288 | 13.2 ms | 0.76 ms | |
| z/out 4096x4096 | 3.75 ms | 0.34 ms | |
| k/v 1024x4096 | 1.53 ms | 0.20 ms | |
| a/b 32x4096 | 0.21 ms | 0.05 ms | |

Largest error 7e-7 of `sum |w x|`. The same tensor operation reaches 29.4
TFLOP/s on large half-precision shapes (our benchmark), the practical peak we
compare against. Accumulation order is the hardware's, so tiles are not
bit-identical to the Coax meaning.

## Decision (`psionic/tests/run_elephant_metal.sh`, `PSIONIC_DEVICE_QUALIFY=1`)

| Build | warm runs (ms) | warm median |
| --- | --- | --- |
| before (warp projections) | 2698.9 ... 3506.7 (4 runs) | 2915.5 ms |
| tiles | 201.7, 260.7, 400.2, 506.6, 481.5, 449.9 | 425.1 ms |
| tiles + forks (whole decision) | 200.3, 258.2, 391.7, 516.4, 507.0, 484.3 | 438.0 ms |

GPU time inside one command buffer varies the same way (182 to 515 ms): the
machine was shared with other agents' work, so the minimum shows the engine
and the median shows this machine that evening. Profile (tiles): 397.7 ms,
`dt_project_16x64` 56.3%, `dl_scan` 14.2%, `dl_gate_norm` 8.8%.

Acceptance (chapter 30's frozen rule, 0.0005 against the independent
reference and the recorded CPU result): passed every run; p(yes) 0.757192428
against the CPU's 0.757196285. Qualification: 40 launches of layers 0 and 3
and the readout repeated on the CPU, 19 identical; float-float reductions
within 4.62e-14; the tiles' f32 relative differences reach 145 on partial sums
near zero, as accumulation order allows.

Forked decision (prefix 49, suffix 37): prefix pass and suffix 566.9 ms,
suffix alone 244.2 ms, 0 of 496,640 logits differ from the whole decision.

## CUDA (`psionic/tests/run_elephant_device.sh`, RTX 4080)

Card free (gpt-oss-20b stopped through the switch, restored after): whole
decision 156.5, 147.1, 146.6, 147.7, 148.7 ms (unchanged from chapter 30);
forked prefix pass and suffix 239.8 ms, suffix alone 120.3 ms, 0 of 496,640
logits differ; both frozen checks pass. A first attempt ran while gpt-oss-20b
had been loaded again by someone else (0.93 GiB budget, 14.19 GiB streamed):
652 ms per decision, fork still bit-identical.

## Lab request (curl, five questions)

| Server | requests (s) |
| --- | --- |
| Mac, before | 29.2, 24.9, 27.0 |
| Mac, tiles | 5.40, 5.38, 4.71, 4.32, 4.09 |
| Mac, tiles + forks, same prefix repeated | 3.02, 2.57, 2.49, 2.42, 2.30 |
| Mac, tiles + forks, a new prefix each time | 2.45, 2.54, 2.47, 2.44, 2.27 |
| 4080, before (origin build) | 1.84, 1.83, 1.84, 1.85, 1.85 |
| 4080, forks | 1.31, 1.17, 1.17, 1.31, 1.31, 1.31 |

Answers are identical before and after on both machines (only `latency_ms`
differs). Per question on the Mac with forks (server timing build, not
committed): 313 to 814 ms, the first question carrying the prefix pass.

## Served and shown

The launchd agent `com.openagents.psionic-elephant-metal` runs the
`fce09c894f` build on 127.0.0.1:8793 (built with
`psionic/platform/serve-build.sh ~/psionic-elephant-metal release plain metal`):
authenticated in 136 ms from its verified-file record, 14.79 GiB placed in
3.2 s. The desktop app built from `fce09c894f`, offscreen (`--shot`, `mode lab
; send hi! what's the capital of France?`), shows "Routed to Psionic ·
gpt-oss-20b · Low · 4.3 s" and gpt-oss-20b's answer from the 4080 (the
server logged 4367 ms for that request, while the app rendered on the same
GPU). The app's GPU switch asked to load gpt-oss-20b, which was already
loaded, and the switch reloaded it (103 s); that is the app's switch logic,
not Elephant.

## Limits

One request shape. Questions run one after another, not batched. The Mac's
times depend on what else uses its GPU. Mammoth has not run.
