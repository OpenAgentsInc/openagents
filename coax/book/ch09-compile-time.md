# 9. Compile time

Most languages draw a hard line between the program and the compiler.
The compiler reads your code, and the program runs later, somewhere
else. Coax moves that line: your ordinary procedures can run *while the
program is being compiled*, and what they compute becomes part of the
program. This is the idea from Jai that the rest of Coax leans on most.
Lookup tables get built before the program starts, facts about the code
get checked on every build, and later in the book, checks
([Chapter 10](ch10-writing-checks.md)), layouts and protocol code all
run on the same machinery.

In this chapter we'll take a small program that computes a table at run
time and move that work into the compiler one step at a time. Then we'll
look at the rules that keep compile-time code safe and predictable: what
it can't do, how much work it's allowed, and what happens when it goes
wrong.

> Note: `#run`, top-level `#assert`, `type_name` and `size_of` work in
> `coax0`, including `#run` results that are structs and arrays, and the
> examples in this chapter are checked. Code as values (`Code`,
> `#insert`, `#expand`), reflection beyond `type_name` and `size_of`,
> compiler plugins and `#if` are designed (spec §10) but not
> implemented; they're in the last part of the chapter.

## A table computed at run time

Let's say we're writing a game where a character levels up by earning
experience points. Each level costs a bit more than the one before.
Here's a first version that works it out with a loop:

Filename: examples/ch09_levels1.coax

```jai
#import "Basic";

MAX_LEVEL :: 10;

// Experience needed to reach a level: each level costs a little more than the last.
xp_for_level :: (level: int) -> int {
  total := 0;
  for n: 1..<level {
    total += 100 + n * n * 25;
  }
  return total;
}

main :: () {
  for level: 1..<MAX_LEVEL + 1 {
    print("level %: % xp\n", level, xp_for_level(level));
  }
}
```

*Listing 9-1: Experience needed for each level, computed when the program runs*

```shell
$ coax run examples/ch09_levels1.coax
level 1: 0 xp
level 2: 125 xp
level 3: 325 xp
level 4: 650 xp
level 5: 1150 xp
level 6: 1875 xp
level 7: 2875 xp
level 8: 4200 xp
level 9: 5900 xp
level 10: 8025 xp
```

This works, but every run of the program does the same arithmetic to
arrive at the same ten numbers. They depend on nothing but the source
code. A C programmer would compute them once by hand, or with a script,
and paste the results in as a table, and then the table and the formula
would slowly drift apart. Let's have the compiler do it instead.

## Running a procedure while compiling with `#run`

`#run` in front of an expression tells the compiler to evaluate it now,
during compilation:

Filename: examples/ch09_levels2.coax

```jai
#import "Basic";

MAX_LEVEL :: 10;
MAX_XP :: #run xp_for_level(MAX_LEVEL);

// Experience needed to reach a level: each level costs a little more than the last.
xp_for_level :: (level: int) -> int {
  total := 0;
  for n: 1..<level {
    total += 100 + n * n * 25;
  }
  return total;
}

main :: () {
  print("the top level, %, needs % xp\n", MAX_LEVEL, MAX_XP);
  print("level 4 needs % xp\n", xp_for_level(4));
}
```

*Listing 9-2: The top level's cost computed by the compiler*

```shell
$ coax run examples/ch09_levels2.coax
the top level, 10, needs 8025 xp
level 4 needs 650 xp
```

`MAX_XP` is a constant, like `MAX_LEVEL`, but its value comes from
calling `xp_for_level(10)` inside the compiler. Notice that
`xp_for_level` is an ordinary procedure. The same procedure runs at
compile time for `MAX_XP` and at run time for level 4 in `main`. We
didn't write it twice, and there's no separate language for "constant"
code the way C++ has `constexpr` rules or C has the preprocessor.

Notice too that `MAX_XP` is declared before `xp_for_level`. Order
doesn't matter at the top level of a file; the compiler works out what
depends on what.

We can see the difference in the C that `coax c` generates. Here's the
first line of `main`:

