# Terminal lifecycle smoke test, 2026-09-20

This record captures the interactive verification for the terminal guard
described in [terminal.md](../runtime/terminal.md). It answers the acceptance item in
issue #9432 that asks for a real-pty run with a normal exit and a `Ctrl-C`
exit, followed by a check that the shell is still sane.

## Setup

- Host: Linux (Ubuntu), `rustc 1.97.1` from `rust-toolchain.toml`.
- Binary: `cargo build -p coder --bin coder` (debug).
- Pty: a Python `pty.fork()` running `bash --noprofile --norc` with
  `TERM=xterm-256color`, window size 30x100. The script types the `coder`
  command into the shell, waits for the first frame, types
  `hello wörld 日本 🦀`, then ends the session, and finally runs `stty -a` in the
  same shell.
- Model door: `TYPESAFE_API_KEY` was unset so the run stayed offline. The smoke
  therefore covers startup, drawing, editing, and exit, not a model turn.

## Runs

| Run | Ending keystroke | Alt screen entered / left | Cursor style reset (`CSI 0 q`) | Cursor color reset (`OSC 112`) | `EXIT=$?` |
| --- | --- | --- | --- | --- | --- |
| Normal exit | `Ctrl-D` on an empty draft | yes / yes | yes | yes | 0 |
| Interrupt | `Ctrl-C` | yes / yes | yes | yes | 0 |
| Resize then exit | `SIGWINCH` to 20x60 and back, then `Ctrl-D` | yes / yes | yes | yes | 0 |

Every run drew the composer frame and echoed the typed Unicode draft with the
accent, the CJK characters, and the emoji intact.

## Shell state afterward

`stty -a` in the same pty after the `Ctrl-C` run:

```text
speed 38400 baud; rows 30; columns 100; line = 0;
isig icanon iexten echo echoe echok -echonl -noflsh -xcase -tostop -echoprt echoctl echoke -flusho
```

The shell is back in cooked mode: `isig`, `icanon`, and `echo` are all set, and
a follow-up `echo READBACK` was echoed and executed normally. The normal-exit
and resize runs produced the same flags.

## Not covered here

- A model turn through the door. The key was deliberately unset.
- macOS terminals. Only Linux was available.
