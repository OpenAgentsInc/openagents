# Desktop visual baselines — automated pixel proof (QA-3, #8908)

OpenAgents Desktop has an automated visual-baseline harness: a deterministic
headless capture of the shell's key states, committed baseline PNGs, and a
pixel diff gate that fails on drift. It is the Desktop lane's pixel evidence
for the no-ship-without-pixel-proof rule (epic #8904).

## Run recipe

From `apps/openagents-desktop/`:

```sh
# The gate: build, capture the current states in a second OS process,
# diff against the committed baselines, exit nonzero on drift.
pnpm run qa:visual

# Baseline refresh — an explicit, reviewed action, never automatic.
pnpm run qa:visual -- --update-baselines
```

On drift the gate prints a `gate FAILED — pixel drift` line naming the scratch
directory holding side-by-side review artifacts
(`<state>.side-by-side.png` = baseline | current | drift mask, plus
`<state>.current.png`). Review the artifacts; if the drift is intended, land
the change together with a `--update-baselines` refresh so the review diff
shows the new pixels.

## What is captured

Five frozen fixture shell states (window 1280x800, device scale 1, TZ=UTC,
animations and caret disabled, clock frozen to the fixture instant):

| State | Surface |
| --- | --- |
| `composer-idle` | Empty transcript, composer idle, both lanes available |
| `thread-plan-card` | Thread with a typed runtime plan card (completed / in-progress / pending steps) |
| `approval-card` | Pending `tool_approval` question card (Approve / Deny) |
| `reasoning-disclosure` | Reasoning system row in the timeline |
| `full-auto-running` | Full Auto enabled with the background `turn_running` badge |

Baselines live in `apps/openagents-desktop/visual-baselines/` (PNG per state +
`manifest.json` with sha256, dimensions, capture geometry, and thresholds).
Baselines are platform-pinned (`darwin-arm64`); the gate refuses to compare
across platforms.

## How it works

- **Probe** (`OPENAGENTS_DESKTOP_VISUAL_BASELINE_PROBE=1` in `src/main.ts`):
  windowless per the existing probe pattern — an offscreen `BrowserWindow`
  loads the renderer with `?visualBaseline=<state>`, waits for the
  `data-visual-baseline-ready` flag, and writes `capturePage` PNGs into
  `OPENAGENTS_DESKTOP_VISUAL_BASELINE_SHOTS`, then `app.exit`s with a
  public-safe receipt line.
- **Renderer fixture mode** (`src/renderer/visual-baseline.ts` +
  `visual-baseline-fixtures.ts`): mounts the REAL React workbench over one
  frozen `DesktopShellState` built with the production state constructors —
  no preload bridge, no providers, no clocks (Date is frozen to
  `2026-07-15T09:45:00.000Z`).
- **Gate** (`scripts/visual-baseline-smoke.ts`): builds the current Desktop,
  then spawns that just-built app through Electron as a second OS process
  (never a possibly stale packaged artifact under `out/`), decodes PNGs in pure
  TS (`src/visual-baseline-diff.ts`, node:zlib only — zero new dependencies),
  and compares pixel-wise with the bounded thresholds recorded in the
  manifest (per-channel tolerance 3, max differing-pixel ratio 0.1%).
- **Tests**: `src/visual-baseline-diff.test.ts` (codec roundtrip, identical /
  threshold-pass / drift-fail, synthetic alteration, manifest handling),
  `src/renderer/visual-baseline-fixtures.test.ts` (fixture determinism and
  content), and `src/visual-baseline-baselines.test.ts` (committed manifest /
  PNG agreement plus a deliberate drift introduced into a real committed
  baseline proving the gate catches it).
