# Appendix C. Coax for Jai programmers

If you've written Jai, you already know most of Coax's syntax. Coax starts
from Jai's design on purpose: the declaration forms, compile-time execution
with `#run`, the implicit context and the idea of the build as a program all
come from there. This appendix walks through what carries over unchanged,
what Coax changed and why, and what `coax0` doesn't have yet, with the two
languages side by side.

The Jai examples follow publicly available Jai code, such as Forrest
Smith's 2022 Advent of Code solutions. Every Coax example here was checked
with `coax0` 0.10.0, and the errors are its real output.

## What looks the same

Declarations read exactly as they do in Jai. A variable with an inferred
type, one with a declared type, a constant, an uninitialized buffer and a
procedure are spelled identically in both languages:

```jai
x := 1;
y: int = 2;
LIMIT :: 64;
buf: [128] u8 = ---;

add :: (a: int, b: int) -> int {
    return a + b;
}
```

`#import "Basic";` works the same way (and `Basic` is the only module
`coax0` has), `ifx c then a else b` is the same expression, and `defer`
runs at scope exit in reverse order. The three array forms are the same
too: `[N] T` for fixed arrays, `[] T` for views and `[..] T` for dynamic
arrays, though `coax0` only implements the first two. `string` is a
counted byte view in both.

Pointers will be familiar as well: the design uses `*x` for the address and
`p.*` to dereference, as current Jai does. They aren't in `coax0` yet.

## Loops and ranges

This is the first difference you'll hit. In Jai, `for` gives you `it` and
`it_index` for free, and a range includes both ends:

```jai
// Jai
for scores print("%\n", it);
for i: 0..scores.count-1 print("%\n", scores[i]);
```

In Coax you name the element and index yourself, and ranges are
half-open, written `..<`:

```jai
// Coax
for s, i: scores {
    print("% %\n", i, s);
}
for i: 0..<scores.count {
    print("%\n", scores[i]);
}
```

`it` and `it_index` are reserved words in Coax, so you can't use them by
accident. If you write a Jai-style `..` range out of habit, `coax0` stops
you instead of guessing which end you meant:

```shell
$ coax check loops.coax
loops.coax:5:13: error[E0216]: ranges are half-open: write '..<'
      for i: 0..scores.count {
              ^
  fix: for i: 0..<n { }
1 error
```

The reason is the same one that led C programmers to `i < n`: with a
half-open range, `0..<count` covers exactly the elements of a collection,
and the classic `count - 1` off-by-one never needs writing. Braces are also
required around a loop body in Coax; there's no single-statement form.

## No shadowing

Jai lets an inner scope declare a name that hides an outer one. Coax
doesn't:

```jai
main :: () {
    count := 3;
    if count > 0 {
        count := 10;
        print("%\n", count);
    }
}
```

```shell
$ coax check shadow.coax
shadow.coax:6:9: error[E0303]: 'count' is already declared at line 4; Coax does not allow shadowing
          count := 10;
          ^
  fix: pick another name
1 error
```

Within a procedure, a name means one thing everywhere it's in scope. That
makes code easier to read in small pieces, for people and for agents
editing it.

## Integer overflow

Jai doesn't check integer overflow at run time by default. Coax does, in
development builds:

```jai
#import "Basic";

main :: () {
    hp: u8 = 250;
    for i: 0..<10 { hp += 1; }
    print("%\n", hp);
}
```

```shell
$ coax run wrap.coax
wrap.coax:5: integer overflow in '+'
$ coax run wrap.coax --release
4
```

A debug build traps at the line that overflowed; a release build wraps, as
Jai would. Overflow is never undefined behavior in either mode, and
overflow during compile-time evaluation is always an error, whatever the
build mode, so a release flag can't change your constants.

## Conversions

Jai's `xx` converts a value to whatever type the context needs:

```jai
// Jai
big: int = 300;
small: u8 = xx big;
```

`xx` is reserved in Coax, and `coax0` refuses it:

```shell
$ coax check xx.coax
xx.coax:3:17: error[E0204]: 'xx' is reserved and not supported by coax0 yet
      small: u8 = xx big;
                  ^
1 error
```

