# Fixed seven-arm-joint PD and gravity

The typed `pjp_forward` query admits current q7, finger positions2, current
rates7, target q7, target rates7, Kp7 and Kd7, then publishes one `PjpOutput`.
Positions/targets are radians within the pinned seven source ranges; both
rate vectors are rad/s within ±20. Fingers are metre coordinates in the
existing ±1 numerical geometry box, not finger-control targets. Kp has units
J/rad² and positive box [1e-6,500]; Kd is J*s/rad² in [1e-6,100]. Defaults are
[60,60,60,60,40,20,10] and [8,8,8,8,5,3,2]. These are numerical profiles, not
a gain-stability recommendation.

```jai
P = Kp * (target_q - q);
D = Kd * (target_rate - rate);
requested = (P + D) + gravity_g0(q, finger_positions);
command = source_arm_clip(requested);
```

Owned g0 includes the moving fingers but no velocity-dependent bias. Joint
passive, fluid, actuation and inverse/forward dynamics remain their separate
stages. The command cap is source 5×±80/two×±12 J/rad. Use command as Pa's arm
input once; its same cap is idempotent. Do not add gravity again or sum the
two stages' clipping counters as unique clipping events.

Five force vectors (proportional,derivative,gravity,requested,command) and one
clipped count make the 288-byte output. Every shape precedes coefficient
admission in the API order; index-order finite checks precede ranges. Refusal
preserves all output and spare/input storage. Raw input is explicitly 44F64
coefficients in the same order. Trusted source-visible arithmetic helpers do
not seal arbitrary caller PODs.

The frozen 44-context corpus has 20 cases over five named valid home-arm SDK
zero-rate gravity tuples and 24 independent matrix cases. It does not substitute
older out-of-range zero-angle joint4 data. Eight new source groups / 101 meaningful
cases and 54+91 emitted root sites qualify only the new logic. Four unit errors
and literal/native arithmetic checks supplement incomplete Quantity-token
mutation provenance. Initial setup and missing-observation failures are kept.

`checks-qualification.json` binds the source checks; `unit-refusals.json`
retains four meaningful E0317 diagnostics. `native-qualification.json` records
the independently authored potential-gradient/scalar/native proof. The example
transcript was independently frozen before debug/release execution.

```shell
coax/bin/coax run psionic/examples/panda-joint-pd.coax
COAX=coax/bin/coax psionic/tests/robotics/panda_joint_pd_check.sh /tmp/new-joint-pd-proof
```

This is a Darwin arm64 scalar-libm pure force query. No SDK/NN execution,
integration, holding/tracking outcome, stability/collision guarantee, generic
frame migration, compiler bounded-control proof or hardware permission is
qualified by it. A behavior experiment has its own separately frozen contract.

## Qualified native result

All 44 contexts pass in debug, release and fail-fast ASan/UBSan, with 6,420
assertions per profile and complete 35-field/count byte equality between
owned profiles. P and D match exactly. Maxima against the independent reference
are 2.14e-14 J/rad gravity, 1.14e-13 requested and 1.43e-14 command, under the
unchanged 5e-10 absolute/zero-relative budget. Independent matrix potential
gradients match 35 cached SDK gravity values to 7.11e-15 J/rad.

Native 220 IEEE probes, 88 scalar boundaries, 86 adjacent-field precedence
cases, four overlapping typed/raw success/refusal cases and full snapshots
pass. The initial case-label zero-padding preflight failure was metadata only,
before numerical/reference/native work; it is retained separately. No gain,
budget, corpus or product arithmetic changed after comparison.
