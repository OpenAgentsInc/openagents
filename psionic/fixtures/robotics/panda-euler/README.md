# One unconstrained Panda Euler step

This is the owned, fixed-source nine-coordinate map for the already-qualified
Panda robot: seven hinge angles in radians and two signed finger slides in
metres. It uses the same body/inertial, armature, actuator, joint-damping and
fluid coefficients as the preceding owned queries. Writable source PODs do
not certify model provenance. Only the exported fixed-source `peu_step` and
explicitly raw `peu_step_raw` perform the stated admission and publication.
Low-level helpers remain visible to Coax source and require their documented
trusted inputs; absence from a C header is not module sealing.

The exact API and experiments were frozen in `experiment.json` before product
implementation or comparison. `cases.f64` contains six rows of 36 little-endian
F64 words: q9, v9, controls9 and external-generalized-force9. The first three
are algebraic targets formed from retained original observations at identical
q/v. They are not recorded SDK timestep endpoints. The other three are the
new, separately reset SDK one-step requests; their acquisition and independent
native/reference receipts are recorded separately.

The pinned source is MuJoCo 3.3.1 at
[`a209134315dda126a714529bfa80f7478902644d`](https://github.com/google-deepmind/mujoco/tree/a209134315dda126a714529bfa80f7478902644d).
Its [Euler implementation](https://github.com/google-deepmind/mujoco/blob/a209134315dda126a714529bfa80f7478902644d/src/engine/engine_forward.c)
uses implicit joint damping for this profile. With a fixed typed step of
0.002 seconds, this map performs:

```jai
net = ((actuator + external) + full_passive) - bias
effective_mass = mass + h * diagonal(joint_damping)
effective_acceleration = solve(effective_mass, net)
next_velocity = velocity + h * effective_acceleration
next_position = position + h * next_velocity
```

Full passive already contains joint damping plus fluid. Damping is included
once in the net force and also modifies the implicit left-hand matrix; do not
remove it from the net, add `B*v` back, or subtract it a second time. Fluid,
actuator position gains, gravity and velocity bias remain explicit at the old
q/v. Armature is already included once in the mass matrix. The instantaneous
smooth acceleration from `psf_forward` is a different result and is not used
as this integrator's effective acceleration.

The typed matrix update retains hinge/hinge inertia in kg m²/rad², cross
inertia in kg m/rad and slide/slide inertia in kg. Multiplying typed seconds
by the appropriate angular/linear damping yields the matching diagonal
inertia. The existing explicit one-radian/one-metre basis then feeds the raw
F64 factorization. No implicit removal of the angle dimension is introduced.

All nine typed span shapes precede values. External forces are admitted
before q/v, then controls; the actuator and owned forward-query primitives
retain their original boxes. Both assembled applied force and bias-subtracted
net force have their own finite inclusive ±10000 cap. Output q/v is checked
again after integration: inclusive ±100 rad/±1 m and ±20 rad/s/±2 m/s. A next
state outside those numerical boxes refuses without publishing any output;
the map never clamps a projected state. These boxes are not source joint
soft limits or hardware safety limits. Controls retain their own qualified
source clipping and counts. A zero external-force array explicitly supplies
no generalized external force; there is no hidden fallback or force effect.

Publication is last and complete. The output stores q9, v9, effective
acceleration9, then the control/force clipping counts. The helper's returned
effective acceleration is not the SDK's post-step `d->qacc`: the SDK uses a
scratch solve for implicit Euler without overwriting that instantaneous
acceleration field. A reference formed from `(v_next-v)/h` is a derived
diagnostic, not a direct SDK field read.

The new SDK experiment disables both CONSTRAINT and CONTACT (flags 17), while
leaving the other used flags clear. This deliberately excludes joint
frictionloss, limits and contact constraints. Contact-only disabling would
not establish the same unconstrained map. Original flags 0 and all source
coefficients remain recorded. This is not an untouched LIBERO task profile,
an object/contact solver, a clock or real-time scheduler, generic model
import, compiler `#bounded`, a hardware controller, or a learned episode.

The arithmetic assembles force in the displayed owned source order, which can
round differently from the SDK's sum order. Frozen tolerances remain absolute
2e-12 for q, 2e-10 for v and 1e-8 for the derived effective acceleration,
with relative tolerance zero. Native same-recipe bit identity is separate
from SDK tolerance parity and does not imply cross-target determinism.

Only the new Euler units and fixtures are checked here. Qualified FK, mass,
bias, actuator, fluid and solve proofs carry by exact source identity; their
old numerical suites, SDK captures and neural episodes are not repeated.

`checks-qualification.json` records 12 new Coax groups/163 cases and 151/151
new-root mutants killed without waivers, plus four precise source unit
refusals. The initial surviving mutations are retained: they exposed a redundant
rate admission and missing value observations. The final checked force helper
observes independent applied/net caps; independent cached targets and a dense
cross-component residual observe the remaining assembly. The original passing
math mutation result carries unchanged.

`qualification.json` records the first native debug/release/fail-fast ASan+UBSan
run: 4,380 assertions/profile, the complete six-record status/state/count payload byte-identical across profiles.
All 162 actual floating fields, 81 cached target fields and 462 SDK observations
are finite and complete before comparisons. It covers malformed spans, every
nonfinite input lane, force caps, refusal precedence, 18 projected-position and
two projected-rate refusals, canaries/spare output/input preservation, and four
overlap variants. The late-state cases first prove the separately qualified
instantaneous smooth path admits the same inputs. Of 72 inclusive initial-box
endpoint queries, 12 refuse downstream; initial admission is not successful
next-state publication.

The three observed SDK steps agree to maximum absolute q/v/effective-acceleration
errors 1.3553e-20, 1.1103e-16 and 3.5528e-14, respectively, under the unchanged
budgets. Clipping counts match the independent source scalar definition on the
actual controls. Hinge and slide coefficient errors are also recorded separately
because their physical units differ. The cached algebraic targets have separate
metrics and remain explicitly distinct from observed SDK endpoints.

The prewritten `example-reference.json` and `example-qualification.json` bind
the runnable example's independent golden and its passing debug/release stdout.
Run the example or the new offline native gate with:

```shell
coax/bin/coax run psionic/examples/panda-euler.coax
COAX=coax/bin/coax psionic/tests/robotics/panda_euler_check.sh /tmp/panda-euler-new-proof
```

The native gate defaults to these public inert reference fixtures. It compiles
only the new Euler consumer, never loads the SDK, captures new observations,
re-runs Gaussian references or runs a neural model. Historical source unit
refusals and example records remain attached to their actual compiler identity.
