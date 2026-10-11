# Selecting and copying transcript text

Coder keeps mouse capture on so the wheel scrolls the transcript and a Run
box's left edge scrolls the box. Capture also stops the terminal's own
selection, so Coder selects text itself. The approach is ported from
grok-build (`~/work/projects/repos/grok-build`, Apache-2.0, already credited
in `crates/coder-new/NOTICE`); no code was copied.

## What grok-build does

- **Mouse.** It keeps `EnableMouseCapture` (button, drag and SGR reports) on
  for the whole session and handles press, drag and release itself
  (`xai-grok-pager/src/app/mouse.rs`). A press only arms a drag; the drag
  starts once the pointer moves to another cell.
- **Cells to text.** Each frame publishes a selection model
  (`scrollback/text_selection.rs`): for every visible line, its screen row,
  the columns that are selectable (gutters and borders left out), the
  line's copy text, and `joiner_to_previous`, the text that joins a soft
  wrap back to the row above (`None` means a real line break). Endpoints
  are stored by block and line index, not screen row, so a selection
  survives scrolling.
- **Highlight.** Selected cells are repainted after the frame with the text
  color as background and the base background as text, falling back to
  reverse video (`apply_selection_highlight`).
- **Scrolling during a drag.** When the pointer sits within two rows of the
  top or bottom edge, or past it, a timer scrolls one to five rows a tick,
  faster the farther out it is, and the moving end is recomputed from the
  last pointer position.
- **Gestures.** Double-click selects a word (paths and URLs stay whole),
  triple-click a paragraph or table cell. Release after a drag copies; a
  double or triple click copies at once. The highlight stays until the next
  click.
- **Clipboard.** Every copy writes all legs that can reach the person
  (`xai-grok-pager-render/src/clipboard`): the native tool (`pbcopy` on
  macOS, `wl-copy`/`xclip` on Linux), `tmux load-buffer` inside tmux, and
  OSC 52 (`ESC ] 52 ; c ; base64 BEL`, wrapped in tmux's DCS passthrough
  under tmux) when it can reach a remote or multiplexed terminal.

## What Coder does

`crates/coder-terminal/src/selection.rs` holds the model and
`crates/coder-new/src/ui.rs` (`TranscriptCache`) holds the state.

- Every transcript row carries a `RowCopy`: decoration cells at its start
  (`❯`, `●`, `⎿`, a Run box's bar, a diff's line number and `+`/`-`
  column, hanging indents), further decoration ranges (quote bars, table
  borders; a table's inner borders copy as tabs), a joiner when it is a
  soft wrap, and `omit` for rows that are all decoration (table rules,
  hunk separators, a Run box's "1–5 of 40" line). Markdown rows get theirs
  from `markdown::wrapped_rows`, Run output from its wrap ranges, Edit and
  Write diffs from `diff::content_column`, and other tool rows drop their
  leading mark spans (`tools::plain_copy`).
- A selection's ends are transcript rows, so it stays on the same text as
  the view scrolls or new output arrives. A resize clears it.
- Copies take the selected cells minus decoration, trim row ends, and join
  soft wraps with the text their break consumed.

### Gestures and keys

| Gesture | Result |
| --- | --- |
| Click-drag over the conversation | Selects; copies on release |
| Drag past the top or bottom edge | Scrolls while held, faster farther out |
| Double-click | Selects and copies the word (paths, URLs stay whole) |
| Triple-click | Selects and copies the whole line, its wraps included |
| Cmd+C or Ctrl+Shift+C | Copies the selection again (where the terminal passes the key) |
| Esc | Puts the selection down |
| Click | Puts the selection down |
| Wheel | Scrolls as before; at a Run box's left edge, scrolls the box |

"Copied" shows on the row under the input for two seconds.

### Clipboard

`coder_terminal::clipboard::to_clipboard` writes OSC 52 (with tmux
passthrough under tmux), `tmux load-buffer -` under tmux, and on this
machine (not over SSH) the first native tool that starts: `pbcopy`, or
`wl-copy`, `xclip -selection clipboard`, `xsel --clipboard --input`.
Helpers run off the draw thread.

## Cmd+A

Cmd+A selects the input when the key reaches Coder (kitty keyboard
protocol; Coder pushes `DISAMBIGUATE_ESCAPE_CODES`, which reports Cmd as
the super modifier). Many macOS terminals never send it: the app's Edit
menu has Select All on Cmd+A, and AppKit runs the menu shortcut before
the terminal view sees the key. No escape sequence an app can send (kitty
flags, xterm `modifyOtherKeys`, mouse modes) changes that.

- **Rex** is built on libghostty (`GhosttyVT`, `GhosttyTerminal`
  frameworks) and its binary has the AppKit `selectAll:` action, so Cmd+A
  selects the terminal screen. Rex's key bindings (`init.lua`, key
  rewrites) are the only way to change that on its side.
- **Ghostty** binds `super+a` to `select_all` by default;
  `keybind = super+a=unbind` in its config passes the key through.
  grok-build turns its Cmd+A handler on for Ghostty only, for this reason.
- **iTerm2** and **Terminal** keep Cmd+A for Select All.

Where Cmd+A doesn't arrive, Ctrl+Shift+A selects the input (`/help` and
`coder --help` say so).
