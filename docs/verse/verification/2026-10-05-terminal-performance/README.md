# Terminal overlay performance over Everglade, October 5, 2026

This receipt measures the in-world terminal overlay
([`docs/verse/in-world-terminal.md`](../../in-world-terminal.md)) while panes
print without pause over Everglade's town, before and after the fixes in the
same change. The JSON reports beside this page are the raw results.

## Method

`crates/verse/examples/terminal_stress.rs` opens a real Verse window,
offline, straight into Everglade's town (its level of detail, wildlife, and
particles), and opens the overlay with one typing pane and N busy panes. The
busy panes cycle through these workloads:

1. `yes`
2. `cat` of a 200,000-line file, in a loop
3. a colored build log replay (SGR-heavy), in a loop
4. `seq 1 10000000`, in a loop
5. `top -s 1 -o cpu`, a full-screen curses program

While it records, the run raises a Wind Wall every 1.5 seconds and casts
Meteor Swarm on the town every 6 seconds. Every 150 ms it types a key into
the typing pane (`cat` with canonical mode off) through the overlay's real
key path. It records every frame for 15 seconds after a 4-second warm-up.

- **Frame interval** is the time between the ends of consecutive frames, as
  the player sees it. The display runs at 120 Hz with vsync, so one frame is
  8.3 ms and one missed vsync is 16.7 ms.
- **Update** is the overlay applying PTY output to its grids (parsing).
- **Draw** is building the overlay's vertices.
- **World** is the rest of the frame on the main thread, including the wait
  for the next vsync inside the renderer.
- **Key to glyph** is from the key reaching the overlay to the end of the
  first frame that drew the pane's changed grid.
- **MB/s** is output the overlay parsed, across panes.

Machine: Apple M5 Max, 18 cores, 128 GB, macOS 26.4, built-in Retina
display at 120 Hz. Release build. Runs used:

```sh
cargo build --release -p verse --example terminal_stress
target/release/examples/terminal_stress --busy N --seconds 15 --warmup 4 --out busy-N.json
```

The same numbers show live in the overlay: the prefix, then `?`, shows a
stats line and logs one JSON summary a second to standard error.

## Results

Before is commit `bb1d0d59ed` with only the instruments added; after is this
change. Times are milliseconds.

| Busy panes | Build | Frame median | p95 | p99 | Worst | Update p95 | Draw p95 | MB/s | Key to glyph median | p95 | Worst |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | before | 8.4 | 16.7 | 25.0 | 33.2 | 0.0 | 0.06 | 0.0 | 8.3 | 24.8 | 25.0 |
| 0 | after | 8.4 | 16.6 | 16.9 | 25.4 | 0.0 | 0.05 | 0.0 | 8.4 | 23.6 | 33.5 |
| 1 | before | never finished: the first frame never ended | | | | | | | | | |
| 1 | after | 8.4 | 16.7 | 17.1 | 24.9 | 3.0 | 0.18 | 12.5 | 16.4 | 31.8 | 34.4 |
| 4 | before | 14.6 | 25.1 | 33.1 | 78.7 | 9.2 | 0.34 | 6.4 | 24.4 | 42.0 | 66.6 |
| 4 | after | 8.4 | 16.5 | 17.0 | 25.6 | 3.1 | 0.30 | 13.0 | 16.2 | 25.2 | 41.8 |
| 8 | before | 15.4 | 26.5 | 216.5 | 653.1 | 15.4 | 0.52 | 18.3 | 24.9 | 380.3 | 719.6 |
| 8 | after | 8.4 | 16.5 | 17.0 | 25.6 | 3.1 | 0.42 | 21.3 | 9.0 | 24.7 | 33.2 |
| 8 | after (a) | 8.4 | 16.8 | 17.2 | 25.1 | 3.1 | 0.37 | 20.6 | 16.2 | 24.9 | 33.2 |
| 8 | after (b) | 8.4 | 16.6 | 16.9 | 25.0 | 3.1 | 0.39 | 22.6 | 8.5 | 17.2 | 33.6 |
| 16 | before | 1,777.8 | 1,797.8 | 1,797.8 | 1,797.8 | 1,792.9 | 0.34 | 56.1 | 1,777.8 | 1,797.8 | 1,797.8 |
| 16 | after | 8.4 | 16.7 | 17.1 | 259.0 | 3.2 | 0.35 | 21.9 | 8.3 | 17.0 | 50.7 |
| 16 | after (a) | 8.4 | 16.7 | 17.0 | 25.3 | 3.2 | 0.34 | 22.4 | 8.4 | 16.8 | 17.0 |
| 16 | after (b) | 8.3 | 16.6 | 16.9 | 17.2 | 3.2 | 0.35 | 23.9 | 8.3 | 8.8 | 16.9 |

Before, 16 busy panes recorded 9 frames in 15 seconds, and one busy pane
never finished its first recorded frame, so it has no report. The first
16-pane run after the fix (`after/busy-16-stall.json`) had one 52-second
frame that four later runs did not repeat; its cause is not established,
and the reports now keep the five longest frames' breakdowns to attribute
the next one.

## What was slow, and what changed

- **Unbounded output per frame (the cause).** Each frame drained every
  pane's queue completely. A pane printing faster than the emulator parses
  never empties its queue, so the frame never ended. Each frame now applies
  output for at most 3 ms (`UPDATE_BUDGET`) and 256 KiB a pane
  (`PANE_BYTES`), focused pane first and the rest in a turn that moves each
  frame. Output past the budget waits in the host's replay ring; a pane that
  falls further behind than the ring holds shows `[output skipped]`, which
  only a program printing without pause causes. Update now holds at 3.0 to
  3.2 ms at the 95th percentile at every load.
- **Allocation while scrolling.** `coder-vt` allocated a new row for every
  line that scrolled and dropped the oldest scrollback row. It now rotates
  rows in place and gives the dropped row's allocation to the new line.
- **Glyph lookup.** The atlas scanned its glyph list for every character.
  It now looks glyphs up in a map, which every HUD string also uses.
- **Not a bottleneck.** Building vertices stayed under 0.6 ms at every
  load, before and after, because unchanged panes already reuse their
  vertices; rebuilding only changed rows would save less than 0.4 ms, so it
  was not done. Backgrounds are now drawn as one rectangle per run of cells,
  which keeps the vertex count down for full-color programs.

## Against the targets

- **No frame over 16.7 ms at the 95th percentile with 8 busy panes over the
  town:** met, at 16.5 to 16.8 ms across three runs. The frames over 16.7 ms
  are single missed vsyncs (25 ms at worst) whose time is the world's: in the
  worst frames the overlay took 3.4 ms and the world, including the vsync
  wait, 21 ms. With no busy panes the world alone misses the same way
  (48 to 114 frames in 15 seconds).
- **Typing latency under one frame plus the program's own time:** the
  median is one or two 120 Hz frames (8.3 to 16.4 ms) and the 95th
  percentile at most 32 ms. Keys are typed right after a frame ends, the
  worst case; when the PTY round trip finishes before the next frame
  starts, the echo shows in that frame, and under load it often shows in
  the one after.

## Captures

`cargo run --release -p verse --example terminal_capture -- OUT.png 2 SCENE`
renders the overlay offline; `panes`, `unicode`, and `select` are its scenes
(the hotbar card beside the open overlay; CJK, emoji, combining marks,
braille, powerline, renditions, true color, and `top`; a drag selection, a
search match, and the stats line).
