# 3. Common concepts

This chapter covers the ideas that show up in almost every Coax program:
variables and constants, the basic types, how numbers convert from one
type to another, operators, procedures, comments and control flow. None
of them is unique to Coax, and if you've written C, Go or Jai you'll
recognize most of the syntax. What's worth slowing down for is where
Coax makes a different choice: there is no shadowing, no implicit
conversion, a cast that loses information stops the program, and
integer overflow is caught. Those choices come up again and again in
later chapters, so we'll meet each one here with a small program and,
where it helps, a program the compiler refuses.

You used several of these pieces in the project in Chapter 2. Here we'll
go through them one at a time.

> Note: Every program in this chapter that runs is in `examples/` next
> to the book source, named with a `ch03_` prefix (`variables.coax` is
> `examples/ch03_variables.coax`), and every output shown is what
> `coax0` 0.10.0 prints. A few
> things the language plans aren't in `coax0` yet: local constants,
> global variables, `switch`, loop labels and doc comments. Each one is
> marked where it comes up, along with what the compiler says if you
> try it.

## Variables

Let's start with a variable that changes. Make a file called
`variables.coax`:

Filename: variables.coax

```jai
#import "Basic";

main :: () {
  lives := 3;
  print("Kiki starts with % lives\n", lives);
  lives = lives - 1;
  print("Kiki falls in the pond and has % left\n", lives);
  lives += 5;
  print("Kiki finds a bag of hearts: %\n", lives);
}
```

*Listing 3-1: Declaring a variable and assigning to it*

```
$ coax run variables.coax
Kiki starts with 3 lives
Kiki falls in the pond and has 2 left
Kiki finds a bag of hearts: 7
```

The line `lives := 3;` declares a new variable called `lives` and gives
it the value `3`. The `:=` is two pieces run together: `:` says "this is
a declaration", and `=` gives the value. We didn't write a type, so the
compiler takes it from the value. `3` is a whole number, so `lives` is
an `int`.

`lives = lives - 1;` is an assignment. It has `=` without the `:`,
because `lives` already exists. `lives += 5;` is the compound form of
`lives = lives + 5;`.

Local variables in Coax can always be assigned. If you're coming from
Rust, there's no `mut` to add; if you're coming from C, there's no
`const` to remember. What Coax checks instead is the type: once `lives`
is an `int`, it stays an `int`. Assigning `lives = "three";` is error
`E0327`, "the target is int, assigned string".

### The declaration forms

`name := value` is the form you'll write most, but there are four ways
to declare a variable, and they're all the same declaration with
different parts left out. The full form is `name: type = value`:

Filename: declarations.coax

```jai
#import "Basic";

main :: () {
  lives := 3;               // the type comes from the value: int
  name: string = "Kiki";    // the type is written out
  score: int;               // zero
  ready: bool;              // false
  title: string;            // the empty string
  buffer: int = ---;        // uninitialized: we promise to write it first

  buffer = lives * 100;
  score += buffer;
  print("% has % lives and % points\n", name, lives, score);
  print("ready: %, title: \"%\"\n", ready, title);
}
```

*Listing 3-2: The four declaration forms*

```
$ coax run declarations.coax
Kiki has 3 lives and 300 points
ready: false, title: ""
```

Let's go through them:

- `lives := 3;` leaves out the type, and the compiler infers it.
- `name: string = "Kiki";` writes the type too. The compiler checks that
  the value really is a `string`, and it won't convert anything to make
  it fit.
- `score: int;` leaves out the value. A variable with no value is set to
  zero: `0` for numbers, `0.0` for floats, `false` for `bool`, and the
  empty string for `string`. In Coax, a variable you didn't initialize
  holds zero, never whatever happened to be in that memory before.
- `buffer: int = ---;` opts out of the zeroing. The three dashes mean
  "uninitialized". Use it for the rare large buffer where zeroing costs
  time you've actually measured. It's a promise that you'll write the
  variable before you read it, and the compiler doesn't check that
  promise at run time: a `---` variable read too early holds garbage.

Notice that `score: int;` is the long form `score: int = 0;` with the
value left off, and `lives := 3;` is `lives: int = 3;` with the type
left off. You're never learning a second syntax, only which parts you
can skip.

### No shadowing

Many languages let you declare a new variable with the same name as one
that's already in scope. The new one *shadows* the old one until its
block ends. Let's try it in Coax:

Filename: lives.coax

```jai
#import "Basic";

main :: () {
  lives := 3;
  if lives > 0 {
    lives := lives - 1;
    print("% left\n", lives);
  }
}
```

```
$ coax check lives.coax
lives.coax:6:5: error[E0303]: 'lives' is already declared at line 4; Coax does not allow shadowing
      lives := lives - 1;
      ^
  fix: pick another name
1 error
```

