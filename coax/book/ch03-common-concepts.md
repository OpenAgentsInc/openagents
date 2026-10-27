# 3. Common concepts

> **Runs today.** Every program in this chapter compiles with `coax0` and
> is checked by `check-examples.sh` (`examples/` in this folder). Sections
> marked *Design* describe what comes next.

This chapter covers what every program uses: declarations, types,
procedures, control flow and `print`. By the end you'll have written
three small programs and seen most of what `coax0` accepts.

## 3.1 Declarations

Coax has one declaration form, `name: type = value`, and three
shorthands for it:

```coax
#import "Basic";

main :: () {
  lives := 3;               // inferred: int
  name: string = "Kiki";    // declared and initialized
  score: int;               // zero
  ready: bool;              // false
  buffer: int = ---;        // uninitialized: you promise to write it first

  buffer = lives * 100;
  score += buffer;
  print("% has % lives and % points; ready: %\n", name, lives, score, ready);
}
```

```
Kiki has 3 lives and 300 points; ready: false
```

- **`name := value`** infers the type from the value. This is the form
  you'll write most.
- **`name: type = value`** says the type too. The compiler checks that the
  value has that type, and there are no implicit conversions to make it
  fit.
- **`name: type;`** sets the variable to zero: `0`, `false`, or the empty
  string. In Coax, a variable you didn't initialize is zero, never garbage.
- **`name: type = ---;`** opts out. The three dashes mean "uninitialized",
  for the rare buffer where zeroing would cost time you can measure.
  Writing `---` is a promise to write the variable before you read it.

And for constants, the one you've seen already:

- **`name :: value`** binds a constant. Procedures are constants. In
  `coax0` the only constants are top-level procedures. Number and type
  constants are *Design*.

The shape is the point. A local variable, a global, a procedure and a
type are all `name`, then what it is, then its value. Code can move from
inside a procedure to the top level of a file with almost no edits. Jai
calls this *code maturation*, and it matters more than it sounds: helper
code starts life as a block in the middle of a function and gets promoted
when it earns it.

### No shadowing

```coax
x := 1;
{
  x := 2;   // error[E0303]: 'x' is already declared at line 1; Coax does not allow shadowing
}
```

A name means one thing inside a procedure. Shadowing is a common source
of bugs that look correct, especially in code an agent wrote and a person
skimmed, so Coax refuses it.

## 3.2 Types

`coax0` has three types:

| Type | What | Zero value |
| --- | --- | --- |
| `int` | 64-bit signed integer (also spelled `s64`) | `0` |
| `bool` | `true` or `false` | `false` |
| `string` | UTF-8 bytes: a pointer and a byte count | `""` |

The language has many more, which the compiler already recognizes and
refuses until they're built:

```
error[E0301]: type 'f32' is not supported by coax0 yet
  fix: use int, bool or string
```

That refusal is a promise. A program that compiles with `coax0` today
won't change meaning when `f32` arrives. Chapter 5 covers the full set:
sized integers `s8` through `u64`, `f32` and `f64`, fixed point, vectors,
structs, enums and unions.

### Integer literals

```coax
a := 1_000_000;       // underscores are ignored
b := 0xff;            // hexadecimal
c := 0b1010;          // binary
```

A literal that doesn't fit in an `int` is an error (`E0104`), not a
silent truncation.

### No implicit conversions

```coax
x: int = "five";      // error[E0325]: 'x' is int but is initialized with string
b := 1 == true;       // error[E0311]: cannot compare int with bool
if 1 { }              // error[E0331]: condition must be bool, found int
```

An `int` is never a `bool`, a `bool` is never an `int`, and nothing
quietly becomes a string. When a conversion is wanted, you'll write it
out (`cast`, *Design*). The cost is a few characters, and the payoff is
that no line hides a conversion from its reader.

## 3.3 Procedures

```coax
#import "Basic";

main :: () {
  print("gcd(1071, 462) = %\n", gcd(1071, 462));
  print("7 is % and 9 is %\n", parity(7), parity(9 + 1));
  print("2^10 = %\n", power(2, 10));
}

// Declaration order does not matter: main can call these before they appear.
gcd :: (a: int, b: int) -> int {
  if b == 0 { return a; }
  return gcd(b, a % b);
}

parity :: (n: int) -> string {
  if n % 2 == 0 { return "even"; }
  return "odd";
}

power :: (base: int, exponent: int) -> int {
  result := 1;
  i := 0;
  while i < exponent {
    result *= base;
    i += 1;
  }
  return result;
}
```

