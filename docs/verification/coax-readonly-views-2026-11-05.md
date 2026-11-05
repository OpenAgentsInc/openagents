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
- The 15 new source-level checker checks cover capability conversions,
  immutable array admission, casts/truncation, target refusal, conditional
  unification, generic calls/mangling, final concrete containment, memo reuse,
  bounded failures and tracked-call relocation. Existing checks stay enabled.
- Parser mutation: **1,791/1,791 killed**, zero survivors. New read-only policy:
  **87/87 killed**, zero survivors. Affected type, generic, expression and
  statement unit gates kill **1,924**, **813**, **1,115** and **1,419** mutants,
  respectively, with zero survivors. Earlier failed mutation attempts supplied
  regression cases; no mutants were marked equivalent.
- The final bootstrap's complete checker mutation gate kills **12,680/12,680**
  mutants with zero survivors, including the call-index null-sentinel regression.
  This supersedes the earlier borrow-unit attempt that exposed that missing check.
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
- With direct static-key borrowing, GCC's native O0 individual-frame reports
  are 48 bytes for `un_class`, 160 for `un_decompose` and 80 for `un_pair`.
  These are individual compiler-reported frames, not total-stack bounds or a
  general speed claim. No fresh Apple, browser or GPU execution is claimed.

## Reproduction and evidence limits

Run `coax/build.sh`, targeted `coax/tests/run.sh --only pass fail checker native`
units, and chapter-6 book examples. Authority gates retain the whole check set;
mutation can be selected by physical unit with `--unit` and split using
`--shard`. One command remains bounded to the repository's 15-minute limit.

The Unicode full-corpus driver includes Apple-specific SDK observations, so on
this Linux machine its unchanged pinned corpus builder and native adapter were
invoked for the two native profiles only. Runtime/ABI receipts identify the
actual development compiler and generated artifacts; they do not establish
new cross-target or whole-engine qualification. Selected verification logs and
artifacts were retained locally under `/tmp/oa-ro-dev/` during this task.
