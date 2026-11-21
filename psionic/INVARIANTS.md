# Invariants — psionic/INVARIANTS.md

> Scope: Psionic ML, tensors, kernels, models, serving, Pylon, robotics, ASR (82 invariants). Indexed from root [INVARIANTS.md](../INVARIANTS.md).

## Inverse physics differentiates the owned solver and exposes ambiguity

`psionic/lib/physics/inverse-*.coax` unrolls chapter 109’s exact left-associated
F64 RK4 transition on chapter 104’s scalar reverse tape, not its learned
surrogate. Mass, step, forces and initial states are known; only spring and
damping are inferred. Sparse observations thin the loss, not the integration.
The frozen experiment keeps optimizer rates, starts, noise streams, sampling,
box constraints and held-out forecasts fixed before fitting.

The 255-update fit plus final objective uses 256 forward/255 reverse passes;
the 16×16 grid uses 256 forward/zero reverse. Equal forward count is not equal
arithmetic cost. Public fit refusal rolls back to admitted parameters.
Equilibrium data admits different damping values at zero loss; empirical clean
recovery does not prove global identifiability or noise robustness. Independent
analytic sensitivities, optimizer/forecast calculations, exact transcript
coverage and all 5,040 serialized state words are qualified separately in
`psionic/fixtures/physics/inverse/qualification.json`.

## Panda clock receipts preserve the old trace and real effect order

The Panda sidecar pairs PlState's action-row cursor with the physical-step
clock. Last-forward state changes only on the actual forward receipt; fresh
sample, control intent and one integration receipt follow. Pl's tick advances
after its 25 trace records, preserving PTR1 field/version identity. Bad or partial
effects return paired halted state. Ordinary admission is atomic, and clock
acceptance is checked before the supervisor publishes an action chunk.

Recorded-command prefixes qualify SDK states and adapter order. Their copied
actuator words/clipping flags and declared unused command tails do not establish
fresh NN replies, controller recomputation or a new episode score. Historical
whole-model artifact hashes remain separate from the updated hook qualification.

## Owned oscillator dynamics and learned surrogates keep their roles distinct

`psionic/lib/physics/oscillator*.coax` owns a fixed-mass F64 damped forced
oscillator, piecewise-constant-force RK4 and atomic bounded rollouts. Parameters,
state, force, step size and horizon are admitted before publication; refusal
preserves the caller’s public state/trajectory. This smooth one-axis plant is
not articulated Panda/contact physics or a compiler bounded-control proof.

Training uses the frozen whole-episode parameter/initial-state/forcing split.
Feature and target scales see training rows only. The affine and MLP heads
predict state increments per unit time; independent learned rollouts feed back
their own predicted state, with refusal and completed length observable.
One-step position error favors the affine model; the MLP improves the held-out
rollout metrics. Neither metric substitutes for the other. The closed-form
physical reference, independent data/SGD oracle and native mode comparisons
are recorded in `psionic/fixtures/physics/oscillator/qualification.json`.


## Owned Panda kinematics preserves the exact model and attitude boundary

The first FK query constructs its immutable twelve-body/seven-hinge profile
from pinned source coefficients. It accepts seven finite radian coordinates
within its declared numerical box, with no soft-limit clamping, and publishes
all poses only after admission. World positions use F64 Point3; Hamilton-wxyz
quaternions and row-major world-from-body matrices remain raw conventions.
The hand BODY and grip SITE attitudes are distinct.

Reference poses are freshly forwarded and paired with their joint coordinates;
cached geometry/current-qpos pairs are not interchangeable FK targets. Scalar
libm host flags qualify cross-mode bytes on the named Darwin target; the prior
default fused-libm byte mismatch is preserved. This is a geometric query, with
no simulation clock, hardware, model import, fingers or articulated dynamics
claim. The historical 281-site FK mutation score predates lowered spatial
operator-token swaps. The later source-token proof is recorded separately;
it does not retroactively broaden that historical score.

## Panda position IK keeps iteration and command meaning separate

The pinned seven-hinge query admits source-range radian seeds and one F64
world/metre grip target. Fixed DLS normalizes columns by one radian, uses the
.05-metre damping basis, globally caps increments at .1 radians and projects
source ranges. The caller update budget is 0..96 and the Euclidean tolerance
is 1e-6 metres. One-step publication is an admitted geometric candidate;
whole-solve publication requires convergence. Every non-OK path, including
exhaustion, preserves the complete output. Nonconvergence is local to the
seed/profile/budget and does not prove global unreachability.

Independent homogeneous-matrix tangents and pivoted Gaussian solves retain
all trajectories under frozen budgets; status/counters and owned-mode bytes
are exact. The original49-case corpus and separate active-limit contract
retain their identities. Trusted coefficient helpers stay source-visible,
with no orientation/collision/physical command/dynamics or compiler-bounded
control claim. Existing FK/Jacobian proofs are carried, not broadened.

## Scalar servo steps preserve complete delay and observation-play state

`psionic/lib/robotics/actuator.coax` owns the illustrative
`scalar-servo-play-f64-v1` CPU map. Requested/limited torque, derivative
feedback and passive loss remain distinct. Its semi-implicit step stages
the complete parameter/state/queue/cursor/counter snapshot before final
admission. Refusal publishes no partial force or consumed target. Output
play is a separately observable, stateful numerical precursor: it applies
no load reaction or feedback and is not a mechanical gearbox model.
The shared finite clamp preserves strict flags and incoming endpoint zero
bits for Panda, the experiment action map and this servo. Numerical boxes
do not authorize physical operating limits. No stored checkpoint, measured
motor, biped/contact or device/timing qualification follows. See
`docs/verification/robotics-actuator-rb02-2026-11-15.md`.

## Running observations retain exact snapshot and transform conventions

`psionic/lib/running-stats.coax` admits the whole finite bounded input and
all active/inactive state fields before publishing an incremental candidate.
Every active count matches; inactive slots and empty moments are zero.
The shared Welford arithmetic retains the benchmark comparator's order,
while observation normalization explicitly uses population variance with
epsilon 1e-8 and identity at zero samples. Candidate admission cannot be
removed: a rounded M2 update can cross its numerical ceiling. Refusals
preserve all state and value outputs. This is an in-memory F64 component;
collector freeze/version fencing, stored checkpoints, neural precision and
learning remain separate work. See
`docs/verification/robotics-running-stats-rb03-2026-11-15.md`.

## Panda finger coordinates preserve the arm contract and rigid branch order

The separate `PffOutput` embeds the unchanged twelve-body `PfkOutput` and four
finger BODY poses. Seven radians and two metre-valued slide coordinates are
admitted by shape, arm index and slide index before atomic publication. The
inclusive +/-1m slide box is numerical admission, not source soft-limit or
normalized-command validity. Each fixed +90-degree BODY rotation precedes its
local +Y slide; the right finger GEOM rotation is not that BODY transform.

Full-F64 reference coordinates and fresh poses describe one generation. No
sensor stamp is inferred. The five new SDK forward queries qualify FK only;
their separately retained mass/bias and compiled inertial facts are future
inputs. Moving finger inertias affect even the arm mass subblock. These are
simplified source simulation parameters, not manufacturer measurements or an
owned articulated/contact/dynamics qualification.

## Panda mass and gravity preserve coordinate dimensions and source scope

The fixed sixteen-body query sums COM translation, principal-frame rotation
and nine source armatures into the full robot9 metric, including moving fingers.
Seven radian and two metre coordinates give separate HH/HS/SH/SS typed blocks:
kg*m^2/rad^2, kg*m/rad, kg*m/rad and kg. Generalized gravity is energy/radian
for hinges and force for slides. Raw source coefficients use an explicit
one-radian generator convention; angles are not silently erased.

The public fixed-source forward entry evaluates owned FK and publishes one
complete result after admission. Source-visible profile/pose arithmetic helpers
have trusted preconditions; their PODs are forgeable and carry no provenance
certificate. Independent COM/rotation kinetic and potential differences keep
their frozen primary step and diagnostic sweep. Five retained zero-rate SDK
biases qualify gravity only, not velocity-dependent bias, passive/actuator forces,
contacts, integration, a simulator replacement or hardware. The earlier failed
sanitizer harness record remains separate from its repaired qualification.

## Panda inverse force preserves moving-axis and armature terms

The fixed-source inverse query propagates physical world velocities and
accelerations, including hinge-axis derivatives and the slide Coriolis factor
two. COM wrenches use world principal inertia, gyroscopic torque and gravity;
all fourteen moving descendants contribute, including the four finger bodies.
The sixteen-body profile retains the two fixed bodies, whose generalized
columns are zero. Nine armatures contribute acceleration force exactly once.

Typed seven/two coordinate, rate and acceleration spans precede atomic output
publication. Hinges return energy per radian and slides return force. Trusted
source-visible pose/profile helpers are unsealed and do not admit arbitrary
caller PODs. SDK mass-times-acceleration plus bias and independent matrix/
Christoffel differences qualify the result. Raw SDK RNE excludes armature and
is compared only with the explicit correction. Passive, actuator, contact,
integration, hardware and whole owned-episode claims remain separate.

## Panda mass solve consumes net generalized force in an explicit basis

The fixed-source public solver owns M(q), admits original force once before
quantity arithmetic and stages all nine accelerations. One-radian/one-metre
bases make the mixed mass blocks homogeneous kg*m² and RHS entries joules;
raw F64 factorization returns inverse-second² coefficients, then the same bases
restore formal Radian/Second² and Metre/Second² quantities. There is no implicit
angle erasure or automatic gravity/velocity-bias subtraction.

Its .25 inclusive Cholesky Schur-pivot floor belongs to the normalized fixed
model profile, supported by the .714286 minimum armature plus PSD body metric;
it is not generic SPD acceptance. Source-visible basis, factor and publication
helpers have trusted preconditions and forgeable PODs. Cached earlier Coax
metric outputs and original SDK matrices retain separate reference provenance.
Independent Gaussian/residual comparisons use complete finite arrays and frozen
per-scalar absolute-plus-relative budgets. The first failed native fault setup
remains recorded. This query supplies no net-force assembly, contacts, integration,
command admission, compiler bounded proof, sensor clock or hardware control.

## Panda applied-force acceleration keeps bias and actuator semantics explicit

The fixed-source forward query admits positions, rates and applied generalized
forces before subtracting owned gravity-plus-velocity bias in matching units.
Applied and net coefficients each have their own inclusive magnitude-10000
numerical cap; a legal applied endpoint may therefore refuse at the net cap.
Refusal preserves all caller output and no force is silently clipped. One
source-bound pose/profile feeds the metric and zero-acceleration inverse query,
then the existing explicit radian/metre basis restores typed acceleration.

Finger applied forces are newtons, whereas controller finger targets are
metres; the query supplies no actuator conversion. Complete finite independent
Gaussian/equation comparisons and original prescribed accelerations qualify
the arithmetic. The 85 emitted zero-survivor sites do not claim complete
Quantity subtraction-token provenance. Native evidence is the named Darwin-arm64
scalar-libm profile, with unsupported targets refused rather than skipped as
passes. Source-visible helpers remain trusted and unsealed. Passive forces,
contacts, integration, complete SDK qacc parity, compiler bounded control and
hardware execution remain separate qualifications.

## Fixed Panda fluid uses COM and principal-inertial frames

The source's zero geom-fluid interaction flags select inertia-box fallback;
they do not disable fluid. Positive-mass robot bodies use COM linear velocity
and principal-inertial angular axes, including the hand without geometry and
artificial fingertip inertias. World wrenches project through those same COM
Jacobians. Explicit one-radian normalization gives angular rate inverse-second
coefficients; physical moments restore hinge energy/radian and slides retain
newtons. Their velocity products are watts.

The fixed source admits shapes, coordinates then rates and publishes fluid,
joint damping and complete passive force atomically. Complete passive relies
on proved absent other passive sources; frictionloss remains a constraint.
Omitted bodies are excluded only after full ancestor/DOF analysis, including
fixed mounts sharing the robot root. Trusted coefficient helpers remain
unsealed. Independent matrix/SDK/virtual-work evidence keeps frozen budgets,
retains the earlier failed omission guard and qualifies no new SDK calls,
physics integration, contact solver, generic models, R14 or hardware.

## Smooth Panda force assembly counts passive sources once

The source-bound smooth query assembles `(actuator + external) + passive`, then
uses the existing applied-force/bias/mass query. Complete passive already
contains joint damping and fluid. Finger control targets remain metres;
external inputs are generalized hinge energy/radian and slide newtons, not
Cartesian force. Every shape precedes numerical admission; the raw adapter can
copy scalars before its delegated output-shape check. External admission comes
before positions/rates and actual controls. Combined applied and net caps
refuse atomically without external/net clipping. Actuator counters still
report the actual controls.

The frozen twenty-target proof distinguishes eighteen cached SDK smooth
accelerations from two derived same-state equal-total-force targets. No new
external-force SDK query is implied. Numerical SDK agreement and exact owned
cross-mode bytes are separate. Typed Quantity token mutation provenance stays
explicitly incomplete. This is unconstrained acceleration, with no contact or
frictionloss solve, implicit-damping integration, robot episode, R14 or hardware
claim.

## Owned Panda Euler keeps implicit damping and reference scopes explicit

`panda-euler*.coax` owns one fixed 2 ms unconstrained robot9 state map.
It solves `(M+hB)*a_eff=((actuator+external)+fullpassive)-bias`, with current
joint damping retained inside passive and no fluid derivative in the metric.
The nine source armatures occur once. Velocity advances before position;
all shapes and external/model/control/applied/net/next-state admission precede
atomic 232-byte publication. Numerical boxes are separate from joint limits.

Three retained-operand Gaussian targets are algebraic references, not observed
SDK steps. Three new SDK observations use constraint/contact-off flags 17 and
a clear EULERDAMP disable bit. Their q/v are direct updated state; effective
acceleration is derived from delta-v/h, not `d.qacc`. Both robot/other-scene
mass cross blocks and all warning/constraint-force fields are zero. The
initial asset-declaration preflight failure and frozen correction remain in
the receipts. Mode byte equality and SDK tolerance parity are separate.