The compiler points at the second declaration and tells us where the
first one is. Coax refuses shadowing because it's a common source of
bugs that read correctly. In the program above, did we mean to change
`lives`, or to compute a temporary? Someone skimming the code (or an
agent editing it) can't tell, and after the block ends `lives` is
silently still `3`. Inside a procedure, a name means one thing.

There are two fixes, depending on what we meant. If we wanted to change
`lives`, we drop the `:` and assign:

```jai
  if lives > 0 {
    lives = lives - 1;
    print("% left\n", lives);
  }
```

If we wanted a separate value, we give it its own name, such as
`remaining := lives - 1;`.

The rule is about names that are *in scope* at the same time. Two
sibling blocks can each declare their own `i`, because the first `i` is
gone by the time the second block starts. And a variable declared inside
a block is gone after the block's `}`: using it later is `E0306`, "'x'
is not declared".

## Constants

A constant is a name for a value that never changes. You declare one
with `::`, at the top level of a file:

Filename: constants.coax

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

*Listing 3-3: Top-level constants*

```
$ coax run constants.coax
Haunted House has 256 tiles; the answer is 42
```

A constant is computed while compiling. The C that `coax0` generates for
`TILES` contains `256`, not the multiplication. Constants can refer to
each other in any order: `LATE` uses `EARLY` before `EARLY` is declared,
and the compiler sorts it out. What it can't sort out is a cycle.
`A :: B; B :: A;` is `E0340`, "constant dependency cycle at 'A'". And a
constant can't be assigned: `TILES = 0;` is `E0344`, "constants cannot be
assigned".

You've already seen `::` before this chapter, on `main`. A procedure is
a constant too, one whose value is a block of code. That's why
`main :: () { ... }` and `TILES :: 256;` look alike. A local variable, a
constant, a procedure and (in Chapter 5) a type are all written as the
name, then what it is, then its value. Jai calls this *code maturation*:
a helper starts life as a few lines inside a procedure and moves to the
top level of the file when it's earned it, with almost no edits.

Constants can also be computed by calling a procedure while compiling,
with `#run`. That's the subject of Chapter 9.

### Where constants can go

Right now, constants live at the top level. Let's see what happens with
one inside a procedure:

Filename: players.coax

```jai
#import "Basic";

main :: () {
  MAX :: 4;
  print("%\n", MAX);
}
```

```
$ coax check players.coax
players.coax:4:7: error[E0206]: local constants, procedures and types are not supported by coax0 yet
    MAX :: 4;
        ^
  fix: move it to the top level
1 error
```

The fix says what to do: move `MAX :: 4;` above `main`. Notice the
wording "not supported by coax0 yet". Local constants are part of the
language and will arrive. Until then `coax0` refuses them by name, so no
program that compiles today can change meaning when they do.

> Note: The same goes for global variables. `score: int = 0;` at the top
> level of a file is `E0212`, "global variables are not supported by
> coax0 yet". Top-level declarations are constants and procedures. Data
> that lives for the whole program goes in a variable in `main`, passed
> to the procedures that need it.

## Data types

Every value in Coax has a type, and the compiler knows every type while
it compiles. This section covers the *scalar* types, the ones that hold
a single value. Structs and enums are Chapter 5, and arrays, views and
the inside of strings are Chapter 6.

| Type | What it holds | Zero value |
| --- | --- | --- |
| `s8 s16 s32 s64` | Signed integers of 8, 16, 32 and 64 bits | `0` |
| `u8 u16 u32 u64` | Unsigned integers of the same widths | `0` |
| `int`, `uint` | Other names for `s64` and `u64` | `0` |
| `f32 f64` | IEEE floating point, 32 and 64 bits | `0.0` |
| `bool` | `true` or `false` | `false` |
| `string` | UTF-8 bytes: a pointer and a byte count | `""` |

Every type has one width on every target. `int` is always 64 bits, and
`uint` is always `u64`, not whatever C's `unsigned` happens to be on
this machine. When Coax code talks to C (Chapter 17), it writes the
width it means.

Here are all of them in one program, along with `type_name`, which
gives the name of an expression's type as a string. `type_name` is
handy when you're not sure what the compiler inferred.

Filename: scalars.coax

```jai
#import "Basic";

MAX_PLAYERS :: 200;          // untyped until it's used
HALF :: 0.5;

main :: () {
  players: u8 = MAX_PLAYERS; // 200 fits in a u8
  score: s32 = -40_000;
  frames: u64 = 0xFFFF_FFFF_FFFF;
  ratio: f32 = HALF;
  wide := 1.0 / 3.0;         // an untyped float with nothing else to go on: f64
  narrow := 1.0f32 / 3.0;    // a typed literal: f32
  letter := 'A';             // a character literal is an integer: 65
  ok := players > 100;

  print("% % % % % % % %\n", players, score, frames, ratio, wide, narrow, letter, ok);
  print("% % % % % %\n", type_name(players), type_name(ratio), type_name(wide),
        type_name(narrow), type_name(letter), type_name(ok));
  print("% % %\n", 100.0, 2.5e-7, 1e21);
}
```

