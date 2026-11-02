# 16. Budgets that refuse

> **Status: partly implemented.** Engine-core task: `ec-budget` (#12609).
> The module is [aethyr/budget.coax](../budget.coax). Every realm declares a
> frame budget beside its state and has a check that draws its frames at
> three screen sizes and fails when any frame passes the budget.
> Content-hashed asset identity is [aethyr/assets.coax](../assets.coax);
> realm packs don't load through it yet.

A realm that draws more than its buffers hold has two bad options. It can
clip quietly, so a lamp or a creature vanishes and nobody knows, or it can
overrun a buffer. Aethyr takes a third: each realm declares what one frame
may use, a meter counts what each frame takes, and a frame that passes a
limit fails the realm's tests.

The first run of these checks found a realm that clipped. Hold the Stair's
swarm stage draws 39,318 vertices a frame at 1440 by 900 CSS pixels. Its
vertex buffer held 36,000, so its quad writer stopped at the full buffer and
dropped 3,318 vertices, about 550 quads, every swarm frame. The buffer is now
52,000 vertices and the budget 48,000.

## What this chapter teaches

- A frame budget as declared data: steps, vertices, draw records and bytes.
- A declaration that is valid only when a full buffer is over budget.
- A meter that takes each request whole or refuses all of it, and keeps the
  first refusal for the report.
- A budget check in every realm, with the numbers measured from each realm's
  frames.

## The declaration

<!-- source: aethyr/budget.coax declare -->
```jai
BUDGET_STEPS :: 0;      // fixed simulation steps one frame runs
BUDGET_VERTICES :: 1;   // vertices one frame writes
BUDGET_DRAWS :: 2;      // draw records one frame submits
BUDGET_BYTES :: 3;      // bytes the host holds for the realm: its state and its buffers
BUDGET_KINDS :: 4;
BUDGET_NONE :: -1;

// What a realm declares one frame may use.
FrameBudget :: struct { steps: int; vertices: int; draws: int; bytes: int; }

// The limit of one kind, or -1 for a kind that isn't one.
budget_limit :: (b: FrameBudget, kind: int) -> int {
  if kind == BUDGET_STEPS { return b.steps; }
  if kind == BUDGET_VERTICES { return b.vertices; }
  if kind == BUDGET_DRAWS { return b.draws; }
  if kind == BUDGET_BYTES { return b.bytes; }
  return -1;
}
```

`FrameBudget` holds four limits:

- `steps`: the fixed simulation steps one frame runs. Every realm declares
  8, the cap of [chapter 1](ch001-a-frame-has-a-fixed-step.md)'s clock.
- `vertices`: the vertices one frame writes.
- `draws`: the draw records one frame submits to the scene renderer.
- `bytes`: what the page host holds for the realm. `budget_arena_bytes`
  counts it the way [coax/web/host.js](../../coax/web/host.js) sizes its
  arena: the state, then four bytes for each float of the vertex buffer and
  the uniforms.

A realm's quad writers stop when its vertex buffer is full. A clipped frame
therefore writes exactly as many vertices as the buffer holds.
`budget_declared` refuses a declaration whose vertex limit is not below the
buffer, so a clipped frame always passes the limit and fails the check. It
also refuses a limit below zero.

## The meter

<!-- source: aethyr/budget.coax meter -->
```jai
// The first take a meter refused: in which frame, of what kind (BUDGET_NONE for one that isn't
// a kind), how much was asked, and how much of the limit that frame had used.
BudgetRefusal :: struct { kind: int = BUDGET_NONE; frame: int; asked: int; used: int; limit: int; }

BudgetMeter :: struct {
  limit: FrameBudget;
  used: [BUDGET_KINDS] int;         // this frame
  peak: [BUDGET_KINDS] int;         // the most any frame has used
  peak_frame: [BUDGET_KINDS] int;   // the first frame that used that much
  frames: int;                      // frames begun
  over_frames: int;                 // frames that refused something
  refused: int;                     // takes refused, in every frame
  over: bool;                       // this frame refused something
  first: BudgetRefusal;             // the first take refused
}

budget_meter :: (limit: FrameBudget) -> BudgetMeter { return BudgetMeter.{limit = limit}; }

// A new frame: nothing used yet. The peaks and the counts carry on.
budget_frame :: (m: [] BudgetMeter) {
  for bk: 0..<BUDGET_KINDS { m[0].used[bk] = 0; }
  m[0].over = false;
  m[0].frames += 1;
}

// Takes n of a kind for this frame, whole, or refuses all of it: when it would pass the
// limit, when n is below zero, or when the kind isn't one. A refusal is counted, marks the
// frame over budget, and the first one is kept for the report.
budget_take :: (m: [] BudgetMeter, kind: int, n: int) -> bool {
  bk := budget_kind_of(kind);
  b_used := 0;
  b_limit := -1;
  if bk != BUDGET_NONE { b_used = m[0].used[bk]; b_limit = budget_limit(m[0].limit, bk); }
  if bk != BUDGET_NONE && n >= 0 && n <= b_limit - b_used {
    m[0].used[bk] = b_used + n;
    if m[0].used[bk] > m[0].peak[bk] { m[0].peak[bk] = m[0].used[bk]; m[0].peak_frame[bk] = m[0].frames; }
    return true;
  }
  m[0].refused += 1;
  if !m[0].over { m[0].over = true; m[0].over_frames += 1; }
  if m[0].refused == 1 { m[0].first = BudgetRefusal.{kind = bk, frame = m[0].frames, asked = n, used = b_used, limit = b_limit}; }
  return false;
}

// Whether every frame so far fitted: what a realm's budget check asserts.
budget_ok :: (m: BudgetMeter) -> bool { return m.refused == 0; }
```

`budget_take` takes `n` of a kind for this frame or refuses all of it. It
refuses when `used + n` would pass the limit, when `n` is below zero, and
when the kind is not one of the four. It compares `n` with `limit - used`,
so a sum near the largest `int` can't wrap. A refusal:

1. Adds one to `refused`.
2. Marks the frame `over`, and adds one to `over_frames` the first time in
   that frame.
3. Keeps the first refusal of the session in `first`: its frame, kind,
   request, what the frame had used and the limit.

`peak` keeps the most any frame used, and `peak_frame` the first frame that
used that much. `budget_ok` is what a realm's check asserts: nothing was
refused.

## Measuring a frame

<!-- source: aethyr/budget.coax measure -->
```jai
// The screens a realm's budget check draws at: a laptop at 1440 by 900 CSS pixels, the same at
// twice the density, and a phone at 390 by 844 at three times.
BudgetScreen :: struct { width: f32; height: f32; dpr: f32; }
BUDGET_SCREENS :: 3;
budget_screen :: (i: int) -> BudgetScreen {
  if i == 1 { return BudgetScreen.{width = 2880, height = 1800, dpr = 2}; }
  if i == 2 { return BudgetScreen.{width = 1170, height = 2532, dpr = 3}; }
  return BudgetScreen.{width = 1440, height = 900, dpr = 1};
}

// One measured frame: a new frame that takes the vertices it wrote, the draw records it
// submitted and the bytes its host holds. The frame is over budget (m[0].over) when any of
// them doesn't fit.
budget_measure :: (m: [] BudgetMeter, vertices: int, draws: int, bytes: int) {
  budget_frame(m);
  if !budget_take(m, BUDGET_VERTICES, vertices) { m[0].used[BUDGET_VERTICES] = vertices; }
  if !budget_take(m, BUDGET_DRAWS, draws) { m[0].used[BUDGET_DRAWS] = draws; }
  if !budget_take(m, BUDGET_BYTES, bytes) { m[0].used[BUDGET_BYTES] = bytes; }
}
```

`budget_measure` starts a frame and takes its vertices, draw records and
bytes. When one doesn't fit, it records what the frame used, so a report
shows the over-budget number rather than the last one that fitted.

Every realm's check draws its frames at the three `budget_screen` sizes:
1440 by 900 at one device pixel per CSS pixel, the same at two, and a phone
at 390 by 844 CSS pixels at three. The checks run at compile time, where a
Grid frame costs millions of evaluation steps, so each check draws three
frames a screen, the last one 0.1 s long. The Garden's:

<!-- source: apps/openagents-web/garden.coax budget -->
```jai
GARDEN_BUDGET :: FrameBudget.{steps = 8, vertices = 4000, draws = DRAWS, bytes = 917504};

// A walk from the spawn: three frames at each budget screen, the last one 0.1 s long. Book
// chapter 16 gives the peaks of 120 frames at each screen, measured natively.
#test "The Garden's frames fit its declared budget" {
  #assert budget_declared(GARDEN_BUDGET, MAX_VERTICES) == BUDGET_NONE;
  frames := 3;
  m := [1] BudgetMeter.[budget_meter(GARDEN_BUDGET)];
  out: [MAX_VERTICES * VERTEX_FLOATS] f32; u: [UNIFORM_FLOATS] f32;
  bytes := budget_arena_bytes(size_of(Garden), MAX_VERTICES, VERTEX_FLOATS, UNIFORM_FLOATS);
  for i: 0..<BUDGET_SCREENS {
    s: [1] Garden;
    sc := budget_screen(i);
    for f: 0..<frames {
      dt := ifx f < frames - 1 then 1.0f32 / 60.0 else 0.1f32;
      n := garden_frame(s[..], out[..], u[..], sc.width, sc.height, sc.dpr, dt, false, f32(SPAWN_X), 0, f32(SPAWN_Z) + f32(f) * 0.05, 0, 0, 0, 0, false, 0, 0, 0, 0);
      budget_measure(m[..], n, garden_draws(s[..]), bytes);
    }
  }
  #assert budget_ok(m[0]);
}
```

## A worked example

Take a realm whose vertex buffer holds 36 vertices, six quads, and which
declares 30. Work these out by hand:

1. **The declaration.** 30 is below 36, so `budget_declared` returns
   `BUDGET_NONE`. A declaration of 36 returns `BUDGET_VERTICES`: a full
   buffer would fit it.
2. **Two quads.** 12 vertices. 12 ≤ 30 − 0, so the take fits.
3. **Five quads.** 30 vertices. 30 ≤ 30, so it fits, and the peak is 30 in
   frame 2.
4. **Seven quads.** 42 vertices. The writer stops at 36, after six quads.
   36 > 30, so the take is refused. The frame is over budget, and the
   report reads 36 asked with 0 of 30 used.
5. **Five quads again.** 30 vertices fit. The check still fails: one frame
   of four was over.

The example [aethyr/examples/budget-a-frame.coax](../examples/budget-a-frame.coax)
runs the same frames:

<!-- source: aethyr/examples/budget-a-frame.coax example -->
```jai
#load "../budget.coax";
#import "Basic";

// A realm with a vertex buffer of 36 vertices (six quads) declares 30, and draws 2, 5, 7 and
// then 5 quads. Its writer stops at a full buffer, as the realms' quad writers do, so the
// third frame shows only 6 of its 7 quads. The budget refuses that frame.
BUFFER :: 36;
main :: () {
  declared := FrameBudget.{steps = 8, vertices = 30, draws = 0, bytes = 4096};
  print("declared % of a % vertex buffer: %\n", declared.vertices, BUFFER, budget_kind_name(budget_declared(declared, BUFFER)));
  same := declared; same.vertices = BUFFER;
  print("declared % of a % vertex buffer: %\n", same.vertices, BUFFER, budget_kind_name(budget_declared(same, BUFFER)));
  m := [1] BudgetMeter.[budget_meter(declared)];
  quads := [4] int.[2, 5, 7, 5];
  for f: 0..<4 {
    written := 0;
    for q: 0..<quads[f] { if written + 6 <= BUFFER { written += 6; } }   // the writer clips
    budget_measure(m[..], written, 0, 1000);
    print("frame %: % quads asked, % vertices written, over budget %\n", m[0].frames, quads[f], written, m[0].over);
  }
  r := m[0].first;
  print("refused: % % in frame % (% used of %)\n", r.asked, budget_kind_name(r.kind), r.frame, r.used, r.limit);
  print("peak % in frame %; frames over % of %; check passes %\n", m[0].peak[BUDGET_VERTICES], m[0].peak_frame[BUDGET_VERTICES], m[0].over_frames, m[0].frames, budget_ok(m[0]));
}
```

```shell
coax/bin/coax run aethyr/examples/budget-a-frame.coax
```

```text
declared 30 of a 36 vertex buffer: none
declared 36 of a 36 vertex buffer: vertices
frame 1: 2 quads asked, 12 vertices written, over budget false
frame 2: 5 quads asked, 30 vertices written, over budget false
frame 3: 7 quads asked, 36 vertices written, over budget true
frame 4: 5 quads asked, 30 vertices written, over budget false
refused: 36 vertices in frame 3 (0 used of 30)
peak 30 in frame 2; frames over 1 of 4; check passes false
```

The third frame is the broken case. Its writer clipped, the meter refused
it whole, and `budget_ok` is false, which fails a realm's check.

## Every realm's budget

Each realm's file declares its budget, named for the realm (for example,
`GARDEN_BUDGET`), and runs its check. The table gives the peaks of 120
frames at each budget screen (the last 20 of them 0.1 s long), measured
natively, and the declared limits. Each vertex limit is the peak plus a
quarter, rounded up to a thousand. Each byte limit is the arena plus a
tenth, rounded up to 64 KiB.

