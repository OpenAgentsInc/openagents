# Owned grip-site geometric Jacobian

The new Coax wrapper evaluates its own fixed-model FK and publishes a 6×7
world-basis grip-site Jacobian. Linear columns carry metre/radian units;
angular columns carry the radian/radian ratio UnitOne. Refusal preserves
caller output; singular geometry is accepted without inversion or control
admission. The projection helper establishes no caller-POD provenance.

[The frozen experiment](../../psionic/fixtures/robotics/panda-jacobian/experiment.json)
preselects 62 configurations, 2e-12 retained-reference tolerance and primary
central-difference step 1e-5 with 3e-8 absolute tolerance. Independent matrix FK
and world-left SO3 differences supply the derivative oracle. This does not
differentiate rounding, refusal or soft-limit dynamics.

[The qualification](../../psionic/fixtures/robotics/panda-jacobian/qualification.json)
retains these results:

- Both new source roots pass; 33 math and 14 wrapper mutants are killed, with
  zero survivors. The initial dense-rotation coverage gap is preserved.
- Each debug/release/fail-fast sanitizer mode checks 62×42 values and 770
  ABI/admission assertions. All 2,604 output words agree byte for byte under
  the explicit scalar-libm host profile.
- All 294 retained SDK entries pass; maximum error is 1.110223e-15. The same
  seven exact bundled PcSample files provide them without a private cache.
- Primary finite differences give linear/angular maxima 3.264755e-11 and
  1.569322e-11. The whole step sweep remains diagnostic: the coarser 1e-3
  result exceeds the primary budget and was not selected in its place.
- Five new nonfinite-reference guard controls pass, and all 10,206 cached
  expected values are finite. Derivatives and native outputs were not rerun
  for that tooling hardening or the byte-identical default-reference alias.
- Merged Coax 0.45 emits the exact previously executed C/headers; prior
  numerical proofs carry. No SDK capture or old controller/native gate ran.

The new chapter 170 example uses actual quantity products to cancel joint
radians explicitly. Its independently written zero-angle geometry gives
0.044m/s grip Y and 0.5rad/s angular Z for joint1 at 0.5rad/s. Two own checks
and both transcripts pass on the merged compiler. Joint4 at zero remains a
geometric fixture outside its source soft-limit range, not a command.

```shell
psionic/check.sh --only robotics/panda-jacobian
```

The registry names only the two new roots and their self-contained native
gate. Its unexported math root uses library mode. The physical solver,
controller integration, finger geometry, R14 bounds and full frame transforms
remain separate work.
