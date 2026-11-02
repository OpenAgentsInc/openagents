# 36. A terminal UI

> **Status: planned.** Nothing in this chapter is written yet.

Coder in a terminal the person already has open, the way Claude Code and
Codex run: an inline live region at the bottom (status, the streaming reply,
the composer), with finished turns written into the terminal's own
scrollback so selection, search and copy keep working. No full-screen
takeover and no widget toolkit. The rendering library is
`coax/lib/tui/tui.coax`; the program is `coder/tui/coder.coax`.

## What this chapter will cover

- Raw keys without signals: Ctrl-C and Esc arrive as bytes, and the program
  decides what they stop.
- A key decoder for control bytes, CSI and SS3 sequences, Alt-Enter and
  bracketed paste, which asks for more bytes on a partial sequence.
- Display width for UTF-8 text, wrapping at spaces, and a composer with
  word motions, kill commands and history.
- Erase and redraw of the live region only, committing lines into
  scrollback, and keeping the composer on the bottom rows.
- Markdown rendered a line at a time, so a streaming reply is styled as it
  arrives.
- A syntax highlighter for Coax (after Jai) and a shared one for other
  languages, used in code blocks and in diffs.
- Edits shown as diffs: line numbers from the file, removed and added lines
  on red and green, and context around each change.
- A pseudo-terminal smoke test that types, pastes, runs a command and quits,
  and checks the terminal is restored.
- The `openagents coder` verb: running it with no task opens this UI.

## Sources

- [Chapter plan](PLAN.md), Part VIII.
