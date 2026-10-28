# 3. Common concepts

> **Runs today.** Every program in this chapter compiles with `coax0`
> 0.3.0 and is checked by `check-examples.sh` (`examples/` in this
> folder): declarations, every scalar type, untyped literals, casts, bit
> operations, `for`, `break`, `continue` and `ifx`. Only the passages
> marked *Design* (local constants, `switch`, loop labels, doc comments)
> describe what comes next.

This chapter covers what every program uses: declarations, types,
procedures, control flow and `print`. By the end you'll have written a
handful of small programs and seen the scalar half of what `coax0`
accepts. Structs and enums are Chapter 5; arrays and views are
Chapter 6.

## 3.1 Declarations

Coax has one declaration form, `name: type = value`, and three
shorthands for it:

```jai
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
- **`name: type;`** sets the variable to zero: `0`, `0.0`, `false`, or
  the empty string. In Coax, a variable you didn't initialize is zero, never garbage.
- **`name: type = ---;`** opts out. The three dashes mean "uninitialized",
  for the rare buffer where zeroing would cost time you can measure.
  Writing `---` is a promise to write the variable before you read it.

And for constants, the one you've seen already:

- **`name :: value`** binds a constant. Procedures are constants, and so
  are values declared at the top of a file:

```jai
#import "Basic";

TILE_EDGE :: 16;
TILES :: TILE_EDGE * TILE_EDGE;     // constants can use other constants
WORLD_NAME :: "Haunted House";
LATE :: EARLY + 1;                  // in any order
EARLY :: 41;

main :: () {
  print("% has % tiles; the answer is %\n", WORLD_NAME, TILES, LATE);
}
```

```
Haunted House has 256 tiles; the answer is 42
```

A constant is computed while compiling, and the generated C contains the
answer (`256`), not the arithmetic. A constant made only of literals,
like `TILE_EDGE`, has no type of its own yet: it takes the type of the
place it's used, the same way a literal does (3.2). Constants can refer
to each other in any order. A cycle (`A :: B; B :: A;`) is an error (`E0340`), and so is
assigning to a constant (`E0344`). Constants can be computed by calling
procedures too, with `#run`, which Chapter 9 covers. Local constants
inside a procedure and type constants are *Design*.

The shape is the point. A local variable, a global, a procedure and a
type are all `name`, then what it is, then its value. Code can move from
inside a procedure to the top level of a file with almost no edits. Jai
calls this *code maturation*, and it matters more than it sounds: helper
code starts life as a block in the middle of a function and gets promoted
when it earns it.

### No shadowing

```jai
x := 1;
{
  x := 2;   // error[E0303]: 'x' is already declared at line 1; Coax does not allow shadowing
}
```

A name means one thing inside a procedure. Shadowing is a common source
of bugs that look correct, especially in code an agent wrote and a person
skimmed, so Coax refuses it.

## 3.2 Types

`coax0` has these scalar types:

| Type | What | Zero value |
| --- | --- | --- |
| `s8 s16 s32 s64` | Signed integers of 8, 16, 32 and 64 bits | `0` |
| `u8 u16 u32 u64` | Unsigned integers of the same widths | `0` |
| `int`, `uint` | Other names for `s64` and `u64` | `0` |
| `f32 f64` | IEEE floating point, 32 and 64 bits | `0.0` |
| `bool` | `true` or `false` | `false` |
| `string` | UTF-8 bytes: a pointer and a byte count | `""` |

Every type has one width everywhere. `int` is always 64 bits, on every target,
and `uint` is `u64`, not whatever C's `unsigned` happens to be. Code
that crosses into C (Chapter 16) writes the width it means.

```jai
#import "Basic";

MAX_PLAYERS :: 200;          // untyped: no type until it's used
HALF :: 0.5;

main :: () {
  players: u8 = MAX_PLAYERS; // 200 fits in u8
  score: s32 = -40_000;
  frames: u64 = 0xFFFF_FFFF_FFFF;
  ratio: f32 = HALF;
  wide := 1.0 / 3.0;         // an untyped float becomes f64
  letter := 'A';             // a character literal is an integer: 65

  print("% % % % % %\n", players, score, frames, ratio, wide, letter);
  print("% % % %\n", type_name(players), type_name(ratio), type_name(wide), type_name(letter));
  print("% % %\n", 100.0, 2.5e-7, 1e21);
}
```

```
200 -40000 281474976710655 0.5 0.3333333333333333 65
u8 f32 f64 int
100.0 2.5e-07 1e+21
```

Floats print the shortest text that reads back as the same value, with
`.0` when the value is whole, so a printed float is never ambiguous with
an integer.

Types the language plans but `coax0` doesn't have yet are refused by
name:

```
error[E0301]: type 'fixed32' is not supported by coax0 yet
```

That refusal is a promise. A program that compiles with `coax0` today
won't change meaning when `fixed32` arrives.

### Literals are untyped