Only actual emitted mutation sites are claimed; scalar Quantity token
coverage remains incomplete. Trusted low-level POD helpers are unsealed.
The normal gate consumes committed fixtures without acquiring SDK data.
This component adds no clock, constrained scene, episode, R13/R14/R15 or
hardware qualification. Evidence: `psionic/fixtures/robotics/panda-euler/`.

## Panda evaluation keeps SDK physics, Coax decisions and clock evidence separate

The chapter 170 profile uses the original LIBERO/Panda scene, selected GR00T
weights and MuJoCo 3.3.1 as an independent evaluation backend. Coax owns
observation conversion, OSC arithmetic, gripper targets, bounded request and
action scheduling, success predicates and canonical trace admission. C performs
SDK, file and GPU effects. This does not implement a Coax rigid-body/contact
solver, differentiable simulation, compiler `#bounded`/typed-clock proofs or
hardware control.

Each source demo's XML stays paired with its own initial state. A fresh SDK
reset clears hidden solver state; the explicit owned controller reset uses
the nominal Panda home and zero internal gripper target. It does not reproduce
unrecorded historical controller state or NVIDIA's ordinary random-reset
distribution. The frozen implementation/evaluation split is not a checkpoint
training holdout. Fixed captured noise does not establish source RNG parity.

State position uses the grip site; quaternion rotation uses the right-hand
body, preserving sign and axis-angle values greater than pi. The controller
uses the grip site's different matrix/Jacobian. Each logical 20 Hz action has
25 SDK steps of 0.002 seconds. Forward/sample precedes each torque evaluation;
after the last step, observation/rendering retains the source's one-step stale
derived poses, while finger qpos is read directly. A zero gripper command holds
the internal target, not necessarily the current physical jaw position.

The actual SDK timestep is admitted before reset. `ps_time` checks finite
clock words against `initial + (25*tick + substep)*0.002`, with initial time
in [0,100] seconds, ticks 0..719, substeps 1..25 and inclusive 2^-26-second
tolerance. This is plain F64/integer logical-clock validation, not a wall
deadline or compiler clock guarantee. Simulation pauses during inference.

Stale/invalid results cannot replace an admitted command chunk. Unexpected
effects or clock failures abort without a completed case ledger; partial SDK
effects are not rolled back. Existing case directories are not overwritten.
Recorded control replay qualifies exact SDK state replay for the recorded
profile; SHA chains detect changes without authenticating remote models or
scenes. Same-input learned-component and action-endpoint comparisons remain
distinct from the independently failed accumulated global hidden diagnostic.
Evidence and limits: `psionic/fixtures/robotics/libero-panda/README.md` and its
clock receipt. The first two episodes' clocks were checked retrospectively;
those executions are not relabeled as having used the later live guard.

## Original GR00T execution qualifies a prepared observation and explicit numerical profile

`psionic/lib/robotics/groot-*` executes the pinned GR00T-N1.7-LIBERO
`libero_10` category-2 learned network through its 16×7 action adapter. The
chapter 169 qualification covers one 140-token prepared-observation profile:
two packed camera streams, normalized state and explicit initial noise.
Coax owns tensor bindings, shapes, numerical operations, masks, every GPU
launch argument/order, completion receipts and finite publication. Native C
performs file, allocation and device effects. The caller supplies authenticated
immutable original files, valid disjoint typed storage and honest effect
receipts; matching metadata alone does not authenticate future payloads.

BF16 stored coefficients feed F32 activations/affine stages and explicitly
wide normalization statistics (Metal float-float). Same-input component
comparisons and unchanged original full-network action endpoints pass their
frozen bounds. The separate global late-hidden diagnostic remains failed;
conditional head comparison does not relabel it or substitute SDK features
into the product path. All 112 adapter outputs match an independent adapter
on actual Coax results; all 16 gripper decisions match the original endpoint.
This is not source-default BF16 identity or a task-success claim.

Raw-image, ASCII request and state normalization operators have separate
qualified boundaries. A raw simulator adapter, quaternion conversion, broader
whole-model request lengths, observation timing, resets and physical control
remain outside chapter 169. Evidence:
`docs/verification/psionic-ch113-2026-10-31.md`.

## Robotics imitation keeps its simulated embodiment and recorded data distinct

`psionic/lib/robotics/` owns `robotics-cartesian-bc-f64-v1`: a two-axis
Cartesian point in a metre-based workspace, per-axis velocity limits, fixed
0.1-second ticks and explicit observation/action scales. Coax validates finite
state, bounded policy and action, time and next workspace before returning a
new state; refusal returns the original state. Four complete owned episodes
train two affine heads through the existing checked SGD. Six different complete
episodes measure teacher-forced action error and independent candidate
rollouts. Neither one-step error nor the finite success count proves transfer
to another embodiment, contact dynamics or hardware.

The separate SingleArm reader admits only the pinned Panda metadata profile
and a bounded inert F32 excerpt: unique semantic fields, exact named 25/8
channels, finite values, increasing frames, bounded capacity and timestamps
within the declared binary tolerance. It stages every sample before publication.
Original-file hashes and the independent Parquet comparison establish this
committed fixture's provenance; declared digest strings cannot authenticate
arbitrary future rows. Panda delta-command scaling, physical units, gripper
semantics, images, GR00T inference and real-robot execution remain unqualified.
The owned gantry's scales are not a Panda adapter. Evidence:
`docs/verification/psionic-ch112-2026-10-31.md`.

## Pylon restarts reserve a durable provider-bound beacon epoch

`psionic/lib/pylon/generation.coax` admits only canonical, provider-bound
counter records and exact JSON integers through 2^53-1. Fresh state begins at
2 for migration from the legacy executable's generation 1; restart advances
the durable counter before any relay publication. A lifetime lock on a separate
sibling inode excludes a second process sharing the state while the counter
itself is atomically replaced. File and parent-directory fsync must both succeed
before Coax permits the epoch. Corrupt, foreign, exhausted, unreadable or
uncommitted state fails closed, without connecting to the relay.

The owner supplies one authoritative persistent state directory per provider;
deleting/restoring an old counter while reusing its key defeats that premise.
The counter is public metadata, not signing-key custody, remote attestation,
durable request deduplication or synchronization across cloned identities.
Host C performs file/lock effects; all parsing, binding, increment, exhaustion
and commit admission are Coax. Checks: the focused generation gate and the
local encrypted task/restart runner in `psionic/tests/pylon/`.

## Psionic layer-major prefill preserves route order and complete chunk publication

`psionic/lib/deepseek/prefill*.coax` owns `deepseek-prefill-cpu-wide-v1`.
Stable expert groups retain original token/route ordinals and their weights.
Each active expert packet is staged once per layer/chunk; gather/expert/scatter
restore those ordinals. Eight-row matrix tiles share decode while every output
keeps chapter 95's increasing-K TwoSum/correction reduction and final F32 cast.
Activation grids, clamp/SiLU and pre-down route weighting remain unchanged.
Combine narrows the routed sum before adding the shared result exactly once.

Grouping/FFN/combine outputs stage before publication. The cursor orders
attention, routing, all expert groups, shared and combine for each layer before
chunk commit; wrong/failed/stale receipts refuse. It does not authenticate
model/history identity or synchronize callers. Intermediate hidden/contribution/
cache state stays private until the integrating engine publishes the chunk.
Native effects observe spans and read files; all arithmetic/decisions are Coax.

Chunk size is the largest count fitting an explicit measured-cost estimate and
workspace inventory. Grouped mode rejects a maximum of one and refuses a
singleton budget while multiple tokens remain. Only a final tail or explicitly
requested serial mode uses one. Cost estimates are not hard deadlines; actual
elapsed time, calibration/context drift and other live memory need accounting.

Caller duties include valid immutable typed allocations/extents and source/model/
recipe identities, successful unmodified plans/borrowed keys, truthful receipts,
correct live-memory/calibration inputs, linear cursor ownership and exclusive
disjoint writes. Whole-model request fencing, cancellation and publication remain
engine work. Owned causal-stack and original layer-0 MoE batch qualify CPU
components, not complete original-model prefill or learned attention.

Generated CUDA products/flags/publication execute on the RTX 4080 with exact
CPU parity: FP4/FP8 ragged tiles, numerical refusal and all three original routed
projection shapes. This qualifies the product primitive, not a complete GPU
expert FFN, model prefill or device performance.
`psionic/tests/run_deepseek_prefill_cuda.sh` is the strict device gate;
`--compile-only` does not imply execution.

Check: `psionic/tests/check_deepseek_prefill.sh` covers independent grouping,
measured-cost chunks, exact serial/layer-major causal state, bounded file reuse,
native debug/release plain/ASan/UBSan and original MoE evidence replay.
`psionic/check.sh --only deepseek/prefill --jobs 8` adds zero-survivor mutation.
Evidence and scope: [chapter 97 record](docs/verification/psionic-ch65-2026-10-29.md).

## Psionic exact hot-expert residency changes only between steps

`psionic/lib/deepseek/hot*.coax` owns `deepseek-hot-exact-cpu-v1`:
profile counts seed fixed-point heat while slots remain empty until copied.
A finished step adds 1,024 units per unique selected key; every 256th step
then floors 9/10 of all heat. Replacement requires `4*hot > 5*cold + 4096`.
Incoming ties choose lowest expert ID; victim ties choose lowest slot.
Each slot changes once per plan; evicted IDs wait until the next plan.
Frozen caches fill holes but retain populated slots. Integer placement heat
is its own deterministic policy, not the old engine's floating heat profile.

Active-step dispatch leases a stable map. Gate-selected next-layer misses can
prefetch into private storage without changing residency. Only idle state
accepts installation, and model/step/revision tickets, old occupants, complete
copy receipts and all proposed bindings validate before any public byte or
slot changes. Late failures preserve all public data/metadata; private work
may change. Finish stages the complete next heat and returns the next header.
This is a single-owner value continuation, not synchronization between callers.

Packets concatenate all six original routed FP4 code/scale spans losslessly.
The native adapter observes addresses and performs mapped/direct I/O; Coax owns
range, placement, arithmetic and publication. Every file read stages before
packet publication. The CPU expert adapter binds unchanged packets and calls
chapter 95's existing arithmetic. No route, route weight or combine order is
altered. Resident coefficient budget excludes private incoming/I/O work and
other model/OS/device allocations; the caller budgets those independently.

Callers own complete source/model/packet authentication, matching immutable
file and packet lifetimes, correct dimensions/identity, successful unmodified
plans and truthful completed-copy receipts, valid typed allocations/extents,
recorded uses, linear header ownership and exclusive disjoint writes.
Device arenas, GPU arithmetic, event fencing, overlap, complete model execution
and measured cache acceleration remain unqualified integration work.

Check: `psionic/tests/check_deepseek_hot.sh` covers independent phase-shift
traces, all heat/maps/plans, file/host/resident FFNs, original expert evidence,
43x384/116 metadata topology and native debug/release plain/ASan/UBSan.
`psionic/check.sh --only deepseek/hot --jobs 8` adds zero-survivor mutation.
Scope and source identities: [chapter 96 record](docs/verification/psionic-ch64-2026-10-29.md).

## Psionic Engram lookup binds the normalized map and reads only planned rows

`psionic/lib/deepseek/engram*.coax` owns `deepseek-engram-lookup-v1`:
original 129,280-ID tokenizer bytes derive 99,092 normalized classes, with the
complete little-endian map authenticated before use. Compressed vocabulary
size determines fixed per-layer multipliers. The original 48 distinct primes,
cumulative ranges and odd multipliers select 24 rows each at layers 1 and 14.
Three compressed history IDs, including dead markers, cross chunk boundaries.
A dead/missing lookback pads itself and every older lookback with ID 2.

Each selected row has 256 E4M3 codes and eight per-row/per-32 UE8M0 scales.
Independent 64-bit tensor extents and whole-file length are checked before
Coax publishes read ranges. C observes actual addresses/file length and performs
lazy read-only mapping or pread; it does not decide hashes, ranges or arithmetic.
Decoded BF16 words and hash IDs publish only after complete success. Late bad
rows or I/O failure preserve all public output and spare tails; private work
may change. The adapter allocates bounded range metadata, never whole tables.

Callers own valid immutable allocations/extents, successful unmodified map
metadata, authenticated immutable files and matching table/model/history
identity, FD lifetime and exclusive disjoint writes. File-size observation is
not authentication; changing/truncating a mapped file violates its lifetime.
Owned sparse files qualify bounded I/O at original shard sizes and offsets.
They do not qualify original embedding payloads, learned lookup projections,
gated residual injection, GPU behavior or complete model execution.

Check: `psionic/tests/check_deepseek_engram.sh` covers independent original
normalization/map, full-prefix hashes/history, range layouts, mmap/pread,
BF16 rows and native debug/release plain/ASan/UBSan failures. The unit
`psionic/check.sh --only deepseek/engram --jobs 8` adds zero-survivor authority
mutation. Actual evidence: [chapter 94 record](docs/verification/psionic-ch62-2026-10-29.md).

## Psionic compressed attention keeps source ownership and causal prefixes explicit

`psionic/lib/deepseek/sparse-*.coax` owns `deepseek-sparse-cpu-wide-v1` at
projected-input boundaries: BF16 query/window vectors, F32 compressor KV/gate
projections and BF16 norm/index weights. Ratio 2 pools per channel and carries
one unfinished F32 pair; ratio 1 is ungated. Completed rows normalize before
rotary placement. Index keys derive from the pre-RoPE latent; compressed KV
and index keys use their distinct chapter 98 grids. V4 keeps adjacent complex
pairs, with pure-window and compressed YaRN profiles and inverse output RoPE.

The pinned 43-layer map distinguishes four KV/index-key owners, eight indexer
sources and candidate source 20. KV receipts may cover a later prefix, but
selected-position/candidate receipts must name this query. Declared receipts
do not authenticate model identity. Visible lengths exclude unfinished groups,
future rows and unfilled ring slots. Unselected compressed values are not read.
Candidate selection pins the newest partial block. CPU score ties use lowest
positions, with selected IDs returned in position order.

