# 9. Compile time

> **Runs today (9.1–9.5), design (9.6–9.9).** `#run`, `#assert` and
> `type_name` work in `coax0` 0.2.0, and the examples in the first half are
> checked. Typed code, macros, plugins and `#if` are designed (spec §10).

Most languages draw a hard line between the program and the compiler.
Coax draws a different line: your ordinary procedures can run while the
program is being compiled, and their results become part of the program.
This is the idea from Jai that the rest of the language leans on most.

## 9.1 `#run`

```coax
#import "Basic";

// Computed while compiling. The program only contains the answers.
CELLS :: #run cells_in_grid(24, 16);
PRIME_100 :: #run nth_prime(100);
SIZE_CLASS :: #run size_class(CELLS);

#assert CELLS == 384;
#assert PRIME_100 == 541;
#assert type_name(SIZE_CLASS) == "string";

cells_in_grid :: (width: int, height: int) -> int {
  return width * height;
}

nth_prime :: (n: int) -> int {
  found := 0;
  candidate := 1;
  while found < n {
    candidate += 1;
    if is_prime(candidate) { found += 1; }
  }
  return candidate;
}

is_prime :: (n: int) -> bool {
  if n < 2 { return false; }
  d := 2;
  while d * d <= n {
    if n % d == 0 { return false; }
    d += 1;
  }
  return true;
}

size_class :: (cells: int) -> string {
  if cells > 256 { return "large"; }
  return "small";
}

main :: () {
  print("% cells (%), prime #100 is %\n", CELLS, SIZE_CLASS, PRIME_100);
  print("still callable at run time: %\n", is_prime(97));
}
```

```
384 cells (large), prime #100 is 541
still callable at run time: true
```

`#run expression` evaluates the expression while compiling. The
procedures it calls are ordinary procedures: the same `is_prime` runs at
compile time for `PRIME_100` and at run time in `main`. Nothing is
written twice, and there is no second language for "constant" code.

Look at the C that `coax c` generates and you'll find the answers, not
the work:

```c
coax_print(COAX_STR("% cells (%), prime #100 is %\n", 29), 3, (coax_any[]){coax_any_int(INT64_C(384)), coax_any_string(COAX_STR("large", 5)), coax_any_int(INT64_C(541))});
```

The program starts with `384` and `541` already in it. The interpreter
that computed them belongs to the compiler and never ships in your
program.

### What `#run` binds to

`#run` applies to the expression right after it: a call, a name, or a
parenthesized expression. To run something larger, put it in parentheses:

```coax
TOTAL :: #run (cells_in_grid(24, 16) + 2);
```

### Calls in constants need `#run`

```coax
AREA :: square(4);
```

```
s.coax:2:9: error[E0408]: a constant procedure call requires #run
```

You can always see where the compiler runs your code, because it only
does so where `#run` says. `TILES :: TILE_EDGE * TILE_EDGE;` needs no
`#run`, because it's arithmetic on constants, not a call.

## 9.2 `#assert`

```coax
#assert CELLS == 384;
```

`#assert` checks a `bool` while compiling. If it's false, there is no
program:

```
as.coax:3:14: error[E0406]: compile-time assertion failed
  #assert AREA == 15;
               ^
```

Use it for the facts code depends on but can't check at run time
cheaply: a table size that must be a power of two, a budget that must
fit, a constant that must agree with another. To call a procedure inside
an assertion, use `#run` there too: `#assert #run is_prime(PRIME_100);`.

## 9.3 `type_name`

```coax
#assert type_name(SIZE_CLASS) == "string";
```

`type_name(expression)` gives the name of the expression's type. It's
the first, smallest piece of reflection. Its argument is type-checked but
**never evaluated**, so `type_name(1 / 0)` is `"int"` and doesn't divide.
Later, `type_of`, `size_of` and `type_info` describe whole structs,
including their layout on a specific target (9.7).

## 9.4 Compile time is sealed

Code running inside the compiler can compute. It can't do anything else.

- **No output or host access.** Calling `print` while compiling is an
  error, not a message in your build log:

  ```
  g.coax:3:25: error[E0404]: compile-time execution cannot perform output or host I/O
    greet :: () -> string { print("hi\n"); return "hello"; }
                            ^
    note: compile-time call at g.coax:2:18
  ```

  There are no files, processes, network, clock or randomness at compile
  time. The same source always compiles to the same constants.
