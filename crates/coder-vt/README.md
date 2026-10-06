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
| Modes | Alternate screen (47, 1047, 1049), save cursor (1048), cursor visible (25), cursor blink (12) and style (DECSCUSR), application cursor keys (1) and keypad (DECKPAM, DECKPNM), bracketed paste (2004), focus events (1004), soft reset (DECSTR), full reset (RIS) |
| Mouse | Tracking modes 9, 1000, 1002, and 1003 with the default, UTF-8 (1005), SGR (1006), and urxvt (1015) encodings; `Terminal::mouse` encodes an event |
| Replies | Device status (`CSI 5 n`), cursor position (`CSI 6 n`), primary and secondary device attributes, Kitty keyboard flags (`CSI ? u`); bounded, taken with `take_replies` |
| Keyboard | Kitty keyboard protocol push, pop, and set (`CSI > u`, `CSI < u`, `CSI = u`), one bounded stack per screen; only flag 1 (disambiguate escape codes) is kept |
| Other | Window title (OSC 0 and 2, bounded), hyperlinks (OSC 8, bounded table, `Attrs::link`), clipboard writes (OSC 52, bounded, never reads; `take_clipboard` leaves the decision to the client), bell count, bounded scrollback for the primary screen with stable line names (`history_dropped`) |

`Terminal::mark` writes a client line, such as a note that the host discarded
output, on a line of its own with the `MARKER` flag, and abandons any sequence
the lost bytes would have finished.

## Snapshots

`snapshot` writes and restores the NIP-TERM snapshot stream
(`openagents.terminal-snapshot.v1`, whose records and checks live in
`coder_pty::ext`). `Terminal::snapshot` writes `TERMINAL`, `STATE`, the rows of
each showing screen, the parser's `CONTINUATION`, and `READY`, then history
pages newest first and `FINISH`; `Terminal::history_stream` answers a history
read. A client feeds the records to a `Restore`, which yields a terminal at
`READY` that draws the screen and continues parsing exactly where the host's
parser stopped, inside an escape sequence or a UTF-8 character. History
pages then attach with `Terminal::attach_history`, which refuses a page from
another line epoch or one that does not adjoin the kept history, and changes
nothing when it refuses.

The continuation is the input since the parser was last at rest. The
emulator follows `vte`'s states to know when that was; replaying the bytes
into a fresh parser with no effects restores its position. Unfinished input
longer than 4,096 bytes is abandoned on both sides, as a cancel would.

A snapshot does not carry the alternate screen while the primary one shows,
a saved cursor's character sets and origin mode, the character `REP`
repeats, shell-integration marks, the bell count, or pending replies and
clipboard writes. The format is this profile's own, ordered as libghostty's
Snapshot v1 is; it does not read libghostty snapshots.

`input` encodes keys (characters with Ctrl and Alt, Enter, Tab, Shift-Tab,
Backspace, Escape, arrows under the cursor key mode, Home, End, Page Up and
Down, Insert, Delete, F1–F24, and the keypad under its mode, with xterm
modifier parameters, or as `CSI code ; modifiers u` for Escape and Ctrl or Alt
chords once a program pushes the Kitty disambiguate flag) and pastes:
line endings become carriage returns, control characters other than tab are
removed so a paste cannot close a bracketed paste early, and mode 2004 wraps
it in the paste markers.

## Not supported

Other operating-system commands (including OSC 52 clipboard reads, which are
deliberately never answered), DCS strings such as Sixel, reflow of
wrapped lines on resize, double-width lines, and 132-column mode. Unknown
sequences change nothing.

## Checks

```sh
cargo test -p coder-vt
cargo clippy -p coder-vt --all-targets -- -D warnings
```

The tests cover each area above, a typical shell and full-screen session, and
a deterministic stream of arbitrary bytes and fragments across resizes that
must keep the grid well formed. `tests/snapshot.rs` restores a session from a
snapshot taken at every byte, sends it through the record checks in parts, and
requires the restored terminal and live output after it to match parsing
without a break; it also covers corrupt, truncated, oversized, and wrongly
bound streams, `READY` without history, and history pages attached among live
output.
