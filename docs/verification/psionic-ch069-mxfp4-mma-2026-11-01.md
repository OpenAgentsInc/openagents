# Chapter 69: original MXFP4 expert products on tensor cores, 2026-11-01

## What was measured

The MXFP4-BF16-MMA-v1 expert products in
`psionic/lib/device/mxfp4-mma.coax` (`mm_split`, `mm_part`, `mm_finish`)
against the binary64 reference and against chapter 66's
gpt-oss-cuda-f32-v1 kernels (`gk_mx_part` and `gk_mx_finish` for one token,
chapter 67's `gk_batch_mx_part` for 32 tokens per expert), at gpt-oss-20b
expert shapes: 32 experts, 2,880 columns, 5,760 rows for the first
projection and 2,880 for the second, four selected experts (IDs 3, 11, 19
and 30). Both profiles' kernels are emitted by `coax cuda` into one module.

Weights: synthetic codes and scales (118 to 125) at the first projection's
shape, and the original layer 0 `mlp1` and `mlp2` tensors and biases of
gpt-oss-20b (`model.safetensors` at revision
`6cee5e81ee83917806bbde320786a8fb61efebee`). Activations are uniform in
[-1, 1] from a fixed seed; they are not retained model activations.

Command (cached compiler, `COAX_CC=gcc`):

```shell
COAX_CC=gcc psionic/tests/check_mxfp4_mma.sh \
  ~/.cache/psionic/gptoss-original/20b/6cee5e81ee83917806bbde320786a8fb61efebee
```

## Host

| Item | Value |
| --- | --- |
| Machine | NixOS, Linux 6.18.49 x86-64, GCC 15.2.0 |
| GPU | NVIDIA GeForce RTX 4080, 16 GB, compute capability 8.9 |
| Driver | 595.71.05 |
| NVRTC | CUDA 13.2, through `psionic/platform/cuda-host.c` (`--fmad=false`) |
| Compiler | the cached coax release build `8925679b5e19…` |

## Bound, frozen before comparison

With n tensor-core instructions adding into one f32 accumulator
(n = 2 · 180 / slices for 2,880 columns), every output y must satisfy

    |y - y64| <= (2^-16 + (n + 2) · 2^-20) · (sum |w x| + |bias|)

where y64 is the binary64 sum of chapter 60's gold coefficient bits times
the f32 activations, plus the bias. Every output must also pass chapter 66's
rule against the gpt-oss-cuda-f32-v1 output:
`|candidate - reference| / (1 + |reference|) < 0.0005`.

## Results

Every checked output was within the bound and no output was flagged. For one
token, every output was checked; for 32 tokens, every seventh row. Errors
are fractions of `sum |w x| + |bias|`.

| Case | Slices | Worst MMA error | Bound | Worst f32 error |
| --- | --- | --- | --- | --- |
| synthetic mlp1, 1 token | 1 / 4 / 12 | 1.56e-06 / 4.94e-07 / 4.24e-07 | 3.60e-04 / 1.03e-04 / 4.58e-05 | 5.94e-08 |
| synthetic mlp1, 32 tokens | 1 / 4 | 1.33e-06 / 5.26e-07 | 3.60e-04 / 1.03e-04 | — |
| original layer 0 mlp1, 1 token | 1 / 4 / 12 | 6.94e-07 / 3.29e-07 / 2.69e-07 | 3.60e-04 / 1.03e-04 / 4.58e-05 | 3.03e-08 |
| original layer 0 mlp1, 32 tokens | 1 / 4 | 7.30e-07 / 3.19e-07 | 3.60e-04 / 1.03e-04 | — |
| original layer 0 mlp2, 1 token | 1 / 4 / 12 | 6.93e-07 / 2.55e-07 / 2.47e-07 | 3.60e-04 / 1.03e-04 / 4.58e-05 | 2.62e-08 |

Chapter 66's rule held for all 23,040 (mlp1) and 11,520 (mlp2) one-token
outputs. The tensor-core profile is about ten times less accurate than the
f32 profile, and more than a hundred times inside its own bound.

## Speed

Mean of 20 launches after one warm-up, on one stream, measured with CUDA
events. The four selected experts of the first projection are 133 MB of
packed coefficients, more than the 4080's 64 MB L2; the second projection's
66 MB is about the size of L2, so repeated launches may find some of it
cached, for both profiles alike.

| Case | gpt-oss-cuda-f32-v1 | MXFP4-BF16-MMA-v1 | Ratio |
| --- | --- | --- | --- |
| original mlp1, 1 token: product + finish | 0.197 ms | 0.195 ms (12 slices, with split) | 1.01x |
| original mlp2, 1 token: product + finish | 0.103 ms | 0.110 ms (4 slices, with split) | 0.94x |
| original mlp1, 32 tokens per expert: product kernel only | 1.794 ms (`gk_batch_mx_part`) | 0.292 ms (4 slices) | 6.14x |
| original mlp1, 32 tokens per expert: MMA split + product + finish | — | 0.327 ms | — |
| synthetic mlp1, 1 token | 0.197 ms | 0.195 ms | 1.01x |
| synthetic mlp1, 32 tokens per expert, product only | 1.783 ms | 0.292 ms | 6.10x |

One-token products read every coefficient once; both profiles run at about
680 GB/s of the card's 717 GB/s, so tensor cores do not change decode. With
32 tokens per expert each coefficient is reused 32 times and the tensor-core
product kernel is about six times faster. The batched f32 time excludes its
lane reduction, so the comparison favours it slightly.

## Limits

- No full-model run uses this profile. Expert routes and top tokens on
  chapter 66's fixture prompts, the retained original vectors of chapter 64
  and the selected-expert time inside a full decode or prefill step are not
  measured here.
- One host and one driver.
