# 7. Errors and cleanup

> **Design.** `coax0` has traps (overflow, division by zero) and checks
> that results are used. Multiple returns, optionals, `try`, `or_else`
> and `defer` are designed (spec §8 and §11.3).

Coax has two kinds of failure, and keeps them apart:

- **Expected failures** are values: a file that isn't there, a request
  that was refused, input that's too large. Code handles them.
- **Bugs** are traps: overflow, an index out of bounds, a stale handle, a
  broken invariant. They stop the operation, because continuing would
  compute nonsense.

There are no exceptions. Nothing unwinds the stack invisibly, and every
line that can fail says so.

## 7.1 Errors are values

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

## 7.2 `try` and `or_else`

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

## 7.3 Errors keep their meaning

Errors from outside Coax keep their origin. An HTTP 429, a protocol
refusal like `limit_exceeded`, and an operating system error code are
different things, and an adapter maps each into a typed Coax error
without flattening it to "error 3". When a log says what went wrong, it
should say whose rule was broken.

## 7.4 `defer`

```coax
f := try open_file(path);
defer close_file(f);

lock(*world.mutex);
defer unlock(*world.mutex);

// ... any return from here runs unlock, then close_file
```

`defer` schedules code to run when the current block exits: normally, by
`return`, or by `try` propagating an error. Deferred blocks run in
reverse order, so the last thing acquired is the first released.

`defer` is cleanup at a call site, not a destructor attached to a type.
That's deliberate:

- You can see the cleanup next to the acquisition.
- Copying a value never triggers hidden work.
- Nothing runs that you didn't write in that scope.

A deferred block can't `return` from its procedure. And it isn't a
guarantee against process death: a trap or a killed process won't run
it. Durable state needs its own crash consistency.

## 7.5 Traps

| Trap | Development | Release |
| --- | --- | --- |
| Integer overflow | Stops (works today) | Wraps (works today) |
| Division by zero | Stops (works today) | Stops (works today) |
| Index out of bounds | Stops | Policy chosen per build |
| Stale handle | Stops | Policy chosen per build |
| Failed `assert` | Calls the context's assert handler | Same |

An assert handler can log and stop the current frame, task or request
instead of the whole app. It can't safely continue past arbitrary memory
corruption, and Coax doesn't pretend otherwise.

## 7.6 Multiple results

```coax
divmod :: (a: int, b: int) -> (quotient: int, remainder: int) {
  return a / b, a % b;
}

q, r := divmod(17, 5);
```

Several results are a plain return of several values. No tuple object is
allocated, and no out-parameter pointer is needed. This is how a value
and its error travel together in 7.1.