The CPU attention profile uses compensated wide reductions and an ideal stable
softmax with sink denominator mass. It does not reproduce the GPU kernel's
FP32/online/BF16 probability-tile rounding. BF16 index operation boundaries,
cache grids and final BF16 publication are explicit. Learned projection
binding, original-device distributions and full-model execution remain in 68.

Every public primitive stages its complete result before publication. Chunk
compression also stages all latents and both canonical tails together; a late
failure preserves them and spare tails. Private work may change. Callers own
valid immutable allocations/extents, matching model/history/projection state,
correct unmodified successful metadata and exclusive disjoint writes. Row
stores are individually atomic; larger cache/model commits need private shadows.
C observes actual span addresses only. The library allocates nothing.

Check: `psionic/tests/check_deepseek_sparse.sh` covers independent prefix
equations, all 43 pinned layer positions, packed caches/candidates/selected
positions, real whole/split compressor chunks, original 32x128 index and
64x512/128-window shapes, native debug/release plain/ASan/UBSan and late
failures. `psionic/check.sh --only deepseek/sparse --jobs 8` adds zero-survivor
authority mutation. Actual scope: [chapter 92 record](docs/verification/psionic-ch60-2026-10-29.md).

## Psionic packed V4.1 caches reconstruct the reference's BF16 grids

`psionic/lib/deepseek/cache*.coax` owns `deepseek-cache-bf16-grid-v1`:
window KV is FP8 E4M3 with per-32 UE8M0 scales; compressed KV is low-first
FP4 E2M1 with per-16 positive E4M3 scales; index keys/queries are low-first
FP4 with per-32 UE8M0 scales. Whole post-RoPE vectors, tails included, round
at the pinned source's quantization sites. F32 scale/division/product and
nearest-even BF16 publication boundaries are explicit. Inputs are BF16 bits.
NaN, invalid scales and overflow refuse rather than changing finite values.

Pack stages both code/scale spans and unpack stages all BF16 words. Public
outputs and spare tails preserve their previous values on any failure; private
work may change. Caller duties include valid immutable allocations/extents,
matching format/model/history identity and exclusive disjoint writes. Native
C observes addresses only. The codec allocates nothing and does not own the
attention ring, rotary operation, compressors or sharing state.

Persistent payload and chunk work have separate enforced limits. A chunk may
exceed the persistent window row count. The pinned Flash inventory counts four
KV/index-key producers, eight query-producing indexers, forty backbone windows
and up to three draft windows. Its reusable codec work includes the larger
32-head index-query row. It excludes other model/device memory and alignment.

Check: `psionic/tests/check_deepseek_cache.sh` covers all BF16 input patterns in
three group forms, every encoded scalar/scale pair, BF16 midpoint neighbors,
native debug/release plain/ASan/UBSan, row-chunk parity, atomic failures and an
independent pinned ownership/work inventory. `psionic/check.sh --only
deepseek/cache --jobs 8` adds zero-survivor authority mutation. This qualifies
CPU source-equation bits, not an original GPU capture or complete attention;
see the [chapter 98 record](docs/verification/psionic-ch66-2026-10-29.md).

## Psionic V4.1 expert formats retain separate scales and pre-down route weights

`psionic/lib/deepseek/expert*.coax` owns `deepseek-expert-cpu-wide-v1`:
original low-first FP4 E2M1 routed bytes with UE8M0 scales per row/32 K,
and shared FP8 E4M3 bytes with UE8M0 scales per 32x32 block. All finite
code/scale pairs decode exactly to binary64, including signed zero and values
above F32 range. Scale 255 and FP8 codes 127/255 refuse. Activations round
to the original per-32 FP8 grid with nearest-even ties and exponent-rounded
UE8M0 scales. Stored projections/activations are F32; products/reductions use
compensated binary64 with checked output narrowing.

Up clamps both sides and gate only from above. The F32 SiLU/up result is
multiplied by the F32 route weight before down-input quantization/projection.
Mixture evaluates selected FP4 experts and adds one shared FP8 expert once.
Every expert's metadata is checked; unselected coefficient values are not
evaluated. Complete output and spare tail preserve their previous values on
any failure, including a late down/shared failure; private work may change.

Caller duties include trustworthy tensor/route identity, consistent shapes,
immutable borrowed bytes, valid typed extents/allocations and exclusive
disjoint writable storage. C observes native addresses and performs effects.
The library owns all format, validation, numerical and publication decisions.

Check: `psionic/tests/check_deepseek_expert.sh` assembles exact bit/grid and
independent FFN fixtures, native debug/release plain/ASan/UBSan, original
expert/6-of-384 MoE evidence replay and book output. The original configuration
selects six of 384; the component admits up to eight unique routed IDs.
`psionic/check.sh --only deepseek/expert --jobs 8` adds zero-survivor mutation.
Actual checks and original component scope: [chapter 95 record](docs/verification/psionic-ch63-2026-10-29.md).

## Psionic hyper-connections carry the previous pre-mix with all four streams

`psionic/lib/deepseek/hyper*.coax` owns `deepseek-hyper-cpu-wide-v1`:
four F32 residual streams, F32 coefficient projections and stored intermediates,
compensated binary64 products/reductions, RMS epsilon 1e-20 and coefficient
epsilon 1e-6. Combination uses source/destination axes, row softmax plus
epsilon, then one column normalization and 19 row/column pairs. Every
normalization denominator includes epsilon; finite iteration sums need only
match the declared equations, not perfect stochasticity.

Prepare computes current coefficients but collapses with the previous
sub-block's pre-mix. Finish expands the sublayer output and returns the current
pre-mix for the next sub-block. Initial pre is one-hot; final readout collapses
using the last returned carry. Boundary state contains both all four streams
and that carry. Outputs publish only after complete success, including late
overflow; private scratch may change. A failed finish returns no valid next
carry. Valid immutable allocations/extents, trustworthy parameter identity,
matching source-major stream layout, unmodified successful mixes, the
initial/returned carry and exclusive disjoint outputs remain caller duties.
C observes borrowed-span addresses only.

Check: `psionic/tests/check_deepseek_hyper.sh` assembles independent Decimal/
scalar fixtures, debug/release, native plain/ASan/UBSan, all seven handoffs
in a six-sub-block chain, final collapse and original-width projections.
`psionic/check.sh --only deepseek/hyper --jobs 8` adds zero-survivor mutation.
Actual checks/source identities and the component scope are in the
[chapter 93 record](docs/verification/psionic-ch61-2026-10-29.md).

## Psionic complete V2-Lite execution commits every layer together

`psionic/lib/deepseek/v2*.coax` owns `deepseek-v2-lite-cpu-wide-v1`:
original BF16 coefficients, F32 activations/caches and compensated binary64
products/reductions, with original V2 YaRN and full-softmax routing. Routed
weights apply after each down projection; the shared FFN is added once.
Compressed and expanded caches have distinct F32 boundaries and references.

Complete SHA-256 admission checks all original files and exact coverage of
the 5,291 tensor intervals before binding. Public model metadata borrows
public block/expert storage and immutable files, never temporary drafts.
Caller-supplied profiles require trustworthy pins. Every token stages all
layer cache rows, residual diagnostics, routes and logits before publication;
failure preserves all public spans. Private work may change. Valid typed
allocations/extents, immutable lifetimes, matching model/cache form,
sequential history and exclusive disjoint writable storage are caller duties.
C observes nested address overlap and performs effects only.

Greedy ties use lowest IDs; EOS, token/context limits and cancellation remain
distinct. Text support is the fixed original Hi prompt and an admitted token
byte table, bounded to 4,096 IDs/65,536 answer bytes. BOS and unmapped IDs
refuse; only a final typed EOS is removed. This is not a general V2 encoder.

Check: `psionic/tests/check_deepseek_v2.sh` covers complete owned prefix
references, admission/ownership, native debug/release plain/ASan/UBSan,
current-source original-answer decoding and lossless original-value replay.
`psionic/check.sh --only deepseek/v2 --jobs 8` adds zero-survivor mutation.
Original execution and arithmetic scope: [chapter 90 record](docs/verification/psionic-ch58-2026-10-29.md).

## Psionic latent attention stages new rows and keeps rotary profiles distinct

`psionic/lib/deepseek/mla*.coax` implements shared latent-attention CPU
equations with F32 activations/caches, BF16 up-projection coefficients and
compensated binary64 products/reductions. Compressed rows store normalized
latent plus one shared rotary key; expanded rows store per-head non-rotary
keys/values plus one shared rotary key. Queries/keys arrive already rotated.
The cache form, shape, model identity and sequential prefix must match.

Attention reads exactly the supplied immutable history prefix and returns a
separate pending row. Public output and pending row publish together only
after all heads/value projections succeed; history is never modified. Caller
work/scores may change on failure. This permits a full decoder to stage every
layer before committing a token. It is not itself a complete model decoder.

`v2-rope.coax` separately owns the original V2-Lite adjacent-pair permutation,
fixed YaRN frequencies (factor 40, original 4,096 positions, beta 32/1), and
the two 0.707 mscale factors in attention scaling. It does not reuse R1's
rotary layout. Rotary query/key outputs also publish together. Valid typed
allocations/extents, immutable inputs and exclusive disjoint writable spans
remain caller duties; C observes addresses only.

Check: `psionic/tests/check_deepseek_v2_math.sh --mutate --jobs 8` covers
independent full-prefix compressed/expanded fixtures and original-parameter
YaRN vectors through position 163,839, native debug/release plain/ASan/UBSan,
atomic failures and zero-survivor mutation. Scope and outstanding complete
V2 execution gates: [component record](docs/verification/psionic-deepseek-v2-math-2026-10-29.md).

## Psionic DeepSeek routers separate selection bias from combination weights

`psionic/lib/deepseek/router*.coax` owns `deepseek-router-cpu-wide-v1`:
F32 logits/biases with binary64 scoring and reduction, bounded to 384 experts
and eight selected experts/groups. V2 retains full-expert softmax mass; V3
uses sigmoid, selection-only bias and top-two group sums; V4.1 uses global
square-root-softplus, selection-only bias, its k>1 epsilon normalization and
route scale. Lowest-ID expert/group ties are explicit. This is a CPU equation
reference; original model/device arithmetic and upstream top-k tie order
require separate qualification. The correction vector is supplied, not trained.

Scores and biased scores publish together only after complete validation and
successful normalization. Failures preserve both public arrays; spare tails
remain untouched. Each result records expert and group cutoff margins and
excluded IDs, with explicit flags when no excluded candidate exists. Comparison
records retain both complete reports and separate ordered-ID agreement, set
replacement and expert-matched weight L1. Comparisons require unmodified
successful results for the same request/model/configuration and authenticate
no identity. The caller owns valid typed allocations/extents, immutable inputs
and exclusive disjoint outputs; native C observes addresses only.

Check: `psionic/tests/check_deepseek_router.sh --mutate --jobs 8` covers an
independent Decimal/sort oracle at all original expert dimensions, tiny
probabilities and near ties, exact boundary checks, native debug/release
plain/ASan/UBSan and exact book output. See the
[chapter 91 record](docs/verification/psionic-ch59-2026-10-29.md).

## Psionic resident decisions have bounded lifecycle and private measurements

`psionic/lib/serve-run.coax` owns Part III's execution-payload admission,
observed-time statistics, private metrics, readiness and bounded restart
policy. C performs file/socket/clock/allocation/process/device effects.
One authenticated session and reserved arenas serve one active request.
Device execution allocates no redundant CPU model-state/work arenas; its
device/pinned/mapped payloads remain separate from the host execution budget.

Verified-file evidence is attached through `fstat` of the same descriptor
that was mapped. Path replacement cannot bind another inode to those bytes.
Local cached records are trusted prior evidence, not signatures. Checkpoint
files must stay immutable for the complete borrowed session lifetime.
`mincore` observes file-page warmth independently of verification-cache reuse.

CPU and device snapshots belong to one request. A hit needs the authenticated
identity and every prefix token to match; the retained prefix is separate
from mutable suffix state. Request boundaries clear metadata. One-question
device requests keep the joint path. No cross-request/tenant cache is implied.
Device safe points refuse busy connections; the bounded socket cleanup pool
never queues model work. Cancellation publishes no answer and the next request
starts fresh. Driver and caller lifetime obligations remain unchanged.

Readiness follows authentication, disjoint buffers and three retained golden
decisions under the pre-existing tolerance. Default restart policy allows five
restarts with 250 ms exponential backoff capped at ten seconds; owner stops
and clean exits do not restart. The native systemd readiness effect follows
the same gate. The first deployment is loopback-only and not boot-enabled.
Private metrics contain identities, outcomes, counts and clocks, never state,
question IDs, option/answer text or probabilities. They authorize no payment;
HTTP usage remains null. Peak process RSS is not aggregate host/GPU usage.

Check: `psionic/check.sh --only live-decision` runs focused Coax checks,
native debug/release plain/ASan/UBSan, transport/lifecycle and descriptor-race
checks and exact book outputs. Actual original Metal operations and managed
CUDA crash/re-warm/restart, paired measurements, producer identities and the
owner-requested omission of full mutation and original CPU inference are in
the [Part III record](docs/verification/psionic-partIII-2026-10-29.md).

## Psionic Responses serving preserves typed state and exclusive GPU access

`psionic/lib/responses/` owns `psionic-gptoss-responses-v1`: bounded HTTP/JSON
admission, bearer authentication, flat tool schemas, stateless Harmony mapping,
response lifecycle and semantic SSE. Original 20b generation uses the existing
resident CUDA profile, one exclusive request, 2048 positions and greedy output
limits. C owns sockets/clocks/threads/device effects; no tool executes here.
The fixture generator is absent from the production import root.

Raw reasoning is transported as a typed reasoning item, separately from final
text. Current tool-turn analysis survives replay; completed prior analysis is
removed. Provisional typed API text is streamable, while the CLI's complete-final
display gate remains unchanged. Argument deltas and incomplete calls grant
nothing. Every well-formed addressed record is a function-call item, whichever
channel carried it (`functions.NAME` and bare `NAME` name `NAME`; a dotted
built-in such as `browser.search` keeps its name and replays verbatim). A call
naming an admitted tool must satisfy its schema; any other name is surfaced
for the client to refuse and is never granted or run here. With function tools
the system message names the commentary channel, as the Harmony reference does.
Unsupported capabilities refuse before model effects; malformed model output
fails rather than fabricating success. This profile is not universal
Open Responses compliance or constrained GPU decoding.

