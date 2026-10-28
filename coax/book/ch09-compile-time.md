# 9. Compile time

> **Runs today (9.1–9.6), design (9.7–9.10).** `#run`, `#assert`,
> `type_name`, `size_of` and the checks of 9.6 work in `coax0` 0.5.0, including `#run`
> results that are structs and arrays, and the examples in the first half
> are checked. Typed code, macros, reflection beyond `type_name` and
> `size_of`, plugins and `#if` are designed (spec §10).

Most languages draw a hard line between the program and the compiler.
Coax draws a different line: your ordinary procedures can run while the
program is being compiled, and their results become part of the program.
This is the idea from Jai that the rest of the language leans on most.

## 9.1 `#run`

```jai
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
{ coax_print_int(INT64_C(384)); coax_put(COAX_STR(" cells (", 8)); coax_put(COAX_STR("large", 5)); coax_put(COAX_STR("), prime #100 is ", 17)); coax_print_int(INT64_C(541)); coax_put(COAX_STR("\n", 1)); }
```

The program starts with `384` and `541` already in it. The interpreter
that computed them belongs to the compiler and never ships in your
program.

### What `#run` binds to

`#run` applies to the expression right after it: a call, a name, or a
parenthesized expression. To run something larger, put it in parentheses:

```jai
TOTAL :: #run (cells_in_grid(24, 16) + 2);
```

### Calls in constants need `#run`

```jai
AREA :: square(4);
```

```
s.coax:2:9: error[E0408]: a constant procedure call requires #run
```

You can always see where the compiler runs your code, because it only
does so where `#run` says. `TILES :: TILE_EDGE * TILE_EDGE;` needs no
`#run`, because it's arithmetic on constants, not a call.

### Tables and structs

`#run` can return anything without a view in it: a scalar, a string, an
enum, a struct or a fixed array. Lookup tables are the common case. The
work runs once, while compiling, and the program starts with the table
already built:

```jai
#import "Basic";

COUNT :: 32;

// The first COUNT primes, found by trial division.
first_primes :: () -> [COUNT] u16 {
  out: [COUNT] u16;
  found := 0;
  candidate := 2;
  while found < COUNT {
    if is_prime(candidate) {
      out[found] = cast(u16) candidate;
      found += 1;
    }
    candidate += 1;
  }
  return out;
}

is_prime :: (n: int) -> bool {
  for d: 2..<n {
    if d * d > n { break; }
    if n % d == 0 { return false; }
  }
  return n >= 2;
}

Gaps :: struct { widest: u16; after: u16; twins: int; }

measure :: () -> Gaps {
  g: Gaps;
  table := PRIMES;                  // a local copy: a view needs a variable
  for p, i: table[1..<COUNT] {      // i counts from 0, so table[i] is the prime before p
    gap := p - table[i];
    if gap == 2 { g.twins += 1; }
    if gap > g.widest { g.widest = gap; g.after = table[i]; }
  }
  return g;
}

PRIMES :: #run first_primes();
GAPS :: #run measure();

#assert PRIMES[COUNT - 1] == 131;
#assert GAPS.twins == 10;

main :: () {
  print("%\n", PRIMES);
  print("%\n", GAPS);
}
```

```
[2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89, 97, 101, 103, 107, 109, 113, 127, 131]
Gaps.{widest = 14, after = 113, twins = 10}
```

One constant can be computed from another: `measure` reads `PRIMES`,
and the compiler evaluates them in dependency order, whatever order the
file lists them in. An aggregate result becomes a `static const` in the
generated C:

```c
static const coax_t_Gaps coax_k1 = {.widest = ((uint16_t)UINT64_C(14)), .after = ((uint16_t)UINT64_C(113)), .twins = INT64_C(10)};
```

The second `#assert` is there because the first draft of this example
said 11. The compiler refused to build it until the claim matched the
table. That is the use: a fact you'd otherwise check by reading output
becomes a line the build checks every time.

`coax/examples/crc32.coax` does the same for a real format: the
256-entry CRC-32 table is a `#run` constant, and `#assert` checks it
against the standard check value `0xCBF43926` before any C is written.

## 9.2 `#assert`

