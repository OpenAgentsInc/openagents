# 36. A terminal UI

> **Status: implemented.** Coder runs inline in the terminal with three
> agents behind one composer: `coder/tui/coder.sh`, or `openagents coder`.
> The rendering library has 11 checks. A pseudo-terminal smoke types,
> pastes, runs a command, searches the transcript with `/find`, switches
> theme, shows a running background job, resizes the window, switches agent
> and quits, then runs the verbs without a terminal. The mouse stays with
> the terminal; see [The mouse](#the-mouse).

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
coder/tui/coder.sh --theme light         # for a light background; or CODER_THEME=light, or NO_COLOR=1 for none
coder/tui/coder.sh resume                # this folder's newest session, to continue it; resume claude for Claude Code's
python3 coder/tui/smoke.py               # the offline smoke, in a pseudo-terminal and then without one
```

When Shift-Tab selects Codex, its model and reasoning level sit at the right
of the working-step line, directly above the composer's top rail. Both stay
on one line as the terminal narrows; `/model` and `/effort` hints sit in the
footer. The configuration resolves before the first message.
`/model` lists Codex's own available model IDs, reasoning
levels and defaults; `/model ID [LEVEL]` selects one. `/effort` shows the
current model's levels, and `/effort LEVEL` changes the next message's
reasoning. Selecting a model without a level uses that model's default.
Selections keep the conversation and last for this Coder session, including
across agent switches. Launch flags work too:
`coder/tui/coder.sh --engine codex --model ID --effort xhigh` (levels are
model-dependent). The catalog comes from Codex's
[`model/list`](https://learn.chatgpt.com/docs/app-server#list-models-modellist),
so Coder does not maintain a separate list of models.

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
code blocks. Code blocks go through the highlighter. A table needs all its
rows before a column's width is known, so Coder holds the rows
(`tui_md_is_row`) until the table ends. `tui_md_table` then draws it with
box borders and a bold header, each column as wide as its widest cell and
aligned as the separator row says. A table wider than the terminal becomes
one block a row: the first cell in bold, then `header: value` lines.

**Highlighting.** `tui_hl_line` colors one line of code: a Coax
highlighter (after Jai) and a shared one for other languages (`tui_hl_lang`
picks it from the fence's language name). Diffs use it on red and green
backgrounds for removed and added lines.

**Search.** Coder keeps the last 256 KB of committed transcript text.
`/find TEXT` lists the lines that hold it, ignoring case, the last 20 of
them, with a count. It runs without a mouse and never leaves the composer.

**Resume.** `coder resume [ID]` starts the UI on an earlier session, drawn
as it was (messages, Markdown replies, tool rows, endings), and the next
message continues it. `/resume` lists this folder's sessions; `/resume ID`
continues one, and `/resume claude` this folder's newest Claude Code
session, imported first. [Chapter 14](ch014-forks-subagents-and-jobs.md#resume-and-import)
covers what a resume carries.

**Usage.** On Claude Code and Codex, the right end of the hint row shows
how much of the plan you have used: a ring that fills by quarters, the
window closest to its limit, and the percent, such as `◕ Week 84%`. It
turns amber at 80% and red at 95%. `/usage` and `coder usage [--json]`
list every window with a bar and its reset time:

```text
  Claude Code · Claude Max · read just now
    Week     ████████░░  84%  resets in 4d 17h
```

Coder reads usage the way Zeron does. Claude's comes from
`api.anthropic.com/api/oauth/usage` (the five-hour Session and the
seven-day Week), with the token in `.credentials.json` under
`CLAUDE_CONFIG_DIR` (default `~/.claude`), or on macOS the Keychain item
`Claude Code-credentials`. Codex's comes from
`chatgpt.com/backend-api/wham/usage`, with the token in `auth.json` under
`CODEX_HOME` (default `~/.codex`). The token goes only to `curl`'s
standard input and is wiped after the call. Coder asks every 5 minutes,
or at most every 30 seconds when you switch agents or type `/usage`. After
a failure it waits: Retry-After for a 429 (30 s to 1 h, default 5 min), 60 s
for a server error, 15 min for another error, 30 s for a network error,
and 10 min with no login. The last reading is kept beside the sessions, so
the row fills at once on the next start. `CODER_USAGE=0` turns it off, and
`CODER_USAGE_KEYCHAIN=0` keeps it to the files.

**Status.** While a turn runs, the row above the composer shows a spinner,
the step number, the time so far, how to stop, and the step running now:

```text
⠙ Working · step 3 · 42s · Esc to stop · Bash(cargo test -q)
```

The hint line under the composer counts the background jobs running on this
machine (`coder run --detach`, chapter 14), checked every 2 seconds:

```text
claude · 2 jobs running · Enter send · Shift-Enter newline · Shift-Tab agent · …
```

**Themes.** `TuiOut.theme` picks the palette that `tui_style`,
`tui_md_sgr` and `tui_hl_sgr` use:

| Theme | For |
| --- | --- |
| `dark` | The default, Coder Noir (the desktop app's dark theme): `#ededed` text, `#818181` and `#656565` for muted and faint lines, `#ff4d42` for errors, `#9fd08a` for success, `#e6c15c` for warnings, and its syntax colors. |
| `light` | A light background: near-black text, darker code colors, pale diff backgrounds. |
| `none` | No colors: bold, italic and underline only. |

`--theme dark|light|none` picks one at start, then `CODER_THEME`. With
neither set and `NO_COLOR` set, the theme is `none`. `/theme NAME` switches
while Coder runs.

### The mouse

Coder doesn't turn on mouse reporting. In an inline UI, the transcript
lives in the terminal's own scrollback, and the terminal already scrolls it
with the wheel and selects, copies and searches it with the mouse. Mouse
reporting would take those away from the person to give Coder clicks it has
no use for: every control is a key, and `/find` searches without leaving the
composer.

### Recaps

Recaps are off unless you set `CODER_RECAPS=1`. A guessed line can read as
the agent's answer (asked "whats this", it showed "I'm a coding agent, here to
help you with your coding tasks"), so by default the status bar shows only
facts: the step, the time and the tool running now. With `CODER_RECAPS=1` on
a Mac with Apple Intelligence, the status bar says in a few words what the
agent is doing, in place of "Working":

```text
⠸ Running tests for calc.py · step 3 · 9.4s · Esc to stop · Bash(python3 test_calc.py)
```

Apple's on-device model writes the line through the bridge in
[psionic/apple-fm](../../psionic/apple-fm/README.md), so nothing leaves the
Mac. Every 4 seconds while the agent works, if something changed, Coder
sends the task, the last six steps and the last 400 bytes of the reply. It
asks for 3 to 8 words in the present tense, then strips quotes, Markdown
marks and a final period. The request and its answer run in the
background (`afm_ask`, `afm_poll`), so the UI never waits for the model. A
recap that arrives after its turn ended is dropped.

With `CODER_RECAPS=1`, recaps start when the bridge is built
(`psionic/apple-fm/build.sh`) and the model is available. On any other machine the status bar says "Working" as
before. `CODER_APPLE_FM=0` turns recaps off. `coder apple-fm` says whether
they can run, and `coder apple-fm "PROMPT"` prints the model's answer. The smoke points Coder
at a stand-in bridge whose every answer is `"Waiting on the sleep
command."`, runs `$ sleep 7`, and checks that the status bar shows
`Waiting on the sleep command ·`.

Live, with Claude Code fixing a bug in `calc.py`, the bar said "Running
pytest -q to find and fix bugs in calc.py" 4 seconds in, then "Running
tests for calc.py". The first recap took about 1.5 seconds.

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