```shell
$ coax c examples/ch09_levels2.coax
...
  { coax_put(COAX_STR("the top level, ", 15)); coax_print_int(INT64_C(10)); coax_put(COAX_STR(", needs ", 8)); coax_print_int(INT64_C(8025)); coax_put(COAX_STR(" xp\n", 4)); }
...
```

`8025` is already there, as a literal. The program doesn't call
`xp_for_level` to get it. The interpreter that computed it belongs to
the compiler, and it never ships in your program. (`xp_for_level` is
still compiled to C as a normal procedure, because `main` calls it for
level 4.)

### Calls in constants need `#run`

What if we leave `#run` out?

```jai
MAX_XP :: xp_for_level(MAX_LEVEL);
```

```shell
$ coax check levels.coax
levels.coax:4:11: error[E0408]: a constant procedure call requires #run
  MAX_XP :: xp_for_level(MAX_LEVEL);
            ^
1 error
```

That's deliberate. You can always find where the compiler runs your
code, because it only does so where `#run` says. Arithmetic on
constants doesn't count as running code, so `TILES :: TILE_EDGE *
TILE_EDGE;` needs no `#run`. A *call* does.

### What `#run` applies to

`#run` applies to the expression right after it: a call, a name, or a
parenthesized expression. To run something larger, put it in
parentheses:

```jai
TOTAL :: #run (xp_for_level(5) + xp_for_level(6));
```

`#run` can also appear inside a procedure body, wherever an expression
can. The value is still computed while compiling:

```jai
main :: () {
  area := #run square(12);   // becomes: int64_t l_area = INT64_C(144);
  print("%\n", area);
}
```

`#run` has to produce a value. A `#run` whose result you don't use,
such as a bare `#run setup();` statement, is rejected with `E0322`
("this expression's value is not used"), the same as any other unused
value.

## Building a whole table

One number is a start. What we really want is the whole table, so the
running program can look up a level without doing any arithmetic. A
`#run` can return a fixed array:

Filename: examples/ch09_levels3.coax

```jai
#import "Basic";

MAX_LEVEL :: 10;

XP_TABLE :: #run build_xp_table();

#assert XP_TABLE[0] == 0;
#assert XP_TABLE[MAX_LEVEL - 1] == 8025;
#assert #run is_increasing(XP_TABLE);

xp_for_level :: (level: int) -> int {
  total := 0;
  for n: 1..<level {
    total += 100 + n * n * 25;
  }
  return total;
}

build_xp_table :: () -> [MAX_LEVEL] int {
  table: [MAX_LEVEL] int;
  for i: 0..<MAX_LEVEL {
    table[i] = xp_for_level(i + 1);
  }
  return table;
}

is_increasing :: (table: [MAX_LEVEL] int) -> bool {
  for i: 1..<MAX_LEVEL {
    if table[i] <= table[i - 1] { return false; }
  }
  return true;
}

level_for_xp :: (xp: int) -> int {
  level := 1;
  for need, i: XP_TABLE {
    if xp >= need { level = i + 1; }
  }
  return level;
}

main :: () {
  print("%\n", XP_TABLE);
  print("1234 xp is level %\n", level_for_xp(1234));
}
```

*Listing 9-3: The experience table built while compiling, with facts checked by `#assert`*

```shell
$ coax run examples/ch09_levels3.coax
[0, 125, 325, 650, 1150, 1875, 2875, 4200, 5900, 8025]
1234 xp is level 5
```

`build_xp_table` fills a `[MAX_LEVEL] int` and returns it, and
`XP_TABLE :: #run build_xp_table();` makes the result a constant. At run
time, `level_for_xp` just loops over a table that already exists. In
the generated C, the table is a `static const` array:

```c
static const coax_a10_s64 coax_k0 = {{INT64_C(0), INT64_C(125), INT64_C(325), INT64_C(650), INT64_C(1150), INT64_C(1875), INT64_C(2875), INT64_C(4200), INT64_C(5900), INT64_C(8025)}};
```

