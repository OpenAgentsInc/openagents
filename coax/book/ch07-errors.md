# 7. Errors and cleanup

> **Partly runs today.** `defer` (7.4) and traps (7.5) run on `coax0`
> 0.3.0, and their examples are checked. So does the rule that results
> are used. Errors as values, `try`, `or_else` and multiple results (7.1
> to 7.3, 7.6) are *Design* (spec §8 and §11.3).

Coax has two kinds of failure, and keeps them apart:

- **Expected failures** are values: a file that isn't there, a request
  that was refused, input that's too large. Code handles them.
- **Bugs** are traps: overflow, an index out of bounds, a stale handle, a
  broken invariant. They stop the operation, because continuing would
  compute nonsense.

There are no exceptions. Nothing unwinds the stack invisibly, and every
line that can fail says so.

## 7.1 Errors are values (design)

```coax
Read_Error :: enum u8 { not_found = 1; too_large = 2; denied = 3; }

read_file :: (path: string, max_bytes: int) -> (string, Read_Error?) { ... }
```

A procedure that can fail returns its error as its last result, typed as
an optional enum or a tagged union. The type says exactly which failures
are possible.

**Ignoring an error is a compile error.** `coax0` already refuses an
unused result (`E0323`), and the same rule covers errors. A result you
receive isn't proof you handled it well, but a result you never looked at
is always a bug.

## 7.2 `try` and `or_else` (design)

```coax
load_level :: (path: string) -> (Level, Load_Error?) {
  text := try read_file(path, 1 << 20);      // on error: return it to our caller
  level := parse_level(text) or_else empty_level();  // on error: use a fallback
  return level, null;
}
```

- **`try`** unwraps a success, or returns the error from the current
  procedure. It runs any pending `defer` cleanup on the way out.
- **`or_else`** handles the failure in place with a value.

Both are visible at the call site. Reading a function, you can see every
point where it might return early.

## 7.3 Errors keep their meaning (design)

Errors from outside Coax keep their origin. An HTTP 429, a protocol
refusal like `limit_exceeded`, and an operating system error code are
different things, and an adapter maps each into a typed Coax error
without flattening it to "error 3". When a log says what went wrong, it
should say whose rule was broken.

## 7.4 `defer`

```coax
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

```
open 0 1 2 3 close
-> 20
open 0 1 2 3 4 close
-> -1
body first second third
```

`defer` schedules a statement or a block to run when the current block
exits. Every exit counts: falling off the end, `return`, `break` and
`continue`. The loop body's `defer` runs once per iteration, including
the one that `continue`s past index 1 and the one that `return`s from
index 3. Deferred code runs in reverse order, so the last thing acquired
is the first released: `first` prints before `second`.

**The result is computed first.** `return v;` evaluates `v`, then runs
the deferred code, then returns. A defer can't change a value that's
already on its way out:

```coax
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

```
30
```

**Deferred code can't leave.** It runs while its block is exiting, so a
`return`, `break` or `continue` that would jump out of it is an error:

```
defer_leave.coax:3:25: error[E0361]: a deferred statement cannot break out of its block
      defer { if i == 1 { break; } }
                          ^
```

A loop inside a deferred block can still `break` out of itself.

`defer` is cleanup at a call site, not a destructor attached to a type.
That's deliberate:

- You can see the cleanup next to the acquisition.
- Copying a value never triggers hidden work.
- Nothing runs that you didn't write in that scope.

It isn't a guarantee against process death: a trap or a killed process
won't run it. Durable state needs its own crash consistency.

In the design, `defer` pairs with resources and `try`, and an error
propagated by `try` runs pending defers on the way out too:

```coax
f := try open_file(path);
defer close_file(f);

lock(*world.mutex);
defer unlock(*world.mutex);

// ... any return from here runs unlock, then close_file
```

## 7.5 Traps

A trap stops the program with the file, the line and what went wrong:

```coax
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

```
30
examples/bounds_trap.coax:4: index out of bounds
```

The process exits through `abort`, so a test harness or a supervising
host sees a failure, not a clean exit. `coax0` has these traps today:

| Trap | Message | Debug | Release |
| --- | --- | --- | --- |
| Integer overflow | `integer overflow in '+'` | Stops | Wraps |
| Division by zero | `division by zero` | Stops | Stops |
| Index out of bounds | `index out of bounds` | Stops | Stops |
| Slice out of bounds | `slice out of bounds` | Stops | Stops |
| `cast` that doesn't fit | `cast out of range` | Stops | Stops |
| `cast` to a non-member of an enum | `value is not a member of enum Mode` | Stops | Stops |
| Shift by a negative amount or the width or more | `shift amount out of range` | Stops | Stops |

Only overflow changes with `--release`, because wrapping arithmetic is
sometimes what a hash or a checksum wants and the check has a measurable
cost in tight loops. The others stay on, because continuing past them
means reading memory you don't own or using a value that means nothing.
At compile time every one of them is an error in both modes (Chapter 9).

The design adds two more:

| Trap | Development | Release |
| --- | --- | --- |
| Stale handle | Stops | Policy chosen per build |
| Failed `assert` | Calls the context's assert handler | Same |

An assert handler can log and stop the current frame, task or request
instead of the whole app. It can't safely continue past arbitrary memory
corruption, and Coax doesn't pretend otherwise.

## 7.6 Multiple results (design)

```coax
divmod :: (a: int, b: int) -> (quotient: int, remainder: int) {
  return a / b, a % b;
}

q, r := divmod(17, 5);
```

Several results are a plain return of several values. No tuple object is
allocated, and no out-parameter pointer is needed. This is how a value
and its error travel together in 7.1.