| Realm | Buffer | Peak vertices | Declared | Draws | Arena bytes | Declared bytes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| The Grid (`newland.coax`) | 400,000 | 49,002 | 62,000 | 0 | 19,868,128 | 21,889,024 |
| Belt 2200 (`belt.coax`) | 90,000 | 44,424 | 56,000 | 0 | 4,871,720 | 5,373,952 |
| Hold the Stair (`stair.coax`) | 52,000 | 39,318 | 48,000 | 0 | 3,567,632 | 3,932,160 |
| Fairy Forest (`fairy.coax`) | 24,000 | 8,724 | 11,000 | 6 | 2,470,568 | 2,752,512 |
| The Haunted House (`realm.coax`) | 36,000 | 5,784 | 8,000 | 0 | 2,343,528 | 2,621,440 |
| The fixture realm (`fixture.coax`) | 24,000 | 5,472 | 7,000 | 0 | 1,541,248 | 1,703,936 |
| Dead Gears (`deadgears.coax`) | 24,000 | 5,094 | 7,000 | 0 | 2,256,304 | 2,490,368 |
| The characters realm (`characters.coax`) | 24,000 | 4,368 | 6,000 | 0 | 1,680,992 | 1,900,544 |
| Everglade (`everglade.coax`) | 30,000 | 4,152 | 6,000 | 0 | 3,897,968 | 4,325,376 |
| The drop realm (`droprealm.coax`) | 24,000 | 3,438 | 5,000 | 0 | 1,543,960 | 1,703,936 |
| Orbit League (`orbitleague.coax`) | 24,000 | 3,294 | 5,000 | 0 | 1,840,064 | 2,031,616 |
| The Garden (`garden.coax`) | 12,000 | 2,838 | 4,000 | 5 | 827,936 | 917,504 |
| The realm builder (`builder.coax`) | 30,000 | 1,878 | 3,000 | 0 | 1,929,928 | 2,162,688 |
| Ice Cream Man (`icecream.coax`) | 16,000 | 1,812 | 3,000 | 0 | 1,060,928 | 1,179,648 |
| A made realm (`myrealm.coax`) | 30,000 | 846 | 2,000 | 0 | 2,210,528 | 2,490,368 |
| Hecate's Cemetery (`villa.coax`) | 48,000 | 306 | 1,000 | 6 | 3,414,992 | 3,801,088 |
| The water lab (`waterlab.coax`) | 6 | 0 | 0 | 0 | 2,014,264 | 2,228,224 |

