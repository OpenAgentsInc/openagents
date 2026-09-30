# Zeron chat fidelity verification

Progress on [#10029](https://github.com/OpenAgentsInc/openagents/issues/10029).
This issue remains open. These slices establish matching component values;
they do not claim pixel-for-pixel fidelity for the completed screen.

## Reference and scope

The design reference is the public MIT [Zeron source](https://github.com/zeronsh/zeron/tree/50cf9e97a32e54a8ea7e1174b80b5adc3b1d2ef4),
including its main [screenshot](https://github.com/zeronsh/zeron/blob/50cf9e97a32e54a8ea7e1174b80b5adc3b1d2ef4/docs/screenshot.png).
The owner requested exact fidelity after #10017. This supersedes the earlier
audit's recommendation to retain Inter for desktop chat. Product code remains
Rust Native; no GPUI dependency or private backend is imported. OpenAgents
identity, Coder execution policy, and existing actions remain in place.

Main commits: `4be3f56c39` (Geist faces), `00fc15b218` (palette and reader),
`7670c7d6dd` (controls), `81ca088a5a` (streaming), and
`a75e5eefba` (native header and fixed scale).

- Bundle the reference's 16 unmodified static Geist and Geist Mono faces,
  including weights and italics, with their SIL Open Font License and notice.
  Face selection participates in shaping, glyph, and measurement caches.
- Match the dark canvas #060606, shell #0d0d0d, primary #e5e5e5,
  muted #a3a3a3, 736-point reading band, 14/22 body type, 16-point
  uniform bubble radius, and 80% maximum user bubble width.
- Match the compact composer at 49 points, expanded minimum at 120 points,
  14/22.75 text, 28-point Send control, 26-point pill radius, and one-point
  border. Immediate spaces, multiline editing, drafts, and current task
  actions survive narrow windows and sidebar collapse. Paste remains on
  Cmd/Ctrl+V for text and images.
- Reduce sidebar filter and row density; keep OpenAgents-specific navigation
  in compact footer controls. The header is one line.
- Fix the capture helper to supply viewport scale before measuring or painting
  custom surfaces. A regression checks actual surface bounds at 2×.
- Retain immutable settled projections during streaming. A 3,300-row regression
  verifies row reuse, exact parity with the ordinary shared projection,
  replacement of a changed source, and eviction outside the observed page.

Captures: [default chat list](list-1200.png), [minimum chat list](list-760.png),
[default task](running-1200.png), and [minimum task](running-760.png).
The fixtures use temporary state and do not reach an owner host or engine.

## Checks

Rust 1.97.1 on the M5 Max Mac. Shared application: 91 tests passed.
Desktop library: 83 passed; binary: 36 passed, four opt-in checks ignored;
version lockstep: three passed. Desktop adapter: 54 passed. Core: 86 passed,
three opt-in checks ignored, the previously recorded CoreText corpus digest
check excluded. Strict all-target Clippy and targeted formatting passed.
The phone library compiles with the new contracts using its separate target.
The optimized desktop build passed after rebasing on current remote main.

## Native performance

The native fixture retains 3,300 rows and 500 chats in a 1,200 × 840 point
window at 2× with the Grid backdrop. It records 115 samples per active phase.
Times include application work plus CPU frame submission, not GPU completion
or scanout.

| Phase | Before shared rows p50/p99 | After shared rows p50/p99 |
| --- | --- | --- |
| Scroll | 3.341 / 4.037 ms | 2.805 / 3.401 ms |
| Streaming | 7.701 / 9.309 ms | 3.515 / 3.769 ms |
| Sidebar | 5.595 / 6.048 ms | 5.058 / 6.044 ms |

Streaming application work p99 falls from 6.690 to 1.221 ms. The repeat's
idle CPU is 2.89% of one core; peak RSS is 207.6 MiB. Active phases meet the
8.3 ms target. Retained measurements: [before](native-before-shared-rows.json)
and [after](native-shared-rows.json).

## Remaining fidelity work

The native header now uses 38 points, with controls centered at 21 points,
88-point window-control clearance on macOS, and 12 points in fullscreen.
The native window preserves fixed logical component sizes on larger displays.
A regression checks both window modes at default and minimum sizes, including
the docked composer and unclipped sidebar. Desktop library: 83 passed; binary:
37 passed and four ignored; adapter: 54 passed; strict Clippy and release build
passed. A separate native scratch window verified the unified header and zoom
and fullscreen transitions; the screen-sharing indicator obscures the standard
traffic lights in the captured image, so their final visual comparison remains.

Updated captures: [default](header-list-1200.png), [minimum](header-list-760.png),
and [512 chats](header-512-chats.png). A native repeat with no simultaneous
window manipulation records 115 samples per active phase: scroll p50/p99
2.390/2.670 ms, streaming 3.422/3.980 ms, and sidebar 1.868/4.833 ms. Idle CPU
is 2.21% of one core; peak RSS is 207.1 MiB. These use the same CPU work and
submission measure above. [Retained report](native-header.json).

The titlebar's tab and navigation controls, exact Markdown headings and code chrome, syntax palette,
activity chips, new-chat composition, model badge, project footer, vector
icons, and menu geometry still need comparison and implementation. These are
tracked within #10029 before the next epic feature.