```
gcd(1071, 462) = 21
7 is odd and 9 is even
2^10 = 1024
```

- A procedure is `(parameters) -> return_type { body }`. With no `->`, it
  returns nothing.
- Each parameter is `name: type`. Parameters can't be assigned to. Use a
  local if you need a value that changes.
- **Order doesn't matter.** `main` calls `gcd` before `gcd` is declared.
  There are no forward declarations and no header files, ever.
- **Every path must return.** If `gcd` could fall off its end without a
  `return`, the compiler says so (`E0338`).
- **Results must be used.** Calling `power(2, 10);` as a statement and
  throwing the answer away is an error (`E0323`). If a procedure's result
  doesn't matter, it shouldn't return one.

Procedures can call themselves, as `gcd` does. Because `coax0` writes C,
recursion uses the C stack.

## 3.4 Control flow

```coax
#import "Basic";

// How many steps does n take to reach 1?
steps :: (start: int) -> int {
  n := start;
  count := 0;
  while n != 1 {
    if n % 2 == 0 { n /= 2; }
    else { n = 3 * n + 1; }
    count += 1;
  }
  return count;
}

main :: () {
  best := 1;
  best_steps := 0;
  n := 1;
  while n < 10000 {
    s := steps(n);
    if s > best_steps {
      best = n;
      best_steps = s;
    }
    n += 1;
  }
  print("below 10000, % takes the most steps: %\n", best, best_steps);
}
```

```
below 10000, 6171 takes the most steps: 261
```

- **`if`** takes a `bool` with no parentheses, and its body is always a
  block in braces. `else if` chains, and `else` ends the chain.
- **`while`** repeats a block while its condition is true.
- **Assignment** is `=`, and `+= -= *= /= %=` work on `int` variables.
- **Operators**, from loosest to tightest binding:

  | Precedence | Operators | Operands |
  | --- | --- | --- |
  | 1 | `\|\|` | bool |
  | 2 | `&&` | bool |
  | 3 | `== != < <= > >=` | `==` and `!=` on any matching types, the rest on int |
  | 4 | `+ -` | int |
  | 5 | `* / %` | int |
  | 6 | unary `-` and `!` | int and bool |

- **Comparisons don't chain.** `1 < x < 10` is an error (`E0203`) with
  the fix "split it with `&&`". In C it compiles and means something
  else.
- **`==` compares strings by content.**
- **Division truncates toward zero**, as in C: `-7 / 2` is `-3`, and
  `-7 % 2` is `-1`.

*Design:* `for` loops over ranges and arrays (`for 0..n-1 { print("%",
it); }`), `break` and `continue` with labels, `switch`, `defer`, and the
`ifx a then b else c` expression. `coax0` reserves all of these words and
refuses them for now.

## 3.5 print

`print` is how a `coax0` program talks:

```coax
print("% has % lives\n", name, lives);   // each % takes the next value
print("100%% sure\n");                   // %% prints a %
```

It prints `int`, `bool` (as `true` or `false`) and `string`. What makes
it Coax rather than C is that the format is checked when you compile:

```
error[E0320]: format has 2 '%' slots but print was given 1 value
  fix: write '%%' for a literal percent sign
```

So the format has to be a string literal. A format built at run time
can't be checked, and the compiler says so (`E0319`). In C, a mismatched
`printf` is undefined behavior. In Jai it's a surprise at run time. In
Coax, a print that compiles prints what it says.

## 3.6 Comments

```coax
// to the end of the line
/* a block, which /* can nest */ inside another */
```

Nesting means you can comment out code that already contains block
comments. *Design:* `///` doc comments that attach to the next
declaration.

## 3.7 What you've seen

You've now used nearly all of `coax0`:

- the `#import` directive and the `Basic` module;
- procedures, parameters, returns, recursion and declarations in any order;
- `int`, `bool` and `string`;
- the four declaration forms, and no shadowing;
- `if`, `else if`, `else` and `while`;
- arithmetic, comparison and logic, with no implicit conversions;
- a checked `print`;
- overflow checks in debug builds and wrapping in release builds.

Chapter 4 is about what the compiler says when you get something wrong.