HTTP bytes, item/node/UTF-8 work and event buffers are bounded. Serialization
state commits after a complete batch fits; failed sends cancel rather than
retry partial events. Disconnect/deadline decisions run between tokens and
committed prefill chunks. Earlier chunks remain committed; the next request
resets the cache. A device failure makes the worker unavailable. Immutable
admitted artifact/table lifetime, truthful native extents and exclusive spans
remain caller duties. The listener is loopback-only and logs no request content.

Check: `psionic/tests/check_responses.sh` covers owned HTTP, typed replay, SSE,
debug/release and native plain/ASan/UBSan. The existing device harness additionally
checks prefill cancellation preserves committed state. SDK/schema and actual
original GPU evidence, numerical scope and operational limits are in the
[Responses record](docs/verification/psionic-responses-2026-10-29.md).

## Psionic DeepSeek front-door boundaries survive tokenization and streaming

`psionic/lib/deepseek/front*.coax` owns `deepseek-front-v1`: a bounded
V4.1-style text recipe and DSML stream protocol, plus an explicitly separate
R1 tokenizer adapter. Ordinary message/tool strings never become control IDs.
Adjacent plain fragments join before BPE; only typed specials split runs.
R1 has no system/DSML control mapping and refuses those boundaries. Its
existing chapter 88 history/decoder contract is unchanged.

`front-v41*.coax` adds `deepseek-front-v41-text-v1`: the pinned original
129,280-entry V4.1 vocabulary and ranked byte-BPE merges, three ordered isolated
splits, Unicode 16 properties and no normalization. Only eight explicitly typed
controls map to original IDs; every other added ID refuses. Ordinary marker
spellings remain byte-BPE text. An ordinary run may span borrowed fragments,
including incomplete UTF-8 fragments, but the rejoined run must be valid UTF-8.
Each real control starts a new independently encoded run. Complete tokens/units
publish only after success. Caller-owned work may change on failure. Exported
low-level span probes require a validated UTF-8 slice, an in-range scalar start
and successful unmodified layout; checked text entry points establish these.

Callers must admit immutable derived tokenizer bytes against the committed
source/packed identities before use; the native gate uses Coax `pw_digest`.
Typed allocations/extents, disjoint writes and borrowed text lifetimes remain
caller duties. The original tokenizer and bounded recipe/actual-token stream
path qualify on CPU. Complete upstream recipe options, namespaces, tool schema
rendering/call-ID sorting, media and V4.1 model execution remain separate work.

Streaming consumes typed bytes/controls once, retaining only a fixed delimiter
window and incomplete UTF-8 scalar. It emits reasoning, content, tool names
and escaped string/raw argument fragments separately. Malformed framing and
UTF-8 refuse; raw JSON validity, schema checks and tool grants belong to a
consumer. A closed frame grants nothing. EOS requires a separate normal finish
event; transport EOF and truncated syntax remain incomplete. Failed feeds
preserve public events and return the supplied state; successful prior chunks
remain committed. Caller-owned staging may change. The caller supplies valid
types/extents, admitted immutable tokenizer data, live borrowed strings,
exclusive disjoint writable storage and only initial/returned parser states.

Check: `psionic/tests/check_deepseek_front_v41.sh` covers original V4.1 text,
whole-vocabulary and Unicode digests, bounded recipe IDs, actual-token stream
splits and native debug/release plain/ASan/UBSan. Its focused mutation unit is
`psionic/check.sh --only deepseek/front-v41 --jobs 8`. Evidence:
[CPU V4.1 tokenizer record](docs/verification/psionic-ch57-v41-2026-10-30.md).

Check: `psionic/tests/check_deepseek_front.sh` covers owned protocol oracles,
all 32,896 split pairs, all truncations, original BPE vectors, native
debug/release plain/ASan/UBSan and book output. `psionic/check.sh --only
deepseek/front --jobs 8` also runs zero-survivor authority mutation. Scope
and evidence: [chapter 89 record](docs/verification/psionic-ch57-2026-10-29.md).

## Psionic DeepSeek R1 distill owns a separate dense CPU profile

`psionic/lib/deepseek/` implements `deepseek-r1-qwen2-cpu-wide-v1` for the
pinned original R1-Distill-Qwen-1.5B BF16 artifact. Complete original-byte
SHA-256 admission binds the reviewed 339-tensor header and intervals;
the shared typed binder additionally checks complete payload coverage,
unique names, disjoint ranges and every expected shape before publishing.
Caller-supplied profiles require trustworthy pins. Borrowed immutable files,
model/layer storage and their lifetime remain the caller’s responsibility.

The decoder has learned RMS scales with epsilon 1e-6, biased Q/K/V,
full split-half RoPE at theta 10,000, causal GQA, ordinary SiLU gate/up
products and an independent vocabulary head. Products/reductions use the
wide CPU profile with F32 stored activations. Every token stages all new
KV rows, residual diagnostics and logits; failures preserve every public
cache/output span. Private work may change. The caller owns sequential
history, valid allocations, truthful extents, immutability and exclusive
access; native C observes nested address overlap and performs effects.

The separate original byte-BPE table reuses Unicode-17 NFC DATA and ranked
merging, with the original letter-only split profile. Typed text history
admits an optional first system message, alternating user/assistant turns
and a final user, then inserts tokenizer BOS 151646 once, rather than the
config’s conflicting 151643. Prior assistant reasoning is stripped after
its last closing marker. Ordinary strings cannot insert control IDs.
Public text/token outputs publish atomically. Decoding refuses model IDs
with no tokenizer entry. Reasoning and final intervals remain separate;
EOS, token/context limits and between-step cancellation remain distinct.
There is no tool execution, sampling, GPU, service or V4.1 claim.

Check: `psionic/tests/check_deepseek.sh` covers owned two-/three-layer dense
oracles, original tokenizer/recipe vectors, native debug/release plain and
ASan/UBSan checks, exact replay and book output. `psionic/check.sh --only
deepseek --jobs 8` also runs the authority mutation roots. The retained
original full-model comparison and current-source original slices, their
actual build identities and limitations are in the
[chapter 88 record](docs/verification/psionic-ch56-2026-10-29.md).

## Psionic original gpt-oss GPU sessions retain model state on device

`psionic/lib/gptoss/device-*.coax` owns `gpt-oss-cuda-f32-v1`: original
BF16/MXFP4 coefficient bytes relocate unchanged into aligned resident intervals,
f32 lane products and ordered f64 reductions feed the complete CUDA decoder.
Single-token and batched packed products use admitted tensor-relative u32
offsets with unchanged lane/fold arithmetic and view bounds.
Decode attention reuses the admitted 64-query/8-KV layouts. Decode softmax
keeps its ordered per-head f64 denominator in no-longer-live norm scratch, then
performs each original f64 division independently in the next ordered kernel.
There is no reciprocal substitution or change to sink mass/reduction order.
Coax admits shapes, byte budgets, tensor-relative u32 index bounds, all kernel
views and the ordered launch program; C performs effects. The native session
has no automatic CPU fallback. Authentication and upload happen once.
Synchronous decode may replay an ordered CUDA graph of that admitted program.
Coax compares semantic descriptors and lists dynamic node updates or a topology
rebuild. Each node depends on its predecessor; immutable bindings retain their
session lifetime. CPU emulation, asynchronous submission and per-kernel
instrumentation use ordinary launches. A control download waits for the step
before the next graph update; closing destroys graphs before buffers/modules.

Single-token steps publish cache/logits/diagnostics only after all numeric
flags succeed, then advance position. Batched prefill uses causal per-query
history, stable expert token groups and a private full cache shadow. Each
chunk publishes together, including wrapped window entries; earlier successful
chunks remain committed if a later chunk fails. Caller-supplied sequential
history, immutable admitted files, valid allocations and exclusive session
access remain required. Driver failures poison the session. Greedy device
generation uses Harmony framing and the shared final-only display gate;
sampled device generation, speculation and tools retain separate gates.

Check: `psionic/tests/check_gptoss_device.sh` uses focused owned fixtures,
debug/release, native plain/ASan/UBSan and exact book output. CUDA/original
runs are explicit opt-ins; original answers compare with retained independent
CPU vectors, never routine full-original CPU inference. Full mutation roots
remain excluded by the owner's request. Scope, hardware and measurements:
[device record](docs/verification/psionic-gptoss-device-2026-10-29.md).

The same programs and kernels run on an Apple GPU (chapter 72) under
`gpt-oss-metal-f32-ff-v1`: `coax metal-compute` emits `device-kernels.coax`
unchanged, and `psionic/platform/gptoss-metal.c` implements the same `pg_*`
interface through `psionic/platform/metal-host.m`. It decides nothing. Every
original weight is resident in shared memory, and nothing streams. There is
no CPU fallback: without a device the result is `PC_ABSENT`. A failed command
buffer or a failed in-kernel Coax check poisons the session. The f64
reductions are APPLE-FF-v1 float-float pairs, and the YaRN table is built on
the host in binary64. CUDA graph replay has no Metal counterpart; each program
is one ordered command buffer. Check: `psionic/tests/run_gptoss_metal.sh` runs
the tiny model by default. With the original files it applies chapter 66's
frozen 0.0005 rule and exact routes and top token. `PSIONIC_CPU_GREEDY=1`
requires the GPU's greedy token IDs to equal chapter 64's CPU engine's, and
`PSIONIC_DEVICE_QUALIFY=1` repeats layers 0 and 1 and the readout on the CPU,
launch by launch. [Metal record](docs/verification/psionic-gptoss-metal-2026-10-29.md).

## Psionic original gpt-oss execution and constrained continuations

`psionic/lib/gptoss/` owns `gpt-oss-cpu-wide-v1`: fixed independently pinned
20b/120b original BF16/MXFP4 profiles, complete SHA-256 admission and exact
parameter intervals, compensated f64 products with f32 storage, all-layer
staging and full-vocabulary readout. A model step commits cache and public
diagnostics only after complete success; work may change. Even-layer cache
storage stays window-bounded, while odd layers reserve the chosen full prefix.
The caller supplies model identity, immutable live inputs, sequential history,
truthful extents, valid allocations and exclusive disjoint writable storage.
C observes addresses and performs effects, never model or grant decisions.

Counter-based SplitMix64 sampling has explicit seed/counter progression,
lowest-ID ties, temperature/top-k/top-p and pre-choice masks. Return, call,
limit and between-step cancellation are distinct. Model and emission are
separate transactions. Speculation uses standard acceptance/positive residuals,
an all-match bonus and complete snapshots including overwritten ring entries.
Its serial CPU measurements do not establish acceleration or a batched backend.

`gpt-oss-json-enum-v1` is a finite root-object enum schema profile, not a
universal schema compiler. Vocabulary masks enforce prefix membership before
sampling; only a complete argument may offer call. A named commentary handoff
still needs the host's grant. Tool results are typed messages. Only recipient-free
assistant final turn bodies pass the display gate; analysis never does.
Shared Harmony record/identifier types are independent of tokenizer execution.

Check: `psionic/tests/check_gptoss.sh` runs focused changed-code checks, native
plain/ASan/UBSan and exact book transcripts. Full mutation reruns were stopped
at the owner's request on 2026-10-29; mutation slicing is separate work. Opt-in
original runs and their exact build/scope limits are in the chapter
[64](docs/verification/psionic-ch45-2026-10-29.md),
[73](docs/verification/psionic-ch46-2026-10-29.md),
[75](docs/verification/psionic-ch47-2026-10-29.md) and
[77](docs/verification/psionic-ch48-2026-10-29.md) records.

## Psionic Harmony text separates ordinary bytes from control IDs

`psionic/lib/harmony/` owns `gpt-oss-harmony-text-v1`: the pinned original
o200k byte ranks, a separate case-sensitive split pattern and Unicode-17
properties, without NFC normalization. The original-derived HST7 table is
SHA-256-admitted once and retained immutable; layout parsing is not identity
admission. The existing byte-BPE loop is shared through an independent pair
index and whole-piece lookup. Ordinary text cannot introduce formatting IDs.

Typed messages preserve supplied history. The three reasoning efforts use the
pinned default system template. Complete parsing recognizes all supported
roles/channels and recipients on either side of a channel; completion mode
continues past message ends and consumes the first return/call, leaving trailing
tokens untouched. Only assistant messages may return or call, with a named
recipient required for calls. Unsupported extensions and malformed input refuse.
Public token arrays, decoded bytes and byte-interval records publish atomically;
workspaces may change. No tool execution or display policy is granted.

Bounds are 64 messages, 16,384 IDs, 65,536 logical decoded bytes, 64-byte ASCII
identifiers, 256-byte headers, 1,024-byte pieces and a 16,777,216 sum-of-squares
piece budget. The model profile ends at ID 201,087; generic Harmony
ID 201,088 is outside it. Valid allocation/extents/alignment, input/table
immutability, lifetime and exclusive writable access remain caller duties.
C observes nested overlaps only. Incremental parsing, optional header and
multimodal extensions and whole-model execution remain separate gates.

Check: `psionic/tests/check_harmony.sh` and the main `psionic/check.sh` cover
owned original-rank/regex/heap oracles, debug/release, zero-survivor authority
mutation, native plain/ASan/UBSan and exact book output. See the
[chapter 63 record](docs/verification/psionic-ch44-2026-10-29.md).

## Psionic gpt-oss attention preserves sink mass and visible cache history

`gptoss-attention.coax` and `gptoss-math.coax` implement
`gpt-oss-attention-cpu-wide-v1`: 64-wide heads, 1–64 query/1–8 KV heads,
learned per-query-head sink logits in the denominator with no value, split-half
YaRN and a ring bounded by its sliding window. Sliding capacity equals the
window, at most 128, with absolute positions 0–131,071; full causal capacity
is separately bounded to 128. Only visible old cache entries are read.
Shape, position, exact input spans, writable capacities, address observation
and finite data are checked before publication. Rotation failure preserves
public cache and output; work may change. Success leaves unused public tails
unchanged. Caller duties include valid aligned allocations, truthful extents,
immutable inputs, exclusive access and matching shape/model/sequential history.

