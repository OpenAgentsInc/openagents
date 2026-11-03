# Parakeet on Linux with CUDA — 2026-11-02

GitHub #12672. This record qualifies Parakeet TDT 0.6B v3 on an NVIDIA GPU
under Linux, with the pinned original weights
(`541d1f99c6b0c3cd0b11a95167540bb8edefd82b`) and Psionic's own
Coax-generated kernels. The frontend runs on the CPU; the FastConformer encoder
and the TDT decoder run on the GPU. No cuBLAS, cuDNN, Python or Rust runs in
the product.
The CUDA driver and NVRTC are opened at run time with `dlopen`, so the build
needs no CUDA headers or libraries.

Machine: coderos-4080, NVIDIA GeForce RTX 4080 (16 GiB, driver 595.71.05),
NVRTC 13.2 from Nix (`~/cuda-13.2`), Intel i7-14700K, GCC 15.2, Linux 6.18.
The machine was shared with other agents' work (load average about 11), so
CPU times are noisy.

## Piece 1: the encoder

`psionic/lib/parakeet/fast.coax` already scheduled the FastConformer encoder
for Metal through one foreign launch, `pkq_launch`. The CUDA profile reuses
that schedule unchanged. Only the kernel table differs:

- `psionic/lib/parakeet/kernels.coax`: every encoder kernel except the
  product (features, convolutions, gather, SiLU, GLU, depthwise conv with batch
  norm, relative-position attention, adds). These used to live in
  `device.coax`; they moved so `coax cuda` and `coax metal-compute` share them.
- `psionic/lib/whisper/device-common.coax`: Whisper's `wd_expf` and the row
  norm `wd_norm`, moved out of `whisper/device.coax` for the same reason.
- `psionic/lib/parakeet/cuda.coax`: `pkc_linear`, a plain f32 SIMT product
  with the same 64-row by 32-output tiling as Metal's `wd_linear`. Profile
  PARAKEET-CUDA-F32-v1: each output adds its terms in f32 in ascending input
  order with one rounding per operation (NVRTC `--fmad=false`), then the bias.
  The C build of the kernel is therefore bit-identical to the device; its
  checks compare it with sequential f32 sums and with binary64.
- `psionic/platform/parakeet-cuda.c`: the effects. It compiles the generated
  CUDA C with NVRTC once per process, uploads the weights once, maps each Coax
  view onto its device buffer, uploads the host-written position table before
  the first launch, launches, waits and downloads the encoded frames.

The Metal source the Mac build compiles is unchanged by the move: the
generated Metal for `whisper/device.coax` and `parakeet/device.coax` differs
only in source line numbers and temporary names (checked by normalizing both
and diffing).

`backend.coax` (`#authority`, 23 of 23 mutants killed) names the device that
ran in every JSON result: `cpu-exact`/`cpu`, `metal`/`apple-metal` or
`cuda`/`nvidia-cuda`. `psionic_parakeet_device()` reports it from C.

### Evidence

- **Tiny shapes against the compensated CPU encoder**
  ([device-check.stdout](encoder/device-check.stdout)):
  `psionic/tests/parakeet-device.c`, now built for either backend, passes
  6 of 6 cases (1, 17 and 33 frames; subsampling and one layer) within
  `0.001 * (1 + max|reference|)`, plus the weight-count and NaN refusals.
- **Original-weight slices**: subsampling and encoder layers 0 and 1 of
  `6930-75918-0011` with the real weights, against the CPU encoder: largest
  error 0.0083 (subsampling, limit 5.3), 0.00026 and 0.00023 (layers).
- **The 29 recorded NeMo clips, whole product path**
  ([full-cuda-comparison.json](encoder/full-cuda-comparison.json),
  [corpus.stdout](encoder/corpus.stdout)): CPU frontend, CUDA encoder, CPU
  TDT decoder, through the public C API (`psionic/tests/parakeet-runtime-gpu.c`),
  compared with `psionic/tests/parakeet_reference_compare.py` against the
  frozen NeMo record in `../parakeet-2026-11-02/reference/nemo-record/`.
  All 29 pass: every token ID, frame ID and text byte equals NeMo's, and every
  feature and encoded value is inside the frozen tolerance
  `0.001 + 0.001 * |reference|`. The largest encoded difference is
  0.0000411 (3.7% of its tolerance); the features are the CPU frontend's,
  unchanged.

### Speed

One resident process, 29 clips, 298.4 s of audio:

| | |
| --- | ---: |
| Model open (hash check, weights into memory, NVRTC compile 0.47 s) | 9.1 s |
| Recognition, all 29 clips | 13.3 s |
| Real-time factor (recognition time / audio time) | 0.045 |
| 3.2 s clip (`6930-75918-0011`) | 209 ms |
| 18.6 s clip (`fi`) | 779 ms |

The first recognition after open also uploads the 2.4 GB of encoder weights
(about 0.2 s). The split for a warm call, measured separately:

| Clip | Audio | CUDA encoder | CPU frontend and TDT decoder |
| --- | ---: | ---: | ---: |
| `6930-75918-0011` | 3.2 s | 140 ms | 62 ms |
| `en` | 10.6 s | 266–273 ms | 154–164 ms |
| `fi` | 18.6 s | 411 ms | 319–355 ms |

For comparison, the CPU path on this machine takes 8 to 15 times the audio's
length (30 s for a 3.8 s turn including the model open;
`docs/verification/verse-parakeet-2026-11-02/`).

## Piece 2: the decoder, and a faster encoder

