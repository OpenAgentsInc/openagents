# What changed verification

Code: `bab483419a`. Implements
[#10019](https://github.com/OpenAgentsInc/openagents/issues/10019), part of
[#10003](https://github.com/OpenAgentsInc/openagents/issues/10003).

The card and the pane reimplement Zeron's unified diff pane (public MIT
[zeronsh/zeron](https://github.com/zeronsh/zeron)) in Rust Native. The shared
parser lives in `openagents-chat-app`, which the desktop and the phone
library compile. Mounting the card on a phone remains part of
[#10028](https://github.com/OpenAgentsInc/openagents/issues/10028).

## What the window shows

A finished Coder task that recorded a unified diff shows a **What changed**
card. Opening the card shows that diff in a pane on the right. The pane
scrolls by line and closes. The diff stays read-only.

The desktop paints only the lines that fit in the pane. Syntax spans color
those lines. Line height and text stay the same. The card appears after the
task finishes.

## Checks

Rust 1.97.1 on an M5 Max Mac, rebased onto current `origin/main`:

- Shared application: 103 tests passed. The change tests cover file and line
  counts, syntax spans that leave the line text unchanged, a 5,000-line
  window that does not walk every line (200 scroll steps under 50 ms),
  keeping the last unified diff in a result, and cutting a line past the
  bound on a character boundary.
- Desktop library: 86 tests passed. A finished task's 5,000-line diff stays
  hidden while the task is running, opens in the pane, scrolls by line to
  the end, paints the visible lines with syntax spans, and closes while the
  card remains. The open view stays under 40 nodes and has no composer in
  the pane.
- Desktop binary: 39 passed, four opt-in checks ignored. The floating chat
  controls still preserve the reader and composer geometry.
- Strict all-target Clippy for `openagents-chat-app` and `openagents-desktop`
  passed. Formatting was applied to the files in this slice.

The 5,000-line fixture is generated in the test. These checks use that
fixture. The owner's repository and live host stay unread. The shared
parser's tests are the compile the phone library depends on. Native mounting
remains #10028. This slice has no owner-only step, so it does not add a line
to `NEEDS_OWNER.md`.
