# 19. Inside coax0

> **Runs today.** This chapter describes `coax0` as it is in `coax/src/`.

`coax0` is about 3,000 lines of C-style C++20. It has no dependencies
beyond a C++ compiler for itself and a C compiler for its output. You can
read it in an afternoon, and that's on purpose.

## 19.1 The pipeline

```
source → lexer → parser → checker → evaluator → C emitter → cc → executable
          lex.cpp  parse.cpp  check.cpp  eval.cpp   emit_c.cpp
```

| File | Lines | Job |
| --- | --- | --- |
| `coax.h` | ~190 | Tokens, syntax tree, types, the arena, declarations shared by every stage |
| `lex.cpp` | ~260 | Bytes to tokens: comments, numbers, strings with escapes, directives (`#run`), annotations (`@x`), reserved keywords |
| `parse.cpp` | ~400 | Tokens to a syntax tree by recursive descent; stops at the first error |
| `check.cpp` | ~450 | Names, types, constants, returns, print formats; reports every error |
| `eval.cpp` | ~285 | Runs `#run`, `#assert` and constants over the checked tree, within budgets |
| `emit_c.cpp` | ~380 | The checked tree to one C11 file, with the runtime embedded |
| `main.cpp` | ~210 | The driver (`run`, `build`, `c`, `check`, `version`), and the unity build |

## 19.2 One translation unit

`main.cpp` `#include`s every other `.cpp` file, so `coax/build.sh` is a
single compiler invocation:

```sh
c++ -std=c++20 -O1 -g -Wall -Wextra -o bin/coax src/main.cpp
```

That's under a second. There is no build system to configure, no
incremental-build bugs, and nothing to cache. It's the performance
stack's unity-build advice, applied to the compiler itself.

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
- Integer literals accept `_`, `0x` and `0b`, and overflow is detected
  while digits accumulate (`E0104`), not after.
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
- "Every path returns" is a small structural check. A block returns if
  any statement in it returns, and an `if` returns if both branches do.

## 19.7 The evaluator

`eval.cpp` runs compile-time code directly on the checked syntax tree.
There is no bytecode yet, and the spec says to add one only when
profiling shows the tree walk is too slow.

- A value (`CtValue`) is a type plus an integer or a string view. It
  can't hold a pointer, so nothing from the compiler's memory can leak
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

1. A header comment and the source path, which the runtime uses in trap
   messages.
2. The runtime: `coax_string`, `coax_print`, checked arithmetic
   (`coax_add` and the others, which call `__builtin_*_overflow` and trap
   with a line number) and wrapping release versions.
3. A prototype for every procedure, so declaration order never matters
   in C.
4. Each procedure, with names prefixed (`coax_p_` for procedures, `l_`
   for locals) so they can't collide with C.
5. A C `main` that calls the Coax `main`.

Constants and `#run` results are emitted as literals. Strings are emitted
with octal escapes for non-printable bytes, so a byte can never merge
with a following digit.

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
- `book/check-examples.sh` compiles and runs every example in this book.

A new feature comes with a passing program, a failing program for each
new error, and a book example once it's in a *Runs today* chapter.