*Listing 3-4: The scalar types*

```
$ coax run scalars.coax
200 -40000 281474976710655 0.5 0.3333333333333333 0.33333334 65 true
u8 f32 f64 f32 int bool
100.0 2.5e-07 1e+21
```

Look at the third value on the first line and the third on the last.
Floats print the shortest text that reads back as exactly the same
value, so `1.0 / 3.0` as an `f64` prints sixteen 3s and as an `f32`
prints eight. A whole float always prints with `.0`, as `100.0` does, so
a printed float can't be mistaken for an integer.

### Integers

An integer type is signed (`s`) or unsigned (`u`), followed by its width
in bits. A signed type of `n` bits holds −2<sup>n−1</sup> to
2<sup>n−1</sup> − 1, so `s8` holds −128 to 127. An unsigned type holds 0
to 2<sup>n</sup> − 1, so `u8` holds 0 to 255. When you don't have a
reason to pick something else, `int` is the right default.

Integer literals can be written in a few ways:

| Literal | Example | Value |
| --- | --- | --- |
| Decimal | `1_000_000` | 1000000 |
| Hexadecimal | `0xff` | 255 |
| Binary | `0b1010` | 10 |
| Character | `'A'`, `'\n'` | 65, 10 |

Underscores are ignored, so you can group digits however reads best. A
character literal is one ASCII character, or an escape such as `'\n'`,
in single quotes. It's an integer holding that byte, which means `'a'`
fits in a `u8` and can be compared with a string's bytes (Chapter 6).

### Floating-point numbers

`f32` and `f64` are IEEE single and double precision. A float literal
has a fraction (`1.5`), an exponent (`2e-3`), or both. `1.` isn't a
float literal; you write `1.0`. That keeps `0..<n` unambiguous as a
range.

`coax0` doesn't fuse a multiply and an add into one instruction behind
your back, so each operation rounds once, as written. That matters when
two machines need the same answer, which is Chapter 13's subject.

### Booleans and strings

A `bool` is `true` or `false`. It's what comparisons produce and what
`if` and `while` need.

A `string` is a run of bytes, stored as a pointer and a count. By
convention the bytes are UTF-8 text. String literals support the
escapes `\n`, `\t`, `\r`, `\0`, `\\` and `\"`. Chapter 6 shows how to
index and slice a string.

### Literals are untyped

Look again at `MAX_PLAYERS :: 200;` and `players: u8 = MAX_PLAYERS;` in
Listing 3-4. We never said what type `MAX_PLAYERS` is. A literal in Coax
has no type until it's used, and where it's used decides. Put `200`
where a `u8` is needed and it becomes a `u8`, as long as it fits. A
constant made only of literals behaves the same way.

When nothing decides, an integer literal becomes `int` and a float
literal becomes `f64`. That's what happened to `wide` and `letter`.

This rule is why you can write `x: u8 = 200;` and `mask: u32 = 0xFF;`
with no casts. It never lets a number change silently, though. Let's
give a literal something too small:

Filename: fit.coax

```jai
#import "Basic";

main :: () {
  y: u8 = 300;
  z: s8 = -129;
  n: int = 2.5;
  print("% % %\n", y, z, n);
}
```

```
$ coax check fit.coax
fit.coax:3:11: error[E0350]: 300 does not fit in u8
    y: u8 = 300;
            ^
  fix: use a wider type, or cast,trunc(u8) to keep the low bits
fit.coax:4:11: error[E0350]: -129 does not fit in s8
    z: s8 = -129;
            ^
  fix: use a wider type, or cast,trunc(s8) to keep the low bits
fit.coax:5:12: error[E0350]: a float literal cannot become int
    n: int = 2.5;
             ^
  fix: write an integer, or cast(int) a float value
3 errors
```

Each literal either fits or the compiler says so. A literal too large
for any integer, like `99999999999999999999999`, is `E0104`, "integer
literal does not fit in 64 bits".

Sometimes you want a float literal to have a type on its own, without a
declaration to give it one. A suffix does that: `1.0f32` is an `f32` and
`1.0f64` is an `f64`. We used it for `narrow` in Listing 3-4.

### No implicit conversions

Once a value has a type, it keeps it. Coax never converts a value to
another type unless you ask. Here's where that tends to surprise
people:

Filename: speed.coax

```jai
#import "Basic";

main :: () {
  speed: f32 = 4.5;
  boost := 2.0;
  print("%\n", speed * boost);
}
```

```
$ coax check speed.coax
speed.coax:6:22: error[E0309]: '*' needs two numbers of one type, found f32 and f64
    print("%\n", speed * boost);
                       ^
  fix: convert one: cast(f32) x
1 error
```

The `2.0` was untyped, but `boost := 2.0;` had nothing to tell it what to
become, so `boost` became an `f64`. By the time we multiply, it's an
`f64` meeting an `f32`, and `*` needs two numbers of one type. The fix
line suggests a cast (the `x` stands for whichever operand you convert),
but here the cleaner fix is to say what `boost` is when we declare it:

