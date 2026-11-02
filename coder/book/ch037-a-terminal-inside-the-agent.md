# 37. A terminal inside the agent

> **Status: partly implemented.** The emulator's core runs in Coax: a
> parser state machine, a cell grid with history, scroll regions, the
> alternate screen, wide characters and a text snapshot, in
> `coax/lib/tui/vt.coax` with 8 checks. Coder reads every command's output
> and tool result through it, so a progress bar reads as its last state and
> colors leave no bytes behind. Still to come: running programs in a
> pseudo-terminal, so tools that only draw for a terminal draw at all;
> keystrokes sent back as tool calls; and checking the emulator against
> libghostty on recorded sessions.

The other direction: Coder runs programs that expect a terminal (test
runners with progress bars, interactive prompts, editors) and needs to read
what they drew. That takes a VT emulator: a parser for escape sequences, a
grid of cells with scrollback, and a snapshot the model can read as text.

## The parser

`vt_byte` in [vt.coax](../../coax/lib/tui/vt.coax) takes one byte at a
time, so a sequence split across two reads parses the same as one read. The
states:

| State | Reads | Ends on |
| --- | --- | --- |
| ground | text as UTF-8, and controls (CR, LF, BS, tab) | ESC |
| escape | one byte: `[` for CSI, `]` for OSC, `7` and `8` (save and restore the cursor), `D`, `E`, `M` (index, next line, reverse index), `c` (reset) | that byte |
| CSI | parameters, a private marker such as `?`, then a final byte | `@` to `~` |
| OSC | a string such as a window title | BEL, or ESC `\` |
| charset | one designation byte after `(` or `)` | that byte |

Controls act inside a CSI sequence too, as a terminal does.

## The grid

The grid holds a code point in each cell, with a marker in the cell after a
wide character. The CSI finals it acts on:

| Final | Effect |
| --- | --- |
| `A` `B` `C` `D` `E` `F` `G` `H` `d` `f` | cursor moves and addressing |
| `J` `K` `X` | erase in the display, in the line, a run of characters |
| `@` `P` | insert and delete characters |
| `L` `M` `S` `T` | insert and delete lines, scroll up and down |
| `r` | the scroll region |
| `s` `u` | save and restore the cursor |
| `?47` `?1047` `?1049` with `h` or `l` | the alternate screen |

SGR (`m`) and the other modes are read and dropped: the snapshot is for
reading, so it keeps no colors. Writing in the last column sets a pending
wrap, and the next character wraps first, as real terminals do.

A line that scrolls off the top of the main screen goes into history. So
`vt_text` returns the whole output of a long command (the history, then the
screen down to its last row with text), and `vt_screen` returns only the
screen, which is what a model reads of a full-screen tool. Leaving the
alternate screen brings the main screen back, as it was.

## In Coder

`cd_screen` in [coder.coax](../../coder/tui/coder.coax) runs a tool's
output through a 200-column emulator when it holds any control byte. Plain
text passes through untouched. Every engine's results go through it: `$ `
commands, Claude Code's and Codex's tool results, and Microcoder's
commands. The session log stores what the screen showed:

```sh
coder run "$ printf '10%%\r100%%\n\033[32mok\033[0m\n'" --engine echo --json
```

```json
{"kind":"result","text":"100%\nok","error":false}
```

Commands still run with their output on a pipe, so a program that checks
for a terminal draws its plain form. Running them in a pseudo-terminal is
the next step. Chapter 33 found the pattern in Toad: a persistent shell in
a pseudo-terminal that reports its working directory after each command.

## Checks

```sh
coax test coax/lib/tui/vt.coax --library    # 18 checks: tui.coax's 10 and vt.coax's 8
python3 coder/tui/smoke.py                  # "output reads as the screen shows it"
```

The checks cover a progress bar and colors, erasing, cursor addressing,
history and wrapping, the alternate screen, wide characters and UTF-8 split
across reads, scroll regions with inserted and deleted lines, and OSC
titles with a sequence split across reads.

## Sources

- `coax/lib/tui/vt.coax`; `coder/tui/coder.coax` (`cd_screen`).
- [Chapter plan](PLAN.md), Part VIII.
- [Chapter 33](ch033-toad.md): Toad's shell in a pseudo-terminal.
