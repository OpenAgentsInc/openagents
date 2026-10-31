# GR00T category-specific linear prerequisites

This independent native oracle qualifies the bounded Coax original-layout
transpose, bias decode and two-layer state-encoder arithmetic. Its matrices are
synthetic and asymmetric; it does not establish original-weight GR00T parity or
complete model support. The separate original-weight exercise records its own
artifacts and reference results.

Original category weights have layout `[input, output]`. The oracle checks
byte-preserving transpose to Psionic's `[output, input]` projection layout for
BF16 and F32, including signed zero, then independently calculates each dot,
F32 narrowing, bias addition, another F32 narrowing and the intermediate ReLU.
A rounding witness distinguishes bias after narrowing from a fused wide dot
and bias, for both dtypes. A cancellation witness recovers the unit term in
`2^100 + 1 - 2^100`, which naive F64 summation loses. The two-layer exercise
has both positive and negative pre-ReLU activations and refuses without
publishing output when the second layer fails; private workspace may change.

Native checks also pin the disjoint workspace layout, boundary metadata and
the original state-encoder dimensions `132 → 1024 → 1536` at admission only.
The large-shape metadata test does not allocate or execute those weights.

```shell
COAX=coax/bin/coax psionic/tests/robotics/check-groot-linear.sh /tmp/groot-linear-native
```

The wrapper emits the actual Coax library and runs debug, release and
ASan/UBSan without fast math or FMA. No original model inference, downloaded
script, external simulator or robot hardware runs in this oracle.

Qualification on 2026-10-31: **263 cases passed in each of native debug,
release and ASan/UBSan** against the frozen runtime. Checks include asymmetric
transpose, signed-zero decoding, two-narrow arithmetic, source preservation,
short/negative/extreme metadata, alias refusal, non-finite weights/bias/input,
projection overflow, bias overflow, and second-stage atomic refusal. All
failures report zero written and preserve the public destination. Private
workspace may change after execution begins.

Runtime source SHA-256:
`91fe573f5a18cb3df20d8044e94a179f9baf8a91da3b7f779386edac416aa743`.
