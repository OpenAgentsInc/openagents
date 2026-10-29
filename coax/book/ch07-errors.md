# 7. Errors and cleanup

Programs fail. A file isn't where we expected it, a user types `80a0`
where a port number should go, a loop runs one step too far. In this
chapter we'll look at how Coax tells those failures apart and what it
gives us for each. We'll grow one small program, a parser for network
addresses like `localhost:8080`, from a version that falls over on bad
input to one that reports every failure clearly, and along the way we'll
meet traps, error values, `defer`, and the rules for choosing between
them.

Coax sorts failures into two kinds. *Expected failures* are things that
happen to correct programs: input that doesn't parse, a request that was
refused, a value that's too large. Code is supposed to handle them, so
Coax treats them as ordinary values that a procedure returns. *Bugs* are
things that only happen when the program itself is wrong: arithmetic
that overflows, an index past the end of an array, an invariant that no
longer holds. Continuing after a bug means computing with nonsense, so a
bug stops the program with a *trap*.

There are no exceptions in Coax. Nothing unwinds the stack behind your
back, and a line that can fail says so where you can see it.

> Note: `coax0` today runs traps, `defer`, `#assert` in procedure bodies,
> and the rule that a returned result must be used. The dedicated error
> syntax from the spec (optional types `T?`, `try`, `or_else` and
> multiple results, spec §8 and §11.3) isn't implemented yet. Everything
> in this chapter that runs is checked against `coax0`; the planned parts
> are marked as such, and we'll see what the compiler says when you try
> them.

## A parser that trusts its input

Let's start with the smallest thing that could work: a procedure that
turns the text of a port number into an `int`.

Filename: examples/ch07_ports1.coax

```jai
#import "Basic";

parse_port :: (text: string) -> int {
  value := 0;
  for c: text {
    value = value * 10 + cast(int) (c - '0');
  }
  return value;
}

main :: () {
  print("%\n", parse_port("8080"));
  print("%\n", parse_port("80a0"));
  print("%\n", parse_port("80 80"));
}
```

*Listing 7-1: A port parser that assumes every character is a digit*

A string iterates as its bytes, so `c` is a `u8`. A character literal
like `'0'` is just an integer (48), so `c - '0'` is the digit's value
when `c` is a digit. We widen it to `int` and add it to ten times what
we had so far. Run it:

```shell
$ coax run examples/ch07_ports1.coax
8080
8490
examples/ch07_ports1.coax:6: integer overflow in '-'
```

The first line is right. The second is wrong in the worst way: `'a'` is
97, so `'a' - '0'` is 49, and the parser quietly folded a letter into
the number and printed `8490`. Nothing told us anything went wrong.

The third call is more interesting. A space is byte 32, which is less
than 48, so `c - '0'` would be negative. `c` is a `u8`, and an unsigned
subtraction that goes below zero overflows. Coax checks integer
arithmetic in a normal build, so the program stopped right there, with
the file, the line and what happened. That's a trap.

The trap is doing its job: it stopped a computation that had gone wrong
before it could produce a wrong answer. But it fired by accident, on one
kind of bad input and not another, and it took the whole program down
over what is really a typo in a configuration file. Build the same
program with `--release` and the picture gets worse:

```shell
$ coax run examples/ch07_ports1.coax --release
8080
8490
104080
```

In a release build, integer overflow wraps instead of trapping, so the
space became a huge "digit" and we got `104080`, which isn't even a
valid port. We'll come back to why release builds treat overflow this
way. For now the lesson is that our parser has no idea what to do with
input it doesn't expect, and we need to decide what it *should* do.

## Stopping on purpose with `#assert`

One option is to say plainly what we expect and stop when it isn't so.
`#assert` inside a procedure checks a `bool` at run time and traps if
it's false:

Filename: examples/ch07_ports2.coax

```jai
#import "Basic";

parse_port :: (text: string) -> int {
  #assert text.count > 0;
  value := 0;
  for c: text {
    #assert c >= '0' && c <= '9';
    value = value * 10 + cast(int) (c - '0');
  }
  return value;
}

main :: () {
  print("%\n", parse_port("8080"));
  print("%\n", parse_port("80a0"));
  print("never printed\n");
}
```