```jai
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

```jai
#assert type_name(SIZE_CLASS) == "string";
```

`type_name(expression)` gives the name of the expression's type. It's
the first, smallest piece of reflection. Its argument is type-checked but
**never evaluated**, so `type_name(1 / 0)` is `"int"` and doesn't divide.
It names every type `coax0` has: `"u8"`, `"Vec2"`, `"[3] int"`,
`"[] int"`.

`size_of(T)` is the second piece. It takes a type, not a value, and is
known while checking, so it can size an array: `buffer: [size_of(Pulse)]
u8;`. It answers for the host the C is compiled for, which is all
`coax0` targets. Later, `type_of` and `type_info` describe whole structs,
including their layout on a specific target (9.8).

## 9.4 Compile time is sealed

Code running inside the compiler can compute. It can't do anything else.

- **No output or host access.** Calling `print` while compiling is an
  error, not a message in your build log:

  ```
  g.coax:3:25: error[E0404]: compile-time execution cannot perform output or host I/O
    greet :: () -> string { print("hi\n"); return "hello"; }
                            ^
    note: compile-time call at g.coax:2:15
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
- **Only values come out, never views.** A scalar result becomes a
  literal in the generated code, and a struct or array becomes a static
  constant. A view would point into the compiler's own memory, so a
  result that contains one is refused:

  ```coax
  EVENS :: #run evens();

  evens :: () -> [] int {
    a := [4] int.[0, 2, 4, 6];
    return a[..];              // a view of the compiler's memory
  }
  ```

  ```
  ct_view.coax:1:10: error[E0409]: a compile-time result cannot contain a view; views borrow memory that exists only while compiling
    EVENS :: #run evens();
             ^
  ```

  Return the array itself.
- **Checks are errors, not traps.** An index, slice, cast or shift that
  would trap at run time stops the build instead, with a note at the
  `#run` that led there:

  ```
  ct_bounds.coax:5:15: error[E0410]: index 4 is out of bounds for 4 elements
      return table[n];           // should be n - 1
                  ^
    note: compile-time call at ct_bounds.coax:1:14
  ```

  ```
  ct_cast.coax:4:10: error[E0411]: cast to u8 is out of range
      return cast(u8) n;
             ^
    note: compile-time call at ct_cast.coax:1:15
  ```

  A constant can't be half-computed, so there is no "trap and continue"
  while compiling.

## 9.5 Compile time is bounded

A loop that never ends shouldn't hang your build:

```jai
FOREVER :: #run spin();

spin :: () -> int {
  i := 0;
  while true { i += 1; }
  return i;
}
```

```
spin.coax:5:21: error[E0400]: compile-time evaluation exceeded 100000 steps
    while true { i += 1; }
                      ^
  note: compile-time call at spin.coax:1:17
```

Creating an array counts too, one step per element, so a table can't
dodge the budget by being one big declaration:

```jai
MAP :: #run blank_map();

blank_map :: () -> [200000] u8 {
  map: [200000] u8;          // 200,000 elements, one step each
  return map;
}
```

```
big.coax:4:3: error[E0400]: compile-time evaluation exceeded 100000 steps
    map: [200000] u8;          // 200,000 elements, one step each
    ^
  note: compile-time call at big.coax:1:13
```

| Limit | Value | Error |
| --- | --- | --- |
| Evaluation work in one compile | 100,000 steps (expressions and statements visited, and array elements created) | `E0400` |
| Nested compile-time calls | 64 | `E0403` |
| Constants depending on constants | 64 deep | `E0403` |
| Overflow or division by zero | Always an error, in debug **and** release | `E0407` |
| Reading an uninitialized (`---`) local | Always an error | `E0405` |
| A view in a result | Always an error | `E0409` |
| Index or slice out of bounds | Always an error | `E0410` |
| Cast, enum value or shift out of range | Always an error | `E0411` |

The limits count work, not seconds, so a program that compiles on a fast
laptop compiles on a slow CI machine too. Overflow is checked at compile
time in every build mode, so a release build can never compute a
different constant from a debug build.

Results are remembered. A constant used in ten places is computed once.

## 9.6 Checks first

The evaluator that runs `#run` also runs checks: `#test` and `#forall`.
They are written before the code they check, and they are never part of
the program.