Notes on the measurements:

- The Grid's numbers come from its compile-time check, three frames a
  screen: its native build needs the page host's foreign functions.
- Belt 2200's peak varies by a few hundred vertices from run to run
  (44,088 to 44,424).
- Everglade submits its `DRAWS` records only once its asset pack has loaded,
  and no pack loads natively, so its measured draws are 0. Its declared draw
  limit is `DRAWS`, as are the Garden's, Fairy Forest's and Hecate's
  Cemetery's.
- Hecate's Cemetery and the water lab draw their scenes from draw records
  and uniforms. Their vertices are the HUD's.
- The water lab's frame packs the sea into its uniforms as float bits,
  which compile-time evaluation refuses (`E0412`), so its check takes only
  its arena.

## GPU memory has a budget too

The frame budget limits what a realm's writers produce. Its GPU objects have
a separate limit. The renderer's ledger,
[aethyr/render/registry.coax](../render/registry.coax)
([chapter 11](ch011-the-wgsl-renderer.md)), admits each buffer and texture
before it is created, within 512 slots and 1 GiB counted in four-byte words.
Past either limit it refuses, and nothing is created; nothing is clipped or
partly allocated. Destroying an object returns its words. Its checks and a
zero-survivor mutation run are in the file. A headless run on the served
Grid and fixture realm counts exactly the objects each renderer holds:
3 objects (6,175,024 words) and 24 objects (7,395,518 words).

