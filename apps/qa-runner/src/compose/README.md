# compose — polished, shareable video layer (EPIC #6187)

A **data-driven** compose/polish layer for QA-runner demos. It turns a completed
run directory (raw `session.mp4` + `00-*.png` screenshots + `result.json` +
`session-trace.json`) into a polished mp4 with a title card (scenario + verdict),
step/keystroke labels, brand framing, and an optional **before/after
side-by-side** of two runs.

The raw `recordVideo` clip stays the _evidence_ artifact. This is the
_shareable_ layer (Rhys, PRs, social).

## Design

The interesting logic is a **pure, deterministic** function:

```
buildComposePlan(input: ComposeInput) -> ComposePlan
```

It reads run metadata (projected from `result.json` + `session-trace.json`) and
emits a fully-resolved `ComposePlan` — a typed description of title-card and clip
segments with overlays (titles, labels, keystroke pills, verdict badge, layout).
It does **no rendering, no I/O, and reads no clock**: the same input always
yields a byte-identical plan (unit-tested).

A thin **ffmpeg executor** (`ffmpeg.ts`) consumes the plan and renders via
ffmpeg primitives only (`color` sources for title cards, `scale`+`pad`+`hstack`
for side-by-side, `drawtext`/`drawbox` for overlays, `concat` to stitch). The
arg builder (`buildFfmpegArgs`) is exported and unit-tested without spawning.

There is **no hand-written per-video code** — every overlay is a prop on the
plan, derived from run metadata.

## License note — why ffmpeg, not Remotion

The original reference (droid-control) uses Remotion. **Remotion requires a paid
company license for organizations with more than 3 employees**, which conflicts
with keeping this pipeline fully OSS. We therefore use an **ffmpeg-only**
compositor (`drawtext`/`overlay`/`concat`), which is BSD/LGPL/GPL-licensed and
free of per-seat company terms. The plan model is render-engine-agnostic, so a
Remotion executor could be added later behind the same `ComposePlan` if its
licensing is ever acceptable — but the default and only shipped renderer is
ffmpeg.

## CLI

```sh
# single run
pnpm --dir apps/qa-runner run compose --run runs/khala-zeratul-demo --out /tmp/demo.mp4

# before/after side-by-side
pnpm --dir apps/qa-runner run compose \
  --before runs/before-dir --after runs/after-dir --out /tmp/ba.mp4

# inspect the resolved plan without rendering
pnpm --dir apps/qa-runner run compose --run runs/khala-zeratul-demo --plan-only
```

Flags: `--run`, `--before`/`--after`, `--out`, `--brand`, `--plan-only`.

If `ffmpeg` is not on `PATH`, the executor will fail. Use `--plan-only` to verify
the plan offline.

### ffmpeg build requirements / graceful degradation

Full text overlays need an ffmpeg built with **libfreetype** (the `drawtext`
filter). The executor probes for `drawtext` at render time:

- **drawtext present** → real title cards, step labels, and verdict text.
- **drawtext absent** (a stripped ffmpeg build, e.g. some Homebrew bottles) →
  the executor **degrades gracefully**: it renders the title card, brand
  framing, and verdict/pill overlays as colored `drawbox` blocks (so the verdict
  color and layout still read), drops plain text, and prints a NOTE. The render
  still succeeds and produces a valid mp4. Install a libfreetype-enabled ffmpeg
  (`brew install ffmpeg` usually includes it, verify with
  `ffmpeg -filters | grep drawtext`) for full text.

## Timed text on any video

The QA compose command above is for QA run directories.
Use `overlay-text` to add timed text to an arbitrary video:

```sh
pnpm --dir apps/qa-runner run overlay-text \
  --input /path/to/source.mp4 \
  --cues src/compose/timed-text.example.json \
  --out /path/to/output.mp4
```

The cue sheet uses `openagents.media.timed_text.v1`.
Each cue has a start time, an end time, text, and one of these styles:
`title`, `center`, `lower-third`, or `state-label`.
Newline characters make multiline text.

The command gets the source size and duration with `ffprobe`.
It renders each text frame as a transparent PNG with Playwright.
It then uses the FFmpeg `overlay` filter for the specified time range.
This method does not require the FFmpeg `drawtext`, `subtitles`, or `ass`
filters.

The command maps the optional source-audio stream into the output.
The default `--audio copy` keeps the source audio without a re-encode.
Use `--audio aac` when the MP4 container does not support the source codec.

The command refuses to replace an output file.
Use `--force` only when you intend to replace that exact file.

## Files

- `plan.ts` — typed `ComposeInput` / `ComposePlan` Effect Schema model.
- `build-plan.ts` — the pure `buildComposePlan` planner (+ helpers).
- `load.ts` — reads a run dir into public-safe `ComposeRunMeta` (the only input I/O).
- `ffmpeg.ts` — `buildFfmpegArgs` (pure) + `renderComposePlan` (spawns ffmpeg).
- `cli.ts` — the `compose` CLI entrypoint.
- `timed-text.ts` — the cue schema, text-frame renderer, and FFmpeg plan.
- `timed-text-cli.ts` — the `overlay-text` CLI entrypoint.
- `timed-text.example.json` — a bounded example cue sheet.
- `*.test.ts` — unit tests for the planner and arg builder.

## Deferred follow-up

Wiring `compose` into `demo-khala.ts` / `runner.ts` (auto-render a shareable mp4
at the end of a run, and attach it to the gh-attach PR post) is intentionally
**deferred** to avoid colliding with in-flight runner lanes. This module is
standalone and only adds the `compose` script entry. See issue #6187 for the
follow-up.
