# GPT-OSS original 20b device execution, 2026-10-29

Scope: chapters 45a–45c, original BF16/MXFP4 weights resident on the owned
NVIDIA RTX 4080, Coax-generated CUDA arithmetic and Coax dispatch policy.
The plan was updated first in `d1380d5015`. Full mutation roots and routine
original CPU inference were excluded at the owner's explicit direction.

## Build and device

- Worker: `coderos-4080`, Linux/NixOS, NVIDIA GeForce RTX 4080, compute 8.9,
  reported memory 16,376 MiB, driver 595.71.05. The desktop also uses this GPU.
- Isolated build/run directory: `/tmp/psionic-gptoss-gpu.z4KpOS`. No edits to
  another agent's checkout, no downloaded scripts executed.
- CUDA/NVRTC toolkit: Nix CUDA 13.2,
  `/nix/store/qsqinxjbdm680fd6jwnp9lbxiwdhh72l-cuda-13.2`.
- CUDA emitter: `coax0 0.23.0`, executable SHA-256
  `0634fac1dc50a9eb2c9bf18e51486b567d996dcdfe47e9d66112c847f010eb46`.
  Trusted previously built compiler copied into this isolated directory.
- Native emitter: local trusted `coax0 0.22.0`, executable SHA-256
  `c5acf492faaef7ea8450245c44b454e69b208f83d9594e874be4d06eb3d3ae1a`.
- Worker host C: GCC 15.2.0, C11, `-O3 -ffp-contract=off -fno-fast-math`.
- NVRTC: device compute architecture, `--fmad=false --prec-div=true
  --prec-sqrt=true --ftz=false --std=c++17`. No cuBLAS/cuDNN runtime.
- Generated CUDA SHA-256:
  `7466822732327e7e428febefb3d0b167a154ae3229e743d09bbf550c436ee11b`.
- Original identity: the independently pinned chapter 45 20b profile, revision
  `6cee5e81ee83917806bbde320786a8fb61efebee`; complete model artifacts
  authenticated by Coax at session admission. Original bytes remain in the
  external cache, outside Git.

## Focused checks

`psionic/tests/check_gptoss_device.sh` tests the changed kernels/planners,
executes owned tiny weights in debug/release native plain/ASan/UBSan,
compares independent dense expected results, checks serial/batch splits and
complete cache byte equality, and verifies the book example. The CUDA tiny
run exercises the same launch program on the real 4080. Missing CUDA has no
CPU fallback; default CPU emulation is deliberately tiny-only.

Seven authored kernel checks cover all E2M1 codes/signed zero/scales, BF16,
packed products and bias, routing, normalization, clamps, residuals,
sink/window attention, ragged expert groups and shadow publication. The
prefill root also executes its imported model/format/YaRN checks.

All batch splits 1/3, 2/2, 3/1 and 4 match the owned dense model within the
frozen 0.0005 scaled bound. Every public cache byte equals serial execution.
An Inf injected in the final original-format head causes arithmetic refusal
in both serial and batch modes, preserving the entire public cache and logits
and leaving position unchanged. Production does not expose the patch API.

## Original qualification

`gpt-oss-cuda-f32-v1` uses f32 short product sums, ordered small f64 reductions
and f32 storage. It is distinct from `gpt-oss-cpu-wide-v1`.
The bound was frozen at `abs(actual-reference)/(1+abs(reference)) < 0.0005`
before reading GPU captures. Expert IDs and vocabulary top token must match
exactly. `gptoss_device_oracle.py` reads the retained independent original CPU
NPZ; no original CPU inference is run by this gate.

The real-device harness requires both retained prefixes and a completed,
parsed low-reasoning Harmony return with exactly one final display message.
A 64-token exploratory generation was truncated and rejected as evidence;
the qualified run allows 256 and completes “Say hi.”. Intermediate model
arithmetic and routing stay on device. Production reads only small control
records until the generated token stream is parsed on the host by Coax.

## Memory and throughput

The plan reserves 13,761,300,984 weight bytes, 527,933,440 workspace bytes and
134,217,728 additional headroom. Parameters relocate unchanged to aligned
intervals in one device allocation. No dense weight expansion is stored.
Public cache and chunk shadow are included in workspace. Admission checks
observed free bytes after module setup; allocation remains fallible.

Timing is host monotonic wall time, including Coax launch-plan construction,
launches and explicit synchronization. Startup authentication, compilation
and upload are reported separately. Forced-token core timing is separate from
actual generated Harmony text.

An exploratory repeated-token serial workload measured about 19 seconds for
1,024-token prefill and 44.6 decode tokens/s at that context. A first batched
four-row/eight-token array tile measured about 17.3 seconds prefill and was
rejected as an adequate acceleration. It is not the delivered scalar tile.
Those exploratory numbers use distinct corpora and are not a controlled
speedup comparison. The final benchmark uses diverse deterministic IDs,
one warm-up and five measured repetitions of 1,024 input / 128 forced output.