## Checks

```shell
coax/bin/coax test aethyr/budget.coax --library
coax/bin/coax mutate aethyr/budget.coax --library
coax/bin/coax test apps/openagents-web/garden.coax --only "declared budget"
coax/bin/coax test apps/openagents-web/stair.coax --steps 3000000000 --only "declared budget"
```

`aethyr/budget.coax` has six checks. They cover:

- each limit by kind, a declaration refused for a limit below zero or a
  vertex limit at or above its buffer, and the kinds' names;
- the arena's bytes, worked by hand;
- takes that add up within a frame, a new frame that starts from nothing,
  and the peak and the frame that first reached it, with a tie that doesn't
  move it;
- the planted overflow below: one vertex too many is refused whole and
  reported with its kind and counts, and the check fails;
- a measured frame over budget in each kind, and the three screens;
- a count below zero, a kind that isn't one, and a sum that would pass the
  largest `int`.

<!-- source: aethyr/budget.coax planted -->
```jai
#test "one vertex too many is refused whole, reported with its kind and counts, and fails the check" {
  bm := [1] BudgetMeter.[budget_meter(budget_small())];
  budget_frame(bm[..]);
  #assert budget_take(bm[..], BUDGET_VERTICES, 94);
  #assert !budget_take(bm[..], BUDGET_VERTICES, 7);       // 101 > 100: none of the 7 is taken
  #assert bm[0].used[BUDGET_VERTICES] == 94 && bm[0].peak[BUDGET_VERTICES] == 94;
  #assert bm[0].over && bm[0].over_frames == 1 && bm[0].refused == 1 && !budget_ok(bm[0]);
  br := bm[0].first;
  #assert br.kind == BUDGET_VERTICES && br.frame == 1 && br.asked == 7 && br.used == 94 && br.limit == 100;
  #assert budget_take(bm[..], BUDGET_VERTICES, 6);        // what fits still fits
  #assert bm[0].used[BUDGET_VERTICES] == 100;
  // A second refusal in the same frame counts, but the frame is over once, and the first is kept.
  #assert !budget_take(bm[..], BUDGET_DRAWS, 5);
  #assert bm[0].refused == 2 && bm[0].over_frames == 1 && bm[0].first.kind == BUDGET_VERTICES;
  // The next frame starts within budget; the session stays failed.
  budget_frame(bm[..]);
  #assert !bm[0].over && bm[0].over_frames == 1 && !budget_ok(bm[0]);
  #assert !budget_take(bm[..], BUDGET_STEPS, 9);
  #assert bm[0].over_frames == 2 && bm[0].refused == 3 && bm[0].first.frame == 1;
}
```

