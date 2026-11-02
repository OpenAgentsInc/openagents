# Spatial types: the initial affine R12 profile

The authored compiler accepts nominal three-dimensional points and vectors in
explicit frames. It checks their affine operations and minimum precision,
then lowers them to ordinary three-scalar aggregates. This native C profile
is a foundation for [R12](ROADMAP.md). The source-bound Panda queries now use
typed world positions; full frame/state transforms remain pending.

```jai
#load "lib/quantity.coax" as u;

World :: struct @frame(handedness=right, convention=cartesian,
    continuity=continuous, min_precision=f64) {}

displacement_x :: () -> f64 {
    origin: Point3(World, u.Metre, f64);
    origin.x = 149597870700.0;
    target: Point3(World, u.Metre, f64);
    target.x = 149597870760.0;
    delta := target - origin;
    moved := delta + origin;
    return moved.x - origin.x;
}
```

All four frame attributes are required literal vocabulary. Handedness is
`right` or `left`, convention `cartesian`, `body` or `optical`, continuity
`continuous` or `jumping`, and minimum precision `f32` or `f64`. Optical means
X-right/Y-down/Z-forward and requires right handedness. A frame tag is empty,
nongeneric and distinct from unit or quantity roles. Equal metadata never
makes independently declared frames identical.

`Point3(Frame, Unit, R)` and `Vector3(Frame, Unit, R)` initially require concrete
frame and unit arguments and `f32`/`f64`. Points require physical length;
vectors admit any existing unit dimension. M2's nominal and canonical derived
unit identities remain intact. A frame with minimum `f64` refuses `f32`
coordinates even when a particular value happens to be small.

The construction surface is an annotated zero variable followed by component
assignments. Bare/inferred intrinsic struct literals and symbolic constructor
arguments remain outside this profile. `.x`, `.y` and `.z` expose raw scalar
coordinates; constructing these values asserts their units/frame at that raw
boundary. It does not validate sensor provenance, SDK conventions or a physical
frame realization.

| Operation | Result with matching frame, unit and carrier |
| --- | --- |
| point − point | vector |
| point + vector; vector + point; point − vector | point |
| vector + vector; vector − vector | vector |
| −vector | vector |
| vector × raw scalar; raw scalar × vector; vector / raw scalar | vector |

Raw scales are logical `f32`/`f64` values of the vector's carrier; ordinary
untyped literals may coerce to it. A dimensionless Quantity remains a quantity,
so it needs an explicit `.value` escape before raw scaling. Point addition,
point scaling/negation, vector products, equality, ordering, dot and cross are
refused. Neither kind is numeric, ordered or equatable; both remain copyable.
There is no implicit frame, unit or precision conversion.

The compiler owns these types and operations. It synthesizes a private ordinary
procedure for each admitted operator/type tuple, whose by-value parameters
freeze each whole operand once before its components are read. Existing ordered
call lowering preserves source order, including scalar-left multiplication and
vector-plus-point. These are compiler implementation procedures, with generated
expression provenance. The lowered source call retains an authored marker so
mutation still visits cloned operand expressions, including array indices.
The initial source-token mutation profile now admits `+ ↔ -` for point/vector
and vector/vector pairs, and `* ↔ /` for vector/raw-scalar pairs, when Coax proves
exact result identity. A mutant redirects one source call to a Coax-selected
alternate helper; arguments and shared helper bodies stay unchanged. A point
difference, vector + point and raw × vector have no admitted paired alternate;
unary negation remains unsupported by this binary profile. Separate counts do
not count those gaps as check kills. This does not introduce user-defined
operator overloads or complete all authored-spatial mutation coverage.

The [new token witness](tests/spatial/operator-native.md) qualifies twelve exact
F32/F64 source mutants, one-callee-only apply/undo and independent native outputs.
The focused [operator record](../docs/verification/coax-spatial-operators-2026-10-31.json)
separates that evidence from the earlier 1,403-site checker scope. Preparing
alternates is mutation-only and can exhaust the existing shared instance budget;
that is a preparation refusal, not successful partial coverage. The earlier
FK mutation score is retained with its original emitted-site scope.

