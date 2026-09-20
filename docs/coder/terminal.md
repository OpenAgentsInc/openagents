# The terminal's lifecycle, lanes, and scrollback

How the `coder` terminal holds the terminal, how a turn's reports reach
the draw loop, and how much transcript the loop keeps. All three live in
`crates/coder-terminal` and the shell in `crates/coder/src/main.rs` uses
them; the composer example in `crates/coder-terminal/examples/shell.rs`
uses the first.

Status: implemented. Source: `crates/coder-terminal/src/guard.rs`,
`events.rs`, and `scrollback.rs`.

## Why

The [September 2026 audit](../audits/2026-09-19-codebase-audit/README.md)
read the interactive shell and found three things it did by hand. Raw
mode was entered before fallible setup and undone only on the normal
return, so an error between the two, or a panic anywhere in the draw
loop, left the user's shell in raw mode on the alternate screen with an
amber cursor. The turn's worker reported every event with `try_send` on a
bounded channel and ignored the result, so a burst of streamed text could
push a command's outcome, or a refusal, out of the transcript without a
word. And every frame re-wrapped every line of an unbounded transcript.
None of these was reproduced interactively; the fixes below are held to
tests that exercise the failure paths directly, plus a smoke test in a
real terminal.

## The guard

`Guard` holds the terminal in the shell's mode. Each change is a `Step`:

| Step | Applies | Undoes |
| --- | --- | --- |
| `RawMode` | `enable_raw_mode` | `disable_raw_mode` |
| `AlternateScreen` | `EnterAlternateScreen` | `LeaveAlternateScreen` |
| `CursorStyle` | a blinking block | the user's shape, and `Show` |
| `CursorColor` | OSC 12, amber | OSC 112 |

`Guard::enter(console, steps)` applies the steps in order and records
each one that succeeded. If a step fails, the steps before it are undone
in reverse and the error returns; nothing is left applied. When the guard
drops — after the draw loop returns, after an error, or while a panic
unwinds — it undoes what is still recorded, in reverse. `Guard::restore`
does the same on request and reports an error the drop would swallow, so
the shell calls it at the end of the normal path.

`Guard::full_screen` applies all four steps on standard output and arms
the panic hook. The hook shares the guard's list of applied steps: when a
panic fires, the hook restores whatever is still on the list, empties it,
and then calls the hook that was installed before it, so the panic
message and backtrace print on a terminal that can show them. The guard's
own drop, which runs as the panic unwinds, finds the list empty and does
nothing more. Neither side restores twice.

The steps land on a `Console`. `guard::Stdout` is the real one; the tests
use a recording console that fails on a chosen step, which is how the
failure ordering above is checked without a terminal.

## Two lanes

A turn reports two kinds of thing. A control event changes what the
transcript says: a classify verdict, a shell proposal, a command's
outcome, a refusal, a program choice, the turn's completion. A text event
changes only what the live preview shows: one streamed delta of a reply
that arrives whole with the completion anyway.

`events::channel` returns a `Feed` for the worker and an `Inbox` for the
draw loop. Both lanes ride one unbounded queue, so events stay in order,
but `Feed::send` treats them differently:

- A control event is always queued. The send cannot fail for pressure;
  the only way it does not land is that the inbox is gone, which the
  worker learns as `Sent::Closed`.
- A text event is queued while fewer than `TEXT_BACKLOG_MAX` bytes of
  text are in flight. Past that it is dropped and its size is counted.
  The draw loop reads the count with `Inbox::take_dropped` before each
  frame and writes one line into the transcript — `preview fell behind —
  N bytes not drawn; the reply arrives whole` — so a drop is visible
  rather than silent.

`Inbox::drain` takes everything ready and merges adjacent text events, so
a frame that wakes to a burst of tokens applies one `push_str` per burst
and one control event per control event, in the order they were sent.
This matters for the shell loop: a plan's JSON streams as deltas and the
`$` command lines replace it when the proposals land, and that
replacement is a control event ordered after the deltas it replaces.

The `Work` type in `crates/coder/src/main.rs` implements `events::Event`
to say which of its variants is which. `turn::run` is unchanged; it still
calls one callback per event, and headless mode still reads that callback
directly.

## Scrollback

`Scrollback` keeps at most `LINES_MAX` lines (5,000) and evicts the oldest
past that. A line that leaves the screen this way is still in the
session's trace under `~/.openagents/traces/`, which records the whole
conversation.

Each line's wrapped rows are cached at the width they were wrapped for.
`Scrollback::rows(width, keep)` wraps the lines that have no rows yet,
returns the rows of every line `keep` admits, and, when `width` differs
from the last call, drops every cached row first so the whole transcript
reflows once. A frame at a steady width therefore wraps only the lines
pushed since the last frame. The streaming reply and the working line
still wrap every frame, because they change every frame.

The verbose filter is applied over the cache rather than into it:
toggling detail lines on wraps the detail lines once and hides them again
without rewrapping anything.

## Verifying

Unit tests in `crates/coder-terminal` cover the guard's failure ordering,
drop, unwind, and hook coordination; control delivery under a hundred
thousand unread sends; text dropping and counting past the backlog;
combining marks, ZWJ emoji sequences, and wide characters in the editor
and the wrap; and reflow across a resize:

```sh
cargo test -p coder-terminal
```

The interactive check is a real terminal. Run `./scripts/coderdev`, type a
line, resize the window, quit once with `Ctrl-C` and once with an empty
`Ctrl-D`, and confirm with `stty -a` that the shell is back in cooked mode
with echo on. The `2026-09-20-terminal-lifecycle.md` record under
[`verification/`](verification/) holds one such run.

## Related

- [`shell-loop.md`](shell-loop.md) — the events the control lane carries.
- [`traces.md`](traces.md) — where the lines the scrollback evicts still
  live.
- [`headless.md`](headless.md) — the mode that shares `turn::run` with the
  terminal and has no terminal to guard.
