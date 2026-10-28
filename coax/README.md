# Coax

Everything Coax lives in this folder, at the root of the openagents repo.

| Path | What |
| --- | --- |
| [`spec.md`](spec.md) | The language specification and implementation plan |
| [`grammar.ebnf`](grammar.ebnf) | The bootstrap grammar for what coax0 accepts |
| [`book/`](book/README.md) | *The Coax Programming Language*, a GitBook draft |
| `src/`, `build.sh`, `tests/`, `examples/` | `coax0`, the bootstrap compiler |

Commands below run from the openagents repo root.

## coax0: the Coax bootstrap compiler

Coax is specified in
[the language design](spec.md).
The [bootstrap grammar](grammar.ebnf) describes version 0.2.0's
accepted subset. The broader design is a plan, not a list of working features.

The C++20 compiler parses and checks one source file, executes bounded scalar
compile-time work, and emits a self-contained C11 program. The host C compiler
builds it. Generated programs include small string/print/arithmetic helpers;
they do not contain the compiler's interpreter.

```sh
coax/build.sh
coax/bin/coax version
coax/bin/coax check coax/examples/budget.coax
coax/bin/coax c coax/examples/budget.coax
coax/bin/coax run coax/examples/budget.coax
coax/bin/coax build coax/examples/budget.coax -o /tmp/coax-budget --keep-c
coax/build.sh test
```

`COAX_CC` picks the C compiler (default `cc`). `--release` requests C
optimization and wrapping runtime integer arithmetic. `--keep-c` preserves the
C file alongside a built executable. `check` performs typechecking and
compile-time evaluation without invoking a C compiler or running `main`.
Normal compilation needs C++20 and C11 tools; the tests also use Python 3.

## What works

```coax
#import "Basic";

square :: (n: int) -> int { return n * n; }
TILE_COUNT :: #run square(16);
#assert TILE_COUNT == 256;
#assert type_name(TILE_COUNT) == "int";

main :: () {
  print("tiles: %\n", TILE_COUNT);
}
```

- Procedures `name :: (a: int, b: string) -> int { ... }`, declared in any
  order; immutable parameters and a void `main :: ()` entry point.
- Types `int`/`s64`, `bool` and counted byte `string`; no implicit conversions.
- Top-level immutable constants `NAME :: expression;`, including forward
  references and parenthesized initializers. No local constants or shadowing.
- Locals `x := e;`, `x: int = e;`, `x: int;` (zero) and `x: int = ---;`
  (explicitly uninitialized).
- Blocks, `if`/`else if`/`else`, `while`, `return`, assignment and integer
  compound assignments.
- Integer arithmetic, boolean short-circuiting, comparisons and string
  equality. Comparisons do not chain. Debug overflow traps; release wraps;
  division by zero always traps.
- `#import "Basic"` provides `print` with literal `%` format strings checked
  against argument count; `%%` prints a percent sign.
- `#run` computes a scalar value using ordinary checked procedures, locals,
  loops and recursion. Results become C literals; executed host output is
  rejected. Constant procedure calls require explicit `#run`.
- `#assert` checks a compile-time bool. `type_name(expression)` is a scalar
  reflection intrinsic: it typechecks but does not evaluate its operand.
- Stable lexer/parser/checker/evaluator diagnostic codes and source locations;
  compile-time failures include bounded call-site notes.

## Compile-time contract

`#run` binds to the next unary expression. Parenthesize a larger expression:
`#run (square(16) + 2)`. Only scalar int/bool/string values can be retained.
No runtime local or parameter can be captured, including in a nested `#run`
inside a procedure that another `#run` calls.

There is no interpreted host I/O, process, network, clock, random, allocator or
foreign-call API. Executing `print` fails with E0404. The driver still reads
source and invokes its configured toolchain; it is not a process sandbox.

One compilation allows 100,000 evaluated expression/statement visits, 64 active
interpreted calls and 64 constant-dependency levels. Cycles, uninitialized
reads, overflow and division by zero are errors in debug **and release**.
Folded sites and constants are memoized. These are execution limits, not a
complete parsing/allocation limit for hostile source.

Inspect `coax c` output to see the actual constants. `budget.coax` emits a
literal 64, rather than invoking an evaluator on startup.

## Layout

| Path | Responsibility |
| --- | --- |
| `src/coax.h` | Tokens, syntax tree, scalar types and declarations |
| `src/lex.cpp` | Source to tokens |
| `src/parse.cpp` | Recursive-descent parser |
| `src/check.cpp` | Names, types, constants, immutable parameters, returns and formats |
| `src/eval.cpp` | Bounded typed-AST evaluator and literal folding |
| `src/emit_c.cpp` | C11 output with small support helpers |
| `src/main.cpp` | Driver and unity build |
| `tests/pass/` | Exact native output in debug and release |
| `tests/fail/` | Expected diagnostics and runtime traps |
| `tests/compile_time.py` | Isolation, dependency, budget, error and generated-artifact checks |

## Limits and next work

The bootstrap does not implement structs, arrays, pointers, general reflection,
macros, compiler plugins, context, effects, shaders, a build language, a native
backend or a resident compiler. Unsupported reserved syntax is diagnosed.
Runtime operand lowering does not yet guarantee left-to-right execution for
multiple effectful calls; sequence them into locals. C debug locations still
refer to generated C.

Next: data types, views, half-open loops, cleanup, ordered operand lowering and
a tested C ABI. Prove a small `src/paynet` kernel before changing the app or
replacing its existing C++/CMake/Xcode paths. See §20 of the specification for
later metadata, build, parallelism, browser and reload gates.

## Shared protocol

The [OpenAgents Protocol](../docs/protocol/openagents-protocol-spec.md) defines
interchange for every OpenAgents surface, including Verse. Source schemas and
selected-profile descriptors live in root [proto/](../proto/README.md), beside
`coax/`. Metareal is the engine name. Coax's future importer and bounded codecs
must consume this source rather than defining a parallel language-only schema.
