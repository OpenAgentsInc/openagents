# Actual Panda spatial-token witness

This new test closes one specific application-level mutation gap: the authored
`origin + world` inside the unchanged `psionic/lib/robotics/panda-fk-math.coax`
procedure `pfk_shift`. The actual frontend prepares its same-type point-minus-
vector alternate; the actual `Mutator` selects that one `m.spatial` site.
Its source location is line51, column230 in the qualified source.

The new consumer's only `#test` checks independent zero-angle hand BODY and
grip SITE positions: `(-.662,0,1.8385)` and `(-.662,0,1.7415)` metres. Those
coordinates follow the fixed source offsets. No previous FK check executes.
The baseline passes; applying the real source-token mutant kills this check.
Thirty-six ordinary sites in that procedure are enumerated but never applied
or executed. The old 195/281-site mutation proofs and the separate 2,340-value
compiler token proof are not rerun or relabelled.

The test-only C++ unity seam invokes the compiler's own frontend, mutation,
evaluator and emitter. It contains no alternate spatial/FK arithmetic. During
apply, 2,179 expression payloads, 359 statement payloads, 17 procedure payloads
and resolved initializer payloads remain identical except for the one source
call's selected callee. Shared generated helpers stay unchanged. Public headers
are identical across the mutation, and undo restores the exact original C and
header bytes. These snapshots do not claim a complete compile-time heap or
every type-description snapshot.

Independent native C then checks the six endpoint coordinates against the
preselected 2e-12 absolute budget, with zero relative tolerance. It expects
their negatives for the mutant. All translated positions negate because the
root origin is zero and each recursive fixed translation changes from addition
to subtraction; attitude computation does not call `pfk_shift`. The native
transcripts separately confirm that relation for every pose, rather than
assuming it from the two endpoints.

Both variants pass native O0, O2 and fail-fast ASan/UBSan: 1,248 observed F64
words total, 117 position-negation comparisons with zero error, and 507
quaternion/matrix words unchanged byte for byte. Each variant is byte-identical
across these profiles with separate scalar-libm calls and
`-fno-fast-math -ffp-contract=off -fno-builtin-sin -fno-builtin-cos`. This is a
zero-coordinate application witness, not cross-target math qualification,
sensor/robot validity, a full compiler release gate or an SDK/model experiment.

[panda-shift-token-qualification.json](panda-shift-token-qualification.json)
binds the executed unity probe, private-compiler context anchor, graph
object/header/runtime, product source,
emitted C/header and native binaries. The later normal-reconstruction binding
is deliberately static: frozen frontend sources and graph C/header/runtime
inputs match exactly, so it carries the qualified source/emission result.
It does not claim identical probe/release binaries or a repeated execution on
the normal compiler.

To run this new witness in a fresh output directory after preparing a private
frontend build:

```shell
python3 coax/tests/panda_spatial_token_native.py \
  --frontend-build "$COAX_PRIVATE_FRONTEND_BUILD" \
  --output /tmp/panda-spatial-token-new
```

The script compiles its own disjoint unity probe by default. An explicit
`--prepared-unity` accepts an already compiled test probe and records its exact
identity; this is an honest local build receipt, not artifact authentication.
No downloaded repository code, SDK, renderer, physics step, neural model,
old mutation corpus or previous native gate runs.
