# Coax 0.11.0 source units

## What shipped

`#load` composes explicit physical source units in one namespace, once per
canonical path, in deterministic depth-first visitation order. The checked
Coax source kernel owns syntax, identity/cycle decisions, graph budgets and
verification-scope policy. Native code performs filesystem I/O and transports
storage. The existing Coax grammar validates each physical unit before
composition; declarations and delimiters cannot cross file boundaries.

`#authority` retains physical ownership. Loading an authority unit preserves
its executed-check requirements; loading an unmarked utility does not mark
it. Mutation includes reachable helpers, with entry/authority coverage gates.
`test` and `mutate --library` work without a main or native exports. Original
file diagnostics, mutation reports, descriptor-relative paths and composed
native traps retain their source owner. `deps` exposes a checked JSON path
inventory. General modules/visibility, packages and a content-hashed build
workspace remain planned.

The actual bootstrap now reads `coax/lib/compiler/compiler.coax`. Its source
kernel has an explicit generated seed, checks, a zero-survivor mutation gate
and final regeneration. The iOS HUD loads shared types, and the book's request
policy/capstone and frontend example use actual source graphs.

## Evidence

All commands ran in a fresh worktree from remote main on the build Mac.

- `coax/build.sh test`: Python-free bootstrap, exact seeds, combined-module
  fixed point, compiler fixtures and complete Python/native regression suite.
- Source kernel: 13 checks; 145/145 generated mutants killed, no exemptions.
  The changed type checker also passes its full authority/mutation gate.
- `coax/tests/sources.py`: 17 cases through the real CLI, also run against the
  entire compiler instrumented with ASan/UBSan. Includes diamonds, symlinks,
  cycles, limits, physical syntax boundaries, authority scope/dead calls,
  private library checks, relative descriptors, missing files, original error
  locations, native traps, escaped filenames, relocated/helper-free operation,
  the Metal backend and native policy ABI. The native budget oracle uses
  independent widened addition, in debug and optimized builds.
- `PROTOC=... python3 coax/tests/proto_import.py --oracle`: official protoc
  29.3 interoperability, complete current-owned-profile generation/typechecking,
  legacy declaration characterization and native/importer sanitizer checks.
- `PROTOC=... BOOK_SANITIZE=1 coax/book/check-examples.sh`: every runnable book
  program in debug/release, authored checks, request mutations, native C/Swift
  examples, codec/file round trip, protoc in both directions, navigation and
  checked source excerpts.
- `apps/openagents-ios/build.sh`: simulator build succeeded with the actual
  generated Coax C/headers and Metal.
- iOS 26.5 simulator: `HomeUITests` and `HauntUITests`, four tests passed:
  Start/movement, Foreman, hotbar/terminal, and portal/conversation/edit/save/return.

The live owned descriptor had advanced with the voice schemas on upstream
main. Its old declaration-characterization input is now an exact frozen copy
from the already recorded reference commit. Existing descriptor/declaration
hash expectations were preserved. The live aggregate descriptor is separately
generated and typechecked; this does not replace independent wire oracles.
No published schema or compatibility snapshot changed.

After rebasing over the new voice app/docs work, compiler inputs were byte-
identical to the verified build. The combined book examples and simulator
build passed, as did the current native voice engine/protobuf fixture suite
(all 11 frame kinds decoded by protoc). The scoped Home/Haunt smoke tests
were repeated on that combined app.

## Reproduce whole-compiler sanitizer coverage

After the normal bootstrap, from the repository root:

```sh
validation=$(mktemp -d)
cc -std=c11 -O1 -g -fsanitize=address,undefined -fno-omit-frame-pointer \
  -c coax/bin/proto_importer.c -o "$validation/compiler.o"
c++ -std=c++20 -O1 -g -fsanitize=address,undefined -fno-omit-frame-pointer \
  -I coax/bin coax/src/main.cpp "$validation/compiler.o" -o "$validation/coax"
ASAN_OPTIONS=detect_leaks=0 UBSAN_OPTIONS=halt_on_error=1 \
  COAX_TEST_COMPILER="$validation/coax" python3 coax/tests/sources.py
```

Leak detection is disabled for the compiler's existing process-lifetime arena;
address and undefined-behavior instrumentation remain enabled. Keep failures
and toolchain details when reporting a regression.

## Limits

This is finite evidence, not a soundness or bootstrap-security proof. Seeds,
native source-owner annotations, parser/evaluator/emitter, filesystem behavior
and toolchains remain trusted. The input inventory is not a lockfile/cache key.
Canonical paths define identity; hard-link names remain distinct. Bounds are
64 units, 32 active units, 16 MiB raw source, 512 loads/unit and delimiter depth
128, alongside the existing parser/checker/evaluator limits. Source-byte bounds
do not bound every compiler allocation or all wall time. Debugger stepping
still uses generated C. No runtime permission is granted by source authority,
and no device release or TestFlight upload was part of this change.
