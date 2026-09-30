# Native transcript and long-content measurements

Issue #10005 paints shared Rust Native transcript rows on the desktop. Messages,
Markdown, code, tables, tools, status widgets, and code-copy controls use the
shared layout. Only visible rows paint. Scroll-up stops following the reply;
**Latest** resumes following. A row-key anchor survives streaming and earlier
messages loading. Unchanged updates cause no layout or surface damage. Code and
table blocks scroll horizontally. Tool details truncate with an ellipsis, and
explicit pointer releases expand tools or copy code.

This reimplements Zeron's transcript design in Rust Native. It copies no Zeron
code and introduces no agent harness. Further rich-text interaction is #10010.

## Native measurements

Measured on September 30, 2026, on an Apple M5 Max with 128 GiB of memory, using
pinned Rust 1.97.1 and an optimized build. The fixture uses 3,300 message rows and
500 sidebar chats. Each active phase has 120 scheduled operations, discards the
first five frames, and retains at least 115 samples. p99 uses nearest rank.
Default is 1,200 × 840 points; minimum is 760 × 540 points. Both effective 1x and
2x rendering densities run on the Mac's 2x display, with matching physical window
sizes. These are display-density measurements on one machine, not two monitors.

Frame values below add the scripted application update to the native frame's
CPU submission time, including layout, painting, upload, drawable acquisition,
and presentation. They exclude GPU completion, scanout, and input delivery from
the operating system. CPU is process CPU as a percentage of one core over each
phase. RSS is the process's peak resident memory, not an incremental allocation.
Idle lasts five seconds with a 1,000-line draft. Streaming appends a word every
scheduled update. Verse uses the actual empty renderer with a refused loopback
relay; no owner host, public relay, credentials, or persistent chats are used.

| Case | Scroll p50 / p99 ms | Stream p50 / p99 ms | Sidebar p50 / p99 ms | Idle CPU | Stream CPU | Peak RSS MiB |
| --- | --- | --- | --- | --- | --- | --- |
| default-1x-plain | 1.59 / 1.81 | 5.10 / 6.05 | 1.66 / 1.89 | 0.61% | 33.32% | 146.5 |
| default-1x-verse | 2.29 / 2.54 | 5.67 / 6.51 | 2.40 / 2.81 | 2.86% | 40.94% | 159.8 |
| default-2x-plain | 3.28 / 3.71 | 6.05 / 7.05 | 3.09 / 3.59 | 0.70% | 38.95% | 175.7 |
| default-2x-verse | 2.65 / 4.38 | 6.51 / 7.46 | 2.18 / 4.14 | 3.19% | 45.78% | 184.9 |
| minimum-1x-plain | 1.05 / 1.18 | 5.10 / 5.93 | 1.37 / 1.47 | 0.35% | 32.04% | 139.5 |
| minimum-1x-verse | 1.80 / 2.03 | 5.56 / 6.76 | 2.04 / 2.51 | 2.79% | 40.26% | 157.3 |
| minimum-2x-plain | 1.58 / 1.74 | 5.22 / 5.81 | 2.12 / 2.41 | 0.29% | 33.72% | 152.5 |
| minimum-2x-verse | 2.36 / 2.59 | 5.58 / 6.38 | 1.80 / 3.22 | 2.87% | 40.71% | 166.1 |

All eight scrolling cases pass the 8.3 ms acceptance target. Streaming changes
relayout one transcript row; ordinary scrolling paints only the visible rows.
The retained `relaid` field during scrolling describes the most recent layout
update, rather than a new layout for each scroll event.

GPU text and a new layout solver are not justified by these measurements. Keep
the visible-row painter and retained surface invalidation. Verse's animation
raises idle CPU; its reduced-motion and idle behavior remain part of #10022.
The plain window's long-draft idle measurements replace the earlier callback-only
estimate for #10004. Linux hardware measurements remain unverified and are
listed in `NEEDS_OWNER.md`; no Linux performance claim is made here.

The [summary](summary.json) and [raw samples](samples/) contain only numeric
measurements and fixture configuration. Optional application timing capture uses
`OPENAGENTS_DESKTOP_TIMINGS=/absolute/path.jsonl`. It records bounded phase,
duration, damaged-pixel, and region values through a background writer. It
records no chat text, clipboard data, IDs, credentials, or network payloads and
sends nothing remotely.

## Repeat the matrix

Run on a machine with a native desktop session:

```sh
cargo build --locked --release -p openagents-desktop --target-dir target
python3 scripts/benchmark-desktop-chat.py target/release/openagents-desktop target/chat-matrix
```

The fixture opens and closes eight native windows sequentially. Keep them
visible; closing or occluding a window invalidates the measurement. The script
retains each run and fails when scrolling p99 exceeds 8.3 ms. Use `--summarize`
to recompute the report from existing runs. On Linux, run from the desktop
session so Wayland or X11 can create a real window.

## Checks

- Desktop library, executable, and version tests: 105 passed; three opt-in timing
  tests remain ignored in the ordinary run.
- Desktop adapter: 41 passed, including stable reading anchors, tail following,
  unchanged update invalidation, tool expansion, code copy on release, ellipsis,
  and immediate caret advancement after every trailing space.
- Clicking the transcript cancels marked IME text and rejects a late commit.
- Targeted formatting, Clippy with warnings denied, and `git diff --check` pass.
- The native eight-case matrix passes. No release gate or owner-host smoke ran.
