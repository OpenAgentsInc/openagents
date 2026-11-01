# Robotics assessment at the requested stopping point

This records the delivered robotics work and remaining work when the owner
asked to stop after the current chapter 171 update. A chapter marked
`implemented` in the book registry has a runnable **named profile**; that
does not mean every original chapter acceptance criterion is complete.

## What is implemented

| Area | Delivered and checked | Practical boundary |
| --- | --- | --- |
| [168: imitation](../../book/ch168-imitation-before-robotics-scale.md) | Own CPU gantry behavior cloning, fixed scales, whole-episode splits, independent action and rollout measures; learned policy succeeds on 6/6 held-out episodes versus 0/6 for the zero policy. Pinned NVIDIA SingleArm data admission is separate. | Data handling and a small owned learner; no general Panda imitation result. |
| [169: original GR00T](../../book/ch169-execute-a-groot-policy.md) | Original LIBERO weights execute the vision/language/conditioning/flow path on Metal, with prepared-observation component and decoded-action comparisons. Original BF16 weights use an explicit F32/wide-stat arithmetic profile. | The qualified request has 140 tokens. A separately recorded global hidden-state diagnostic misses its criterion. General observations, embodiments, default-BF16 parity and model training remain open. |
| [170: genuine control loop](../../book/ch170-close-the-simulated-control-loop.md) | Original GR00T drives genuine LIBERO/Panda scenes through Coax preparation, supervision and an owned OSC controller, with MuJoCo as the evaluation backend. The earlier six-episode profile reports three task successes. | Six fixed episodes do not establish general reliability. MuJoCo still supplies contacts, integration and cameras in that evaluation lane. |
| 170: owned Panda model | Source-bound all-link FK, grip Jacobian, finger branches, mass/gravity, inverse dynamics, mixed-coordinate mass solve, actuator/passive/fluid force assembly and unconstrained Euler integration. Independent equations, finite differences, source captures and native profiles qualify the individual pieces. | A fixed-base, source-bound Panda model. A generic robot importer, floating-base quadruped and owned contact solver are absent. |
| 170: neural/owned-query integration | A genuine nine-action prefix uses owned FK/Jacobian/mass/bias queries inside the GR00T control loop: two neural calls and 225 live physics steps, followed by 225 exact recorded-control replay steps. | This short prefix is an integration result; its task-success flag is false. It does not establish an all-owned full episode. |
| [167: commands and control](../../book/ch167-command-a-robot.md) | Position IK, principal orientation error, whole-pose solving, linearized posture projection, rest-to-rest quintic timing, typed PD/gravity and sampled rotor analysis. Nonconvergence and numerical refusals are explicit. | Twelve of fourteen configured whole-pose goals converge; two retain budget exhaustion. Posture projection is a delta query. Timing stops at waypoints. Collision-safe motion and moving-state retargeting remain open. |
| 167: measured controller behavior | The initial nonlinear tracking experiment retains its misses. The nine-profile gain search retains **no winner**. A separate common scheduled PD law subsequently passes the original cases and eight frozen prospective C/D cases, with 6,000 SDK and 6,000 owned steps for C/D parity. | Fixed test cases with contacts disabled. The closest C/D terminal ratio is 0.9710715211 against a limit of 1; this is useful evidence, not a general stability or collision guarantee. The old gain-search result remains NONE. |
| Clocks, replay and language boundaries | Explicit step/clock adapters, ownership/supervision records, scoped replay, native affine point/vector support and qualified type-preserving source-token mutation support. Physical nominal types are used across the owned Panda queries. | General units/operator provenance, frame transforms, full Panda migration, compiler-enforced bounded control and whole-system replay remain incomplete. |
| [171: return targets and distributions](../../book/ch171-learn-a-gait-in-many-worlds.md) | GAE distinguishes termination, truncation and collector tails. PPO exposes the actual sign-aware clipped objective. Gaussian density/scores, two-draw finite-grid sampling and checked scalar new/old log-ratio admission have independent native comparisons. | Normalized scalar CPU F64 building blocks. They do not create physical worlds, a neural actor or a gait. |
| 171: current scalar-LQR learner | The owned plant, sampled rollout, detached analytic nominal critic, GAE, actual PPO coefficient, reverse-tape actor gradient, persistent AdamW state and gain projection form a genuine one-parameter learner. The independent optimum scorer is isolated from learning. | Fixed variance and one trainable gain. The analytic critic is not learned and is not an exact value function for the finite-top53 sampler. |

## Current learning result

The fixed experiment completed all 128 batches and 512 AdamW updates per seed. All 31 execution/audit actions passed, including byte-exact next-batch checkpoint replay. **All three seeds miss both prospective convergence goals.** No seed is promoted and no setting is retuned.

| Seed | Final K | Absolute gain error | Worst nominal held-out cost gap | Result |
| --- | ---: | ---: | ---: | --- |
| 0 | 0.5184256094 | 0.085306525 | 1.404964% | Both goals missed |
| 1 | 0.5227558679 | 0.080976266 | 1.259577% | Both goals missed |
| 7 | 0.5214162090 | 0.082315925 | 1.303622% | Both goals missed |

The nominal optimum is K*=0.6037321344. The frozen limits are gain error at most .05 and cost gap at most 1%. The initial K=.25 has gain error .3537321344 and worst nominal gap 40.451147%; the learned gains improve those measures substantially, but improvement does not satisfy the acceptance rule. The independent real-arithmetic margins also reject all three seeds. Rounded table values are explanatory; the original receipts retain full precision.

