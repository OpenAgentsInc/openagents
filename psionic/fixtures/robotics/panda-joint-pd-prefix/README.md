# A short owned Panda PD loop

This fixed experiment couples the already qualified seven-joint PD/gravity
query to the owned robot9 Euler map. Both controller and dynamics arithmetic
remain in Coax. It is a 250-step, 2 ms, gravity-loaded, no-contact example;
it does not qualify contact physics, general stability or hardware control.

`experiment.json` and the 48-double `initial.f64` were frozen before the
first numerical run. The arm target is the source home plus a small fixed
offset; target velocities are zero, default gains stay fixed, and the two
signed finger targets stay at their initial coordinates. External force is
zero. Source constraints and contacts are disabled with flags 17, while
implicit Euler joint damping remains enabled.

Exactly one new SDK acquisition produced `sdk-trace.bin`: one initial
explicit forward, 250 steps and no post-step forwards, neural inference or
rendering. `sdk-progress.jsonl` records all 502 before/after events. Each
step recomputes the Coax PD command on the current SDK state. The immutable
`sdk-reference.json` binds the input, authored sources, SDK headers/library,
scene and all 85 unique asset paths (86 XML declarations).

The independent matrix/potential controller oracle then checks all 8,750
P/D/gravity/requested/command values using the initial state and each prior
post-step state. P/D are exact; the other maximum error is 3.20e-14 J/rad
under the frozen 5e-10 absolute budget. Every clipping count and step time
matches. No SDK or Coax numerical execution is performed by that oracle.

The offline native comparison has two owned branches: fresh PD on the owned
state, and the owned plant driven by recorded SDK commands. Each of debug,
release and fail-fast ASan/UBSan passes 56,058 assertions. Both complete
250-record branches are byte-identical across those three native profiles.
All actual and expected floating arrays are finite before error maxima.

| Fresh owned branch versus SDK | Maximum absolute error | Frozen absolute budget |
| --- | ---: | ---: |
| Arm position | 8.33e-17 rad | 1e-8 rad |
| Finger position | 1.74e-18 m | 1e-9 m |
| Arm velocity | 4.83e-16 rad/s | 1e-7 rad/s |
| Finger velocity | 1.02e-17 m/s | 1e-8 m/s |
| All PD force fields / commands | 2.49e-14 J/rad | 1e-6 J/rad |

Relative tolerance is zero. PJP clipping counts match exactly (all zero).
Owned actuator-force clipping is checked against the separate scalar source
law; it is not an observed SDK counter. Status checks precede every committed
record but status fields are not serialized. `transport-profile-v1.json`
clarifies that distinction without changing frozen inputs or budgets.

The arm target-error norm changes from 0.0384057287 to 0.00993709027 rad.
That is an observation on this half-second prefix, without a convergence,
stability or steady-state acceptance threshold. The recorded-command shadow
isolates plant agreement from feedback agreement; it supplies no independent
full-episode success result.

The first strict generated-C build refused unused parameters in empty unit-tag
printers, before any SDK load. Its driver, failure and stderr remain in the
`preparation-*-v1` files. The followup compiled generated C separately with
only its unused-parameter warning suppressed, kept effects C strict, and
reused the already passing C/H and decoder checks. No codegen, passed guard,
SDK acquisition or native result was repeated.

The example transcript was written from the SDK endpoint and total PJP clip
count before its first execution. Debug and release match it exactly.
Historical joint-PD and Euler mutation results retain their own scopes;
this composition adds no product admission root. Incomplete Quantity-token
mutation provenance remains a limitation of those earlier source proofs.

The explicit offline gate runs only this component, with no SDK load:

```shell
COAX=coax/bin/coax psionic/tests/robotics/panda_joint_pd_prefix_check.sh /tmp/new-panda-pd-prefix
```

Native evidence is the named Darwin-arm64 scalar-libm profile. Full pose
control, gain/step-response analysis, collision constraints, runtime clock
integration, compiler R14 bounds and hardware remain separate work.
