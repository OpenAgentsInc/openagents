# Bounded valid-program differential verification

`tools/differential.py` is an optional R0/R7 verification experiment. It does
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

## Checked-AST native-check C emission

`coax checks-c FILE [--only SUBSTR] [-o checks.c]` emits a separate,
verification-only C11 translation unit. It uses the original checked `#test`
procedures and resolved callees, rather than copying check source into new
procedures or replacing assertions. Ordinary `coax c` continues to omit checks.
The new command emits no `main` and does not execute a check or produce native
execution evidence. The legacy comparison and strict baseline companion below
retain their existing contracts.

Normal compilation and unfiltered authority validation happen before emission.
`--only` selects emitted checks by a byte substring of the original display
name; an empty substring selects all. It cannot filter authority validation.
No match, no checks, or a selected `#forall` is a refusal. Unrelated product
`main` and foreign declarations may remain in the file, but a selected check
cannot reach them. The Coax-owned policy in `lib/compiler/native-checks.coax`
selects roots, closes their resolved call graph and refuses unsupported facts;
the native bridge transports checked-tree and address observations.

This first slice admits zero-argument, void `#test` bodies using supported C
types, checked arithmetic, scalar/aggregate operations, caller-buffer writes,
compile-time values and ordinary cleanup. Reachable host effects, foreign
calls, product main, dynamic view-to-string conversions and unsupported
borrow/escape or type forms refuse with source-located `E0413`. Graphs are
bounded to 4,096 procedures/checks and 65,536 edges; check names and selection
queries are bounded to 4,096 bytes. These are verification-emission limits,
not new limits on ordinary Coax programs. Wrapping arithmetic and standalone
source-map output refuse; checked release build profiles and native `#line`
debug information are independent choices. Build-only stage0 explicitly
refuses this command.

The emitted native LP64 C interface is versioned:

```c
typedef struct Coax_VerificationCheck_v1 {
    int64_t declaration_index;
    const char *name; int64_t name_count;
    const char *source; int64_t source_count;
    int64_t line, column;
    void (*run)(void);
} Coax_VerificationCheck_v1;

const Coax_VerificationCheck_v1 *
coax_verification_checks_v1(int64_t *count);
```

Each row preserves its index in the original checked file's check inventory,
its physical source location, original name and actual body. Names are byte
spans: punctuation, duplicates and embedded NUL bytes remain distinct from
private generated C identifiers. String/table storage has static lifetime;
callers borrow it. A null count pointer is permitted. A passing body returns
normally; assertions and runtime traps retain the ordinary emitted C runtime's
behavior. An executing host must isolate faults and account for execution
itself. Neither obtaining this table nor compiling it establishes that its
checks pass. The focused independent C-table tests are in
`tests/native_check_emission.py`.

## Original-source bounded native execution

`tools/native_check_execution.py` is an optional maintenance driver over the
compiler-owned C interface above. It never reads or rewrites check bodies.
Coax owns validation and verdict rules in `lib/compiler/native-execution.coax`;
Python transports JSON/ABI facts and performs compiler, filesystem and process
effects. A fixed C observer links the same emitted source and generated Coax
policy C at checked `-O0` and `-O2`, using C11, `-ffp-contract=off` and
`-fno-fast-math`. This tooling is separate from product and ordinary compiler
build dependencies.

Run from the repository root with a new artifact directory:

```sh
python3 coax/tools/native_check_execution.py coax/tests/native-check-execution/identity.coax --keep /tmp/coax-native-execution --out /tmp/coax-native-execution.json --json
```

Each file first runs exactly one original `coax test --library` baseline,
without `--only`, under the normal 20,000,000-step limit or an explicitly lower
`--steps`. The Coax policy refuses larger step requests before execution. A
missing, malformed, incomplete or nonpassing original baseline cannot qualify
native execution. The compiler's complete `coax.evidence.v1` record remains
unchanged inside the separate `coax.native-checks.execution.v1` companion.
Its target still means the evaluator.

That same baseline invocation also captures its input profile. Coax binds its
Coax source set and imported protobuf descriptor hashes to the emitted artifact,
then checks the observed files again after execution. Missing descriptor lists
are distinct from observed empty lists. This adds input identity evidence without
changing the evaluator record's schema or meaning.

`--only SUBSTR` scopes native emission/execution and leaves that original
baseline unfiltered. Passing a selected scope does not qualify unselected
native declarations. The selection ledger records them, and authority
execution coverage remains evaluator-only. A selected `#forall` is an explicit
`not-checked` gap in this first execution slice. The original compiler authority
admission also applies when emitting C. No legacy source-lifting or alternate
authority policy is used as a fallback.

The baseline streams evidence on stderr and its input profile on stdout;
emission streams its profile on stderr and C on stdout. The host bounds both
outputs while reading them. Each selected check runs in a
fresh process group, with default limits of ten seconds and one MiB of native
output, within the command's 840-second and 64-MiB aggregate subprocess-capture limits. Phase
budgets, actual process statuses, pipe EOF/error observations and bounded raw
bytes are retained. Expected-fault children use the process-local quiet crash
launcher. Exhaustion kills the process group and is inconclusive.

