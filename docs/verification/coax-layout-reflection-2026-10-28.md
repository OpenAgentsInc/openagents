# Coax 0.12.0: CPU layout reflection

## What ships

`type_info(T)`, `field_info(T, index)` and `enum_info(T, index)` take type
syntax and produce ordinary schema-1 Coax records. The parser supplies their
record declarations; the Coax checker resolves layouts and lowers queries into
typed literals and ordinary bounds-checked indexing. The native bridge
allocates/copies trees, including generated field initializers and Coax-owned
provenance. The existing native evaluator, C emitter and mutation runner remain
trusted components.

Direct scalar projections fold during type checking, including inside another
struct's array length. A `#run` procedure can construct a persistent inspector
table containing names and offsets. Runtime selectors use a finite array and
retain bounds traps in debug and release. The query-bearing graph requests
65,536 extra nodes; expansion shares the checked workspace and fails with
E0381 rather than publishing a partially allocated value. Wrong categories or
empty selections are E0382; constant bounds errors retain E0374.

Enum metadata preserves the underlying-width bits zero-extended to u64,
including negative s8 values and the u64 maximum. Names use resolved aliases.
Views report their element type with a zero type-level element count.

## Actual consumer

`verse/haunt-layout.coax` describes the existing packed vertex format and
derives the 48-byte stride, texture-layer byte and eight scalar admission
offsets. `haunt-buffer.coax` loads it and uses its compile-time table against
the actual bundled geometry. No protobuf identity, binary asset or Metal
packing contract changes. Emitted C asserts the vertex's native size,
alignment and every field offset; the independent native oracle also checks
randomized nested structures and arrays against separately authored C types.

The runnable book example generates a Transform inspector; the compiler
fixture generates field offsets and enumerates signed enum metadata.

## Evidence and commands

- Parser and checker `#test`/`#forall`, and their zero-survivor mutation gates.
  The refreshed seeds are emitted with the previously trusted 0.11.0 compiler;
  normal bootstrap must reproduce them and the complete compiler source graph.
- `coax/tests/reflection.py`: ten cases covering ordinary metadata values,
  `#run` tables, cross-unit/type-resolution dependencies, scalar and enum edges,
  malformed/invalid queries, both-mode bounds traps, expansion refusal,
  helper-free relocated compilation, source mutation provenance, and independent
  C layout oracles under ASan/UBSan.
- The same ten cases against an ASan/UBSan-instrumented compiler, with leak
  detection disabled only for the existing process-lifetime compiler arena.
- `coax/tests/run.sh`: existing compiler fixtures, CLI, native and frontend
  bridges, source graphs, Nostr/proto import and verification regressions.
- `PROTOC=... python3 coax/tests/proto_import.py --oracle`: official protoc
  descriptor and wire interoperability.
- `PROTOC=... BOOK_SANITIZE=1 coax/book/check-examples.sh`: exact debug/release
  output and status, checks, mutation, native hosts and linked book excerpts.
- `PROTOC=... PROTO_PYTHON=... verse/test_haunt.sh`: the real bundled world,
  corrupted span rejection, rendering packets, gameplay, all nine triggers,
  editing/teaching, saves and official protobuf round trips under sanitizers.
- `apps/openagents-ios/build.sh`, then `HomeUITests` and `HauntUITests` on a
  dedicated iOS 26.5 simulator: app/GPU integration of the derived buffer gate.

### Verified results

The Python-free bootstrap reproduced the explicit seeds and complete compiler
module, then built the iOS app successfully. Parser checks: 159 passed;
checker checks: 551 passed; both authority mutation gates had zero survivors.
The engine gate killed all 96 reachable mutants. The complete compiler suite
passed its 82 native fixtures and all Python/kernel suites; all 21 explicit
protoc-oracle tests passed. The ten reflection cases also passed against the
instrumented compiler. All book checks and the real-world sanitizer/protobuf
smoke passed.

After rebasing onto the latest main (including iOS 1053), the dedicated iOS
26.5 simulator passed all four Home/Haunt UI tests with zero failures: walking,
Foreman/collision, hotbar/terminal, and portal/conversation/editing/return.

## Mutation scope

Compiler-created metadata nodes carry provenance, so the mutation runner does
not alter unconsumed schema fields and call those mutations of source code.
Authored dynamic selectors, surrounding logic and explicitly authored metadata
record values still supply mutation sites. No equivalent-mutant exemptions
were added. The current runner mutates checked trees; it does not reparse a
mutated type operand or enumerate alternate layouts. Compiler authority checks
and the independent C oracle verify the lowering itself. A zero-survivor score
is finite test evidence, not a correctness proof.

## Deliberate limits

This is bootstrap LP64 CPU layout metadata, not a portable wire format or a
Metal/WebAssembly target query. A dynamic query can materialize a table at
runtime; use `#run` once for frequently inspected tables. Metadata does not
grant admission authority or expose live compiler objects. First-class types,
stable declaration IDs, annotations/defaults/procedure/effect metadata, general
derivation, typed syntax insertion and plugins remain planned. Stage 3 has
started; it is not complete. This change does not claim frame-time parity or a
full engine migration.
