# Zeron chat fidelity verification

Progress on [#10029](https://github.com/OpenAgentsInc/openagents/issues/10029).
The [closeout record](closeout.md) maps the acceptance items to evidence and
lists the remaining mismatches. These slices establish matching component
values; they do not claim pixel-for-pixel fidelity for the completed screen.

## Palette viewport and titlebar navigation

`ad40a50bef` adds Rust Native `Style.viewport`: a vertical stack shown through
a bounded window the application scrolls. The desktop adapter clips drawing
and hit targets to it, clamps the offset, fades each edge that hides content,
and reports the region. The command palette now follows Zeron's scrolling list
instead of a window of whole rows: results up to (height − 180) points clamped
to 100–360, every action and the first 30 matching conversations, 8-point
insets that scroll with the rows, a 2-point gap, an 18-point edge fade, wheel
scrolling by points over the results, keyboard reveal with the least movement,
and hover that never scrolls. While a command overlay is open it owns the wheel,
so the conversation beneath no longer scrolls behind the scrim.

The titlebar now matches Zeron's cluster: a 24-point sidebar toggle, Back and
Forward two points apart, and the plus, with groups eight points apart and
centered 21 points down. The title uses 12-point medium type at 85% and starts
16 points past the sidebar, followed by the project as a muted target. Back and
Forward step through visited conversations and pages. New Forward and Plus
glyphs use the reference artwork.

The viewport, palette, titlebar, and closeout transcript checks pass at both
window sizes; the palette and titlebar checks also run at 1× and 2×. Captures:
[titlebar](titlebar-1200x840-1x.png), [titlebar at minimum](titlebar-760x540-2x.png),
[scrolled palette](palette-scroll-1200x840-1x.png), and
[scrolled palette at minimum](palette-scroll-760x540-1x.png). Four native runs at
this commit keep every steady phase below 8.3 ms at p99 with no acquisition
stall; the cold palette frame still misses in two of three default runs. See the
[closeout record](closeout.md#native-benchmark).

## Stable palette hover

Keep the palette's visible range separate from its selected row. Previously,
hovering a row near the list edge recentered the range, moving another row
under the pointer. Subsequent pointer movement could change selection and move
the rows again. Hover now changes only selection. Arrow keys reveal the next
row with the minimum necessary movement; opening and filtering reset the range.

The regression uses 500 chats at 1,200 × 840 and 760 × 540 points, at both
1× and 2×. It checks unchanged row rectangles and menu bounds through repeated
edge hover, then verifies forward and backward keyboard navigation. All ten
command fixtures pass, including retained-versus-complete repaint comparisons,
draft preservation, menu input admission, and profile navigation. This fixes
the selection-driven layout shifts; the native acquisition stalls recorded
below remain a separate unresolved latency failure.

## Paint before waiting for a drawable

Move CPU foreground painting before native surface acquisition. Keep texture
uploads after successful acquisition, preserving the occlusion memory fix.
A skipped acquisition invalidates the prepared CPU submission so the next
successful frame uploads the complete foreground even when its scene stays
unchanged. The recovery regression removes a card before a skipped submission
and verifies that its stale pixels are cleared on retry.

Both native runs retain all 690 active samples and the first opening frames.
They use 3,300 rows, 500 chats, a 1,200 × 840 point window at 2×, and the Grid
backdrop. Against the earlier acquire-first run, palette first-frame submission
falls from 33.155 ms to 10.638 ms and 17.004 ms; chat-menu first-frame submission
falls from 27.089 ms to 12.590 ms and 20.303 ms. These are separate runs, not
paired samples. The repeat still misses the 8.3 ms target: frame p99 is
24.382 ms for scroll, 22.670 ms for streaming, 22.756 ms for sidebar, 21.010 ms
for composer, 20.421 ms for commands, and 20.907 ms for chat-menu selection.
The first run's low acquisition times in sidebar and composer do not recur
consistently. Idle CPU is 3.57% and 4.01% of one core; peak RSS is 209.3 MiB
and 202.8 MiB. Six and three skipped acquisitions leave bounded memory and
recover to complete coverage.

Retain the [first run](native-paint-before-acquire.json) and
[repeat](native-paint-before-acquire-repeat.json), including their failures.
Adapter checks: 82 tests passed; the separate GPU menu upload/readback check
also passed. All ten desktop command
fixtures, strict all-target adapter Clippy, scoped formatting, and the
optimized desktop build pass. This improves opening latency without claiming
the full performance acceptance or display scanout timing.

## Rename dialog

Reimplement the public reference's centered rename dialog: 360-point card,
20-point insets, 16-point corners, 15-point semibold title, and a framed
14/22.75 field. The field follows a 12-point top margin; the trailing action
row follows a 16-point margin with an eight-point gap. Cancel is a quiet ghost
button, followed by the single primary Rename button. Both use 13-point type,
12×6-point content insets, and eight-point corners. The surrounding scrim
consumes outside presses; Cancel, Escape, and a successful rename dismiss it.
The first Escape during composition cancels the composition.

The conversation and composer remain mounted, including the centered empty
composer. The geometry, draft, IME, and keyboard-order fixture passes at both
window sizes and scales. The retained-versus-complete repaint fixture now
covers 28 steps, including dialog opening, editing, and dismissal. Desktop
checks: 132 library tests, 95 binary tests, and three version checks pass;
five opt-in checks remain ignored.

Captures: [default 1×](rename-1200x840-1x.png),
[default 2×](rename-1200x840-2x.png), [minimum 1×](rename-760x540-1x.png),
and [minimum 2×](rename-760x540-2x.png).

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

## Occluded surface uploads

Three native keycap runs are incomplete: [initial](native-keycaps-incomplete.json),
[visible attempt](native-keycaps-partial-visible.json), and
[repeat](native-keycaps-partial-repeat.json). Some phases have no submitted
frames or insufficient samples; these are not passing benchmark results.
Two runs record peak RSS near 1.4 GiB while acquisition repeatedly fails.
The visible attempt records 184.7 MiB and all phases, but only 36 scroll
samples.

Inspection identifies a renderer ordering problem: `write_texture` runs
before surface acquisition, while a skipped acquisition returns without
submitting the queue. Pending upload allocations can accumulate across
retries. The renderer now acquires first, so an occluded, timed-out, lost,
or outdated surface queues no foreground upload. Failed acquisitions also
appear in the bounded, content-free timings log and the native report.

The report retains at most 512 skipped-frame samples and counts dropped
samples separately. It marks coverage explicitly and returns an error unless
the first five active phases each contain at least 90 submitted samples,
the chat menu contains 20, and both first menu frames are present. The
short-lived benchmark window stays above other windows while it samples;
ordinary product windows retain their existing window level. The benchmark
coverage regression, 60 adapter tests, scoped formatting, and strict
all-target Clippy pass. Native verification follows the merged build.

## Acquisition-first native follow-up

The [merged-build report](native-acquire-first-incomplete.json) remains
incomplete: 245 occluded acquisitions prevented the commands and chat-menu
phases from submitting frames. Peak RSS is 189.7 MiB, compared with about
1.4 GiB in the earlier skipped-acquisition runs. This supports the upload
ordering fix's memory bound; it does not establish a complete timing pass.
The benchmark rejects the missing coverage.

## Sidebar context and title

Reimplemented Zeron's two-line sidebar typography: the context appears first
at 11 points with 16-point line height, followed by the title at 13 points
with 17-point line height. Each line ellipsizes independently while the
semantic button retains its complete label. A leading glyph aligns with the
title. The 45-point chat row stays fixed. Captures:
[default](sidebar-type-1200.png) and [minimum](sidebar-type-760.png).

Five chat-management checks, 88 core tests (one benchmark ignored), 60 desktop
adapter tests, scoped formatting, and strict all-target Clippy pass. The
product regression checks context order, both font sizes and line heights,
and the fixed row height at both window sizes.

## GPU upload regression

The explicit graphics-adapter regression passes on this Mac:
`cargo test -p rust-native-desktop partial_menu_uploads_preserve_the_gpu_foreground --lib -- --ignored`.
It reads back every pixel of 32 submitted GPU frames: opening and closing a
rounded menu, moving its selection, stationary frames with no texture write,
and replacing the texture when the backdrop resolution changes. Each pixel
matches the complete software frame's composited color within one byte.
The menu region starts at an uneven offset and uses a partial-width upload,
so the check also covers row strides and preserved surrounding pixels.
Strict adapter Clippy and formatting pass. This is GPU completion evidence
for the upload and compositing path; it does not measure display scanout.

The original scratch chat preview's process started at 10:34, before the
retained menu damage and texture replacement fixes. It stays open to preserve
its temporary chats and draft. A separate updated preview provides the merged
build without replacing that state.

## Palette history and pointer selection

Reimplemented Zeron's palette history rows with the sidebar's 11/16-point
context and 13/17-point title, a 45-point row, an 8-point radius, and a section
rule with 8-point vertical insets. Action rows retain their 30-point minimum
and 10-point radius. Palette results use 8-point outer insets, making action
labels start 16 points inside the card. The search placeholder follows the
reference. Captures: [default](palette-history-1200.png) and
[minimum](palette-history-760.png).

Explicit pointer motion and keyboard navigation now share one selected row.
A resting pointer cannot add a second hover fill or steal selection when
scrolling moves a row beneath it. Hover looks up a bounded map of mounted
rows; it does not rebuild the full chat registry for each pointer event.
The generic button hover fill remains optional, preserving other adapters'
existing defaults.

Seven product menu checks pass, including mouse-to-keyboard handoff, history
geometry and typed activation, draft preservation, and retained/full repaint
parity across 24 transitions at both scales. Core: 88 passed, one ignored.
Adapter: 60 passed, the explicit GPU check is ignored in the ordinary suite
and passed separately in the previous slice. Strict all-target Clippy and
scoped formatting pass.

## Empty conversation docking

Reimplemented Zeron's quiet empty canvas with a centered composer, including
its 8-point offset. The desktop no longer inserts a synthetic welcome message
or the four large starter buttons. Shared phone suggestions remain available
to the phone hosts; real reply actions and cards remain in desktop chats.
The composer docks at the bottom when the conversation has rows, preserving
the same editor lifetime, draft, selection, and typed submission. Attachments
and expanded drafts fit at both sizes. Archived chats and task views retain
their existing docking. Captures: [default](empty-chat-1200.png) and
[minimum](empty-chat-760.png).

The geometry regression checks the empty reading pane, centered field's
visible input region, and bottom docking after a reply while retaining
trailing spaces in the draft. The card interaction fixture now exercises
a real reply's follow-up instead of an empty-state starter button. The
full desktop binary suite passes: 58 tests, with five native/network/timing
checks explicitly ignored. Adapter: 60 passed, one explicit GPU check ignored.
Strict scoped all-target Clippy and formatting pass.

This preserves the shared host's existing empty-chat persistence behavior.
Zeron's draft route creates its conversation on first submission; OpenAgents
still uses its already-created host thread for that draft.

## Native benchmark frame coverage

Active benchmark phases now wait for both 120 input steps and 120 submitted
frames, with a 10-second deadline for unavailable surfaces. Reports include
both counts. A phase retains at most 120 samples after its first five frames,
so extra backdrop frames cannot consume a global sample budget before menus
run. Idle CPU is measured separately. Missing frames still fail the existing
coverage checks; the deadline does not convert an occluded run into a pass.

Regression checks cover coalesced input, blocked presentation, the deadline,
and 1,000 submissions per phase without starving later phases. This improves
the measurement driver; it does not establish a new native performance result.

The optimized native run at `a112fcfbb9` now has complete coverage: 115
samples in all six active phases, both opening frames, and one skipped
acquisition. [Retained report](native-frame-coverage.json). Chat-menu p50/p99:
3.156/4.426 ms. Palette: 7.000/9.931 ms; first palette/menu frames:
14.867/12.018 ms. Scroll: 7.710/19.314 ms; streaming: 8.297/11.555 ms;
sidebar: 5.335/14.140 ms; composer: 3.515/7.419 ms. Idle CPU: 3.84% of one
core; peak RSS: 206.0 MiB. This passes coverage, but several frame timings
exceed the 8.3 ms target. Acquisition accounts for 12.251 ms at scroll p99;
the report retains painting, upload, and presentation separately.

The empty scratch preview was refreshed from the same optimized build.
Repeated command-palette and chat-menu openings, keyboard selection, and
closing render correctly. The original morning fixture remains open with its
temporary chats. The updated preview enables the existing local, content-free
timing log. GPU readback and software repaint parity remain the automated
checks for texture preservation; screenshots do not prove display scanout.

## Authored colors and inline typography

Reimplemented the reference's authored translucent colors: soft-white
selection wash at 11%, user-bubble wash at 8%, white hairlines at 8%, and the
cool composer border at 9%. These blend against their actual destination,
rather than using one opaque approximation across different backgrounds.
The raw accent is RGB 124/134/255, matching the public source's independently
checked OKLCH conversion. Inline code uses that text color and its 12% wash,
14-point monospace type, a 2-point vertical inset, and a 4.5-point radius.
Links retain monochrome text and their inert destinations. Strong Markdown
uses semibold, preserving heavier table-header text. Ordinary menu labels use
90% text coverage and brighten when selected.

These are optional shared layout metrics; the defaults preserve existing
readers. The UTF-8/UTF-16 source ranges and copy text stay unchanged. Core:
88 passed, one timing benchmark ignored. Four reference typography checks
pass, including non-ASCII inline code at both text scales. The full desktop
binary suite passes: 60 tests, five native/network/timing checks ignored.
Repeated-menu repaint parity and pointer/keyboard selection pass with the new
translucent washes. Captures: [default palette](authored-inks-palette-1200.png),
[minimum palette](authored-inks-palette-760.png),
[chat menu](authored-inks-chat-menu-760.png), and
[transcript](authored-inks-transcript-760.png).
Scoped formatting and strict all-target Clippy pass for the core, shared
chat presentation, and desktop.

## Submitted menu telemetry and the subsequent native run

The [authored-color native report](native-authored-inks.json) retains the next
optimized run at `f1ca0c2777`: all six active phases complete, with at least
115 submitted samples each and both first menu frames. This run does not pass
the 8.3 ms latency target. Commands are 11.877/23.616 ms at p50/p99; chat-menu
selection is 13.782/22.778 ms. Chat-menu paint is 0.279/0.424 ms, upload is
0.198/0.326 ms, and acquisition is 10.740/19.691 ms. Idle CPU is 5.62% of
one core; peak RSS is 205.6 MiB. Other worktrees were compiling concurrently;
this run does not establish how much that load contributed to the stalls.

The existing bounded, content-free local timing log now records `ModalOpen`,
`ModalFrame`, and `ModalClose` after successful submissions. Each includes CPU
frame duration and foreground damage counts. A stationary menu can therefore
be distinguished from repeated repainting, and failed acquisitions cannot
produce false open or close records. No menu identifiers, queries, or chat text
enter the log. The transition and blocked-writer regressions, all seven menu
fixtures, scoped formatting, and strict all-target desktop Clippy pass.

The chat menu and archive confirmation no longer focus or accept text into
the palette's hidden search editor. Previously, typing or an IME commit could
filter all menu rows out despite the absence of a visible search field. Only
the palette searches; menu arrow, Tab, Enter, and Escape behavior is retained.
The new regression compares menu geometry and pixels through text, preedit,
composition cancellation, and four idle ticks at both display scales, then
checks that the original draft remains intact. All eight menu fixtures and
strict all-target desktop Clippy pass. Formatting is checked on the changed
files; an unrelated formatting difference in `worker.rs` is left unchanged.
The field caret also uses the pinned Zeron accent instead of the earlier
approximation.

The updated scratch preview at `4cfa05d9d1` keeps the chat menu visible through
native typing, keyboard selection, and repeated openings. Its two chat-menu
openings submit in 3.183 and 3.092 ms. Stationary frames have zero damaged pixels
and regions, but still include acquisition stalls, reaching 23.640 ms in this
short interaction. The first palette frame is 22.529 ms. These are CPU and
submission measurements, not scanout verification or a latency pass. The
[local timing snapshot](menu-stability-preview.jsonl) retains all available
content-free samples from the interaction.

The native event loop now avoids requesting a foreground frame for consumed
keyboard or IME events when the cached view, layout, interaction, and tracked
surfaces are unchanged. It still redraws editing, selection, focus, menu,
semantic-view, and surface-height changes, and remains conservative for custom
surfaces without revisions. The intended behavior is to remove surface acquisition from unchanged
input rather than repainting or presenting the same menu again. Backdrop
animation remains independently scheduled. Input timing records only pending
foreground work, and IME events now have the same input-duration records as
keyboard events.

The redraw-decision regression covers unchanged input, editing revisions,
surface geometry, opening a modal, keyboard focus, missing caches, and loss of
surface revision tracking. Native adapter: 62 passed, one GPU check ignored.
Full desktop binary: 62 passed, five native/network/timing checks ignored,
including pairing captures, keyboard and IME editing, attachments, retained
repaint parity, and clicks during streaming. Scoped formatting, diff whitespace,
and strict all-target native-adapter/desktop Clippy pass.


The native preview at `dfa97ca990` exposed a remaining invalidation defect:
19 ignored text/key events produced nine unchanged modal frames and surface
acquisitions. The menu rows stayed visible, but the redraw optimization did not
hold in that run. The cache compared a filling transcript's allocated height
with its intrinsic height, so a stable layout repeatedly appeared resized.
The adapter now caches intrinsic surface requests separately and compares
successive requests, including width changes and loss of size information.
It also caches the normalized scroll offsets after layout. Regression coverage
checks a 700-point allocated surface with a stable 400-point intrinsic height,
actual width/height changes, and stable desktop view and surface revisions
through ignored text, IME, and idle ticks. Native verification of the revised
preview remains required; these checks do not establish scanout behavior.


For the intrinsic-size fix, the native adapter passes 68 checks (one GPU check
ignored), and the full desktop binary passes 63 checks (five native, network,
and timing checks ignored). Strict all-target Clippy for both packages, scoped
formatting, and diff whitespace checks pass.


The optimized scratch preview at `c5ba84826d` confirms the correction for the
ignored-input case: 19 text/key events generate zero foreground frames, zero
surface acquisitions, and zero modal frames. The chat menu remains visible.
The first opening submits in 1.265 ms. Repeated native chat/palette openings
and closings still submit real transitions; all available content-free local
samples are retained in [intrinsic-size-preview.jsonl](intrinsic-size-preview.jsonl).
Palette transitions include a full-window dimming scrim and still reach
10.837 ms in this short run. This evidence establishes submitted-frame
behavior, not scanout timing or the epic's complete latency gate.


### Profile footer and pointer admission

The footer reimplements public Zeron `shell.rs`'s profile trigger and settings
toggle at reference `50cf9e97a32e54a8ea7e1174b80b5adc3b1d2ef4`: 28-point
controls, an 8-point trigger radius, a 16-point circular initial with 10-point
semibold Geist Mono, and a 13/17-point medium label. The settings glyph uses
the reference's actual `settings.svg` at 15 points, inside a 28-point hit area.
Open and hover fills remain authored alpha washes, and the settings button
returns to the previous page. Computers, Grid, saved sessions, and Commands
remain reachable through a local profile menu; no Zeron account or sync
behavior is imported. Generic fallback buttons now use the product's neutral
fill and text colors instead of the adapter's white default.

The native adapter also fixes pointer delivery under popups. It previously
looked for a custom surface anywhere under the pointer, allowing the transcript
behind the menu to consume clicks or change hover selection. Modal admission
now uses the semantic subtree and cached node bounds. It preserves popup
editor selection and capture, rejects a reused surface resource outside the
popup, and clears an underlying drag capture when the popup opens.

The source geometry and navigation fixture covers 1200×840 and 760×540 at
1× and 2×, all four profile actions, repeated trigger activation, settings
return, and draft preservation. Retained-versus-complete painting includes
profile open, ignored typing, and dismissal. Core avatar checks cover style
composition/reset, serialization, and bounds; absent properties preserve
existing views. Frost, shadows, popup animation, and the remaining components
listed earlier still require work; this footer slice is not a claim of full
visual fidelity.


Retained captures: [default at 1×](profile-1200x840-1x.png),
[default at 2×](profile-1200x840-2x.png),
[minimum at 1×](profile-760x540-1x.png), and
[minimum at 2×](profile-760x540-2x.png).
Core: 102 passed, one benchmark ignored. Native adapter: 69 passed, one GPU
check ignored. Desktop library: 113 passed; binary: 64 passed, five native,
network, and timing checks ignored; version checks: three passed. The nine
command fixtures include retained painting of the profile menu. Shared chat: 117 passed. Strict all-target Clippy for core, native adapter,
shared chat presentation, and desktop, scoped formatting, and diff whitespace
checks pass.


The optimized preview at `3a7f088030` verifies real native pointer activation:
the chat menu's Rename row opens its editor, the Local trigger opens the profile
menu, Commands opens the palette, and clicking its search field then typing
filters the rows without delivering text to the underlying composer. The
original preview with the owner's scratch conversations remains running.

The [complete native repeat](native-popup-pointer.json) retains all 690 samples,
both first opening frames, and two warm-up occlusion skips. All six active
phases have 115 samples, but this is a coverage pass and a latency failure.
The palette's first frame submits in 33.155 ms and the chat menu's in 27.089 ms.
Idle CPU is 3.879% of one core and peak RSS is 203.5 MiB. Acquisition contributes
most of the observed stalls. These failures remain required work.