Math helpers stage split-half rotation and bounded biased projection; the
interleaved expert activation validates all pairs before writing and uses an
upper-only gate clamp, two-sided linear clamp, alpha 1.702 and a plus-one
linear term. Clamp diagnostics count changed lanes. Wide CPU arithmetic with
f32 storage is separate from original Torch/BF16 parity. Original checkpoint
binding, full-size projections, block composition and generation remain
separate qualification gates.

Check: `psionic/tests/check_gptoss_attention.sh` and the main
`psionic/check.sh` include independent scalar references, debug/release and
zero-survivor mutation plus native plain/ASan/UBSan gates. See the
[chapter 52 record](docs/verification/psionic-ch36-2026-10-29.md).

## Psionic bounded top-k mixtures preserve route and output semantics

`psionic/lib/moe.coax` implements `cpu-topk-affine-v1`: 1–128 finite logits,
1–4 selected experts, descending scores and lowest-ID ties, selected-only
softmax with wide accumulation and f32 coefficients. Tiny affine mixtures
have width 1–16, unique resident IDs and exact row-major matrix/bias extents.
Every resident entry counts against a logical payload budget; only selected
parameter values are numerically evaluated. Missing selected experts refuse
instead of changing the route. Output and the successful route publish only
after every selected expert and the bounded combination succeed; spare output
capacity is unchanged. This is not original BF16 gpt-oss arithmetic, a complete
expert layer, model identity admission or OS memory reservation. The native
adapter observes nested overlap; valid allocations, extents, alignment,
parameter identity, immutable lifetimes and exclusive access are caller duties.

Check: `psionic/tests/check_moe.sh` and the main `psionic/check.sh` include
independent Decimal fixtures, debug/release checks, zero-survivor mutation
and native plain/sanitized gates. See the
[chapter 51 record](docs/verification/psionic-ch35-2026-10-29.md).

## Psionic original MXFP4 decoding admits complete finite windows

`psionic/lib/mxfp4.coax` decodes the gpt-oss original-format low/high nibble
pairs with one scale per 32 values. The `gpt-oss-mxfp4-f32-v1` profile admits
positive complete rows, at most 1,048,576 output values and scale bytes 0–252.
It validates all counts and scales and observed overlap before any output
write. Admitted values are exact finite binary32, preserving signed zero and
subnormals; spare output capacity is unchanged. This decoder does not
inspect or authenticate files. Callers supply the correct tensor pairing,
shape, provenance, valid disjoint storage, alignment, lifetime and exclusive
access; the C adapter only observes addresses. Original checkpoint coverage
and other numerical/back-end profiles require separate evidence.

Check: `psionic/tests/check_mxfp4.sh` and the main `psionic/check.sh` cover
Coax checks, zero-survivor mutation, independent scalar bit fixtures and
native debug/release plus sanitizer checks. See the
[chapter 50 record](docs/verification/psionic-ch34-2026-10-29.md).

## Psionic registered operators retain typed IR semantics and code leases

`psionic/lib/operator-registry.coax` admits bounded unary same-shape f32
schemas, disjoint writes, versioned names, optional input VJPs and pointwise
batching. Registered IR tag 11 requires an attached admitted registry; absent
hooks or unknown IDs refuse before execution. Coax checks shapes, colocated
operands, backward capability and code/schema identities. Native hooks bind
trusted Coax CPU callbacks and generated GPU entries inside the existing
command stream; no operator-specific formula lives in core C dispatch.

