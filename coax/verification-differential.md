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

## Legacy authored-check comparison

`python3 coax/tools/native_checks.py --selection coax/tests/native-checks.json`
runs real authored `#test` checks three ways: the evaluator (`coax test`), and
the same bodies emitted with `coax c --arithmetic checked` and compiled by the
host C compiler at `-O0` and at `-O2` (`-std=c11 -ffp-contract=off -lm`). Each
eligible check's outcome is "passed", "assertion N failed" (counting its
`#assert`s from one, matching the evaluator failure to the native assertion
number) or "trap" (checked arithmetic, bounds, any signal). All three must
agree; a disagreement is a counterexample, minimised to that test alone, cut
after its first disagreeing assertion, and written to `--keep DIR`.
`coax/tests/native_checks.py` exercises this host runner and its seeded defects.

The published `coax.native-checks.v1` record measures agreement for the eligible
subset. A file or run can have `outcome: "passed"` when all compared checks
agree on an assertion failure (`agreed-assert`) or a trap (`agreed-trap`). It
can also pass while some source checks are rejected from that subset. Agreement
is useful compiler evidence; it does not mean the original file's checks pass
or that every authored claim ran natively. The strict companion below keeps
these meanings intact and adds separate observations.

The native harness is a temporary source beside the checked file (so `#load`
resolves): the file unchanged, each eligible body as `__native_check_K ::
() -> int` with `#assert E;` as `if !(E) { return N; }`, and a `main` printing
one line per check. A trap ends the process; the harness is rebuilt without the
checks already run. The harness is deleted after emission and never enters
product C: the tests check that `coax c` of the product has no
`__native_check` symbol. Returned values, caller-buffer and guard writes, and
`defer` cleanup are compared through the checks' own assertions, each assertion
separately.

The runner distinguishes execution from transport failure. An unavailable
native binary is an infrastructure error, not an agreed trap. A nonzero exit
after all result rows is also an infrastructure error; it cannot be assigned
to an unreported check. C compiler timeouts are inconclusive. For an actual
executing check, legacy trap agreement still groups signals without requiring
the evaluator and native trap reasons to match. Expected-fault native children
use the quiet crash launcher so these probes do not open macOS crash dialogs.

A `#test` is eligible when its body uses no directive but statement `#assert`
(`#run`, `#expect` and asserts inside expressions are "rejected"). Standalone
`#forall` blocks are outside this runner's lifted `#test` subset.
Files with their own `main` are rejected rather than replaced. A file the
evaluator or emitter refuses, a missing compiler and a timeout make its checks
"not-checked", "infrastructure-error" or "inconclusive": none is ever
"passed", and the run exits nonzero. The record names the compiler SHA-256, C
compiler, target and per-profile C and binary SHA-256s, every lifted `#test` in
each selected file with its outcome, and (with `--selection`) the lifted
`#test`s under `verse/` and `psionic/` the selection doesn't run.

`--seed emitter` swaps the emitted checked multiply for a checked add;
`--seed evaluator` reports the evaluator's first passing check as failing at
its first assertion. Both produce counterexamples (the tests require it).

On arm64 macOS with Apple clang 21, the selection of 18 files (10 Verse, 8
Psionic) ran 62 checks, all agreeing at `-O0` and `-O2`. This is the recorded
selection result, not a qualification of every later source or compiler.

## Strict original-source baseline companion

`tools/native_check_baseline.py` adds the versioned
`coax.native-checks.baseline.v1` companion. It is optional host maintenance
tooling around the legacy source lifting and comparison above. It does not
change `coax.native-checks.v1` or the evaluator-only `coax.evidence.v1` contract.

Run from the repository root:

```sh
python3 coax/tools/native_check_baseline.py verse/look.coax --json --out /tmp/coax-native-baseline.json --keep /tmp/coax-native-artifacts
python3 coax/tools/native_check_baseline.py --selection coax/tests/native-checks.json --json --out /tmp/coax-native-selection.json
```