The 2-second prefill goal, packaged-reference comparison and concurrency
1/4/8 serving qualification remain explicit gates in `psionic/book/PLAN.md`.
This work does not establish tensor-core use, 120b CUDA/Metal execution,
sampled device speculation, schema-masked tools or production serving.

## Delivered original 20b measurements

The exact [run log](psionic-gptoss-device-2026-10-29/run.log),
[benchmark summary](psionic-gptoss-device-2026-10-29/benchmark.json),
[qualification/digests](psionic-gptoss-device-2026-10-29/qualification.json)
and [tested build hashes](psionic-gptoss-device-2026-10-29/build.json)
are retained with this record.

| Fixed workload / quantity | Measured result |
| --- | --- |
| Warm-up | One uncounted 1,024-input / 128-forced-output run |
| 1,024-token prefill, five repeats | P50 **4.325127 s**, nearest-rank P95 4.333102 s |
| 128-token decode, five repeats | P50 **4.498565 s**, nearest-rank P95 4.501769 s |
| Decode rate | Median **28.454 tokens/s**, range 28.433–28.501 |
| Observed allocated device memory (includes other GPU users/context) | 15,212,740,608 bytes, 1,498,611,712 free at observation |
| Cold admission/authentication/module/upload | 84.684232 s; excluded from warm timing |
| Actual “Say hi.” request | 59 input IDs, 24 generated IDs, return stop; prefill 0.235215 s, decode 0.637129 s |
| Long GPU parity | Complete cache and logits bitwise identical for a varied 129-token serial/batch prefix across window/chunk boundaries |
| Retained CPU original vectors | Both prefixes pass all fixed bounds; exact routes/top token. Largest scaled residual error 0.00048050535092247825, below 0.0005 |

The initial decode-rate goal passes. The **2-second prefill goal does not**.
No packaged comparator is installed on this worker, and no concurrent-service
claim is made. A diagnostic [event timing run](psionic-gptoss-device-2026-10-29/profile.log)
points primarily at the selected MXFP4 products; it synchronizes each launch,
so its totals are diagnostic, separate from the uninterrupted benchmark.
The next work is faster cooperative/matrix products, tiled attention and
launch replay, followed by the same fixed qualification and throughput gates.

The production shell was also built without test APIs and exercised with two
independent stdin requests in one resident session: “Say hi.” and “What is
2 + 2?”. Both returned complete final messages, with warm prefill 0.234/0.253 s
and decode 0.632/0.607 s. Admission/upload happened once (60.319 s in this run).
The [interactive log](psionic-gptoss-device-2026-10-29/interactive.log) is retained.

## Qualified tuning: row views and launch occupancy

The [second run](psionic-gptoss-device-2026-10-29/tuning-1/run.log),
[summary](psionic-gptoss-device-2026-10-29/tuning-1/benchmark.json),
[qualification](psionic-gptoss-device-2026-10-29/tuning-1/qualification.json)
and [exact build](psionic-gptoss-device-2026-10-29/tuning-1/build.json) retain
the next delivered implementation. Original arithmetic and the frozen bound
are unchanged. Complete 129-position serial/batched GPU cache and logits are
still bitwise identical, routes/top token still exact, and Harmony returns.

The qualified 64-query/8-KV-head attention path uses bounded row views and
u32 addresses; power-of-two ring masking is used only for capacities 128/2048.
Other admitted head/capacity combinations retain the generic path. An authored
wrapped-window check covers old and current-chunk values at distant heads and
dimensions. Packed-product blocks now contain 128 threads, chosen by Coax.
Static driver [resource data](psionic-gptoss-device-2026-10-29/tuning-1/resources.log)
([read-only tooling source](psionic-gptoss-device-2026-10-29/tuning-1/resources.c))
reports 154 registers/thread for batch MXFP4, zero local-memory bytes and zero
shared-memory bytes. This supported testing smaller blocks; it is not a
hardware-counter occupancy measurement.

| Same workload, one warm-up and five repeats | Tuned result |
| --- | --- |
| 1,024-token prefill | P50 **3.219239 s**, nearest-rank P95 3.232044 s |
| 128-token decode | P50 **4.326436 s**, nearest-rank P95 4.332186 s |
| Decode rate | Median **29.586 tokens/s** |
| Observed allocated device memory | 15,212,740,608 bytes, 1,498,611,712 free |
| Complete 59-ID prompt / 24-ID generation | Prefill 0.208834 s, decode 0.607641 s |

Warm prefill falls about 26% from the first delivered baseline. The 2-second
goal is still open. A metadata-only memory preflight now refuses impossible
residency before original SHA-256 work or module compilation; successful
construction still authenticates and checks memory again. Real-device tiny
checks verify zero-budget refusal even with invalid artifact bytes.

Further tile experiments used the opt-in owned original-shaped
`gptoss_device_product_bench.c`, avoiding whole-model startup. Four-row/four-token
products improved this isolated constant-data slice slightly; two-row/four-token
products regressed. Neither was promoted. Slice timing is not full-model
throughput. An original-wide column-view specialization also stayed near the
baseline and was discarded. Cooperative matrix support remains the next
substantial step, with graph replay and the serving gates still planned.