```jai
  speed: f32 = 4.5;
  boost: f32 = 2.0;
  print("%\n", speed * boost);
```

```
$ coax run speed.coax
9.0
```

`boost := 2.0f32;` would work too. Writing `speed * 2.0` directly would
also have worked, because there the literal is still untyped when it
meets `speed`, and takes `f32` from it.

The same rule covers every pairing. An `s32` and an `int` don't mix. A
`u8` doesn't widen quietly to a `u32`. An `int` is never a `bool` (so
`if count { }` is `E0331`, "condition must be bool, found int", with the
fix "compare it: x != 0"), a `bool` is never an `int`, and nothing turns
into a string by accident. Adding two strings with `+` is `E0309` too,
with the fix "build strings with print's % formatting".

In C, mixing widths and signedness follows promotion rules that most
people (and most models) get wrong under pressure. In Coax the
conversion is on the page, or it doesn't happen.

## Converting between number types

When you do want a conversion, you write a cast:

Filename: conversions.coax

```jai
#import "Basic";

main :: () {
  small: u8 = 200;
  total := cast(s32) small + 1000;    // widen first, then add as s32
  print("% %\n", total, type_name(total));

  print("% %\n", cast(int) 7.9, cast(int) -7.9);    // toward zero
  print("% %\n", f64(3), f32(16777217));             // the same, as a call

  print("%\n", cast,trunc(u8) 300);   // keep the low 8 bits: 44
  print("%\n", cast(u8) total);       // 1200 doesn't fit: this stops the program
  print("not reached\n");
}
```

*Listing 3-5: Checked and truncating conversions*

```
$ coax run conversions.coax
1200 s32
7 -7
3.0 16777216.0
44
conversions.coax:12: cast out of range
```

`cast(T) x` converts a number to type `T`. It checks that the value
fits, in debug builds and in release builds, and stops the program with
the file and line if it doesn't. That's what happened on the last
`print`: 1200 doesn't fit in a `u8`, so the program stopped before
printing anything and `not reached` never appeared. A float becomes an
integer by dropping its fraction (toward zero, so `-7.9` becomes `-7`),
and a float that's NaN or out of range stops the program too. An integer
or an `f64` becomes the nearest float (an `f64` too large for an `f32`
stops the program), which is why `16777217`, one past
the last integer an `f32` can hold exactly, prints as `16777216.0`.

`f64(3)` and `int(x)` are the same conversion written as a call, one for
every number type. You'll see this form a lot in real code because it
reads well inside arithmetic, like `f64(i) * 0.25`.

`cast,trunc(T) x` is the other kind of conversion: between integer
types, keeping only the low bits. 300 is `0x12C`, and its low 8 bits are
`0x2C`, which is 44. Truncation is what you want when hashing or packing
bits, where dropping the top is the point, and the word `trunc` makes
that visible where it happens.

`cast` applies to the value right after it. `cast(s32) small + 1000`
casts `small`, then adds 1000 as an `s32`. Put parentheses around a
larger expression to cast all of it.

Why check every narrowing cast, even in release builds? A conversion
that loses information silently is a bug that shows up far from its
cause, often as a wrong number several steps later. The check costs one
comparison. Coax keeps it.

## Operators

Here are the arithmetic and logic operators at work:

Filename: arithmetic.coax

```jai
#import "Basic";

main :: () {
  sum := 5 + 10;
  difference := 95.5 - 4.3;
  product := 4 * 30;
  quotient := 56.7 / 32.2;
  truncated := -7 / 2;     // integer division rounds toward zero
  remainder := -7 % 2;     // and the remainder takes the dividend's sign

  print("% % % %\n", sum, difference, product, quotient);
  print("% %\n", truncated, remainder);

  hungry := true;
  tired := false;
  print("% % %\n", hungry && tired, hungry || tired, !tired);
}
```

*Listing 3-6: Arithmetic and logic*

```
$ coax run arithmetic.coax
15 91.2 120 1.7608695652173911
-3 -1
false true true
```

`+ - * /` work on integers and floats. `%` (remainder) works on integers
only. Integer division truncates toward zero, as in C, so `-7 / 2` is
`-3` and `-7 % 2` is `-1`. Unary `-` works on signed integers and
floats; on an unsigned integer it's `E0307`, "'-' needs a signed integer
or a float, found u8".

`&&` and `||` take two `bool`s and stop early: in `a && b`, if `a` is
`false`, `b` is never evaluated. `!` flips a `bool`.

Comparisons (`== != < <= > >=`) produce a `bool`. `==` and `!=` work on
numbers, `bool`s, enums and strings, and strings compare by content.
Structs and arrays have no `==`; you compare the fields you mean.

One thing you might try from math class:

Filename: between.coax

