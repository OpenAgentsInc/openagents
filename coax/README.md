# Coax

Everything Coax lives in this folder, at the root of the openagents repo.

| Path | What |
| --- | --- |
| [`spec.md`](spec.md) | The language specification and implementation plan |
| [`ROADMAP.md`](ROADMAP.md) | What to build next, dependencies, real consumers and completion gates for the full language |
| [`web.md`](web.md) | Proposal: the Coax web framework for openagents.com (Coaxial), with open questions |
| [`font.md`](font.md) | Proposal: Filament, the stroke font Coaxial pages draw text with |
| [`grammar.ebnf`](grammar.ebnf) | The bootstrap grammar for what coax0 accepts |
| [`book/`](book/README.md) | *The Coax Programming Language*, source for the published book |
| `src/`, `build.sh`, `tests/`, `examples/` | `coax0`, the bootstrap compiler |

Commands below run from the openagents repo root.

## coax0: the Coax bootstrap compiler

Coax is specified in [the language design](spec.md). The
[bootstrap grammar](grammar.ebnf) describes version 0.13.0's accepted subset.
The broader design is a plan, not a list of working features.

The lexer, parser and type checker are Coax compiled to C11. The native
C++20 host supplies storage, reconstructs the typed tree, evaluates bounded
compile-time work and emits one self-contained C11 file. The host C compiler
builds it into a program, or into an object file plus a generated header that
C, C++ and Swift hosts call. Generated code includes small print, checked
arithmetic, bounds and cast helpers; it never contains the compiler's
interpreter.

```sh
coax/build.sh
coax/bin/coax run coax/examples/mandelbrot.coax
coax/bin/coax run coax/examples/crc32.coax          # its table is built while compiling
coax/bin/coax run coax/examples/life.coax
coax/examples/pulse/run.sh                          # a Coax kernel inside a C host
coax/bin/coax layout coax/examples/pulse/pulse.coax
coax/bin/coax explain E0350
coax/bin/coax test coax/examples/money.coax         # checks written before the code
coax/bin/coax mutate coax/examples/money.coax       # every mutant must fail a check
coax/build.sh test
```

| Command | Does |
| --- | --- |
| `coax run FILE [--release]` | Compile and run |
| `coax build FILE [-o out] [--release] [--keep-c]` | Build an executable |
| `coax lib FILE [-o out.o] [--release]` | Build an object file of the `#export_c` procedures |
| `coax header FILE [-o out.h]` | Write the C header for those procedures, with layout assertions |
| `coax c FILE [--release] [--library [--header out.h]]` | Print the generated C, optionally as a library with its header |
| `coax metal FILE` | Print the file's f32/bool procedures as Metal Shading Language |
| `coax check FILE` | Typecheck and run compile-time code, without a C compiler |
| `coax proto PROFILE.pb --prefix Name [-o FILE]` | Generate inspectable Coax types/codecs directly from a descriptor |
| `coax deps FILE` | Print a checked JSON source/descriptor input inventory |
| `coax layout FILE` | Print every struct's size, alignment, offsets and padding, and every enum |
| `coax test FILE [--steps N]` | Run every `#test` and `#forall`; print counterexamples |
| `coax mutate FILE` | Change the checked code one site at a time; report mutants no check fails |
| `coax explain CODE` / `--all [--markdown]` | Explain an error code |
| `coax version` | Print the version |

`--json` prints diagnostics as one JSON object per line. `COAX_CC` picks the
C compiler (default `cc`). `--release` turns on C optimization and wrapping
integer arithmetic; bounds, casts, shifts and division by zero are checked in
every build. Normal compilation, including proto imports and iOS code generation, needs
C++20 and C11 tools. The checked Coax importer and its embedded wire runtime ship in
the compiler binary; no interpreter or helper installation is needed. Tests
and schema maintenance still use Python 3. `COAX_HOST_TARGET` can supply an
explicit host triple for bootstrap builds inside a cross-target environment;
the iOS generator selects the Mac triple and SDK. Compiler builds publish the
new binary atomically.

Version 0.13.0 adds native source debugging and phase reports:
`--debug-info`, `--source-map FILE` and `--timings FILE`. Debug executable and
library builds map to original Coax files by default. Plain `coax c` opts in
explicitly, so bootstrap output stays reproducible. The optional LLDB helper
shows authored names and checks source hashes. See [DEBUGGING.md](DEBUGGING.md)
for commands, artifacts and current limits. The [R0 engine harness](../verse/benchmarks/README.md)
uses the real cast and a pinned native evaluator; [original evidence](../docs/verification/coax-r0-2026-10-28.md)
records the initial gap, and [follow-up evidence](../docs/verification/coax-r0-pose-2026-10-28.md)
records the unchanged median gate passing in all three profiles.

