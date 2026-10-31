# Resident original-weight Panda policy

`panda-policy.coax` owns the request preparation, buffer and kernel namespaces,
graph descriptors, cross-graph transfers, effect receipts, finite checks and
final publication. `platform/panda-policy.c` caches the resources and executes
those descriptors through Metal. Its integer effect methods return **1 on
success, 0 on failure**; Coax status enums instead use zero for success.

One context owns one ASCII task instruction and its unpadded request length.
The new action profile accepts 137–512 tokens. It preserves the published 140-token
exports and the exact 2,877-operation action graph; only conditioning geometry
and its private allocations change. The complete policy contains 3,561 GPU
launches across 189 wait/flag boundaries. Its 1,030 active original tensors,
1,106 buffers, 32 kernels and six generated Metal libraries are loaded once.

The six module paths, in order, are the generated tile, device, vision, wide
normalization, text and action sources. The numerical profile retains original
BF16 weights with F32 learned arithmetic/storage and wide normalization
statistics implemented in Coax Metal float-float arithmetic. It makes no
source-default BF16 identity claim.

Build the reusable objects and optional single-frame effects driver:

```shell
psionic/tests/robotics/build-panda-policy.sh /tmp/panda-policy-build
psionic/tests/robotics/check-panda-policy-native.sh /tmp/panda-policy-native
coax/bin/coax mutate psionic/lib/robotics/panda-policy.coax --root-only --jobs 8
```

Link `panda-policy-generated.o`, `panda-policy.o` and `metal-host.o` into a C
effect host with Metal/Foundation and libm. Runtime inputs are two genuine
raw OpenGL 256×256RGB images, the source physical state 8 and an explicit finite
F32 noise 5280 array. Coax performs source cropping, area resizing, orientation,
normalization, patch packing, state normalization and 16×7 action decoding.
No Python, Torch, Transformers or vendor neural runtime is linked or called.
One owner calls a context without overlapping calls. Failed effects or numeric
flags prevent public output publication; retrying begins a new private frame.

The evaluation profile supplies explicit PCG64(169)-derived F32 Gaussian noise
from the authenticated first-frame artifact and reuses that same caller-owned
array for each replan. Runtime does not invoke a Python RNG or claim source RNG
sequence parity. Different explicit noise can be supplied through the same API.

`panda_policy_capture` enables private development captures and accepts NULL
to disable them. It is disabled in measured scene episodes. Named diagnostics
come from the latest completed frame; the saved initial noise is distinct from
the GPU buffer subsequently overwritten by Euler actions.

The genuine 152-token first frame is a fresh MuJoCo reset/forward of original
LIBERO demo 0 XML and preaction state 0, rendered at 256 pixels; it is not a stored
postaction observation or an upscaled 128-pixel image. Coax preparation matches
every pixel/state/noise/token word of the independent original GPU capsule.
The original reference uses pinned installed SDK components on MPS; cloned
NVIDIA repository code is read, never executed.

Keep these numerical claims separate:

- Four public vision features, the text backbone, all 31 global action-head
  captures, all nine velocity/Euler/normalized endpoints and 112 decoded values
  pass their unchanged declared budgets; all 16 gripper decisions match exactly.
- A second independent original head conditioned on the actual Coax backbone
  passes all 31 same-input component captures.
- Four late global vision hidden boundaries fail the original 0.002+0.002|ref|
  budget. Their original failures and metrics remain recorded. Public endpoint
  qualification does not relabel them or widen their bounds.

`panda_policy_reference.py` defaults to the stricter global-vision audit;
`--public-endpoints` writes a different, explicit endpoint-scope schema and
retains every global diagnostic. These first-frame numerical receipts do not
establish task success; the separate fresh-scene control-loop evaluations
measure actual simulator outcomes.
