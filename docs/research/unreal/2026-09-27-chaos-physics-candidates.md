# Chaos physics: candidates for `crates/physics`

**Status:** Research note, 2026-09-27. No candidate here is approved work yet;
each needs an issue before implementation. Every item's status is *candidate* until it says otherwise. Studied under the rules in
[AGENTS.md](AGENTS.md).

**Source studied:** Unreal Engine 5.8.3 (`release` at `396c9f05`, 2026-09-22),
sparse checkout of `Engine/Source/Runtime/Experimental/{Chaos,ChaosCore,ChaosVehicles}`,
`Engine/Source/Runtime/PhysicsCore`, and
`Engine/Plugins/Experimental/ChaosVehiclesPlugin`. Paths below are relative
to `Engine/Source/Runtime/Experimental/Chaos/` unless they start with
`Engine/`. This note contains no Unreal code. It describes techniques in our
own words and cites the public source to implement from.

**Compared against:** `crates/physics` at `8e2a99608f` (GP-0 to GP-7 of the
[Genesis port roadmap](../../physics/2026-09-27-genesis-port-roadmap.md)).
Lagrange 1 is the first consumer: an astronaut with a thruster pack grabs
about six drifting parts in zero-g and latches them into a jig. Momentum
ledgers, traces, and replay are acceptance tools.

## Summary

Chaos is built for large scenes under gravity, and much of it is throughput
or stacking machinery we do not need. It is still worth studying in four
areas:

1. **Position correction without injected energy.** Our contacts (`erp` bias
   in `contact.rs`) and hard joints feed position error back as real
   velocity. Momentum is conserved, but energy is not. In zero-g nothing
   removes that energy: a part pushed out of an overlap drifts forever.
   Chaos separates position correction from velocity and caps push-out
   speed. This is the most important gap.
2. **Contact persistence.** We solve every step from zero: no feature IDs,
   no warm start, no static-friction anchors. Chaos matches manifold points
   across frames and reuses their state. Several later items depend on this.
3. **Joint behaviour for grip and latch.** Chaos has capped drives,
   break thresholds separate from force caps, and plasticity. These map
   directly onto Lagrange's grip, which drops the part on the first
   saturated step, and its latch, which snaps a hard weld onto the seat.
4. **Networked determinism.** Chaos adds tick-stamped inputs, a rewind ring
   buffer with resimulation, stable solve ordering, and quantized inputs.
   Remote EVA commands need all of these.

Suggested order: D1 → C1/S1 → S2 → J1 → S3/C3/C4 → J2 → D2. The rest are
opportunistic.

## Solver

### S1. Contact persistence and warm starting — P1, M

- **Unreal:** `Public/Chaos/Collision/PBDCollisionConstraint.h`,
  `Private/Chaos/Collision/PBDCollisionConstraint.cpp`
  (`SavedManifoldPoints`, `AssignSavedManifoldPoints`,
  `FindSimpleSavedManifoldPoint`, `TryRestoreManifold`); cvars
  `p.Chaos.Collision.Manifold.MatchPositionTolerance`,
  `MatchNormalTolerance`.