## What works

```jai
#import "Basic";

Pulse :: struct {
  position: f32;
  speed: f32 = 1;
  hops: s32;
}

advance :: (pulses: [] Pulse, dt: f32) -> s64 #export_c {
  moved: s64 = 0;
  for i: 0..<pulses.count {
    if pulses[i].hops == 0 { continue; }
    pulses[i].position += pulses[i].speed * dt;
    moved += 1;
  }
  return moved;
}

SQUARES :: #run squares();
squares :: () -> [8] u16 {
  out: [8] u16;
  for i: 0..<8 { out[i] = cast(u16) (i * i); }
  return out;
}
#assert SQUARES[7] == 49;
```

- **Scalars.** `s8 s16 s32 s64 u8 u16 u32 u64` (`int` is `s64`, `uint` is
  `u64`), `f32 f64`, `bool`, counted byte `string`. No implicit conversions.
  Integer, float and character literals (`'a'`) and constants made only of
  literals are untyped: they take the type their use needs if the value fits,
  so `x: u8 = 200;` works and `x: u8 = 300;` is E0350.
- **Operators.** Arithmetic, `& | ^ << >>`, comparisons that do not chain,
  short-circuit `&& ||`. `cast(T) x` checks that the value fits; `cast,trunc(T)`
  keeps the low bits; `cast(string)` borrows a `[] u8` view. Debug builds trap
  on integer overflow and release builds wrap; floats are plain IEEE with no
  fused operations.
- **Data.** `Name :: struct { field: T; field: T = default; }` with defaults,
  literals `Name.{field = value}`, field access and assignment, and value
  copies. `Name :: enum u8 { a; b = 4; }` with `Name.a`, equality, and checked
  casts to and from integers. Fixed arrays `[N] T` (`[3] int.[1, 2, 3]`,
  `.count`) with value semantics, views `[] T` that borrow an array's storage
  (`a[..]`, `a[lo..<hi]`, writable through), strings as `[]`-like byte
  sequences (`s[i]`, `s[lo..<hi]`), and `size_of(T)`.
- **Control.** Blocks, `if`/`else`, `while`, `for i: lo..<hi`, `for x, i:
  array_view_or_string`, `break`, `continue`, `defer` (reverse order at every
  exit: end of block, `return`, `break`, `continue`), `ifx c then a else b`.
- **Order.** Operands and arguments run left to right. When an operand list
  has a call and another operand that could call or trap, the emitter stores
  them in order before using them.
- **Compile time.** `#run` evaluates ordinary procedures, including arrays,
  structs and loops; aggregate results become static constants in the C.
  Math intrinsics (`sin cos tan exp log sqrt floor abs pow`) and the
  conversions `int(x)`, `f32(x)` work there too, and must give finite results.
  `#assert` checks a bool while compiling. `type_name(x)` typechecks without
  evaluating. `print` while compiling is refused (E0404).
- **The C boundary.** `#export_c` procedures keep their names, use the C
  calling convention, and appear in `coax header`, with `_Static_assert`
  checks that the C compiler's layout of every struct equals Coax's. Views
  cross as `{data, count}`; `int` is `int64_t`. `#foreign` imports a host C
  symbol, and `coax metal` emits f32/bool functions as Metal (see below).
- **Checks first.** `#test "name" { ... }` and `#forall a: u8, b: 0..<10 { ...
  }` run in the evaluator, never in the program, each with a 20,000,000-step
  budget; `#forall` covers every combination of bool, enum, small-integer and
  range bindings and reports the first counterexample. `#assert cond;` is a
  statement too, and traps in debug and release. `#authority;` makes every
  procedure need a check (E0380) and runs the checks on every build. `coax
  mutate` checks the checks: boundary, operator, literal, condition and
  statement mutants must each fail one. See
  [`examples/money.coax`](examples/money.coax) and
  [the verification report](../docs/verification/2026-10-27-formal-verification-brainstorm.md).
- **Diagnostics.** Stable codes with locations and fix text, `--json` for
  tools, and `coax explain` for every code. Compile-time failures add call
  site notes.

## Multiple source files

`#load "part.coax";` shares declarations in one source graph. Paths resolve
beside their declaring file; canonical units load once, active cycles fail,
and checks/diagnostics retain their physical source ownership. `#authority`
does not spread to unmarked loaded units. `coax test` and `coax mutate` accept
`--library` for pure units without a main or C exports.

`coax deps FILE` lists canonical sources in deterministic traversal order and
resolved descriptor inputs as schema-version-1 JSON. It is a path inventory,
not a cache key. No Python or external preprocessing is used. The compiler's
own graph is `lib/compiler/compiler.coax`; the iOS HUD and the book capstone
are actual consumers. General module visibility and packages remain planned.
See [Chapter 22](book/ch22-organizing-programs.md) for exact scope and limits.

