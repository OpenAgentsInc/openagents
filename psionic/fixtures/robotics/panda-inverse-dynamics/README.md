# Fixed robot9 inverse force

The owned Coax query returns the generalized force for supplied positions,
velocities and accelerations: `M(q)*a + c(q,v) + g(q)`. It does not advance
physics or choose a motor command. The fixed source model has16 selected
bodies: base/link0 have no generalized motion, while all14 moving descendants
(including the four finger bodies) enter the world kinematic recurrence.
Massless bodies contribute no wrench. The nine armatures contribute once.

`experiment.json` and `cases.f64` froze24 complete q9/v9/a9 rows before new
implementation/reference results:15 named SDK contexts and9 seeded owned
cases. q order is seven radians then two metres; v and a carry the corresponding
per-second and per-second-squared dimensions. The source model is the original
demo0 LIBERO scene, not a manufacturer-identified physical Franka. Original
XML hash is `13e900d1086ecee8e16df2876104e6b6c95e06c46a14b5860ccebb16ee1af7e5`;
retained rewritten XML hash is
`95a4049c1454ffd77b2185f0a21966832c5be55af67f64c2b110cc32e617d02f`.
The source gravity, normalized attachments/inertial frames, masses/COMs/
principal inertias and9 armatures are inherited from the qualified M9 model.

The implementation propagates BODY-origin world linear/angular velocity and
acceleration. Hinges add axis speed/acceleration and the moving-axis term;
slides add world-axis translation and twice the parent-angular-velocity cross
slide velocity. COM acceleration adds angular-acceleration and centripetal
offset terms. Each body's force is mass times COM acceleration minus gravity;
its COM torque is `I_world*alpha + omega cross (I_world*omega)`. The qualified
COM columns project each wrench into generalized force. This is world
Newton–Euler body arithmetic plus Jacobian projection, not a claim of a
generic CRBA/RNEA importer or a complete simulation backend.

`pid_inverse` takes shapes7/2 for q, v and a, then one output. All shapes refuse
first. Position admission delegates to `pff_forward`; velocity7, slide-velocity2,
acceleration7 and slide-acceleration2 then check in that order, each index
checking finite number before range. Inclusive numerical bounds are±100rad/
±1m,±20rad/s/±2m/s and±200rad/s²/±20m/s². These are not hardware or source soft
limits. `pid_inverse_raw` explicitly labels three9-F64 views and requires all
shapes9/9/9/1 before copying. Both return `PfkStatus` (zero success), preserve
the output on refusal and publish a completed local output once.

`PidOutput` has7 `PmHingeGravity` and2 `PmSlideGravity` quantities. The existing
unit-tag names mean energy/radian and force; they do not restrict the value
to a gravity contribution. The explicit one-radian inertia/generator convention
makes generalized force times rate physical power. Native ABI is exactly9F64
words. All low-level pose/motion/profile helpers and PODs remain unsealed and
Coax-source-visible: they require trusted valid poses/source constants and do
not authenticate arbitrary models, snapshots, frames or writable output data.
Only the exported fixed-source admitted forward query constructs its profile.

## Independent references

The new opt-in C collector uses the installed MuJoCo3.3.1 binary SDK and performs
15 reset+forward contexts,15 mass expansions,15 raw body-RNE calls,15 site
Jacobians and15 world SITE-velocity calls. There are zero integration steps,
renderers, neural calls or imported LIBERO/robosuite/NVIDIA scripts. Twelve
contexts have nonzero velocity and three are zero-rate baselines. Each supplied
a is explicit; the collector never treats forward-solver qacc as that input.

`sdk-profile.json` and `sdk-pc-sample-profile.json` were frozen before the first
query. The primary independent force is SDK M81 times the chosen a9 plus its
qfrc_bias9. Passive damping/friction/fluid, actuator and contact/constraint
forces are separate. Public raw `mj_rne(...,1)` omits joint armature*a; the
separately recorded body RNE therefore needs that diagonal term exactly once.
Its135 corrected values match the primary construction within7.99361e-15
under the predeclared5e-11 absolute bound. `mj_inverse` and qfrc_inverse are
not used. The exact SDK versioned source defines these separate contracts.

The same forward generations supply NEW full PcSample144 observations in
`sdk-pc-samples.f64` (15×144 LE F64). They contain grip-site position/matrix,
q/qd, M7, Jpos/Jrot, world linear/angular SITE velocity, arm bias and actuator
limits. The separate PcSample receipt records raw SDK evidence only; it does
not qualify an owned sample adapter or controller. One zero-rate context,
case5, has old M/bias subfields identical to the earlier finger capture row4.
Its previously unmeasured matched J/velocity/limits justify the new whole
observation; the old M9/g0 proof is carried, not rerun as a new qualification.

The second oracle uses full-angle homogeneous matrices and analytic first
matrix derivatives propagated by the product rule. It derives COM/angular
columns without the product quaternion/cross recurrence, forms kinetic metrics
in principal coordinates, and only then differences the metric for Christoffel
velocity bias. This avoids nested COM finite differences and never uses the
Coax M9 product as its oracle. Primary h=1e-5 was frozen before comparison;
the full1e-3…1e-7 sweep remains in the record, including the1e-3 diagnostic that
misses the primary bound. These are derivatives of the smooth geometric model,
not float-rounding or admission derivatives.

`qualification.json` records17 new Coax groups/136 top-level cases and250/250
killed focused mutants (math63,motion49,force47,wrapper91; no waivers). In EACH
debug/release/fail-fast ASan+UBSan mode,24 serialized cases plus54 inclusive
boundary cases pass2,507 assertions. All216 force words match byte for byte
across the named Darwin-arm64 separate scalar-libm profile. All135 SDK forces
have maximum error2.48690e-14 under5e-10 absolute; all216 independent-primary
force values have maximum2.96954e-10 under3e-7. Relative tolerance is zero.
Three compile-time unit refusals and a positive acceleration/power unit check
pass. All actual, expected and cached reference extents/finite words are
checked before maxima, including each nested expected row.

`power-qualification.json` adds a genuinely new cached-force witness:
`v dot (tau-M*a-g) = .5*v transpose*Mdot*v`. Independent directional matrix
differences use a predeclared1e-5-second displacement parameter, not a simulator
clock or step. The24-case maximum error is6.79929e-10W under the predeclared
2e-6W bound. Only72 new matrix-metric calculations run; no native, SDK or earlier
coordinate-FD table is repeated.

## Focused commands and scope

```shell
COAX=/path/to/qualified-coax
COAX="$COAX" bash psionic/tests/robotics/panda_inverse_dynamics_check.sh /tmp/new-panda-inverse-proof
python3 psionic/tests/robotics/panda_inverse_dynamics_power_reference.py --native /tmp/new-panda-inverse-proof --output /tmp/new-panda-power-proof
```

The opt-in new SDK acquisition is separate; it is not needed by that ordinary
gate and should not repeat a passing capture merely to confirm it:

```shell
python3 psionic/tests/robotics/panda_inverse_dynamics_sdk_reference.py --output /tmp/new-explicitly-needed-SDK-capture
```

Original model/mesh payloads stay in the authenticated private cache. These
small derived numeric observations contain no weights or scene payloads.
The dataset card and MuJoCo declare Apache-2.0; LIBERO/robosuite are separately
pinned MIT sources. Attribution/provenance remains in
[the acquisition record](../libero-panda/acquisition.json). Python is tooling;
the product query requires Coax and the host compiler only. This slice supplies
no forward integration, passive/actuator/contact force, controller rollout,
physical safety, generic importer/frame system, differentiability, R15 or
compiler `#bounded` claim.