```jai
a := 1_000_000;       // underscores are ignored
b := 0xff;            // hexadecimal
c := 0b1010;          // binary
d := 2.5e-7;          // a float has a fraction or an exponent
e := '\n';            // a character: the integer 10
```

A literal has no type until it's used. Where the context needs a type,
the literal takes it, if the value fits:

```jai
x: u8 = 200;          // fine
y: u8 = 300;          // error[E0350]: 300 does not fit in u8
z: s8 = -129;         // error[E0350]: -129 does not fit in s8
n: int = 2.5;         // error[E0350]: a float literal cannot become int
```

With nothing to decide it, an integer literal is `int` and a float
literal is `f64`. A constant made only of literals behaves the same way,
so `MAX_PLAYERS` above is a `u8` where it's used as one, and checked
there. The rule lets you write `0xFF` and `200` without casts, and it
never lets a number change silently: either it fits, or the compiler
says so. A literal larger than `u64` can hold is `E0104`.

A character literal is one ASCII character or escape in single quotes.
It is an integer holding that byte, so `'a'` fits a `u8` and compares
with a string's bytes (Chapter 6).

### No implicit conversions

```jai
x: int = "five";      // error[E0325]: 'x' is int but is initialized with string
b := 1 == true;       // error[E0311]: cannot compare integer literal with bool
if 1 { }              // error[E0331]: condition must be bool, found int
```

Once a value has a type, it keeps it. An `s32` and an `int` don't mix:

```
e.coax:1:45: error[E0309]: '+' needs two numbers of one type, found s32 and int
  main :: () { a: s32 = 1; b: int = 2; c := a + b; }
                                              ^
  fix: convert one: cast(s32) x
```

An `int` is never a `bool`, a `bool` is never an `int`, a `u8` doesn't
quietly widen, and nothing quietly becomes a string. In C, mixing widths
and signedness follows promotion rules that most people (and most
models) get wrong under pressure. In Coax the conversion is on the page,
or it doesn't happen.

### Casts

```jai
#import "Basic";

main :: () {
  small: u8 = 200;
  total := cast(s32) small + 1000;      // widen first, then add as s32
  print("% %\n", total, type_name(total));

  print("% %\n", cast(int) 7.9, cast(int) -7.9);   // toward zero
  print("% %\n", cast(f64) 3, cast(f32) 16777217); // nearest float

  print("%\n", cast,trunc(u8) 300);     // keep the low 8 bits: 44
  print("%\n", cast(u8) total);         // 1200 doesn't fit: this traps
}
```

```
1200 s32
7 -7
3.0 16777216.0
44
examples/casts.coax:12: cast out of range
```

- **`cast(T) x`** converts between numbers. It checks that the value
  fits, in debug **and** release builds, and stops the program if it
  doesn't. A float becomes an integer by truncating toward zero; NaN or
  an out-of-range float traps too.
- **`cast,trunc(T) x`** converts between integer types and keeps the low
  bits. It's for hashing and packing, where dropping bits is the point,
  and the word `trunc` says so at the call site.
- **`cast` binds to the value right after it.** `cast(s32) small + 1000`
  casts `small`, then adds. Parenthesize to cast a larger expression.

A cast that can lose information is checked because a wrong narrow
conversion is a silent bug that shows up far from its cause. The check
costs a compare, and the release build keeps it.

## 3.3 Procedures

```jai
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

```jai
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
- **Assignment** is `=`. The compound forms `+= -= *= /=` work on any
  number, and `%= &= |= ^= <<= >>=` on integers.
- **Operators**, from loosest to tightest binding:

  | Precedence | Operators | Operands |
  | --- | --- | --- |
  | 1 | `\|\|` | bool |
  | 2 | `&&` | bool |
  | 3 | `== != < <= > >=` | `==` and `!=` on scalars, enums and strings; the rest on numbers |
  | 4 | `+ - \| ^` | numbers; `\|` and `^` on integers |
  | 5 | `* / % & << >>` | numbers; `%`, `&` and the shifts on integers |
  | 6 | unary `-`, `!`, `cast` | signed integers and floats, bool, numbers |

- **Both operands have one type.** `+` takes two `u8`s or two `f64`s,
  never one of each (`E0309`). Unsigned integers have no unary `-`
  (`E0307`).
- **Comparisons don't chain.** `1 < x < 10` is an error (`E0203`) with
  the fix "split it with `&&`". In C it compiles and means something
  else.
- **`==` compares strings by content.** Structs and arrays have no `==`
  (`E0311`): you compare the fields you mean.
- **Division truncates toward zero**, as in C: `-7 / 2` is `-3`, and
  `-7 % 2` is `-1`. Dividing by zero stops the program in every build.

### Bit operations

The bit operators bind the way Go's do: `&` and the shifts with `*`,
`|` and `^` with `+`, and all of them tighter than comparison.

