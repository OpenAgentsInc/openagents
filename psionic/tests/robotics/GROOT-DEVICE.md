# GR00T F32 device primitives

`psionic/lib/robotics/groot-device-kernels.coax` contains genuine Coax-authored
Metal compute kernels for private inference stages. The existing Metal host
performs compilation, allocation and dispatch effects. There is no handwritten
Metal arithmetic or CPU fallback in the device exercise.

The profile is `groot-device-f32-v1`: F32 storage, products and explicit stages,
corrected F32 TwoSum reductions, safe Metal math, and source-specific epsilon
supplied by the caller. It is distinct from the CPU wide-reduction reference
and NVIDIA's source-default BF16 execution. It does not establish complete
original-model inference or physical action suitability.

| Kernel | Work item and layout |
| --- | --- |
| `gdk_norm` | One row. Kind 0 plain LayerNorm, 1 affine LayerNorm, 2 learned RMSNorm. Scale/bias vectors broadcast across rows. |
| `gdk_modulate` | One value. AdaLN order 0 `[scale,shift]`, order 1 `[shift,scale]`; multiplication by `1+scale` then shift. |
| `gdk_activate` | One value. Kind 0 SiLU, 1 tanh-approximate GELU. Exact GELU is separate. |
| `gdk_qkv_split` | One token/channel. `[token,3,width]` into three `[token,width]` views. |
| `gdk_scores` | One `[query,head,key]` score. Q is `[token,head,dim]`; K is `[token,kv_head,dim]`; contiguous query-head groups share a KV head. |
| `gdk_softmax` | One query/head row; a separate byte keep-mask distinguishes excluded keys. Empty rows produce zero probabilities. |
| `gdk_context` | One `[query,head,dim]` value from F32 probabilities and V `[token,kv_head,dim]`. |

Attention admits 1–1024 queries/keys, 1–32 query heads, dividing KV-head counts
and dimensions 1–128. The source head widths 48, 64 and 128, equal-head layouts
and 16/8 grouped-query mapping are exercised. Mask mode bits are 1 shared key
validity, 2 causal prefix with explicit query offset, and 4 equal camera groups.
Camera masking requires equal query/key lengths. Norm widths are 1–8192,
with at most 16,384 independent rows. These bounds describe operator admission;
the caller separately budgets real allocations and model workspace.

Every destination is private. The caller poisons each stage's disjoint flags
before dispatch, waits successfully, then requires **every expected flag to
equal zero** before any public publication. Metadata refusal returns without
writes, preserving poison. Numeric receipts count completed finite operand and
result checks. Norm flags are per row; score flags per score; softmax flags per
query/head; other flags per output element. Existing outputs must never be used
as proof a new stage completed. Typed allocation extents, source identity,
disjoint ownership, dispatch order and eventual publication belong to the
integrating Coax planner/engine.

Checks were written first. Qualification on 2026-10-31 passed **13 Coax checks**,
including **1,152 exhaustive mask combinations**, and killed **605/605 focused
root mutants**, with no waivers. Native debug/release/ASan+UBSan each passed
54 component fixtures and 300,143 finite output values, plus native nonfinite
refusal and ABI checks. The real **Apple M5 Max** ran the same 54 fixtures in
Metal. Independently authored pinned Torch 2.9.0 equations compared all four
backends: **216 comparisons, 1,200,572 values**. Frozen before observation:
absolute and relative tolerance each **2e-5**. The largest observed error used
**0.018882** of its allowed budget.

```shell
coax/bin/coax mutate psionic/lib/robotics/groot-device-kernels.coax --root-only --jobs 4
psionic/tests/robotics/check-groot-device.sh /tmp/groot-device-native
```

The device gate requires macOS Metal and the already provisioned private Torch
reference environment, or `GROOT_REFERENCE_PYTHON` naming the pinned interpreter.
It fails if the GPU is absent. It never installs packages or runs downloaded
repository scripts. The gate emits `cases.jsonl`, exact fixture inputs, separate
CPU/Metal outputs and `reference.json` containing all metrics and source hashes.
Existing passing gates on unchanged source were not repeated.

Qualified kernel SHA-256:
`b04fd9f25f40cea80cce975ef5b1d8e4bf7a4ce0582b94c7d0d8f935207587af`.
