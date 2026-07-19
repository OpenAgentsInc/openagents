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

The QA-1 swarm Desktop lane runs the same gate verbatim from the repository
root:

```sh
pnpm run qa:swarm:desktop
```

Its machine-readable invocation and receipt contract is checked in at
`docs/qa/swarm/desktop-visual-lane.json`. Success is exit code 0 plus the
`[openagents-desktop visual-baseline] gate OK ` line whose JSON payload has
schema `openagents.qa.desktop-visual-lane.v1` and `lane: "desktop"`. QA-1 may
embed that payload unchanged in its dated findings report. It must not infer
success from screenshots or build output alone.

On drift the gate prints a `gate FAILED — pixel drift` line naming the scratch
directory holding side-by-side review artifacts
(`<state>.side-by-side.png` = baseline | current | drift mask, plus
`<state>.current.png`). Review the artifacts. If the drift is intended, land
the change together with a `--update-baselines` refresh so the review diff
shows the new pixels.

## What is captured

Sixteen frozen fixture states (window 1280x800, device scale 1, TZ=UTC,
animations and caret disabled, clock frozen to the fixture instant):

| State                          | Surface                                                                                           |
| ------------------------------ | ------------------------------------------------------------------------------------------------- |
| `composer-idle`                | Empty transcript, composer idle, both lanes available                                             |
| `thread-plan-card`             | Thread with a typed runtime plan card (completed / in-progress / pending steps)                   |
| `approval-card`                | Pending `tool_approval` question card (Approve / Deny)                                            |
| `reasoning-disclosure`         | Reasoning system row in the timeline                                                              |
| `full-auto-running`            | Full Auto enabled with the background `turn_running` badge                                        |
| `workbench-messages-reasoning` | Every shared message and reasoning fixture, including honest redacted absence                     |
| `workbench-commands`           | Running, completed, failed, and bounded-output command cards                                      |
| `workbench-files`              | Running, applied, failed, and bounded file-change/diff cards                                      |
| `workbench-tools-mcp-dynamic`  | All shared MCP and dynamic tool-call fixtures                                                     |
| `workbench-tools-web-image`    | All shared web and image tool-call fixtures                                                       |
| `workbench-plans-approvals`    | Every shared plan and approval fixture                                                            |
| `workbench-agents`             | Every shared delegation/agent-group fixture                                                       |
| `workbench-context`            | Every exact-value context/rate-limit meter fixture                                                |
| `workbench-notices-long-tail`  | Notice severities plus compaction, sleep, review, hook, and declined-command rows                 |
| `workbench-shell`              | Work-group fold, composer, queued follow-up, populated rail, header, timeline, and shell controls |
| `workbench-frame`              | The complete shared app frame: rail, header, timeline, and composer together                      |

Baselines live in `apps/openagents-desktop/visual-baselines/` (PNG per state +
`manifest.json` with sha256, dimensions, capture geometry, and thresholds).
Baselines are platform-pinned (`darwin-arm64`). The gate refuses to compare
across platforms.

## How it works

- **Probe** (`OPENAGENTS_DESKTOP_VISUAL_BASELINE_PROBE=1` in `src/main.ts`):
  windowless per the existing probe pattern — an offscreen `BrowserWindow`
  loads the renderer with `?visualBaseline=<state>`, waits for the
  `data-visual-baseline-ready` flag, and writes `capturePage` PNGs into
  `OPENAGENTS_DESKTOP_VISUAL_BASELINE_SHOTS`, then `app.exit`s with a
  public-safe receipt line.
- **Renderer fixture mode** (`src/renderer/visual-baseline.ts`,
  `visual-baseline-fixtures.ts`, and `visual-baseline-workbench.tsx`): mounts
  the REAL React shell for five frozen `DesktopShellState` projections and
  mounts the shared `@openagentsinc/ui/desktop-workbench` components for eleven
  catalog pages consuming every #8870 fixture. There is no preload bridge,
  provider, network, or live clock (Date is frozen to
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