The three `#assert` lines near the top are new. Let's look at them.

## Checking facts with `#assert`

At the top level of a file, `#assert` checks a `bool` while compiling.
If it's false, there's no program. Suppose someone changes the formula
and the top level no longer costs what the design document says. Change
the second assertion to expect 8000 and build:

```shell
$ coax check levels.coax
levels.coax:8:33: error[E0406]: compile-time assertion failed
  #assert XP_TABLE[MAX_LEVEL - 1] == 8000;
                                  ^
1 error
```

The third assertion is the more interesting one: it calls a procedure,
`is_increasing`, to check a property of the whole table. If someone
turns the `+` in the formula into a `-` by mistake, each level would
cost *less* than the one before:

```shell
$ coax check levels.coax
levels.coax:8:9: error[E0406]: compile-time assertion failed
  #assert #run is_increasing(XP_TABLE);
          ^
1 error
```

An `#assert` that calls a procedure needs `#run` too, by the same rule
as constants. Without it:

```shell
$ coax check levels.coax
levels.coax:9:9: error[E0408]: a constant procedure call requires #run
  #assert is_increasing(XP_TABLE);
          ^
1 error
```

Use top-level `#assert` for facts the code depends on but can't cheaply
check at run time: a table size that must be a power of two, a budget
that must fit, a constant that must agree with another, a table that
must be sorted. A fact you'd otherwise check by reading the output
becomes a line the build checks every time.