Each realm's check is sharp. With Hold the Stair's buffer put back to 36,000
and its budget at 35,000, its check fails on the swarm. Its check also pins
the swarm's peak at 39,318.

`coax mutate` kills all 139 mutants of `aethyr/budget.coax`. The realm
files' changes are checks and declarations, and Hold the Stair's buffer,
which its check pins.

## Still to do

- **Asset packs through the registry.** An asset's identity is its content
  hash, so two realms that use one mesh share it and a changed texture is a
  new asset. [aethyr/assets.coax](../assets.coax) is the registry: it admits
  a pack whole or refuses it (an entry that doesn't hash to its table's id,
  new bytes past the limit, no free slot) and frees bytes with their last use
  (mutation 162 of 162). The realms' `VPK1` packs still load without it;
  their byte budgets are in `verse/packs/`, checked by
  `verse/tools/pack_realm.py` ([realm-host.md](../../docs/verse/realm-host.md)),
  and moving them onto it is [chapter 53](ch053-bring-in-assets.md)'s import
  path.
- **Refusal at the writer.** The realms' quad writers still stop silently
  at a full buffer. The check catches a clipped frame because the vertex
  limit is below the buffer. The renderer's writers
  ([chapter 11](ch011-the-wgsl-renderer.md)) take from the meter directly,
  and a development build shows the report on the HUD.
- **Frame time.** GPU time per pass is measured in the browser
  ([coax/web/frametime.ts](../../coax/web/frametime.ts)) and the 60 FPS
  walk in [coax/web/perf.ts](../../coax/web/perf.ts). A per-realm time
  budget per tier (desktop, phone) joins them.

## Dependencies

Chapters [1](ch001-a-frame-has-a-fixed-step.md) (the clock's step cap),
[3](ch003-state-starts-from-its-defaults.md) (declared defaults, where the
budget lives), [6](ch006-entities-are-born-and-die.md) (entities whose
spawns take from budgets) and 11–15 (the writers that take). From Coax: R22
compile and startup budgets for the build-side counterpart, R0 evidence for
the measured ones.