With the encoder on the GPU, the CPU TDT decoder took 30–45% of the time.
`psionic/lib/parakeet/device-tdt.coax` now runs greedy TDT decoding on the
device. Its loop, admission and results are `tdt_decode`'s: after each joint it
reads back three values (token, duration, a non-finite flag), applies
`tdt_rule` in Coax and chooses the next launches. The kernels are in
`cuda.coax`, launched through the same `pkq_launch` ABI (kinds 13–17):

- `pkc_rows`: one dense product per warp, used by the LSTM gates (both matrices
  and both biases), the prediction projection and the joint head. The
  encoder projection of every frame is one `pkc_linear` call.
- `pkc_joint_input`, `pkc_cell`: the joint's ReLU and the LSTM cell, keeping
  `tdt.coax`'s binary64 formulas (now in `tdt-math.coax`) rounded once to f32.
- `pkc_pick`: the first largest token and duration, as `tdt_joint` picks them.

Profile PARAKEET-CUDA-TDT-F32-v1: dense sums are f32 in a fixed lane order,
not the CPU's compensated binary64, so decisions are qualified against NeMo's
tokens rather than bit for bit against the CPU profile. Decoder weights are
not re-checked per call: the engine only decodes with the model it
authenticated by SHA-256 at open. Encoded frames are checked on every call.

The encoder also got faster. Profiling one 3.2 s clip with Nsight Systems
showed the attention at 2.3 ms per layer (320 single-thread items). Kind 10
now binds `pkc_attend`: one 128-thread group per (head, query), the same
scores and a direct softmax. `pkc_linear` now runs 256 threads and loads the
next step while it multiplies the current one. Every output still adds its
terms in ascending order, so its values are unchanged. The engine uploads the
weights at open, so the first recognition is as fast as the rest.

### Evidence

- **Tiny decoder against `tdt_decode`**
  ([device-check.stdout](decoder/device-check.stdout)): 36 seeded tiny-shape
  decodes (3 symbol caps, 6 seeds, 5 and 17 frames) produce exactly the CPU
  decoder's tokens and frames, including 60 tokens emitted on the same frame
  as the one before (zero durations and the symbol cap). A short token store
  returns the capacity status and an out-of-range frame the number status.
  The encoder's tiny and original-weight slice checks pass again.
- **Decoder alone on the 29 NeMo encodings**
  ([decode-only-record.json](decoder/decode-only-record.json)):
  `parakeet_reference_native.py --gpu` decodes NeMo's own encoded frames.
  29 of 29 have NeMo's exact tokens, frames and text, with backend `cuda`. The
  four refused inputs leave the last result unchanged, and a second engine is
  refused.
- **Whole path on the 29 clips**
  ([full-cuda-comparison.json](decoder/full-cuda-comparison.json)): 29 of 29
  have NeMo's exact tokens, frames and text. Every encoded value is inside the
  frozen tolerance; the worst is 0.0000383 (3.5% of its tolerance).

### Speed

| | Encoder only on CUDA | Whole engine on CUDA |
| --- | ---: | ---: |
| Recognition, 29 clips (298.4 s of audio) | 13.3 s | 2.74 s |
| Real-time factor | 0.045 | 0.0092 |
| 3.2 s clip | 209 ms | 46 ms |
| 18.6 s clip | 779 ms | 193 ms |
| Model open | 9.1 s | 9.5 s, including the 2.6 GB weight upload |

A warm call, split (`6930-75918-0011`, 3.2 s): encoder 33 ms, decoder 1.5 ms,
CPU frontend about 9 ms. On the decoder-only run the decoder takes 4 to 24 ms
per clip, against 60 to 295 ms on 8 CPU threads.

### Reproduce

```shell
psionic/platform/parakeet-build.sh /tmp/pk           # on Linux: CUDA + CPU
coax/bin/coax c psionic/tests/parakeet-device-reference.coax --library --release --header /tmp/pk/parakeet-ref.h > /tmp/pk/parakeet-ref.c
cc -std=c11 -O2 -ffp-contract=off -fno-fast-math -DPSIONIC_PARAKEET_CUDA -I/tmp/pk -Ipsionic/platform \
  /tmp/pk/parakeet-ref.c psionic/tests/parakeet-device.c /tmp/pk/parakeet-cuda.o /tmp/pk/cuda-host.o -ldl -lpthread -lm -o /tmp/pk/device-check
/tmp/pk/device-check                                  # tiny shapes
python3 -I psionic/tests/parakeet_oracle.py ranges "$PARAKEET_MODEL" /tmp/pk
/tmp/pk/device-check "$PARAKEET_MODEL" /tmp/pk/ranges.txt psionic/fixtures/parakeet/6930-75918-0011.pcm
cc -std=c11 -O2 -Ipsionic/platform psionic/tests/parakeet-runtime-gpu.c /tmp/pk/libpsionic-parakeet.a -ldl -lm -lpthread -o /tmp/pk/runtime-gpu
/tmp/pk/runtime-gpu "$PARAKEET_MODEL" INPUTS.tsv /tmp/pk/corpus      # ID<TAB>PCM path, 29 lines
python3 -I psionic/tests/parakeet_reference_compare.py psionic/docs/verification/parakeet-2026-11-02/reference/nemo-record \
  /tmp/pk/corpus psionic/fixtures/parakeet-reference/manifest.json /tmp/pk/comparison.json
```

The decoder alone on NeMo's encodings:

```shell
python3 -I psionic/tests/parakeet_reference_native.py "$PARAKEET_MODEL" /tmp/pk/libpsionic-parakeet.a \
  psionic/docs/verification/parakeet-2026-11-02/reference/nemo-record /tmp/pk/decode-only --gpu
```

`INPUTS.tsv` names the four LibriSpeech clips in `psionic/fixtures/parakeet/`
and the 25 FLEURS clips in `psionic/fixtures/parakeet-reference/`, in the
reference record's order.
