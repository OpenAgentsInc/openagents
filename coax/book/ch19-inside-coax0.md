# 19. Inside coax0

> **Runs today.** This chapter describes `coax0` as it is in `coax/src/`.

`coax0` is about 5,000 lines of C-style C++20. It has no dependencies
beyond a C++ compiler for itself and a C compiler for its output. You can
read it in an afternoon, and that's on purpose.

## 19.1 The pipeline

```
source → lexer → parser → checker → evaluator → C emitter → cc → executable
          lex.cpp  parse.cpp  check.cpp  eval.cpp   emit_c.cpp
```

| File | Lines | Job |
| --- | --- | --- |
| `coax.h` | ~330 | Tokens, interned types, values, syntax tree, the arena, declarations shared by every stage |
| `lex.cpp` | ~340 | Bytes to tokens: comments, integers, floats, characters, strings with escapes, directives, reserved keywords |
| `parse.cpp` | ~710 | Tokens to a syntax tree by recursive descent, including types, structs, enums and loops; stops at the first error |
| `check.cpp` | ~1,330 | Types and layouts, untyped literals, names, places, returns, print formats; reports every error |
| `eval.cpp` | ~760 | Runs `#run`, `#assert` and constants over the checked tree, within budgets, and folds literals |
| `emit_c.cpp` | ~1,170 | The checked tree to one C11 file with its runtime; headers and layout reports |
| `errors.cpp` | ~130 | The explanation for every error code (`coax explain`) |
| `main.cpp` | ~320 | The driver (`run`, `build`, `lib`, `header`, `c`, `check`, `layout`, `explain`, `version`) and the unity build |

## 19.2 One translation unit

`main.cpp` `#include`s every other `.cpp` file, so there is one C++20
compiler invocation. Run the build script from the repository root:

```sh
coax/build.sh
```

The script first copies the authoritative `.coax` protobuf runtime sources into
an ignored C++ include, then compiles with `-I bin`. It runs the pure wire core's
checks and mutation gate before atomically publishing `coax/bin/coax`. There is
no Python generator or installed runtime-file dependency. Directly compiling
`src/main.cpp` requires the generated include from that build step.

## 19.3 Memory: one arena

Syntax tree nodes come from one bump-pointer `Arena` and are never freed
individually. The compiler runs once and exits, so freeing nodes would
be wasted work. Lists inside nodes are `Slice<T>` (pointer and count),
copied into the arena once parsing has the final count. Strings are
`Str` views into the source text, or into the arena for literals after
escape processing. That's the same discipline the language recommends:
one owner, one reset point, no per-node bookkeeping.

## 19.4 The lexer

The lexer is a hand-written loop over bytes. Points worth knowing:

- Block comments nest, tracked with a depth counter.
- Integer literals accept `_`, `0x` and `0b`, and a value past the u64
  maximum is detected while digits accumulate (`E0104`), not after.
- A float needs a digit after the point (`1.5`, `2e-3`), so `0..<n` still
  lexes as a range. `'a'` is an integer token.
- `#name` becomes a directive token and `@name` an annotation token.
- Every word in the spec's keyword list is reserved now, so later
  features can't change the meaning of today's programs.
- Lexer errors don't stop the lexer. It reports each bad token and
  continues.

## 19.5 The parser

Recursive descent, one function per grammar rule, with precedence
climbing for binary operators. It stops at the first syntax error
(Chapter 4 explains why). Two details:

- **Comparisons don't chain.** After parsing one comparison, the parser
  checks whether another follows and reports `E0203`.
- **Top-level `::` is decided by lookahead.** `name :: (` might begin a
  procedure or a parenthesized constant like `LATER :: (40);`. The
  parser scans to the matching `)` and checks for `{` or `->`.

The accepted grammar is written down in `coax/grammar.ebnf`.

## 19.6 The checker