```jai
#import "Basic";
#authority;

#forall paid: 0..<40, refund: -2..<45 {
  #assert refund_ok(paid, refund) == (refund >= 0 && refund <= paid);
}

#test "a full refund is allowed" {
  #assert refund_ok(7, 7);
}

refund_ok :: (paid: int, refund: int) -> bool {
  return refund >= 0 && refund <= paid;
}

main :: () {
  print("refund 30 of 40: %\n", refund_ok(40, 30));
}
```

`#forall` runs its block once for every combination of its bindings:
here 40 × 47 = 1,880 cases. A binding can be `bool`, an enum, an integer
of up to 16 bits (all 65,536 values of a `u16`), or a range of `int`
with constant bounds. `#assert` inside a block is a statement. In a check
it fails the check; in the program it traps with `assertion failed`, in
debug and release alike.

```
$ coax test refunds.coax
ok   #forall paid: 0..<40, refund: -2..<45 (1880 cases, 40884 steps)
ok   #test a full refund is allowed (1 case, 14 steps)
2 checks, 2 passed, 0 failed
```

Each check has its own budget, 20,000,000 steps unless `--steps` says
otherwise, separate from the 100,000 steps a compile gets.

**Red first.** Write the checks, then `refund_ok`. Until it is right,
the checks fail with values you can paste into a regression test. Here,
`refund <= paid` was written `refund < paid`:

```
refunds.coax:5:3: error[E0406]: assertion failed
    #assert refund_ok(paid, refund) == (refund >= 0 && refund <= paid);
    ^
  note: in #forall paid: 0..<40, refund: -2..<45
  counterexample: paid = 0, refund = 0
```

**`#authority`.** A file that guards money, signatures or grants starts
with `#authority;`. Its checks run on every build, so that failure above
is a failed `coax build`. And every procedure except `main` and
`#foreign` imports must be called from some check, directly or through
other procedures. Code with no check doesn't compile:

```
error[E0380]: 'triple' is not exercised by any #test or #forall
```

**Checking the checks.** A check can pass and still pin nothing down.
`coax mutate` changes the code one site at a time and runs the checks
against every change. It swaps `<` and `<=`, `==` and `!=`, `+` and `-`,
`&&` and `||`; changes a literal by one; negates conditions; and removes
assignments and calls. Each change, a mutant, must fail some check. With
only one weak test, `refund_ok(7, 3)`, all four survive:

```
$ coax mutate weak.coax
weak.coax:10:22: mutant survived: '&&' became '||'
    return refund >= 0 && refund <= paid;
                       ^
weak.coax:10:17: mutant survived: '>=' became '>'
weak.coax:10:20: mutant survived: 0 became 1
weak.coax:10:32: mutant survived: '<=' became '<'
4 mutants, 0 killed, 4 survived
```

With the `#forall` above, all four are killed. Some changes can't be
observed by any input. In `if x < lo { return lo; }`, making it `x <= lo`
returns the same value. A reviewer marks such a line
`// mutant: equivalent`, and mutants there that survive are counted
apart rather than failing the run.

This is how Coax does test-driven development for code that must not
fail. It copies Bitcoin Core's habits: checks that never turn off,
exhaustive tests of small instances, and test vectors written before
implementations. `coax/examples/money.coax` applies it to money ranges,
checked sums, routing fees and market deadlines, with every mutant
killed.

## 9.7 Typed code (design)

The next step is code as a value. A parameter of type `Code` receives an
expression without evaluating it. `#insert` puts code into the program,
and `#expand` makes a procedure into a macro that expands at its call
site:

```jai
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

## 9.8 Reflection (design)

`type_info(T)` describes a type: fields, offsets, alignment, defaults,
annotations, enum values and procedure signatures. Layout questions take
an explicit **target**, because a struct can be laid out differently on
an iPhone, in WebAssembly and on a GPU. The compiler never answers with
its own machine's `sizeof`.

A temporary type ID from the compiler is not a stable identity. Engine
component IDs and protocol names stay explicit and survive a rebuild.

## 9.9 Plugins (design)

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

## 9.10 Conditional compilation (design)

```jai
#if TARGET_IS_IOS { ... } else { ... }
```

Both branches must parse, and only the chosen one is checked and
compiled. The condition is a compile-time constant, an explicit build
input, and part of the cache key. It is not text deletion before the
compiler sees the file, as C's preprocessor does.