## Native libraries and Metal

The [native iOS home](../apps/openagents-ios/README.md) runs its scene, HUD and
resource admission in Coax and its shader math through the Metal emitter:

```sh
coax/bin/coax c verse/home.coax --library --header /tmp/Home.h > /tmp/Home.c
coax/bin/coax metal verse/light.coax > /tmp/Light.metal
apps/openagents-ios/build.sh
```

- `name :: (h: int) -> f64 #foreign;` imports a host C symbol by its exact
  name. Foreign procedures pass integers, floats and bool (E0376), and
  compile-time code cannot call them (E0404). `coax header` and
  `c --library --header` declare them next to the `#export_c` procedures.
- `int(x)`, `f32(x)`, `u8(x)` and the other number types convert like
  `cast(T) x`: checked, float to integer truncates toward zero, f64 to f32
  traps when a finite value doesn't fit. `1.5f32` and `1.5f64` are typed
  literals; a bare `1.5` is untyped.
- `sin cos tan exp log sqrt floor abs pow` take floats of one type (E0375) and
  call the C library's `float` or `double` function. Compile-time results
  must be finite (E0412). There is no cross-platform libm guarantee.
- `len(x)` is `x.count`; `byte_at(s, i)` is `cast(int) s[i]`, bounds-checked.
- `coax metal FILE` emits Metal Shading Language from the same checked tree:
  f32 and bool values, `if`/`else`, locals and acyclic calls. Loops, strings,
  integers, f64, `print` and `#foreign` calls are E0377. Native stages bind
  textures and attributes; this is not a general GPU compiler.

## Compile-time contract

`#run` binds to the next unary expression; parenthesize a larger one. Results
may be scalars, strings, structs and arrays, but not views (E0409), because a
view points at memory that exists only while compiling. No runtime local or
parameter can be captured.

There is no interpreted host I/O, process, network, clock, random, allocator or
foreign-call API. The driver still reads source and invokes its configured
toolchain; it is not a process sandbox.

One compilation allows 100,000 steps (each evaluated expression or statement,
and each array element created), 64 active interpreted calls and 64
constant-dependency levels. Cycles, uninitialized reads, overflow, division by
zero, bounds and bad casts are errors in debug **and release**. These are
execution limits, not a complete limit for hostile source.

## CPU layout reflection

