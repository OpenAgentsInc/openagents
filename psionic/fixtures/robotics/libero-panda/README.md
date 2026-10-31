# Original LIBERO/Panda evaluation profile

This folder contains metadata for one genuine LIBERO 10 book-to-caddy task.
The original HDF5, embedded scenes, meshes, textures, rendered images and model
weights remain private. Coax owns observation conversion, controller arithmetic,
action scheduling and success decisions. MuJoCo 3.3.1 is the explicitly selected
independent simulation backend; its dynamics and renderer are not claimed as
a Coax physics implementation.

| Record | What it fixes |
| --- | --- |
| [acquisition.json](acquisition.json) | Original dataset revision and whole-file hash, 85 asset hashes, source pins and license identities, SDK binary/header identities |
| [contract.json](contract.json) | Exact controller, observation, image orientation, reset, timing and source success semantics |
| [evaluation-profile.json](evaluation-profile.json) | Named nominal-home/fixed-noise profile, frozen implementation/evaluation split, every original XML/init-state pair hash |
| [frame0.json](frame0.json) | Genuine 256-square SDK camera capture and prepared 152-token numerical-reference inputs |
| [qualification.json](qualification.json) | Focused Coax, native, NumPy and independent SDK effects evidence |
| [clock-qualification.json](clock-qualification.json) | Retrospective native guard/stdlib clock checks on 9,700 saved physical-step records from the first two episodes |
| [policy-qualification.json](policy-qualification.json) | Genuine 152-token preparation and public learned endpoints, with separate retained failed global vision diagnostics |
| [closed-loop-qualification.json](closed-loop-qualification.json) | Six frozen task outcomes, four fault prefixes, actual run artifacts and exact controls replay |
| [integration-qualification.json](integration-qualification.json) | Regenerated current-main C/Metal under ASan+UBSan; one request, eight ticks and exact replay |

The source is `yifengzhu-hf/LIBERO-datasets` revision
`f13aa24a3da8c43c7225569f28c562979fa0e35a`, task
`STUDY_SCENE1_pick_up_the_book_and_place_it_in_the_back_compartment_of_the_caddy`.
Its 940,528,916-byte HDF5 has SHA256
`2e0127d0cf73afceacc2c0854e505d5743b6d3d5aaf5572c842f84c6a1c8173b`.
The card declares Apache-2.0. Pinned LIBERO and robosuite source licenses are
MIT; NVIDIA reference code and the installed MuJoCo SDK use Apache-2.0.
The exact source/license files and hashes are retained in acquisition metadata.
Original mesh paths do not authenticate the historical collector filesystem;
this evaluation fixes their referenced data to the recorded official Git
revisions. Metadata alone cannot authenticate arbitrary future files.

Demo 0 supplies controller and neural implementation fixtures. Evaluation
demos **1, 10, 20, 30, 40 and 49** were selected before policy task outcomes.
Every demo has its own original `model_file` XML and `init_state`; all 50 fixed
caddy poses differ. Pair them correctly. The split separates implementation
fixtures from evaluation scenes. The fine-tuned checkpoint may have trained
on these demonstrations, so this is not a model-training holdout or a
generalization estimate.

The owned reset profile restores the original paired 45-word state, resets
MuJoCo's hidden state, and initializes the Coax controller at the restored
grip-site pose with the nominal source Panda home and a zero internal gripper
target. It differs from NVIDIA's ordinary seeded environment-reset
distribution and does not reconstruct unrecorded historical controller or
solver state. Fixed captured Gaussian noise is an explicit input; source RNG
parity is not claimed.

The controller advances **25 physics steps of 0.002 s per 20 Hz action**.
Inference pauses simulation; this logical cadence is not a wall-clock control
rate. Forward/sample happens before each torque evaluation. After the final
`mj_step`, observation and rendering happen without another forward: derived
site/body poses lag one physics substep, while finger qpos is read directly.
A fresh reset/forward has zero derived lag. Source HDF5 `states[t]` is
pre-action but `obs[t]` is post-action. Its last-row done/reward labels are
synthesized and must not replace the online success predicate.

The session admits initial clocks in [0,100] seconds and checks physical time
against the counted 0.002-second steps with inclusive 2^-26-second tolerance.
This is a plain F64/integer SDK clock, not a compiler typed clock or a wall
deadline. The first two completed episodes predate the live guard; checking
their saved clocks does not relabel those executions as having run it.

State XYZ comes from `gripper0_grip_site`, but quaternion rotation comes from
the **`robot0_right_hand` body**. Axis-angle conversion preserves quaternion
sign, including angles greater than pi. The controller uses the grip site's
different rotation/Jacobian. Camera readout is raw OpenGL RGB; the NVIDIA
wrapper flips both spatial axes. The Python SDK Renderer already flips its
vertical axis, so it needs an inverse flip before comparison with raw C
`mjr_readPixels` output.

The source target is a site: its contact method always returns true. Success
is strict point containment with `extent = abs(site_matrix @ site_size)`,
world-coordinate bounds, and a 0.01 m lower-z allowance. It is the source AABB
approximation, not a substituted contact test or improved oriented-box test.

The [Coax boundary](../../../lib/robotics/panda-env.coax) and
[C SDK effects ABI](../../../platform/panda-env.h) are separate. All C SDK
integer functions return **1 for success, 0 for failure**; Coax status enums
use zero for success. The native SDK functions do not clip controls, select
goals, determine success, or advance a policy schedule.

The portable boundary checks require no MuJoCo installation:

```shell
coax/bin/coax test psionic/lib/robotics/panda-env.coax
coax/bin/coax mutate psionic/lib/robotics/panda-env.coax --root-only
psionic/tests/robotics/check-panda-env.sh /tmp/panda-env-new
```

The optional SDK effects gate consumes an already authenticated, audited
original scene with locally resolved pinned assets and its matching init
state. It links the privately installed binary SDK and renders through
nonvisible CGL. Set `MUJOCO_ROOT` and `MUJOCO_REFERENCE_PYTHON` when the private
installation uses different paths:

```shell
psionic/tests/robotics/check-panda-env-sdk.sh \
  "$PANDA_SCENE" "$PANDA_INITIAL_STATE" /tmp/panda-sdk-new
```

No LIBERO, NVIDIA or robosuite checkout scripts/modules are executed. The
independent NumPy and installed-SDK references are development tooling, with
render comparison separated from exact physics replay. Learned-policy task
outcomes and wall-clock performance belong in a separate runtime record.

The completed fixed evaluation measures **3/6 successes**, 343 model requests
and 68,325 exactly replayed physics steps. The other three cases reach their
720-tick limit and remain task failures. Four 16-tick fault prefixes resume
admitted control with expected holds/refusals/motor clipping and exact replay.
These outcomes are separate from source-framework full-trajectory parity,
checkpoint-training holdout or NVIDIA benchmark performance. Four accumulated
global vision diagnostics remain false under their unchanged budgets, while
the independently checked public learned endpoints pass.

The [source-framework baseline](framework-baseline-qualification.json) also
reaches the limit on all three failed cases (20,30,40), using the same reset,
noise, Coax prep/decoder/controller and a distinct actual-MPS reference provider.
Its first frame matches all 5,280 frozen normalized outputs exactly. This is
not NVIDIA BF16 benchmark parity, proof of identical trajectories or causal
attribution. Product execution remains Coax; Python is reference tooling.