In Coax you say which conversion you mean. `cast(u8) big` checks that the
value fits and traps if it doesn't; `cast,trunc(u8) big` keeps the low bits
(here, 44). The plan is to allow `xx` again only between integer widths,
never as a general autocast. Literals don't need any of this: an untyped
literal takes the type its use needs if the value fits, so `hp: u8 = 250;`
just works and `hp: u8 = 300;` is error E0350.

## Enums

A Jai enum can leave out its integer type. A Coax enum always names it,
because the width is part of the layout:

```jai
// Coax
Color :: enum u8 { red; green; blue = 7; }
```

Leaving it out is an error:

```shell
$ coax check en.coax
en.coax:1:15: error[E0219]: an enum names its underlying integer type
  Color :: enum { red; green; }
                ^
  fix: Outcome :: enum u8 { ... }
1 error
```

Jai's `#specified` makes explicit values opt-in. In the Coax design, an
enum that crosses a boundary (a save file, a protocol message) must give
explicit values, so that reordering the members can't change what stored
data means. `coax0` doesn't enforce that rule yet.

## Compile-time code

`#run` looks the same, but Coax's is sealed. Jai's `#run` can do anything a
program can, including printing and reading files. Coax's can compute, but
it can't print or touch the host, and it runs within a step budget. Chapter
20 shows the real error you get from a `print` inside `#run` (E0404). The
point is that the same source always produces the same constants, and that
an editor can evaluate your compile-time code safely while you type. File
and process access at build time is planned through declared capabilities
(Chapter 16).

Coax also adds something Jai doesn't have built in: checks. `#test` and
`#forall` blocks live next to the code, run in the compile-time evaluator,
and never end up in the program; `coax mutate` then checks the checks.
Chapter 10 covers them.

Generic procedures already bind `$T` and fixed-array lengths `$N`, with
bounded checked instantiation and instance-count reporting. Fixed-storage
generic structs also instantiate type and length parameters; view type
arguments are refused. Constraints remain planned. Chapter 8 explains
those separate scopes.

## Remaining design differences

The following differences remain in the design. Each keeps an idea from
Jai with an explicit rule changed:

| Jai | Coax design | Why |
| --- | --- | --- |
| `union` overlays storage | `union` is tagged, and `switch` over it must be exhaustive | The safe form should be the default |
| Equality and hashing found by name; `#poke_name` to make a procedure visible | `#derive(eq, hash)` and named constraints | Errors at the use site, with a fix |
| `$T/interface` constraints are available | Named constraints are planned; current unconstrained generic procedures support copying, assignment and indexing | A generic's signature should state its additional required operations |
| Macro backticks reach into the caller | The caller variables a macro can touch are listed in its signature | Reading a call should show what it can change |
| `using` and `#as` | The same split: `using` for naming, `#as` for a declared conversion | Kept as Jai has it |
| `context` and `push_context` | The same idea, with allocation as a tracked effect | Allocation should be visible to the checker |
| Threads and mutexes | Jobs with declared read and write effects, scheduled from a frame graph (Chapter 12) | Parallelism with a contract |
| No published float model | `strict` and `deterministic`, checked through every call (Chapter 13) | Bit-identical simulation and replay |
| Hash tables iterate in table order | Insertion order by default | Replays and determinism |
| No shader language | Shared GPU layouts; the shader backend is decided by experiment (Chapter 14) | CPU and GPU struct mismatches are a real bug class |
| Metaprogram plugins with compiler messages | The same idea, with typed messages and declared capabilities | Builds stay reproducible |
| Its own x64 backend and LLVM | C output first; native backends only when measured to be better | Ship through Xcode and Emscripten from the start |

Some Jai features are deliberately left out of the plan: `Any` in
persistent or protocol data, raw memory images as save files, loading code
at run time on iOS, a package manager inside `#import`, and `#modify`
(unless ordinary constraints turn out to be insufficient).

## What Coax owes Jai

The central idea, that the compiler's knowledge of a program is
infrastructure the program itself should be able to use, is Jai's. So are
the declaration syntax, compile-time execution, the context, the build as a
program, and the conviction that a large program should rebuild in
seconds. Coax exists because we can't use Jai in OpenAgents, not because we
think we can do without its ideas.
