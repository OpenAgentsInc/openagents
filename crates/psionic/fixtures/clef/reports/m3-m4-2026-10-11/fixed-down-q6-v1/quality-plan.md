# Quality work after the controlled comparison

The current evidence does not support a runtime-only fix for the unchanged Q4
artifact. For case 014, independent CPU-f32 execution with the same GGUF
reproduces the native wrong answer within 0.000164 in probability. Using
original BF16 weights with those f32 operators restores the reference answer;
replacing only the FFN weights also restores it. This identifies weight
precision as a cause in that case. It does not prove the cause of all 104
answers or establish that any proposed Q6 or calibrated Q4 artifact passes.

Keep the strict M4 gate: all 104 top answers agree and maximum probability
difference is at most 0.02 against the original BF16 goldens. Same-input head,
encoder, image, prefix, batching, and speed requirements remain separate.
Do not change temperatures, thresholds, reference artifacts, or head weights
to fit the acceptance answers.

1. Build and verify the fixed FFN-down Q6 candidate described in the README.
   This is the smallest ready-to-implement mixed-precision experiment: it uses
   the upstream predetermined 32-layer policy, preserving 160 of 192 FFN
   matrices as Q4_K and every other tensor byte. Validate the helper against
   upstream and the generated Q6 payloads with an independent decoder before
   model evaluation. Check whether current resident placement fits the
   existing 52 GiB process cap; refuse if the preflight does not fit.
2. If needed, add a separate opt-in packed Q4_K/Q6_K/Q8_0 FFN placement.
   `clef_metal.rs` already has all three dequantizers and a format-generic
   `linear_with_workspace`; only `upload_matrix_with_placement` restricts
   on-demand retention to Q4_K. Keep default behavior and conversion math.
   Cover serial and batched requests, the 170 MiB shared conversion slot,
   buffer accounting, format parsing, repeated requests, and exact comparison
   with resident f16 weights. This patch enables the candidate to fit; it
   cannot repair old Q4 weights.
3. Evaluate the frozen candidate against the full unchanged acceptance set,
   then images and fresh paired speed measurements. The current large misses
   warrant a full check. Do not use each acceptance failure to choose another
   layer to promote.
4. If the fixed mixture fails, build an activation-weighted Q4 candidate from
   original BF16 FFNs, using a separate, fixed calibration corpus covering
   decision schemas, long states, and image-bearing records. Collect input
   squared magnitudes for gate/up/down tensors; validate complete, finite
   coverage of all 192 matrices. The existing streamed original-f32 reference
   can collect these statistics without a full-model allocation. Do not
   assume the generic llama-imatrix text driver reproduces Clef record
   boundaries: its driver tokenizes a continuous text stream. Compare the
   collected format and statistics against the pinned upstream collector.
   Keep the acceptance set out of calibration and model selection.
5. If stronger mixed precision is still required, select a format budget on
   the independent development corpus before testing acceptance. Q6 or Q8
   FFNs provide controls and possible fallbacks. An all-Q6/all-Q8 model is not
   evidence that the explicit Q4 requirement passed. The published pinned
   Q8 artifact is 28,732,215,360 bytes; it has not been downloaded here.

There is no repository GGUF Q4 importance-matrix quantizer to reuse. The
`psionic-nn` module quantizer currently accepts its own `Int8Symmetric` format,
which is not GGML Q8_0. Reuse pinned upstream GGML quantization for this
infrastructure work. Regenerate higher-precision values from the original
BF16 checkpoint; requantizing the current Q4 payloads cannot recover discarded
information.

These payload estimates keep all non-FFN bytes from the current artifact.
Metadata stays the same size for the prepared fixed-subset candidate.

| FFN policy | Estimated artifact bytes | Additional packed Metal weights |
| --- | ---: | ---: |
| Current all-Q4 FFNs | 19,232,219,200 | 0 |
| Fixed 32 down matrices Q6 | 19,967,533,120 | 0.685 GiB |
| All 64 down matrices Q6 | 20,702,847,040 | 1.370 GiB |
| All 192 FFN matrices Q6 | 23,644,102,720 | 4.109 GiB |
| All 192 FFN matrices Q8 | 27,788,599,360 | 7.969 GiB |
| All 192 FFN matrices BF16 | 43,831,812,160 | 22.910 GiB |

The packed-weight column is not a process RSS forecast: mapped source pages
can coexist with Metal copies. Request scratch remains separate. Without the
packed-placement extension, Q6/Q8 FFNs become resident f16 and all-FFN
promotion adds 22.910 GiB of Metal buffers. Loading a dense BF16 GGUF also
eagerly retains CPU-f32 matrices in the current host loader; it is not a safe
drop-in memory plan. The one-layer original-f32 diagnostic already ran within
the 12 GiB process-group guard, so that remains the bounded calibration path.

Primary sources: [quantizer options](https://github.com/ggml-org/llama.cpp/blob/62a6f74bab748b1755f4cd8dfc0871caf865c6c8/tools/quantize/README.md),
[importance-matrix collection](https://github.com/ggml-org/llama.cpp/blob/62a6f74bab748b1755f4cd8dfc0871caf865c6c8/tools/imatrix/README.md),
[selection and quantization implementation](https://github.com/ggml-org/llama.cpp/blob/62a6f74bab748b1755f4cd8dfc0871caf865c6c8/src/llama-quant.cpp),
and [published Q8 artifact](https://huggingface.co/ggml-org/Clef-GGUF/blob/d2aa75cb425ae922b5c3c15ef3157d417b7f7641/Clef-Q8_0.gguf).
