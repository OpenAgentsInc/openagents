# Terminal design principles

These principles are binding for OpenAgents Terminal: the standalone window
and the same terminal in the Grid's `T` overlay. The owner set them on
2026-10-05. When a design, a review, or the code disagrees with them, they
win; change the design or the code, not the principles.

## The principles

The owner's words, verbatim:

"User interfaces that entirely remove the concept of resizing. Desktop monitors are all fixed 3:2 aspect ratios. Windows cannot be resized by the user; the size is prescribed by the program at compile time: WYSADTIWYG, what you see at design time is what you get. Design interfaces as if they are a physical object like a piece of paper. Keyboards have dedicated F1-F24 function keys. No touch. No animation. Standardized action buttons on the keyboard (CONFIRM/REJECT). Nothing floats; everything is anchored to the window frame. Input latency is sub-millisecond. The UI shows the state of important metrics and program features at all times, visible and never hidden. Nothing slides. Nothing moves. No emojis, only ASCII-128 (no UTF-8). Scrolling is OK with a scroll bar visible at all times. There is no concept of notifications. Computer state in a dedicated area, always visible, cannot be hidden. No HDR in UI. No markdown."

## How OpenAgents Terminal follows them

- **One fixed sheet.** The window is 1200 by 800 points (3:2), set at
  compile time (`terminal_gfx::SHEET_POINTS`), and cannot be resized or
  zoomed. Text renders at the display's backing scale, so it is crisp on a
  Retina display. The Grid draws the same sheet at the same size, anchored
  at the center of the screen.
- **Anchored regions.** From top to bottom: the status area (three rows),
  the transcript with its scroll bar, the input line, and the key strip.
  Nothing floats over them: no previews, popovers, cards, or splits that
  open by themselves.
- **One input line.** Type a command or a question and press ENTER. The
  terminal decides on this computer whether the line is a shell command or
  a question, and the label before the line (`SHELL >` or `ASK   >`) says
  which before you press ENTER. No prefix or mode key is needed.
- **Keys.** ENTER is CONFIRM and ESC is REJECT. Function keys do the rest,
  and the key strip names them at all times: F1 help, F2 context, F3 copy,
  F4 thread, F5 run as shell, F6 ask, F8 panes, and F10 quit.
- **One conversation page.** F4 draws the conversation the input line's
  questions go to in place of the transcript, as plain text with its state
  (`reading`, `current`, `reply arriving`, `missing`, or `unavailable`).
  The input line becomes `REPLY >`, and F4 or ESC returns. Opening the page
  only reads the thread through the shared chat client; it never sends.
- **State is always visible.** The status area shows the directory, the git
  branch and whether it has changes, the last exit status, whether a request
  is running, the queue, pending proposals, where requests go, the load
  average, and the time. The last message the terminal has for you is a
  field there, not a notification.
- **ASCII and the four whites.** Every character on the sheet is printable
  ASCII. Program output, model answers, and the frame map other characters
  to ASCII equivalents or `?`. Colors map to the four whites of the ladder;
  there is no other color and no HDR.
- **No Markdown.** Answers arrive as plain text and are shown as plain text.
  Markdown markers are removed, links keep their address in parentheses, and
  a typed command plan becomes a `PROPOSED:` line, never visible JSON.
- **No motion.** Nothing slides, fades, or blinks; the caret is steady.
- **Latency.** Editing the input line changes only local state, so the next
  frame shows it. The measured key-to-frame time is in the verification
  receipt.