```jai
#import "Basic";

main :: () {
  x := 5;
  if 1 < x < 10 { print("in\n"); }
}
```

```
$ coax check between.coax
between.coax:5:12: error[E0203]: comparisons cannot be chained
    if 1 < x < 10 { print("in\n"); }
             ^
  fix: split it with '&&'
1 error
```

In C this compiles and means `(1 < x) < 10`, which compares a 0 or 1
with 10 and is always true. Coax refuses it, and the fix is
`1 < x && x < 10`.

### Overflow

What happens when an integer gets too big? Let's find out:

Filename: overflow.coax

```jai
#import "Basic";

main :: () {
  big := 9223372036854775807;   // the largest int
  print("before: %\n", big);
  big += 1;
  print("after: %\n", big);
}
```

*Listing 3-7: Adding one to the largest `int`*

```
$ coax run overflow.coax
before: 9223372036854775807
overflow.coax:6: integer overflow in '+'
```

In a normal (debug) build, integer overflow stops the program and names
the line and the operator. That applies to signed and unsigned types
alike: a `u8` at 255 plus 1 stops, and so does a `u32` 3 minus 5. Now
build it for release:

```
$ coax run overflow.coax --release
before: 9223372036854775807
after: -9223372036854775808
```

With `--release`, `+ - *` wrap around instead, the way the hardware
does. So debug builds catch the mistake while you're developing, and
release builds don't pay for the check on every addition. A few things
stop the program in both modes, because there is no sensible answer to
wrap to: dividing by zero (or taking `%` of zero), dividing the smallest
`int` by `-1`, and shifting by a negative amount or by the type's width
or more.

### Bit operations

`&`, `|`, `^`, `<<` and `>>` work on integers of one type. They're for
flags, masks and packed data. This program packs a color into a `u32`
and counts its set bits:

Filename: bits.coax

```jai
#import "Basic";

// Pack a color into one u32: 0xRRGGBB.
pack :: (r: u32, g: u32, b: u32) -> u32 {
  return r << 16 | g << 8 | b;          // << binds tighter than |
}

// How many bits are set?
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

*Listing 3-8: Packing and counting bits*

```
$ coax run bits.coax
16744464 128 10
32751 1099511627776
```

The interesting line is `if rest & 1 == 1`. In C, `==` binds tighter than
`&`, so that parses as `rest & (1 == 1)`, a classic bug that compiles
and usually works by accident. Coax follows Go here: `&` and the shifts
bind like `*`, and `|` and `^` bind like `+`, so they all bind tighter
than comparison and the line means what it looks like.

`<<` drops the bits shifted off the top. That's the one integer
operation that discards bits with no check and no `cast,trunc`, because
discarding them is what a left shift is for. Bit operators on floats are
`E0370`.

### Precedence

From loosest to tightest:

| Level | Operators | Operands |
| --- | --- | --- |
| 1 | `\|\|` | `bool` |
| 2 | `&&` | `bool` |
| 3 | `== != < <= > >=` | `==` and `!=` on scalars, enums and strings; the rest on numbers |
| 4 | `+ - \| ^` | numbers; `\|` and `^` on integers |
| 5 | `* / % & << >>` | numbers; `%`, `&` and shifts on integers |
| 6 | unary `-`, `!`, `cast` | signed numbers, `bool`, numbers |

Operands are evaluated left to right, and so are a call's arguments.
C leaves that order unspecified; Coax makes sure the generated C keeps
it.

Assignment has compound forms for every binary operator: `+= -= *= /=`
on numbers and `%= &= |= ^= <<= >>=` on integers.

## Procedures

You've been writing one procedure, `main`, since Chapter 1. Here are a
few more:

Filename: procedures.coax

```jai
#import "Basic";

main :: () {
  print("gcd(1071, 462) = %\n", gcd(1071, 462));
  print("7 is % and 10 is %\n", parity(7), parity(10));
  print("2^10 = %\n", power(2, 10));
  greet("Kiki");
}

// Declaration order doesn't matter: main can call these before they appear.
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

greet :: (name: string) {
  print("Hello, %!\n", name);
}
```

*Listing 3-9: Procedures with parameters and return values*

```
$ coax run procedures.coax
gcd(1071, 462) = 21
7 is odd and 10 is even
2^10 = 1024
Hello, Kiki!
```

A procedure is a parameter list in parentheses, then `->` and a return
type, then a body in braces. Each parameter is `name: type`. A procedure
with no `->`, like `greet`, returns nothing.

`main` calls `gcd` on line 4, but `gcd` isn't declared until line 11.
That's fine. Coax reads the whole file before checking it, so order
doesn't matter, and there are no forward declarations or header files to
keep in sync. `gcd` also calls itself; recursion uses the C stack,
because `coax0` compiles to C.

A call checks everything about its arguments. `gcd(1071)` is `E0316`,
"'gcd' takes 2 arguments, given 1", and passing a `string` where an
`int` goes is `E0317`, "argument 1 of 'gcd' is int, given string".

### Parameters can't be assigned

A parameter is the value the caller passed, and inside the procedure it
stays that value. Writing `n -= 1;` in `parity` is `E0344`, "parameters
cannot be assigned", with the fix "copy it into a local variable". That's
what `power` does with `result` and `i`, and what `ones` in Listing 3-8
does with `rest := bits;`.

### Results must be used

If a procedure returns a value, the caller has to do something with it.
`power(2, 10);` on a line by itself is `E0323`, "the result of 'power'
(int) is not used". A dropped result is usually a mistake, such as
calling a procedure that computes a new value and thinking it changed
something in place. If a result really doesn't matter, the procedure
shouldn't return one.

### Every path must return

Here's a program that finds the first number whose square is over a
limit. It loops forever until it finds one:

Filename: squares.coax

```jai
#import "Basic";

