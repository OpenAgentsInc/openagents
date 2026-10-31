# Qwen3-VL vision layout and rotary components

This is a bounded CPU F32 component for the selected Cosmos-Reason2-2B
backbone. It does not execute a vision transformer or establish GR00T/VLA
support. It assumes typed, already resized and normalized RGB input; image
admission, decoding, smart resizing, normalization and camera selection remain
caller work.

The independently authored Coax implementation follows Transformers v4.57.3,
commit `47b0e478f324b54f177ea7998a0791870fdd0324`, inspected without executing its
scripts:

- [Qwen2-VL image packing](https://github.com/huggingface/transformers/blob/47b0e478f324b54f177ea7998a0791870fdd0324/src/transformers/models/qwen2_vl/image_processing_qwen2_vl.py): SHA-256 `7820a0fcca107e75605e08d9b774285ca2b0316f857bc0225c779794705ecf4f`.
- [Qwen3-VL vision/RoPE/merge code](https://github.com/huggingface/transformers/blob/47b0e478f324b54f177ea7998a0791870fdd0324/src/transformers/models/qwen3_vl/modeling_qwen3_vl.py): SHA-256 `685b10cbc48556343df4e159199c5884d02db9ad227640f0f4fbe75ddf0b2a99`.

The authorized model config is Cosmos-Reason2-2B revision
`9ce19a195e423419c349abfc86fd07178b230561`: vision hidden size 1024, 16 heads,
24 blocks, temporal patch 2, spatial patch 16, merge 2, learned position table
48 by 48 and deepstack visual indices 5, 11, 17. These are different from the
text tower's 128-wide, multimodal rotary heads.

## Contracts

`gv_plan` admits one clip of 1–8 frames, height/width 32–512 pixels and multiples
of 32. Input is contiguous `[frame, channel, y, x]`, with exactly three RGB
channels. Missing final temporal frames are repeated. Token order is temporal
group, block row, block column, intra-block row, intra-block column. Each
1536-value patch has `[channel, temporal, y16, x16]` order. Separate cameras must
be packed independently rather than being mislabeled as neighboring times.

`gv_pack` validates all finite input before copying; shape/capacity/alias/number
refusal preserves destination. Its vectors match original Conv3d weights
`[output, channel, temporal, y16, x16]` flattened to output-major matrices.
Those raw flattened convolution weights can use the existing `gl_linear`;
category-specific `gl_pack` transposition is unnecessary for that layout.

`gv_position` returns full-resolution 2D patch coordinates in this blocked order.
`gv_rope` rotates a token/head/64 tensor: 16 height frequencies and 16 width
frequencies form its first half, and rotate-half combines coordinates 32 apart.
Theta is 10000. Frequency powers, reciprocals, phases, sine/cosine and products
are stored as F32; products round before their final F32 sum/difference. Native
libm/transcendental differences use the predeclared oracle tolerance
`2e-5 + 1e-5 * abs(reference)`. This is not a claim of bit identity with default
BF16 Transformers execution. Caller-owned scratch stages the entire output
before publication, including arithmetic failure. Writable spans must be
disjoint from each other and all read-only inputs; the overlap flag must be
truthful, and callers retain valid extent/lifetime/exclusive-access duties.

`gv_posmix` returns four indices and F32 bilinear coefficients for the learned
48-by-48 position table. It does not read or evaluate that learned table.
`gv_merge_layout` describes a contiguous four-patch reshape. Ordinary merger
normalization precedes concatenation (1024 wide); deepstack normalization follows
it (4096 wide). The learned merger MLPs are not implemented by this layout
primitive. In the pinned source their GELU is the default exact-erf variant,
while the vision blocks use the configured tanh approximation.
`gv_deepstack` maps visual blocks 5/11/17 to slots 0/1/2, which the text model
injects at its layers 0/1/2.

## Qualification

The changed authority root killed **296/296 focused mutants**, zero survivors
and no equivalent waivers. The independent C oracle passed **65,192 cases each**
in native debug, release and ASan/UBSan. It enumerates every packed pixel for a
single-frame nonsquare image, an odd three-frame clip and a four-frame clip;
checks blocked positions, complete multi-head RoPE, learned-position indices
and weights, merge/deepstack metadata and shape boundaries; and verifies that
nonfinite, capacity, alias and late arithmetic refusal cannot publish output.
It uses independently nested C indexing and standard C `powf`/`cosf`/`sinf`,
not the Coax planner to generate expected values.

```shell
coax/bin/coax test psionic/lib/robotics/groot-vision.coax
coax/bin/coax mutate psionic/lib/robotics/groot-vision.coax --root-only --jobs 4
psionic/tests/robotics/check-groot-vision.sh /tmp/groot-vision-native
```

No original vision weights, GPU run, image decode, learned patch/position
projection, transformer block, learned merger, text mRoPE or complete model
execution is part of these checks. Previously passed unchanged numerical and
state-encoder mutation roots were not repeated.
