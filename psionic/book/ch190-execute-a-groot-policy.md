# 190. Execute a GR00T policy

Coax now executes the selected original GR00T learned network on Metal, from
one admitted prepared observation through a decoded **16 × 7** action chunk.
The complete path includes vision, language, state encoding and all four flow
steps. Its qualification combines same-input component comparisons, unchanged
original full-network action endpoints and exact buffer handoffs. A separate
global hidden-state diagnostic still fails; we keep that result visible.

The controller in [chapter 189](ch189-imitation-before-robotics-scale.md) learned
two velocities from a target error. GR00T takes a richer observation: two
camera images, an instruction and the arm's state. It produces a chunk of
continuous actions. We need to reproduce the exact observation contract and
the complete learned network before judging those actions in a simulated task.

This chapter targets one actual fine-tuned artifact and one embodiment. The
[source contract](../docs/targets/groot.md) records its architecture, licenses,
original metadata and provenance. The
[verification record](../../docs/verification/psionic-ch113-2026-10-31.md)
separates passing components, the composed-policy result and diagnostic failures.
Chapter 191 owns observation timing, resets, controller behavior and
closed-loop task outcomes.

## Choose the policy that actually has the embodiment

The selected checkpoint is
[`nvidia/GR00T-N1.7-LIBERO`](https://huggingface.co/nvidia/GR00T-N1.7-LIBERO/tree/2ea293aa20ba7cf5bbf3ba17a5fbcb1a01cbfe21/libero_10),
revision `2ea293aa20ba7cf5bbf3ba17a5fbcb1a01cbfe21`, subfolder `libero_10`.
Its processor supplies `libero_sim` / `LIBERO_PANDA`, category **2**, with eight
state and seven action coordinates. An embodiment identifier in another
checkpoint's ID map does not establish that its processor or weights support
that embodiment.

| Source | Pin | Role |
| --- | --- | --- |
| GR00T-N1.7-LIBERO | `2ea293aa20ba7cf5bbf3ba17a5fbcb1a01cbfe21/libero_10` | Original learned weights, processor and statistics |
| [Isaac-GR00T](https://github.com/NVIDIA/Isaac-GR00T/tree/c56f54128d92f743889b7e4cecfc1ceafd88739e) | `c56f54128d92f743889b7e4cecfc1ceafd88739e` | Reference architecture, processing and LIBERO adapter |
| Cosmos-Reason2-2B | `9ce19a195e423419c349abfc86fd07178b230561` | Exact Qwen3-VL config, tokenizer and image processor |
| LIBERO submodule | `8f1084e3132a39270c3a13ebe37270a43ece2a01` | Simulator contract to qualify in chapter 191 |

The [retained license evidence](../fixtures/robotics/groot-libero/NOTICE) stays
attached to this particular release. The selected card names the NVIDIA Open
Model License Agreement; the reference code carries Apache-2.0 headers. The
base N1.7 checkpoint's conflicting included noncommercial clause remains a
separate issue. Neither another model card nor a source-code license resolves
that conflict for unused base weights.

The owner has accepted the Cosmos terms and authenticated its download. Its
earlier unauthenticated 401 is historical. The authorized assets and weights
stay in private caches; the repository retains names, exact hashes and derived
contracts. Reading cloned reference source never requires executing that
repository's scripts.

The two complete learned-weight shards total **6,910,499,416 bytes**. The
[full-cache receipt](../fixtures/robotics/groot-libero/full-cache-verification.json)
records completed whole-file hashing, distinct from the earlier header-only
audit. All 1,031 stored tensors are inventoried. Both token embedding and an
unused language readout are serialized; this policy consumes hidden features
and does not need the readout to generate text. A digest written into an input
document is only a claim until the corresponding bytes are authenticated.

## An observation has an exact meaning

The source state order is EEF xyz, three axis-angle rotation coordinates and
two finger positions. The rotation is a unit axis multiplied by its angle in
**radians**. Source field names `roll`, `pitch`, `yaw` do not make these Euler
angles. Our current state boundary accepts those eight values after quaternion
conversion; it does not itself turn a quaternion into axis-angle.

The fixed original `libero_sim` q01/q99 values define normalization. We do not
estimate new bounds from a request or substitute chapter 189's metre/velocity
scales. The first arithmetic step divides `(value - q01)` by `(q99 - q01)`
using the source statistics, assigns that result to F32, then computes the
F32 `2 * ratio - 1` and clips to `[-1,1]`. Moving the F32 assignment changes
rounding. The normalized eight values are padded with 124 zeros.

<!-- source: psionic/lib/robotics/groot-embodiment.coax normalize -->
```jai
ge_normalize :: (state:[]f32,out:[]f32,overlap:bool)->GeRead #export_c {
  if state.count!=8 || out.count<132 {return GeRead.{status=GeStatus.shape};}
  if overlap {return GeRead.{status=GeStatus.overlap};}
  for v:state {if !ge_finite(v) {return GeRead.{status=GeStatus.number};}}
  staged:[132]f32;clipped:=0;
  for i:0..<8 {v:=ge_state_clip(i,state[i]);staged[i]=v.value;if v.clipped {clipped+=1;}}
  for i:0..<132 {out[i]=staged[i];}return GeRead.{written=132,clipped=clipped};
}
```

`clipped` counts strict outliers; exact endpoints do not count as clipping.
The [adapter](../lib/robotics/groot-embodiment.coax) stages every result before
publication, refuses nonfinite consumed values and preserves the caller's
output on refusal. The original q01/q99 values give the coordinates their
model meaning. Conversion into a simulator controller's pose or velocity
units remains part of the embodiment contract.

The cameras are `image` followed by `wrist_image`, each raw 256-square HWC RGB
u8. The LIBERO adapter reverses both spatial axes. Deterministic evaluation
then takes the 243-square centre crop `[6:249,6:249]` and resizes it to 256
using OpenCV `INTER_AREA`. The crop fraction `.95` scales each edge before
integer truncation. The first smallest-edge-256 resize is an identity for
this admitted source size.

<!-- source: psionic/lib/robotics/groot-image.coax preprocess -->
```jai
// Input is one raw simulator256x256HWC RGBu8 camera BEFORE its wrapper flip.
// Output is normalized[3,rows,256]F32. (first=0,rows=256) is the full camera,
// ready for gv_pack; smaller genuine row slices keep VM qualification bounded.
gi_preprocess :: (first:int,rows:int,raw:[]u8,out:[]f32,work:[]f32,overlap:bool)->GiRead #export_c {
  if !gi_rows(first,rows) {return GiRead.{status=GiStatus.shape};}need:=rows*768;
  if raw.count!=196608 || out.count<need || work.count<need {return GiRead.{status=GiStatus.shape};}
  if overlap {return GiRead.{status=GiStatus.overlap};}
  axis:[256]GiAxis;for d:0..<256 {axis[d]=gi_axis(d);}
  for y:0..<rows {for x:0..<256 {for c:0..<3 {work[c*rows*256+y*256+x]=gi_normalize(gi_pixel(raw,axis[first+y],axis[x],c));}}}
  for i:0..<need {out[i]=work[i];}return GiRead.{written=need};
}
```

The [Coax image operator](../lib/robotics/groot-image.coax) supports complete
cameras and genuine row slices. It reproduces this u8 upscale's special
11-bit coefficients and vertical rounding. Generic floating bilinear
interpolation gives different pixels. The Qwen fast processor fuses rescale
and normalization into F32 `(pixel - 127.5) / 127.5`, then packs neighboring
merge cells before channel, temporal and within-patch coordinates.

A still image is repeated into a two-frame temporal patch. Each camera yields
256 raw patches of 1,536 values, grid `[1,16,16]`, then **64 merged visual
tokens**. Camera order, temporal duplication and patch order are learned-input
contracts.

## Make the original model request

The selected instruction path lowercases and removes characters outside
Python word/whitespace classes. Our current implementation admits **ASCII**
language, where those operations are qualified exactly, and explicitly refuses
non-ASCII. It preserves underscores and Python's ASCII whitespace controls.
The byte limit is 4,096; the existing tokenizer's per-piece and work budgets
also apply.

[`gr_render`](../lib/robotics/groot-request.coax) writes one user message:
the two image markers in camera order, then the instruction, with no assistant
generation prompt. The existing Coax BPE encoder uses the privately packed
**original Cosmos tokenizer**, including its actual vocabulary and merges.
The table has its own authenticated identity; a compatible-looking tokenizer
is insufficient.

For `Open the drawer!`, the formalized instruction is `open the drawer`.
The rendered request is 1,767 bytes and the actual tokenizer produces 140
tokens. Image-pad IDs occupy `[4,68)` and `[70,134)`. The instruction IDs are
`[2508,279,26482]`. These values come from the pinned original tokenizer and
are independently compared against the Coax encoder.

The source processor left-pads to the longest request in a batch. It does not
truncate every request to 512 tokens. **137 through 512 tokens** is our current
resource admission profile. [`gr_plan`](../lib/robotics/groot-request.coax)
emits IDs, attention and image masks, and three multimodal position arrays.
Each camera's merged `8 × 8` grid occupies 64 sequence slots while advancing
the maximum position by eight. The unpadded RoPE delta is `-112`.

Use the actual unpadded length for the single-observation reference. The four
later VL self-attention blocks receive no padding mask in the original source;
adding padding merely to fill a GPU allocation can change valid features.
The [SDK metadata check](../fixtures/robotics/groot-libero/request-sdk-qualification.json)
compares these positions and deltas directly with installed Transformers,
without allocating learned weights or running a model forward pass.

## Follow every learned stage

The actual checkpoint uses the Cosmos/Qwen3-VL backbone. Its tensor inventory
and config take precedence over older model-card descriptions and current
reference defaults.

| Stage | Actual selected computation |
| --- | --- |
| Vision | Conv3d patch projection, learned positions, visual RoPE, 24 width-1024 blocks and ordinary/three deep-stack mergers |
| Language | 16 retained width-2048 Qwen3-VL blocks; 16 query heads, eight KV heads, dimension 128; three visual streams injected into the first three blocks |
| VL conditioning | Learned layer norm and four width-2048 self-attention blocks |
| State | Category-2 `132 → 1024 → 1536` MLP with ReLU |
| Action encoding | Category-2 action projection, sinusoidal time features, swish MLP and learned positions |
| Flow transformer | 32 width-1536 blocks over one state plus 40 action tokens; 32 heads of dimension 48 |
| Action decoding | Time-conditioned output projection, category-2 `1024 → 1024 → 132` ReLU MLP and last-40-row selection |

The flow transformer alternates odd self-attention blocks with even
cross-attention blocks. Cross blocks alternate non-image and image tokens.
Its final norm uses epsilon `1e-6`; the VL and block norms use `1e-5`.
AdaLN inside a block splits **scale, shift**; the final output modulation
splits **shift, scale**. Those similar-looking operations are different
source contracts.

Start from an explicit Gaussian noise tensor `[1,40,132]`. Four Euler steps
use buckets **0,250,500,750**, each with `dt=1/4`. Each step re-embeds the
current actions, executes all 32 blocks, predicts velocity and updates the
entire chunk. Action encoding and DiT conditioning use different sinusoidal
embedding formulas, even though they share a bucket. All 7,168 time-embedding
words are compared independently with the reference.

Record the actual initial noise bytes. A shared nominal seed across runtimes
does not establish identical Gaussian samples. The current profile uses fresh
chunks; the source's optional real-time chunk inpainting is a separate feature.

## State the numerical and ownership profile

Original learned weights are stored as BF16. The composed Metal profile uses
F32 activation storage and affine stages, with wide Coax statistics for
normalization across vision, language, conditioning and the action head.
Those statistics use Coax's float-float representation on Metal. This is an
explicit profile with its own measured error budgets; it does not claim the
source's default BF16-activation rounding.

Coax describes matrix operations, shapes, masks, order, buffer arguments and
publication. The native host compiles generated Metal, allocates buffers,
submits commands and reports completion. Learned projections run on the GPU;
CPU work prepares small input metadata and supplies independent development
references.

Private outputs and numeric flags begin poisoned. A kernel leaves a malformed
work item's flags poisoned; a consumed nonfinite value produces a failure flag.
A successful wait and every expected zero flag are prerequisites for public
output. Buffer extents, live source storage and honest platform receipts remain
the caller's duties. A numerical replay receipt is not remote attestation.

The [text support kernels](../lib/robotics/groot-text-kernels.coax) implement
BF16 embedding lookup with visual replacement, paired half-rotation, deep-stack
addition and F32 elementwise operations. They have been compared on real Metal
with independent Torch fixtures. The complete 16-layer text graph consumes
the actual Coax vision outputs in the composed replay. Its 286,720 features
pass the unchanged original full-network endpoint comparison at maximum
absolute error about `0.00070417`, under the frozen
`2e-3 + 2e-3 * |reference|` budget.

## Decode an action chunk before using it

The internal model predicts **40 × 132** normalized values. The selected
processor consumes the first **16 × 7**, clipping normalized values and
inverting the original action q01/q99 statistics. Source policy output is cast
to F32 before the environment wrapper transforms the gripper.

<!-- source: psionic/lib/robotics/groot-embodiment.coax decode -->
```jai
ge_decode :: (model:[]f32,out:[]f32,overlap:bool)->GeRead #export_c {
  if model.count!=5280 || out.count<112 {return GeRead.{status=GeStatus.shape};}
  if overlap {return GeRead.{status=GeStatus.overlap};}
  staged:[112]f32;clipped:=0;
  for row:0..<16 {for axis:0..<7 {
    v:=model[row*132+axis];if !ge_finite(v) {return GeRead.{status=GeStatus.number};}
    action:=ge_action_clip(axis,v);staged[row*7+axis]=action.value;if action.clipped {clipped+=1;}
  }}
  for i:0..<112 {out[i]=staged[i];}return GeRead.{written=112,clipped=clipped};
}
```

The gripper mapping is `-sign(2*g - 1)` after unnormalization. Dataset gripper
zero means close and one means open; the resulting LIBERO commands are `+1`
and `-1` respectively. The tie at `g=0.5` preserves negative zero. Source
processor labels `ABSOLUTE/NON_EEF` do not by themselves make the six remaining
numbers world-frame target poses.

The reference rollout executes **eight** actions before requesting another
chunk. Internal prediction horizon 40, decoded horizon 16 and execution
horizon eight are separate bounds. The simulator's controller scale, sample
period, reset state and success predicate must be qualified together in the
next chapter before these values are treated as movements.

## Qualify the complete replay, then measure its cost

The independent development reference uses installed Torch2.9.0,
Transformers4.57.3 and Diffusers0.35.1 on MPS. It reads authenticated original
weights, processes two owned patterned cameras and an authored normalized
state, and records an explicit noise tensor and 66 numerical boundaries.
No captured robot episode or task-success result is implied by these inputs.

Current wide-statistics vision replay passes all **34 boundaries / 16,777,216 values** under
its frozen `2e-3 + 2e-4 * |reference|` budget. An initial last-block failure
identified a real difference in Torch's symmetric F32 `linspace` behavior;
the implementation was corrected without widening the budget. The action
head's component replay passes **31 boundaries / 1,595,296 values** under its
separately frozen `2e-3 + 2e-3 * |reference|` budget after adding wide
normalization statistics. Its earlier intermediate failure is retained in
the verification history. Passing only the final action would have hidden
that diagnosis.

The [composed-policy receipt](../fixtures/robotics/groot-libero/composed-policy-qualification.json)
records the actual Coax vision → text → conditioning → state/flow → adapter
path. Four visual outputs enter text byte-for-byte; its resulting hidden
features enter the action head byte-for-byte. Original IDs, normalized state
and explicit noise remain bound to the same fixture. Recorded upstream
reference features are not substituted inside this executed path.

There are three different numerical questions:

1. **Does a component reproduce the reference on the same inputs?** An
   independent original-weight MPS head now consumes the actual Coax backbone
   features. All **31 boundaries / 1,595,296 values** pass the existing head
   budget. This conditional comparison localizes head arithmetic; it does
   not replace the original full-network reference.
2. **Does the complete composed path agree at its action endpoints?** All
   **nine original full-network velocity/action endpoints** pass their
   unchanged budgets. Final normalized-action maximum absolute error is
   about `0.000558734`. All 112 decoded values agree bit-for-bit with an
   independent adapter applied to the actual Coax network output. Against
   the original full-network result, the 16 gripper decisions are exact and
   the continuous decoded maximum error is about `3.10242e-5`.
3. **Does every hidden trace agree with the original full-network trace?**
   No. `flow3-block31` reaches **4.88684 times** its frozen mixed-error budget.
   Earlier vision/text rounding differences propagate through the head;
   the same-input head comparison passes, but the global hidden diagnostic
   remains failed. Neither its reference nor its tolerance was changed.

This qualifies execution for the pinned prepared-observation fixture and
named numerical profile, rather than uniform hidden-state parity. The fixture
has **140 unpadded tokens**. Request/text helpers admit a wider bounded range;
the complete executed policy graph currently supports this fixed 140-token
fixture. It starts with packed F32 cameras, normalized state and explicit
noise. The independently checked raw-image/state/request helpers are not
presented as one already-qualified raw-simulator adapter.

On the measured Apple M5 Max, the current wide-statistics vision command
batches totalled about **0.2147 s** over 359 launches. The composed text graph
measured about **0.1240 s** over 325 launches; its action head measured about
**0.3227 s** over 142 command batches. Earlier
vision/action trials used different profiles and retain separate records.
These are component GPU command times. They exclude compilation, weight load/decode/upload,
input preparation, CPU flag scans, capture and comparison. They are not a
complete-policy latency or control-rate measurement.

A complete measurement uses one admitted observation and the same synchronized
endpoint throughout: input preparation, queueing, all learned stages, numeric
checks, action decoding and publication. Record cold setup separately from
warm policy latency, plus repeated latency distribution and memory use. The
reference's instrumentation time includes tensor captures and comparison work;
it is also a different measurement.

The focused preparation and support gates are independently runnable:

```shell
coax/bin/coax test psionic/lib/robotics/groot-image.coax
coax/bin/coax test psionic/lib/robotics/groot-request.coax
coax/bin/coax test psionic/lib/robotics/groot-embodiment.coax
coax/bin/coax test psionic/lib/robotics/groot-text-kernels.coax
```

Original-weight runners require the private authenticated shards,
the pinned recorded reference capsule and a new output directory. Set the
following paths to those private inputs; the
[composed runner](../tests/robotics/check-groot-policy-metal.sh) executes each
stage, checks the same-input head reference and retains the independent
global hidden diagnostic:

```shell
psionic/tests/robotics/check-groot-policy-metal.sh \
  "$GROOT_SHARDS" "$GROOT_REFERENCE_CAPSULE" /tmp/groot-policy-new
```

The verification record links the component procedures and exact source
identities. Closed-loop success, recovery under delayed
observations, deployment and physical-robot control remain separate work.
