# Desktop chat latency

The chat fixture exposed work that the sidebar benchmark did not cover: each
plain-window redraw cloned, converted, and uploaded the complete frame; unchanged
application surfaces repainted on every update; rounded borders visited every
interior pixel; and clipboard subprocesses blocked native input.

The adapter now uses a retained RGBA GPU texture for plain windows and windows
with a backdrop. It uploads damaged regions. Applications can supply a drawing
revision for each surface; callers that omit one retain the previous behavior.
Opaque fills use bulk copies, and borders skip their transparent interiors. The
composer reads and writes the clipboard on a background thread and rejects a
result for a newer editing revision. Editable text measures trailing spaces,
retains them for selection, and preserves an empty line after a final newline.
The composer and transcript are Rust Native implementations of Zeron's UI design;
no Zeron implementation or agent harness was copied.

## Local timing capture

Set `OPENAGENTS_DESKTOP_TIMINGS=/absolute/path.jsonl` before launching the app.
The adapter records input handling, application ticks, scene layout, CPU paint,
GPU upload submission, surface acquisition, encoding and presentation submission,
total frames, and input-to-presentation submission. Records contain durations,
pixel counts, and damaged-region counts. They contain no chat text, identities,
clipboard contents, keys, or network payloads. No records are sent remotely.

The last 512 samples stay in memory. An optional background writer has a bounded
512-record queue and drops samples rather than waiting on the UI thread. New
files have mode `0600` on Unix. GPU timings measure submission, not GPU completion
or the display's scanout. Pixel counts describe requested damage, not driver
allocation sizes.

## Evidence

A native offline fixture at 3,110 × 1,960 pixels streamed a Markdown reply and
exercised composer input. Before moving clipboard subprocesses off the UI thread,
370 presented frames measured p50 0.86 ms, p95 2.279 ms, and maximum 7.461 ms.
Input-to-present submission measured p95 2.436 ms; a clipboard input outlier was
336.553 ms. That outlier is why clipboard work moved off the UI thread.

The in-progress application benchmark uses 3,300 real message rows, including
Markdown and code, and asserts that all rows reached the shared layout. At
1,414 × 891 points and scale 2.2, the optimized run measured:

| Interaction | Input, projection, layout, and paint p50 | p95 |
| --- | ---: | ---: |
| Sidebar hover | 0.06 ms | 0.06 ms |
| Typing | 0.37 ms | 0.54 ms |
| Transcript scroll | 1.39 ms | 2.10 ms |

Cold projection and painting took 73–76 ms. These are fixture CPU measurements,
not a claim about Linux, GPU completion, or the later 500-chat sidebar matrix.
The application benchmark lands with the on-screen chat slice. The adapter's
regression tests check unchanged-surface paint and upload suppression, changed
surface equivalence to complete painting, fractional clipped shape coverage,
trailing-space caret movement and selection, final-newline placement, bounded
telemetry, and stale clipboard results.

Checks: pinned Rust 1.97.1; `cargo fmt -p rust-native-desktop`; 37 adapter tests;
`cargo clippy -p rust-native-desktop --all-targets -- -D warnings`; desktop tests
and the optimized application benchmark also passed in the chat worktree. No
owner host, persistent owner chat, installed service, or keychain was used.