`type_info(T)`, `field_info(T, index)` and `enum_info(T, index)` return
schema-1 ordinary records. Generate inspector tables with `#run`, or use
constant scalar projections in array lengths. Invalid categories/indices and
bounded expansion are checked. These queries describe bootstrap LP64 CPU
storage; stable type IDs, alternate targets, defaults and procedure metadata
remain planned. See [the spec](spec.md#102-reflection-now-and-later) and the
[book example](book/examples/layout-reflection.coax).

## Layout

| Path | Responsibility |
| --- | --- |
| `src/coax.h` | Tokens, types, values, syntax tree, declarations |
| `src/lex.cpp` | Native lexer storage and token bridge |
| `src/parse.cpp` | Native syntax-tree storage and parser bridge |
| `src/check.cpp` | Native type/tree storage and checked-result bridge |
| `src/eval.cpp` | Bounded typed-AST evaluator and literal folding |
| `src/emit_c.cpp` | C11 output, ordered operands, defer lowering, headers, layout reports |
| `src/errors.cpp` | The explanation of every error code |
| `src/verify.cpp` | `coax test` and `coax mutate` |
| `src/main.cpp` | Driver and unity build |
| `tests/pass/` | Exact native output in debug and release |
| `tests/fail/` | Expected diagnostics and runtime traps |
| `tests/compile_time.py` | Isolation, dependency, budget, error and generated-artifact checks |
| `tests/tools.py` | Explain, JSON diagnostics, layout, libraries and headers, determinism |
| `tests/reflection.py` | Layout values, generated tables, bounds and an independent native ABI oracle |
| `tests/native.py` | Float math, foreign/export ABI linked into C, ordered effects, Metal profile |
| `tests/verify.py` | Checks, counterexamples, budgets, `#authority` builds and mutation reports |
| `examples/` | Demos with expected output; `pulse/` links a Coax kernel into C |

## Limits and next work

The bootstrap does not implement pointers, dynamic arrays, unions, `switch`,
optionals and errors (`try`/`or_else`), generics, general reflection, macros,
compiler plugins, context, effects, jobs, general GPU programs, a build language, a native
backend or a resident compiler. Reserved syntax for them is diagnosed. Views
borrow storage; E0383 diagnoses direct returns of local fixed-array views,
their simple local aliases/subranges and borrowed strings. Calls, stored
aggregate borrows and general region lifetimes remain outside this check;
see [the scoped verification note](verification-local-borrows.md).
Native line mapping and LLDB name presentation work on the
tested Mac and iOS Simulator; optimized values still follow native debugging
limits, and no Coax expression evaluator is provided.

Next: finish [roadmap R0](ROADMAP.md)'s measured adoption gate. The real pose
comparison and Metal frame capture now run, and the pose rewrite passes the
proposed median budget. Physical-device/frame-tail measurements, full-loop
attribution, release/status generation and independent review remain open. See §20 of the specification for later
metadata, build, parallelism, browser and reload gates.

## Coax frontend and bootstrap seeds

`lib/compiler/lexer.coax` owns byte classification, keywords, longest-match
operators, nested comments, escapes, source locations, recovery, numeric
conversion and diagnostic text. `src/lex.cpp` only adapts tokens, owned strings,
locations and diagnostics to the existing native token ABI. Binary64 values cross
that ABI as raw bits copied into the parser's existing double field; the host
does not parse numbers.

`lib/compiler/parser.coax` owns the accepted grammar, precedence, declaration
lookahead, indexed tree construction, syntax diagnostics and nesting/storage
admission. It also admits `#import_proto` syntax and import prefixes/counts
before native file I/O. `src/parse.cpp` copies that tree into the checker's existing arena
objects; it contains no grammar or fallback. AST tag mappings have compile-time
ABI assertions. Raw token lengths stay separate from decoded string bytes.
Parsing stops with the first error; a failed result admits no partial tree.

The first compiler uses the generated C seeds in `bootstrap/`. Normal builds
rebuild both C/header pairs from Coax and refuse a mismatch, run checks/mutation gates,
link the regenerated lexer/parser with the Coax importer/fingerprints, then check final
self-regeneration before publishing. A seed refresh is explicit and uses an
already trusted compiler. See [the bootstrap contract](bootstrap/README.md).

## Limits of the verification gates

`#authority` requires actual check-time procedure execution, not just a call
in a dead branch. It does not establish branch coverage, prove a signature
verifier or authorize a decoded message. Finite checks quantify only over
their stated nonempty domains. Mutation is a selected defect model; unchecked
procedures, no mutation sites and surviving non-exempt mutants fail the command.
Equivalent marks remain human review obligations. See `tests/verify.py` for
false-green and signed-range regressions.

The authoritative wire runtime is ordinary Coax in `lib/proto/wire.coax`, with
`#authority`, bounded checks and a zero-survivor mutation gate. Bootstrap builds
run both gates before publishing the compiler. `lib/proto/memory.coax` holds two
small native-address wrappers covered by ABI/sanitizer tests. Shell byte copies
embed these sources into coax0; no Python build dependency returns.

`tests/proto/runtime.coax` additionally checks actual imported routines. Legacy
schema declaration hashes characterize compatibility; independent protoc vectors
check interoperability. The descriptor parser and generator now live in
`lib/proto/importer.coax`, with their own authority checks and mutation gate.
`lib/proto/fingerprint.coax` owns SHA-256, runtime extraction and provenance
assembly, with its own authority and zero-survivor mutation gates. Stage0
compiles these actual implementations together to C11; the final compiler links it
and regenerates identical C. The C++ frontend/check interpreter/emitter and
native host effects remain trusted. See [the evidence table](proto.md#native-importer-audit-boundary)
for the scope and limits of each gate.

## Shared protocol

The [OpenAgents Protocol](../docs/protocol/openagents-protocol-spec.md) defines
interchange for every OpenAgents surface, including Verse. Source schemas and
selected-profile descriptors live in root [proto/](../proto/README.md), beside
`coax/`. Verse is the engine name (formerly Metareal). The implemented
[`#import_proto` importer](proto.md) consumes selected descriptor closures and
generates typed Coax views, presence/oneof/repeated accessors and bounded
caller-buffer codecs. It adds no parallel language-only schema.

The first [Nostr mirror profiles](../proto/nostr/README.md) now compile into
selected descriptors. `examples/nostr-wire.coax` is a bounded C-ABI consumer of
the shared Event projection's kind field. `tests/nostr.py` compiles it and reads
a canonical source-derived protobuf fixture, including kind zero, all selected
External kinds, unknown fields, wide timestamps and malformed inputs. That narrow
reader remains as a regression reference. `examples/nostr-proto.coax` now uses
the descriptor-generated API; `tests/proto_import.py` checks actual native
round trips, all scalar types, presence, oneofs, repeats, malformed inputs and
ASan/UBSan. An explicit `--oracle` run checks both directions with pinned
protoc. Signed JSON parsing and Nostr admission remain separate gates.
