# 83. Inspect and profile a running realm

> **Status: partly implemented.** Every realm in `realms.json` is measured
> the same way. [coax/web/realm-perf.coax](../../coax/web/realm-perf.coax)
> times each one in headless Chrome at three sizes and judges every case by
> one rule: [aethyr/frame-report.coax](../../aethyr/frame-report.coax), the
> 60 FPS rule and the frame statistics as one checked module, together with
> the record of one frame's stages. Goldens compare each page's frame, read
> back from the GPU, in 8 px blocks
> ([golden-frames.coax](../../coax/web/golden-frames.coax) on Linux,
> `goldens.ts` on the Mac). Probes read a frame back at a chosen moment.
> `state-defaults` checks that every engine starts from its declared state
> in the browser, the GPU ledger counts every buffer and texture a host
> makes, and the compiler's `--timings` measures its own phases. The example
> below runs natively in debug and release. Not built yet: stage timers in
> the page's frame, the HUD overlay that draws them, and the overlay's cost.
> See [Still to do](#still-to-do).

When a realm drops a frame or draws the wrong thing, you need to know where
the time went and what state made the frame. This chapter gives every realm
the same frame rule, the same release checks and the same probes.
[Chapter 37](ch037-measure-the-engine-honestly.md) measures the physics
engine. This chapter measures a whole realm as a player sees it.

## What this chapter teaches

- How to read a run of frames: median, 95th percentile, the longest frame
  and late frames, and why the rule takes the 95th percentile and the
  longest frame, not the average.
- How to tell a slow page from a busy machine.
- How goldens and probes catch a page that starts but draws nothing, or
  draws the wrong thing.
- How a frame's own time splits into stages, and why a report must account
  for nearly all of it.

## The 60 FPS rule

A run is judged when it has more than 30 frames. It holds 60 FPS when its
95th percentile frame is at most 17.5 ms and no frame is over 33 ms. A
frame over 20 ms is counted late. Percentile p is the frame at index
n · p / 100 of the run sorted. `frame_at` finds that frame without sorting:
it is the frame with at most k frames shorter than it and more than k at
most as long. Counting both ways leaves no tie for a sort's comparison to
round:

<!-- source: aethyr/frame-report.coax rule -->
```jai
FRAME_P95_MAX :: 17500;    // microseconds: the 95th percentile frame at most this
FRAME_MAX :: 33000;        // and no frame longer than this
FRAME_LATE :: 20000;       // a frame over this is counted late
FRAME_LEAST :: 31;         // frames a run needs before it is judged
FrameStats :: struct { n: int; total: int; p50: int; p95: int; max: int; over: int; }

// The frame at index k of the first n frames sorted, without sorting them: the frame with fewer
// than k + 1 frames shorter than it and more than k at most as long. -1 when k is not an index.
frame_at :: (us: [] int, n: int, k: int) -> int {
  for i: 0..<n {
    shorter := 0; within := 0;
    for j: 0..<n {
      if us[j] < us[i] { shorter += 1; }
      if us[j] <= us[i] { within += 1; }
    }
    if shorter <= k && k < within { return us[i]; }
  }
  return -1;
}
// The first n frame times of us. A run of none, or more than us holds, is all zero (n 0).
// Percentile p is the frame at index n * p / 100 of the run sorted.
frame_stats :: (us: [] int, n: int) -> FrameStats {
  f: FrameStats;
  if n < 1 || n > us.count { return f; }
  for i: 0..<n {
    f.total += us[i];
    if us[i] > FRAME_LATE { f.over += 1; }
  }
  f.n = n;
  f.p50 = frame_at(us, n, n * 50 / 100); f.p95 = frame_at(us, n, n * 95 / 100); f.max = frame_at(us, n, n - 1);
  return f;
}
// The 60 FPS rule.
frame_holds :: (f: FrameStats) -> bool { return f.n >= FRAME_LEAST && f.p95 <= FRAME_P95_MAX && f.max <= FRAME_MAX; }
// Frames a second, times ten (600 is 60.0 FPS), rounded.
frame_fps10 :: (f: FrameStats) -> int { return ifx f.total > 0 then (f.n * 100000000 / f.total + 5) / 10 else 0; }
```

The average hides what a player feels. A run of 252 frames at 16.7 ms with
one 40 ms hitch averages 59.5 FPS, close to 60, but the hitch is a visible
stall, and the rule fails it on the longest frame. A machine busy with
other work every tenth frame averages 57 FPS. The rule fails it on the 95th
percentile, because 25 of 252 frames are 25 ms and index 239 of the sorted
run is one of them.

## Every realm, three ways

`coax/web/tool.sh realm-perf [BASE_URL]` reads `apps/openagents-web/realms.json`
and makes three cases for every open realm, listed or not:

- 1440x900 at a device pixel ratio of 1;
- 1440x900 at a device pixel ratio of 2;
- a 390x844 phone at a ratio of 3, with touch.

A realm whose clock turns is pinned to noon (`?clock=0.5`) so that runs can
be compared. Each case opens `/realms/SLUG` and waits up to 30 s for the
scene to draw, then 2 s for it to settle. It walks forward, turns and walks
back, 4.2 s a round, and times every `requestAnimationFrame` in whole
microseconds. The tool refuses to measure when headless Chrome has only
SwiftShader, because software frame times would mean nothing. It prints the
adapter it measured on.

On 2026-11-03 against production, on this machine's RTX 4080:

```text
realm-perf: WebGPU adapter: hardware nvidia lovelace
ok   1440x900 dpr 1 the-grid: 60.0 FPS, p50 16.7 ms, p95 16.8 ms, max 16.8 ms, 0/252 over 20 ms
ok   1440x900 dpr 2 the-grid: 60.0 FPS, p50 16.7 ms, p95 16.7 ms, max 16.8 ms, 0/253 over 20 ms
ok   390x844 phone  the-grid: 60.0 FPS, p50 16.7 ms, p95 16.8 ms, max 16.8 ms, 0/252 over 20 ms
realm-perf: 0 of 3 cases missed 60 FPS
```

Chrome reports frame times to 0.1 ms, so the frames are 16.7 or 16.8 ms,
and a page that holds the display's rate shows a p95 of 16.8 ms at most.

### A slow page or a busy machine

A case that misses is measured once more and the second measurement is
kept. `--against URL` measures each case there too, right after: run
staging against production, and when both miss, the GPU was likely busy,
not the page. `--previous FILE` compares each case with an earlier report.
A case is WORSE when it held 60 FPS then and doesn't now, and slower or
faster when its p95 moved more than 1.5 ms. `--shard I/N` splits the sweep
(17 realms, 51 cases, about 15 minutes) under the 15-minute limit for one
command. `release.sh` runs it after every release against production, in
three shards, compared with the last release's
`releases/<time>/REALMS.tsv`. It is a report: it never holds back a
release.

[coax/web/perf.ts](../../coax/web/perf.ts) keeps Everglade's four scenes
(road at noon, commons at dusk, river at night, market at dusk) and the
homepage. A failing case runs beside a control page, production's Everglade
standing still. When the control misses frames too, the check waits 10 s
and tries again, at most 6 times.

## GPU time per pass

`bun coax/web/frametime.ts URL [WxH] [DPR] [SECONDS]` puts a timestamp query
on every render pass and runs frames unthrottled (`--vsync` keeps the
display's rate). `--skip PASSES` drops the numbered passes' draws to cost
them, and `--phone` emulates a touch phone. It is the tool for "which pass
is slow". `realm-perf` is the tool for "does this realm hold 60 FPS".

## Goldens

A golden is a page's frame as it should look. `golden-frames` (Linux) opens
each page of `golden-pages.ts`'s table in golden mode at its size. It waits
for the scene, then reads the WebGPU frame back on the GPU. The page must
light at least its share of pixels (a channel over 40). A dark frame gets
four more tries, 750 ms apart. The frame then differs from its golden when
more than the page's allowance of 8 px blocks changed. A block changed when
a channel's mean over the block moved more than 8, compared exactly in
integers:

<!-- source: coax/web/golden-frames.coax differ -->
```jai
// Which 8 px blocks differ between two RGBA frames (1 a block): a block differs when the mean of
// a channel over its pixels moved more than GF_SHADE. Frames of different sizes differ everywhere.
// The count of blocks (a's), each set in out.
gf_differ :: (a: [] u8, aw: int, ah: int, b: [] u8, bw: int, bh: int, out: [] u8) -> int {
  cols := (aw + GF_BLOCK - 1) / GF_BLOCK; rows := (ah + GF_BLOCK - 1) / GF_BLOCK;
  for k: 0..<cols * rows { out[k] = 0; }
  if aw != bw || ah != bh { for k: 0..<cols * rows { out[k] = 1; } return cols * rows; }
  for by: 0..<rows {
    for bx: 0..<cols {
      r := 0; g := 0; bl := 0; n := 0;
      y1 := ifx by * GF_BLOCK + GF_BLOCK < ah then by * GF_BLOCK + GF_BLOCK else ah;
      x1 := ifx bx * GF_BLOCK + GF_BLOCK < aw then bx * GF_BLOCK + GF_BLOCK else aw;
      for y: by * GF_BLOCK..<y1 {
        for x: bx * GF_BLOCK..<x1 {
          i := (y * aw + x) * 4;
          r += cast(int) a[i] - cast(int) b[i]; g += cast(int) a[i + 1] - cast(int) b[i + 1]; bl += cast(int) a[i + 2] - cast(int) b[i + 2];
          n += 1;
        }
      }
      // |sum / n| > SHADE, exactly, in integers
      lim := GF_SHADE * n;
      if r > lim || -r > lim || g > lim || -g > lim || bl > lim || -bl > lim { out[by * cols + bx] = 1; }
    }
  }
  return cols * rows;
}
```

Three more frames, 500 ms apart, find the blocks that move on their own.
Those blocks and their eight neighbours are left out. `--require-hardware`
stops unless the page gets a GPU's adapter. On a Mac, `goldens.ts`
compares screenshots by the same rule. `release.sh` runs the goldens on
staging before promoting a release.

## Probes and state

- **Probes.** `?probe` reads the frame back (`host.js`, development only).
  `?probe&at=SECONDS` runs the clock that far in 0.1 s steps before the
  probed frame. `?probe&do=STEPS` runs input before it.
  `coaxial.renderStatus` says what was submitted, the renderer's readiness
  and its last refusal.
- **A state that starts from its defaults.** `coax/web/tool.sh
  state-defaults` runs every engine's `coaxial_state_init` on zeroed memory
  in headless Chrome. It compares the state byte for byte with the native
  defaults of its type, and every face global too
  ([chapter 3](ch003-state-starts-from-its-defaults.md)). On Linux, after
  `apps/openagents-web/build.sh`, it passes 27 states and 34 face globals.
- **The GPU ledger.** A host asks
  [aethyr/render/registry.coax](../render/registry.coax) before it makes a
  buffer or a texture, and tells it when it destroys one. Each object has a
  slot and a generation, within 512 slots and 1 GiB. A handle to a released
  object, or to one from a lost device, is refused. The browser Grid and the
  native Vulkan host own their objects through it.
- **The compiler's own time.** `coax --timings FILE` writes each phase's
  wall time, each check and each library check
  ([coax/DEBUGGING.md](../../coax/DEBUGGING.md); Coax
  [chapter 12](../../coax/book/ch12-debugging-and-cost.md)).

## Analytics

Every page sends the common events in `apps/openagents-web/analytics.coax`:
the session's start, the engine loaded, the world entered, alive every
30 s, and the world left. Realms add their own through helpers such as
`track_walk` and `track_tally`. No words and no personal data are recorded.
The ingest keeps at most 16 short properties an event, 50 events a batch
and 5,000 a session
([analytics-events.md](../../docs/verse/analytics-events.md)).

## One frame's stages

The statistics say a frame was late. The frame's report says where its own
time went: input, the fixed steps, building the draw, and handing it to the
GPU. A report holds when every stage is a time, the steps are at most the
8 a frame may run, and the stages leave at most 0.5 ms of the frame
unaccounted:

<!-- source: aethyr/frame-report.coax report -->
```jai
// One frame's own time (from the frame's start to its submit), in microseconds, by stage.
FRAME_STAGES :: 4;
STAGE_INPUT :: 0;    // the devices read into this frame's actions (chapter 4)
STAGE_STEPS :: 1;    // the fixed steps the frame's time made due (chapter 1)
STAGE_DRAW :: 2;     // the draw built from the state between the last two steps
STAGE_SUBMIT :: 3;   // the draw handed to the GPU
FRAME_SLACK :: 500;  // microseconds of a frame the stages may leave unaccounted
FRAME_STEPS_MOST :: 8;   // no frame runs more fixed steps than this (INVARIANTS.md)
FrameReport :: struct { frame_us: int; steps: int; stage_us: [FRAME_STAGES] int; }

// The time no stage accounts for.
frame_unaccounted :: (r: FrameReport) -> int {
  left := r.frame_us;
  for s: r.stage_us { left -= s; }
  return left;
}
// A report holds when its numbers are times and step counts a frame can have, and its stages
// account for all of the frame but at most FRAME_SLACK.
frame_report_ok :: (r: FrameReport) -> bool {
  if r.frame_us < 0 || r.steps < 0 || r.steps > FRAME_STEPS_MOST { return false; }
  for s: r.stage_us { if s < 0 { return false; } }
  left := frame_unaccounted(r);
  return left >= 0 && left <= FRAME_SLACK;
}
```

## The example

[aethyr/examples/profile-a-frame.coax](../examples/profile-a-frame.coax)
judges three runs of 252 frames, runs a hitch and a 3 s stall through
[chapter 1](ch001-a-frame-has-a-fixed-step.md)'s clock, checks two frame
reports, and walks the Grid with a report kept for every frame, beside the
same walk without:

<!-- source: aethyr/examples/profile-a-frame.coax example -->
```jai
#load "../frame-report.coax";
#load "../../verse/gridwalk.coax";
#import "Basic";

FRAMES :: 252;   // what a 4.2 s walk gives at 60 FPS (realm-perf's one round)

// Three runs of frame times, in microseconds, at the 0.1 ms Chrome reports them in: steady (every
// fifth frame 16.8 ms, the rest 16.7), the same with one 40 ms hitch at frame 120, and a busy
// machine (every tenth frame 25 ms).
run :: (kind: int, us: [] int) {
  for k: 0..<FRAMES {
    us[k] = ifx k % 5 == 4 then 16800 else 16700;
    if kind == 1 && k == 120 { us[k] = 40000; }
    if kind == 2 && k % 10 == 9 { us[k] = 25000; }
  }
}
show :: (name: string, us: [] int) {
  f := frame_stats(us, FRAMES);
  print("%: % frames, %.% FPS, p50 % us, p95 % us, max % us, % over 20 ms: %\n", name, f.n, frame_fps10(f) / 10, frame_fps10(f) % 10,
        f.p50, f.p95, f.max, f.over, ifx frame_holds(f) then "holds 60 FPS" else "misses 60 FPS");
}
// The fixed steps each frame of a run makes due on a 1/64 s clock.
steps :: (name: string, us: [] int) {
  c := loop_clock(LOOP_STEP_NS);
  most := 0;
  for k: 0..<FRAMES {
    c = loop_ran(loop_accept_ns(c, us[k] * 1000));
    if c.ran > most { most = c.ran; }
  }
  print("%: % steps, at most % in a frame\n", name, c.ticks, most);
}

main :: () {
  us: [FRAMES] int;
  names := [3] string.["steady", "a hitch", "busy"];
  for kind: 0..<3 { run(kind, us[..]); show(names[kind], us[..]); }
  run(1, us[..]);
  steps("a hitch", us[..]);
  us[120] = 3000000;   // a 3 s stall instead: counted as 0.25 s, at most 8 steps
  steps("a 3 s stall", us[..]);

  // one frame's report: input, steps, the draw built, submitted
  r := FrameReport.{frame_us = 4200, steps = 1, stage_us = [4] int.[150, 1900, 1400, 450]};
  print("a frame of % us: % us unaccounted, holds: %\n", r.frame_us, frame_unaccounted(r), frame_report_ok(r));
  r.stage_us[STAGE_DRAW] = 1000;
  print("a frame of % us: % us unaccounted, holds: %\n", r.frame_us, frame_unaccounted(r), frame_report_ok(r));

  // reports kept for every frame of a Grid session change nothing it records
  run(1, us[..]);
  with: [1] GridWalk; with[0] = GridWalk.{};
  without: [1] GridWalk; without[0] = GridWalk.{};
  gridwalk_place(with[..], 28, 28, -2.35619449, 1.35, 9);
  gridwalk_place(without[..], 28, 28, -2.35619449, 1.35, 9);
  reports: [FRAMES] FrameReport; held := 0;
  for k: 0..<FRAMES {
    dt := cast(f32) us[k] / 1000000.0;
    before := gridwalk_steps(with[..]);
    gridwalk_frame(with[..], 0, 1, k < 90, k == 120 || k == 190, 0, 0, 0, 1000, 2000, 2, dt, false);
    reports[k] = FrameReport.{steps = gridwalk_steps(with[..]) - before};
    if frame_report_ok(reports[k]) { held += 1; }
    gridwalk_frame(without[..], 0, 1, k < 90, k == 120 || k == 190, 0, 0, 0, 1000, 2000, 2, dt, false);
  }
  print("a Grid walk of % frames: % reports hold, steps %, session digests equal: %\n", FRAMES, held,
        gridwalk_steps(with[..]), gridwalk_session(with[..]) == gridwalk_session(without[..]));
}
```

```text
steady: 252 frames, 59.8 FPS, p50 16700 us, p95 16800 us, max 16800 us, 0 over 20 ms: holds 60 FPS
a hitch: 252 frames, 59.5 FPS, p50 16700 us, p95 16800 us, max 40000 us, 1 over 20 ms: misses 60 FPS
busy: 252 frames, 57.0 FPS, p50 16700 us, p95 25000 us, max 25000 us, 25 over 20 ms: misses 60 FPS
a hitch: 271 steps, at most 2 in a frame
a 3 s stall: 276 steps, at most 8 in a frame
a frame of 4200 us: 300 us unaccounted, holds: true
a frame of 4200 us: 700 us unaccounted, holds: false
a Grid walk of 252 frames: 252 reports hold, steps 271, session digests equal: true
```

The numbers, worked by hand:

- **Steady.** 202 frames of 16,700 µs and 50 of 16,800 make 4,213,400 µs.
  252 · 10⁸ / 4,213,400 = 5,980 (rounded down), plus 5, divided by 10, is
  598: 59.8 FPS. Sorted, index 126 (the median) is 16,700 and index 239
  (the 95th percentile) is 16,800.
- **A hitch.** One frame of 16,700 becomes 40,000: 4,236,700 µs, 59.5 FPS,
  and the longest frame is over 33 ms. On 1/64 s steps, 4.2367 s is 271.15
  steps, so 271 run, and no frame runs more than 2.
- **A 3 s stall.** The clock counts the stall as 0.25 s, 16 steps owed. It
  runs 8 and drops the other 125 ms: (4,213,400 − 16,700 + 250,000 −
  125,000) µs · 64 / 10⁶ = 276.6, so 276 steps run.
- **Busy.** The 25 frames at k = 9, 19, …, 249 were 16,800 each and are now
  25,000: 4,418,400 µs, 57.0 FPS. Indices 227 to 251 of the sorted run are
  25,000, so the 95th percentile, index 239, is 25,000.
- **The reports.** 150 + 1,900 + 1,400 + 450 = 3,900 of 4,200 µs leaves
  300 µs unaccounted, which holds. With the draw at 1,000 µs, 700 µs are
  unaccounted, which is more than 500, so the report doesn't hold.

The Grid walk keeps a report for every frame, read from the session's own
step count, and its session digest
([chapter 5](ch005-replay-a-session-exactly.md)) equals the walk without
reports. A report reads the frame and changes nothing that the record
holds.

## Checks

- `coax/bin/coax test aethyr/frame-report.coax --library`: 4 of 4. The
  percentiles, late frames and the rule; the rule's edges (17.5 ms at the
  95th percentile and 33 ms at most hold, a microsecond more doesn't; a late
  frame is over 20 ms, not at it; rounding of the frame rate); a run of 100
  frames with no two alike, shuffled; and a frame's report.
- `coax/bin/coax mutate aethyr/frame-report.coax --library`: 86 mutants,
  86 killed.
- `coax/bin/coax test coax/web/realm-perf.coax --library`: 153 of 153,
  with the rule's checks now on `frame_stats`. A run longer than its
  buffer is refused, not cut short.
- `coax/web/tool.sh realm-perf https://openagents.com --only the-grid
  --rounds 1`: 3 of 3 cases hold 60 FPS, measured with the rule from
  frame-report.coax (shown above).

## Still to do

- **Stage timers in the page.** `host.js` times the frame from its start
  to its submit and fills a `FrameReport` each frame from chapter 1's loop:
  input, steps, draw and submit, readable by tests.
- **The overlay.** A HUD overlay ([chapter 15](ch015-ui-and-the-hud.md)),
  off by default, that draws the report and the last few hundred frames'
  statistics. Its cost, measured with and without it, must be under 0.2 ms
  a frame.
- **Report on, digest unchanged, in the browser.** Shown natively for the
  Grid here. The same check should run on the page with the overlay drawn.
- **Budgets per stage.** Chapter 16's budgets, refusing a frame plan whose
  stages can't fit, are separate from this measurement.
- **The analytics contract** (`goldens.ts`) and the staging goldens were
  not rerun for this chapter. They are the release's checks.

## What exists today

- [aethyr/frame-report.coax](../../aethyr/frame-report.coax): the 60 FPS
  rule, frame statistics and a frame's report.
- [coax/web/realm-perf.coax](../../coax/web/realm-perf.coax): every realm's
  60 FPS cases, from `realms.json`.
- [coax/web/perf.ts](../../coax/web/perf.ts): Everglade's scenes with a
  control page.
- [coax/web/frametime.ts](../../coax/web/frametime.ts): GPU time per pass.
- [coax/web/golden-frames.coax](../../coax/web/golden-frames.coax) and
  [coax/web/goldens.ts](../../coax/web/goldens.ts): goldens on Linux and
  the Mac, and the analytics contract.
- [coax/web/host.js](../../coax/web/host.js): `?probe`, `at` and `do`.
- [coax/web/state-defaults.coax](../../coax/web/state-defaults.coax): every
  engine's state from its defaults in the browser.
- [aethyr/render/registry.coax](../render/registry.coax): the GPU ledger.
- [docs/verse/analytics-events.md](../../docs/verse/analytics-events.md):
  what each realm records.
- [coax/DEBUGGING.md](../../coax/DEBUGGING.md): native debugging and the
  compiler's timings.

## Acceptance checks and oracles

The chapter is done when:

- Every listed realm has at least one 60 FPS case at dpr 1 and 2, and a
  phone case at 390x844. Holds: `realm-perf` makes all three for every open
  realm.
- The frame report's stage times sum to within 0.5 ms of the measured frame
  time. The rule holds (`frame_report_ok`), but no page fills a report yet.
- The overlay costs under 0.2 ms a frame, measured with and without it. Not
  built.
- Turning the report on does not change the session digest of
  [chapter 5](ch005-replay-a-session-exactly.md). Holds natively for the
  Grid. Not yet run in the page.
- `goldens.ts` and the analytics contract still pass on staging. They are
  run by each release, and were not rerun here.

## Dependencies

Chapters [1](ch001-a-frame-has-a-fixed-step.md) (the loop's steps),
[3](ch003-state-starts-from-its-defaults.md) (state from its defaults),
[5](ch005-replay-a-session-exactly.md) (replay for a reported frame),
[15](ch015-ui-and-the-hud.md) (the overlay) and
[16](ch016-budgets-that-refuse.md) (budgets). Coax:
[chapter 12](../../coax/book/ch12-debugging-and-cost.md) for debugging and
measuring cost.
