# 20. Inside coax0

<!-- coax-release:current:start -->
Current compiler: **coax0 0.24.0**. Status inventory schema: **1**.
<!-- coax-release:current:end -->

The driver embeds the version and status from a generated header. The authored
catalogue and renderer are Coax in [`release.coax`](https://github.com/OpenAgentsInc/openagents/blob/main/coax/release.coax); its
validation model has authority checks and a zero-survivor mutation gate.
`coax status` prints Markdown and `coax status --json` prints inventory schema 1.
Bootstrap checks the generated snapshots and final binary before publication;
app generation and book publication refuse the same drift. Historical feature
introductions keep their own versions. See [the release workflow](https://github.com/OpenAgentsInc/openagents/blob/main/coax/RELEASE.md)
for regeneration and the boundary between consistent metadata and semantic
evidence.
So far we've used `coax` as a black box: we hand it a `.coax` file and get a
program, a C file or an error back. In this chapter we open the box. We'll
take one small program and follow it through every stage of the compiler,
looking at what each stage produces along the way. Where the tool can show
us an intermediate result, we'll ask it to; where it can't, we'll write a
few lines of Coax that call the compiler's own modules and print what they
see.

You don't need to read this chapter to write Coax. It's here for when you
want to fix a bug in the compiler, add a feature, or understand why an error
message points where it does. It also shows that the discipline the earlier
chapters recommend (bounded storage, one owner, checks written next to the
code) is the same discipline the compiler is written in.

> Note: Everything in this chapter describes the current compiler as it is in
> `coax/src/`, `coax/lib/compiler/` and `coax/bootstrap/`.

## A map of the source

`coax0` is two languages working together. The lexer, parser and type checker
are written in Coax and compiled to C. Native bridges copy their inputs and
results; the compile-time evaluator and C emitter remain C++20. A small C++
driver ties them together. Here is where everything lives:

| File | Job |
| --- | --- |
| `lib/compiler/lexer.coax` | Bytes to tokens: keywords, punctuation, comments, escapes, exact number conversion, locations, lexer errors |
| `lib/compiler/parser.coax` | Tokens to an indexed syntax tree: recursive descent, precedence, syntax errors, import admission |
| `src/coax.h` | The declarations every C++ stage shares: tokens, types, values, the syntax tree, the arena |
| `src/lex.cpp` | The bridge from the Coax lexer to C++ `Token`s |
| `src/parse.cpp` | The bridge from the Coax parser's tree to C++ syntax objects |
| `lib/compiler/checker.coax` | Semantic admission: names, types, layouts, numeric decisions, places, returns and diagnostics |
| `src/check.cpp` | Storage and graph transport for the Coax checker, then reconstruction of native checked objects |
| `src/eval.cpp` | Runs `#run`, constants, `#assert` and checks over the checked tree, within budgets |
| `src/emit_c.cpp` | The checked tree to one C11 file with its runtime; headers and layout reports |
| `lib/compiler/compute.coax` | The CUDA compute subset: which types, statements, calls and kernel signatures `coax cuda` admits |
| `src/compute.cpp` | Walks the kernels and what they reach, and reports the policy's refusals (E0700, E0701) |
| `src/verify.cpp` | `coax test` and `coax mutate` |
| `lib/compiler/sources.coax` | Source load syntax, canonical identity/cycles, graph budgets and verification scope |
| `src/sources.cpp` | Filesystem reads and source/diagnostic storage transport |
| `src/proto.cpp` | File reading and buffers for descriptor imports; the importer itself is Coax in `lib/proto/` |
| `src/errors.cpp` | The explanation for every error code (`coax explain`) |
| `src/main.cpp` | The command-line driver, and the file that includes all the others |

Notice the checks alongside the implementation in all four compiler modules.
Each starts with `#authority;`, the directive
from Chapter 10 that says every procedure needs a check and that the
checks run on every build. The compiler's front end is held to the same
rule we asked of the wallet code in `verse/wallet.coax`.

The C++ side is a *unity build*: `main.cpp` `#include`s every other `.cpp`
file, so the whole C++ frontend is one translation unit compiled by one
command. There's no build system to configure.

## Building the compiler

How do you compile a compiler whose lexer is written in the language it
compiles? You start from a copy of that lexer that has already been
compiled. `coax/bootstrap/` holds exactly that: `lexer.c`, `lexer.h`,
`parser.c`, `parser.h`, `checker.c`, `checker.h`, `sources.c` and `sources.h`,
generated from the
`.coax` sources by an earlier
`coax`. These are the *seeds*. They're checked in, and nobody edits them by
hand.

The bootstrap compiles a private source snapshot and checks that it matches the
target inputs before installing artifacts, so a transient edit in the original
checkout cannot become an unnoticed build input. A successful bootstrap also
writes a compiler build record. It binds the compiler and its Coax-checked verifier to a digest of the compiler inputs and
build configuration. A site release checks a copied snapshot against its
clean target tree before staging. A change elsewhere in the repository does
not force a compiler rebuild. The record detects mismatches; it is not a
signature or proof against someone who can replace both the record and the
artifacts, and it does not content-pin the host SDK.

From the repository root, the build is one command:

```shell
$ coax/build.sh
```

It prints nothing when it succeeds. Behind that silence, `coax/build.sh`
does this, stopping at the first failure:

1. Compiles the seed lexer, parser, checker and source-policy C with the C compiler, and links
   them with the C++ frontend into a temporary compiler called *stage0*.
   Stage0 has descriptor imports switched off; it exists only to build the
   real compiler.
2. Uses stage0 to run `coax test` and `coax mutate` on `lexer.coax`. Every
   check must pass and every mutant must be killed.
3. Uses stage0 to compile `lexer.coax` to C again and compares the result
   with the seed, byte for byte. If they differ, the build stops with
   "Lexer seed is stale" instead of quietly updating anything.
4. Repeats steps 2 and 3 for `sources.coax`, `parser.coax` and `checker.coax`,
   then runs the checks and the
   mutation gates for the descriptor importer modules in `lib/proto/`.
5. Loads `lib/compiler/compiler.coax`, whose explicit source graph includes
   the lexer, parser, checker, source policy, fingerprint and importer. It
   compiles that graph to C and links it into the final compiler.
6. Has the final compiler compile those modules again, and requires the
   same C bytes it was built from.
7. Moves the finished binary to `coax/bin/coax`. The move happens only after
   every gate has passed, so a failed build never leaves a half-built
   compiler in place.

The build needs a C compiler, a C++20 compiler and a shell. It doesn't
download anything, and there's no Python step in a normal build.

When you change the lexer, parser, checker or source-policy Coax source, its
seed has to
be refreshed with a compiler you already trust:

```shell
$ coax/refresh-lexer-seed.sh /absolute/path/to/trusted/coax
$ coax/refresh-parser-seed.sh /absolute/path/to/trusted/coax
$ coax/refresh-checker-seed.sh /absolute/path/to/trusted/coax
$ coax/refresh-sources-seed.sh /absolute/path/to/trusted/coax
$ coax/build.sh test
```

The source change and the new seed are committed together.

> Note: A seed that matches its source byte for byte tells you the seed is
> current. It doesn't tell you the compiler that produced it was honest.
> `coax/bootstrap/README.md` is explicit about this: the seeds, the C++
> graph bridges, evaluator and emitter, and the host C toolchain are trusted inputs, and
> passing checks and mutation scores are evidence, not proofs.

## The program we'll follow

Here's the program we'll trace. It has a struct with a default, a
procedure, a constant computed at compile time, and a `print`, which is
enough to exercise every stage.

Filename: ch20_tiny.coax

```jai
#import "Basic";

Pulse :: struct {
    amount: u32;
    hops: u8;
    speed: f32 = 1.5;
}

square :: (x: int) -> int {
    return x * x;
}

AREA :: #run square(7);

main :: () {
    p := Pulse.{amount = 500};
    print("area % pulse % % %\n", AREA, p.amount, p.hops, p.speed);
}
```

*Listing 20-1: The program we'll follow through the compiler*

```shell
$ coax run ch20_tiny.coax
area 49 pulse 500 0 1.5
```

`coax run` does everything this chapter describes, then compiles the C,
runs the program and deletes what it built. In `main.cpp` the front half is
one function, `front_end`, and its body reads like a summary of the
chapter: load the source graph, `lex`, expand any proto imports, `parse`, `check`,
`evaluate`, and, for an `#authority` file, run the checks. Then
`emit_c` writes the C. Let's take those in order.

## Lexing

The lexer turns bytes into tokens. Its entry point is `LX_next` in
`lib/compiler/lexer.coax`. Each call returns one item: a token, a
diagnostic, or both. The caller owns all the storage: the lexer's state is a
one-element `[] LX_State` view, decoded string bytes go into a buffer the
caller passes in, and so do error messages. The lexer never allocates.

There's no command that prints tokens, but we don't need one. `LX_next` is
an ordinary Coax procedure, so we can call it from Coax. Listing 20-2 is a
small `main` that lexes one line of our program, prints each token, then
hands the tokens to the parser and prints the tree it builds. It isn't a
standalone program: it uses `LX_next`, `PS_parse` and their types, so we
load it alongside the compiler's own lexer and parser sources.

Filename: examples/ch20_frontend/driver.coax

```jai
// Not a standalone program. Append it to the compiler's own lexer and parser:
//   cat ../lib/compiler/lexer.coax ../lib/compiler/parser.coax \
//       examples/ch20_frontend/driver.coax > frontend.coax
//   coax run frontend.coax
// main is the only new procedure, and #authority exempts main from coverage.
#import "Basic";

main :: () {
    text := "square :: (x: int) -> int { return x * x; }";

    // Lexing: one LX_next call per token, into storage we own.
    state := [1] LX_State.[LX_begin(text.count)];
    decoded: [64] u8;
    message: [64] u8;
    tokens: [64] PS_Token;
    count := 0;
    while true {
        item := LX_next(text, state[..], decoded[..], message[..]);
        if !item.has_token { continue; }
        t := item.token;
        spelling := text[t.text_begin..<t.text_end];
        print("%:%  %  %\n", t.loc.line, t.loc.column, LX_name(cast(u32) t.kind), spelling);
        // The same copy src/parse.cpp makes before calling the parser.
        loc := PS_Loc.{line = t.loc.line, column = t.loc.column, offset = t.loc.offset};
        tokens[count] = PS_Token.{kind = cast(u32) t.kind, text = spelling, loc = loc,
                                  value = t.int_value, raw_count = spelling.count};
        count += 1;
        if t.kind == LX_Kind.k_eof { break; }
    }

    // Parsing: an indexed tree in nodes we own. Node 0 means "none".
    nodes: [256] PS_Node;
    ps := [1] PS_State.[PS_State.{}];
    out: [600] u8;
    result := PS_parse(text, tokens[0..<count], nodes[..], ps[..], out[..]);
    print("\nparsed: %, % nodes\n", result.ok, ps[0].used);

    // Print the tree depth first, with an explicit stack.
    // ...
}
```

*Listing 20-2: A driver that runs the compiler's lexer and parser on one line (the tree printer is elided; see the file)*

The checked book build now loads the three physical files with `#load`.
To compare with the original single-file walkthrough, from `coax/book`
you can still concatenate them and run the result:

```shell
$ cat ../lib/compiler/lexer.coax ../lib/compiler/parser.coax \
    examples/ch20_frontend/driver.coax > frontend.coax
$ coax run frontend.coax
1:1  identifier  square
1:8  '::'  ::
1:11  '('  (
1:12  identifier  x
1:13  ':'  :
1:15  identifier  int
1:18  ')'  )
1:20  '->'  ->
1:23  identifier  int
1:27  '{'  {
1:29  keyword  return
1:36  identifier  x
1:38  '*'  *
1:40  identifier  x
1:41  ';'  ;
1:43  '}'  }
1:44  end of file
```

That's the real lexer, the one compiled into `coax`, showing us its work.
A few things are worth noticing.

`int` is an *identifier*, not a keyword. Type names aren't special to the
lexer; the checker decides later that `int` names a built-in type. The
keywords are a fixed list of 41 words in `LX_keyword`, including every word
reserved for features that don't exist yet (`switch`, `union`, `using`,
and so on). Reserving them in the lexer is what lets the parser refuse them
with E0204 instead of treating them as names.

Punctuation is matched longest first, so `::` is one token rather than two
colons, and `->` isn't a minus followed by a greater-than.

Every token carries a line, a column and a byte offset. The columns are in
bytes, counted from 1. These locations are what end up in an error message
as `file:line:column`.

The end-of-file token is a real token. The parser relies on it: the token
stream it accepts must end with one.

What the listing doesn't show is the lexer's handling of numbers.
Integer literals accumulate modulo 2^64 with an overflow flag, which is
where E0104 comes from. Decimal literals such as `1.5` are converted to
binary64 exactly, by integer arithmetic on fixed arrays of 32-bit limbs,
keeping up to 800 significant digits and rounding to nearest with ties to
even. There's no call to the C library's `strtod`. The conversion is all
Coax, and it has its own checks at the bottom of `lexer.coax`.

> Note: Our first try at this driver had a small helper procedure,
> `indent`, to print the indentation. The build refused it:
>
> ```
> frontend.coax:5075:1: error[E0380]: 'indent' is not exercised by any #test or #forall
>   indent :: (depth: int) {
>   ^
>   fix: #authority files check every procedure: add a #test or #forall that calls it
> 1 error
> ```
>
> Because we appended our code to an `#authority` file, our code joined
> that file's rules. `main` is exempt from coverage, so the final version
> keeps everything inside `main`.

## Parsing

The second half of the output is the parser's tree:

```jai
parsed: true, 10 nodes
#1 decl_file
  #2 decl_proc square
    #5 type_named int
    #6 stmt_block
      #7 stmt_ret
        #10 expr_binary '*'
          #8 expr_ident x
          #9 expr_ident x
    #3 decl_param x
      #4 type_named int
```

The parser in `lib/compiler/parser.coax` is recursive descent: one
procedure per grammar rule (`PS_stmt`, `PS_type`, `PS_expr`, and so on),
with precedence climbing for binary operators. Like the lexer, it allocates
nothing. It writes nodes into an array the caller provides, and nodes refer
to each other by index. Index 0 means "no node", which is why the driver
treats a zero child as absent.

Each `PS_Node` has the same shape: a `kind` from the `PS_Tag` enum, a
location, a name, four child slots `a` to `d`, and a linked list of further
children through `head` and `next`. The procedure node `#2` uses `a` for its
return type (`#5`), `b` for its body (`#6`) and its list for the parameters
(`#3`). That's why the parameter prints last.

Look at the node numbers. The procedure is `#2`, but its return type is
`#5`, and the `*` expression is `#10`, after both of its operands. Nodes
are numbered in the order the parser *creates* them. It creates the
parameter while reading `(x: int)`, before it has seen the return type, and
it can only create the binary node once it has parsed both sides. The tree
structure, not the numbering, carries the meaning.

A few decisions in the grammar are worth knowing about when you read
`PS_expr` and its neighbors:

- Comparisons can't chain. After parsing `a < b`, if the next operator is
  another comparison, the parser reports E0203 instead of building
  `(a < b) < c`.
- At the top level, `name :: (` could start a procedure or a constant
  whose value is in parentheses. `PS_starts_proc` looks ahead over balanced
  parentheses to decide.
- Recursion is bounded. At most 128 grammar rules can be active at once;
  deeper nesting reports E0221 instead of exhausting the host stack.

The parser stops at the first error. Let's see that. If we delete the
semicolon after `return x * x` in Listing 20-1 and check the file:

```shell
$ coax check tiny.coax
tiny.coax:10:17: error[E0201]: expected ';' after the return, found '}'
      return x * x
                  ^
  fix: add ';'
1 error
```

The caret points just after the `x`, where the semicolon belongs, not at
the `}` on the next line where the parser noticed. The parser keeps each
token's raw spelling length, so it can place the caret right after the
previous token even when that token was a string with escapes in it.

Why stop at one? After a syntax error, the parser's idea of where it is in
the program may be wrong, and errors reported from a wrong position are
noise. The first error freezes the parser's cursor and message, and the
partial tree is thrown away. The checker, as we'll see, takes the opposite
approach.

## The bridge into C++

The native evaluator and emitter use C++ objects. The parser and checker
use indexed Coax graphs. The bridges transport these representations without
implementing a second grammar or semantic checker.

`src/lex.cpp` calls `LX_next` in a loop, exactly like our driver does, and
copies each result into a C++ `Token`. Decoded string bytes are copied into
the compiler's arena before the lexer's scratch buffer is reused. The top of
the file is a long list of `static_assert`s, one per token kind, such as:

```cpp
static_assert((uint8_t)Tok::arrow == COAX_LX_Kind_k_arrow, "lexer token ABI: arrow");
```

If someone adds a token to `LX_Kind` in Coax without adding it to `Tok` in
`coax.h`, the C++ build fails on that line. The two enums can't drift apart.

`src/parse.cpp` calls `PS_parse` and turns the indexed tree into pointers.
It makes three passes over the nodes. The first allocates one C++ object per
node. The second fills in scalar fields and pointers to other objects; since
every object already exists, the order of the nodes doesn't matter. The
third copies the lists (arguments, statements, parameters, fields), which
can only be done once every element is complete. The file has the same kind
of `static_assert`s for the tree's tags.

Neither bridge makes a grammar decision. If you want to change what Coax
accepts, the change goes in the `.coax` files.

The C++ objects live in one `Arena` (in `coax.h` and `main.cpp`): a
bump-pointer allocator that hands out 16-byte-aligned chunks from 1 MB
blocks and never frees anything. The compiler runs once and exits, so
freeing individual nodes would be wasted work. Lists inside nodes are
`Slice<T>`, a pointer and a count, copied into the arena once their final
length is known. Strings are `Str` views into the source text or the
arena. This is the arena pattern from Chapter 6, used for real.

## Checking

The checker in `lib/compiler/checker.coax` decides whether the program means
something. `src/check.cpp` transports the parsed graph into caller-owned
`CK_Node`, `CK_Type`, local, diagnostic and byte arrays, calls the Coax
checker, then reconstructs native typed objects only after success. It has
no native semantic fallback. Storage and active semantic entries are
bounded; E0381 reports those resource limits.

The checker works in passes over the declarations, in this order:

1. Imports. `coax0` knows one module, `Basic`.
2. Type names for every struct and enum, then a check that no two
   top-level things share a name, then enum members and struct layouts.
3. Procedure signatures. This is why you can call a procedure that's
   declared further down the file: every signature is known before any body
   is checked.
4. Constants, whose types are resolved on demand, so they can refer to
   each other in any order (and a cycle is E0340).
5. Struct field defaults and top-level `#assert`s.
6. `main`: it must exist (unless the file is a library) and take nothing.
7. Procedure bodies, then `#test` and `#forall` bodies.
8. For an `#authority` file, coverage; for `coax metal`, the Metal profile.

Within a body, a few design choices explain a lot of the errors you've met:

- Types are *interned* in an indexed type table. Equal types share an ID;
  the native bridge reconstructs corresponding `TypeInfo` objects. `[4] int`
  and `[] u8` are created when needed.
- Literals are *untyped* until a use gives them a type. The Coax `CM_*`
  helpers represent wide integer magnitudes and IEEE bit patterns explicitly
  for numeric folding and range decisions. Native conversion is not a
  second semantic implementation.
- Locals live in caller-owned flat storage, with a mark saved at the start
  of each block. Leaving a block truncates the vector back to the mark. Looking a
  name up scans backwards, and finding the name already in scope when
  declaring is E0303, because Coax has no shadowing.
- Assignment targets are checked as *places*: a local, a field of a place,
  or an element of a place or view. Parameters, constants and loop
  variables aren't places.
- "Every path returns" is a small structural rule: a block returns if any
  statement in it returns, and an `if` returns if both branches do.
- `print` is special-cased. The checker counts the `%` slots in the literal
  format string and compares them with the arguments.

Unlike the parser, the checker keeps going after an error, so one run shows
you everything it can find. Let's break Listing 20-1 in three unrelated
places: give `hops` the default `300`, make `square` return `x * 2.5`, and
drop the last argument to `print`.

```shell
$ coax check tiny.coax
tiny.coax:5:16: error[E0350]: 300 does not fit in u8
      hops: u8 = 300;
                 ^
  fix: use a wider type, or cast,trunc(u8) to keep the low bits
tiny.coax:10:16: error[E0350]: a float literal cannot become int
      return x * 2.5;
                 ^
  fix: write an integer, or cast(int) a float value
tiny.coax:17:11: error[E0320]: format has 4 '%' slots but print was given 3 values
      print("area % pulse % % %\n", AREA, p.amount, p.hops);
            ^
  fix: write '%%' for a literal percent sign
3 errors
```

All three come from the rules above: the literal `300` met the type `u8`
and didn't fit; the literal `2.5` met `int` and can't become one; and the
format's four slots didn't match three values. Every message is built by a
call to `CK_error` with a code, the message and, where the checker can suggest
one, a fix. `src/errors.cpp` holds the longer explanation for each code,
which is what `coax explain` prints and what Appendix B is generated from.

## Evaluating

After checking, `src/eval.cpp` runs the program's compile-time work: struct
field defaults, constants, top-level `#assert`s, and `#run` expressions
inside procedure bodies. In our program that's `speed: f32 = 1.5` and
`AREA :: #run square(7)`.

The evaluator walks the checked syntax tree directly. There's no bytecode;
the spec says to add one only if profiling shows the tree walk is too slow.
A compile-time `Value` is a type plus an integer bit pattern, a float, a
string, or shared storage for a struct's fields or an array's elements.
A `Value` never holds a host address, which is how the compiler guarantees
that none of its own memory can end up in your program.

Every step of evaluation calls `tick`, which counts against a budget of
100,000 steps per compilation by default; `--run-steps N` selects a finite
limit up to 250,000,000. Zero-initializing an array element costs a step too;
deep-copy, assignment and view-detection traversal is not charged per element. The
independent aggregate storage cap counts live capacity and containers, not
total process memory.
Calls and constant dependencies are capped at a depth of 64. Arithmetic
uses the same overflow, bounds, cast and shift rules as the generated C,
except that overflow is always an error, in debug and release alike. We can
see that by asking for a square that doesn't fit, `square(4_000_000_000)`:

```shell
$ coax check tiny.coax
tiny.coax:10:14: error[E0407]: integer overflow in int during compile-time evaluation
      return x * x;
               ^
  note: compile-time call at tiny.coax:13:14
1 error
```

The `note:` line is the evaluator's call trace: the error happened inside
`square`, called from the `#run` on line 13. The trace refers to your source,
never to the evaluator's own C++.

Compile-time code also can't do I/O. If we add a `print` to `square`:

```jai
square :: (x: int) -> int {
    print("squaring %\n", x);
    return x * x;
}
```

the checker is happy, because a `print` in `square` is fine at run time.
It's the evaluator that refuses, when `#run` tries to execute it:

```shell
$ coax check tiny.coax
tiny.coax:10:5: error[E0404]: compile-time execution cannot perform output or host I/O
      print("squaring %\n", x);
      ^
  note: compile-time call at tiny.coax:14:14
1 error
```

The evaluator stops at its first error, like the parser. Once evaluation
has failed, the values it would compute next can't be trusted.

When a constant or a `#run` succeeds, the evaluator *freezes* the result:
it replaces the expression in the tree with the value. From then on, the
emitter sees `49` where we wrote `#run square(7)`. Each constant is
evaluated once and remembered.

## Emitting C

Now the tree is checked and every compile-time value is known, and
`src/emit_c.cpp` writes one C11 file. `coax c` prints it:

```shell
$ coax c ch20_tiny.coax | head -2
/* Generated by coax0 from ch20_tiny.coax. Do not edit: change the .coax file. */
static const char *coax_source_path = "ch20_tiny.coax";
```

The file for our 18-line program is 210 lines, and almost all of it is
the runtime that every program carries. In order, the file contains:

1. A header comment and the source path, which runtime traps print.
2. `#pragma STDC FP_CONTRACT OFF`, so the C compiler can't fuse a multiply
   and an add into one instruction and change a float result.
3. The runtime: `coax_string`, the print helpers, checked and wrapping
   arithmetic for every integer width, and bounds, cast and shift checks.
4. Every type, with layout assertions.
5. A prototype for every procedure, so declaration order never matters in C
   either.
6. Each procedure's body.
7. A C `main` that calls the Coax `main`, unless the file is a library.

Here's everything after the runtime, which is the part generated from our
code:

```c
/* Types */
typedef struct coax_t_Pulse coax_t_Pulse;
struct coax_t_Pulse { uint32_t amount; uint8_t hops; float speed; };
COAX_LAYOUT(sizeof(coax_t_Pulse) == 12 && _Alignof(coax_t_Pulse) == 4, "layout of Pulse");
COAX_LAYOUT(offsetof(coax_t_Pulse, amount) == 0, "offset of Pulse.amount");
COAX_LAYOUT(offsetof(coax_t_Pulse, hops) == 4, "offset of Pulse.hops");
COAX_LAYOUT(offsetof(coax_t_Pulse, speed) == 8, "offset of Pulse.speed");

static coax_t_Pulse coax_init_t_Pulse(void);
static coax_t_Pulse coax_init_t_Pulse(void) {
  coax_t_Pulse v;
  memset(&v, 0, sizeof v);
  v.speed = 0x1.8p+0f;
  return v;
}
static void coax_show_t_Pulse(const coax_t_Pulse *v);
static void coax_show_t_Pulse(const coax_t_Pulse *v) {
  fputs("Pulse.{", stdout);
  fputs("amount = ", stdout); coax_print_uint(v->amount);
  fputs(", hops = ", stdout); coax_print_uint(v->hops);
  fputs(", speed = ", stdout); coax_print_float(v->speed, 1);
  fputc('}', stdout);
}

static int64_t coax_p_square(int64_t l_x);
static void coax_p_main(void);

static int64_t coax_p_square(int64_t l_x) {
  return coax_mul_s64(l_x, l_x, 10);
}

static void coax_p_main(void) {
  coax_t_Pulse l_p = ((coax_t_Pulse){.amount = ((uint32_t)UINT64_C(500)), .speed = 0x1.8p+0f});
  { coax_put(COAX_STR("area ", 5)); coax_print_int(INT64_C(49)); coax_put(COAX_STR(" pulse ", 7)); coax_print_uint((l_p).amount); coax_put(COAX_STR(" ", 1)); coax_print_uint((l_p).hops); coax_put(COAX_STR(" ", 1)); coax_print_float((l_p).speed, 1); coax_put(COAX_STR("\n", 1)); }
}

int main(void) {
  coax_p_main();
  return 0;
}
```

*Listing 20-3: The C that coax0 generates for Listing 20-1, after the runtime*

Let's read it from the top.

The struct becomes `coax_t_Pulse`. Coax names get prefixes in C:
`coax_t_` for types, `coax_p_` for procedures and `l_` for locals, so a
Coax local called `int` or `main` can't collide with anything in C. An
`#export_c` procedure keeps its own name; that's the point of exporting it.

The `COAX_LAYOUT` lines are `_Static_assert`s. The checker computed the
layout of `Pulse`, and these lines make the C compiler confirm it agrees.
The checker's numbers are the same ones `coax layout` prints:

```shell
$ coax layout ch20_tiny.coax
Pulse  size 12  align 4
       0     4  amount: u32
       4     1  hops: u8
       5     3  (padding)
       8     4  speed: f32
```

If the C compiler ever laid out the struct differently, the build would
fail rather than produce a program where Coax and C disagree about where
`speed` lives.

`coax_init_t_Pulse` builds a `Pulse` with every field zero except the
defaults, and `coax_show_t_Pulse` prints a whole struct with `%`. Our
program uses neither (the literal in `main` fills `speed` itself), but the
emitter writes them for every struct. That's also why the runtime turns off
warnings about unused functions.

The float `1.5` is written as `0x1.8p+0f`, C's hexadecimal float notation.
Hex floats are exact: the C compiler reads back the same bits the Coax
lexer computed, with no second decimal conversion that could round
differently.

`x * x` became `coax_mul_s64(l_x, l_x, 10)`. That's the checked multiply
from the runtime, and `10` is the source line it reports if it traps. Here
is its definition, one of a family generated for every integer width:

```c
static T coax_mul_##N(T a, T b, int32_t l) { T r; if (__builtin_mul_overflow(a, b, &r)) coax_trap("integer overflow in '*'", l); return r; }
```

A release build uses the wrapping form instead. We can see the only
difference with `diff`:

```shell
$ diff <(coax c ch20_tiny.coax) <(coax c ch20_tiny.coax --release)
199c199
<   return coax_mul_s64(l_x, l_x, 10);
---
>   return coax_wmul_s64(l_x, l_x);
```

The wrapping form also goes through `__builtin_mul_overflow`, and ignores
the flag. Signed overflow in plain C is undefined behavior; spelled this
way, it's a defined wrap.

`AREA` is gone. In its place is `INT64_C(49)`, the value the evaluator
froze. The compiled program never calls `square(7)`, and the evaluator
itself is nowhere in the output.

The `print` call has been split up while compiling. The format string was
a literal, so the emitter cut it at each `%` and wrote a `coax_put` for each
piece of text and a typed print call for each value: `coax_print_int` for
`AREA`, `coax_print_uint` for the unsigned fields, `coax_print_float` for
the `f32`. At run time there's no format string to parse. Floats print the
shortest text that reads back as the same value, which is why `1.5` prints
as `1.5`.

### Two lowerings worth knowing

Two rules from earlier chapters need more than a direct translation. To see
both, here's a program with `defer` and with calls as arguments:

Filename: ch20_defer_order.coax

```jai
#import "Basic";

f :: (n: int) -> int {
    defer print("one\n");
    defer print("two\n");
    if n > 0 { return n + 1; }
    return 0;
}

g :: (a: int, b: int) -> int {
    return a - b;
}

main :: () {
    x := g(f(1), f(0));
    print("%\n", x);
}
```

*Listing 20-4: Deferred statements and argument order*

```shell
$ coax run ch20_defer_order.coax
two
one
two
one
2
```

Here's what `f` and `main` become:

```c
static int64_t coax_p_f(int64_t l_n) {
  int64_t coax_ret;
  if (l_n > INT64_C(0))
  {
    coax_ret = coax_add_s64(l_n, INT64_C(1), 6);
    { coax_put(COAX_STR("two\n", 4)); }
    { coax_put(COAX_STR("one\n", 4)); }
    return coax_ret;
  }
  coax_ret = INT64_C(0);
  { coax_put(COAX_STR("two\n", 4)); }
  { coax_put(COAX_STR("one\n", 4)); }
  return coax_ret;
  { coax_put(COAX_STR("two\n", 4)); }
  { coax_put(COAX_STR("one\n", 4)); }
}
static void coax_p_main(void) {
  int64_t coax_t0;
  int64_t coax_t1;
  int64_t l_x = (coax_t0 = coax_p_f(INT64_C(1)), coax_t1 = coax_p_f(INT64_C(0)), coax_p_g(coax_t0, coax_t1));
  { coax_print_int(l_x); coax_put(COAX_STR("\n", 1)); }
}
```

*Listing 20-5: The C for Listing 20-4*

`defer` has no C equivalent, so the emitter copies the deferred statements
to every exit of the block, in reverse order. Each `return e` first stores
`e` in `coax_ret`, then runs the deferred code, then returns. That ordering
is why a deferred statement can't change what's returned. The last copy,
after the final `return`, is the block's fall-through exit; it's
unreachable here, and harmless.

C doesn't define the order in which a function's arguments are evaluated.
Coax does: left to right (Chapter 3). So when an argument list contains a
call and another operand that could call or trap, the emitter evaluates
them into temporaries, in source order, inside one C comma expression. Both
calls to `f` print, so if C evaluated `f(0)` first we would see it in the
output. We don't: `f(1)` runs first, `f(0)` second, every time, with any C
compiler.

Strings are emitted with their escapes. Bytes that aren't printable are
written as three-digit octal escapes (a NUL becomes `\000`), so an escaped
byte can never merge with a digit that follows it.

### Libraries and headers

For a file built with `coax lib` or `coax header`, the same emitter skips
the C `main`, gives `#export_c` procedures their own names and the C
calling convention, and writes a header with the same layout assertions,
usable from both C and C++. Chapter 17 covers that side.

## Building and running

The last step isn't the compiler's at all. `coax build` writes the C next to
the output and runs the host C compiler on it: `cc` by default, or whatever
`$COAX_CC` names, with `-O0 -g` for a debug build or `-O2` for `--release`.
Normally the `.c` file is deleted afterwards; `--keep-c` keeps it:

```shell
$ coax build ch20_tiny.coax --keep-c
$ ls -1
ch20_tiny
ch20_tiny.c
ch20_tiny.coax
ch20_tiny.dSYM
$ ./ch20_tiny
area 49 pulse 500 0 1.5
```

(The `.dSYM` folder is the debug information `cc -g` writes on macOS.) If
the C compiler rejects the generated code, `coax` says so with "this is a
coax bug". The checker's job is to make sure that never happens, so if
you see it, please report it.

`coax run` does the same in a fresh temporary directory, runs the program,
and deletes the directory.

## Tests

`coax/build.sh test` builds the compiler and then runs `coax/tests/run.sh`:

- Every `tests/pass/NAME.coax` must print exactly `NAME.out`, in a debug
  build and in a release build.
- Every `tests/fail/NAME.coax` starts with a comment such as
  `// expect: E0350`, and must fail with that code; or with
  `// expect: trap TEXT`, and must compile, then stop at run time printing
  `TEXT`.
- The examples in `coax/examples/` with a `.out` file run in both modes,
  and `examples/pulse/run.sh` links a Coax kernel into a C host and
  compares it with a C reference.
- Python test files (`tests/*.py`) cover compile-time evaluation, the
  tools (`explain`, `--json`, `layout`, headers compiled as C and as C++),
  the lexer, parser and checker through their actual native bridges against
  independent oracles, sanitizer runs and fuzzing,
  descriptor imports, and `coax test` and `coax mutate` themselves.
- The checks inside the Coax modules run with `coax test`, and the
  mutation gates with `coax mutate`.

> Note: Python is needed to run the test suite, not to build or use the
> compiler.

You can run a module's checks yourself:

```shell
$ coax test coax/lib/compiler/lexer.coax
...
ok   #test decimal records retain bounded precision and a sticky remainder (1 case, 4426239 steps)
222 checks, 222 passed, 0 failed
$ coax mutate coax/lib/compiler/lexer.coax
1069 mutants, 1069 killed, 0 survived
```

The parser, checker and source loader have their own checks and mutation gates too. Run
`coax test coax/lib/compiler/checker.coax` and `coax mutate` on that file
to inspect the current results. `coax/tests/checker.py` adds independent
IEEE, layout and policy expectations plus native sanitizer coverage.
This book's `check-examples.sh` runs its examples in debug and release,
executes authored checks, and exercises the native boundary examples.

When you add a feature, it comes with a passing program, a failing program
for each new error code, and an entry in `errors.cpp` for each code. When
the feature belongs in the lexer, parser, checker or source policy, it comes with
checks in the `.coax` file that survive `coax mutate`, and a refreshed seed.

## Summary

We followed one program through `coax0`. The Coax lexer turned bytes into
tokens with locations; the Coax parser built an indexed tree in storage it
didn't own, and stopped at the first syntax error. Native bridges
transported those results and reconstructed arena-allocated objects. The Coax checker resolved
every name and type and reported every error it could find. The evaluator
ran `#run square(7)` within its budget and froze the result into the tree.
The emitter wrote one self-contained C file, with layout assertions,
checked arithmetic and a format string already split into typed calls, and
the host C compiler did the rest.

Along the way we saw the compiler's own rules at work: `#authority` on its
front end, seeds that must regenerate byte for byte, and a build that only
publishes a compiler after every gate passes. Chapter 21 looks at what
comes next for the compiler and the language.

## Layout metadata lowering

The Coax parser recognizes type/field/enum queries and supplies schema-1
record declarations once per source graph. The Coax checker resolves the
operand's CPU layout and lowers the result into ordinary typed literals.
A dynamic selector becomes a bounded metadata array and ordinary indexing;
a direct scalar projection can fold while resolving an array length.
The C++ bridge allocates native tree storage and copies those nodes, including
field initializers and Coax-owned synthetic-node provenance. The mutator
uses that provenance to exclude compiler-created metadata literals; authored
selectors and surrounding logic remain mutation sites. This is not an
equivalent-mutant exemption. The bridge makes no reflection decisions. The existing evaluator
and C emitter handle the resulting structs, arrays and bounds checks.

`tests/reflection.py` compares padding, array stride and offsets against
independently authored C declarations under ASan/UBSan. The parser and checker
include authority checks and mutation gates for the new paths. Verse's
Haunted House buffer admission derives its stride and scalar table through
these queries. Metal's packed vertex declaration is still an explicit
platform boundary, not a second reflection target.

## Observing compilation and native execution

Version 0.13.0 adds original-source `#line` output, schema-1 hashed name/location
maps and host phase reports. `--timings FILE` observes load, lex, imports,
parse, check, evaluate, authority checks and emit; native builds also separate
C compilation from linking. The monotonic clock lives in the driver. It is
not exposed to `#run` and does not change compile-time determinism.

The mapping emitter and timing driver remain part of the trusted C++ compiler
boundary. Source hashes reuse the checked Coax fingerprint implementation.
Debug builds can now be stepped through Coax files, with an optional LLDB
helper presenting authored names; Chapter 24 gives the runnable example.
A hashed map detects stale files, not a compromised compiler or false oracle.