The second command requests strict accounting for the named selection. A
coverage gap or a nonpassing original baseline produces a nonzero exit, even
when the nested legacy differential result reports agreement. It is not a
command that promises every selected file is currently supported. Selection
scopes the gate; a pass does not cover unselected repository files. `--seed
emitter|evaluator` retains the deliberate-defect modes. `--steps N` sets an
explicit lower baseline check budget; without it the compiler's normal
20,000,000 steps per check apply. Zero, negative or larger values are refused.
The companion never raises that budget to the legacy runner's 400,000,000
steps. Subprocesses share an 840-second command budget and retain their shorter
individual limits.

For each file, the companion invokes the compiler on the **original source**
with `test`, `--library` and `--evidence`, without `--only` or an added `main`.
It retains that complete compiler-authored evidence record and the process
observation. A valid passing baseline supplies the same observation to the
legacy comparison; an invalid or nonpassing baseline stops before native
comparison. There is one original-source evaluator invocation, rather than a
second evaluator run on a rewritten source. Normal authority admission during
C emission still applies to the temporary native harness; it is never bypassed.

Read the companion's observations separately:

| Field | Meaning |
| --- | --- |
| `files[].baseline` | The original `coax.evidence.v1` record, or `null` if none was obtained. Its claims, source hashes, compiler identity, selection, budgets and evaluator target retain their original meanings. |
| `files[].baseline_execution` | The original evaluator process observation, including `argv`, `cwd`, exit status, evidence path and its hash when available. A process exit alone does not replace valid complete evidence. |
| `differential` | The unchanged `coax.native-checks.v1` comparison, including eligible outcomes and native C/binary identities at `-O0` and `-O2`. Matching assertion failures or traps remain agreement, not successful authored checks. |
| `files[].native_coverage` | Each original authored `#test`/`#forall` claim and its native support status. Quantified, imported, indented, duplicate-name and otherwise unsupported bodies remain explicit gaps. `#authority` remains evaluator-only in `baseline`, not a native body. |
| `files[].validated_sources` | Original source paths and hashes checked against the complete baseline. Sources and compiler are checked again after native comparison; these observations do not freeze the filesystem. |
| `files[].outcome` / `outcome` | The strict file/run verdict. A pass requires a valid complete passing original baseline, legacy agreement, and unambiguous native coverage for every original authored check body. |

A failed check, an exhausted evaluator budget, missing or invalid evidence,
missing claim results, unsupported native coverage, tool failure, or native
disagreement cannot become a strict pass. The legacy runner can report
`agreed-assert` or `agreed-trap`; the companion stops at the failed original
baseline before requesting those comparisons. A passing evaluator `#forall`
cannot supply its missing native coverage. Authority execution coverage remains
an evaluator claim, preserved without inventing a native check body. An
unchanged-looking check name does not establish original source,
imported-source or compiler identity.
A baseline `counterexample` is a failed authored check; the nested records
distinguish that from evaluator/native disagreement. Neither label alone proves
a compiler defect.

Focused regressions live in [tests/native_check_baseline.py](tests/native_check_baseline.py):
original-source and budget identity, incomplete or invalid evidence, unsupported
physical claims, authority refusals and CLI output protection. The separate
[legacy tests](tests/native_checks.py) retain agreement, seeded-defect and
transport-failure coverage.

This companion still rewrites a bounded subset of source text into a temporary
harness. It does not emit checks from the compiler's resolved AST, enumerate
native finite-property domains, establish complete native authority execution
coverage, or implement a bounded native output/evidence protocol. Those
`r0-native-check-exe` children remain open. Its strict verdict is limited to the
recorded sources, compiler, host C profiles and observations; it grants no
broader language, ABI, Wasm, Metal, WGSL, device or mutation qualification.
Evaluator authority coverage remains an evaluator claim and does not authorize
a native product build or replace zero-survivor authority mutation.