Canonical learning executes 196,608 plant steps, 1,536 AdamW updates, 12,288 tape forward/backward pairs and 786,432 sample-gradient evaluations. Declared checkpoint replay adds 1,536 steps/12 updates/96 tape pairs; four mean-policy evaluations add 3,072 deterministic steps. The summed action time is about 208 seconds on this named host, not a GPU throughput or end-to-end engineering-time measurement. The [LQR fixture](../../fixtures/robotics/locomotion-lqr/README.md) retains the full outcomes and compact curves.

This closes the mathematical and experiment-integrity slice. The LQR convergence criterion remains **open**. A future separately frozen experiment may study optimizer/update budget; extending this profile would also require revisiting its state/counter bounds. The current result is retained without extra training or in-place tuning.

The prescribed workload is three seeds, each with 16 persistent worlds,
32-step fragments, 128 batches and four full-batch updates per fragment.
Canonical learning accounts for 196,608 plant steps, 1,536 AdamW updates,
12,288 tape forward/backward pairs and 786,432 sample-gradient evaluations.
One next-batch checkpoint replay per seed adds 1,536 steps and 12 updates.
Baseline plus three final mean policies add 3,072 deterministic evaluation
steps over six held-out initial starts. Analytic noisy-policy costs are
separate from those executed mean-policy trajectories.

Every seed and outcome is retained. The release experiment permits no
best-seed selection, early stopping, retries or in-place retuning. Replay
checks one complete next batch, not a second full learning curve.

The [LQR fixture](../../fixtures/robotics/locomotion-lqr/README.md) and
[publication record](../../../docs/verification/psionic-locomotion-lqr-2026-11-01.json)
bind the source, independent references, native modes, experiment and
actual results. New LQR product roots close 554 emitted mutation sites;
the separate coefficient test transport closes 23, all with zero survivors.
The first mathematical debug, release and ASan/UBSan contexts each pass
229,063 assertions, 177 refusals, 12 overlaps and 83 reader poisons.
Their complete output streams agree byte for byte. Existing passing
optimizer, RNG, tape, model and robot gates were carried rather than rerun.

Pre-effect review also tightened experimental integrity: exact qualified
initial-state bytes are required; later chunks and final actors bind to
prior passed audits; replay consumes the authenticated checkpoint directly.
Old reader checks carry through exact declaration binding, while new
initial/chain guards have their own qualification. Historical preparations
are retained as nonfinal and have no learning effects.

## What remains

1. **Neural PPO preparation.** A batched actor/critic network, its parameter
   VJP and a learned value objective are missing. Multiple action dimensions
   also need one declared diagonal-Gaussian joint log probability and joint
   PPO ratio; independently clipping each coordinate would change the
   objective. Existing tensor IR/autodiff is useful infrastructure, but
   scalar gain learning does not qualify this composition.
2. **A real quadruped contract and environment.** Pin one original robot,
   asset terms, joint order, floating-base convention, limits/motors,
   reset distribution and observation/action meanings. Qualify source-bound
   proprioception and PD mapping. Aethyr chapters 31, 33 and 35 remain
   design drafts with no batch/sensor/import implementations; Panda
   receipts cannot supply those missing quadruped capabilities. An SDK
   evaluation lane and an owned-solver lane must retain explicit identities
   and separate evidence.
3. **Walking and scale.** Foot contacts/friction, independent resets,
   sensors, rewards, commanded-speed tracking and a frozen randomized
   training/held-out split are still needed. Then qualify actual training,
   GPU batched physics/model updates, throughput, complete learning-curve
   replay and teacher/student transfer. Original GR00T inference already
   runs on Metal; the new scalar PPO reference is CPU F64. GPU PPO
   training has not been delivered.
4. **Broader Panda reliability.** Source-bound collision geometry and
   clearance queries must precede RRT-Connect/shortcutting. Moving-state
   profiles, finite posture/IK integration, broader disturbances and
   contact-rich controller validation remain. The short owned-query neural
   prefix should grow into full episodes with frozen success/failure goals.
   Replacing the remaining SDK physics/rendering is separate work.
5. **Language and runtime guarantees.** Finish Quantity-operator
   provenance, units/frame migration and transforms, bounded-control and
   scratch/lifetime enforcement, and broader replay/import/device-buffer
   contracts. Fixed arrays and tested runtime ceilings do not complete Coax
   R14 or a real-time guarantee. Current scalar libm receipts do not prove
   cross-target deterministic math.
6. **Other NVIDIA chapters.** Chapters 165 (temporal observations) and 166
   (a complete Cosmos3-Edge path) remain design drafts with empty source
   inventories. Their complete model/codec and physical-evaluation work is
   still outstanding. Generic vision/codec/flow chapters 142–146 also remain
   registry drafts; the vertical GR00T path does not qualify those generic
   APIs. No hardware deployment or hardware qualification has been attempted.

When work resumes, the shortest useful sequence is to pin the quadruped
contract, qualify vector-distribution semantics and a real batched
actor/critic VJP, add the learned critic update, then execute a small frozen
SDK-backed quadruped experiment. Expand to owned contact dynamics and GPU
scale with explicit parity and throughput gates. Keep full-gait acceptance
open until actual held-out walking, complete replay and transfer results exist.

Work is paused after this push at the owner's request. No subsequent chapter
implementation or additional experiment is authorized by this handoff.
