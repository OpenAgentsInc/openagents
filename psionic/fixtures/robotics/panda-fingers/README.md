# Owned Panda finger kinematics

`pff_forward` extends the fixed arm query with four source-bound BODY poses:
left finger, left tip, right finger and right tip. Its separate `PffOutput`
contains the unchanged `PfkOutput` followed by those four poses. It takes seven
`Quantity(Radian,f64)` coordinates, two `Quantity(Metre,f64)` slide coordinates
and exactly one writable output. World positions retain `PfkWorld` Point3;
Hamilton-wxyz quaternions and row-major world-from-body matrices remain raw
attitude boundaries. This does not implement a general typed transform chain.

Admission checks all view shapes, then the arm coordinates in index order,
then the slides in index order. Each coordinate is checked for finiteness before
its range. The arm keeps its inclusive +/-100 radian numerical bound; each
slide admits inclusive +/-1 metre. Those bounds do not admit an actuator home,
enforce source joint limits or convert a normalized gripper command. Shape,
number or range refusal leaves every caller output word unchanged.

Both finger BODY attachments have source quaternion `(.707107,0,0,.707107)`.
After normalization that fixed +90-degree rotation precedes the local +Y slide.
In the gripper parent frame, slide `q` therefore moves the body along -X. The
right finger's separately rotated GEOM must not replace its BODY orientation.
The tip-to-tip distance within the source coordinate ranges is
`q_left - q_right + .017` metres. It describes geometry, not contact or commands.

Run the self-contained numerical gate with the committed reference data:

```shell
COAX=coax/bin/coax psionic/tests/robotics/check-panda-fingers.sh /tmp/panda-fingers-new
```

The output directory must be new. The gate uses Coax, the native C compiler
and Python stdlib test tooling; it does not need an installed SDK, weights or
a private cache. The opt-in SDK collector is documented separately in
[SDK-README.md](SDK-README.md); normal checks consume its frozen data.

[qualification.json](qualification.json) records 106/106 killed new-root
mutants, two wrong-unit source refusals and 153 native configurations with
1,753 ABI/admission/I/O assertions per mode. Debug, release and ASan/UBSan
produce byte-identical complete outputs under the explicit Darwin-arm64 scalar
libm profile. Separate `sin`/`cos` calls use `-fno-builtin-sin
-fno-builtin-cos -fno-fast-math -ffp-contract=off`. Default fused-libm byte
identity, cross-target determinism and inherited lowered spatial operator-token
mutation coverage are outside this proof.

The unchanged absolute pose budget is 2e-12, with zero relative tolerance.
Independent homogeneous matrices check 31,212 values (max error 3.22e-15);
literal parent-relative geometry checks 1,836 values (7.44e-15), and an
independent fixed-attitude relation checks another 5,508 (2.67e-15). Five
genuinely new SDK reset/forward queries check all sixteen bodies and the grip
site: 1,340 words, max error 1.78e-15. Compiled source local transforms match
all 112 normalized/position words exactly, and all nine named axes, anchors,
references and coordinate addresses are checked. No integration, rendering,
controller, neural model or old FK gate runs for that capture.

The source profile preserves original decimal coefficients. Reference query
coordinates are full F64 and same-generation; a cached body pose cannot be
paired with current post-step finger qpos without an explicit generation
contract. The pure API authenticates neither sensors nor clocks.

The same five SDK forwards also saved nine-joint mass/bias facts and compiled
inertial metadata for later work. [sdk-dynamics.json](sdk-dynamics.json) explicitly
does not qualify an owned dynamics implementation. The moving fingers/tips
carry .22 kg in this simulation file; their configuration affects even the arm
mass subblock. These simplified source parameters are not manufacturer physical
measurements. Import, dynamics, contacts, robot control and hardware remain
separate work.

The [runnable example](../../../examples/panda-fingers.coax) and its
[qualification](example-qualification.json) show metre inputs, independent
branch motion and atomic refusal. Its prewritten golden uses the exact .057 m
and .084 m distance relations; only display output rounds to micrometres.