The [actual Panda witness](tests/spatial/panda-shift-token.md) separately kills
the one newly admitted source token in unchanged `pfk_shift`, using a new
endpoint check and native all-pose observations. The old FK mutants are not
rerun. [Publication bindings](../docs/verification/coax-spatial-operators-publication-2026-10-31.json)
record the focused normal seed reconstruction, exact default emissions and
static source/graph carry of the native token witnesses. This is no full
release, installed-cache or other-target qualification.

The type key includes point/vector kind, nominal frame, resolved nominal/formal
unit and carrier. Generated nodes and types use a private scope; internal C
names are not a source or wire API. Spatial types and operator procedures share
the existing 256-instance budget with generic and generated-unit instances.
They use bounded caller-owned compiler pools. Refusal may change private scratch
but does not publish a partial type/helper cache entry or a successful result.

Values contain only `x,y,z`: 12 bytes/alignment4 for `f32`, 24 bytes/alignment8
for `f64` on the qualified LP64 target. They use an ordinary aggregate ABI and
never Quantity's transparent scalar-carrier flag. Debug output can retain
private helper calls. The new independent native proof compares 84,978
observations per mode in debug, release and fail-fast ASan/UBSan, with twenty
whole-aggregate order cases, eight deliberate scalar-host interruptions and
alias snapshots. Release object bytes match all thirty examined raw functions;
twenty-eight are also relocation-free. The two alias cases retain relocations,
and debug has helper overhead, so this is not a universal zero-cost claim.
Whole generated C files and typed/raw function text differ.

`size_of`, physical layout and `type_name` remain available. Schema1 semantic
reflection is refused with E0835; Metal, CUDA, SPIR-V and WGSL spatial-value use is refused
with E0836. Frame/type admission uses E0832/E0833, and affine/mismatch refusals
E0834. No device ABI or physical metadata reflection is qualified here.

The narrow real consumer is [Spacecam](../aethyr/spacecam.coax). Its world point
and floating origin become internal minimum-f64 points, and subtraction yields
a vector before explicit raw D3 coordinates return to the existing rendering
path. The published D3/SpaceCam signatures and layouts stay unchanged. Two
affected camera checks and a new independent 1,536-coordinate F64 oracle pass
in debug, release and sanitizer builds. This does not type the whole camera
transform, record an origin change, or migrate Panda joints and poses.

Frame metadata does not establish a correction epoch, coverage, inertial frame,
pose, angular interpretation or continuous motion of an object. A jumping frame
needs a future realization/clock contract. Rotating-frame state transforms need
position and velocity together, origin motion and a rotation derivative; this
profile offers none of those transforms. Kelvin is also absent from the current
four-axis unit metadata, so temperature dimensions remain a separate extension.

The [focused checker record](../docs/verification/coax-spatial-2026-10-31.json)
records 731 retained check groups and 1,403/1,403 killed mutants across twenty
changed procedures, with no equivalence waivers. It separates the frozen 0.44
source proof from the later merged-source reconstruction. The
[native record](tests/spatial/native-qualification.json) retains the initial
numerical/ABI evidence and exact production-emission binding. The checked-in
parser/checker seeds include this profile. The existing private source builder
reconstructs them without installing a release cache:

```shell
python3 coax/tests/quantity_bootstrap.py /tmp/coax-spatial-private
COAX_TEST_COMPILER=/tmp/coax-spatial-private/coax python3 coax/tests/spatial.py
python3 coax/tests/spatial_scope.py /tmp/coax-spatial-scope
/tmp/coax-spatial-private/coax mutate /tmp/coax-spatial-scope/root.coax --root-only --jobs 8
```
