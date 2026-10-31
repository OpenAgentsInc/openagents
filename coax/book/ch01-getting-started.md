# Getting started

<!-- coax-release:current:start -->
Current compiler: **coax0 0.25.0**. Status inventory schema: **1**.
<!-- coax-release:current:end -->
Let's get Coax running. In this chapter we'll build the compiler, write
the traditional first program, and look at the different ways the
compiler can handle it: running it, building an executable, printing the
C it generates, and checking it without building anything. Along the way
we'll see what happens when a program overflows an integer, and what the
compiler says when we make a mistake.

> Note: Everything in this chapter works with `coax0`, the current
> compiler. Every program shown is in [`examples/`](https://github.com/OpenAgentsInc/openagents/tree/main/coax/book/examples/) with its
> output.

## Building the compiler

The compiler lives in the openagents repository, in the `coax/` folder,
so there's nothing to download or install. You need a C and C++
compiler that supports C++20. On a Mac, Xcode's command line tools are
enough; on Linux, any recent clang or gcc will do.

From the root of the repository, run:

```shell
$ coax/build.sh
```

The script prints nothing when it succeeds. It's doing more than you
might expect, though. Parts of the compiler, including its lexer, parser, type checker
and protocol importer, are themselves written in Coax. So the script
first builds a small "stage 0" compiler, uses it to check and compile
those Coax parts to C, makes sure the result matches the checked-in
copies, and only then links the final compiler and puts it at
`coax/bin/coax`. If any of those checks fail, you don't get a new
compiler. [Chapter 20](ch20-inside-coax0.md) walks through all of this.

Let's make sure it worked:

<!-- coax-release:version-example:start -->
```bash
$ coax/bin/coax version
coax0 0.25.0
```
<!-- coax-release:version-example:end -->

If you see a version, you're set. The rest of the book writes `coax` for
`coax/bin/coax`. To type it that way yourself, add the folder to your
path for this terminal session:

```shell
$ export PATH="$PWD/coax/bin:$PATH"
```

Running `coax` with no arguments prints every command it knows. We'll
meet the important ones in this chapter.

## Hello, world

It's traditional to start a new language with a program that prints
`Hello, world!`, and we'll keep the tradition. Make a folder for your
experiments anywhere you like, and in it create a file called
`hello.coax`. Coax source files always end in `.coax`.

Filename: hello.coax

```jai
#import "Basic";

main :: () {
  print("Hello, world!\n");
}
```

*Listing 1-1: A program that prints `Hello, world!`*

Save the file and run it:

```shell
$ coax run hello.coax
Hello, world!
```

If you see `Hello, world!`, congratulations: you've written a Coax
program. If something went wrong, check the listing carefully against
yours. The compiler will usually tell you exactly what's different, as
we'll see at the end of this chapter.

## Anatomy of a Coax program

Let's go through Listing 1-1 one line at a time.

```jai
#import "Basic";
```

Words that start with `#` are *directives*. They're instructions to the
compiler rather than code that runs when the program does. `#import`
makes a module's names available in this file. `Basic` is the module
that provides `print`; it remains the only named built-in module. `#load`
adds authored source units to a program (Chapter 22).
Nothing is imported unless you ask for it, so the first lines of a file
tell you what it depends on.

```jai
main :: () {
```

This line declares something named `main`. Read `::` as "is defined
as": `main` is defined as a procedure. The `()` is its parameter list,
which is empty here, and the `{` begins its body. There's no return
type, so `main` doesn't return a value. Every Coax program has exactly
one `main`, and it's where the program starts.

This `name :: value` form is how Coax declares every constant thing:
procedures, types and numbers alike. `WIDTH :: 64;` declares a number,
and `Point :: struct { ... }` declares a type. We'll see both in the
next chapter.

> Note: If you know C, notice that the name comes first and there's no
> separate declaration of the procedure's type. If you know Jai, this
> is exactly what you'd expect.

```jai
  print("Hello, world!\n");
```

This line does all the work. It calls `print` with one argument, the
string `"Hello, world!\n"`. The `\n` at the end is a newline character.
Statements end with a semicolon.

The indentation is two spaces, which is the style used throughout the
book and the OpenAgents code. Coax doesn't care about indentation, but
people reading your code will.

```jai
}
```

And this closes the body of `main`.

## Printing values

`print`'s first argument isn't just a string. It's a *format*: each `%`
in it is replaced by the next argument after it. Let's change the
program to print a value:

Filename: hello_name.coax

```jai
#import "Basic";

main :: () {
  name := "Coax";
  print("Hello, %!\n", name);
}
```

*Listing 1-2: Printing a variable with a `%` slot*

```shell
$ coax run hello_name.coax
Hello, Coax!
```

The new line, `name := "Coax";`, declares a *variable* called `name`
and gives it the value `"Coax"`. The `:=` means "declare and assign".
We didn't say what type `name` has, so Coax works it out from the
value: it's a `string`. `print` then replaces the `%` with it.

`%` works for any value Coax knows how to print: numbers, booleans,
strings, and even whole structs and arrays, as we'll see in Chapter 2.
You don't write a different code for each type the way you do with C's
`printf`. And because the format is a literal string, the compiler
counts its `%` slots and checks them against the arguments. If we write
two slots and pass one value:

```jai
  print("Hello, % and %!\n", name);
```

we get an error when compiling, before the program ever runs:

```jai
hello_name.coax:5:9: error[E0320]: format has 2 '%' slots but print was given 1 value
    print("Hello, % and %!\n", name);
          ^
  fix: write '%%' for a literal percent sign
1 error
```

As the fix line says, if you want a literal `%` in the output, write
`%%`.

## Running, building and looking inside

`coax run` is convenient while you're experimenting, but it isn't the
only way to compile a program. Here are the four commands you'll use
most.

### `coax run`

`coax run FILE` compiles the program into a fresh temporary folder, runs
it, and deletes the folder afterward. Nothing is left behind next to
your source.

### `coax build`

`coax build FILE` writes an ordinary executable next to the source file,
named after it:

```shell
$ coax build hello.coax
$ ./hello
Hello, world!
```

Use `-o` to choose the name: `coax build hello.coax -o greet`. On macOS
the driver retains a `hello.o` file: Darwin's debug map needs that object.
Ordinary debug builds include line mappings so a debugger can show your Coax
source lines. Keep the object with the executable, or package the matching
symbols with your native toolchain. Chapter 24 shows source maps and LLDB.

### `coax c`

`coax0` doesn't produce machine code by itself. It translates your
program into a single C file and then hands that file to a C compiler
(`cc`, or whatever the `COAX_CC` environment variable names). `coax c`
stops halfway and prints the C:

```shell
$ coax c hello_name.coax
```

The output is a couple of hundred lines. Most of it is a small runtime
that's written into every generated file: the printing code and the
checked arithmetic you'll meet in a moment. That way the generated C
depends on nothing but the C standard library. The part that comes from
our program is at the end:

```c
static void coax_p_main(void);

static void coax_p_main(void) {
  coax_string l_name = COAX_STR("Coax", 4);
  { coax_put(COAX_STR("Hello, ", 7)); coax_put(l_name); coax_put(COAX_STR("!\n", 2)); }
}

int main(void) {
  coax_p_main();
  return 0;
}
```

*Listing 1-3: The end of the C that `coax c hello_name.coax` prints*

A few things are worth noticing here:

- Our `main` became a `static` C function called `coax_p_main`, and our
  variable `name` became `l_name`. Coax adds prefixes to every name so
  that a Coax name can never collide with a C keyword or a C library
  function.
- A string is a pointer and a byte count (`COAX_STR("Coax", 4)`), not a
  NUL-terminated C string. A Coax string can contain any bytes, and
  asking for its length costs nothing.
- The format string is gone. Because the compiler knew the format while
  compiling, it split it into pieces and wrote one output call per
  piece. There's no format parsing left at run time.
- The real C `main` just calls ours.

You never need to read this output, but you always can. When you're
wondering what a piece of Coax costs, `coax c` will show you.

> Note: Generating C is a deliberate choice for now. C goes through
> toolchains we already trust, like Xcode's clang for the iPhone app,
> and it lets Coax code link directly into existing C and C++ programs.
> [Chapter 21](ch21-road-ahead.md) discusses other backends.

### `coax check`

`coax check FILE` does everything except produce a program: it parses
the file, checks the types, and runs any compile-time code (Chapter 9).
It doesn't call the C compiler, so it's the fastest way to ask "is this
program right?". When the answer is yes, it prints nothing:

```shell
$ coax check hello.coax
$
```

Editors and agents use `coax check` constantly. Add `--json` and each
error comes out as a single line of JSON instead, which is easier for a
tool to read.

### The other commands

You'll meet the remaining commands later in the book. Here's the whole
set for reference:

| Command | What it does |
| --- | --- |
| `coax run FILE` | Compile and run |
| `coax build FILE [-o out]` | Build an executable |
| `coax check FILE` | Check without building |
| `coax c FILE` | Print the generated C |
| `coax test FILE` | Run the file's `#test` and `#forall` checks ([Chapter 10](ch10-writing-checks.md)) |
| `coax mutate FILE [--root-only] [--jobs N] [--shard I/N]` | Check that the checks would catch small bugs, in parallel or in shards ([Chapter 10](ch10-writing-checks.md)) |
| `coax layout FILE` | Print every struct's size, alignment and field offsets ([Chapter 11](ch11-data-layout.md)) |
| `coax lib FILE [-o out.o]` | Build an object file of the `#export_c` procedures, with no `main` ([Chapter 17](ch17-interop.md)) |
| `coax header FILE [-o out.h]` | Write the C header for those procedures ([Chapter 17](ch17-interop.md)) |
| `coax metal FILE` | Print `f32` and `bool` procedures as Metal shader code ([Chapter 17](ch17-interop.md)) |
| `coax proto ...` | Generate Coax from a protocol descriptor ([Chapter 15](ch15-protocol.md)) |
| `coax explain CODE` | Explain an error code ([Chapter 4](ch04-reading-errors.md)) |
| `coax version` | Print the compiler's version |

`run`, `build`, `lib` and `c` also take the legacy `--release` preset.
It selects wrapping runtime integers and release build-profile branches;
native builds also use `-O2`. The next section shows the overflow difference.
Version 0.25.0 also lets you optimize without choosing wrapping arithmetic:
`--opt 2` on `run`, `build` or `lib` keeps the default checked arithmetic.
[Chapter 24](ch24-debugging-and-cost.md) explains the independent selections
and their profile report.

## When a number gets too big

Here's a program that adds one to the largest number an `int` can hold:

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

*Listing 1-4: Adding one to the largest `int`*

The `//` starts a comment, which runs to the end of the line. `big`'s
type is inferred as `int`, a 64-bit signed integer, and `big += 1` is
short for `big = big + 1`. Run it:

```shell
$ coax run overflow.coax
before: 9223372036854775807
overflow.coax:6: integer overflow in '+'
```

The program stopped at line 6 and told us why. In C, overflowing a
signed integer is undefined behavior, and in practice it usually wraps
around to a large negative number without a word. In Coax, an ordinary
build checks every integer operation and stops the program the moment
one overflows. You can see the check in the generated C, where `big +=
1` became:

```c
  l_big = coax_add_s64(l_big, INT64_C(1), 6);
```

`coax_add_s64` adds and checks, and the `6` is the line number it
reports. Now try a release build:

```shell
$ coax run overflow.coax --release
before: 9223372036854775807
after: -9223372036854775808
```

In a release build, the same line becomes `coax_wadd_s64`, which wraps
around instead of stopping. Checking every addition costs time in the
hot loops of a shipped game, so release builds leave it out. The checks
are on while you develop, and off only where you ship.

Some checks stay on in every build, because there's no sensible value
to carry on with: dividing by zero, indexing past the end of an array,
and casting a value to a type it doesn't fit. [Chapter 3](ch03-common-concepts.md)
covers them all.

## When you get it wrong

Let's make some mistakes on purpose. Delete the `#import` line from
`hello.coax` and run it again:

```shell
$ coax run hello.coax
hello.coax:3:3: error[E0313]: print is in module Basic, which is not imported
    print("Hello, world!\n");
    ^
  fix: add #import "Basic"; at the top of the file
1 error
```

Every Coax error has this shape. The first line gives the file, line and
column, a stable error code in brackets, and a sentence saying what's
wrong. Then comes the line itself with a caret under the spot, and, when
the compiler knows one, a line starting with `fix:` that tells you what
to do.

Put the import back, and this time delete the semicolon after the
`print` call:

```jai
hello.coax:4:27: error[E0201]: expected ';' after the statement, found '}'
    print("Hello, world!\n")
                            ^
  fix: add ';'
1 error
```

The caret points just past the end of the call, where the semicolon
should be.

The error codes are stable: `E0313` will mean the same thing next year.
That makes them easy to search for, and it's what lets tools and agents
react to an error without parsing its English. If an error's sentence
isn't enough, ask the compiler to explain the code:

```shell
$ coax explain E0313
E0313: print needs Basic
  print is in module Basic. Add #import "Basic"; at the top of the file.
```

[Appendix B](appendix-b-error-codes.md) lists every code, and
[Chapter 4](ch04-reading-errors.md) is all about reading errors.

## Summary

You've got Coax working. Let's recap what we covered:

- `coax/build.sh` builds the compiler into `coax/bin/coax`, checking its
  own Coax-written parts along the way.
- A Coax program is a `.coax` file with a `main :: () { ... }`
  procedure, and `#import "Basic";` gives it `print`.
- `print` takes a format where each `%` is replaced by the next argument,
  and the compiler checks the count.
- `coax run` compiles and runs, `coax build` makes an executable, `coax
  c` shows the generated C, and `coax check` checks without building.
- Default builds stop on integer overflow; the legacy `--release` preset
  wraps unless an explicit arithmetic selection overrides it.
- Errors come with a location, a stable code, the source line and often
  a fix, and `coax explain` says more about any code.

In the next chapter we'll put these pieces to work on a real program,
and draw a fractal in the terminal.