The verification-only EP1 stream has a 16-byte header: `COAXEP1\0`, a little
endian u32 kind, and a little endian u32 total frame length. HEADER (kind 1)
adds a u32 table count. ROW (2), BEGIN (3) and END (4) add six u32 fields:
native ordinal, original declaration index, line, column, name-byte count and
source-byte count, followed by those exact spans. Spans admit NUL and UTF-8;
each is at most 4,096 bytes. Row frames are at most 8,232 bytes. A metadata stream
is HEADER plus its exact ordered ROW inventory. Execution is one matching
BEGIN followed by END. All encoders, decoders and identity checks are Coax.

A passed observation needs normal exit0, complete matching frames, both pipe
EOFs and no read error. A signal after a valid BEGIN is a raw counterexample;
it is not labelled a recognized Coax trap. A missing binary, malformed or
truncated successful stream, identity mismatch, or nonzero exit after complete
END is infrastructure failure. Time/output limits are inconclusive. The
existing `EV_*` aggregation keeps infrastructure, counterexample,
inconclusive and missing observations from becoming a passing qualification.

The companion records actual compiler/target/SDK queries and settings, source,
C, observer, policy and native executable hashes, exact selection and each
profile's observations. Original source/C/producer identities are checked
across phases. SDK path/version/settings observations do not claim a complete
SDK-content or toolchain lock. A policy library that cannot start produces an
explicit nonqualifying setup error. Report destinations refuse aliases of known
consumed source, descriptor, launcher, toolchain and policy artifact files; a
preexisting temporary output is preserved. The opt-in finite-domain and semantic
companions below extend this verification path; broader real-consumer qualification
remains separate. The independent vectors and host
failure fixtures live in `tests/native_check_execution.py`.

## Finite authored properties with verification ABI 2

Since 0.42.0, `checks-c --verification-abi 2` emits both ordinary checks and
original parameterized `#forall` procedures. Its immutable versioned table
includes binding names/types, canonical enum words and labels, finite domains
and exact case counts. The guarded `run_values` wrapper calls the original
checked procedure; no check body is lifted or cloned. ABI 1 remains the default.

```sh
python3 coax/tools/native_check_execution.py coax/tests/native-check-forall/order.coax --verification-abi 2 --keep /tmp/coax-forall-execution --out /tmp/coax-forall-execution.json --json
```

The `check-domains.coax` kernel is shared by the evaluator and native observer.
It owns bool, signed/unsigned 8- and 16-bit domains, sparse enums and half-open
`int` ranges; the nonempty product is bounded by 2²⁴. The last binding changes
fastest. Original bound evaluation retains its order and budget; rejected
products stop before later expressions run. Native profile arity is at most 64,
independently of the language's case limit.

The optional ABI 2 runner still captures one complete, unfiltered original
baseline and input profile before native emission. `--only` scopes native work
only. The same C artifact is linked at checked O0/O2. Each selected property
gets one fresh bounded process and a new 32769-byte shared progress file.
Coax encodes catalogue/progress bytes, plans each tuple, validates transitions
and formats bool, signed and enum witnesses. C maps the file, atomically
publishes alternating 16KiB slots, and calls the admitted procedure pointer.

A property publishes BEGIN before a tuple's body, END after return and DONE
only after every case. The parent validates the committed and preceding slots,
exact tuple, counts, process status and complete pipe EOF. DONE plus exit0 is
required for pass. A signal after a valid BEGIN retains a raw counterexample
with original source/binding names; it is not automatically a recognized Coax
trap. Incomplete or malformed successful output is infrastructure failure;
time/output exhaustion is inconclusive. The `coax.native-checks.execution.v2`
record keeps these semantics separate from execution.v1 and evaluator evidence.
An artifact-only failing-property observation remains explicitly unqualified
when its original baseline fails.

## Independent semantics and retained artifact defects

Since 0.43.0, the optional `native_semantics_execution.py` companion uses the
same strict original-source baseline gates and ABI 2 bodies. Independent literal
expectations cover signed division/remainder, casts, guarded caller-buffer
writes, exactly-once destination evaluation and reverse cleanup on ordinary,
return, break and continue exits.

Verification C alone exposes an optional runtime-trap callback. The fixed
semantics observer sends Coax-encoded ST1 frames with the known reason and
original physical path/line. Existing observers leave the callback unset;
ordinary product C has no hook. Recognition also requires matching begun-case
identity, expected abort, complete pipe EOF and valid progress. Arbitrary
signals, spoofed diagnostic text and malformed frames remain unrecognized.
The checked reason subset covers assertions, integer overflow, division by
zero, shifts, indices, slices and numeric casts.

A negative fixture keeps its failed original evaluator evidence. Exact JSON
diagnostic and native-trap parity is an explicitly artifact-only experiment,
with `baseline_qualified=false`; it never becomes execution.v1/v2 qualification.
The new `coax.native-checks.semantics.v1` record distinguishes experiment outcome
from the actual native counterexample and original baseline outcome.

The seeded-defect experiment retains both original and changed C. Coax admits
one multiplication-to-addition helper edit only in the verification copy. A
7×7 literal product table detects the seed at O0/O2. Bounded Coax reduction then
proposes tuples from the original domain for fresh-process replay against the
same seeded executable. Separate SP1 progress describes that single invocation;
it does not claim earlier property cases ran. Accepted reductions preserve
optimization, physical check identity, reason, fault location and seed identity.
The record labels arbitrary source minimization `not-supported` and makes no
global-minimum claim. Fixtures and regression controls live in
`tests/native_check_semantics.py`.

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
