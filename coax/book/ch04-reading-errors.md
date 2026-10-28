# 4. Reading errors

> **Runs today.** Every error in this chapter is exactly what `coax0`
> prints.

A compiler spends most of its life rejecting programs, and many of
those programs were written by agents. So Coax treats error messages as
part of the language, with the same care as its syntax.

## 4.1 The shape of an error

Here's a program with four mistakes:

```coax
#import "Basic";

half :: (n: int) -> int {
  if n > 0 { return n / 2; }
}

main :: () {
  name := "Kiki";
  if name { print("%\n"); }
  x: int = name;
}
```

`coax check` reports all four:

```
b.coax:3:1: error[E0338]: 'half' can reach its end without returning int
  half :: (n: int) -> int {
  ^
  fix: add a return at the end
b.coax:9:6: error[E0331]: condition must be bool, found string
    if name { print("%\n"); }
       ^
b.coax:9:19: error[E0320]: format has 1 '%' slot but print was given 0 values
    if name { print("%\n"); }
                    ^
  fix: write '%%' for a literal percent sign
b.coax:10:12: error[E0325]: 'x' is int but is initialized with string
    x: int = name;
             ^
4 errors
```

Every error has the same five parts:

1. **Where:** `file:line:column`, in the format editors and terminals
   already turn into links.
2. **A stable code:** `E0338`. Codes don't change between versions, so a
   script, a test or an agent can match on the code instead of the
   wording. [Appendix B](appendix-b-error-codes.md) lists every one.
3. **What's wrong**, in one sentence that names the things involved
   (`'half'`, `int`, `string`).
4. **The line, with a caret** under the exact spot.
5. **A fix**, when the compiler knows one. Fixes are written as the edit
   to make, not as advice.

## 4.2 How far the compiler gets

Errors come from four stages, and the code's first two digits say which:

| Codes | Stage | After an error |
| --- | --- | --- |
| `E01xx` | Lexer: characters into tokens | Keeps going, reports every bad token |
| `E02xx` | Parser: tokens into a syntax tree | **Stops at the first error** |
| `E03xx` | Checker: names, types, returns, formats | Keeps going, reports everything it finds |
| `E04xx` | Compile-time evaluation (`#run`, `#assert`) | Stops at the first failed evaluation |

The parser stops at its first error on purpose. After a missing brace,
a parser that keeps guessing produces a cascade of errors that aren't
real, and someone (often an agent) wastes time on them. One true error
beats ten invented ones. The checker is the opposite: by the time it
runs, the program's shape is known, so every type error it reports is
real, and you want all of them at once.

## 4.3 Errors that point at the right place

A missing semicolon is reported where the semicolon belongs, not at the
next thing the parser happens to see:

```coax
main :: () {
  x := 1
  y := 2;
}
```

```
c.coax:2:9: error[E0201]: expected ';' after the declaration, found 'y'
    x := 1
          ^
  fix: add ';'
1 error
```

The caret is just after the `1` on line 2, even though the parser only
found out on line 3.

## 4.4 Not yet, rather than no

Coax reserves the whole language's vocabulary now, even the parts that
`coax0` can't compile yet:

```
d.coax:3:3: error[E0204]: 'for' is reserved and not supported by coax0 yet
    for 0..9 { }
    ^
1 error
```

"Not supported yet" is a different message from "unknown", so you can
tell a typo from a feature that's coming. It's also a guarantee: no
program that compiles today can change meaning when `for`, `struct` or
`f32` arrive, because none of them can appear in a program that compiles
today.

## 4.5 Compile-time errors show the call chain

When the error happens while the compiler is running your code (Chapter
9), the message points at the failing line and then lists how the
compiler got there:

```coax
#import "Basic";

SLOTS :: #run slots_for(40);

slots_for :: (rooms: int) -> int {
  return capacity(rooms) / (rooms - 40);
}

capacity :: (rooms: int) -> int { return rooms * 8; }

main :: () { print("%\n", SLOTS); }
```

```
ct.coax:6:26: error[E0407]: division by zero during compile-time evaluation
    return capacity(rooms) / (rooms - 40);
                           ^
  note: compile-time call at ct.coax:3:15
1 error
```

The `note:` lines are the call stack, innermost first, kept to a
bounded length so a deep recursion can't flood the terminal.

## 4.6 Runtime errors

Some errors can only happen while the program runs. `coax0` turns the
ones C leaves undefined into clean stops, with the Coax source file and
line:

```
overflow.coax:6: integer overflow in '+'
```

| What | Debug build | `--release` |
| --- | --- | --- |
| Integer overflow (`+ - *`, unary `-`) | Stops, with file and line | Wraps |
| Division or `%` by zero | Stops | Stops |
| Smallest `int` divided by `-1` | Stops | Stops |
| Index or slice out of bounds | Stops | Stops |
| `cast(T)` of a value that doesn't fit | Stops | Stops |
| Shift by a negative count or the width or more | Stops | Stops |

## 4.7 Writing for the reader who fixes it

The rules Coax follows for its messages are worth knowing, because
they're also rules for the errors your own code reports:

- **Name things.** "'x' is int but is initialized with string" beats
  "type mismatch".
- **Say it once.** One error per mistake. No cascades.
- **Point at the cause**, not the symptom. A missing `;` belongs on the
  line that's missing it.
- **Give the edit.** "add ';'" or "add #import \"Basic\"; at the top of the
  file", not "check your syntax".
- **Keep codes stable** so tools can depend on them.

## 4.8 Errors for tools

Two flags make errors easy for programs, and agents, to use.

`--json` prints each diagnostic as one JSON object per line:

```sh
coax check program.coax --json
```

```
{"file": "tests/fail/array_count.coax", "line": 2, "col": 19, "code": "E0358", "message": "[3] int needs 3 elements, given 2", "fix": ""}
```

`coax explain` describes any code, with the rule behind it and the usual
fix:

```sh
coax explain E0350
```

```
E0350: value does not fit its type
  Untyped literals take the type their use needs, if the value fits: x: u8 = 300 does not. A float literal cannot become an integer. Use a wider type, or cast,trunc to keep the low bits.
```

`coax explain --all --markdown` prints every code, and is how
[Appendix B](appendix-b-error-codes.md) is generated.
