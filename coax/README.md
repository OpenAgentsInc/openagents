# Coax

Coax is a programming language for real-time software: the Verse, its
renderers and simulations, and the rest of OpenAgents' code that runs at
120 frames a second on a phone. Everything Coax lives in this folder.

| Path | What |
| --- | --- |
| [`spec.md`](spec.md) | The language specification (draft) and its start plan |
| [`book/`](book/README.md) | *The Coax Programming Language*, a GitBook draft |
| `src/`, `build.sh`, `tests/`, `examples/` | `coax0`, the first compiler |

## coax0

`coax0` is Phase 1 of the start plan (spec §20): a compiler from a Coax
subset to one self-contained C11 file, which `cc`, Xcode or Emscripten build
like any other C source. Commands run from the openagents repo root:

```sh
coax/build.sh                         # builds coax/bin/coax (one unity translation unit)
coax/bin/coax run coax/examples/hello.coax
coax/bin/coax build file.coax -o out    # --release: wrapping arithmetic, -O2; --keep-c keeps out.c
coax/bin/coax c file.coax               # print the generated C
coax/build.sh test                    # build, then run tests/run.sh
```

`COAX_CC` picks the C compiler (default `cc`).

## What coax0 accepts today

```coax
#import "Basic";

main :: () {
  print("Hello, world!\n");
}
```

- `#import "Basic"` (the only module); `print(format, values...)` with `%`
  slots, `%%` for a percent sign. The format must be a literal, and its slot
  count is checked against the values at compile time.
- Top-level procedures `name :: (a: int, b: string) -> int { ... }`, in any
  order. `main :: ()` is the entry point.
- Types `int` (= `s64`), `bool` and `string`. No implicit conversions.
- Locals `x := e;`, `x: int = e;`, `x: int;` (zero) and `x: int = ---;`
  (uninitialized). No shadowing.
- `if` / `else if` / `else`, `while`, `return`, `=` and `+= -= *= /= %=`.
- Operators `+ - * / %`, `== != < <= > >=` (comparisons do not chain),
  `&& || !`, unary `-`. `==` works on strings.
- Integer overflow traps with the file and line in debug builds and wraps in
  `--release`. Division by zero traps in both.
- Every error has a stable code (`error[E0313]`), the source line with a
  caret, and a `fix:` line where there is one. Lexer errors are `E01xx`,
  parser errors `E02xx`, checker errors `E03xx`.

The spec's other reserved words and types (`for`, `struct`, `f32`, `job`...)
are recognized and refused with "not supported by coax0 yet", so programs
written now will not silently change meaning later.

## Layout

| Path | What |
| --- | --- |
| `src/coax.h` | Tokens, syntax tree, types, the arena |
| `src/lex.cpp` | Source to tokens |
| `src/parse.cpp` | Tokens to a syntax tree (stops at the first syntax error) |
| `src/check.cpp` | Names, types, returns, print formats (reports every error) |
| `src/emit_c.cpp` | Syntax tree to C, with the Basic runtime written into the output |
| `src/main.cpp` | Driver; includes the other `.cpp` files (unity build) |
| `tests/pass/` | Programs and their exact output, run in debug and release |
| `tests/fail/` | Programs that must fail with a given error code, or trap at run time |

## Next (Phase 1 of the start plan)

`for` over ranges, structs with `#derive(eq, hash)`, `SOA` and `SOA(4)`,
arrays and views, multiple returns, `ifx`, `defer`, `#distinct`, optionals
and `try`, unions, `pure` operators and `job` capture lists. The exit test
is the `src/paynet` pulse update written in Coax and shipped in the iOS app.
