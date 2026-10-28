# 2. Hello, world

> **Runs today.** Every program in this chapter compiles with `coax0`.

## 2.1 Building the compiler

The compiler is in the repository, so there is nothing to install. You
need a C++20 compiler (Xcode's clang, or any recent clang or gcc).

```sh
coax/build.sh
```

That builds the C++ frontend as one translation unit, uses it to check and
compile the Coax descriptor importer to C11, then links the final compiler.
The importer and wire runtime must pass their checks and mutation gates before
publishing the compiler at `coax/bin/coax`. Check it:

```sh
coax/bin/coax version
```

```
coax0 0.10.0
```

The rest of this book writes `coax` for `coax/bin/coax`. To type
it that way yourself, add the folder to your path:

```sh
export PATH="$PWD/coax/bin:$PATH"
```

## 2.2 The program

Make a file called `hello.coax`:

```jai
#import "Basic";

main :: () {
  print("Hello, world!\n");
}
```

Run it:

```sh
coax run hello.coax
```

```
Hello, world!
```

## 2.3 What each line means

```jai
#import "Basic";
```

Words that start with `#` are **directives**: instructions to the
compiler rather than code that runs. `#import` brings a module's names
into this file. `Basic` is the module with `print`, and in `coax0` it is
the only module. Nothing is imported unless you ask for it, so a file's
first lines say what it depends on.

```jai
main :: () {
```

This declares a constant named `main` whose value is a procedure. Read
`::` as "is defined as". The `()` is the parameter list, empty here.
There's no return type, so `main` returns nothing. Every program has one
`main`, and it is where the program starts.

The same `name :: value` form declares every constant in Coax:
procedures, types and numbers. Chapter 3 shows the others.

```jai
  print("Hello, world!\n");
```

A call to `print` with one argument, a string. `\n` is a newline, and the
statement ends with a semicolon. `print`'s first argument is a **format**.
Each `%` in it is replaced by the next argument, which you'll see in a
moment.

## 2.4 Ways to compile

`coax run` compiles to a temporary folder, runs the program, and deletes
it. There are three other commands:

```sh
coax build hello.coax            # writes ./hello, an ordinary executable
./hello
coax build hello.coax -o greet   # choose the name
coax c hello.coax                # print the C that coax0 generates
coax check hello.coax            # check it, without calling the C compiler or running it
```

`coax check` is the fastest answer to "is this program right?". It does
everything except generate machine code: parsing, type checking, and the
compile-time work in Chapter 9.

The full set of commands:

| Command | Does |
| --- | --- |
| `coax run FILE` | Compile and run |
| `coax build FILE [-o out]` | Build an executable |
| `coax lib FILE [-o out.o]` | Build an object file of the `#export_c` procedures, with no `main` (Chapter 16) |
| `coax header FILE [-o out.h]` | Write the C header for those procedures |
| `coax c FILE` | Print the generated C (`--library --header out.h` for a library and its header) |
| `coax metal FILE` | Print f32/bool procedures as Metal Shading Language (Chapter 16) |
| `coax check FILE` | Check without building |
| `coax layout FILE` | Print every struct's size, alignment and field offsets (Chapter 10) |
| `coax explain CODE` | Explain an error code (Chapter 4) |
| `coax version` | Print the version |

Add `--release` to any of them for an optimized build. Release builds
change one behavior you can see, in §2.6.

## 2.5 What the compiler makes

`coax0` doesn't produce machine code itself. It writes one C file and
hands it to your C compiler (`cc`, or whatever `COAX_CC` names). That's
deliberate. The C output goes through Xcode and Emscripten exactly as
the engine's C++ does, so Coax code can ship in the iPhone app and the
web build from the first day, with no new toolchain to trust. Our own
machine-code backends come later (Chapter 20).

Here's the end of what `coax c hello.coax` prints:

```c
static void coax_p_main(void);

static void coax_p_main(void)
{
  coax_print(COAX_STR("Hello, world!\n", 14), 0, NULL);
}

int main(void) {
  coax_p_main();
  return 0;
}
```

Above that is a small runtime: the `print` implementation and the
arithmetic checks from the next section. It's written into every output
file, so the C has no dependencies. Notice a few things:

- Coax procedures become `static` C functions with a `coax_p_` prefix, and
  locals get an `l_` prefix, so a Coax name can never collide with a C
  keyword or a library function.
- Strings are a pointer and a byte count (`COAX_STR(..., 14)`), not
  NUL-terminated. A Coax string can contain any bytes, and finding its
  length is free.
- The C `main` just calls the Coax `main`.

You never need to read this output, but you always can. A compiler that
hides nothing is easier to trust.

## 2.6 A second program

```jai
#import "Basic";

main :: () {
  big := 9223372036854775807;   // the largest int
  print("before: %\n", big);
  big += 1;
  print("after: %\n", big);
}
```

`big := ...` declares a variable and infers its type from the value:
`int`, a 64-bit signed integer. The `%` in each format is replaced by
`big`. Run it:

```sh
coax run overflow.coax
```

```
before: 9223372036854775807
overflow.coax:6: integer overflow in '+'
```

The program stopped. In a normal build, integer overflow is an error
that halts the program and says where it happened. In C it would be
undefined behavior, and in practice it would usually wrap silently.
Now try a release build:

```sh
coax run overflow.coax --release
```

```
before: 9223372036854775807
after: -9223372036854775808
```

Release builds wrap instead, because checking every addition costs time
in the hot loops of a shipped game. The rule is the same as the rest of
Coax's checks: **on while you develop, and off only where you ship**.
Division by zero is the exception. It stops the program in every build,
because there's no sensible answer to wrap to.

## 2.7 When you get it wrong

Delete the `#import` line from `hello.coax` and run it again:

```
hello.coax:2:3: error[E0313]: print is in module Basic, which is not imported
    print("Hello, world!\n");
    ^
  fix: add #import "Basic"; at the top of the file
1 error
```

Every Coax error looks like this: the file, line and column, a stable
code, a sentence that says what's wrong, the line itself with a caret,
and a fix when the compiler knows one. Chapter 4 is about reading these.