> Note: Inside a procedure body, `#assert` is a run-time check that
> traps when it fails ([Chapter 7](ch07-errors.md)). At the top level,
> it runs while compiling. Same keyword, same meaning ("this must be
> true"), checked at the earliest moment it can be.

## Structs, and constants built from constants

A `#run` can return anything that doesn't contain a view: a scalar, a
string, an enum, a struct or a fixed array. And one compile-time
constant can be computed from another. Here's a table of the first 32
primes, and a struct of facts measured from that table:

Filename: examples/prime_table.coax

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

*Listing 9-4: A prime table and a struct computed from it, both while compiling*

```shell
$ coax run examples/prime_table.coax
[2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89, 97, 101, 103, 107, 109, 113, 127, 131]
Gaps.{widest = 14, after = 113, twins = 10}
```

`measure` reads `PRIMES`, so the compiler has to evaluate `PRIMES`
before `GAPS`. It works that order out for itself: swap the two
declarations and the program builds and prints the same thing. A struct
result becomes a `static const` too:

```c
static const coax_t_Gaps coax_k1 = {.widest = ((uint16_t)UINT64_C(14)), .after = ((uint16_t)UINT64_C(113)), .twins = INT64_C(10)};
```

The line `table := PRIMES;` deserves a word. Slicing makes a view, and a
view must borrow a variable's storage, so we copy the constant into a
local before slicing it ([Chapter 6](ch06-memory.md)).

The second `#assert` is there because the first draft of this example
said there were 11 twin-prime pairs. The compiler refused to build it
until the claim matched the table.

Results are remembered. A constant used in ten places is computed once.
And constants can't depend on themselves: `A :: B + 1; B :: A * 2;`
fails with `E0340` ("constant dependency cycle"), and a cycle that goes
through a procedure (a `#run` that reads the constant it's computing)
fails with `E0402`.

## Asking about types: `type_name` and `size_of`

Coax has layout reflection today. `type_name(expression)`
gives the name of an expression's type as a string:

```jai
#assert type_name(XP_TABLE) == "[10] int";
```

Its argument is type-checked but never evaluated, so `type_name(1 / 0)`
is `"int"` and doesn't divide by anything. It names every type `coax0`
has: `"u8"`, `"Vec2"`, `"[3] int"`, `"[] int"`.

`size_of(T)` takes a type, not a value, and gives its size in bytes. It's
known while checking, so it can size an array:

```jai
#import "Basic";

Pulse :: struct { a: u32; b: u8; c: f64; }

main :: () {
  buffer: [size_of(Pulse)] u8;
  print("% % %\n", size_of(Pulse), buffer.count, type_name(buffer));
}
```

```jai
16 16 [16] u8
```

`size_of` describes the bootstrap's LP64 CPU target. Since 0.12.0,
`type_info(T)` also gives its name, kind, alignment, scalar width/sign,
field/member counts and array/view element information. `field_info(T, i)`
gives a struct field's name, type name, byte offset, size and alignment.
`enum_info(T, i)` gives an enum member's name and underlying bits.

These are ordinary records, with `schema_version` equal to 1. You can pass
them to procedures or build an inspector table once during compilation:

<!-- checked-source: examples/layout-reflection.coax -->
```jai
Transform :: struct {x:f32;y:f32;selected:bool;}
FIELDS :: #run transform_fields();

transform_fields :: ()->[3]CoaxFieldInfo {
 fields:[3]CoaxFieldInfo;
 for i:0..<fields.count {fields[i]=field_info(Transform,i);}
 return fields;
}
```

The complete [layout-reflection example](examples/layout-reflection.coax)
checks the table and prints it:

```jai
Transform: 12 bytes, alignment 4
x f32 @0 (4 bytes)
y f32 @4 (4 bytes)
selected bool @8 (1 bytes)
```

`type_info(Transform).field_count` and `field_info(Transform, 1).offset`
are known while checking, so they can also appear in array lengths.
Selectors follow declaration order. A constant invalid index is rejected;
a dynamic invalid index traps in both modes. A dynamic selector creates a
bounded array of metadata, so use `#run` for a table you will inspect often.
The generated program receives literals, not a compiler or reflection VM.

`CoaxTypeInfo` contains `schema_version`, `name`, `kind`, `size`, `alignment`,
`bits`, `is_signed`, `field_count`, `member_count`, `element_count` and
`element_name`. `CoaxFieldInfo` contains `schema_version`, `index`, `name`,
`type_name`, `offset`, `size` and `alignment`. `CoaxEnumInfo` contains
`schema_version`, `index`, `name`, `value_bits`, `bits` and `is_signed`.
Names use resolved spellings, so `s64` is `int`. Enum `value_bits` is the
underlying-width pattern zero-extended to u64: an s8 member of -2 is 254.
A view's element count is zero because its length belongs to the value.

The queries take type syntax. They don't make types first-class or expose
procedure bodies. They describe CPU storage, not protobuf bytes or Metal
vector packing. The actual C compiler still checks emitted layout assertions.
See [Chapter 11](ch11-data-layout.md) for those boundaries.

## Compile time is sealed

Code running inside the compiler can compute. It can't do anything else.
These rules are what make it safe to run arbitrary procedures in the
middle of a build, and what make the same source always compile to the
same constants.

### No output, no files, no clock

Let's try to print from `xp_for_level` while it builds the table:

```jai
#import "Basic";

MAX_XP :: #run xp_for_level(10);

xp_for_level :: (level: int) -> int {
  total := 0;
  for n: 1..<level {
    total += 100 + n * n * 25;
  }
  print("level % needs % xp\n", level, total);
  return total;
}

main :: () { print("%\n", MAX_XP); }
```

```shell
$ coax check levels.coax
levels.coax:10:3: error[E0404]: compile-time execution cannot perform output or host I/O
    print("level % needs % xp\n", level, total);
    ^
  note: compile-time call at levels.coax:3:16
1 error
```

The error points at the `print`, and the note points at the `#run` that
led there, so you can tell which compile-time call was responsible. The
same procedure can still print when it's called at run time. What's
refused is printing *during compilation*.

There are no files, processes, network, clock, randomness, foreign calls
or allocation at compile time. If compile-time code could read the clock
or a file, two builds of the same source could produce different
constants, and nothing about the program would tell you why.

> Note: Jai lets `#run` code do almost anything, including reading files
> and calling the operating system, and its build scripts rely on that.
> Coax keeps compile-time *program* code sealed. Build-time work that
> needs the outside world is planned as plugins with explicit
> capabilities, described below.

### No reaching into the running program

A `#run` inside a procedure can't read that procedure's parameters or
locals, because they don't exist yet when the compiler runs it:

```jai
level_xp :: (level: int) -> int {
  return #run xp_for_level(level);
}
```

```shell
$ coax check levels.coax
levels.coax:4:28: error[E0401]: compile-time expression cannot capture runtime value 'level'
    return #run xp_for_level(level);
                             ^
1 error
```

If you want a run-time lookup, look in a table computed at compile time,
like `XP_TABLE`.

### Every `#run` must succeed

A `#run` inside a branch that never runs at run time is still evaluated
while compiling, and it must still succeed:

```jai
#import "Basic";

clamp_level :: (n: int) -> int {
  if n > 1000 { return #run ceiling(0); }   // never runs while main runs
  return n;
}

ceiling :: (levels: int) -> int { return 100 / levels; }

main :: () { print("%\n", clamp_level(3)); }
```

```shell
$ coax check br.coax
br.coax:8:46: error[E0407]: division by zero during compile-time evaluation
  ceiling :: (levels: int) -> int { return 100 / levels; }
                                               ^
  note: compile-time call at br.coax:4:29
1 error
```

Compile-time code is checked as part of the program, not only when
execution happens to reach it.

### Only values come out, never views

A scalar result becomes a literal in the generated code, and a struct or
array becomes a static constant. A view would point into the compiler's
own memory, which is gone by the time the program runs, so a result that
contains one is refused:

```jai
EVENS :: #run evens();

evens :: () -> [] int {
  a := [4] int.[0, 2, 4, 6];
  return a[..];              // a view of the compiler's memory
}

main :: () {}
```

```shell
$ coax check ct_view.coax
ct_view.coax:1:10: error[E0409]: a compile-time result cannot contain a view; views borrow memory that exists only while compiling
  EVENS :: #run evens();
           ^
1 error
```

The fix is to return the array itself, `-> [4] int`, as
`build_xp_table` does.

### Mistakes are errors, not traps

An index, slice, cast or shift that would trap at run time stops the
build instead, with a note at the `#run` that led there:

```jai
LAST :: #run last(4);

last :: (n: int) -> int {
  table := [4] int.[1, 2, 3, 4];
  return table[n];           // should be n - 1
}

main :: () {}
```

```shell
$ coax check ct_bounds.coax
ct_bounds.coax:5:15: error[E0410]: index 4 is out of bounds for 4 elements
    return table[n];           // should be n - 1
                ^
  note: compile-time call at ct_bounds.coax:1:14
1 error
```

```jai
BYTE :: #run to_byte(300);

to_byte :: (n: int) -> u8 {
  return cast(u8) n;
}

main :: () {}
```

```shell
$ coax check ct_cast.coax
ct_cast.coax:4:10: error[E0411]: cast to u8 is out of range
    return cast(u8) n;
           ^
  note: compile-time call at ct_cast.coax:1:14
1 error
```

Notice that the compile-time message says more than the run-time trap
would: it gives the index and the length. A constant can't be
half-computed, so there's no "trap and continue" while compiling.
Overflow is checked here in every build mode, so a `--release` build can
never compute a different constant from a debug build, even though
run-time overflow wraps in release ([Chapter 7](ch07-errors.md)).

## Compile time is bounded

A loop that never ends shouldn't hang your build:

```jai
FOREVER :: #run spin();

spin :: () -> int {
  i := 0;
  while true { i += 1; }
  return i;
}

main :: () {}
```

```shell
$ coax check spin.coax
spin.coax:5:21: error[E0400]: compile-time evaluation exceeded 100000 steps
    while true { i += 1; }
                      ^
  note: compile-time call at spin.coax:1:17
1 error
```

A *step* is one expression or statement the evaluator visits. Creating
a zero-initialized array counts too, one step per element, so a table can't dodge the
budget by being one big declaration:

```jai
MAP :: #run blank_map();

blank_map :: () -> [200000] u8 {
  map: [200000] u8;          // 200,000 elements, one step each
  return map;
}

main :: () {}
```

```shell
$ coax check big.coax
big.coax:4:3: error[E0400]: compile-time evaluation exceeded 100000 steps
    map: [200000] u8;          // 200,000 elements, one step each
    ^
  note: compile-time call at big.coax:1:13
1 error
```

The budget covers the whole compilation, not each `#run`. Here are all
the limits and the errors that go with them:

| Limit | Value | Error |
| --- | --- | --- |
| Evaluation work in one compile | 100,000 steps by default; `--run-steps N` selects up to 250,000,000 | `E0400` |
| Live aggregate storage | 1,572,864 vector capacity slots and 131,072 containers | `E0400` |
| Aggregate traversal | 134,217,728 value/field visits per frontend evaluation or individual check | `E0400` |
| Owned cast-string storage | 67,108,864 live bytes per compile; 268,435,456 materialized bytes per frontend evaluation or individual check | `E0400` |
| Nested compile-time calls | 64 | `E0403` |
| Constants depending on constants | 64 deep | `E0403` |
| Overflow or division by zero | Always an error, in debug and release | `E0407` |
| Reading an uninitialized (`---`) local | Always an error | `E0405` |
| A view in a result | Always an error | `E0409` |
| Index or slice out of bounds | Always an error | `E0410` |
| Cast, enum value or shift out of range | Always an error | `E0411` |

`--run-steps N` is a build input shared by all compile-time sites. It does not
change `coax test --steps`, mutation or native optimization. `coax deps`
records the selected limits; `--timings FILE` records frontend evaluation work
and tracked peaks before authority checks. A byte-view-to-string cast creates
an owned compile-time snapshot. Copies and slices share it; a frozen result
keeps it through code emission. Each snapshot charges its length plus a
terminator to live bytes and materialization work. Releasing its last owner
returns the live bytes; cumulative work stays charged until that evaluation or
check ends. Source strings and allocator metadata are outside these limits.
Deep copies, aggregate assignment, view detection and struct or array
initialization charge a separate traversal budget for each visited value or
field, including work before a failure. This does not change the ordinary step
counter. Each check, including all of its quantified cases, has its own
traversal budget. `coax deps` records the limit and `--timings` reports frontend
traversal work. Cleanup is not charged to this meter; live aggregate storage
and the weak-storage registry are independently bounded. These limits do not
cover all compiler memory or work.
Views keep their storage alive, and cycles are cleared at compilation teardown;
an unreachable cycle can use budget until then.

The limits count work, not seconds. A program that compiles on a fast
laptop compiles on a slow CI machine too, and a build never passes or
fails depending on how busy the machine was.

The call-depth limit catches runaway recursion. A recursive procedure
that goes 100 calls deep, `down(100)`, fails with `E0403` ("compile-time
call depth exceeds 64"), and the error lists each call on the way down,
so expect a long message.

## Checks run here too

The evaluator that runs `#run` also runs Coax's checks: `#test` blocks,
`#forall` blocks that try every combination of their inputs, and the
`coax test` and `coax mutate` commands that run them. Checks are written
before the code they check, are never part of the program, and have
their own, much larger step budget. A file marked `#authority;` runs its
checks on every build. That's a big enough subject to have its own
chapter: [Chapter 10](ch10-writing-checks.md).

## In the real world: a CRC-32 table

`coax/examples/crc32.coax` does for a real file format what Listing 9-3
did for our game. CRC-32, the checksum in zlib and PNG, uses a 256-entry
table derived from a polynomial. The file computes that table with
`#run`, then checks it against the standard before any C is written:

```jai
TABLE :: #run make_table();

#assert TABLE[1] == 0x77073096;
#assert TABLE[255] == 0x2D02EF8D;

// ... crc32 :: (data: string) -> u32 uses TABLE

CHECK :: #run crc32("123456789");
#assert CHECK == 0xCBF43926;
```

The last two lines are the important ones. `0xCBF43926` is the
published check value for CRC-32 over the text `123456789`, so the
build itself confirms the table and the procedure that uses it are
right. The same `crc32` then runs at run time on real data.

## Planned: code as a value

Everything from here on is design, from spec §10. None of it runs in
`coax0`.

The next step after running code at compile time is *producing* code at
compile time. A parameter of type `Code` receives an expression without
evaluating it. `#insert` puts code into the program, and `#expand` makes
a procedure into a macro that expands at its call site:

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
text of both sides of a failed comparison, so a failure reads `left: 3
(count)` rather than just "not equal". Today `coax0` stops at `#expand`
with `E0210` ("#expand is not supported on procedures by coax0 yet").

Coax plans rules that Jai doesn't have:

Names a macro declares stay inside it (*hygiene*). To touch a caller's
variable, a macro has to name it in its signature, with read or write
permission, so it can't reach a caller's locals by guessing their names.

An argument used twice must be bound to a temporary, or the macro has to
say that it duplicates it. A macro can't silently evaluate `next_id()`
twice.

The compiler keeps the chain from the call, to the generator, to the
generated code, to the C it became. Errors in generated code point at
the call first, with notes into the expansion.

Budgets count work, not time: proposed at 64 nested expansions and
100,000 new syntax nodes per workspace. A millisecond limit would make
whether a program compiles depend on the machine.

## Reflection still planned

The working layout queries are the first part of the metadata system.
First-class `type_of`/`Type`, defaults, annotations, procedure signatures,
effects and explicit alternate-target layout queries remain planned.
Nominal declaration IDs and original source spans already appear in the
checked graph reports described in Chapter 22; exposing richer identity
metadata as compile-time values is separate work. `type_of` is still
reserved and rejected. Today's metadata does not permit macros, typed
syntax insertion or compiler plugins.

A query-bearing source graph gets 65,536 extra checker nodes. Metadata
expansion shares that bounded pool and refuses exhaustion with E0381;
the compile-time evaluator's separate step limits still apply to `#run`.
This bounds generated tables without pretending finite tests prove every
possible generator correct.

A temporary type ID from the compiler is not a stable identity. Engine
component IDs and protocol names stay explicit and survive a rebuild.

## Planned: compiler plugins

A plugin is Coax code the compiler loads during a build. It gets typed
events (file loaded, parsed, signature ready, typechecked, all ready
work checked, before lowering, and complete) and can propose new source,
dependencies and diagnostics through checked APIs. Everything it adds
goes through the same parser, checker and layout rules as handwritten
code, and a plugin can never mark unchecked code valid. A plugin's
identity, version and inputs are part of the build's cache key.

The Jai plugin that runs a program after a successful compile is the
model. Coax's version gets a tagged union of messages instead of
unchecked casts, and a capability model for any file or process work.

## Planned: conditional compilation

```jai
#if TARGET_IS_IOS { ... } else { ... }
```

`#if` chooses code while compiling. Both branches must parse, and only
the chosen one is checked and compiled. The condition is a compile-time
constant, an explicit build input, and part of the cache key. This isn't
C's preprocessor deleting text before the compiler sees the file. And
it's different from a run-time `if`, whose branches are all checked
(and whose `#run` sites must all succeed, as we saw). `coax0` rejects
`#if` at the top level of a file with `E0210` ("#if is not supported by
coax0 yet").

## Summary

`#run` evaluates an ordinary procedure while the program compiles, and
its result becomes a literal or a static constant in the generated C.
We used it to turn a run-time loop into a table that exists before the
program starts, and top-level `#assert` to make facts about that table
part of the build. Compile-time code is sealed: no output or host
access, no capturing run-time values, no views in results, and every
mistake that would trap at run time is a compile error, in every build
mode. It's also bounded by work, not time, so builds behave the same on
every machine. `type_name` and `size_of` are the first pieces of
reflection; typed code, macros, full reflection, plugins and `#if` are
planned.

The same evaluator powers Coax's checks. In [Chapter 10](ch10-writing-checks.md)
we'll write them: tests and exhaustive checks that run before the code
they check, and the tools that check the checks.
