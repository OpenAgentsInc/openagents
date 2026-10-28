# Bounded valid-program differential verification

`tools/differential.py` is an optional R0/R7a verification experiment. It does
not change compiler semantics, ordinary build dependencies or product code.
It uses only Python's standard library and an already-built Coax compiler
plus the host C compiler. The focused tests live in `tests/differential.py`.

## Subset and oracle

Generator `coax-small-state-v1` uses pinned SplitMix64 integer generation.
Each recipe contains at most 64 operations and four two-integer input vectors,
with input magnitudes at most 12. Every full six-operation cycle includes:

- checked signed `int` arithmetic (`+`, `-`, `*`, `/`, `%`, `&`, `|`, `^`),
  with nonzero divisors and a remainder reduction after arithmetic;
- a scalar branch and a returned struct containing an integer/fixed array;
- compound writes, writes reading another view element, and writes into the
  aggregate, with effectful index calls in source order;
- deferred caller-buffer writes, executed in reverse registration order.

The buffer is a four-element view inside a six-element caller-owned array.
Each input compares the return value, all four returned lanes, all six buffer
elements (including two guards), and the index-call count: twelve integers.
There are no loops, recursion, foreign calls or escaping local views in a
generated probe. Even before the first remainder reduction the scalar's
magnitude is at most 247; fewer than 64 additions of such bounded values keep
buffer and aggregate values far inside signed 64-bit range. Index results are
always in `0..<4`, and cleanup stays inside the borrowed view.

The independent model uses Python integers and direct state transitions. Its
signed division explicitly truncates toward zero; its remainder follows
`a - trunc(a / b) * b`, avoiding Python's different negative `%` rule. It
neither interprets emitted C nor asks the compiler to compute expected values.
Hand-calculated operator, index-order and reverse-cleanup vectors test this
model. A frozen recipe hash pins generation.

Each recipe is rendered once with evaluator `#test` assertions and a separate
native `main`. `coax test` receives a 1,000,000-step budget per check. `coax c`
without `--release` supplies checked arithmetic; the same emitted C is compiled
as pedantic C11 at `-O0` and `-O2`. The native main prints the twelve values;
the tool compares them with the model. These are generated eligible probes,
not a compiler feature that lowers arbitrary existing `#test` blocks to native
code. Checks remain outside the native main in ordinary C emission.

## Evidence and reduction

Run from the repository root, choosing an output directory that does not exist:

```sh
python3 coax/tests/differential.py
python3 coax/tools/differential.py --seed 20261028 --cases 24 --steps 24 --output /tmp/coax-differential-normal
python3 coax/tools/differential.py --seed 20261028 --cases 1 --steps 12 --seed-defect --output /tmp/coax-differential-seeded
```

The seeded command is expected to exit 1 with `counterexample`. `--coax` and
`--cc` select explicit compiler commands; `COAX_DIFFERENTIAL_COMPILER` selects
the compiler for the focused tests. Each process has a recorded timeout
(default 30 seconds). The run record and per-case records use schema version 1.
They retain the generator/tool/source/compiler hashes, built-in Basic dependency,
toolchain version/target/driver hash, semantic profile, inputs, commands,
exit statuses, output and generated/native artifact hashes. Existing output
directories are refused so a rerun cannot overwrite an earlier witness.

An evaluator assertion failure or a native value/nonzero-exit mismatch is a
counterexample. A process timeout or evaluator step exhaustion is inconclusive.
Tool startup, source rejection, emission and C compilation failures are
infrastructure errors; unexecuted native profiles are `not_checked`. The run
succeeds only if every selected execution and tool identity query succeeds.
This classification preserves failures for investigation; it does not infer
that every nonzero native process proves a compiler defect.

For a mismatch, the reducer removes operation chunks and input vectors, then
shrinks input scalars toward zero. Every candidate remains in the same bounded
grammar. A candidate is accepted only if it reproduces the original evaluator
assertion or the same native optimization/mismatch kind. Infrastructure failure
and timeout cannot satisfy that predicate. The default budget is 64 candidate
executions; the record reports attempts, exhaustion and retained size. The
final candidate is rerun and its preserved signature is recorded. Global
minimality, expression simplification and arbitrary source reduction are not
claimed. Original and reduced sources, recipes, emitted C and JSON observations
remain in the output directory.

Replay the retained seeded witness in a fresh output directory:

```sh
python3 coax/tools/differential.py --replay coax/tests/differential/seeded-multiply/program.json --seed-defect --output /tmp/coax-differential-replay
```

Omit `--seed-defect` to run its correct source normally. Replay rejects unknown
recipe versions, unsupported operations, zero divisors and out-of-bound inputs.

## Observed result and limits

The initial check-first suite failed before the harness existed. The completed
focused suite passes ten tests, including real evaluator/native execution,
seeded-defect detection and reduction, retained-fixture identities, deterministic
generation, oracle vectors, replay admission, and missing-tool/timeout outcomes.

On arm64 macOS with Apple clang 21.0.0, 24 generated 24-operation programs
passed: 96 evaluator input cases and 192 native input executions across
`-O0`/`-O2`. The trusted Coax 0.13.0 compiler SHA-256 was
`3746fef915ddd452bab14758bd8eead5230f2efe586bf669565bfc6cb7028edd`.
The machine-readable [sample result](tests/differential/sample-results.json)
retains the selection and artifact identities.

The deliberately seeded defect changes the emitted checked-multiply helper's
overflow builtin to checked addition. Both native profiles disagree while all
evaluator checks pass. Reduction takes four candidate executions, reducing
twelve generated operations/four input vectors to zero operations/one `(0, 0)`
vector. The probe's fixed initializer `a * 17 + b * 3` remains: expected total
0, observed 20. Its [source](tests/differential/seeded-multiply/source.coax),
[recipe](tests/differential/seeded-multiply/program.json) and
[counterexample record](tests/differential/seeded-multiply/counterexample.json)
are retained as a deliberate-defect regression. The production compiler and
emitted output are never modified on disk by this mode; the corruption is
written to a separate `emitted-seeded.c` verification artifact.

This sample does not establish general compiler correctness. It excludes
overflow/trap boundaries, casts/shifts, floating-point/literal rounding, `#run`
constant materialization, loop exits, nested cleanup, alias overlap, storage
lifetimes, foreign/host effects, ABI/layout oracles, Metal, Wasm and other
targets. No sanitizer, full bootstrap, book or app gate was run for this tooling
slice. The generator, model, renderer and comparison code are reviewable trust
inputs; shared mistakes can still agree. This is a small differential corpus
with an independent state oracle, not a general fuzzer or proof system.
