# 4. Reading errors

In Chapter 3 the compiler refused about a dozen of our programs. That's
normal. Most of the time a compiler spends with a program, the program
is wrong, because you're in the middle of writing it. So it pays to get
fast at reading what the compiler says: where to look, what the message
is telling you, and when the suggested fix is the right one.

In this chapter we'll take a compiler error apart piece by piece, see
how far `coax0` gets through a broken program before it stops, and then
fix a program with six mistakes in it, one round at a time, until it
runs. We'll finish with the errors that happen while a program runs, and
with the two features meant for tools: `coax explain` and `--json`.

> Note: Every diagnostic in this chapter was produced by running
> `coax0` 0.10.0 on the program shown.

## The parts of an error

Here's a short program with four mistakes in it:

Filename: half.coax

```jai
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

*Listing 4-1: A program with four mistakes*

`coax check` compiles a file without building or running it, which makes
it the quickest way to see what the compiler thinks:

```
$ coax check half.coax
half.coax:3:1: error[E0338]: 'half' can reach its end without returning int
  half :: (n: int) -> int {
  ^
  fix: add a return at the end
half.coax:9:6: error[E0331]: condition must be bool, found string
    if name { print("%\n"); }
       ^
half.coax:9:19: error[E0320]: format has 1 '%' slot but print was given 0 values
    if name { print("%\n"); }
                    ^
  fix: write '%%' for a literal percent sign
half.coax:10:12: error[E0325]: 'x' is int but is initialized with string
    x: int = name;
             ^
4 errors
```

Let's read the first one line by line.

```
half.coax:3:1: error[E0338]: 'half' can reach its end without returning int
```

The line starts with where the problem is: file, line and column,
`half.coax:3:1`. That's the format most editors and terminals already
recognize, so in many setups you can click it to jump to the spot.

Next comes `error[E0338]`. Every diagnostic has a code like this: `E`,
then four digits. The code names the *kind* of mistake, and it doesn't
change between compiler versions even if the wording does. When a code
stops being used, it's marked "retired" rather than handed to a
different error. That's what lets a script, a test or an agent match on
`E0338` instead of on the English text.

After the code is the message itself, one sentence that names the things
involved: the procedure `'half'` and the type `int`. Good messages say
"'x' is int but is initialized with string", not "type mismatch".

```
  half :: (n: int) -> int {
  ^
```

Then the compiler prints the source line and puts a caret under the
column it's talking about. Here the caret is at the start of the
procedure, because the problem belongs to `half` as a whole: some path
through it reaches the closing brace with no `return`.

```
  fix: add a return at the end
```

Last, when the compiler knows a likely edit, it says so on a `fix:`
line. Not every error has one. The second error, about `if name`,
doesn't, because the compiler can't guess what you meant to test about a
string.

After the last error, `4 errors` gives the count. `coax check` exits
with status 1 when there are errors, and when there aren't, it prints
nothing at all and exits with status 0.

### Fix lines are suggestions

Read the third error again:

```
half.coax:9:19: error[E0320]: format has 1 '%' slot but print was given 0 values
    if name { print("%\n"); }
                    ^
  fix: write '%%' for a literal percent sign
```

The message is exactly right: the format has a `%` and there's no value
for it. The fix assumes we meant a literal percent sign, which isn't
what happened here. We forgot the value. The right edit is
`print("%\n", name);`. A `fix:` line is the compiler's best guess at the
most common cause. It's often right, and it's always worth reading, but
the message above it is the part to trust.

### Fix from the top

When you get several errors, start with the first one. Errors are
listed in the order they appear in the file, and an early mistake
sometimes explains a later one. Then run `coax check` again. It takes a
fraction of a second, and seeing the new list is faster than reasoning
about which old errors are still real.

## Asking the compiler to explain

Every code has a longer explanation, which you can ask for with
`coax explain`:

```
$ coax explain E0338
E0338: missing return
  Some path reaches the end of a procedure that must return a value.
```

```
$ coax explain E0331
E0331: condition must be bool
  if, while and ifx take a bool. Compare explicitly: x != 0.
```

The first line is the code's short name and the second is the rule
behind it, often with the usual fix. Some explanations are more about
the *why*:

```
$ coax explain E0204
E0204: reserved word not supported yet
  The word is reserved for a planned feature (union, switch, using, try...). coax0 refuses it rather than guessing what it means.
```

Codes are case-sensitive, and an unknown code says so:

```
$ coax explain E9999
coax: no error code 'E9999'
```

`coax explain --all` prints every code, and `coax explain --all
--markdown` prints them as tables. That's how
[Appendix B](appendix-b-error-codes.md) is made, so the appendix and the
compiler always list the same codes.

## How far the compiler gets

`coax0` reads a program in stages, and the first two digits of each code
say which stage found the problem:

| Codes | Stage | What it does | After an error |
| --- | --- | --- | --- |
| `E01xx` | Lexer | Turns characters into tokens | Reports every bad token, then stops before parsing |
| `E02xx` | Parser | Turns tokens into a syntax tree | Stops at the first error |
| `E03xx` | Checker | Names, types, returns, formats | Reports everything it finds |
| `E04xx` | Compile-time evaluation | Runs `#run` and `#assert` (Chapter 9) | Stops at the first failure |
| `E05xx` | Descriptor imports | Reads protobuf descriptors (Chapter 15) | Stops at the first failure |
| `E06xx` | Source loading | Admits the source graph and physical units (Chapter 22) | Stops at the first failure |

A stage only runs if the ones before it succeeded. You can see this with
a program that has a bad character and a missing semicolon:

Filename: stages.coax

```jai
#import "Basic";
main :: () {
  x := 1 $ 2;
  y := 3
  z := 4;
}
```

```
$ coax check stages.coax
stages.coax:3:10: error[E0108]: unexpected character '$'
    x := 1 $ 2;
           ^
1 error
```

Only the lexer's error appears. Fix the `$` and the parser runs and
reports the missing `;` on line 4. So a short error list doesn't always
mean you're nearly done. It may mean the compiler hasn't got that far
yet.

The parser stops at its first error on purpose. After a missing brace or
semicolon, a parser that keeps going has to guess where the next
statement starts, and when it guesses wrong it reports errors that
aren't real. Chasing those wastes time, and an agent fixing errors in a
loop can waste a lot of it. The checker is the opposite. By the time it
runs, the shape of the program is known, so each type error it finds is
a real one, and you want them all at once.

The checker also avoids a different kind of cascade. When it can't work
out a variable's type because of an earlier error, it stays quiet about
later uses of that variable instead of reporting each one. You'll see
that happen in the walkthrough below.

## Fixing a program, end to end

Let's put this to work. Here's a program that's supposed to work out how
many apples are left and how they'd split between friends. It has
several mistakes, of the kind that come up every day:

Filename: pantry.coax

```jai
main :: () {
  apples := 12
  eaten: u8 = 3;
  left := apples - eaten;
  if left { print("% % left\n", plural(left)); }
  for people: 0..4 {
    print("shared by %: % each\n", people, split(left, people));
  }
}

share :: (count: int, people: int) -> int {
  return count / people;
}

plural :: (n: int) -> string {
  if n == 1 { return "apple"; }
  else if n > 1 { return "apples"; }
}
```

*Listing 4-2: `pantry.coax`, before we fix it*

### Round 1: a missing semicolon

```
$ coax check pantry.coax
pantry.coax:2:15: error[E0201]: expected ';' after the declaration, found 'eaten'
    apples := 12
                ^
  fix: add ';'
1 error
```

One error, from the parser, which stopped there. Look at where the caret
is: right after `12` on line 2, even though the parser only noticed
something was wrong when it reached `eaten` on line 3. A missing `;` is
reported where the `;` belongs. The message still tells you what it
found instead (`found 'eaten'`), which helps when the real problem is
something else, like a stray word.

We add the semicolon: `apples := 12;`.

### Round 2: a closed range

```
$ coax check pantry.coax
pantry.coax:6:16: error[E0216]: ranges are half-open: write '..<'
    for people: 0..4 {
                 ^
  fix: for i: 0..<n { }
1 error
```

The parser got further this time and stopped at the next problem. As we
saw in Chapter 3, Coax has only half-open ranges. We meant 0 to 3, so
the loop becomes `for people: 0..<4 {`.

### Round 3: the checker's turn

Now the program parses, and the checker reports everything at once:

```
$ coax check pantry.coax
pantry.coax:4:18: error[E0309]: '-' needs two numbers of one type, found int and u8
    left := apples - eaten;
                   ^
  fix: convert one: cast(int) x
pantry.coax:5:13: error[E0313]: print is in module Basic, which is not imported
    if left { print("% % left\n", plural(left)); }
              ^
  fix: add #import "Basic"; at the top of the file
pantry.coax:5:19: error[E0320]: format has 2 '%' slots but print was given 1 value
    if left { print("% % left\n", plural(left)); }
                    ^
  fix: write '%%' for a literal percent sign
pantry.coax:7:5: error[E0313]: print is in module Basic, which is not imported
      print("shared by %: % each\n", people, split(left, people));
      ^
  fix: add #import "Basic"; at the top of the file
pantry.coax:7:44: error[E0315]: procedure 'split' is not declared
      print("shared by %: % each\n", people, split(left, people));
                                             ^
pantry.coax:15:1: error[E0338]: 'plural' can reach its end without returning string
  plural :: (n: int) -> string {
  ^
  fix: add a return at the end
6 errors
```

Six errors, but not six separate problems. The two `E0313` errors have
one cause, the missing `#import "Basic";`, and the fix line gives the
exact line to add.

The first error is the one Chapter 3 warned about: `apples` is an `int`
and `eaten` is a `u8`, and `-` won't mix them. The fix line suggests
`cast(int)` on one side (the `x` in the fix stands for whichever operand
you pick). That would work, but it treats the symptom. Nothing about a
count of eaten apples needs to be a `u8`, so the better fix is to
declare it like `apples`: `eaten := 3;`.

Notice what's missing from this list. Line 5 has `if left`, and `left`
isn't a `bool`, but there's no `E0331` for it. Because the subtraction
on line 4 failed, the checker doesn't know what type `left` has, so it
doesn't guess. Once we fix line 4, `left` becomes an `int` and we'll see
the error then.

We make both fixes, adding the import at the top and changing `eaten`.

### Round 4: what was hiding

```
$ coax check pantry.coax
pantry.coax:7:6: error[E0331]: condition must be bool, found int
    if left { print("% % left\n", plural(left)); }
       ^
  fix: compare it: x != 0
pantry.coax:7:19: error[E0320]: format has 2 '%' slots but print was given 1 value
    if left { print("% % left\n", plural(left)); }
                    ^
  fix: write '%%' for a literal percent sign
pantry.coax:9:44: error[E0315]: procedure 'split' is not declared
      print("shared by %: % each\n", people, split(left, people));
                                             ^
pantry.coax:17:1: error[E0338]: 'plural' can reach its end without returning string
  plural :: (n: int) -> string {
  ^
  fix: add a return at the end
4 errors
```

The line numbers moved down by two because we added the import and a
blank line. And there's the `E0331` that was hiding. We'll take the four
in order:

1. `if left` needs a `bool`. Coax never treats a number as true or
   false. We want "if any are left", so we write `if left > 0`.
2. The format has two slots, for the count and the word, but we only
   passed the word. Here the fix line's guess about a literal percent
   sign is wrong, and the message is what matters. We add the missing
   value: `print("% % left\n", left, plural(left));`.
3. `split` isn't declared. We named the procedure `share` and then
   called it by a different name. Coax doesn't suggest a near match, so
   when you see `E0315` or `E0306`, check the spelling against the
   declaration. We change the call to `share(left, people)`.
4. `plural` has two `return`s, both inside `if`s. If `n` is zero or
   negative, neither runs. The fix line says "add a return at the end",
   and that's right: the last case should be the default, so we replace
   the `else if` line with `return "apples";`.

### Round 5: it compiles, then it stops

```
$ coax check pantry.coax
$ coax run pantry.coax
9 apples left
pantry.coax:14: division by zero
```

`coax check` printed nothing, which means no errors. Running it gets
partway and then stops. This is a *runtime* error, which the compiler
couldn't have caught: the program divided by zero while running. The
message names the file and the line where it happened, line 14, which
is the `return count / people;` in `share`.

The line where a program stops isn't always where the bug is. `share`
did what it was told. The bug is in the caller: the loop starts at 0,
and sharing between zero people makes no sense. So we fix the loop, not
`share`, and start it at 1:

Filename: pantry.coax

```jai
#import "Basic";

main :: () {
  apples := 12;
  eaten := 3;
  left := apples - eaten;
  if left > 0 { print("% % left\n", left, plural(left)); }
  for people: 1..<4 {
    print("shared by %: % each\n", people, share(left, people));
  }
}

share :: (count: int, people: int) -> int {
  return count / people;
}

plural :: (n: int) -> string {
  if n == 1 { return "apple"; }
  return "apples";
}
```

*Listing 4-3: `pantry.coax`, fixed*

```
$ coax run pantry.coax
9 apples left
shared by 1: 9 each
shared by 2: 4 each
shared by 3: 3 each
```

Nice! Five rounds, and each round's error list told us what to do next.
The order they came in is worth remembering: the lexer and parser report
one problem at a time, so fix and re-run quickly; the checker reports
all of its problems together, so read the whole list and look for
errors that share a cause; and a program that compiles can still stop
at run time, at a line that may not be where the mistake is.

## "Not yet" is different from "no"

Coax reserves the vocabulary for the whole language now, including parts
`coax0` can't compile yet. When you use one of those, the message says
"not supported by coax0 yet", which is different from the message for a
name that doesn't exist:

Filename: types.coax

```jai
main :: () {
  x: flaot = 1.0;
  y: fixed32;
}
```

```
$ coax check types.coax
types.coax:2:6: error[E0302]: unknown type 'flaot'
    x: flaot = 1.0;
       ^
types.coax:3:6: error[E0301]: type 'fixed32' is not supported by coax0 yet
    y: fixed32;
       ^
2 errors
```

`flaot` is a typo, so it's an unknown type. `fixed32` is a real Coax
type that `coax0` doesn't implement, and the compiler says exactly that.
The same goes for planned keywords:

```
modes.coax:5:3: error[E0204]: 'switch' is reserved and not supported by coax0 yet
    switch mode {
    ^
1 error
```

This tells you whether to fix your spelling or change your approach. It
also works as a guarantee: since no program that uses `switch` or
`fixed32` compiles today, none can change meaning when they arrive.
Chapter 21 lists what's planned.

## Errors while compiling your code

Some code runs while the program compiles: constants built with `#run`,
and `#assert` checks, both in Chapter 9. When that code fails, the error
points at the line that failed and then lists how the compiler got
there:

Filename: slots.coax

```jai
#import "Basic";

SLOTS :: #run slots_for(40);

slots_for :: (rooms: int) -> int {
  return capacity(rooms) / (rooms - 40);
}

capacity :: (rooms: int) -> int { return rooms * 8; }

main :: () { print("%\n", SLOTS); }
```

```
$ coax check slots.coax
slots.coax:6:26: error[E0407]: division by zero during compile-time evaluation
    return capacity(rooms) / (rooms - 40);
                           ^
  note: compile-time call at slots.coax:3:15
1 error
```

The `note:` lines are the call stack, innermost first. Here there's one:
the division happened inside `slots_for`, which was called from the
`#run` on line 3. In a deeper chain you'd see one note per call, with the
outermost last. The list is cut off at 64 lines, so a runaway recursion
can't flood the terminal.

Compile-time evaluation only runs on a program the checker accepted. If
your file has type errors and a failing `#assert`, you'll see the type
errors first, and the assertion once they're fixed.

## Errors while the program runs

Round 5 showed a runtime error. `coax0` turns the operations that C
leaves undefined into clean stops, each with the Coax source file and
line, printed to standard error:

```
overflow.coax:6: integer overflow in '+'
bounds_trap.coax:4: index out of bounds
conversions.coax:12: cast out of range
```

The program exits with status 134, the status of an abort, so a script
can tell a crash from a normal failure. Some of these checks are only
in debug builds, and some stay in release builds too:

| What | Debug build | `--release` |
| --- | --- | --- |
| Integer overflow (`+ - *`, unary `-`) | Stops | Wraps |
| Division or `%` by zero | Stops | Stops |
| The smallest `int` divided by `-1` | Stops | Stops |
| Index or slice out of bounds | Stops | Stops |
| `cast(T)` of a value that doesn't fit | Stops | Stops |
| Shift by a negative count, or by the width or more | Stops | Stops |

Overflow wraps in release builds because that's what the hardware does
and checking every addition has a cost. The others stop in both, because
there's no reasonable value to carry on with. Chapter 7 covers handling
failure on purpose, and Chapter 10 covers finding these bugs with checks
before the program ever runs.

## Errors for tools

Everything so far is formatted for a person in a terminal. Tools,
including editors, test scripts and agents, want something they can
parse. Add `--json` and each diagnostic comes out as one JSON object per
line. Here's round 4 of the pantry program again:

```
$ coax check pantry.coax --json
{"file": "pantry.coax", "line": 7, "col": 6, "code": "E0331", "message": "condition must be bool, found int", "fix": "compare it: x != 0"}
{"file": "pantry.coax", "line": 7, "col": 19, "code": "E0320", "message": "format has 2 '%' slots but print was given 1 value", "fix": "write '%%' for a literal percent sign"}
{"file": "pantry.coax", "line": 9, "col": 44, "code": "E0315", "message": "procedure 'split' is not declared", "fix": ""}
{"file": "pantry.coax", "line": 17, "col": 1, "code": "E0338", "message": "'plural' can reach its end without returning string", "fix": "add a return at the end"}
```

Each object has the same six fields: `file`, `line` and `col` (both
counted from 1), `code`, `message`, and `fix`, which is an empty string
when there's no suggestion. There's no count line at the end; count the
lines instead. The exit status is the same as without `--json`: 0 for a
clean file and 1 for errors. Like the text form, the JSON goes to
standard error.

A tool that fixes errors in a loop can work from just these fields: jump
to `file:line:col`, decide by `code` what kind of edit is needed, apply
the `fix` when there is one, then run `coax check` again. Because codes
are stable, that logic doesn't break when the wording of a message
improves. Chapter 19 looks at how agents use this.

> Note: Two things don't come out as JSON yet. A compile-time error's
> `note:` lines are printed after its JSON object as plain text, in the
> same form as the terminal output, so a strict JSON-lines reader should
> skip lines that don't start with `{`. And runtime errors such as
> `division by zero` come from the running program, not the compiler, so
> they're always plain text.

## Summary

A Coax error has a location, a stable code, a message that names what's
involved, the source line with a caret, and sometimes a `fix:` line,
which is a suggestion to weigh against the message. `coax explain CODE`
gives the rule behind any code. The lexer and parser stop early, so
fix-and-rerun is the fastest way through them; the checker reports
everything at once and stays quiet about errors that follow from
earlier ones. "Not supported by coax0 yet" means a planned feature, not
a typo. Runtime errors name the line where the program stopped, which
may not be the line with the bug. And `--json` gives tools the same
information one object per line.

That's the end of Part I. You can write, run and fix small Coax
programs. In Part II we'll start building bigger ones, beginning in
Chapter 5 with structs, enums and unions.
