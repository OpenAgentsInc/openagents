# 36. A terminal UI

> **Status: partly implemented.** Coder runs inline in the terminal with
> three agents behind one composer: `coder/tui/coder.sh`, or `openagents
> coder`. The rendering library has 10 checks, and a pseudo-terminal smoke
> types, pastes, runs a command, switches agent and quits, then runs the
> verbs without a terminal. Still to come: the chapter's walkthrough of each
> piece, mouse-free scrollback search inside the UI, and a resize check in
> the smoke.

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
python3 coder/tui/smoke.py               # the offline smoke: 24 checks
```

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