- **No reaching into the running program.** A `#run` inside a procedure
  can't read that procedure's parameters or locals, because they don't
  exist yet:

  ```
  cap.coax:1:37: error[E0401]: compile-time expression cannot capture runtime value 'n'
    f :: (n: int) -> int { return #run (n + 1); }
                                        ^
  ```

- **Every `#run` must succeed**, even one inside a branch that never runs.
  Compile-time code is checked as part of the program, not only when it's
  reached.
- **Only plain values come out.** A `#run` result is an `int`, `bool` or
  `string`, and becomes a literal in the generated code. Nothing from the
  compiler's own memory can leak into your program.

## 9.5 Compile time is bounded

A loop that never ends shouldn't hang your build:

```
ct2.coax:5:14: error[E0400]: compile-time evaluation exceeded 100000 steps
    while true { i += 1; }
               ^
  note: compile-time call at ct2.coax:9:17
```

| Limit | Value | Error |
| --- | --- | --- |
| Evaluation work in one compile | 100,000 steps (expressions and statements visited) | `E0400` |
| Nested compile-time calls | 64 | `E0403` |
| Constants depending on constants | 64 deep | `E0403` |
| Overflow or division by zero | Always an error, in debug **and** release | `E0407` |
| Reading an uninitialized (`---`) local | Always an error | `E0405` |

The limits count work, not seconds, so a program that compiles on a fast
laptop compiles on a slow CI machine too. Overflow is checked at compile
time in every build mode, so a release build can never compute a
different constant from a debug build.

Results are remembered. A constant used in ten places is computed once.

## 9.6 Typed code (design)

The next step is code as a value. A parameter of type `Code` receives an
expression without evaluating it. `#insert` puts code into the program,
and `#expand` makes a procedure into a macro that expands at its call
site:

```coax
assert_eq :: (a: Code, b: Code) #expand {
  va := #insert a;
  vb := #insert b;
  if va != vb {
    fail("left: % (%)\nright: % (%)", va, #text a, vb, #text b, at = #location(a));
  }
}
```

This is the macro from Forrest Smith's Jai code that prints the source
text of a failed comparison. Coax adds rules Jai doesn't have:

- **Hygiene.** Names a macro declares stay inside it. To touch a caller's
  variable, a macro names it in its signature, with read or write
  permission.
- **No silent double evaluation.** An argument used twice must be bound to
  a temporary, or the macro has to say it duplicates it.
- **Provenance.** The compiler keeps the chain from the call, to the
  generator, to the generated code, to the C it became. Errors in
  generated code point at the call first, with notes into the expansion.
- **Budgets in work, not time:** proposed at 64 nested expansions and
  100,000 new syntax nodes per workspace. A millisecond limit would make
  validity depend on the machine.

## 9.7 Reflection (design)

`type_info(T)` describes a type: fields, offsets, alignment, defaults,
annotations, enum values and procedure signatures. Layout questions take
an explicit **target**, because a struct can be laid out differently on
an iPhone, in WebAssembly and on a GPU. The compiler never answers with
its own machine's `sizeof`.

A temporary type ID from the compiler is not a stable identity. Engine
component IDs and protocol names stay explicit and survive a rebuild.

## 9.8 Plugins (design)

A plugin is Coax code the compiler loads during a build. It gets typed
events: file loaded, parsed, signature ready, typechecked, all ready work
checked, before lowering, and complete. It can propose new source,
dependencies and diagnostics through checked APIs. Everything it adds
goes through the same parser, checker and layout rules as handwritten
code, and a plugin can never mark unchecked code valid. A plugin's
identity, version and inputs are part of the build's cache key.

The Jai plugin that runs a program after a successful compile is the
model. Coax's version gets a tagged union of messages instead of
unchecked casts, and a capability model for any file or process work.

## 9.9 Conditional compilation (design)

```coax
#if TARGET_IS_IOS { ... } else { ... }
```

Both branches must parse, and only the chosen one is checked and
compiled. The condition is a compile-time constant, an explicit build
input, and part of the cache key. It is not text deletion before the
compiler sees the file, as C's preprocessor does.
