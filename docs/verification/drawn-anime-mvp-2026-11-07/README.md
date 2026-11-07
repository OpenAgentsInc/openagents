# Drawn Anime MVP verification

Spec: [Drawn Anime](../../verse/2026-11-06-drawn-anime.md), section "MVP".
Scope: the CPU ink-and-paint, exposure-sheet, brush and photography
libraries and the shot "Wind on the hill", rendered headless. No GPU pass,
no page, no deployment.

## Compiler

The checks ran in a Linux cloud container on the stage0 compiler from this
tree's bootstrap (`coax/build.sh`, SHA-256
`a6c2134bed8b64e3c0c66f317fdda101a7fb5e58f1cac060f5a33e4e1ad5b18b`).
The full bootstrap did not finish within the container's 30-minute background
limit, so the checks are not on a qualified `coax/bin/coax`. Re-run them on one
before relying on them.

## Checks

`coax test FILE --library`:

| File | Own checks | Result with loaded files |
| --- | --- | --- |
| `aethyr/anim/ink.coax` | 5 | 5 passed |
| `aethyr/anim/xsheet.coax` | 3 | 8 passed |
| `aethyr/anim/brush.coax` | 4 | 9 passed |
| `aethyr/anim/photo.coax` | 3 | 12 passed |
| `verse/animlab/drawn/wind-hill.coax` | 3 | 18 passed |
| `verse/animlab/drawn/run-everglade.coax` | 4 | 22 passed |

None of these files decides money, admission or policy, so none is
`#authority` and no mutation run was required. No `coax mutate` was run.

`coax/web/tool.sh tools-check` could not run in this clone (its base commit
is not fetched). Every new file is Coax, so none needs a firm-reason header.

## Render

`verse/tools/drawn-anime.coax` (release C, checked arithmetic off), 1280 × 720.

"Wind on the hill" (`--shot wind`):

| Measure | Result |
| --- | --- |
| Frames | 96 (4 s at 24 fps), 48.8 s wall for all of them in the container |
| Determinism | Two complete renders: 96 of 96 frame digests equal |
| Distinct frames | 96 digests, all different (the camera trucks every frame; grain is seeded by frame) |
| Palette | Mean saturation 0.373 (max over frames 0.374); 0.09 % of pixels above 0.75 saturation |
| Timing | Wren's hair, scarf and skirt change drawing every second frame (on twos), checked in `wind-hill.coax` |

"Running to Everglade" (`--shot run`):

| Measure | Result |
| --- | --- |
| Frames | 96 (4 s at 24 fps), 41.0 s wall |
| Determinism | Two complete renders: 96 of 96 frame digests equal |
| Palette | Mean saturation 0.313 (max over frames 0.314); 0.02 % of pixels above 0.75 saturation |
| Timing | Eight run drawings, each held two frames (`rn_drawing`, checked); the meadow, grass and smoke on the same sheet |

The frames and the encoded video stay out of git. Re-create them with the
two commands in the spec.

## Owner review

The look has not had owner review against a reference board yet (spec, D0).
