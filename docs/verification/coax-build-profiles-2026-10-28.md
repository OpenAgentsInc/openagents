# Coax build-profile conditionals

Task #7e39 selects the minimal part of spec §10.5. General conditional
compilation, declaration scheduling and target/module conditions remain open.

## Implementation

The Coax parser owns `#if BUILD_DEBUG` and optional `else` inside struct field
lists and statement blocks. The native bridge supplies the explicit development
boolean from the compiler command's existing `--release` option. The original
`PS_parse` entry point remains a development-default wrapper; `PS_parse_build`
is explicit. `PS_State` gains a trailing development flag, an internal ABI
change whose consumers compile against the freshly generated header.

Both branches parse, allocate nodes and count toward the existing resource
budgets. Allocation ranges are disjoint, so inactive branch nodes are cleared
in one bounded pass before metadata/wide-helper discovery. The allocation count
is retained. Field holders splice selected fields in order and then disappear;
statement blocks retain lexical scope. The checker, evaluator and emitters see
ordinary selected AST. There is no C++ grammar, token-text deletion, arbitrary
condition evaluator or new native AST kind.

All pre-existing parser indexed-tree snapshots retain their original node
identity/contents for plain source. Profile parsing also tests ordered fields,
nesting, no-else blocks, unsupported selectors, invalid disabled syntax,
caller prefix/spare-buffer preservation and absence of unreachable helper nodes.
The full parser has 175 passing groups and 1119/1119 killed mutants, with no
waivers or scoped exclusions.

## Actual native boundaries

`coax/tests/build_profiles.py` has five focused tests. Its compiled C and
public header produce Cell size16/field_count2 in development, size4/field_count1
in release, with native default initialization and values9/107. ASan/UBSan C
consumers use those same generated headers. Actual Wasm executes both profiles;
Metal checks exclude inactive host code while rejecting active host strings.

Additional cases exercise unknown inactive names/types, selected-only #run
failure, inactive reflection and mul_wide helper suppression, both-branch syntax
errors, lexical scope, duplicate fields, original physical diagnostics, loaded
source units and disabled-branch nesting-budget refusal. The selected build
profile is distinct from native optimization and from the source-debug map
option. Required #assert and ordinary runtime-if behavior stay unchanged.

The existing Python-free seed refresh and bootstrap remain the adoption gate.
Use the normal compiler suite once for this frontend change; unrelated main
commits do not invalidate results for identical source/tool closures. New
pass/fail fixtures are in the standard suite, along with the focused profile
suite. The book example is exercised in both profiles. Optional Python/Bun and
native test tooling are not required by ordinary bootstrap or product builds.

## Crypto source admission

The scalar-v1 and aggregate-v1 source auditors do not yet analyze multiple build
selections. They explicitly reject lexer-recognized #if directives in every
physical unit before parsing/pruning. Runtime if and string contents such as
"#if" retain their prior meaning. The policy is checked Coax; both representation
bridges call it. This keeps the new compiler feature from silently broadening a
previous source-audit admission claim. It does not prove object/JIT lowering,
runtime guard obligations or production secret safety.

## Consumer and ABI limits

Profile-dependent fields change native layout. Producers, consumers, reflection
and generated headers must use the same profile; signed/wire schemas never
inherit a changed memory image. Current iOS generation keeps the checked Coax
profile across Xcode configurations. This compiler change does not switch the
app to release arithmetic, add crypto support, rebuild TestFlight or alter
phone sign-in ownership. Native/iOS build evidence cannot stand in for physical
phone timing or production-secret approval.

The final Python-free bootstrap and compiler suite passed: 98 fixture cases,
the focused five profile tests and the remaining suite; the optional pinned
protoc oracle was skipped by the ordinary suite. The book example passed and
killed all mutants in both profiles (5 development, 1 release); book structure
checks passed. The source-admission policy passed 2 groups and killed its one
mutant; four actual-compiler guard tests and both six-test auditor regressions
passed. An arm64 iOS Simulator app build generated the app's Coax code and
succeeded. This is a build check, not a physical-device runtime test.
