# Three first unconstrained SDK Euler steps

The frozen experiment binds six cases. The first three are independent
Gaussian targets from retained SDK mass/bias row 0 and actuator/passive
rows 12/13/14; they are algebraic references, not observed simulator steps.
The last three are NEW observations from the installed, pinned MuJoCo 3.3.1.

The new collector uses the unchanged authored actuator support/metadata
functions. It checks the source's original flags 0, Euler integrator and .002 s
period, zero wind, gravity, density, viscosity, stateless unit-gear actuation,
zero springs/gravcomp/plugins/tendons/flex/callbacks and named DOF mapping.
Only then it sets disableflags 17: constraints and contact are off, while the
Euler-damping disable bit is clear. The complete modified model metadata is
admitted before case evaluation and identical in the actual capture instance.

Each new row resets once, applies its frozen q/v/controls/generalized external
force, explicitly forwards once, records operands and steps once. No post-step
forward, rendering, neural request or episode runs. Both 9×12 mass crossblocks
between robot and off-scene DOFs must be exactly zero. Every SDK warning count
must be zero after forwarding and stepping. Model/SDK/headers/assets, collector
and binary hashes, exact input bits and the 12-event progress ledger are bound
to sdk-reference.json. The collector is fail-fast ASan/UBSan; the installed SDK
binary itself is not instrumented. Darwin leak detection is disabled explicitly.

Each 154-word row stores M81,bias9,passive9,actuator9,smooth-force9,
instantaneous-acceleration9,post-q9,post-v9,derived-effective-acceleration9,time.
The effective target is (new_v-old_v)/h, not SDK d->qacc. The independently solved
(M+hB) acceleration reproduces the new SDK q/v at ≤1.69e-21/6.94e-18 and the
derived effective acceleration at ≤2.85e-14, under the original fixed budgets.
Instantaneous/effective accelerations differ by up to 7.31 in these rows, so
advancing the old smooth acceleration directly is observably a different step.

Before the first SDK load, the initial tooling inventory mistakenly required
85 asset declarations. The exact scene has 86 declarations referring to 85 unique
paths (83 distinct content hashes). That preflight stopped before compilation
or SDK loading. Its record and the original frozen capture profile remain;
a source addendum fixes only that path/declaration guard before any acquisition.
Cases, h, force arithmetic, output shape and tolerances stayed fixed.

The SDK and algebraic references qualify a fixed robot9 model without
constraints. They do not qualify contact/frictionloss, full scene/object
integration, clock receipts, wall-time control, compiler bounds or hardware.