first_square_over :: (limit: int) -> int {
  n := 1;
  while true {
    if n * n > limit { return n; }
    n += 1;
  }
}

main :: () {
  print("%\n", first_square_over(50));
}
```

```
$ coax check squares.coax
squares.coax:3:1: error[E0338]: 'first_square_over' can reach its end without returning int
  first_square_over :: (limit: int) -> int {
  ^
  fix: add a return at the end
1 error
```

We know the loop only ends through the `return`, but the compiler
doesn't reason about whether a `while` condition can ever be false. It
checks the shape of the code: after the `while` there's no `return`, so
as far as it can see, the procedure can reach its closing brace without
a value. The better fix is to make the loop's condition say what we're
waiting for, and return after it:

Filename: squares.coax

```jai
#import "Basic";

// The first n whose square is over the limit.
first_square_over :: (limit: int) -> int {
  n := 1;
  while n * n <= limit {
    n += 1;
  }
  return n;
}

main :: () {
  print("% % %\n", first_square_over(50), first_square_over(100), first_square_over(0));
}
```

*Listing 3-10: A loop whose condition says when it ends*

```
$ coax run squares.coax
8 11 1
```

Now every path ends in a `return`, and the loop's condition reads as
the question we're asking.

### Built-in math

A handful of math procedures are built in, with no import needed:
`sqrt`, `sin`, `cos`, `tan`, `exp`, `log`, `floor`, `abs` and `pow`.
They take floats of one type and return that type. `len(s)` is a
string's byte count.

Filename: math.coax

```jai
#import "Basic";

main :: () {
  side := 3.0;
  diagonal := sqrt(side * side + side * side);
  print("a %-metre square has a %-metre diagonal\n", side, diagonal);
  print("% % %\n", floor(-2.5), abs(-4.0), pow(2.0, 0.5));
  print("% %\n", sqrt(2.0f32), len("coaxial"));
}
```

*Listing 3-11: Math built-ins*

```
$ coax run math.coax
a 3.0-metre square has a 4.242640687119285-metre diagonal
-3.0 4.0 1.4142135623730951
1.4142135 7
```

`sqrt(2.0f32)` is computed in `f32`, so it prints fewer digits. These
call the C library's `sqrt` or `sqrtf`, and so on, so their last digit
isn't guaranteed to match across platforms.

## Comments

You've seen `//` comments, which run to the end of the line. Coax also
has block comments, and they nest:

```jai
// to the end of the line
/* a block, which /* can nest */ inside another */
```

Nesting means you can comment out a stretch of code that already has a
block comment in it, and the outer comment ends where you expect.

> Note: `///` doc comments that attach to the next declaration are
> planned. Today `coax0` treats `///` as an ordinary `//` comment.

## Control flow

Programs decide what to run and repeat things. Coax has `if` for
deciding, `ifx` for choosing between two values, and `while` and `for`
for repeating.

### `if` and `else`

Filename: branches.coax

```jai
#import "Basic";

describe :: (temperature: int) -> string {
  if temperature < 0 {
    return "freezing";
  } else if temperature < 15 {
    return "cold";
  } else if temperature < 25 {
    return "mild";
  } else {
    return "hot";
  }
}

main :: () {
  print("% % % %\n", describe(-5), describe(10), describe(20), describe(31));

  apples := 1;
  print("% %\n", apples, ifx apples == 1 then "apple" else "apples");
  apples = 4;
  print("% %\n", apples, ifx apples == 1 then "apple" else "apples");
}
```

*Listing 3-12: An `if` chain and `ifx`*

```
$ coax run branches.coax
freezing cold mild hot
1 apple
4 apples
```

An `if` takes a condition with no parentheses around it, and the body is
always a block in braces, even for one statement. That rules out the C
bug where a second, indented line looks like it's part of the `if` but
isn't. `else if` adds another condition, checked only if the earlier ones
were false, and `else` ends the chain. The conditions are checked in
order and only the first true one runs, so `describe(10)` is `"cold"`
even though `10 < 25` is true too.

The condition must be a `bool`. `if apples { }` is `E0331`: Coax doesn't
treat zero as false. Write `if apples != 0 { }`.

Every path through `describe` ends in a `return`, including the `else`,
so the compiler is satisfied that it always returns a `string`.

### Choosing a value with `ifx`

In `if`, the branches are blocks of statements. When you want a *value*
that depends on a condition, use `ifx`: `ifx condition then a else b`.
Only the chosen branch is evaluated, and both branches must have one
type. If you reach for the `if` you know from Rust or Kotlin, the
compiler stops you:

Filename: label.coax

```jai
#import "Basic";

main :: () {
  apples := 4;
  word := if apples == 1 { "apple" } else { "apples" };
  print("% %\n", apples, word);
}
```

```
$ coax check label.coax
label.coax:5:11: error[E0204]: 'if' is reserved and not supported by coax0 yet
    word := if apples == 1 { "apple" } else { "apples" };
            ^
1 error
```

The message is a little indirect: it says `if` can't go where an
expression goes. The fix is `ifx`:

```jai
  word := ifx apples == 1 then "apple" else "apples";
```

Mixing types in the branches, like `ifx n == 1 then "one" else 2`, is
`E0365`, "ifx branches must have one type, found string and integer
literal".

### Repeating with `while`

`while` runs its block for as long as its condition is true. Here's a
search over the Collatz sequence, where each number is halved if it's
even and turned into `3n + 1` if it's odd, until it reaches 1:

Filename: collatz.coax

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

*Listing 3-13: Two `while` loops*

```
$ coax run collatz.coax
below 10000, 6171 takes the most steps: 261
```

The outer loop in `main` counts `n` from 1 to 9999, with the counter
declared before the loop and stepped at the end of the body. That
pattern is common enough that Coax has a loop for it.

Coax has no separate `loop` keyword for a loop with no condition. Write
`while true { }` and leave with `break` or `return`, but remember from
Listing 3-10 that a procedure still needs a `return` after the loop.

### Counting and walking with `for`

`for` loops over a range of numbers or over the items of a collection:

Filename: for.coax

```jai
#import "Basic";

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
  vowels := 0;
  for c, i: word {
    if c == 'x' { print("x is byte % of %\n", i, word.count); }
    if c == 'a' || c == 'e' || c == 'i' || c == 'o' || c == 'u' { vowels += 1; }
  }
  print("% has % %\n", word, vowels, ifx vowels == 1 then "vowel" else "vowels");

  for i: 3..<3 { print("never\n"); }
  print("done\n");
}
```

*Listing 3-14: `for` over ranges and a string, with `break` and `continue`*

```
$ coax run for.coax
0 1 4 9 16 
7 14 21 28 
x is byte 3 of 7
coaxial has 4 vowels
done
```

Let's take the pieces in order:

- `for i: 0..<5` counts from 0 up to, but not including, 5. The `..<`
  makes that visible: the range is *half-open*. `0..<n` runs exactly `n`
  times, and an empty range like `3..<3` runs zero times, which is why
  `never` never printed.
- `continue` skips the rest of the body and starts the next iteration.
  `break` leaves the loop entirely. Both apply to the innermost loop.
  Outside a loop, either one is `E0360`.
- `for c, i: word` walks a string. `c` is each byte, as a `u8`, and `i`
  is its index, an `int`. `for c: word` gives just the bytes. The same
  form walks arrays and views (Chapter 6).

Two mistakes come up a lot with `for`. The first is writing a closed
range:

Filename: inclusive.coax

```jai
#import "Basic";

main :: () {
  for i: 1..10 { print("% ", i); }
}
```

```
$ coax check inclusive.coax
inclusive.coax:4:11: error[E0216]: ranges are half-open: write '..<'
    for i: 1..10 { print("% ", i); }
            ^
  fix: for i: 0..<n { }
1 error
```

A bare `..` could mean "up to 10" or "up to and including 10", and
off-by-one bugs live in that gap. Coax has only the half-open form, so
the syntax answers the question. For 1 through 10, write `1..<11`.

The second is trying to move the loop variable yourself:

Filename: skip.coax

```jai
#import "Basic";

main :: () {
  for i: 0..<10 {
    if i == 3 { i += 1; }
    print("% ", i);
  }
}
```

```
$ coax check skip.coax
skip.coax:5:19: error[E0359]: a loop variable cannot be assigned
      if i == 3 { i += 1; }
                    ^
  fix: copy it into a local variable
1 error
```

A `for` loop owns its counter. To skip an item, use `continue`. When you
really need to move the counter by hand, write a `while` loop with its
own variable, as `power` does in Listing 3-9.

> Note: Loop labels (to `break` out of an outer loop), reverse ranges and
> an exhaustive `switch` are planned. `coax0` reserves `switch` and
> refuses it with `E0204`. Until labels arrive, put the nested loops in a
> procedure and `return` from it.

## Printing

`print` comes from the `Basic` module, which is why every program starts
with `#import "Basic";`. Leave the import out and the compiler says so
(`E0313`, "print is in module Basic, which is not imported").

Filename: print.coax

```jai
#import "Basic";

main :: () {
  name := "Kiki";
  lives := 3;
  print("% has % lives\n", name, lives);
  print("100%% sure\n");
  print("% % % %\n", 7, -2.5, true, 'A');
  print("no values at all\n");
}
```

*Listing 3-15: `print` formats*

```
$ coax run print.coax
Kiki has 3 lives
100% sure
7 -2.5 true 65
no values at all
```

Each `%` in the format takes the next value, whatever its type: integers
of every width, floats, `bool`s, strings, and the structs, enums and
arrays of Chapters 5 and 6. `%%` prints a percent sign. There are no
width or precision specifiers yet.

What makes `print` different from C's `printf` is that the compiler
checks it. Give it the wrong number of values and you hear about it
while compiling:

```
pct.coax:2:20: error[E0320]: format has 2 '%' slots but print was given 3 values
  main :: () { print("100%% sure, % and %\n", 1, true, 3); }
                     ^
  fix: write '%%' for a literal percent sign
1 error
```

For that to work, the format has to be a string literal the compiler can
see. Passing a format in a variable is `E0319`, "print's format must be
a string literal, so it can be checked". In C a mismatched `printf` is
undefined behavior; in Coax, a `print` that compiles prints what it says.

## In the real world

Here are four small procedures from `verse/home.coax`, the Coax code
that draws the OpenAgents iOS app's home screen. Nothing in them is new
to you now: `f64` parameters, `if` with `return`, a local in `smooth`,
and procedures calling procedures declared just above them. We've added
a `main` that tries `smooth`, the curve that eases an animation in and
out:

Filename: smooth.coax

```jai
#import "Basic";

// From verse/home.coax, the native home screen of the OpenAgents iOS app.
minimum :: (a: f64, b: f64) -> f64 { if a < b { return a; } return b; }
maximum :: (a: f64, b: f64) -> f64 { if a > b { return a; } return b; }
clamp :: (a: f64, low: f64, high: f64) -> f64 { return minimum(high, maximum(low, a)); }
smooth :: (a: f64, b: f64, x: f64) -> f64 { t := clamp((x-a)/(b-a),0.0,1.0); return t*t*(3.0-2.0*t); }

main :: () {
  for i: 0..<6 {
    x := f64(i) * 0.25;
    print("smooth(0, 1, %) = %\n", x, smooth(0.0, 1.0, x));
  }
}
```

*Listing 3-16: Easing procedures from the iOS home screen*

```
$ coax run smooth.coax
smooth(0, 1, 0.0) = 0.0
smooth(0, 1, 0.25) = 0.15625
smooth(0, 1, 0.5) = 0.5
smooth(0, 1, 0.75) = 0.84375
smooth(0, 1, 1.0) = 1.0
smooth(0, 1, 1.25) = 1.0
```

The curve starts slowly, speeds up through the middle and settles at
1.0, and `clamp` holds it there past the end. `f64(i)` converts the loop
counter, because `i` is an `int` and `0.25` needs a float to multiply.
The same file declares its sizes as top-level constants, such as
`NODES :: 256;`, and walks its buffers with the same `while` and `for`
loops you've seen in this chapter.

## Keeping both halves of a product

Coax0.16.0 adds `mul_wide` for two64-bit unsigned words:

```coax
product := mul_wide(cast(u64)18446744073709551615, cast(u64)2);
// product.hi == 1; product.lo == 18446744073709551614
```

The result is a `CoaxU64Product` with named `hi` and `lo` fields. Ordinary
multiplication retains its existing overflow rules. Use this primitive when
you need the exact two-word product, including during `#run`; it does not
introduce a general128-bit integer type. Arguments run once, left to right.
The native backend uses128-bit arithmetic and wasm32 combines four smaller
products. Platform code generation and cryptographic constant-time assurance
remain separate questions.

## Summary

That was a lot of ground. You can now declare variables in each of the
four forms, and you know that Coax zeroes what you don't initialize and
refuses shadowing. You've written top-level constants and seen that
local constants and globals aren't in `coax0` yet. You've met every
scalar type, untyped and typed literals, and the rule that nothing
converts unless you write `cast(T)`, `T(x)` or `cast,trunc(T)`, with
narrowing casts checked in every build. You've used the operators and
their Go-style precedence, and seen overflow stop a debug build and wrap
a release build. You've written procedures, used `if`, `ifx`, `while`
and `for` with `break` and `continue`, and printed with a format the
compiler checks.

Along the way you've seen the compiler refuse about a dozen programs.
Each time, the message said where, what and usually how to fix it. In
Chapter 4 we'll slow down and look at those messages properly: how
they're built, how far the compiler gets before it stops, and how to get
them as JSON for tools.
