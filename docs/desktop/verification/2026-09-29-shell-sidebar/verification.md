# Desktop shell and sidebar, September 29, 2026

[#9993](https://github.com/OpenAgentsInc/openagents/issues/9993) reimplements
the shell and sidebar design from the public Zeron checkout at `~/zeron`,
revision `ed3b1aae`. The implementation uses existing Rust Native semantic
nodes and extends the desktop adapter. The main baseline was updated to
`f672ab5d25` for the pairing changes, then rebased onto `d9e2a1020a` for
the final checks, including the desktop/phone version alignment at 1.0.0.

## Scope and checks

The shell includes grouped sample chats, selection, new sample chats,
section disclosure, a fixed sidebar header and footer, independent clipped
body scrolling, scrollbars, collapse, drag resizing, and command shortcuts.
The live Grid and existing computer/pairing controls remain reachable.
Sample chat actions do not submit tasks. Leaving pairing cancels its codes.

Targeted verification uses Rust 1.97.1 and this checkout's target directory:

```sh
cargo fmt --check -p openagents-desktop -p rust-native-desktop
cargo clippy --locked --target-dir /home/christopherdavid/openagents/target \
  -p rust-native-desktop -p openagents-desktop --all-targets -- -D warnings
cargo test --locked --target-dir /home/christopherdavid/openagents/target \
  -p rust-native-desktop -p openagents-desktop
cargo build --release --locked \
  --target-dir /home/christopherdavid/openagents/target -p openagents-desktop
cargo test --release --locked \
  --target-dir /home/christopherdavid/openagents/target \
  -p openagents-desktop --bin openagents-desktop \
  shell_paint_benchmark -- --ignored --nocapture
target/release/openagents-desktop --capture /tmp/openagents-shell-9993-final
```

Formatting, Clippy, tests, the optimized build, the timing test, and all
12 capture fixtures pass. The final ordinary test run has 105 passing tests and
two ignored tests: the timing test, run separately above, and the existing
integration test that writes a real systemd user unit. That unit test was
not run. No owner tasks or pairings were created by the fixtures.

Regression coverage includes width bounds, hidden sidebar controls, clipped
pointer targets, fixed headers and footers during scrolling, focus access,
stale activations after leaving pairing, and unchanged task lists after
sample navigation. Retained frames match complete frames pixel for pixel
through hover, focus, press, scrolling, resize, collapse, route changes, and
QR rotation at 1× and 2× scale, with transparent and opaque backgrounds.
Optimized spans match the original per-pixel coverage for fractional shapes,
clipping, strokes, and alpha. A blocked screen-lock probe leaves UI reads
available, and lock changes wake the window.

## Foreground performance

The first live review reported severe lag. Whole-window CPU foreground
painting was the bottleneck: warmed median painting took 121.14 ms at
1200×840, 207.77 ms at 1271×1428, and 460.57 ms at 1200×840 with 2× scale,
in the unoptimized build. Layout took about 0.12 ms.

The window now retains foreground pixels, compares drawing operations,
repaints changed regions, and uploads only those regions over the live Grid.
The painter culls hidden operations and clips raster loops before drawing.
Large rectangles use spans instead of a distance calculation per pixel.
Screen-lock polling runs on a separate thread and cannot block pointer
handling or wait behind a slow host request.

With the same unoptimized profile, warmed hover painting fell to about
2.6 ms at 1× and 8.5 ms at 2×. The app launched for final review uses the
optimized build. Its final timing run measured:

| Interaction | Logical size | Scale | Layout p50 / p95 | Paint p50 / p95 |
| --- | --- | --- | --- | --- |
| Hover | 1200×840 | 1× | 0.02 / 0.02 ms | 0.19 / 0.20 ms |
| Scroll | 1200×840 | 1× | 0.02 / 0.02 ms | 0.81 / 0.87 ms |
| Resize | 1200×840 | 1× | 0.05 / 0.68 ms | 1.66 / 2.94 ms |
| Hover | 1271×1428 | 1× | 0.01 / 0.01 ms | 0.12 / 0.13 ms |
| Scroll | 1271×1428 | 1× | 0.01 / 0.01 ms | 0.11 / 0.12 ms |
| Resize | 1271×1428 | 1× | 0.02 / 0.51 ms | 1.78 / 2.67 ms |
| Hover | 1200×840 | 2× | 0.01 / 0.02 ms | 0.36 / 0.55 ms |
| Scroll | 1200×840 | 2× | 0.01 / 0.02 ms | 1.61 / 2.35 ms |
| Resize | 1200×840 | 2× | 0.02 / 0.36 ms | 3.35 / 3.72 ms |

Each case warms four interactions and records 20. The reported p95 is the
maximum of those 20 samples. These times cover CPU layout and foreground
painting with warm font caches. They exclude GPU Grid rendering, uploads,
display latency, and cold startup. The tall sidebar fits all sample rows,
so its scroll case mostly measures hover. This is evidence for the bounded
sample shell, not for an unimplemented large transcript.

## Captures and remaining work

Headless fixtures retain sample data only:

- [Welcome](shell-welcome.png)
- [Selected chat](shell-selected-chat.png)
- [Collapsed sidebar](shell-collapsed.png)
- [Wide sidebar](shell-wide-sidebar.png)
- [Minimum window, 760×540](shell-minimum.png)
- [Settings](shell-settings.png)

The sidebar data is local to the window and bounded at 40 sample chats.
Real chat state, persistence, a transcript, an editable composer, overlays,
and review panes remain outside this issue. The existing view schema is
unchanged. Phone hosts and native macOS/Windows input were not exercised
on this Linux machine.

A [live tiled preview](live-tiled.png) shows the optimized shell with the
Grid behind it at 1697×711 pixels. It opened in the compositor's ordinary
pane layout. A temporary fixed-size floating preview was closed; no
floating window rule was saved. Live keyboard automation was not used as
acceptance evidence because desktop focus changed during the check.
