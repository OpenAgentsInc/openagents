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

The owner's follow-up on the jump control and menus is implemented in
`c74faf63d7`. The **Scroll to bottom** control is a 30-point dark pill floating
six points above the composer. The keyboard notice below the composer is
removed. Commands now float in a centered 560-point card with a 44-point search
header and a quiet scrim; chat actions use a 216-point floating card. Context
menus anchor at the pointer and clamp to eight points from the window edges.
Rows use 13-point type and seven-point menu corners. The conversation and
composer stay mounted, and opening either overlay preserves their bounds.
Escape, outside clicks, keyboard selection, IME admission, and unsent drafts
pass the interaction checks at default and minimum sizes.

Captures: [jump pill](floating-latest-760.png),
[commands](floating-palette-minimum.png), and [chat actions](floating-header-menu-minimum.png).
Desktop library: 83 passed; binary: 38 passed and four ignored; version checks:
three passed; adapter: 54 passed. Strict Clippy, formatting, and the optimized
build passed. The rebased native adapter also compiles with main's new input
boundary. [Native report](native-floating.json), 115 samples per active phase:
scroll p50/p99 3.662/3.981 ms, streaming 3.798/4.470 ms, sidebar
1.834/4.947 ms; idle CPU 2.22% of one core, peak RSS 209.7 MiB.
Menu icons, shortcut badges, shadow, and blur still need the final reference
comparison; this slice does not close #10029.

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

## Menu repaint stability

The owner reported flickering menus. The open preview was still running the
older replacement-panel build; a separate scratch window uses the floating
menu build without discarding that preview's conversations or draft.

Fix a foreground invalidation defect in the native compositor: when a backdrop
change replaces the GPU foreground texture without resizing the CPU frame,
the retained painter must upload the entire frame. Otherwise an unchanged
scene can leave the new texture blank. The regression verifies full damage
after invalidation and returns to no damage on the next unchanged update.
A second regression compares retained and complete frames through 24 menu
opening, query, hover, keyboard selection, and dismissal steps at 1× and 2×.
It passes with the actual overlay layout and surface revision keys.
Desktop adapter: 56 tests passed; five command fixtures passed; strict
all-target Clippy and scoped formatting passed. Native checks follow after
building the merged commit. These checks do not establish the cause of every
flicker seen in the older preview.

The extended latency fixture also measures composer editing, command filtering,
and chat-menu keyboard selection. The uncovered repeat records 115 frames
for scroll, streaming, sidebar, composer, and commands, and 25 changed frames
for the chat menu. Frame submission p50/p99: scroll 3.105/3.674 ms, streaming
3.521/3.962 ms, sidebar 2.268/5.825 ms, composer 1.157/3.049 ms, commands
5.584/6.406 ms, and chat menu 0.984/1.073 ms. Idle CPU is 2.22% of one core;
peak RSS is 206.2 MiB. This is CPU submission timing, not GPU completion or
scanout. [Retained report](native-controls.json).


The foreground fix landed as `2c83ae4daf`. Its optimized build passed, and
the refreshed scratch preview opened, filtered, and switched menus successfully.
The [post-fix native report](native-menu-fix.json) records commands p50/p99
5.972/6.779 ms and chat-menu selection 1.213/1.404 ms. Idle CPU is 2.23% of
one core. The older fixture remains open to preserve its temporary chats.

## Markdown dimensions

Reimplement Zeron's heading and code dimensions as optional shared layout
metrics: heading font/line sizes 19/27, 16/24, 15/22, and 14/22 in semibold;
code 12.5/18, 28-point header, 11-point language label, ten-point vertical
padding, and a 24×22 copy control. Default callers retain their existing
metrics and serialized copy widgets. Copy retains the original code bytes.
The following Solar artwork slice supplies the exact copy glyph.

Shared application: 104 tests passed; core: 87 passed and three opt-in checks
ignored, including the now-fixed CoreText corpus digest check; desktop adapter:
57 passed. Strict all-target Clippy passed for core, adapter, shared application,
and desktop. This remains a component slice of open #10029.

## Solar artwork and shortcut badges

Render Zeron's public Solar Linear assets for the chat controls, command
palette, and chat menu, including the code-copy control. Keep the artwork's
CC BY 4.0 attribution beside the embedded files. Cache alpha masks by asset
and pixel size, then tint at paint time; a bounded cache avoids parsing SVGs
on each frame. The source interface's attachment glyph is 18 points inside
a 28-point hit area. Menu labels use regular 13-point Geist, with muted
16-point icons and a ten-point gap.

Shortcut badges use ten-point Geist Mono and retain typed activation separately
from their display hints. Geist lacks the Command symbol, so that symbol alone
uses the existing bundled JetBrains Mono fallback. Verify both macOS badges
and the portable Ctrl label path without changing application shortcuts.

[Default palette](solar-palette-1200.png),
[minimum palette](solar-palette-760.png),
[chat menu](solar-menu-760.png), and
[context menu](solar-context-menu.png) retain this component slice. The
retained-versus-complete repaint comparison passes through 24 menu transitions
at 1× and 2× with the actual vector masks and surface revision keys.

Shared application: 104 tests passed; core: 88 passed and three opt-in checks
ignored; adapter: 58 passed; command fixtures: five passed. Scoped formatting
and strict all-target Clippy pass for core, adapter, shared application, and
desktop. All-target checks pass for Coder computers, mobile, terminal, and the
chat load benchmark. The separate OpenAgents phone workspace also compiles.