- **Technique:** Store each manifold point in both shapes' local frames.
  After the narrowphase, match each new point to last frame's point by
  local distance (a tolerance scaled to the object's size) and normal
  agreement. A matched point inherits the saved state (accumulated impulse,
  friction anchor, and first-contact depth).
- **Ours today:** `collision.rs` `Manifold` and `ContactPoint` carry only
  world-space points with no identity. `contact.rs` starts every
  accumulated impulse at zero, and 20 iterations are shared by welds,
  tethers, and contacts.
- **Why it matters:** A tether pulling a part against the jig, or a part
  seated in a slot, converges slowly and depends on the iteration count.
  Warm starting fixes that, and S2, S3, and C1 all need a persistent
  per-point state. Key the cache by (collider pair, feature ID), iterate it
  in sorted order, and serialize it with `World` so replay stays exact.
- **Public reference:** Catto, "Iterative Dynamics with Temporal Coherence"
  (GDC 2005) and "Fast and Simple Physics using Sequential Impulses" (GDC
  2006); Box2D v3 feature IDs (MIT); Jolt `ContactConstraintManager` (MIT).

### S2. Soft contacts with a relax pass (no energy from push-out) — P1, M

- **Unreal:** `Public/Chaos/Collision/PBDCollisionSolver.h`
  (`ApplyPositionCorrectionNormal`, split-impulse option `bUseSplitImpulse`,
  and a velocity phase allowed to remove speed that position correction
  created).
- **Technique:** Correct overlap in a way that does not survive as
  velocity. Chaos corrects positions in a separate phase, then lets the
  velocity phase cancel the closing speed that correction added.
- **Ours today:** Contacts use `erp·(depth − slop)/dt` (`contact.rs`, `erp`
  0.2). Hard point and weld joints use `erp/dt` in the joint rows. Both push
  real separation velocity into the bodies. Soft joints (`soft_step`)
  already use Catto's soft-constraint coefficients.
- **Why it matters:** A latched weld snapping shut, a deep speculative
  contact after a thruster burst, and a part spawned in overlap all add
  kinetic energy that zero-g never removes. The ledger can measure this, so
  add an energy-drift test first and record the baseline.
- **Plan:** Rather than switch to position-based dynamics, reuse `soft_step`
  for contacts and hard rows, then add a relax iteration with no bias, as
  in Box2D v3's soft step. This stays velocity-based, so impulse reports and
  the ledger keep working.
- **Public reference:** Catto, "Solver2D" (2024 blog) and Box2D v3
  `b2_softStep`; Catto, "Soft Constraints" (GDC 2011). Bullet split impulse
  (zlib) is a secondary source.

### S3. Cap push-out speed for initial overlaps — P1, S

- **Unreal:** `Private/Chaos/Collision/PBDCollisionContainerSolver.cpp`,
  `PBDCollisionConstraint.cpp` (`InitialOverlapDepenetrationVelocity`, the
  larger of the two particles' values; `MaxPushOutVelocity`; per-point
  initial depth).
- **Technique:** A contact that first appears already overlapped records its
  depth as an allowed floor. The floor shrinks by (push-out speed × dt) each
  step, so the pair separates gently. All corrections are also capped per
  step.
- **Ours today:** No cap.
- **Why it matters:** Scene authoring, edited replay states, and reaching
  through overlaps all create initial overlaps. In zero-g a violent
  separation becomes permanent drift. This needs S1's "first seen" flag.
- **Public reference:** Box2D v3 `contactPushMaxVelocity` / `contactSpeed`
  (about 3 m/s); PhysX `maxDepenetrationVelocity` (BSD-3).

### S4. Restitution threshold scaled to the scene — P1, S

- **Unreal:** `Public/Chaos/PBDRigidsEvolutionGBF.h`
  (`DefaultRestitutionThreshold`, which is an acceleration); the solver
  multiplies it by dt to get a velocity.
- **Technique:** Express the bounce cutoff as "one step of acceleration" so
  it tracks the scene and the step size.
- **Ours today:** `SolverSettings::bounce_threshold = 0.5` m/s, with a
  gravity justification. EVA parts bump at 0.1–0.4 m/s, so restitution
  never fires at the speeds that actually occur in Lagrange.
- **Plan:** Derive the threshold from the field's acceleration × dt with a
  small floor for zero-g, and keep bouncing on the pre-solve `closing`
  speed.
- **Public reference:** Box2D `restitutionThreshold`.

### S5. Static friction anchors — P2, M (after S1)

- **Unreal:** `Public/Chaos/Collision/PBDCollisionSolver.h` (static friction
  in the position phase, `StaticFrictionRatio`, anchors moved to the cone
  edge on slip); `PBDCollisionConstraint.cpp` (`bHasStaticFrictionAnchor`,
  `ShapeAnchorPoints`, `p.Chaos.Collision.Manifold.EnableFrictionRestore`).
- **Technique:** Store the touch point on both bodies when contact starts.
  While the correction stays within the static friction cone, pull the
  bodies back to that anchor. Past the cone, apply dynamic friction and
  slide the anchor to the cone edge.
- **Ours today:** Velocity-level friction with one coefficient. A seated
  part creeps under a weak tether load, and how far depends on the
  iteration count.
- **Why it matters:** Parts must stay put in jig slots, and separate static
  and dynamic coefficients give a real breakaway. The Genesis
  `friction_breakaway` reference already motivates this.
- **Public reference:** Müller et al., "Detailed Rigid Body Simulation with
  Extended Position Based Dynamics" (2020), static friction section.

### S6. Sleep on smoothed velocity, with hysteresis and size scaling — P2, S

- **Unreal:** `Private/Chaos/Island/IslandManager.cpp` (sleep counter,
  `VSmooth`/`WSmooth` low-pass with `p.Chaos.SmoothedPositionLerpRate`,
  `AngularSleepThresholdSize`, 10× wake multipliers).
- **Technique:** Test for sleep against a filtered velocity, so a one-step
  jitter does not reset the timer. Require a larger disturbance to wake a
  body than to put it to sleep. Convert the angular threshold to a linear
  speed at the body's extent.
- **Ours today:** `World::settle` tests raw speed against 0.01 m/s and
  0.02 rad/s, with the same threshold both ways. A long truss and a bolt
  get the same angular threshold. The microgravity rule is ours and stays.
- **Public reference:** Box2D v3 sleep: the larger of |v| and
  maxExtent·|ω|, with a per-body threshold.

### S7. Average-point pass for flat impacts — P2, S

- **Unreal:** `SolveVelocityAverage` in `PBDCollisionSolver.h`;
  `p.Chaos.PBDCollisionSolver.Velocity.AveragePointEnabled`.
- **Technique:** With several active manifold points, solve one extra
  normal row at their centroid, so restitution acts through the centre
  rather than point by point.
- **Why it matters:** When a box face hits the station flat, per-point
  Gauss-Seidel creates a false spin. The angular ledger counts it as a real
  transfer.

### S8. Speed and rotation limits, off by default — P2, S

- **Unreal:** `MaxAngularSpeedSq` and `MaxLinearSpeedSq` in the integrate loop
  of `Private/Chaos/PBDRigidsEvolutionGBF.cpp`.
- **Why it matters:** A thruster misfire on a light part can spin it until
  the speculative margin (`omega·(offset+bound)·dt` in `World::step`)
  becomes huge. Add the limit only as a guard, and record every clamp in the
  ledger as a non-conservative event, like `slept`.
- **Public reference:** Box2D's `b2_maxRotation` (a quarter turn per step).

### S9. Opt-in damping, inertia conditioning, solve priority — P3, S

- **Damping** (`LinearEtherDrag`/`AngularEtherDrag`): default it to zero,
  since space has no drag. If added for gameplay feel, use the implicit form
  v ← v / (1 + h·c) and put the lost momentum in the ledger.
- **Inertia conditioning** (`Private/Chaos/MassConditioning.cpp`,
  `p.Chaos.Solver.InertiaConditioning.*`): this inflates the inverse inertia
  of thin bodies inside the solver. It breaks exact angular accounting, so
  allow it only per body, opt-in, and flagged in the ledger.
- **Solve priority** (`p.Chaos.Solver.{Collision,Joint}.Priority`): we always
  solve joints before contacts. Gauss-Seidel favours whatever is solved
  last, so measure whether solving welds last seats latched parts better.

## Collision

### C1. Pair exemptions and no collision between jointed bodies — P1, S

- **Unreal:** `Public/Chaos/Collision/CollisionFilter.h`, the
  `IgnoreCollisionManager` hook, and the joint setting that disables
  collision between the joined pair.
- **Ours today:** Only colliders on the same body are skipped, and bitmasks
  cannot express one specific pair.
- **Why it matters:** A welded part in its slot, a tethered tool against
  the suit, and the pack against the astronaut should stop fighting.
- **Public reference:** Box2D `collideConnected`; Rapier joint
  `contacts_enabled` (Apache-2.0).

### C2. Sensor colliders — P1, S

- **Unreal:** `bIsProbe` in `Private/Chaos/Collision/ParticlePairMidPhase.cpp`;
  `ContactModification.h` can turn a pair into a probe.
- **Technique:** Run the full narrowphase and report contacts, but create no
  solver rows.
- **Why it matters:** "Part is in the slot" and "hand is in grab range"
  become deterministic contact events, replacing hand-written distance
  checks in the zone.
- **Public reference:** Box2D v3 sensor shapes; Rapier `Sensor`.

### C3. Box–box reference-face hysteresis — P1, S

- **Unreal:** `Private/Chaos/CollisionOneShotManifolds.cpp` (face-over-edge
  bias, `p.Chaos.Collision.Manifold.PlaneContactNormalEpsilon`; GJK warm
  start). The specific gap below comes from the public sources.
- **Ours today:** `box_box` prefers faces to edges, but picks between A's
  face and B's face by a strict maximum. A part lying flush in the jig can
  swap reference faces every frame, and its manifold points jump.
- **Plan:** Add relative and absolute tolerance favouring the previous
  reference (or A), and optionally cache the separating axis per pair.
- **Public reference:** Box2D Lite `Collide` (relative tolerance 0.95);
  Gregorius, "The Separating Axis Test between Convex Polyhedra" (GDC 2013).

### C4. Capsule manifolds that share one normal — P1, S–M

- **Unreal:** `Private/Chaos/CollisionOneShotManifoldsMiscShapes.cpp`
  (capsule against convex and capsule; `CapsuleAxisAlignedThreshold`,
  `CapsuleDeepPenetrationFraction`).
- **Technique:** Find the closest feature first. If the capsule axis is
  nearly parallel to a face, clip the segment to the face's side planes
  and emit two points with that face's normal.
- **Ours today:** `capsule_box` evaluates each point separately with
  `box_distance`. A capsule overhanging an edge or deep inside a box gets
  points whose normals disagree and fight.
- **Why it matters:** Tethers, tool handles, and struts are capsules.
- **Public reference:** Gregorius, "Robust Contact Creation for Physics
  Simulations" (GDC 2015); Jolt `ManifoldBetweenTwoFaces`.

### C5. Direction-aware speculative margin — P1, S

- **Unreal:** `DoBoundsOverlap` in `ParticlePairMidPhase.cpp` (grows bounds
  by relative motion); `FCCDHelpers::DeltaExceedsThreshold` in
  `Private/Chaos/CCDUtilities.cpp` (motion per axis against the thinnest
  extent).
- **Ours today:** `World::step` adds both bodies' full speeds in every
  direction, so parts moving apart or side by side still get large margins
  and ghost-contact risk.
- **Plan:** Grow bounds by relative displacement, and accept a speculative
  point only if `sep < base + max(0, −v_rel·n)·dt`.
- **Public reference:** Catto, "Continuous Collision" (GDC 2013).

### C6. Contact modification hook — P2, S

- **Unreal:** `Private/Chaos/ContactModification.cpp`,
  `MidPhaseModification.cpp`, `CCDModification.cpp`.
- **Technique:** A callback edits manifolds between detection and solve. It
  can disable a point, override the normal or material, or make a pair a
  probe.
- **Why it matters:** One-way slot guides, glove grip rules, and custom slot
  friction without forking the solver. Run it in manifold order for replay.
- **Public reference:** Box2D v3 `b2PreSolveFcn`; PhysX
  `PxContactModifyCallback`.

### C7. Time-of-impact CCD for fast bodies — P2, M

- **Unreal:** `Private/Chaos/CCDUtilities.cpp`
  (`FCCDManager::ApplyIslandSweptConstraints`), `Collision/GJKContactPointSwept.cpp`,
  `GJKRaycast2` in `Public/Chaos/GJK.h`.
- **Technique:** Sweep bodies that move past a threshold, and handle pairs in
  order of impact time. Rewind each body to its impact pose with a small
  target penetration, re-sweep anything it touches (with a cap), then push
  out remaining deep overlaps against static bodies last.
- **Why it matters:** Speculative contacts miss grazing hits and thin
  rotating plates, which thruster-driven approaches to jig plates can
  produce. Start with dynamic against static only.
- **Public reference:** Catto, "Continuous Collision" (GDC 2013),
  `b2TimeOfImpact`; Mirtich (1996).

### C8. Broadphase split between static and dynamic — P2, M

- **Unreal:** `Public/Chaos/AABBTree.h` (fattened leaves, dirty grid),
  `ISpatialAccelerationCollection.h` (separate static and dynamic
  structures).
- **Ours today:** O(n²) `detect` and linear `raycast`.
- **Why it matters:** A station built from many boxes creates many
  static–static pairs. Emit pairs in sorted `(i, j)` order (D1), and share
  the structure with queries (D4).
- **Public reference:** Box2D `b2DynamicTree`; Rapier SAP.

### C9. GJK/EPA convex hulls, manifold restore, point ordering — P3

- **Convex hulls** (`Public/Chaos/GJK.h`, `Convex.cpp`): needed only if
  assets outgrow boxes and capsules. Compounds may be enough. Build from van
  den Bergen's book, Gregorius "Implementing GJK" (2013), or parry3d.
- **Manifold restore** (`TryRestoreManifold`): skip the narrowphase when the
  pair's relative pose barely moved. Mostly a speed win.
- **Point ordering** (`p.Chaos.Collision.SortMeshManifoldByDistance`): solve
  points near the centre of mass first to add less spin in the first
  iteration.

## Joints

### J1. Break threshold separate from the force cap — P1, S

- **Unreal:** `Private/Chaos/Joint/PBDJointContainerSolver.cpp`
  (`GetJointShouldBreak`, `LinearBreakForce`, `AngularBreakTorque`);
  `PBDJointConstraints.cpp` (`BreakConstraint`, break callback).
- **Technique:** After the solve, convert the applied impulse to a force and
  torque, and mark the joint broken if either exceeds its own threshold. The
  threshold is separate from any drive's cap.
- **Ours today:** `max_force`/`max_torque` are both the transmission cap and,
  through `saturated`, the break trigger. `station.rs` releases the grip on
  a single saturated step.
- **Plan:** Give `Joint` a break threshold and a `broken` flag set inside the
  step. Add a few steps of hysteresis (our addition) so a single impact
  spike does not drop the part.

### J2. Capped capture drive for latching and winching — P1, M

- **Unreal:** `Public/Chaos/PBDJointConstraintTypes.h` `FPBDJointSettings`
  (position and velocity drive targets, SLERP drive, `LinearDriveMaxForce`,
  `AngularDriveMaxTorque`); `Private/Chaos/Joint/PBDJointCachedSolverGaussSeidel.cpp`.
- **Technique:** A spring-damper toward a target pose, and optionally a
  target velocity, capped per axis. The rotation drive acts along the
  shortest-arc error.
- **Ours today:** A soft weld is already a position drive toward a fixed
  relative pose. It lacks a velocity target, a target that can change while
  the joint lives, and per-axis caps. The latch snaps a hard weld onto the
  `seat` and lets `erp` close the gap, which injects energy (see S2).
- **Plan:** Pull the part into the slot under a force budget, then switch to
  a hard weld below a tolerance. The same drive with a velocity target along
  a tether is a winch.
- **Public reference:** Catto, "Soft Constraints" (GDC 2011); Jolt
  `SixDOFConstraint` motors.

### J3. Plastic grip (re-seat instead of break) — P2, S

- **Unreal:** `ApplyPlasticityLimits` in `Private/Chaos/PBDJointConstraints.cpp`
  (`LinearPlasticityLimit`, `AngularPlasticityLimit`).
- **Technique:** When the relative pose drifts past a limit from the target,
  move the target to the current pose. The joint yields permanently.
- **Why it matters:** A heavy part twisted in the glove slips to a new hold
  instead of dropping. This sits between J1's "holds" and "breaks".
  Rebasing the anchor after the step changes no velocity, so momentum is
  untouched.

### J4. Joint error readout — P2, S

- **Unreal:** `GetJointIsViolating` in `PBDJointContainerSolver.cpp`
  (`LinearViolationCallbackThreshold`, `AngularViolationCallbackThreshold`).
- **Technique:** Report the joint's remaining linear and angular error after
  the final iteration, with a threshold flag.
- **Ours today:** `angle_error()` exists, but the linear error is not stored.
- **Why it matters:** A soft grip can stretch far without saturating, and a
  HUD cue for "hands pulled apart" needs the error, not the force. It also
  makes latch tolerances testable.

### J5. Six-axis joint with swing/twist limits — P2, L

- **Unreal:** `EJointMotionType`, `LinearLimit`, `AngularLimits`;
  `Private/Chaos/PBDJointConstraintUtilities.cpp`
  (`DecomposeSwingTwistLocal`, elliptical cone error); soft limits and limit
  restitution.
- **Technique:** Split relative rotation into twist about the joint axis
  and swing of that axis. Each can be free, locked, or limited by a cone or
  pyramid, and each limit can be hard or soft. Limit rows switch on only
  within a speculative distance.
- **Why it matters:** A wrist grip that pivots within a cone, hinged jig
  clamps and hatches, and tether end fittings. Implement as velocity rows in
  our block solver.
- **Public reference:** Müller et al. 2020 (hinges and swing/twist limits);
  swing-twist decomposition (Dobrowolski 2015).

### J6. Merge welded assemblies into one body — P2, M

- **Unreal:** `Public/Chaos/ClusterUnionManager.h`, `PBDRigidClustering*`.
- **Technique:** Merge bodies into one compound with combined mass, centre
  of mass, and inertia, and split them again later.
- **Why it matters:** A weld chain is only as stiff as the solver iterations
  allow. If an assembled keel ever leaves the jig, a compound body is exactly
  rigid and cheaper. Merging and splitting can conserve linear and angular
  momentum exactly, and the ledger can test that.
- **Public reference:** Parallel-axis theorem and compound bodies
  (Featherstone, *Rigid Body Dynamics Algorithms*).

### J7. Parent/child mass conditioning — P3, S

- **Unreal:** `ConditionInverseMassAndInertia`, `MinParentMassRatio`,
  `MaxInertiaRatio` in `PBDJointConstraintUtilities.cpp`.
- **Note:** Only do this if a chain-convergence test fails (astronaut → grip
  → part → contact → jig). Size impulses with the conditioned masses, but
  apply them with the true ones, or momentum breaks.

## Determinism, networking, and tooling

### D1. Determinism invariant and its test — P1, S

- **Unreal:** `CHAOS_DETERMINISTIC` and stable particle IDs in
  `Public/Chaos/GeometryParticles.h`; `SetIsDeterministic` in
  `Public/PBDRigidsSolver.h`; sorted collision keys in
  `Public/Chaos/Collision/CollisionKeys.h`.
- **Technique:** When determinism or rewind is on, sort active constraints
  each tick by a key built from the body pair, so solve order does not depend
  on hash maps or the order the broadphase emits pairs.
- **Ours today:** Deterministic by accident: `BodyId`s are never reused and
  `detect` is an index-order loop. S1, C8, or any parallelism would break
  this silently.
- **Plan:** State "manifolds and joints are solved in sorted id order" as an
  invariant in the crate docs, and test it before optimizing.
- **Public reference:** Box2D v3 deterministic solver sets; Jolt
  `CROSS_PLATFORM_DETERMINISTIC`.

### D2. Tick-stamped commands, rewind ring buffer, resimulation — P1, M+L

- **Unreal:** `Public/Chaos/SimCallbackObject.h`, `SimCallbackInput.h`,
  `ChaosMarshallingManager.h` (inputs tagged with a timestamp and a step
  count, consumed at named phases); `Public/RewindData.h`
  (`FRewindData`, `FFrameAndPhase`, `EDesyncResult`, `FindValidResimFrame`,
  `BlockResimFrame`, resim cooldown).
- **Technique:** Commands land on a definite tick and are pure inputs to the
  step. Keep a fixed-length history of states. When authoritative state
  disagrees beyond a leniency threshold, rewind to the earliest valid frame
  and replay the buffered commands. Some changes (adding bodies, creating
  joints) block rewinding past them, and a cooldown rate-limits resims.
- **Ours today:** A cloned or serialized `World` is already a perfect
  snapshot, and `Trace::compare` finds the first divergence. There is no
  command stream in the crate, no history, and no rewind loop. GP-0 plans
  tick-stamped NIP-MV commands at the zone level.
- **Plan:** Snapshot the whole `World` into a ring buffer, which is cheap at
  EVA body counts. Treat joint creation and latching as rewind barriers
  until they are part of the recorded commands.
- **Public reference:** Fiedler, "Deterministic Lockstep", "State
  Synchronization"; Rocket League, "It IS Rocket Science" (GDC 2018);
  Overwatch netcode (GDC 2017).

### D3. Quantized inputs and correction smoothing — P2, S

- **Unreal:** `Engine/Source/Runtime/Experimental/ChaosVehicles/ChaosVehiclesCore/Public/SimModule/ModuleInput.h`
  (quantization, decay while extrapolating, clear once consumed);
  `Public/Chaos/Framework/PhysicsProxyBase.h` (render error-correction
  cvars).
- **Technique:** Quantize throttles to N bits before simulating and before
  sending, so client and server simulate identical values. Clear
  trigger-like inputs (grip, weld) once consumed. After a resim correction,
  decay the visual offset over a short window, or snap when it is past a
  threshold. That offset is visual only and never feeds back into the
  simulation.
- **Public reference:** Fiedler, "Snapshot Compression", "Snapshot
  Interpolation".

### D4. Overlap and sweep queries — P1, M

- **Unreal:** `Public/Chaos/GeometryQueries.h` (`OverlapQuery` with
  minimum-translation output, `SweepQuery`, inflation), `Collision/SimSweep.h`.
- **Technique:** An overlap query reports whether shapes intersect and the
  minimum translation that separates them. A sweep reports time of impact,
  point, and normal, and reports an initial overlap separately.
- **Ours today:** Raycasts only (`sensors.rs`). The narrowphase already
  computes separation for each shape pair, so an overlap query is mostly
  plumbing.
- **Why it matters:** A "does the part fit here?" check before a weld, and
  a capsule sweep to vet remote thruster commands before they run.
- **Public reference:** Ericson, *Real-Time Collision Detection*; Jolt
  `CastShape`/`CollideShape`; Rapier `cast_shape`.

### D5. Brute-force oracle for accelerated queries — P1, S (with C8/D4)

- **Unreal:** `Engine/Source/Runtime/PhysicsCore/Public/SQVerifier.h`,
  `SQCapture.h`.
- **Technique:** Capture a query together with its scene, and compare the
  accelerated result against a reference.
- **Plan:** When C8 lands, keep today's linear `detect`/`raycast` as the
  oracle. Property tests on random scenes then assert that the accelerated
  results match, in the same order.

### D6. Material combine modes and static/dynamic friction — P2, S

- **Unreal:** `Engine/Source/Runtime/PhysicsCore/Public/PhysicsSettingsEnums.h`
  (`EFrictionCombineMode`), `PhysicalMaterials/PhysicalMaterial.h`
  (`StaticFriction`, per-material override), `Public/Chaos/PhysicalMaterials.h`
  (`ChooseCombineMode`).
- **Ours today:** Friction uses the geometric mean and restitution uses the
  max, both hard-coded (`collision.rs` `Material`).
- **Why it matters:** The Genesis oracle uses different combine rules, which
  partly explains the restitution mismatch noted in `oracle.rs`. A small
  enum with priority resolution lets us match each oracle. Static friction
  feeds S5.
- **Public reference:** PhysX `PxCombineMode`.

### D7. Recording format and staged steps — P2, M

- **Unreal:** `Public/ChaosVisualDebugger/ChaosVisualDebuggerTrace.h`
  (keyframes plus changed-only frames, per-stage data channels, a string
  table, recorded scene queries and resim events); `Public/Chaos/Framework/DebugSubstep.h`.
- **Technique:** Record periodic full keyframes and, in between, only the
  bodies that changed. Stage snapshots (post-integrate, pre-solve,
  post-solve) and query and resim channels can each be toggled.
- **Ours today:** `Trace` samples every body every tick, and `debug_lines`
  covers contacts and joints.
- **Plan:** Extend `Trace` with contact reports, commands (D2), and query
  results, and keyframe it so a long EVA session can be opened mid-run.
  Splitting `World::step` into callable stages would also let tests observe
  state between them.
- **Public reference:** PhysX Visual Debugger; Jolt `DebugRendererRecorder`.

### D8. Deferred force queue with provenance — P2, S

- **Unreal:** `Engine/Source/Runtime/Experimental/ChaosVehicles/ChaosVehiclesCore/Public/SimModule/DeferredForcesModular.h`.
- **Technique:** Modules queue forces, forces at a point, torques, and
  impulses, each tagged with its source. One place applies them, and the
  same queue can be drawn.
- **Why it matters:** One queue would feed the ledger's named impulses, the
  debug overlay, and the recording (D7), and make applied forces replayable.

## Rejected

| Unreal feature | Why not |
| --- | --- |
| Full quasi-PBD solver (`PBDRigidsEvolutionGBF`, position/velocity/projection iterations) | A rewrite. S2's soft step gets most of the benefit and keeps impulse reports and the ledger. |
| Shock propagation (`MinInvMassScale`, collision and joint variants) | Makes the lower body artificially heavy. It exists for gravity stacks, and it breaks equal-and-opposite momentum. |
| Joint projection and teleport (`ApplyProjections`, `TeleportDistance`) | Moves only the child and hides solver error; not momentum-conserving. |
| Implicit gyroscopic torque | Slowly loses rotational energy. `Body::rotate` already conserves angular momentum exactly. |
| Graph colouring, Jacobi/SIMD/ISPC solvers, marshalling threads | Throughput machinery with batch-dependent results. At about 10 bodies, a serial deterministic solve is better. |
| Partial island sleep and momentum propagation | Heuristic energy transfer through sleeping bodies conflicts with the ledger. |
| Randomized constraint order | A test aid that works against determinism. |
| Resim follower particles and `FEvolutionResimCache` | Partial-resim optimizations for thousands of bodies. Whole-world snapshots are simpler at our scale. |
| Variable time steps (`ITimeStep` variants) | They break replay. `FixedStep` is the right design. |
| Character ground constraint, suspension, wheels, aerofoils | They assume gravity or ground contact, and neither exists in vacuum EVA. |
| Levelset, trimesh, and heightfield paths; fracture clusters; stress solver | No meshes or fracture in scope. The stress solver assumes gravity. |
| One-way interaction stiffness and CCD clipping when over budget | They give up correctness for frame time, which is the wrong trade for replay. |
| Rewriting manifold reduction | Our `reduce` already does effectively the same as `ReduceManifoldContactPoints`. |
