# Native witnesses for lowered source operator mutations

This is a new test-only unity-frontend probe. Coax chooses type-preserving
alternate procedures; the real `Mutator` changes one checked source call's
callee. C++ supplies no spatial arithmetic. The native C oracle uses fixed
independent scalar coordinates.

The fixture has twelve eligible source operators: point/vector addition and
subtraction, vector addition/subtraction, and vector/raw multiply/divide, in
F32 and F64. Six point-point difference, vector-plus-point and scalar-left
multiplication sites have no admitted alternate in this profile. Two vector
negations are explicitly unsupported. None of those eight are counted as
killed mutants or equivalent waivers.

For each eligible site the probe checks its source owner, exact token pair,
parameter/result identity and Coax-selected alternate. Apply must change one
source callee only. Every checked expression record, argument link, resolved
initializer and referenced type description remains unchanged. Shared helper
bodies remain untouched. Undo restores those records and the complete C/header
emission exactly. This does not snapshot every statement, procedure field or
compile-time heap object.

The two native observers write ten triples per carrier. Their fixed inputs are
`p=(10,20,30)`, `q=(1,2,3)`, `v=(8,16,32)`, `w=(1,2,4)` and scalar `2`.
The independent C oracle checks the baseline and all twelve applied variants.
Each variant changes exactly its three coordinates, leaving the other carrier
and every other operation unchanged. All thirteen emitted programs run with
native O0, O2 and fail-fast ASan/UBSan: 39 executions, 2,340 exact coordinates.
Each variant's transcript agrees across the three host profiles.

[The receipt](operator-native-qualification.json) records the first passing
run (48.954 seconds), exact source/frontend-object/header hashes, all 80
compile/emission/execution commands and the source inventory. No old numerical,
model, SDK or full mutation gate ran. The ordinary Coax mutant runner's checks,
scheduling, weak-check control and operand-order fixtures are separate evidence.

```shell
python3 coax/tests/spatial_operator_native.py \
  --frontend-build /absolute/private/frontend-build \
  --output /new/private/artifact-directory --jobs 4
```

The private build must supply its matching `proto_importer.o`, generated
header and `proto_runtime.inc`. This probe neither installs a compiler nor
qualifies a release/cache, all source mutations, unary mutations, quantities,
frame transforms or other targets.
