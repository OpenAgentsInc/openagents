# coder-vt

A small terminal emulator: the VT100 and xterm subset a shell and common
full-screen programs use, applied to a character grid a renderer draws. The
Coder mobile terminal screen feeds it the output of
[NIP-TERM](../../nips/openagents/NIP-TERM.md) frames and draws the grid
through Rust Native in the amber palette
(`coder-computers::terminal`, [issue #9733](https://github.com/OpenAgentsInc/openagents/issues/9733)).

`vte` parses escape sequences; the grid, the cursor, the modes, and the
replies are this crate's. See [the dependency review](../../docs/dependencies.md#terminal-emulator-parser).

## Supported

| Area | Sequences |
| --- | --- |
| Printing | Autowrap with a deferred wrap (DECAWM), insert mode (IRM), line feed/new line mode (LNM), UTF-8 split across feeds, wide and combining characters, DEC line drawing (`ESC ( 0`, SO/SI), repeat (`REP`) |
| Cursor | CUU, CUD, CUF, CUB, CNL, CPL, CHA, HPA, HPR, VPA, VPR, CUP, HVP, CHT, CBT, HT, BS, CR, IND, NEL, RI, DECSC/DECRC, SCOSC/SCORC, origin mode (DECOM), tab stops (HTS, TBC) |
| Editing | ED 0–3 (3 clears the scrollback), EL, ICH, DCH, ECH, IL, DL, SU, SD, DECSTBM, DECALN; erase uses the current background |
| Rendition | SGR bold, dim, italic, underline, blink, inverse, hidden, strike and their resets; 16, 256, and 24-bit colors in semicolon and colon forms |
| Modes | Alternate screen (47, 1047, 1049), save cursor (1048), cursor visible (25), application cursor keys (1), bracketed paste (2004), soft reset (DECSTR), full reset (RIS) |
| Replies | Device status (`CSI 5 n`), cursor position (`CSI 6 n`), primary and secondary device attributes; bounded, taken with `take_replies` |
| Other | Window title (OSC 0 and 2, bounded), bell count, bounded scrollback for the primary screen |

`Terminal::mark` writes a client line, such as a note that the host discarded
output, on a line of its own with the `MARKER` flag, and abandons any sequence
the lost bytes would have finished.

`input` encodes keys (characters with Ctrl and Alt, Enter, Tab, Shift-Tab,
Backspace, Escape, arrows under the cursor key mode, Home, End, Page Up and
Down, Insert, Delete, and F1–F12, with xterm modifier parameters) and pastes:
line endings become carriage returns, control characters other than tab are
removed so a paste cannot close a bracketed paste early, and mode 2004 wraps
it in the paste markers.

## Not supported

Mouse reporting, other operating-system commands (including OSC 52 clipboard
access, which is deliberately ignored), DCS strings such as Sixel, reflow of
wrapped lines on resize, double-width lines, and 132-column mode. Unknown
sequences change nothing.

## Checks

```sh
cargo test -p coder-vt
cargo clippy -p coder-vt --all-targets -- -D warnings
```

The tests cover each area above, a typical shell and full-screen session, and
a deterministic stream of arbitrary bytes and fragments across resizes that
must keep the grid well formed.