*Listing 7-2: Trapping on anything that isn't a digit*

```shell
$ coax run examples/ch07_ports2.coax
8080
examples/ch07_ports2.coax:7: assertion failed
```

That's better than Listing 7-1. The parser can no longer return a wrong
number, and the trap points at the exact rule that was broken. Unlike
overflow, an `#assert` stays on in every build mode, so `--release`
gives the same result.

> Note: `#assert` also appears at the top level of a file, where it's
> checked while compiling. That form belongs to [Chapter 9](ch09-compile-time.md).
> Inside a procedure it's an ordinary run-time check.

But think about who gets hurt. A port number usually comes from outside
the program: a config file, a command line, a form someone filled in.
When that text is wrong, our program isn't broken. The *input* is wrong,
and a correct program should say so and carry on, or at least stop with
a message meant for a person rather than a line number meant for us. A
trap is the right response to a bug. It's the wrong response to bad
input.

## Returning errors as values

So let's make failure part of what `parse_port` returns. First we name
the ways parsing can fail, with an enum:

```jai
Port_Error :: enum u8 { none; empty; not_a_digit; out_of_range; }
```

Then we return the port and the error together in a struct:

Filename: examples/ch07_ports3.coax

```jai
#import "Basic";

Port_Error :: enum u8 { none; empty; not_a_digit; out_of_range; }

Parsed_Port :: struct {
  value: u16;
  error: Port_Error;
}

parse_port :: (text: string) -> Parsed_Port {
  if text.count == 0 { return Parsed_Port.{error = Port_Error.empty}; }
  value := 0;
  for c: text {
    if c < '0' || c > '9' { return Parsed_Port.{error = Port_Error.not_a_digit}; }
    value = value * 10 + cast(int) (c - '0');
    if value > 65535 { return Parsed_Port.{error = Port_Error.out_of_range}; }
  }
  if value == 0 { return Parsed_Port.{error = Port_Error.out_of_range}; }
  return Parsed_Port.{value = cast(u16) value};
}

main :: () {
  inputs := [5] string.["8080", "80a0", "", "70000", "443"];
  for text: inputs {
    p := parse_port(text);
    if p.error != Port_Error.none {
      print("\"%\": %\n", text, p.error);
      continue;
    }
    print("\"%\": port %\n", text, p.value);
  }
}
```

*Listing 7-3: `parse_port` returns its error alongside its value*

Let's walk through the new parts. The result type is now `Parsed_Port`.
Each early `return` builds one with a struct literal that names only the
field it sets; the other field takes its default, which for an integer
is zero and for an enum is the member whose value is zero, `none`. The
last line returns a success: a value and no error.

We check each digit *before* using it, so `c - '0'` can never go below
zero any more, and we check the running value against 65535 after each
digit, so it can never get anywhere near overflowing an `int`. Port 0
isn't a port you can connect to, so we reject it too. Once the loop
finishes, `value` fits a `u16`, and `cast(u16) value` says so. (If our
reasoning were wrong, that `cast` would trap with `cast out of range`
rather than hand back a truncated port.)

In `main`, the caller looks at `p.error` before it looks at `p.value`.
Run it:

```shell
$ coax run examples/ch07_ports3.coax
"8080": port 8080
"80a0": not_a_digit
"": empty
"70000": out_of_range
"443": port 443
```

Every input gets an answer, and every answer is honest. Printing an enum
prints the member's name, which is handy for a demo; a real program
would turn each member into a message for whoever wrote the bad input.

### Make zero mean "no error"

We put `none` first on purpose. A field we don't set in a literal, or a
variable declared without a value, is zeroed, and if zero isn't a member
of the enum, you get a value that names nothing. Here's what happens if
`Port_Error` starts at 1:

```jai
Port_Error :: enum u8 { empty = 1; not_a_digit; out_of_range; }
// ...
  p := Parsed_Port.{value = 80};
  print("%\n", p);
```

```jai
Parsed_Port.{value = 80, error = Port_Error(0)}
```

`Port_Error(0)` is how `print` shows a value with no member, and the
check `p.error != Port_Error.empty` and friends will all say "not that
one" without ever saying "fine". Give the "no error" case the value
zero and every default means success.

### You can't drop a result