Registry fingerprints include name/entry lengths and bytes, versions,
properties, compiler and supplied source/module digests. They are part of
`TiTarget.operators`. Compiled plans freeze registration tables through Coax
leases; derived plans inherit independent registry/module leases, and owners
cannot close early. Operator authors retain their declared memory/numerical
contract obligations; callback provenance is trusted producer metadata, not
sandboxing or attestation. See the focused `operator-registry` gate and
[chapter 40 evidence](docs/verification/psionic-ch26c-2026-10-29.md) for actual
CPU/CUDA/Metal inference, finite-difference VJPs, compiled MLP, batching and
scalar training. `operator-registry.coax` and `examples/operators/squared-relu.coax` are gated by
root-only mutation with zero survivors (#12698).

## Psionic symbolic templates share code and specialize guarded plans

`psionic/lib/tensor-template.coax` resolves up to four guarded integer symbols,
a declared two-way branch and bounded unary unrolling, admitting at most 64
variants including control bindings. Every variant is checked at open. Input
storage/strides/offsets are derived as packed bindings; explicit view nodes own
striding semantics. Cached keys include shape and control bindings inside a
copied template and fixed compiler/backend/module identity. Coax chooses four
LRU slots and aggregate payload-budget evictions, and installs only successful
native specializations. Evictions can survive an allocation failure; outputs
remain unpublished. CUDA/Metal specializations borrow one compiled module;
Coax leases refuse unloading its owner before children close. No new backend
compilation occurs on a shape miss. Synchronous execution and numerical/driver
failure rules remain 38's. See the focused `tensor-template` runner and
[chapter 39 evidence](docs/verification/psionic-ch26b-2026-10-29.md). Full
mutation runs are omitted under the owner's explicit instruction.

## Graph-function transforms preserve formal calculus and atomic outputs

`psionic/lib/function-*.coax` transforms immutable permanent tensor-IR graphs,
not arbitrary Coax procedure values. The fixed profile admits CPU F32 primals
and formal F64 partials through order two; it does not differentiate the literal
rounded floating-point map. ReLU has zero derivative/curvature at zero.
Registered operators, device differentiation and third-order requests are
refused. Input nodes sharing a flat input coordinate contribute to the same
adjoint; strided/view/gather reads keep their exact root mapping.

All public execution admits metadata, caller-declared complete overlap facts,
finite used inputs, capacity and a declared nominal work ceiling before public
publication. A later numerical or batch-row refusal preserves every public
output and spare tail; private scratch may change. Batching reads selected
shared/mapped coordinates only. Raw helpers require an admitted layout and
fitted, disjoint work. Fixed capacities and the nominal 67,108,864-unit ceiling
are not compiler `#bounded`, a hardware deadline or a general ownership proof.
The independent native receipt is `psionic/fixtures/function-native-qualification.json`.

## Psionic tensor IR plans preserve SSA roots and publish whole results

`psionic/lib/tensor-ir*.coax` owns `tensor-ir-f32-v1`: an explicitly authored,
32-node straight-line graph, rank-0–4 f32 values and declared placement. Unknown
operations, non-f32 dtypes, foreign placement, future operands, invalid views,
noncontiguous reshapes, gather indices, scalar work and buffer budgets refuse
before tensor execution. Every materialized node owns a fresh logical root;
views alias read-only root-relative storage. Last-use analysis follows aliases
and pins the chosen output until publication. A slot can be reused only after
its root's last use is strictly before the current command, never over an
operand of that command. The full lowered command stream and persistent
workspace are owned by an opaque native object; there is no hidden CPU fallback.

The CPU reference uses chapter 5's compensated f64 operator procedures and
wide LayerNorm equations. Planned CPU/CUDA/Metal use the same sequential f32
Coax arithmetic and distinct per-command numerical flags. Reused workspace
never erases earlier failures. A final logical gather stages the output;
publication first checks every flag, preserving caller output on all refusals.
Native address observations reject overlap with inputs or owned metadata and
host buffers. Host access is exclusive and synchronous. Transfer failure paths
attempt to drain submitted work; numerical failures permit reuse, driver
failures poison further replays, and an idle poisoned object can be closed.
Coax owns those lifecycle decisions; C performs allocation and driver effects.

Cache matching compares active graph descriptors exactly, including shapes,
strides, bindings, dtype and placement, and target/compiler/backend/module
identities. The trusted caller supplies the qualified compiler digest; Coax
hashes actual module bytes and observed backend descriptions. Objects copy
their graph and never execute a different caller descriptor on a stale plan.
This is process-local identity, not execution attestation or a persistent cache.

Check: `psionic/check.sh --only tensor-ir` and explicit CUDA/Metal runners;
authored debug/release, independent Decimal(100) graph oracles, observed
allocation counts/bytes and native debug/release ASan/UBSan. GPU memory is not
instrumented by host sanitizers. Metal uses the qualified release emission;
the failed checked candidate is recorded in
[chapter 38 verification](docs/verification/psionic-ch26a-2026-10-29.md).
`tensor-ir.coax` and `tensor-ir-host.coax` are gated by root-only mutation with
zero survivors, and `tensor-ir-kernels.coax` has zero under the host's checks
(#12698). Metal requalification of the changed kernels is an owner action.
Automatic graph extraction, dynamic shapes, narrow dtype IR execution, direct
`PtdTensor` binding, fusion and differentiation remain separate work.

## Psionic device kernels and plans own the device arithmetic and schedule

`psionic/lib/device/project.coax`, `layer.coax` and `warp.coax` are `coax cuda` kernels:
each output element belongs to exactly one work item, so a result does not
depend on scheduling, and the same source compiled to C reproduces the device
bit for bit except where the device math library's `exp` or `log` differs
or, in `warp.coax`, the tensor cores round f32 partial sums differently from
the Coax meaning of `cu_mma_bf16_16x8x16`; those are compared within frozen
tolerances.
`plan.coax` decides the workspace layout, which original weights stay resident
and which stream through two slots, and every copy, wait, release and launch;
`psionic/platform/cuda-host.c` executes those effects and decides nothing.
Authentication, the session's borrowed weights and their lifetime remain the
CPU session's (chapter 14); the host must keep a slot's previous launches
complete before refilling it, which the plan's wait and release operations
express. Check: `psionic/tests/check_device.sh` covers debug/release checks,
zero-survivor mutation, CUDA emission and the device projection; the
[chapter 37](docs/verification/psionic-ch26-2026-10-29.md) and
[chapter 44](docs/verification/psionic-ch30-2026-10-29.md) records hold the
device evidence and limits.

The same kernels run on an Apple GPU through `coax metal-compute`
(chapter 47): `psionic/platform/elephant-metal.c` performs the plan's
launches through `psionic/platform/metal-host.m` and decides nothing. On
unified memory every weight is resident; a plan that would stream is refused,
never silently degraded. There is no CPU fallback: no Metal device is
`PM_ABSENT`, and a launch in which any Coax check failed is `PM_CHECK` and its
results are discarded. Apple GPUs have no binary64, so `f64` is a float-float
pair (APPLE-FF-v1) whose arithmetic and `exp`/`log`/`sqrt` are qualified
against binary64 within frozen bounds (`psionic/tests/run_metal_kernels.sh`),
and the whole decision meets chapter 44's frozen 0.0005 rule against the CPU
reference (`psionic/tests/run_elephant_metal.sh`); the
[chapter 47 record](docs/verification/psionic-ch31-2026-10-29.md) holds the
evidence.

## Psionic complete model mapping publishes the layer stack together

`psionic/lib/model-io/model-map.coax` binds the authenticated inventory's
complete hybrid schedule, stages decoded parameter vectors and rebases layer
views onto caller-owned storage. Public layers and parameters publish only
after all layers bind; scratch may change on failure. Authentication, valid
disjoint allocations, immutable file bytes and lifetimes remain caller duties.
Check: `psionic/check.sh` covers authority mutation and native plain/sanitized
checks; the [model-map record](docs/verification/psionic-model-map-2026-10-29.md)
records the scope and original-file evidence.

## Psionic request blocks start fresh and publish atomically

`psionic/lib/qwen-request.coax` executes a caller-specified token stream in
prefix and suffix blocks, with a fresh linear state and deterministic rotary
positions. Live state and probabilities publish only on success; scratch may
change. Authentication, tokenizer and split identity, slots, allocation,
alignment, lifetimes and exclusive access remain caller obligations. Check:
`psionic/check.sh` covers authority mutation and native plain/sanitized checks;
the [request record](docs/verification/psionic-request-blocks-2026-10-29.md)
describes the tested cases and limits.

## Psionic original layer mapping binds validated views

`psionic/lib/model-io/layer-map.coax` maps the fixed hybrid schedule by exact
name, BF16 shape and finite decoded vectors. It stages owned parameters and
borrows matrices from caller-owned immutable files. Authentication of the
inventory, valid disjoint storage and its lifetime remain caller obligations.
Failed mapping does not publish a layer; scratch may change. Check:
`psionic/check.sh` covers Coax authority mutation and native plain/sanitized
checks; the [layer record](docs/verification/psionic-layer-map-2026-10-29.md)
describes original-file evidence and limits.

## Psionic original sessions and typed text decisions

`psionic/lib/decision-session.coax` admits a pinned target, authenticates the
original artifacts and packed tokenizer, binds the exact inventory and maps
the complete text stack. Names, tensor bytes and bound layer storage remain
borrowed and immutable for the session lifetime. `decision-prepare.coax`
turns one typed request into a staged token stream and a safe shared-prefix
boundary; the caller must associate that result, the tokens and option names
with the authenticated session. `decision-answer.coax` uses pinned slot IDs,
resets request state, runs prefix and suffix and maps the chosen index back to
an option. The caller supplies capacity, temperature and valid, disjoint,
live storage; native address checks do not establish allocation validity or
exclusive ownership. Workspace can change on failure, but session, request
output, answer probabilities and live state publish only on success. The text
profile does not admit images or provide a service or calibration.

Check: `psionic/check.sh` covers debug/release checks, authority mutations,
book examples and native plain/sanitized adapters. The chapter 14 verification
record describes the independent original-weight comparisons and limits.

## Psionic decision plans size caller storage for one pinned request

`psionic/lib/decision-plan.coax` derives a pinned target's session workspace,
state and request workspace from its own dimensions and shards, using the same
layouts `ps_open` and `ps_answer` check. It refuses unknown targets, unbounded
capacity and token or prefix counts that the request would refuse. A plan is
arithmetic, not an allocation or an admission of host memory: checkpoint
mapping, page cache and allocator behavior remain the caller's concern. Check:
`psionic/check.sh` covers authority mutation and native plain/sanitized checks;
the chapter 15 verification record gives the measured Mammoth run.

## Psionic token blocks stage model state through full-vocabulary readout

`psionic/lib/qwen-tokens.coax` gathers original-format embedding rows, runs
caller-bound hybrid layers, applies the final offset RMS norm and projects all
vocabulary rows in chunks before reading the requested slots. It stages state
until readout succeeds; errors preserve public state and probabilities. The
caller owns authenticated immutable weights, complete layer mapping, token and
slot identity, rotary positions and state history. `qwen-rope.coax` supplies
the pinned 64-dimensional text-only Qwen rotary coefficients. Native adapters
observe addresses, while valid allocations, extents, alignment, lifetimes and
exclusive access remain caller duties. These primitives alone do not implement
a complete request or a service.

Check: `psionic/check.sh` includes debug/release authority mutation and native
plain/sanitized checks, including a vocabulary larger than one projection
chunk. Original model and independent runtime qualifications are recorded
separately.

## Psionic decision-model targets use pinned original identities

`psionic/lib/decision-models.coax` exposes only the pinned Elephant and Mammoth
text profiles, including exact revisions, sidecar and shard identities,
architecture dimensions and bounded state capacity. The target metadata alone
does not authenticate supplied bytes or qualify execution. Bundle admission
must check every original and callers retain valid immutable storage.

Check: `psionic/check.sh` includes zero-survivor authority mutation and native
plain/sanitized checks; see the [target record](docs/verification/psionic-targets-2026-10-29.md).

## Psionic rank-five bundles bind complete original identities

`psionic/lib/model-io/rank5-bundle.coax` authenticates caller-pinned config,
tokenizer, index and every shard, validates all rank-five inventory metadata
and binds the complete caller-supplied tensor manifest before publishing it.
The sharded index must map each tensor exactly once and its declared total
bytes and optional total parameters must match the scanned originals. The
caller supplies trustworthy pins and keeps borrowed files and names immutable
and alive; C observes address ranges only. The rank-four v1 contract remains
separate.

Check: `psionic/check.sh` includes zero-survivor authority mutation and native
plain/sanitized tests; the [bundle record](docs/verification/psionic-original-bundles-2026-10-29.md)
distinguishes original-file evidence from model execution.

## Psionic rank-five original inventory and tensor access

`psionic/lib/model-io/rank5.coax` validates every BF16/F32 tensor, retaining up
to five dimensions and requiring unique names, disjoint ranges and complete
payload coverage. Public inventory is published only after the entire file is
valid. `access.coax` checks an expected name, dtype, shape, shard and byte range
before returning an original byte view; finite bounded decoding stages output.
Inspection does not authenticate the file. Callers must authenticate and retain
immutable originals and metadata, and provide valid, disjoint writable storage.

Check: `psionic/check.sh` includes authority mutation and native plain/sanitized
checks. See the [rank-five record](docs/verification/psionic-rank5-2026-10-29.md)
for exact evidence and the separate bundle-authentication boundary.

## Psionic text rotary positions are deterministic and staged

`psionic/lib/qwen-rope.coax` generates text-only Qwen rotary coefficients with
64 rotated dimensions and theta 10,000,000. It checks positions and capacities,
requires disjoint writable buffers and stages coefficients before publication.
Caller pointer validity, extent, alignment and exclusive access are obligations.
This does not implement image-position rotary semantics.

Check: `psionic/check.sh` includes zero-survivor authority mutation and native
plain/sanitized reference checks. See the
[hybrid executor record](docs/verification/psionic-hybrid-executor-2026-10-29.md).

## Psionic hybrid execution publishes a complete block

`psionic/lib/qwen-model.coax` runs a caller-supplied ordered layer sequence.
Linear layers process tokens causally and full-attention layers process the
whole block against their live prefix. Coax validates matching state and layer
shapes and stages activations, recurrent/convolution state and KV additions;
a failed layer cannot publish partial output or state. Unused KV tails remain
unchanged. Weight mapping, model identity and rotary positions remain caller
obligations. The C adapter observes nested spans only; valid pointers, extents,
alignment, immutable weights and exclusive access remain caller obligations.

Check: `psionic/check.sh` includes independent nonzero linear and cached
full-attention references, authority mutation and native plain/sanitized tests.
This bounded executor alone does not establish complete model execution.

## Psionic decision blocks fork all live hybrid prefix state

`psionic/lib/qwen-decision.coax` prepares every Q/K/V row in a current block
before full attention. Queries see every prior-prefix and current-block key;
sibling blocks are processed from separate snapshots. DeltaNet remains causal.
The full block stages outputs and new KV entries until the entire block succeeds.

`psionic/lib/qwen-prefix.coax` snapshots all convolution histories and recurrent
matrices plus the live KV prefix of every full-attention layer. It requires an
exact shape and matching nonzero caller-supplied 32-byte model identity; it does
not authenticate that identity or the source history. Destination publication
is atomic on success, scratch may be dirty on refusal, and unused KV tails are
left untouched. Caller pointer, extent, alignment, state identity, lifetime and
exclusive-access obligations remain explicit; C observes address ranges only.

Check: `psionic/check.sh` includes independent nonzero block/fork fixtures,
debug/release zero-survivor mutation and native plain/sanitized checks. See
[chapter 12 verification](docs/verification/psionic-ch08-2026-10-29.md) for the
CoderOS CPU scope and remaining whole-model and upstream-reference gates.

## Psionic Qwen-style causal layers publish complete state transitions

`psionic/lib/qwen-block.coax`, `qwen-linear-layer.coax` and
`qwen-full-layer.coax` compose offset-RMS, original BF16/F32 projections,
convolution/DeltaNet or gated full attention, residuals and SwiGLU. The bounded
CPU profile admits the target Elephant and Mammoth per-layer dimensions;
it does not claim full-checkpoint execution or upstream bitwise parity.

Each linear layer stages its convolution history, recurrent matrix and output
until the full layer succeeds. Each full layer stages its next KV entry and
output. Invalid shape, length, address observation, non-finite input and
arithmetic failures do not publish those outputs or states; workspaces may be
dirty. The full layer's present path is single-token causal. Callers retain
valid allocation/lifetime/alignment, exclusive access and consistency of state,
position, weights and supplied rotary coefficients.

Check: `psionic/check.sh` contains debug/release authority mutation, independent
full-layer fixtures and native plain/ASan+UBSan checks. See the
[chapter 11 record](docs/verification/psionic-ch07-2026-10-29.md) for tested
limits and the separate CoderOS CPU results. Decision masks and full model
execution remain future work.

## Psionic hybrid primitives stage attention and recurrent work

`psionic/lib/delta.coax`, `qwen-attention.coax` and `qwen-matvec.coax` own
bounded Qwen-style convolution/recurrence, split-half rotary and gated grouped
attention, and original BF16/F32 row projections. Bounds include 16 key heads,
48 value heads, 128-wide recurrent dimensions, 32 full-attention query heads,
256-wide full heads, 16,384 visible positions and 32,768 matrix rows/columns.
The delta `A_log` upper bound is 80. Shape admission precedes multiplication.

The delta step stages the next convolution history, recurrent matrix and output
before publishing any of them. Full-attention preparation and matvec publish
only complete outputs; masked attention stages output in scratch. Finite input,
coefficient, mask, capacity and overlap checks are explicit; scratch may change
on arithmetic failure. Callers must keep valid aligned allocations, truthful
extents and exclusive access; native wrappers only observe address ranges.

Check: `psionic/check.sh` includes debug/release authority and mutation gates,
independent scalar fixture generators and native plain/sanitized runs. The
[hybrid verification](docs/verification/psionic-hybrid-2026-10-29.md) defines
the scope; these primitives do not yet run a complete model or establish
upstream numerical parity.

## Psionic causal attention publishes a complete block and cache entry

`psionic/lib/attention.coax` owns a bounded causal dense block with learned RMS
scales, row-major Q/K/V/output and SwiGLU matrices, grouped-query heads,
caller-supplied interleaved rotary coefficients and a position-major KV cache.
Width is at most 16 with an even head width, hidden width at most 32 and cache
capacity at most 16. Shape checks precede multiplication. Each score and
intermediate vector is narrowed to binary32; products and reductions use the
chapter 4 compensated binary64 profile without FMA or reassociation.

Inputs, weights, coefficients and the live cache prefix must be finite; rotary
coefficient magnitudes are at most one. The caller owns the chosen rotary
convention and ensures cache positions are sequential and correspond to the
same immutable shape, weights and table. A step stages output and the new cache
entry; failure preserves both. A full pass stages the entire sequence, and a
failure preserves its public destination. Unused writable tails are unchanged.
The native adapter observes aliases; valid aligned allocations, truthful
extents, lifetimes and exclusive access remain caller obligations.

Check: `psionic/check.sh` includes debug/release Coax and zero-survivor
mutation, independent full-sequence fixtures, native debug/release and
ASan/UBSan checks, causal isolation and book examples. The chapter 10 record
scopes the evidence: this small causal profile does not establish Qwen3.5,
block-bidirectional decision masks or Elephant/Mammoth execution.

## Psionic text requests publish complete bounded byte/token layouts

`psionic/lib/text.coax` and `psionic/lib/text/` own `qwen35-text-v1`: frozen
tagged Base Labs choice/noul/score prompts, original Qwen byte-BPE/added-token
data, Unicode-17 NFC and split classes, and shared token-prefix matching over
complete branch encodings. Original and packed tokenizer identities are pinned;
callers admit the exact table SHA-256 once and keep those bytes immutable and
alive for subsequent calls. Layout parsing is not a fresh identity admission.

State strings preserve whitespace; instructions/descriptions use Python
Unicode stripping. Structured entries preserve insertion order and exact integer,
boolean/null/string values with two-space JSON formatting and a 4,300-digit
integer limit; duplicate keys and
floating/exponent JSON numbers refuse. Choices admit 1–255 options, noul has
yes/no slots, and the pinned score schema admits 1–10 levels. Question IDs never
enter the prompt. The exact disabled-thinking assistant prefix is preserved.

Each input/rendered branch is at most 65,536 UTF-8 bytes, each candidate/full
branch at most 16,384 tokens, and a split has 1–32 branches. BPE pieces are at
most 1,024 bytes, with a sum-of-squared-piece budget of 16,777,216; the encoder
reports this conservative bound as `work`, zero on failure. Combining runs
admit at most 128 nonzero classes. JSON has depth 16 and 256 members per
container. No truncation, option pruning or silently changed template is used.

Public prompt bytes, token IDs and split metadata change only on success;
workspaces may change on refusal. Split intervals refer to flattened complete
encodings; `answer` is the final token position within its complete branch.
Native C observes every writable and nested read-only span; callers retain
pointer/extent/alignment/lifetime/immutability and exclusive-access duties.

Check: `psionic/check.sh` runs debug/release checks and zero-survivor mutation,
original-data token/request fixtures, independent Unicode/regex/BPE oracles and
real native storage in plain and ASan/UBSan builds, plus previous chapters and
the book gate. See `docs/verification/psionic-ch05-2026-10-29.md`. The retained
corpus uses an owned oracle; broader upstream-runtime parity and whole-model
execution remain separate qualification gates.

## Psionic interchange carries inert weights and explicit storage leases

`psionic/lib/interchange.coax` owns `psionic-interchange-v1`: 1–16 uniquely
named nonempty rank 0–4 F32/BF16 tensors, at most 1,048,576 elements per tensor,
4 MiB aggregate payload and 16 KiB header. It writes canonical lexicographic
Safetensors maps without dtype/layout/name conversion and rejects nonfinite
encodings. Inspected names are owned metadata; payload offsets borrow the
file. Refusals preserve public output and report zero written. This separate
profile leaves the original bundle contract below unchanged.

`psionic/platform/interchange.h` exposes fixed-width versioned ABI v1 on LP64
little-endian binary32 hosts, without generated Coax types. DLPack accepts
legacy and versioned 1.0–1.2 CPU:0 contiguous f32, including scalar/empty tensors;
exports are read-only versioned 1.0 with explicit strides. Coax owns layout,
address-observation admission and reference transitions; C performs allocation,
byte copies and managed-deleter effects. Raw transport preserves f32 bits;
weight/numerical admission remains stricter.

A recognized nonnull carrier and empty nonnull output transfer ownership,
including validation refusal; earlier argument errors do not consume it. A
versioned major mismatch reads only its stable prefix and calls its deleter.
Successful imports pin the producer until the last explicit lease; exports
own another lease. Opaque capabilities, truthful pointers/extents, producer
lifetime for null deleters and exclusive coordinated access remain caller
duties. No GPU stream, arbitrary graph or optimizer conversion is claimed.

Check: `psionic/check.sh --only interchange` exercises debug/release Coax,
native plain/ASan/UBSan and real NumPy producer/consumer lifetimes, with an
independent external-weight to Coax-training to portable-inference round trip.
Full mutation is omitted under owner instruction. See
`docs/verification/psionic-ch4a-2026-10-29.md`.

## Psionic original-weight admission binds complete immutable byte inventories

`psionic/lib/model-io/` owns `original-weights-v1`: rank 0–4 BF16/F32
Safetensors inventories with exact positive shape/byte intervals, no overlap
or unindexed payload, unique names, bounded JSON and explicit unsupported
refusals. Limits are 64 GiB per raw weight file, 128 GiB per weight bundle,
16 shards, 4,096 tensors, 1 MiB per header/index and 128-byte printable
unescaped ASCII identifiers. Header metadata has at most 64 string pairs.
These artifact budgets are separate from CPU execution budgets.

Bundle admission binds exact original config/tokenizer/index/shard SHA-256
hashes and shard sizes to caller-provided trusted pins, complete expected
name/dtype/canonical-shape/shard inventory and exact shard-index coverage.
Labels/hashes do not authenticate their source or establish model semantics.
Names borrow immutable original headers; callers retain those bytes unchanged
and alive until the descriptors die. Full inventory is published only on
success; drafts may change on refusal. Config/tokenizer remain opaque bytes.

Decoding reparses the file, validates a named window of at most 1,048,576
values, and checks every selected encoding for finiteness before converting
BF16/F32 to F32 scratch and then destination. Every refusal preserves BOTH
float spans and reports zero written/bytes; metadata workspaces may change.
Success preserves unused writable capacity. Native wrappers observe all
writable spans against each other and every nested read-only view; pointer
validity, truthful extents, alignment, immutability, lifetime and exclusive
access remain caller obligations. There is no concurrent transaction claim.

Check: `psionic/check.sh` runs the authority/import closure in debug/release
with zero-survivor mutation, independent owned original-file/hash fixtures,
all BF16 encodings and F32 representation/alias/truncation cases under native
debug/release and ASan/UBSan, plus earlier chapters and the book gate. See
`docs/verification/psionic-ch04-2026-10-29.md`. These checks do not qualify
full Elephant/Mammoth execution, every Safetensors dtype or a larger backend.

## Psionic CPU numerical operations publish only complete admitted results

`psionic/lib/ops-contract.coax` owns `CPU-f32-wide-v1` operation/resource
admission; `psionic/lib/ops.coax` owns its checked data boundary and kernels.
The profile has contiguous binary32 row/matrix storage, explicit binary64
products and compensated reductions, round-to-nearest arithmetic without
FMA/reassociation, and one narrowing per output. Shapes/typed counts reuse the
1,048,576-element cap; matmul also admits at most 1,048,576 multiply-add pairs
by a division-before-multiplication check. It does not inherit M0's magnitude
cap or silently change that trainer's arithmetic.

All float operands, including masked logits/unselected rows, must be finite.
Masks are bytes 0/1 with an allowed value in every row. Indices, canonical
parameters, exact read-only view counts and disjoint writable capacities are
validated before staging. Unscaled RMSNorm/LayerNorm put positive finite
epsilon inside the root. Add/sum/dot/matmul refuse a computed wide value
outside the finite binary32 interval. Every failure reports zero written;
pre-execution refusal preserves all spans, arithmetic failure may dirty scratch
but leaves destination unchanged. Success preserves read-only storage and
unused writable tails. Typed native adapters only observe ranges; C callers
retain pointer/extent/alignment/lifetime and exclusive-access obligations.

Check: `psionic/check.sh` runs each authority root's debug/release checks and
zero-survivor mutation gate, Decimal(120) fixtures independent of the Coax
algorithms, analytic cancellation/magnitude cases, actual short/aliased typed
storage and a maximum-work matmul in plain/sanitized debug/release builds,
plus all earlier chapters and source-linked book/transcript checks. See
`docs/verification/psionic-ch03-2026-10-28.md`. This qualifies a bounded CPU
profile, not learned affine layers, arbitrary layouts, gradients or devices.

## Psionic owned tensor placement keeps device policy and arithmetic in Coax

`psionic/lib/tensor-placement.coax` and `tensor-device-host.coax` own the
`tensor-device-v1` bounded device/dtype contract. Opaque native sessions and
handles hold authoritative Coax records; metadata snapshots do not retain
allocations. Views retain a root, copies own a new root, and ready storage is
immutable. Host API access is exclusive. Storage/session IDs never repeat.
Pending outputs and borrowed inputs cannot be freed. Explicit wait/cancel
drains work before releasing input reader claims; aliased input roots count
both reads, and independent pending outputs release only their own claims.
Cancellation invalidates its output, numerical failure invalidates only the
result, and a driver error poisons further session execution. Idle references
can still be released; close refuses live allocations. There is no implicit
wait, implicit placement or CPU fallback.

`tensor-device-kernels.coax` supplies the actual CPU/CUDA/Metal finite packed
casts, dtype-grid uniform generator and add/ReLU/dropout program. Integer RNE
preserves finite f16/bf16 encodings, signed zero and subnormal cast bits.
Equal dtypes retain their type; mixed pairs produce f32. Program arithmetic
uses f32 then one output narrowing; numerical flags prevent publication of a
partial/nonfinite result. Counter32-v1 uses explicit seed/stream/logical ranges
without wrap; allocation or scheduling consumes no hidden random state. It is
a reproducibility primitive with no cryptographic/training-quality claim.

Native code allocates Coax-admitted resident/temporary buffers and performs
copy/driver effects. Export stages a complete disjoint logical readback;
GPU copies/programs run the requested generated kernels. CPU work completes
eagerly but waits logically; CUDA queues immediately; Metal commits its batch
at wait. Tensor payload budgets exclude driver/module/allocator overhead and
separately bounded host transport scratch. This small profile does not extend
all 27 CPU operators to GPUs or supply in-place updates/autograd.

Check: `psionic/check.sh --only tensor-device`, with explicit `--metal` and
`--cuda` device harnesses, qualifies independent integer/format references,
every finite half/bfloat16 encoding, RNE boundary vectors, all dtype pairs,
strided transfers, completion/lifetimes, failure preservation and native
ASan/UBSan in debug/release. Numerical bounds/device behavior are recorded in
[chapter 6 verification](docs/verification/psionic-ch03b-2026-10-29.md).
`tensor-placement.coax` is gated by root-only mutation with zero survivors (#12698).

## Psionic tensor operators infer support and publish complete value results

`psionic/lib/tensor-ops*.coax` owns `CPU-f32-wide-views-v1`: 27 bounded CPU
operators with pure shape/dtype inference and explicit analytical value VJPs
for 25 of them. The machine-readable `psionic/fixtures/tensor-ops/operators.json`
records exact ranks/options, f32 values/masks, i64 indices/auxiliaries, CPU
backend and differentiation status. It does not widen chapter 4's old profile.
Operands reuse chapter 3's admitted signed-stride views and allocation epochs.

Forward work and direct VJP traversal pairs have separate 1,048,576-unit
bounds. Products/reductions use f64 and ordered compensation before checked f32
narrowing; FMA/fast-math are disabled. Empty identities, arithmetic domains,
last-update-wins behavior and stable first-index subgradients are explicit.
Only supported convolution stride one is admitted. Every logical operand is
finite, including unselected/overwritten values. Public values/indices or both
gradients publish only after all staging succeeds; arithmetic refusal may dirty
scratch but preserves all public outputs and returns zero counts.

C only observes address ranges, including metadata and integer buffers. Native
caller obligations include truthful aligned/living spans, immutable inputs,
exclusive access and fresh unpublished results. Registered tensors are replaced
through chapter 3's versioned scatter, and saved graphs must validate their
version tokens before a functional VJP. Returned gradients use logical input
coordinates; shared-backing/view accumulation and automatic tape integration
remain separate. No dtype promotion, general operator options, GPU execution or
large-model gradient throughput is inferred from this bounded CPU reference.

Check: `psionic/check.sh --only tensor-ops`; authored debug/release checks,
independent scalar and central-difference references for 38 cases in three
layouts, explicit tie conventions, actual aliases/work/domain/late-publication
refusals, native plain/ASan/UBSan and the exact book transcript. See
[chapter 5](docs/verification/psionic-ch003a-2026-10-29.md).

## Psionic CPU tensor views share one authoritative storage version

`psionic/lib/tensor-view.coax` adds a separate bounded borrowed-f32 view layer;
chapter 2's contiguous/nonempty contract is unchanged. Views have rank 0–4,
nonnegative extents, signed element strides, absolute origins and allocation
identity/epoch. Extents, absolute strides, nonempty logical element counts and
backing counts are bounded by 1,048,576. Empty tensors dereference no element.
Every public boundary re-admits metadata and proves all reachable offsets fit.
Writable layouts require a sufficient injectivity proof; broadcast views are
read-only, and interval alias queries conservatively retain possible overlap.

Gather/scatter validate exact owner/backing extents, capacities, finite selected
or source values and disjoint declared writable/read spans before any writes.
Refusal preserves all spans and returns zero written. Scatter increments the
single authoritative version exactly once on a successful nonempty write;
saved tokens detect any alias write and refuse retired/reused allocation epochs.
Empty writes do not increment; version overflow refuses before staging.
The C adapter only observes ranges, including the owner record. Caller-owned
storage, truthful valid/aligned extents, fresh identities/epochs, one live
record per allocation, mutation through this boundary and exclusive synchronous
access remain obligations. This is not automatic ownership, unforgeable native
handles, concurrency, a backward engine or device placement.

Check: `psionic/check.sh --only tensor-view`; independent native integer address
and uniqueness enumeration, actual aliases, finite-bit copying, short buffers,
zero-element null views and maximum-size transpose, plus debug/release native
ASan/UBSan and the exact book transcript. The dated record is
[chapter 3](docs/verification/psionic-ch002a-2026-10-29.md).

## Psionic tensor storage admits shapes before touching caller data

`psionic/lib/tensor.coax` owns contiguous binary32 shape/index/copy admission:
rank 0–4, positive active dimensions, zero inactive dimensions, a scalar count
of one, and at most 1,048,576 elements / 4,194,304 logical data bytes. Product
limits are checked by division before multiplication. Failed layout/index
results have zero payload fields; callers must inspect status.

The synchronous staged copy requires an exact logical source view and sufficient
disjoint destination/scratch capacities, including their declared tails. Every
source value must be finite; chapter 1's magnitude cap is not inherited. All
refusals precede any write and report zero written elements. Success preserves
source bits and unused writable capacity on the tested Apple LP64 CPU profile.
The shared native `psionic/platform/spans.h` observes address ranges; Coax owns
admission. Native callers still own pointer validity, truthful allocation
extent, alignment, lifetime and exclusive access during the call.

Check: `psionic/check.sh` runs the chapters' Coax debug/release checks and
zero-survivor authority mutation gates, independent native layouts/indices,
short allocations, aliases/non-finite representations and a maximum-size copy
in plain/sanitized debug/release builds, plus source-linked book/transcript
checks. See `docs/verification/psionic-ch02-2026-10-28.md`. Chapter 4 now adds
a separately qualified dense CPU profile. Autodiff and GPU execution remain
planned; these checks are bounded evidence,
not general memory safety or concurrent atomic publication.

## Psionic's first trainer publishes only completed bounded work

`psionic/lib/learn.coax` owns the one-parameter CPU model, its mean squared
loss/analytic gradient, fixed `1/8` updates, prediction and admission. Its
`#authority` checks cover 1–16 paired samples, 1–64 updates, two caller trace
buffers, and finite input/target/weight magnitudes at most 16. The native
adapter only observes span overlap. Input storage and the published loss
trace stay unchanged on refusal; scratch can contain partial work after an
update leaves the weight domain. C callers still own pointer validity and
lifetime. This is a bounded learning example, not tensor, autodiff or GPU
support, nor a claim that arbitrary admitted data has decreasing loss.

Check: `psionic/check.sh` runs authored checks and a zero-survivor mutation
gate, independent native oracles and real alias/non-finite/short-buffer cases
in debug/release under ASan/UBSan, and exact book source/transcript/status
checks. See `docs/verification/psionic-m0-2026-10-28.md` for evidence scope.

## Resident Parakeet binds one original model and publishes whole utterances

`psionic/platform/psionic-parakeet.h` owns one serialized engine per process.
Coax checks the pinned complete checkpoint/tokenizer hashes and all 723 tensor
names, shapes, dtypes and disjoint payload ranges before exposing model state.
Input is 320..327520 finite mono 16 kHz samples in [-1,1]. Model-specific
normalization excludes the centred STFT's extra physical column; the generic
spectral teaching profile is unchanged. Coax owns the frontend, encoder kernels,
schedule and TDT arithmetic; C performs file, memory, mutex and GPU effects.

A GPU request refuses when Metal cannot run; it never silently selects CPU.
Failed recognition leaves caller text and the previous successful token/frame,
feature and encoder snapshots intact. Inspection copies into caller storage;
the next call cannot mutate a borrowed result. Closing releases the model and
device resources. Empty text is valid; microphone consent, VAD, long-clip
segmentation and draft insertion remain the caller's explicit responsibilities.

The exact decoder's persistent pool has at most eight participants, including
the caller. Coax owns complete disjoint rows and preserves each compensated
sum's original bias/input/recurrent order. Every gate finishes before hidden
state changes. Host dispatch is serialized, partial thread startup uses the
observed smaller pool, and close joins workers before freeing their inputs.
The added thread-count query preserves the existing info-struct ABI. Native
O0/O2/ASan/UBSan bitwise and lifecycle checks plus the same 29-recording
reference replay qualify this optimization separately from the baseline.

Checks: focused model/frontend/text/launch-policy mutations, native O0/O2 and
ASan/UBSan API boundaries, and actual full Metal feature/encoder/token/frame/text
comparison against pinned NeMo on 29 frozen recordings. These finite checks
qualify the named offline Metal profile, not population accuracy, a complete
CPU encoder, true streaming or key-release latency. Evidence:
`psionic/docs/verification/parakeet-2026-11-02/README.md`.

## Streaming CLI capture publishes provisional snapshots and one guarded final

`openagents dictate --stream` is explicit Linux CUDA English RNNT adoption.
The default multilingual offline mode and `openagents.dictation.v1` retain
their identities. CPU, macOS, `--json`, terms and code formatting refuse before
capture. A separate helper avoids generated-C symbol collisions; native effects
copy one bounded model job into owned worker storage. The worker never prints
and never borrows the capture controller or caller stack.

`apps/shared/dictation-stream.coax` (`#authority`) copies only a current-ticket,
consented/active/ready held, trailing or frozen clip, at a bounded offset.
`dictate-stream-rules.coax` (`#authority`) owns one in-flight call, exact model
sequence, 2,560-sample batches, the 960,000-sample cap, current-ticket partial
and final eligibility, revocation and the 250 ms output-retry bound. Capture,
resampling/VAD, pre-roll/tail and final speech/ticket admission remain the
unchanged shared controller's. Backlog stays in its fixed clip buffer.

Stop drains that same model state and finalizes once. Partials are display-only;
the existing controller must admit the final before stdout publication.
Cancellation closes capture, revokes publication and ends the process without
waiting on a pending GPU job; its detached worker owns all its buffers and
cannot print a late reply. Output retries poll controls and are bounded; an
interrupted last line may be incomplete. Already published text cannot be
retracted. The new JSONL projection starts in
`proto/openagents/cli/dictation/v1/stream.proto` and
`docs/protocol/dictation-stream-v1.md`.

Checks: 78/78 flow/output and 25/25 live-copy mutants killed; actual CLI recorded
capture/recorder, controls, input/model refusals, no speech and undrained-output
checks; host ASan/UBSan speech and in-flight cancellation; existing offline CLI
regressions and protocol compatibility. Physical microphone/UI acceptance is
separate. Evidence: `docs/verification/dictate-stream-2026-11-03/README.md`.

## File transcription defaults to bounded local Parakeet

The ordinary `openagents transcribe FILE` selects the pinned Parakeet model;
legacy Psionic Whisper and whisper.cpp require explicit backend names. Missing
model/GPU/input failures never download a model or choose another backend.
`psionic/lib/parakeet/file.coax` owns exact 320..327520-sample windows, short-tail
redistribution, global resampling phase, TDT emission bounds and all writers.
At most 4096 windows cover every original resampled sample exactly once; no
model padding is passed off as real audio. Input WAVs must be complete.

File JSON is explicitly `openagents.parakeet.file.v1`; single-window metadata
retains its earlier top-level tokens/frames. Each nonempty independent window
is one subtitle cue. Frame emission is not acoustic word alignment. The input
mapping assumes immutable backing and grows in virtual extent; inference
scratch is fixed. Before final publication, failed/cancelled inference emits no
transcript. Output failure/cancellation during publication may leave partial
bytes and returns nonzero. Inspection dumps are a separate explicit file effect.
The model C ABI and its per-request limit are unchanged; this is not streaming.
Checks and finite original-weight/NeMo evidence:
`docs/verification/parakeet-file-cli-2026-11-02/README.md`.

## The public Pylon board verifies original events and labels live observation

`apps/openagents-web/pylon.coax` displays read-only Pylon state. Its private
C byte bridge passes the identical original event to the shared Coax
`landing-nostr.coax` reader: beacon projection requires the event ID,
BIP-340 signature, decoded-content SHA-256, provider binding, schema and
freshness checks. Activity requires the same original event verification.
No public Wasm export accepts an admitted flag or a projected machine.
Owner authorization, hardware claims and encrypted outcomes are not proved
by a self-signed beacon. The agent guide states these limits.

Kinds 26910/27010 are ephemeral. The board counts only live observations
from its opening time, for keys with an admitted beacon, with duplicate IDs
suppressed. Its bounded 64-ID window freezes explicitly when full; it never
evicts IDs and silently recounts replays. Expired/non-online beacons are
labelled offline. Newer generations cannot be replaced by older ones.
`pylon-policy.coax` is an authority-marked display-policy unit with a
zero-survivor mutation gate. `coax/web/tests/pylon_page.test.ts` exercises
the actual page Wasm and C bridge with independent signed fixtures, including
tampered signatures/content, invalid beacons, old activity and duplicates.
Desktop/phone page-start and golden checks cover `/pylon`; release smoke
also requires its Markdown guide and the verbatim audience join script.

## GGUF descriptors publish only after bounded structural admission

`gguf-v3-le-v1` (`psionic/lib/gguf.coax`) accepts little-endian v3, pinned
GGML layout IDs and explicit header/file/count/value budgets. Every dimension
product, encoded block size, aligned offset and payload end fits before use;
keys/names are unique, UTF-8 is valid, booleans canonical, ranges disjoint and
header padding zero. Zero-tensor upstream headers may omit final padding.
The immutable header owns borrowed strings. Public descriptor arrays change
only on success; separate scratch may change on refusal. C observes address
spans only. Reading structure does not authenticate or execute tensor bytes.

Check: `psionic/tests/check_gguf.sh --mutate --jobs 8` (CPU only, independent
struct oracle, pinned original headers, debug/release and ASan/UBSan).

## Tokenizer families and bounded Jinja retain their named profiles

`psionic/lib/tokenizer-families.coax` owns bounded ASCII BERT WordPiece and
T5 SentencePiece-style Unigram wrappers, plus normalized UTF-8 Unigram.
Immutable unique sorted pieces have explicit count/byte/ID/finite-score limits.
WordPiece rolls a failed word back to one unknown; Unigram maximizes score,
retains first-encountered ties and fuses adjacent unknown IDs. Original ASCII
normalization DATA is pinned; full Unicode BERT/T5 normalization and added-token
recognition are separate work. `chat-template.coax` admits its entire bounded
Jinja subset before rendering, never interprets message content, bounds loops,
condition depth and output, and refuses unsupported syntax even in inactive
branches. Value results own their storage; errors return zero written payload.

Check: `psionic/tests/check_tokenizer_families.sh --mutate --jobs 8`: CPU-only,
290 full original-vocabulary vectors, 12 independent Jinja outputs including
TinyLlama's actual template, debug/release and native ASan/UBSan.

## The byte bigram trains and generates under an explicit bounded CPU profile

`psionic/lib/byte-lm.coax` admits 2–4096 bytes, 1–64 full-batch SGD updates,
rate (0,64], complete 256x256 f64 tables and disjoint public/scratch/trace spans.
It trains from zero using only the training partition, normalizes gradients by
all transitions, and publishes only the completed table and loss trace.
Evaluation uses the full 256-byte softmax and finite logits in [-4096,4096].
Generation uses `splitmix64-top53-v1`, one seed/counter draw per byte, ascending
strict CDF buckets and a final residual bucket. Length and counter exhaustion
are checked before output writes. Licensed synthetic whole-document splits
are fixed and disjoint; their likelihood does not claim general language quality.
C observes spans only; Coax owns arithmetic and admission. Valid pointer/view
lifetimes and exclusive synchronous output access remain caller duties.

Check: `psionic/tests/check_byte_lm.sh --mutate --jobs 8` (CPU only): independent
full-table SGD/likelihood/sample capture, finite differences, real aliases,
debug/release, native plain/ASan/UBSan and exact book output.

## Learned byte BPE keeps training and inference boundaries identical

`psionic/lib/bpe-train.coax` owns `byte-bpe-nanochat2-u17-v1`: the pinned
nanochat two-digit regex over Unicode-17 properties, no normalization or added
tokens, and 256 byte IDs followed by at most 64 merge IDs. Highest overlapping
pair count wins, ties choose the lowest pair of IDs, and replacements are
disjoint left to right. No pair crosses a piece or document boundary.
Up to 16 documents / 4096 UTF-8 bytes and 64-byte expanded tokens are admitted;
late growth failure returns an empty model. PST5 packing validates rule order,
references and uniqueness, checks capacities/actual aliases before writes and
publishes only a completed table. The new encoder shares chapter 9's layout,
span reader and ranked BPE core, with its own explicit splitting profile.
Inputs/class tables retain immutable valid storage and recorded identities.
C observes spans and performs verification file effects only.

Check: `psionic/tests/check_bpe_train.sh --mutate --jobs 8` (CPU only): full
64-merge independent recount, exact persisted PST5, 147 Unicode/ID/byte round
trips, 20 tiny training traces, debug/release and ASan/UBSan.

## Psionic fleet results retain the selected identity and attempt

`psionic/lib/fleet/` is the chapter 25 opt-in loopback JSON adapter. A verified
fresh v1 beacon routes only an online free slot advertising the configured
exact decision identity and free terms. Held generations do not roll back;
a newer withdrawal removes the old route. Unknown unrelated services never
consume route slots. The bounded 32-coordinate history lives until restart.

A remote result publishes only after relay acceptance and full event
verification, NIP-44 opening, exact provider/buyer/job/request/attempt binding,
matching service identity/door, valid asked answer shapes and a live budget.
An older generation's result cannot replace a current attempt. Local fallback
is explicitly loopback, completely framed HTTP200, and validates the same
exact identity and answer contract. Stop does not start new fallback work.
No timeout or lost ephemeral event is represented as a completed decision.

The signing identity is fresh, random and test-grade, in memory only. Public
beacons remain signed provider declarations, not remote model attestation.
This does not approve production keys, public ingress, owner-auth beacons,
v2 groups, payments, receipts, durable retries or production provider trust.
C owns socket/lifecycle effects only; Coax owns the protocol and routing.
Check scope and actual commands: `docs/verification/psionic-ch20-2026-10-30.md`;
the host has its separate Mac/Linux sanitizer record.

## AdamW steps are admitted whole over a binary64 master copy

`psionic/lib/optim/adamw.coax` (`cpu-adamw-v1`) keeps parameters and both
moments in binary64. Groups cover the parameters exactly once, in order, at
most 16, each with lr in (0,1], betas in [0,1), eps in (0,1] and decay in [0,1]
with lr*decay < 1. Decay multiplies the incoming value before the moment step;
the step is bias-corrected with eps outside the root. Compute precision is
declared: binary32 compute admits only binary32-valued gradients and writes a
binary32 copy. Every element is computed and checked before any write: a
non-finite gradient or an unrepresentable result refuses, or under the skip
policy returns `skipped` and advances only the skip counter; views and the step
counter are otherwise unchanged. C observes spans only.
Check: `psionic/check.sh --only adamw`: independent oracle, resume at every
step, debug/release, zero surviving mutants, native ASan/UBSan.

## Checkpoints resume only into the experiment they name

`psionic/lib/data/pack.coax`, `psionic/lib/checkpoint/record.coax` and
`psionic/lib/checkpoint/resume.coax` (`psionic-resume-v1`). Only training
documents are packed; every document starts with the boundary token and no
target is a boundary. An epoch's order depends only on (seed, epoch), so the
data position is the cursor alone. A checkpoint is the versioned
little-endian `PSCK` record with its SHA-256; it resumes only when its
model, tokenizer, dataset and optimizer identities equal the run's, its
pointer names its revision and digest, and its cursor fits the dataset.
Every refusal writes nothing. The host publishes a record before its pointer,
each through a temporary file, fsync and rename, so a crash leaves the
previous pointer naming a whole record.
Check: `psionic/check.sh --only resume`: independent oracle, interrupted runs
bit for bit, zero surviving mutants, native crash-and-resume under ASan/UBSan.


## Psionic weight edits name an immutable base and publish a whole value result

`psionic/lib/weight-edit.coax` owns `weight-edit-f64-v1`: 1–4096 binary64
values within ±2^20, 1–32 unique named contiguous rank-0–4 tensors, and at
most 256 distinct expected-old/replacement edits. Shapes cover the complete
input in order. The semantic profile, names, canonical shapes and every
weight bit participate in a versioned SHA-256 encoding. Signed zero retains
its bits. Missing names, bad indices, duplicates, stale old bits, a foreign
base digest and nonfinite/out-of-profile values refuse before publication.

The source is immutable and successful output owns its fixed-capacity value
array. Refusal returns zero count, changed count, digest and payload; exact
inverse edits on the edited base restore the original digest. Names are
borrowed only during the synchronous call; callers retain valid immutable
views and exclusive source ownership for that call. The profile does not
provide in-place concurrency, original-model-sized overlays, GPU edits,
file publication, model provenance or quality/promotion authority.

Checks: chapter 253's focused Coax gate, entry-source zero-survivor mutation,
independent SHA-256/full-prefix decoder oracle and native nonfinite tests
under debug/release plain/ASan/UBSan. Recorded model proposals are exploratory
measurements, never deployment permissions; likelihood gains alone have
counterexamples with zero recovered answers. Evidence is in
`docs/verification/psionic-ch253-2026-11-02.md`.

## Constructed networks publish only complete bounded artifacts

`psionic/lib/construct.coax` owns `construct-relu4-f64-v1`: 1–4 inputs and
outputs, four ReLU units and an affine skip, row-major W,b,V,D,e. Checked
forward admits exact lengths, finite input within ±64 and finite weights
within ±2^20 before publishing owned output. The proposal parser admits only
a complete integer JSON array with coefficients within ±1024. Its five
named tensors use the immutable weight-edit identity. Refusals return zero
count and payload. The arithmetic core requires admitted immutable views.

`construct-program.coax` admits 1–24 postfix tokens per output, with explicit
stack arity and a step-produced bit for select. It returns a complete graph
or no graph. Existing gate lowering owns the compiled arithmetic; chapter
254's study preflights every integer intermediate within ±2^26 before running
actual weights. Readout coefficients count even when there are no hidden
gated layers. Parsing, identity and finite-case success confer no quality or
deployment authority. Evidence: `docs/verification/psionic-ch254-2026-11-02.md`.

## Bounded original-model weight windows

`psionic/lib/original-window.coax` owns `original-window-v1`, separate from
`weight-edit-f64-v1`. A window admits little-endian finite BF16/F32 bytes,
matrix dimensions 1–1048576 and a contained flattened span of 1–16384
coefficients. SHA-256 binds profile, trusted model/shard pins, name, shape,
offset, count, dtype and all source bits. Whole-shard authentication and
trustworthy pins remain caller obligations. Identity alone is not provenance.

Replacement requires the exact source identity/count and finite proposed
values within ±2^20. It rounds F64 to F32, then BF16 ties-to-even where
applicable, and returns owned complete bytes. Count is bytes; changed counts
encoded coefficients. Refusals publish zero payload/count/digests. Source
bytes never mutate; restoration binds the edited identity and original bits.
Decode/encode helpers and the study's matrix arithmetic require admitted
views; the public identity/apply boundary performs admission.

The original-model experiment patches only private idle GPU copies through a
test-only effect hook, restoring both original rows before candidate writes.
Any upload failure aborts before inference. It adds no serving patch API,
quality authority or automatic promotion. Same-digest carried observations
are explicitly identified and never counted as new trials. Scoring, finite
worksheet success and chapter-241 independent qualification stay distinct.
Evidence: `docs/verification/psionic-ch255-2026-11-02.md`.

The later Metaverse-dialogue study uses the same boundary for 32 final-MLP
expert-bias windows, restoring the complete original bias tensor before
each candidate upload. Its recorded dialogue and model-critic preferences
grant no game-state authority or serving promotion. Fixed-prefix numerical
equivalence, original parity, no-op/restoration and failures are recorded in
`docs/verification/verse-villain-2026-11-02.md`.

