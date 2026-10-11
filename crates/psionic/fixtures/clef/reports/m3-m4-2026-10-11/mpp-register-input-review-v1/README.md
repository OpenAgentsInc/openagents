# MPP precision and register-input review

This is a source-only review. It introduces no kernel, benchmark, or product change. The scheduled interleaved gate/up experiment remains unchanged.

## Precision finding

The current [Apple Metal Shading Language specification](https://developer.apple.com/metal/Metal-Shading-Language-Specification.pdf), version 4.1 dated 2026-06-04, section 7.2.1, Table 7.4 on printed page 354, describes relaxed precision as permission to truncate the mantissa of float values before multiplication. It does not specify reduced half-input precision, reduced accumulator precision, a mantissa width, a throughput improvement, or a slower half-input path when disabled.

The installed SDK header defaults the flag to false. Its implementation forwards the descriptor to externally defined functions specialized for operand and output types, including half×half→float; it exposes no implementation branch proving a different lowering for that combination. Header filenames, hashes, and relevant lines are in `sources.json`.

**Conclusion:** enabling the flag is locally untested, but the primary documentation gives no current reason to expect a faster half×half→float operation. Keep this lead low priority, with no new probe currently planned and no 2× claim. A float output alone does not establish that the multiplicands can be truncated. If this is revisited, preserve the same independent f64, f32 absolute 0.001, and end-to-end raw-logit/top-answer gates.

`prior-descriptors.json` records 45 descriptor occurrences across 27 retained Metal source files in nine projection experiment folders. Every occurrence explicitly sets relaxed precision to false. This is an inventory of those files, not a claim about every historical or third-party experiment.

MLX's [generic register MMA helper](https://github.com/ml-explore/mlx/blob/06eb7483f03bca49a80e596f4089d02a507a2cf9/mlx/backend/metal/kernels/steel/gemm/nax.h#L371) sets the flag to true. Its [host selection](https://github.com/ml-explore/mlx/blob/06eb7483f03bca49a80e596f4089d02a507a2cf9/mlx/backend/metal/matmul.cpp#L964) also excludes float32 from the NAX path when TF32 is disabled. These choices fit the documented float-input use; neither proves a half-input speed difference.

## Distinct register-input mechanism

Our rejected independent-SIMD probe used four 32×32 output tiles. Each SIMD group passed device tensor views directly to one 32×32×128 MPP operation per K128 segment and synchronized threadgroups after each segment. Its destination was a persistent f32 cooperative tensor.

MLX's [GEMM loop](https://github.com/ml-explore/mlx/blob/06eb7483f03bca49a80e596f4089d02a507a2cf9/mlx/backend/metal/kernels/steel/gemm/gemm_nax.h#L24) explicitly loads half input fragments into per-thread storage in K32 steps before invoking register-operand operations. Its [MMA helper](https://github.com/ml-explore/mlx/blob/06eb7483f03bca49a80e596f4089d02a507a2cf9/mlx/backend/metal/kernels/steel/gemm/nax.h#L371) passes cooperative inputs and f32 destination through 16×32×16 primitives. The unrolled fragment loop reuses a loaded weight fragment across output-row fragments. This source structure was not tested by our direct-device 32×32×128 probe.

For a 32×32 SIMD output tile and K32 input block, logical source storage is 32 half values for X, 32 for weights, and32 f32 accumulator values per lane, excluding temporary cooperative operands and compiler overhead. This is not measured register allocation. Four 16×32×16 primitive calls cover that block. Any benefit depends on compiler lowering, and the existing MPP device-input operation might already perform equivalent internal reuse.

MLX's [regular kernel](https://github.com/ml-explore/mlx/blob/06eb7483f03bca49a80e596f4089d02a507a2cf9/mlx/backend/metal/kernels/steel/gemm/kernels/steel_gemm_fused_nax.h#L150) fixes the input load step at32, independently of its threadgroup synchronization interval. The [host selector](https://github.com/ml-explore/mlx/blob/06eb7483f03bca49a80e596f4089d02a507a2cf9/mlx/backend/metal/matmul.cpp#L207) chooses intervals 64,256, or512 for different shapes and device classes. Therefore our one K128 synchronization experiment does not cover the complete MLX organization. The host may instead route large-K shapes to split-K; the regular-kernel description must not be presented as every shape's selected path.

The justified remaining hypothesis is explicit cooperative input loading and reuse, separate from relaxed precision. It is unmeasured. A later single-factor implementation would need to preserve f32 output, nonzero residual semantics, exact half input bits, bounded tails, immutable weights, and existing numerical gates. Smaller primitive steps change reduction order, and explicit loads may increase register pressure or spill. No implementation or execution is authorized by this note.

## Retention

`mlx-source.json` pins public MLX source to commit `06eb7483f03bca49a80e596f4089d02a507a2cf9`. The small source snapshots retain their copyright notices and the MIT license in `mlx-LICENSE`. They are research references; no source was copied into product code. The Apple PDF and extracted text remain local research material; do not include either full document in a publication bundle. Publish this note and the source hashes/URLs instead.
