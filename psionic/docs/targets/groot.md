# GR00T: exact LIBERO target and source contract

Source audit and execution status, 2026-10-31.
[Chapter 168](../../book/ch168-execute-a-groot-policy.md) now executes this
original learned VLA on Metal for one prepared 140-token observation. Its
[composed receipt](../../fixtures/robotics/groot-libero/composed-policy-qualification.json)
qualifies same-input components, unchanged original action endpoints and the
16×7 adapter. The separately recorded global hidden-trace diagnostic remains
failed; source-default BF16 identity and arbitrary request lengths are not
claimed. Chapter 169 still owns the matching simulated control loop. Chapter
167's gantry and SingleArm excerpt remain distinct embodiments/profiles.

## Selected artifact and reference

Use the separately released
[`nvidia/GR00T-N1.7-LIBERO`](https://huggingface.co/nvidia/GR00T-N1.7-LIBERO/tree/2ea293aa20ba7cf5bbf3ba17a5fbcb1a01cbfe21/libero_10)
at revision `2ea293aa20ba7cf5bbf3ba17a5fbcb1a01cbfe21`, subfolder
`libero_10`, with embodiment `libero_sim` / `LIBERO_PANDA`, category **2**.
The [metadata fixture](../../fixtures/robotics/groot-libero/README.md) pins
its original small files, full header inventory and derived embodiment contract.

The official reference source is
[`NVIDIA/Isaac-GR00T`](https://github.com/NVIDIA/Isaac-GR00T/tree/c56f54128d92f743889b7e4cecfc1ceafd88739e)
at `c56f54128d92f743889b7e4cecfc1ceafd88739e`, inspected locally at
`~/work/projects/repos/Isaac-GR00T`. No repository scripts or submodules were
executed. Relevant source paths are:

| Component | Official reference path |
| --- | --- |
| Model/action head | `gr00t/model/gr00t_n1d7/gr00t_n1d7.py` |
| Qwen backbone | `gr00t/model/modules/qwen3_backbone.py` |
| DiT, alternating masks, AdaLN | `gr00t/model/modules/dit.py` |
| Category-specific state/action MLPs | `gr00t/model/modules/embodiment_conditioned_mlp.py` |
| Inference preprocessing and decoding | `gr00t/model/gr00t_n1d7/processing_gr00t_n1d7.py` |
| Normalization/relative transforms | `gr00t/data/state_action/state_action_processor.py`, `gr00t/data/utils.py` |
| Deterministic image transforms | `gr00t/model/gr00t_n1d7/image_augmentations.py` |
| LIBERO observation/action adapter | `gr00t/eval/sim/LIBERO/libero_env.py` |
| Policy API and benchmark setup | `getting_started/policy.md`, `examples/LIBERO/README.md` |

Its dependency declarations pin torch 2.9.0, torchvision 0.24.0,
transformers 4.57.3, diffusers 0.35.1, albumentations 1.4.18 and numpy 1.26.4.
These identify a development reference, not dependencies for a shipped Coax
runtime. The LIBERO submodule gitlink is
`8f1084e3132a39270c3a13ebe37270a43ece2a01`; simulator, assets and task/init-state
versions need their own explicit reference qualification for chapter 169.

The base `GR00T-N1.7-3B@2fc962b973bccdd5d8ce4f67cc63b264d6886495`
does **not** supply this LIBERO processor profile. Its processor supports eight
pretrain embodiment tags. Merely finding `libero_sim: 2` in its ID map does not
make it support Panda/LIBERO. The official policy documentation requires a
fine-tuned checkpoint for posttrain tags. Chapter 167's joint-action SingleArm
data is also a different embodiment and cannot be relabeled as LIBERO input.

## License evidence stays attached to each artifact

The selected LIBERO card identifies the
[NVIDIA Open Model License Agreement](https://www.nvidia.com/en-us/agreements/enterprise-software/nvidia-open-model-license/).
That agreement permits commercial use subject to its terms. The observed
selected repository tree contains no separate conflicting `LICENSE` file;
the fixture retains the dated agreement PDF and an attribution notice.
Original source code in Isaac-GR00T carries Apache-2.0 headers. These are
separate observations about model and reference-code terms.

The original base artifact remains conditional: its included
[`LICENSE` §3.3](https://huggingface.co/nvidia/GR00T-N1.7-3B/blob/2fc962b973bccdd5d8ce4f67cc63b264d6886495/LICENSE)
limits use to noncommercial research/evaluation while its card describes
commercial availability. This audit does not resolve that conflict by applying
another repository's card or the code's Apache license to the base weights.
N1.5 and N1.6 base snapshots inspected here carry the same limiting clause.

Another official candidate,
[`nvidia/gr00t17-lerobot-libero_10-640@5ee08ab09fac5c5ef2388a14c882ea825ac861db`](https://huggingface.co/nvidia/gr00t17-lerobot-libero_10-640/tree/5ee08ab09fac5c5ef2388a14c882ea825ac861db),
has an explicit Apache-2.0 card annotation and a 12.576 GB model file. It is a
different LeRobot package with separate preprocessing sidecars and a config
that still names the base checkpoint. It is not substituted into this target.

Cosmos is a separate dependency whose exact assets and applicable notices must
be retained. The selected model's broader terms do not authenticate or replace
missing dependency files. The Open Model Agreement also has redistribution
and Cosmos attribution conditions; the retained agreement is the source of
those conditions, rather than a new generic permission claim here.

## Complete header inventory

The range audit fetched only the two eight-byte header lengths and their JSON
headers, then compared every tensor with the original index. All 1,031 tensors
are BF16. Every tensor's shape product times two equals its byte span; sorted
spans cover each payload contiguously without overlap or gaps. Names and file
mapping match the index exactly.

| Original file | Header JSON | Complete file bytes | Advertised full SHA-256 |
| --- | --- | --- | --- |
| `model-00001-of-00002.safetensors` | 103,352 B | 4,990,519,232 | `39b7beaadd9a06c87502f2b741a36f939b84941f37fef757c6177e54e34a5eff` |
| `model-00002-of-00002.safetensors` | 34,192 B | 1,919,980,184 | `8c4fa56f1b4f25a1a842c811e8a12d2bf6d77d73942f9a04f5b876a11bd6d703` |

Payload totals are **6,910,361,856 bytes**; complete files total
**6,910,499,416 bytes**. The stored shapes contain 3,455,180,928 elements.
The index's `total_parameters` is 3,144,016,000: the difference is exactly one
`[151936, 2048]` matrix. Both token embedding and `lm_head` are serialized;
this count observation does not prove their payloads equal or authorize
unqualified deduplication. Logical parameters, serialized storage and live
execution memory need separate accounting.

| Group | Tensors | BF16 bytes |
| --- | --- | --- |
| Backbone | 494 | 3,669,329,920 |
| Action head | 537 | 3,241,031,936 |

The [inventory JSON](../../fixtures/robotics/groot-libero/tensor-inventory.json)
lists every shape and absolute file range. Whole-file hashes above are
advertised LFS metadata, not a claim that this header audit downloaded or
verified the complete payload. The four privately captured category-2 state
encoder components have their own
[capture hashes](../../fixtures/robotics/groot-libero/category2-capture.json).

## Execute the actual network

The checkpoint's actual configuration and tensor names define a Qwen3-VL /
Cosmos-Reason2 backbone, not the card's older SigLip2/T5 prose. The pinned
checkpoint also differs from current source defaults: it selects **16**
language layers and **32** DiT layers, rather than adopting the source's
12/16-layer defaults.

- Vision: 24 blocks at width 1024; a `[1024, 3, 2, 16, 16]` Conv3d patch
  embedding, learned spatial positions, visual RoPE and three deep-stack
  mergers. Patch order, interpolation and deep-stack injection are part of
  the model, not optional image formatting.
- Language: 16 retained Qwen3-VL blocks, width 2048, MLP width 6144,
  128-wide query/key normalization, Q projection width 2048 and KV width 1024.
  Exact Qwen multimodal positions, tokenizer and masks are required.
- VL conditioning: layer norm plus four width-2048 self-attention blocks.
- State: category-indexed `132 → 1024 → 1536` MLP with ReLU between layers.
- Actions: category-indexed `132 → 1536` encoding, concatenated sinusoidal
  timestep features, `3072 → 1536 → 1536` projections with swish, plus learned
  positions. Category-specific weights use `[category, input, output]` layout.
- Flow transformer: 32 width-1536 blocks, 32 heads of width 48, AdaLN and
  approximate-GELU FFNs. Odd blocks self-attend; even blocks cross-attend.
  With `attend_text_every_n_blocks=2`, cross blocks alternate non-image and
  image token masks. The output projection has its own time-conditioned norm.
- Decoder: category-indexed `1024 → 1024 → 132` MLP with ReLU.

Inference starts from standard Gaussian noise `[1, 40, 132]`. The four Euler
flow steps use bucket times **0, 250, 500, 750**, with `dt=1/4`, repeatedly
embedding the current actions, running the complete DiT and decoding predicted
velocity. Record the actual initial noise for parity: matching a nominal RNG
seed across different runtimes does not establish identical random samples.
The model's BF16 conversion/rounding boundaries need explicit numerical
qualification; an independently useful F32 component is not yet full BF16
VLA parity.

The category-2 state encoder is a first bounded original-weight component:

| Tensor | Category shape | Absolute file-2 offset | Bytes |
| --- | --- | --- | --- |
| `action_head.state_encoder.layer1.W` | `[132,1024]` | 1,408,168,600 | 270,336 |
| `action_head.state_encoder.layer1.b` | `[1024]` | 1,416,282,776 | 2,048 |
| `action_head.state_encoder.layer2.W` | `[1024,1536]` | 1,422,635,672 | 3,145,728 |
| `action_head.state_encoder.layer2.b` | `[1536]` | 1,517,013,656 | 3,072 |

These are individual category slices within full `[32, ...]` tensors. Selecting
one embodiment does not eliminate the backbone or flow transformer from the
complete model path.

## LIBERO observation, normalization and action contract

There are **eight state coordinates**, despite seven named groups:
EEF xyz (3), quaternion-derived axis-angle (3), and two finger positions (2).
The source calls its rotation fields `roll`, `pitch`, `yaw`, but its actual
adapter uses exponential axis-angle coordinates, not Euler angles. The two
RGB camera streams are `image` and `wrist_image`; the benchmark reverses both
spatial axes before policy input. It does not reverse RGB channels.

For this selected profile:

1. Use its own `libero_sim` q01/q99 statistics, in declared group order.
   Normalize to `[-1,1]` and clip outliers; mean/std is disabled. The original
   helper assigns the intermediate `(x-q01)/(q99-q01)` into `zeros_like(input)`
   before a second `2*x-1` assignment, so input-dtype rounding matters.
2. Pad the eight normalized state coordinates with zeros to width 132.
   No sin/cos state expansion is configured. Category is exactly 2.
3. In evaluation, albumentations uses smallest-edge 256, fractional centre
   crop 0.95, then smallest-edge 256, with OpenCV `INTER_AREA`. On a 256-square
   image the crop is **243 × 243**, at `(6,6)`, then resized to 256. The
   `image_crop_size=230` fallback is not selected while `crop_fraction` is set.
   The gated Qwen processor performs additional exact processing afterward.
4. Formalized language is lowercased and stripped of non-word/non-whitespace
   punctuation. Camera images precede the instruction in a user message;
   the Qwen chat template uses `add_generation_prompt=false` and left padding.
5. The model predicts 40 × 132 actions. The embodiment processor selects the
   first **16** rows and seven meaningful channels, then clips and reverses
   q01/q99 scaling. LIBERO's action configs skip relative-to-state conversion;
   their `ABSOLUTE/NON_EEF` labels do not turn simulator commands into absolute
   Cartesian world poses. Legacy strict padding-mask flags are absent/false.
6. The environment adapter transforms the last channel with
   `-sign(2*g - 1)`, including zero at `g=0.5`. The reference rollout replans
   after **eight** executed actions. Model prediction, decoded chunk and
   execution horizon are different bounds and cannot be interchanged.

The exact action controller scale, episode reset state, rendering, observation
timing and success predicate must remain tied to the selected LIBERO reference
for chapter 169. Chapter 167's fixed `0.1 s`/`0.5 m/s` Cartesian profile is not
a substitute for those conventions.

The bounded Coax adapter now lives in
[`groot-embodiment.coax`](../../lib/robotics/groot-embodiment.coax).
`ge_normalize` accepts exactly eight **already axis-angle** F32 state values,
uses the original fixed statistics and writes 132 values with zero padding.
`ge_decode` accepts exactly `40 × 132` F32 model values, consumes only the first
`16 × 7`, applies the original inverse statistics, rounds to F32 at the policy
boundary, then applies the LIBERO gripper wrapper. The gripper tie preserves
**negative zero**. Quaternion conversion and the simulator controller remain
separate prerequisites.

Both exports validate shape, caller-declared overlap and consumed finite
numbers before publishing any output. Successful calls preserve output tails;
refusals preserve the entire output. `GeRead.clipped` counts consumed
coordinates strictly outside the normalization bounds, distinguishing true
outliers from exact endpoints. Unused model channels and rows are deliberately
ignored, including their nonfinite values. This adapter authenticates no
checkpoint and supplies no learned policy by itself.

Its qualification is eight authored Coax checks, 85/85 killed focused mutants,
67,501 independent native cases in each of debug, release and ASan/UBSan, and
an independent NumPy reference reading the hash-pinned original statistics:
298 state vectors, 265 action chunks and 71,831 exact F32 word comparisons.
Those synthetic conversion vectors establish arithmetic/admission behavior,
not original learned model performance. Python is optional development tooling;
ordinary native checks and the runtime need no Python or model framework.

```shell
coax/bin/coax test psionic/lib/robotics/groot-embodiment.coax
coax/bin/coax mutate psionic/lib/robotics/groot-embodiment.coax --root-only --jobs 4
psionic/tests/robotics/check-groot-embodiment.sh /tmp/groot-embodiment-check
```

## External assets and qualification sequence

The checkpoint does not contain the Cosmos-Reason2 configuration, tokenizer or
Qwen image-processor assets. Its reference loader fetches
`nvidia/Cosmos-Reason2-2B` separately, pinned here at
`9ce19a195e423419c349abfc86fd07178b230561`. The initial unauthenticated
`config.json` request returned **401** on 2026-10-31. The owner subsequently
accepted the terms and authenticated; that access prerequisite is **resolved**.
Authorized original assets now reside in the private Hugging Face cache.
Their names, sizes and exact hashes are recorded in
[`cosmos-dependency.json`](../../fixtures/robotics/groot-libero/cosmos-dependency.json),
which contains metadata only. Gated asset payloads are not redistributed here.
The earlier header-only provenance record remains an immutable observation of
the original audit, rather than being rewritten as later execution evidence.

The image VLA path requires `config.json`, `tokenizer.json`,
`tokenizer_config.json` and `preprocessor_config.json`. The original
`generation_config.json` is separately pinned but text generation is not used
for the selected action path. The tokenizer JSON embeds its BPE vocabulary and
merges; separate optional vocab/merges sidecars are not an additional requirement
for that fast-tokenizer representation.

The dependency config describes Qwen3-VL: 28 source language layers, of which
the selected GR00T checkpoint retains 16; hidden size 2048, 16 query and eight
KV heads, head dimension 128, RMS epsilon `1e-6`, RoPE theta 5,000,000 and
interleaved multimodal sections `[24,20,20]`. Vision has 24 width-1024 blocks,
16 heads, patch size 16, temporal patch size 2, spatial merge size 2, learned
2,304-position embeddings and deep-stack outputs at blocks 5, 11 and 17.

Its actual tokenizer is NFC-normalized BPE with 151,643 base entries, 151,387
merges and 26 added tokens. It uses the pinned regex split followed by byte-level
encoding, no automatic BOS, `<|im_end|>` EOS and `<|endoftext|>` padding. The
reference forces left padding. The selected user chat lists camera images
before the instruction and adds no assistant generation prompt. This cannot be
replaced with a superficially similar GPT-OSS or generic Qwen tokenizer.

The pinned
[Transformers fast image processor](https://github.com/huggingface/transformers/blob/v4.57.3/src/transformers/models/qwen2_vl/image_processing_qwen2_vl_fast.py)
uses resize factor 32 and **pixel budgets** 65,536 through 16,777,216. These are
not literal edge lengths. A selected 256-square image stays 256-square, is
temporally duplicated into one two-frame patch group and yields grid
`[1,16,16]`: 256 raw patches of 1,536 values. Each camera therefore produces
64 merged visual tokens. Patch flattening groups neighboring merge cells before
channel, temporal and within-patch coordinates. The
[Qwen3 processor](https://github.com/huggingface/transformers/blob/v4.57.3/src/transformers/models/qwen3_vl/processing_qwen3_vl.py)
expands each image marker to precisely that many image-pad tokens.

The [fast normalization implementation](https://github.com/huggingface/transformers/blob/v4.57.3/src/transformers/image_processing_utils_fast.py)
folds the reciprocal rescale factor into mean and standard deviation. With the
original `.5` means/stds and `1/255` rescale factor, its F32 arithmetic is
`(pixel - 127.5) / 127.5`. Separately rounded `pixel / 255` preprocessing can
differ. Image resize, crop, patch order, unrestricted Unicode language and the
full BF16 network need their own executed qualification; cached files and
descriptive metadata do not establish that the full VLA is implemented. The
bounded 256-square pixel path below has its own executed qualification.

## Exact selected camera preprocessing

[`groot-image.coax`](../../lib/robotics/groot-image.coax) accepts one raw
simulator camera as **256 × 256 HWC RGB u8, before the LIBERO wrapper flip**.
`gi_preprocess(0,256,...)` writes the full normalized NCHW F32 camera for
`gv_pack`. Other admitted `first_row,rows` arguments compute genuine slices of
that same source camera in `[3,rows,256]` layout. The full call remains atomic;
VM checks use 23 source rows to stay within their step budget, while native
and original OpenCV comparisons cover complete images.

The exact recipe reverses both spatial axes, keeps RGB order, crops the
243-square region `[6:249,6:249]`, resizes to 256 with `INTER_AREA`, then applies
the fused F32 normalization above. The first smallest-edge-256 transform is an
identity at this admitted size. Crop fraction `.95` scales each **edge** before
integer truncation; it is not square-rooted as an area fraction. The source
augmentation path keeps this evaluation image as u8 until the Qwen processor.

The [OpenCV4.11 resize path](https://github.com/opencv/opencv/blob/4.11.0/modules/imgproc/src/resize.cpp)
uses special area-mode coordinates and 11-bit coefficients when upsampling
this u8 image. Its vertical uchar specialization rounds separate high-word
products before the final two-bit rounding. Generic floating bilinear
interpolation or a single rounded 22-bit weighted sum can yield different
pixels. Coax implements that exact bounded arithmetic without linking OpenCV.

The [image qualification receipt](../../fixtures/robotics/groot-libero/image-qualification.json)
pins six source files and eight deterministic input/reference hashes. Six
authored checks passed; all **101 focused mutants** were killed. Independent
native debug, release and ASan/UBSan each passed **2,082,874 cases**. Original
OpenCV4.11.0/NumPy1.26.4 comparisons covered eight full cameras and **1,572,864
exact F32 words**. A separate installed Torch2.9.0/torchvision normalization
comparison covered every u8 value in all three channels, 768 exact F32 words.
These are synthetic pixel probes, not evidence of a learned robotics outcome.
The runtime and ordinary native checks need no OpenCV, NumPy or Torch.

```shell
coax/bin/coax test psionic/lib/robotics/groot-image.coax
coax/bin/coax mutate psionic/lib/robotics/groot-image.coax --root-only --jobs 4
psionic/tests/robotics/check-groot-image.sh /tmp/groot-image-check
```

## Bounded original-tokenizer request preparation

[`groot-request.coax`](../../lib/robotics/groot-request.coax) now implements the
selected two-camera chat and position contract. Call `gr_render`, then existing
Coax `pt_encode_checked` with the authenticated **original Cosmos** table,
then `gr_plan`. The private derived table is 5,100,329 bytes, SHA-256
`f856fb8ef64650f2841a3ec36792d9c52ada0ead2b44d04cb120c9be6cc2d73d`.
Its [request contract](../../fixtures/robotics/groot-libero/request-contract.json)
records the original source hash, deterministic packing recipe and independent
original-tokenizer observations. Neither the gated vocabulary nor its derived
table is redistributed. Callers must authenticate the immutable table before
using it; PST header admission alone does not establish its identity.

Current language admission is **ASCII, at most 4,096 bytes**. In this domain,
lowercasing and deleting non-word/non-whitespace characters exactly reproduce
the original Python formalization, including underscores and Python's ASCII
whitespace controls. Non-ASCII is explicitly unsupported until Unicode case
mapping and Python word classes are independently qualified. Tokenization keeps
the existing bounded per-piece and work budgets. Those limits refuse requests;
they never truncate or replace text silently.

For `open the drawer`, rendering produces 1,767 bytes and the actual original
tokenizer produces 140 tokens. The prefix is `[151644,872,198,151652]`;
64 image-pad IDs occupy `[4,68)` and `[70,134)`. Instruction IDs are
`[2508,279,26482]`; the suffix is `[151645,198]`. These are observed original
IDs, not a guessed token-range substitution.

The original processor uses **left padding to the longest request in a batch**
with `padding=True`; it specifies no `max_length`, truncation or fixed 512-token
padding. The current Coax profile admits **137 through 512 tokens** as its own
resource bound. `pad_to=0` preserves an unpadded single observation; an explicit
admitted batch length left-pads with ID 151643. `gr_plan` writes six contiguous
integer arrays: IDs, attention mask, image mask and temporal/height/width
positions. The last three arrays reproduce the original
[Qwen3-VL position construction](https://github.com/huggingface/transformers/blob/v4.57.3/src/transformers/models/qwen3_vl/modeling_qwen3_vl.py).
Each camera's `8 × 8` merged grid advances the maximum position by eight,
despite occupying 64 sequence slots. The unpadded RoPE delta is therefore
`-112`; padding subtracts the additional left-pad length. Pad positions are
one on all three axes, with attention and image masks zero.

Use the **actual unpadded length** for a single-observation model reference.
The original action head runs its four VL self-attention blocks without a
padding mask before constructing the alternating DiT masks. Artificially padding
one request to 512 can therefore change its valid VL features. Explicit padded
batch metadata must be compared with the same batch shape in the reference;
buffer capacity alone is not a reason to add padding tokens.

Both new exports stage publication and preserve the caller's entire output on
refusal, and the unused tail on success. The actual original private tokenizer
was exercised through the Coax encoder against installed `tokenizers 0.23.1`:
167 requests, 662 unpadded/padded plans and 1,261,412 exact rendered-byte,
token-ID, mask and position comparisons. The original NVIDIA/Transformers
scripts were read, never executed. This qualifies request preparation only;
it does not establish image transforms, BF16 backbone execution, action quality
or a completed VLA chapter.

The focused authority run killed all **259 mutants**, with 21 passing checks
including the unchanged tokenizer/NFC checks. Independent native admission,
grammar, output-canary and position checks passed **441,317 cases** in each of
debug, release and ASan/UBSan, including an actual privately packed original
tokenizer sample. Source hashes are recorded in the
[qualification receipt](../../fixtures/robotics/groot-libero/request-qualification.json).
An additional [SDK receipt](../../fixtures/robotics/groot-libero/request-sdk-qualification.json)
compares Coax and the general-grid integer oracle with installed
Transformers4.57.3's actual `Qwen3VLModel.get_rope_index`: 18 metadata cases and
28,662 exact integer words, including empty/minimum, exact-512 and left-padded
requests. The model was initialized entirely on `meta`; no learned forward
pass or CPU model evaluation was performed for this check.

```shell
coax/bin/coax test psionic/lib/robotics/groot-request.coax
coax/bin/coax mutate psionic/lib/robotics/groot-request.coax --root-only --jobs 4
psionic/tests/robotics/check-groot-request.sh /tmp/groot-request-check
```

The implementation proceeded in explicit slices: manifest/header/range admission;
original category-2 state MLP; action/time encoders and normalization; one
original DiT block; complete four-step action head with recorded VL features;
exact image/tokenizer/Qwen3-VL backbone; then the complete original VLA on a
fixed observation and noise tensor. Each component earned its own original
weight/reference checks. The complete learned path now feeds actual Coax
vision features into Coax text and all four Coax flow steps, then decodes the
actions. An independent head reference consumes the same actual Coax backbone;
the unchanged full-network reference separately checks observable action
endpoints. This does not relabel the failed accumulated global hidden trace.
Chapter 169 requires the matching simulated loop, timing, resets, held-out
outcomes and recovery; the gantry cannot substitute for that qualification.