What if a caller forgets to look at the result at all? Copy Listing 7-3
to `ports.coax` and add a call that throws its result away:

```jai
main :: () {
  inputs := [5] string.["8080", "80a0", "", "70000", "443"];
  parse_port("8080");
  // ...
}
```

```shell
$ coax check ports.coax
ports.coax:24:3: error[E0323]: the result of 'parse_port' (Parsed_Port) is not used
    parse_port("8080");
    ^
  fix: use it: result := parse_port(...);
1 error
```

A procedure that returns a value must have that value used, so the
error inside it can't be thrown away silently. Be clear about what this
does and doesn't promise. `coax0` makes sure you *received* the result.
It doesn't yet make sure you looked at `error` before reading `value`;
`p := parse_port(text); print("%\n", p.value);` compiles. The planned
optional types in the last section of this chapter close that gap.

## Passing errors along

A port is usually half of something bigger. Let's parse a whole address,
`host:port`, using `parse_port` for the second half. The new procedure
has failures of its own (no colon, nothing before the colon) and it can
also fail because the port inside it failed. Here's the whole program:

Filename: examples/ch07_address.coax

```jai
#import "Basic";

Port_Error :: enum u8 { none; empty; not_a_digit; out_of_range; }

Parsed_Port :: struct {
  value: u16;
  error: Port_Error;
}

parse_port :: (text: string) -> Parsed_Port {
  // ... unchanged from Listing 7-3
}

Address_Error :: enum u8 { none; no_colon; empty_host; bad_port; }

Address :: struct {
  host: string;
  port: u16;
  error: Address_Error;
  port_error: Port_Error;     // set when error is bad_port
}

parse_address :: (text: string) -> Address {
  colon := -1;
  for c, i: text {
    if c == ':' { colon = i; }
  }
  if colon < 0 { return Address.{error = Address_Error.no_colon}; }
  if colon == 0 { return Address.{error = Address_Error.empty_host}; }

  port := parse_port(text[colon + 1..<text.count]);
  if port.error != Port_Error.none {
    return Address.{error = Address_Error.bad_port, port_error = port.error};
  }
  return Address.{host = text[0..<colon], port = port.value};
}

main :: () {
  checked := 0;
  defer print("% addresses checked\n", checked);

  inputs := [6] string.["localhost:8080", "example.com:443", "localhost", ":80", "db:5432x", "db:"];
  for text: inputs {
    checked += 1;
    a := parse_address(text);
    if a.error == Address_Error.bad_port {
      print("%: bad port (%)\n", text, a.port_error);
    } else if a.error != Address_Error.none {
      print("%: %\n", text, a.error);
    } else {
      print("%: host %, port %\n", text, a.host, a.port);
    }
  }
}
```

*Listing 7-4: `parse_address` reports its own errors and passes the port's along*

`parse_address` finds the last colon (so a host that contains colons
still splits at the port), checks its own two failures, and then hands
everything after the colon to `parse_port`. Slicing a string with
`text[lo..<hi]` gives another string, so the host and the port text are
both plain `string`s.

The interesting lines are the ones after the call:

```jai
  port := parse_port(text[colon + 1..<text.count]);
  if port.error != Port_Error.none {
    return Address.{error = Address_Error.bad_port, port_error = port.error};
  }
```

This is *propagating* an error: when the inner call fails, we stop and
return a failure of our own. Notice that we don't just say "bad port".
We keep the port's own error in `port_error`, so the caller can still
tell a missing port from one with a letter in it. Errors should keep
their meaning as they travel. An HTTP 429, a protocol refusal like
`limit_exceeded` and an operating-system error code are different
things, and when a log line says what went wrong, it should say whose
rule was broken. Flattening them all into "error 3" on the way up throws
that away.

Here's the output:

```shell
$ coax run examples/ch07_address.coax
localhost:8080: host localhost, port 8080
example.com:443: host example.com, port 443
localhost: no_colon
:80: empty_host
db:5432x: bad port (not_a_digit)
db:: bad port (empty)
6 addresses checked
```

The last line comes from the `defer` at the top of `main`, which is
the next thing to look at.

## Cleaning up with `defer`

`defer` schedules a statement, or a block, to run when the current
block exits. In Listing 7-4 we wrote:

```jai
  checked := 0;
  defer print("% addresses checked\n", checked);
```

The `print` doesn't run there. It runs when `main`'s block ends, and it
reads `checked` at that moment, which is why it prints 6 rather than 0.
A deferred statement is run later, not evaluated early.

Printing a summary is a gentle use. The real reason `defer` exists is
cleanup: whenever we acquire something that must be released, we write
the release right next to the acquisition, and it runs however the block
ends. Every exit counts: falling off the end, `return`, `break` and
`continue`. Here's a program that shows each of them:

Filename: examples/cleanup.coax

```jai
#import "Basic";

// Find the first value at or above limit. Every exit closes what was opened.
first_at_least :: (values: [] int, limit: int) -> int {
  print("open ");
  defer print("close\n");
  for v, i: values {
    defer print("% ", i);              // runs at the end of every iteration
    if v < 0 { continue; }
    if v >= limit { return v; }        // v is computed, then the defers run
  }
  return -1;
}

nested :: () {
  defer print("third\n");
  {
    defer print("second ");
    defer print("first ");
    print("body ");
  }
}

main :: () {
  data := [5] int.[3, -1, 8, 20, 1];
  print("-> %\n", first_at_least(data[..], 10));
  print("-> %\n", first_at_least(data[..], 99));
  nested();
}
```

*Listing 7-5: Deferred statements run on every way out of a block*

```shell
$ coax run examples/cleanup.coax
open 0 1 2 3 close
-> 20
open 0 1 2 3 4 close
-> -1
body first second third
```

Let's read the first line of output. `first_at_least` prints `open` and
defers `close`. The loop body is a block too, so its `defer` runs once
per iteration: at index 1, where `continue` skips the rest of the body,
and at index 3, where `return` leaves the whole procedure. On that
`return`, the loop's defer runs first (printing `3`) and then the
procedure's (printing `close`). Inner blocks clean up before outer ones.

`nested` shows the order within one block: deferred code runs in
reverse, so the last thing set up is the first thing torn down. That's
the order you want when one resource depends on another, like a lock
taken while a file is open.

### The result is computed first

When a procedure returns, the returned value is computed *before* the
deferred code runs. A defer can't change a value that's already on its
way out:

Filename: examples/deferred_result.coax

```jai
#import "Basic";

count_down :: () -> int {
  n := 3;
  defer n = 0;            // runs after the result is computed
  return n * 10;
}

main :: () {
  print("%\n", count_down());
}
```

*Listing 7-6: `return` evaluates its value, then runs the defers*

```shell
$ coax run examples/deferred_result.coax
30
```

### Deferred code can't leave

Deferred code runs while its block is already exiting, so it can't
start a new exit of its own. A `return`, `break` or `continue` that
would jump out of a deferred statement is an error:

```jai
main :: () {
  for i: 0..<3 {
    defer { if i == 1 { break; } }
  }
}
```

```shell
$ coax check defer_leave.coax
defer_leave.coax:3:25: error[E0361]: a deferred statement cannot break out of its block
      defer { if i == 1 { break; } }
                          ^
1 error
```

A loop *inside* a deferred block can still `break` out of itself; it's
only leaving the deferred code that's ruled out.

### Why not destructors?

> Note: C++ and Rust attach cleanup to a type, through destructors or
> `Drop`, so it runs automatically when a value goes out of scope. Coax,
> like Jai and Go, puts cleanup at the call site instead. You can see
> the release next to the acquisition, copying a value never triggers
> hidden work, and nothing runs in a scope that you didn't write there.
> The cost is that you have to write the `defer`; forgetting it is a
> leak, not a compile error.

`defer` isn't a guarantee against process death. A trap or a killed
process won't run pending deferred code, so state that must survive a
crash needs its own crash consistency.

Once the planned `try` arrives, a propagated error will run pending
defers on the way out too, which is what makes the pattern below safe
in the design:

```jai
f := try open_file(path);
defer close_file(f);

lock(*world.mutex);
defer unlock(*world.mutex);

// ... any return from here runs unlock, then close_file
```

None of `open_file`, pointers or `try` exist in `coax0` yet; read this
as the shape the feature is aiming for.

## Traps

We've met two traps already. Here's a third, the one you'll probably
hit most:

Filename: examples/bounds_trap.coax

```jai
#import "Basic";

at :: (values: [] int, i: int) -> int {
  return values[i];
}

main :: () {
  scores := [3] int.[10, 20, 30];
  print("%\n", at(scores[..], 2));
  print("%\n", at(scores[..], 3));    // one past the end
  print("never printed\n");
}
```

*Listing 7-7: Indexing one past the end traps*

```shell
$ coax run examples/bounds_trap.coax
30
examples/bounds_trap.coax:4: index out of bounds
```

The message has the file, the line and what went wrong. The process
exits through `abort`, so a shell, a test harness or a supervising host
sees a failure (exit status 134 on macOS), not a clean exit. Here's
every trap `coax0` has today, and whether `--release` changes it:

| Trap | Message | Debug | Release |
| --- | --- | --- | --- |
| Integer overflow | `integer overflow in '+'` | Stops | Wraps |
| Division by zero | `division by zero` | Stops | Stops |
| Index out of bounds | `index out of bounds` | Stops | Stops |
| Slice out of bounds | `slice out of bounds` | Stops | Stops |
| `cast` that doesn't fit | `cast out of range` | Stops | Stops |
| `cast` to a non-member of an enum | `value is not a member of enum Mode` | Stops | Stops |
| Shift by a negative amount or the width or more | `shift amount out of range` | Stops | Stops |
| A false `#assert` in a procedure | `assertion failed` | Stops | Stops |

Only overflow changes with `--release`. Wrapping arithmetic is sometimes
exactly what code wants (a hash or a checksum), and the check has a
measurable cost in tight loops. The others stay on in every build,
because continuing past them means reading memory you don't own or
using a value that means nothing. That's also why Listing 7-1 printed
`104080` under `--release`: our parser was relying on a check that a
release build turns off. Code that depends on overflow trapping for its
correctness has a bug that only debug builds catch.

While compiling, every one of these is a compile error instead, in both
modes, because a constant can't be half-computed. [Chapter 9](ch09-compile-time.md)
shows what those errors look like.

The design adds two more traps:

| Trap | Development | Release |
| --- | --- | --- |
| Stale handle | Stops | Policy chosen per build |
| Failed `assert` with a handler | Calls the context's assert handler | Same |

An assert handler will be able to log and stop the current frame, task
or request instead of the whole app. It can't safely continue past
arbitrary memory corruption, and Coax doesn't pretend otherwise.

## To trap or not to trap

So how do you decide whether a failure should be a trap or an error
value? The question to ask is: *if this happens, is my program wrong, or
is the world just being the world?*

Return an error value when the failure is something a correct program
should expect. Bad input from a user or a file, a network peer that
refuses a request, a resource that isn't available yet, a number that's
outside what the caller is allowed to ask for. The caller usually knows
more than you do about what to do next (show a message, try another
server, use a default), so give them the information and let them
decide. `parse_port` is the model here: its input comes from outside,
so every failure is a value.

Trap when continuing would mean computing with something that's already
wrong. An index past the end of an array that *your own code* computed,
a value that your earlier logic guarantees is in range but isn't, a
struct whose fields contradict each other. These mean there's a mistake
in the program, and the caller can't fix that by handling an error code.
Write the assumption down with `#assert` so the mistake is caught at the
point where it first becomes visible, not three procedures later.

A few rules of thumb follow from that:

When you're writing a procedure that other code will call, decide which
of its inputs are the caller's responsibility. If `at(values, i)` is
documented as "`i` must be in range", an out-of-range `i` is the
caller's bug and a trap is right. If the caller can't reasonably know in
advance (is this text a valid port?), return an error.

When the data crosses a boundary into your program (a file, a socket, a
C library, a user), validate it once at the edge and return errors
there. Inside, after validation, trap on anything that contradicts what
you validated.

When you're writing example code, a quick experiment or a test, trapping
is fine. A trap with a line number is a much better failure than a
silently wrong answer, and you can turn it into error handling once you
know which failures matter.

And never let overflow be your input validation. As we saw, it doesn't
exist in release builds.

## In the real world: the wallet's rules

The OpenAgents iOS wallet keeps its payment rules in
`verse/wallet.coax`, and it follows exactly this split. When the
person confirms a payment, Swift asks Coax whether it may be sent:

```jai
Pay :: enum u8 { idle; quoted; sending; succeeded; not_sent; unknown; }
Verdict :: enum u8 { send; over_cap; over_limit; expired; bad_amount; busy; }

verse_wallet_may_send :: (state: Pay, amount: int, fee: int, cap: int, left: int,
                             now: int, expires: int) -> Verdict #export_c {
  if state != Pay.quoted { return Verdict.busy; }
  if amount <= 0 || fee < 0 || amount > MAX_SAT { return Verdict.bad_amount; }
  if now >= expires { return Verdict.expired; }
  if fee > cap { return Verdict.over_cap; }
  if amount > left - fee { return Verdict.over_limit; }
  return Verdict.send;
}
```

A quote that expired or a fee that's too high is not a bug. It's a
normal outcome that the app has to show the person, so it's a value of
`Verdict`, and the Swift side switches on it. Notice that the success
case, `send`, is the zero member, just as `none` was for us. Elsewhere
in the same file, procedures that cross into C return plain `int`s and
use `-1` for "invalid", a convention any C or Swift caller understands
([Chapter 17](ch17-interop.md) covers the C boundary). Both are error
values; the enum is the clearer of the two, because its type lists
every reason.
The bugs, meanwhile, are caught by `#assert` in checks that run on
every build, which is the subject of [Chapter 10](ch10-writing-checks.md).

## What's coming: `try`, `or_else` and optional types

Listing 7-4 works, but propagating the port's error took four lines,
and nothing stopped a caller from reading `a.port` without checking
`a.error`. The spec plans dedicated syntax for both problems. None of it
runs in `coax0` today; if you try, the compiler tells you so:

```shell
$ coax check ports.coax
ports.coax:25:10: error[E0204]: 'try' is reserved and not supported by coax0 yet
      p := try parse_port(text);
           ^
1 error
```

In the design, a procedure that can fail returns its error as its last
result, typed as an optional enum (`Port_Error?`) or a tagged union, so
the type says exactly which failures are possible:

```jai
Read_Error :: enum u8 { not_found = 1; too_large = 2; denied = 3; }

read_file :: (path: string, max_bytes: int) -> (string, Read_Error?) { ... }
```

Ignoring that error will be a compile error, extending the rule we saw
with `E0323`. Two operators will handle it at the call site:

```jai
load_level :: (path: string) -> (Level, Load_Error?) {
  text := try read_file(path, 1 << 20);              // on error: return it to our caller
  level := parse_level(text) or_else empty_level();  // on error: use a fallback
  return level, null;
}
```

`try` unwraps a success, or returns the error from the current
procedure, running any pending `defer` on the way out. `or_else`
handles the failure in place with a value. Both are visible where the
call is, so reading a procedure you can still see every point where it
might return early. The spec doesn't yet say how `try` converts one
error type into another (our `Port_Error` into an `Address_Error`);
until it does, assume that mapping stays explicit, the way Listing 7-4
writes it.

Multiple results are the other planned piece. They're a plain return of
several values, with no tuple object allocated and no out-parameter
pointer:

```jai
divmod :: (a: int, b: int) -> (quotient: int, remainder: int) {
  return a / b, a % b;
}

q, r := divmod(17, 5);
```

That's how a value and its error will travel together. Until then, a
struct with an error field, as in `Parsed_Port`, is the way to do it,
and it converts mechanically when the syntax lands.

## Summary

Coax keeps two kinds of failure apart. Bugs trap: overflow, division by
zero, out-of-bounds indexing and slicing, casts that don't fit and false
`#assert`s stop the program with a file and line, and all but overflow
stay on in release builds. Expected failures are values: an enum of
reasons, returned alongside the result, with zero meaning "no error",
and a result the caller isn't allowed to drop. When an error passes
through a procedure, it should keep its original meaning. `defer` puts
cleanup next to the thing it cleans up and runs it on every way out of a
block, in reverse order, after the return value is computed.

The tool for choosing is one question: is the program wrong, or is the
input? Next, in [Chapter 8](ch08-generics.md), we'll look at how Coax
plans to let one procedure, like `parse_port`'s cousins, work for many
types without being written many times.
