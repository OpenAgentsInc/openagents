# Three-scalar point/vector native qualification

This is the new M3 native corpus. It runs independently of the earlier scalar
quantity, derived-unit and spatial-policy corpora. It requires a compiler that
implements `Point3(Frame, Unit, R)` and `Vector3(Frame, Unit, R)`.

```shell
COAX_TEST_COMPILER=/path/to/qualified/coax \
COAX_SPATIAL_RECORD_DIR=/tmp/spatial-native-new \
python3 coax/tests/spatial_native.py
```

`native.coax` begins with the two original positive checks. The native driver
runs that complete evaluator baseline before emitting or executing native C.
Construction uses an annotated zero value followed by raw `.x`, `.y`, `.z`
assignments. There is no contextual literal dependency. A scalar foreign tick
is called from Coax aggregate makers; no foreign procedure takes or returns a
point/vector.

The fixed seed `0x53334d333031` generates 1,088 seven-word rows for each carrier:
576 ordered pairs from 24 IEEE edge patterns, with deliberately different
coordinate patterns, plus 512 random rows. Each row holds two XYZ triples and
a scalar. The numeric oracle is separate volatile C coordinate arithmetic,
not the raw Coax fixture or another invocation of typed spatial arithmetic.
It checks all coordinates of point subtraction, both point/vector addition
orders, point minus vector, vector addition/subtraction, both scalar
multiplication orders, vector division and negation. Separate second-unit
vectors check that vector layout/arithmetic is not restricted to length units.

The profile is host IEEE binary32/binary64, round to nearest, no fast math or
contraction, with AArch64 FZ and x86 SSE FTZ/DAZ disabled. Non-NaN results must
match exact bits, including signed zeros, subnormals and infinities. NaNs are
compared by classification and canonicalized only in the observation stream;
NaN payloads and floating exception flags are outside scope. This is not a
cross-architecture determinism claim.

The C consumer derives private aggregate typedefs from exported prototypes.
It asserts real three-scalar sizes, alignments and XYZ offsets against raw C
records: 12 bytes/alignment 4 for F32 and 24 bytes/alignment 8 for F64. It also
checks aggregate function-pointer arguments/results, mixed nested records,
fixed arrays and typed views. There is no hidden runtime frame or unit payload.
The test compares named fields, not unspecified aggregate padding bytes.

Twenty whole-aggregate effect traces cover every supported source operand
order, including vector plus point and scalar times vector. Eight child
processes deliberately interrupt a scalar foreign tick and require the exact
prefix of effects, with no later operand observed. These are foreign-effect
order tests; they do not recognize typed Coax traps or reinterpret arbitrary
crashes as language fault evidence. A further case for each carrier starts
with a caller-owned point `(1,2,3)`: the right operand changes that storage to
`(101,202,303)` and returns vector `(2,3,4)`. The result must be `(3,5,7)`, proving
the left whole value was captured before the right mutation.

The three native modes are default Coax C at C O0, release Coax C at C O2, and
default Coax C under fail-fast ASan/UBSan. Their complete canonical observation
streams must agree. Every successful native mode must have empty stderr.

`native-raw.coax` exists only to measure code generation. The driver retains
homologous typed/raw C bodies after removing source-line directives and reports
actual differences; it does not normalize generated type names or assume
aggregate lowering is free. On a Mach-O64 host it reads exact object symbol
extents and relocations at O0/O2. Byte equality is called instruction-extent
equality only when both extents are relocation free. Symbol extents can include
alignment padding. No measured wall-time speedup is implied.

The receipt binds the compiler, exact commands, source files, input cases,
generated C/headers/objects, mode outputs and comparisons. It makes no claim
about transforms, temporal frame realizations, jump epochs, angular roles,
foreign aggregate admission, GPU/device ABI, Panda/model execution or full R12.
