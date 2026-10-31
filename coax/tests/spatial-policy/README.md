# Spatial admission and affine operator policy

[`spatial-policy.coax`](../../lib/compiler/spatial-policy.coax) is the pure
producer agreed for future M3 compiler hooks. It does not add frame syntax,
Point3/Vector3 source types, coordinate arithmetic, transforms or consumer
migrations. The current robotics chapters are 189–191; their source/framework
evidence is not rerun or promoted by this policy qualification.

`SP_Frame` requires handedness ±1, convention cartesian/body/optical, continuity
continuous/jumping and minimum representation 32/64. Convention/continuity enums
have unset 0, valid codes 1..3/1..2 and invalid 255; no omitted metadata defaults
to an admitted frame. Optical means X-right/Y-down/Z-forward and requires right
handedness. A body convention does not invent ROS axes for a MuJoCo link.
These are declarations, not evidence that a sensor or transform matches them.

`SP_Type { kind, frame, unit, rep }` uses raw 0/point 1/vector 2 (invalid 255),
positive nominal frame/unit IDs 1..2,147,483,647 for spatial values, and float
representation codes 32/64. These codes mean **F32/F64**, not arbitrary integer
bit widths. The future compiler maps them explicitly to its carrier IDs 6/7.
Raw values require both IDs 0; a dimensioned Quantity or a UnitOne quantity is
not automatically a raw float. Program-local IDs are not public wire IDs.

| API | Contract |
| --- | --- |
| `SP_frame_valid(frame)` | Exact closed metadata admission, including optical handedness |
| `SP_type_valid(type)` | Descriptor kind, representation and nominal ID shape |
| `SP_admit(type,frame,unit)` | Descriptor, metadata, point-length and minimum-precision admission |
| `SP_binary(op,left,right)` | Affine kind/result and exact representation/frame/unit compatibility |
| `SP_negate(type)` | Vector only; preserves the complete descriptor |

Admission checks descriptor shape first, then frame/physical-unit metadata,
then a point's pure-length dimension (M0, L1, T0, A0), then minimum precision.
Vectors accept **any admitted physical unit**, without becoming a qualified
axis-angle, angular-velocity or state-transform representation. Raw descriptors
ignore absent frame/unit metadata. Physical metadata uses the unchanged Q_Unit
bounds: four exponents −8..8 and numerator/denominator 1..2^20.

Binary/unary policy consumes **previously admitted** semantic types. It checks
descriptor shape itself, but does not look up registries or repeat frame/unit
metadata admission. The future checker must supply metadata from the actual
nominal declarations and retain that relationship. Matching numerical IDs with
invented metadata is not authenticated by this producer.

The binary table admits point−point→vector, point+vector/vector+point→point,
point−vector→point, vector±vector→vector, vector×raw/raw×vector→vector and
vector/raw→vector. Only vectors negate. Point+point, vector−point, point scaling,
raw/vector, equality/ordering, dot/cross and vector products are refused.
Descriptor errors precede operation refusal; a supported operation then checks
representation, frame and unit in that order. No implicit conversion is chosen.

Statuses are ok 0, type 1, metadata 2, unit 3, precision 4, frame 5, operation 6. Refused
SP_Result values contain an entirely zero, invalid descriptor; no partially
admitted result escapes. Successful operator results name the fixed future
point/vector kind; they do not invent an association among user-defined families.

Continuity metadata does not enforce a runtime realization epoch, freshness or
coverage. The producer reads no clocks and provides no moving/rotating-frame
velocity correction. Transforms, quaternion/axial-vector conventions, jump
handling, origin rebasing, R13/R14 and the full R12 gates remain separate.
The metadata descriptor ABI below is not the future three-scalar Point3 ABI.

## Qualification

Eight authored check families cover 1,404 cases; six unchanged quantity-policy
checks are loaded by the new unit. Its changed-root mutation gate kills
**158/158** mutants with zero waivers. Initial VM checks attempted casts to
undeclared enum values, which Coax correctly traps; explicit invalid 255 poison
members made the intended metadata refusal observable in authored checks.
The native oracle additionally covers every u8 nonmember value.

The independent Python oracle uses an explicit finite affine relation table
and an independently enumerated allowed-frame set. It never calls the producer
for answers. Its **98,064** cases cover all 256 operation codes, all 256
kind/convention/continuity codes, ID and representation bounds, metadata and
mismatch precedence, point units versus arbitrary vector units, and fixed
random inputs (seed `0x5350415449414c33`). It freezes the exact case count and
compares full transcript lengths and every logical output word.

Debug, release and fail-fast ASan/UBSan each match **588,384** expected words
exactly, including every refused descriptor and Boolean result, with empty
stderr and identical transcript hashes. C transports values and calls Coax;
Python is development tooling only. No prior UA/M1/native corpus was repeated.
The [receipt](qualification.json) binds sources, qualified private Coax 0.43,
generated artifacts and the exact outputs on the recorded Darwin arm64 host.
It does not qualify cross-architecture behavior or whole-compiler correctness.

```shell
/tmp/coax-quantity-bootstrap-043/coax test coax/lib/compiler/spatial-policy.coax
/tmp/coax-quantity-bootstrap-043/coax mutate coax/lib/compiler/spatial-policy.coax --root-only --jobs 8
COAX_TEST_COMPILER=/tmp/coax-quantity-bootstrap-043/coax python3 coax/tests/spatial_policy.py
```

The driver defaults to the repository's coax/bin/coax and accepts
COAX_TEST_COMPILER and COAX_SPATIAL_POLICY_RECORD_DIR. It does not install a
compiler. Reuse these passing results when their sources remain unchanged.
