# GR00T DiT reference primitives

`psionic/lib/robotics/groot-dit.coax` implements the CPU F32 reference
normalization, activation, attention and mask operators needed to compose
GR00T's alternating DiT. It accepts already projected Q/K/V and already
computed learned modulation vectors. It does not execute a complete block,
the original checkpoint, a VLA request or a robot controller.

The source is NVIDIA's Apache-2.0
[Isaac-GR00T revision c56f54128d92f743889b7e4cecfc1ceafd88739e](https://github.com/NVIDIA/Isaac-GR00T/blob/c56f54128d92f743889b7e4cecfc1ceafd88739e/gr00t/model/modules/dit.py).
The repository was read as reference; none of its scripts was executed.

## Operators and layout

| API | Contract |
| --- | --- |
| `gdt_norm_admit` / `gdt_norm` | Row-wise plain LayerNorm, affine LayerNorm, block AdaLN or final AdaLN. |
| `gdt_activation_admit` / `gdt_activate` | Element-wise SiLU or tanh-approximate GELU. |
| `gdt_block_route` | Selected 32-block configuration: text, self, image, self, repeated. |
| `gdt_cross_mask` | Boolean intersection of valid backbone tokens with the requested image class. |
| `gdt_attention_plan` / `gdt_attention_admit` / `gdt_attention` | Bidirectional projected attention over `[token,head,dimension]` Q/K/V. |

Plain LayerNorm takes no parameters. Affine LayerNorm takes contiguous
`[gamma,beta]` vectors, each the row width. Block AdaLN takes `[scale,shift]`;
final AdaLN takes `[shift,scale]`. These vectors broadcast across rows. Plain,
affine and block normalization use epsilon 1e-5; final AdaLN uses 1e-6.
AdaLN multiplies the normalized values by `1+scale` before adding shift.
Learned timestep projections that produce these vectors are separate work.

Normalization reuses the existing checked LayerNorm and its 1,048,576-element
bound, with 2N scratch values. Activation reuses that bound and needs N scratch
values. Larger independent row/element batches can be tiled by their caller.
The selected DiT's 1536 channels and VL transformer's 2048 channels are covered.

Attention supports 1–1024 queries and keys, 1–32 heads and dimensions 1–256;
the released 32×48 DiT and 32×64 VL layouts are tested. Its shared key mask is
one byte per key, with values zero or one. It needs `3*keys + queries*heads*dim`
F32 scratch values: scores, probabilities, softmax scratch and staged context.
An all-zero mask gives a zero context before any learned output bias.
This operator has no causal mask, grouped-query head mapping or dropout.
No padding mask is invented for the source VL self-attention, whose caller
passes none. Cross-attention masks distinguish valid text and image tokens.

## Numerical and ownership profile

Storage is F32. LayerNorm and attention reuse compensated F64 CPU reductions;
attention scores and probabilities narrow to F32 before subsequent stages.
Affine multiplication narrows to F32 before the checked addition. SiLU narrows
the stable sigmoid to F32 before multiplication. Tanh GELU evaluates the
analytic approximation in F64 and narrows its result. This is a declared CPU
reference profile, not a claim of PyTorch vector-math, BF16 or device bit parity.
It must not replace exact GELU where a different model component requires it.

Callers provide valid immutable typed input views, disjoint exclusive outputs
and scratch, and truthful overlap metadata. Metadata and number failures leave
public outputs unchanged. Arithmetic failures may modify private scratch and
preserve the public destination. Successful operators write only their stated
output prefix. There is no allocation or implicit shared state.

## Qualification

Checks were written before implementation. The final source passed its
**11 authored Coax checks** plus existing dependency checks, killed
**253/253 focused root mutants** without waivers, and passed **1,658,609
independent native cases in each of debug, release and ASan/UBSan**.

The independently authored C oracle uses long-double equations for all four
normalizations, both activations and masked scaled dot-product attention.
It covers source-width rows, nonuniform learned parameters, all attention
heads/positions, first/last/all/no-key masks, finite extremes, NaNs and
infinities, overflow refusal, public-output atomicity and native ABI bounds.
Compilation disables fast math and contraction. Existing numerical dependencies
were not independently remutated.

```shell
coax/bin/coax test psionic/lib/robotics/groot-dit.coax
coax/bin/coax mutate psionic/lib/robotics/groot-dit.coax --root-only --jobs 4
psionic/tests/robotics/check-groot-dit.sh /tmp/groot-dit-native
```

Qualified source SHA-256:
`5b3ef6ba3049594189b1ac605e7b30ff85fc1b29294619a36697c21e60bfd4d7`.
No original checkpoint payload, external inference service or physical device
participates in this CPU primitive gate.
