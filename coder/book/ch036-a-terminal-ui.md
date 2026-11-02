# 36. A terminal UI

> **Status: partly implemented.** Coder runs inline in the terminal with
> three agents behind one composer: `coder/tui/coder.sh`, or `openagents
> coder`. The rendering library has 10 checks. A pseudo-terminal smoke
> types, pastes, runs a command, searches the transcript with `/find`,
> resizes the window, switches agent and quits, then runs the verbs without
> a terminal. Still to come: mouse support, a status line for long-running
> work, and themes beyond the default palette.

Coder in a terminal the person already has open, the way Claude Code and
Codex run: an inline live region at the bottom (status, the streaming reply,
the composer), with finished turns written into the terminal's own
scrollback so selection, search and copy keep working. No full-screen
takeover and no widget toolkit. The rendering library is
`coax/lib/tui/tui.coax`; the program is `coder/tui/coder.coax`.

## What runs

```sh
coder/tui/coder.sh                       # Claude Code by default
coder/tui/coder.sh --engine microcoder   # or codex, psionic, echo (offline)
python3 coder/tui/smoke.py               # the offline smoke, in a pseudo-terminal and then without one
```

### The pieces

Each piece is a few functions in
[coax/lib/tui/tui.coax](../../coax/lib/tui/tui.coax), and the program wires
them together in [coder/tui/coder.coax](../../coder/tui/coder.coax).

**Output.** Everything the program writes goes into a `TuiOut` buffer
first: text with `tui_put`, escape sequences with `tui_csi`, and styles with
`tui_style`. `cd_flush` writes the buffer to the terminal in one call, so a
redraw never shows half-drawn.

**Width.** `tui_rune` decodes UTF-8 and `tui_width` gives a code point's
columns: 0 for combining marks, 2 for wide characters such as CJK and most
emoji, and 1 otherwise. `tui_text_width` skips escape sequences, and
`tui_fit` finds how much of a string fits in a number of columns. Wrapping
depends on these, so a line never runs past the right edge.

**Keys.** The terminal is in raw mode with signals off, so Ctrl-C arrives
as byte 3 and the program decides what it stops. `tui_key` decodes one key
from the bytes read so far: control bytes, CSI and SS3 sequences, the kitty
keyboard protocol's `CSI … u` (so Shift-Enter and Shift-Tab are distinct),
and bracketed paste. With a partial sequence it asks for more bytes instead
of guessing.

**The composer.** `TuiEditor` holds the text and the cursor as a byte
offset. `tui_ed_key` applies a key: insert, delete, word motions (Alt-B and
Alt-F), kill commands (Ctrl-K, Ctrl-U and Ctrl-W), and history with Up and
Down (`tui_ed_remember`, `tui_ed_show_history`). Shift-Enter and pasted
newlines insert a line; Enter submits.

**Layout.** `TuiRows` is the list of rows to draw. `tui_rows_wrap` wraps
text at spaces to the screen width, with a first-line prefix (such as
`● `) and an indent for the lines after it. `tui_rows_editor` lays out the
composer and reports the cursor's row and column.

**The live region.** Only the bottom of the screen is redrawn: the
streaming reply, the composer between two rules, and the hint line.
`tui_live_draw` erases what it drew last time and draws the new rows.
`tui_live_commit` writes finished lines above the region, into the
terminal's own scrollback, so the terminal's selection, search and copy
keep working on them. `tui_live_pin` keeps the composer on the bottom rows,
and `tui_live_resize` and `tui_live_width` follow a resized window: the
kernel sends `SIGWINCH`, `cxterm_resized` reports it, and the next draw uses
the new size.

**Markdown.** `tui_md_line` styles a reply one line at a time, as it
streams: headings, lists, quotes, inline code, bold and italics, and fenced
code blocks. Code blocks go through the highlighter.

**Highlighting.** `tui_hl_line` colors one line of code: a Coax
highlighter (after Jai) and a shared one for other languages (`tui_hl_lang`
picks it from the fence's language name). Diffs use it on red and green
backgrounds for removed and added lines.

**Search.** Coder keeps the last 256 KB of committed transcript text.
`/find TEXT` lists the lines that hold it, ignoring case, the last 20 of
them, with a count. It runs without a mouse and never leaves the composer.

## What this chapter covers

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
- Three agents behind one composer: Claude Code over stream-json, Codex as
  `codex app-server` over JSON-RPC, and Microcoder as a worker process of
  our own speaking JSON lines, each with its own conversation, and Shift-Tab
  to cycle them. Codex reports file changes as unified diffs, shown the same
  way.
- A pseudo-terminal smoke test that types, pastes, runs a command and quits,
  and checks the terminal is restored.
- The `openagents coder` verb: running it with no task opens this UI
  (chapter 23).

## Sources

- [Chapter plan](PLAN.md), Part VIII.
- `coax/lib/tui/tui.coax`: keys, width, the composer, the live region,
  Markdown and the highlighter.
- `coder/tui/`: `coder.coax` (the program), `codex.coax`, `microcoder.coax`,
  `mcworker.coax`, `diff.coax`, `session.coax`, `smoke.py`.