```jai
#import "Basic";

// Pack a color into one u32: 0xRRGGBB.
pack :: (r: u32, g: u32, b: u32) -> u32 {
  return r << 16 | g << 8 | b;          // << binds tighter than |, as in Go
}

ones :: (bits: u32) -> int {
  count := 0;
  rest := bits;
  while rest != 0 {
    if rest & 1 == 1 { count += 1; }    // (rest & 1) == 1, unlike C
    rest >>= 1;
  }
  return count;
}

main :: () {
  color := pack(0xFF, 0x80, 0x10);
  print("% % %\n", color, color >> 8 & 0xFF, ones(color));
  print("% %\n", color ^ 0xFFFFFF, 1 << 40);
}
```

```
16744464 128 10
32751 1099511627776
```

In C, `x & 1 == 1` parses as `x & (1 == 1)`, a classic bug that
compiles. In Coax it means what it looks like. Bit operators take
integers of one type, never floats (`E0370`).

A shift by a negative amount, or by the type's width or more, stops the
program in every build. `<<` drops the bits shifted off the top; that is
the one integer operation that discards bits without a trap or a
`cast,trunc`, because discarding them is what a left shift is for.

## 3.5 Loops

`while` is enough for any loop, but most loops count or walk a
collection, and `for` says that directly:

```jai
#import "Basic";

// Count the vowels in a string, byte by byte.
vowels :: (text: string) -> int {
  count := 0;
  for c: text {
    if c == 'a' || c == 'e' || c == 'i' || c == 'o' || c == 'u' { count += 1; }
  }
  return count;
}

main :: () {
  for i: 0..<5 { print("% ", i * i); }
  print("\n");

  for i: 1..<100 {
    if i % 7 != 0 { continue; }    // skip to the next i
    if i > 30 { break; }           // leave the loop
    print("% ", i);
  }
  print("\n");

  word := "coaxial";
  for c, i: word {
    if c == 'x' { print("x is byte % of %\n", i, word.count); }
  }
  n := vowels(word);
  print("% has % %\n", word, n, ifx n == 1 then "vowel" else "vowels");
}
```

```
0 1 4 9 16 
7 14 21 28 
x is byte 3 of 7
coaxial has 4 vowels
```

- **`for i: lo..<hi`** counts from `lo` up to, but not including, `hi`.
  Ranges are always half-open, so `0..<n` runs `n` times and an empty
  range runs zero times. A bare `..` is an error (`E0216`), because "is
  the end included?" is a question the syntax should answer.
- **`for x: collection`** walks a string, an array or a view (Chapter 6).
  A string gives its bytes as `u8`. **`for x, i: collection`** adds the
  index, an `int`.
- **Loop variables can't be assigned** (`E0359`). Use a `while` loop when
  you need to move the counter yourself.
- **`break`** leaves the innermost loop and **`continue`** starts its next
  iteration. Outside a loop either one is `E0360`.
- **`ifx condition then a else b`** is the expression form of `if`. Both
  branches have one type (`E0365`), and only the chosen one runs.

*Design:* loop labels for `break` and `continue`, reverse ranges, and
exhaustive `switch`. `coax0` reserves `switch` and refuses it.

## 3.6 print

`print` is how a `coax0` program talks:

```jai
print("% has % lives\n", name, lives);   // each % takes the next value
print("100%% sure\n");                   // %% prints a %
```

`%` prints any value: integers of every width, floats (shortest
round-trip form), `bool` as `true` or `false`, strings, and the structs,
enums and arrays of Chapters 5 and 6 (`Vec2.{x = 1.0, y = 2.0}`,
`[1, 2]`, an enum by its member name). There are no width or precision
specifiers yet. What makes it Coax rather than C is that the format is checked when you compile:

```
error[E0320]: format has 2 '%' slots but print was given 1 value
  fix: write '%%' for a literal percent sign
```

So the format has to be a string literal. A format built at run time
can't be checked, and the compiler says so (`E0319`). In C, a mismatched
`printf` is undefined behavior. In Jai it's a surprise at run time. In
Coax, a print that compiles prints what it says.

## 3.7 Comments

```jai
// to the end of the line
/* a block, which /* can nest */ inside another */
```

Nesting means you can comment out code that already contains block
comments. *Design:* `///` doc comments that attach to the next
declaration.

## 3.8 What you've seen

You've now used the scalar half of `coax0`:

- the `#import` directive and the `Basic` module;
- procedures, parameters, returns, recursion and declarations in any order;
- top-level constants, computed while compiling;
- sized integers, `f32` and `f64`, `bool` and `string`;
- untyped literals and constants, and character literals;
- the four declaration forms, and no shadowing;
- `if`, `else if`, `else`, `while`, `for`, `break`, `continue` and `ifx`;
- arithmetic, bit operations, comparison and logic, with no implicit
  conversions;
- `cast`, checked in every build, and `cast,trunc`;
- a checked `print`;
- overflow checks in debug builds and wrapping in release builds.

Chapter 4 is about what the compiler says when you get something wrong.
