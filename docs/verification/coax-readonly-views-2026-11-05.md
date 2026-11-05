# Coax read-only views verification — 2026-11-05

Issue: [#12924](https://github.com/OpenAgentsInc/openagents/issues/12924).
Implementation: `coax/readonly-views.md`. Compiler release: 0.49.0.

## Scope

The implemented contract covers evaluator/C read-only descriptors, one-way
restriction of mutable views, immutable-array borrowing in expected contexts,
returned carriers, generic readers, conditional capability unification and
lifetime-preserving provenance. Generated native descriptors have const element
pointers and distinct names; existing writable descriptors keep their identity.

Concrete element containment is checked after layouts and instantiation.
Embedded writable or recursively unresolved descriptors refuse (E0950), rather
than yielding write access through a copied element. Device profiles explicitly
refuse read-only types (E0951). These are documented admissions, not GPU,
exclusive-ownership, deep immutability or Aethyr scheduling qualifications.

## Observations

- The trusted 0.48 compiler in cache fingerprint prefix `dbc5292e433c` compiled
  the authored parser/checker and refreshed their explicit seeds. Development
  compilers then exercised actual source programs and C artifacts. Normal
  bootstrap separately checks seed reproduction and the final compiler graph.
- The 16 new source-level checker checks cover capability conversions,
  immutable array admission, casts/truncation, target refusal, conditional
  unification, generic calls/mangling, final concrete containment, memo reuse,
  bounded failures, tracked-call relocation and reflection. Dynamic read-only
  descriptors report zero `element_count`, just like writable descriptors;
  the internal capability flag is not an array extent. Existing checks stay enabled.
- Parser mutation: **1,791/1,791 killed**, zero survivors. New read-only policy:
  **87/87 killed**, zero survivors. Affected type, generic, expression and
  statement unit gates kill **1,924**, **813**, **1,117** and **1,419** mutants,
  respectively, with zero survivors. Earlier failed mutation attempts supplied
  regression cases; no mutants were marked equivalent.
- The complete checker mutation gate kills **12,682/12,682**
  mutants with zero survivors, including the call-index null-sentinel regression.
  All **1,346** checker checks remain enabled. This supersedes the earlier
  borrow-unit attempt that exposed that missing check.
- `coax/tests/pass/readonly_views.coax` runs the same `exercise` routine through
  its authored check and its native main in debug/release. It covers static and
  parameter borrows, alias observation, returned carriers, inline aggregate
  copies, generic readers, branch unification and single evaluation of a
  side-effecting conversion operand. All ten `readonly_*.coax` refusal fixtures
  reject with their expected diagnostics.
- The native suite's new ABI/ASan/UBSan test checks exact pointer/count aliasing,
  static backing across calls, read-only byte/string borrowing and const header
  fields in checked/release modes. Existing native tests also pass. The target
  refusal test checks cuda, metal-compute, WGSL, SPIR-V and scalar Metal.
- After rebasing the concurrent compiler-timing change, the final binary passes
  all ten native integration tests and all ten checker integration tests,
  including generated-header layout and native sanitizer probes. The positive
  read-only fixture also passes its authored check and native debug/release
  execution, including the reflection assertions.
- Final compatibility checks pass: 42 native pass-fixture runs, 72 refusal/trap
  fixtures, ten reflection tests, seven debugging tests (four other debugging
  tests are skipped by their platform conditions), and both new chapter-6
  example units. The required `coax/web/tool.sh tools-check` reports zero new
  tool files without a reason. Book structure was checked on the unchanged
  book content before the final reflection fix.
- `coax/tests/readonly/native.coax` passes original-source native-check execution
  at O0/O2 with an evaluator baseline. Command:
  `python3 coax/tools/native_check_execution.py coax/tests/readonly/native.coax --compiler PATH --out REPORT`.
  The larger view-returning helper example remains outside the existing
  `checks-c` ABI1 borrow-escape profile (E0413); its complete routine instead
  runs through normal native main. No ABI1 escape qualification is inferred.
- Actual NFKC generated C passes **394,045** independent pinned-corpus/scalar
  cases in each checked/release profile: 100,855 official column invariants,
  293,187 omitted-assigned-scalar identities and three composed boundary cases.
  Native boundary calls also pass ASan/UBSan. The oracle and adapter are the
  existing `coax/tests/unicode/conformance.py` corpus builder and `native.c`;
  Python's installed normalizer is not the expected-answer source.
  The final compiler reproduces those checked/release C artifacts and their
  generated header byte for byte, so the unchanged native corpus is not rerun.
- With direct static-key borrowing, GCC's native O0 individual-frame reports
  are 48 bytes for `un_class`, 160 for `un_decompose` and 80 for `un_pair`.
  These are individual compiler-reported frames, not total-stack bounds or a
  general speed claim. No fresh Apple, browser or GPU execution is claimed.

## Reproduction and evidence limits

Run `coax/build.sh`, targeted `coax/tests/run.sh --only pass fail checker native`
units, and chapter-6 book examples. Authority gates retain the whole check set;
mutation can be selected by physical unit with `--unit` and split using
`--shard`. One command remains bounded to the repository's 15-minute limit.

The cold bootstrap was executed in bounded phases from isolated source snapshots:
stage0 construction and policy gates, frontend checks/mutation and seed
reproduction, final host compilation, and final checks/reproduction/publication.
These are the original bootstrap commands and flags, with no mandatory gate
disabled. The final input fingerprint and artifact record must verify against
the worktree before installation. An earlier uninterrupted build was stopped
before the time limit; it is not reported as a successful complete command.
The completed bootstrap's input fingerprint is
`3e543190935a9ed27af463366c4ae68a7ff940cf3664ee852791a590c05aa23d`.
Its provenance verifies against the rebased worktree. `coax/build.sh` then
successfully installs that checked build through its normal verified-cache path.

The Unicode full-corpus driver includes Apple-specific SDK observations, so on
this Linux machine its unchanged pinned corpus builder and native adapter were
invoked for the two native profiles only. Runtime/ABI receipts identify the
actual development compiler and generated artifacts; they do not establish
new cross-target or whole-engine qualification. Selected verification logs and
artifacts were retained locally under `/tmp/oa-ro-dev/` during this task.