The dependency gate reports the same 30 failures as parent `482b8b811b`, after
reviewing the new `arrayref` 0.3.9 BSD-2-Clause license and retaining its notice
under a per-version exception. Existing source, advisory, license, wildcard,
and yanked-version failures remain recorded; this slice adds none. See the
[dependency review](../../../dependencies.md#embedded-vector-artwork).

Menu shadow and background blur, tabs, new-chat composition, activity chrome,
syntax colors, and the project/model footer still need the reference pass.
This slice does not complete #10029. After integration with `2129cbc83a`, the shared application passes 109 tests;
the desktop passes 92 library tests, 53 application tests (five opt-in checks
ignored), and three version checks. Native performance verification follows
the optimized build rebased on current main.

## First menu frames

The native report now retains a bounded `openings` array: the first submitted
frame after opening the palette and the chat menu. Keep these separate from
the steady-state samples, which omit the first five frames in each phase.
This includes SVG mask preparation and full damage when a menu first appears.
It uses the same CPU work and submission measure; it does not measure GPU
completion or scanout. The local timings writer remains opt-in, bounded, and
content-free.

The first native Solar repeat at `9bccdcf814` exposes a remaining cost:
palette filtering p99 is 8.618 ms and the first palette frame is 9.851 ms.
The palette repaints and uploads the whole 4,032,000-pixel foreground whenever
its filtered rows change the display-list length. The [raw report](native-solar-before-damage.json)
retains that failing performance result; scroll, streaming, sidebar, composer,
and chat-menu selection remain below 8.3 ms at p99.

The retained painter now compares drawing operations with their effective
pixel clips. A changed clip, insertion, or removal damages the old and new
clipped drawing bounds, while stable leading and trailing drawing stays
retained. Dynamic surfaces still refresh, and texture invalidation still
forces full damage. The actual menu regression verifies byte-for-byte parity
with complete frames and bounds filtering damage below 70% of the minimum
window at both scales. Adapter: 59 tests passed; five command fixtures and
the broader shell retained-paint regression pass; scoped formatting and
strict all-target Clippy pass. A native repeat follows the merged build.

The optimized build of `4aa4d78772` passes. The [uncovered repeat](native-solar-damage.json)
records scroll p50/p99 2.909/3.307 ms, streaming 3.869/4.395 ms, sidebar
1.946/3.355 ms, composer 2.010/3.976 ms, palette filtering 3.433/3.873 ms,
and chat-menu selection 2.099/2.223 ms. Each of the first five phases has
115 samples; the chat menu has 25 changed frames. Palette damage falls
from 4,032,000 to 775,760 pixels at the median. Idle CPU is 2.23% of one
core; peak RSS is 199.0 MiB. Active p99 timings meet the 8.3 ms target.

First submitted palette and chat-menu frames are 9.162 and 7.589 ms. The
palette's first scrim still requires a full-window update; its cold frame
remains above 8.3 ms and is explicitly retained in `openings`. A previous
[blocked repeat](native-solar-damage-blocked.json) records median window
acquisition waits of 10.682 ms while the scratch preview interferes with
fullscreen presentation. Closing that empty scratch window and repeating
without UI manipulation removes those waits. Both reports remain retained.

## Syntax palette

The transcript and changes pane now use Zeron's dark syntax palette after
its 72% HSL saturation treatment. The semantic colors are reimplemented from
the public MIT reference; compiled tree-sitter queries stay on the bounded
highlighting worker. Rust numbers and booleans retain separate roles instead
of sharing the upstream constant capture. Palette changes discard pending
results from the previous palette without changing source bytes or text
measurements.

The product regression checks comment, keyword, number, macro, and string
colors against UTF-8 source ranges. Both syntax core tests, the rich-text
fixture, the shared visual geometry tests, scoped formatting, and strict
all-target Clippy pass. The earlier full core run passes 88 tests with one
ignored. Issue #10029 remains open for the remaining chat chrome and surfaces.

## Palette keycaps and jump pill

The palette header now shows the platform command shortcut. Its footer uses
separate 16-point keycaps, 10-point labels, 5-point gaps inside each hint, and
12-point gaps between hints. Header and footer separators use the reference's
6% white hairline. Native horizontal stacks can preserve an explicitly
measured content width, which prevents keycaps and short labels from
wrapping during the final layout pass. The regression checks that all three
legends stay on one row at both window sizes.

The jump pill uses a separate muted 13-point down arrow, a regular 13-point
label, a 6-point gap, and 11/13-point left/right insets. Its 30-point height
and 6-point composer clearance remain unchanged. Retained painting matches
complete frames at both scales through opening, filtering, navigation, and
dismissal. Five desktop command fixtures, 88 core tests with one ignored,
59 adapter tests, scoped formatting, and strict all-target Clippy pass.

Retained captures: [palette at 1,200 points](keycaps-palette-1200.png),
[palette at 760 points](keycaps-palette-760.png),
[jump pill at 1,200 points](jump-pill-1200.png), and
[jump pill at 760 points](jump-pill-760.png). Frosted surfaces, empty-chat
docking, tabs, and the sidebar profile remain open fidelity work.
