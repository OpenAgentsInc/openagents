# Fixed Panda forward kinematics

This is Coax-owned forward kinematics for one source-bound twelve-body Panda
chain with seven local-Z hinges, through the hand and grip site. It evaluates
all twelve body poses and the grip pose from seven radian coordinates. It does
not integrate physics. `source-profile.json` names every parent, joint, offset
and original decimal quaternion, bound to the authenticated demo0 XML. The
library constructs this immutable table itself; callers cannot substitute a
model. `SDK-README.md` describes the new independent forward-only SDK recording.

`pfk_forward` takes seven `Quantity(Radian,f64)` values and one writable
`PfkOutput`. `pfk_forward_raw` explicitly attaches those radian labels to seven
raw F64 values. Both return `PfkStatus`: zero is success. Wrong shapes,
nonfinite inputs and absolute coordinates above 100 rad refuse before publishing
any output lane. This conservative FK admission range does not clamp or enforce
source soft joint limits. Native callers supply valid backing storage and
truthful view extents.

`PfkPose.position` is `Point3(PfkWorld,Metre,f64)`. Quaternion arrays are raw
Hamilton wxyz; matrices are raw row-major world-from-body orientations acting
on column vectors. Their multiplication is an explicit untyped attitude
boundary. `links[9]` is the hand body used by the existing observable;
`grip` is the identity-local site in `links[11]`, used by the controller.
Those orientations differ. Source quaternion signs and decimal attachment
coefficients are retained. The type labels describe the declared model and
caller input; they do not authenticate a physical sensor or frame realization.

The recorded numerical budget is 2e-12 absolute, frozen before comparisons.
`qualification.json` records 11 passing Coax check groups (34 cases), 281/281
killed new-root mutants, 158 native cases and 1,307 ABI/atomic-admission assertions
per mode. Each mode qualifies 24,648 independent homogeneous-matrix values,
1,428 new SDK position/quaternion/matrix values, 84 retained fresh grip endpoint
values, and the byte-exact immutable source table. Source constants normalized
by independent tooling also match the compiled SDK model. Different nominal
worlds and metre-valued joint inputs are compile-time refusals.

The 281-mutant result covers the candidate compiler's emitted mutation sites
in the three new FK source files. It does not cover swapping the lowered
spatial `+` token in `pfk_shift`; that compiler mutation feature remains pending.
The spatial operation itself is exercised by the analytic and independent FK
checks. Preserving authored operand traversal is a separate compiler proof.
The current operands are the aggregate identifiers `origin` and `world`, so
exposing those operands adds no integer, boolean or binary mutation site here.

The qualified host profile is Darwin-arm64 F64 with separate scalar libm
`sin`/`cos`, using `-fno-builtin-sin -fno-builtin-cos -fno-fast-math
-ffp-contract=off`. All 158 complete 208-F64-word outputs match byte for byte
between the qualified debug, release and ASan/UBSan artifacts. The already
passing debug numerical proof is reused; only the changed release/sanitized
scalar-call artifacts were executed again.

`default-fused-diagnostics.json` preserves the earlier failed strict byte check
unchanged. Default Clang O1/O2 fused the two intrinsics into `___sincos_stret`;
792 words differed from debug by at most 1.56e-15, while every frozen numerical
budget passed. That default profile has no cross-mode byte-parity claim. R15,
cross-target determinism and universal libm behavior remain outside this proof.

The retained controller samples are fresh reset+forward poses. Dataset action
rows and cached post-step observations are not interchangeable same-state FK
targets. This pure FK API receives coordinates only and supplies no clock or
freshness stamp. No old SDK episode, learned model or controller gate was rerun.
Fingers, Jacobians, importers, dynamics, full R12 transforms, compiler bounded
control and replacement of the complete simulation backend remain later work.

The seven existing controller samples are bundled under
[retained-controller](retained-controller/binding.json). Its data-only binding
maps each original published hash to the same 1,152 bytes and original filename:
8,064 bytes in total. This supplies the native reference input without a
private-cache dependency. The copy adds no numerical or SDK execution result.

To reproduce the native check with a qualified M3 compiler, use a fresh output
path and the bundled samples:

```shell
COAX=/path/to/qualified-m3-coax
COAX="$COAX" psionic/tests/robotics/check-panda-fk.sh psionic/fixtures/robotics/panda-fk/retained-controller /tmp/new-panda-fk-proof
```

The original authenticated cache directory remains an optional input:

```shell
COAX="$COAX" psionic/tests/robotics/check-panda-fk.sh /path/to/authenticated/controller-fixtures /tmp/another-new-panda-fk-proof
```

The portable source checks and focused mutation roots are separate:

```shell
"$COAX" test psionic/lib/robotics/panda-fk-chain.coax
"$COAX" mutate psionic/lib/robotics/panda-fk-profile.coax --root-only
"$COAX" mutate psionic/lib/robotics/panda-fk-math.coax --root-only
"$COAX" mutate psionic/lib/robotics/panda-fk-chain.coax --root-only
```

No Python package, SDK, renderer or neural runtime is needed by the product
library or that reference comparison. Python is stdlib development tooling.
The opt-in SDK collector is separate and its frozen records are reused. The receipt preserves the original candidate execution identities and binds
the final reconstructed Coax 0.45 compiler through byte-identical C/header
emission, with no passing numerical gate repeated.
