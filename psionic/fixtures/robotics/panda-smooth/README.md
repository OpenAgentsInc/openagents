# Assemble the fixed Panda smooth acceleration

`psf_forward` combines source actuator force, explicit generalized external
force and the complete fixed-profile passive force, then uses the owned mass
and bias query. Hinge forces are J/rad, slides are N, and finger actuator
controls remain signed metre targets. Passive includes joint damping once.

```jai
applied = (actuator + external) + passive
mass * acceleration = applied - bias
```

All typed spans are checked before reads. The packed entrypoint may copy input
scalars before its delegated output-shape check; every shape still precedes
numerical admission. External force is admitted first, then q/v, actual
controls, combined applied force and bias-subtracted net force. The numerical
force cap is inclusive +/-10,000. Complete output publishes only after success;
external/net forces are not clipped. Actuator counters describe actual controls.

The frozen 20-case corpus contains 18 previously captured SDK targets and two
derived equal-force targets. At the same exact q/v, substitute the recorded
actuator-force difference between rows 8/9 as generalized external force. The
already captured `qacc_smooth` then supplies the target. No external-force SDK
query was executed. SDK operation order differs from this composition; target
agreement is numerical, not SDK byte identity.

Six new Coax groups cover 67 cases; 88/88 actual emitted root sites are killed,
with no equivalence waivers. Typed Quantity addition token provenance remains
an explicit compiler gap. Four static unit refusals and the independent SDK
fields supplement that stated scope. Debug/release/fail-fast ASan+UBSan each
pass 4,406 native assertions, 20 admitted cases, 180 nonfinite payload probes,
numerical boundaries/caps and four input/output overlaps. All actual/reference
fields are finite before comparison. Applied error is at most 1.78e-14 against
5e-10; acceleration at most 4.27e-14 against 5e-9, zero relative tolerance.
Owned status/output/counter streams agree byte for byte across the three modes.

`checks-qualification.json` preserves initial import and VM fixture failures,
exact source followup, new 88-site mutation result, four meaningful unit errors
and prewritten example transcript. The VM refuses creation of nonfinite F64
payloads; native boundary cases cover them. `qualification.json` records the
new native proof, source closure and exact corpus, with no SDK/NN/step or old
model gates. The example passes its independent golden in debug and release.

```shell
coax/bin/coax run psionic/examples/panda-smooth.coax
COAX=coax/bin/coax psionic/tests/robotics/panda_smooth_check.sh /tmp/new-smooth-proof
```

This is a Darwin-arm64 scalar-libm force/acceleration query. It is not the
constrained SDK acceleration, contact/frictionloss solve, implicit-damping
integrator, a robot episode, clock/real-time guarantee or hardware permission.
