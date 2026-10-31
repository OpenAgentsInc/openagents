# Owned Panda forward kinematics

The fixed twelve-body/seven-hinge model now computes the hand and grip poses
in Coax. Seven typed radian inputs yield F64 world points plus explicitly raw
Hamilton-wxyz quaternions and row-major matrices. Shape, finite-number and
numerical-range refusals preserve every output word. This is a manually bound
geometric component, without dynamics, fingers, robot import or clock stamps.

[The source profile](../../psionic/fixtures/robotics/panda-fk/source-profile.json)
preserves the actual decimal coefficients and model identities. The hand BODY
orientation differs from the grip SITE orientation. Source soft joint limits
are separate from the FK numerical admission box; zero joint4 is a geometric
example outside that joint's source soft range.

[The qualification](../../psionic/fixtures/robotics/panda-fk/qualification.json)
records 11 source check groups and 281/281 killed candidate-emitted mutants.
The lowered spatial operator token itself is outside that mutation coverage.
Native debug/release/fail-fast sanitizer modes cover 158 configurations and
1,307 ABI/admission assertions per mode. At the frozen 2e-12 absolute budget:

- Independent stdlib homogeneous matrices check 24,648 values per mode.
- Seven genuinely new SDK forwards check 1,428 body/site values; largest error
  is 4.33e-15. There are zero integration steps, renders or model calls.
- Seven retained fresh samples check 84 grip endpoint values; largest error
  is 1.92e-15. Their complete 8,064 numerical reference bytes are bundled with
  an exact source-hash binding, so the gate needs no private cache.

The earlier default optimized sin/cos fusion passed the numerical budgets but
failed strict cross-mode bytes: 792 words differed by at most 1.56e-15. That
[failed record](../../psionic/fixtures/robotics/panda-fk/default-fused-diagnostics.json)
is preserved. The qualified Darwin-arm64 profile uses separate scalar-libm
calls; only changed release/sanitized artifacts were rerun. All 158 complete
208-F64 outputs then matched the retained debug bytes exactly. This does not
qualify R15 or cross-target libm.

The merged 0.45 compiler reproduces FK and example C/headers exactly; prior
executions carry without repeats. The new example has independently written
zero-angle geometry, two passing own checks and matching debug/release golden
output. Chapter 170 links its three exact source listings. The registry selects
only the three new roots and their self-contained native gate:

```shell
psionic/check.sh --only robotics/panda-fk
```

No old controller, full mutation, SDK episode or learned-policy gate ran.
The SDK still supplies physics for the genuine learned-policy evaluation.