The checker runs in passes over declarations: imports, then procedure
signatures (so procedures can be called before they're declared), then
constants, whose types are resolved on demand in dependency order, then
`#assert` conditions, then the `main` check, then procedure bodies.

- Locals live in a flat vector with a scope mark per block. Leaving a
  block truncates the vector. Lookups scan backwards, and a duplicate
  name anywhere in scope is `E0303`, because Coax has no shadowing.
- `print` is special-cased as a builtin from `Basic`. The checker counts
  `%` slots in the literal format and compares them with the arguments.
- Types are interned: each `TypeInfo` exists once, so two types are equal
  when their pointers are. `[N] T` and `[] T` are made on demand.
- Literals are **untyped** until a use gives them a type. The checker folds
  literal arithmetic exactly in 128 bits, then checks that the result fits
  the type it becomes (`E0350`, or `E0407` for a folded result).
- Assignments and `for` variables are checked as **places**: a local, a
  field of a place, or an element of a place or view. Parameters, constants
  and loop variables aren't places.
- "Every path returns" is a small structural check. A block returns if
  any statement in it returns, and an `if` returns if both branches do.

## 19.7 The evaluator

`eval.cpp` runs compile-time code directly on the checked syntax tree.
There is no bytecode yet, and the spec says to add one only when
profiling shows the tree walk is too slow.

- A `Value` is a type plus an integer, a float, a string, or shared storage
  for a struct or array. Storing an aggregate copies it; a view points into
  its array's storage, so writes through a view are seen. A view can't be
  a `#run` result (`E0409`), so nothing from the compiler's memory can leak
  into the output program.
- Every statement and expression visited costs one step. The budget is
  `MAX_STEPS = 100000` per compilation, and call and dependency depth
  are capped at `MAX_DEPTH = 64`.
- Arithmetic checks overflow with the same rules in debug and release.
- Constants and folded `#run` sites are memoized.
- Calling `print` at compile time is `E0404`. There is no host access to
  forbid beyond that, because no other host operation exists in the
  language yet.

## 19.8 The C emitter

The emitter writes one C file:

1. A header comment, the source path for trap messages, and
   `#pragma STDC FP_CONTRACT OFF`, so the C compiler can't fuse float
   operations and change results.
2. The runtime: `coax_string`, the print helpers, checked arithmetic for
   every integer width (trapping with a line number in debug, wrapping in
   release), and bounds, cast and shift checks used in every build.
3. Every type: structs as `coax_t_Name`, arrays as `struct { T e[N]; }`
   named `coax_aN_<tag>` so they copy by value, views as `{data, count}`
   named `coax_v_<tag>`, and a `coax_init_<tag>` for each type with
   defaults.
4. A prototype for every procedure, so declaration order never matters.
5. Each procedure, with names prefixed (`coax_p_` for procedures, `l_` for
   locals) unless it's `#export_c`.
6. A C `main` that calls the Coax `main`, unless the file is built as a
   library.

Three lowerings are worth reading in the output:

- **Order.** C doesn't fix the order of operands. When an operand list has
  a call and another operand that could call or trap, the emitter stores
  them into temporaries left to right, inside a comma expression.
- **`defer`.** Deferred code is copied to every exit of its block, in
  reverse order. `return e` stores `e` in `coax_ret` first, so a deferred
  statement can't change what's returned.
- **`print`.** The format is split while compiling into a sequence of
  `coax_put` and typed print calls. Floats print the shortest text that
  reads back as the same value.

Constants and `#run` results are emitted as literals, or as
`static const` initializers for structs and arrays. Strings are emitted
with octal escapes for non-printable bytes, so a byte can never merge
with a following digit. `coax header` writes the C declarations of the
`#export_c` procedures and their types, with `_Static_assert`s that the C
compiler lays out every struct exactly as Coax did.

## 19.9 Tests

```sh
coax/build.sh test
```

- `tests/pass/*.coax` must print exactly the matching `.out`, in debug
  and in release.
- `tests/fail/*.coax` start with `// expect: E0xxx` and must fail with
  that code, or with `// expect: trap TEXT` and must stop at run time
  printing `TEXT`.
- `tests/compile_time.py` checks evaluator isolation, dependencies,
  budgets, errors and the generated output.
- `tests/tools.py` checks `explain`, `--json`, `layout`, libraries,
  headers (compiled as C and C++) and deterministic output.
- `examples/*.coax` with a `.out` file run in debug and release, and
  `examples/pulse/run.sh` links a Coax kernel into a C host and compares
  it with a C reference.
- `tests/verify.py` checks `coax test` and `coax mutate`: counterexamples,
  budgets, `#authority` builds, and that `examples/money.coax` kills every
  mutant.
- `book/check-examples.sh` compiles and runs every example in this book.

A new feature comes with a passing program, a failing program for each
new error, and a book example once it's in a *Runs today* chapter.
