# Effuse Storybook (Web)

This repo has a React-free “storybook” for **Effuse templates** and a matching **visual regression** test suite.

There are 2 ways to run it:

- **UI-only (default):** Vite dev server (fast iteration, no Worker bindings, no Convex/Auth side effects).
- **Worker-hosted (optional):** Wrangler dev serving the real `apps/web` Worker (prod-parity for SSR/headers/assets).

## What You Get

- A component explorer at `GET /__storybook` with:
  - left sidebar (Atoms / Molecules / Organisms)
  - iframe canvas rendering the selected story
- Direct “canvas” URLs per story: `GET /__storybook/canvas/:storyId`
- A story metadata API for test automation: `GET /__storybook/api/stories`
- A visual regression suite (`effuse-test`) that:
  - enumerates stories from `/__storybook/api/stories`
  - screenshots each canvas in headless Chromium
  - compares to a committed baseline PNG

Autopilot-specific posture:

- Add “wire transcript” stories that render chat from a deterministic `messageParts` transcript fixture and show the raw JSON alongside the rendered UI.
- See `docs/autopilot/STREAM_TESTING.md` for the fixture format and goals.

## Where It Lives

- Story registry + definitions:
  - `apps/web/src/storybook/types.ts`
  - `apps/web/src/storybook/index.ts`
  - `apps/web/src/storybook/stories/*`
- Storybook pages (Effuse templates):
  - `apps/web/src/effuse-pages/storybook.ts`
- Worker endpoint for story list:
  - `apps/web/src/effuse-host/storybook.ts`
- Routes:
  - `apps/web/src/effuse-app/routes.ts` (`/__storybook`, `/__storybook/canvas/:id`)

Visual testing harness:
- Runner: `packages/effuse-test/src/cli.ts`
- Visual snapshot code: `packages/effuse-test/src/runner/visualSnapshot.ts`
- Visual suite: `packages/effuse-test/src/suites/apps-web.ts` (`apps-web.visual.storybook`)

## Run Locally

### 1) Prereqs

- Node/npm for `apps/web`
- Bun for `packages/effuse-test` (only needed for the test runner)
- A local Chrome/Chromium executable (for visual tests)

### 2) Env Vars

UI-only storybook does **not** require any env vars.

The **Worker-hosted** storybook (and the visual suite) expects `VITE_CONVEX_URL` to be available at runtime. In practice, you should have `apps/web/.env.local` containing at least:

```bash
VITE_CONVEX_URL="https://<your-dev-deployment>.convex.cloud"
```

### 3) Start Storybook

```bash
cd apps/web
npm run storybook
```

Open:

- `http://localhost:6006/` (also supports `http://localhost:6006/__storybook`)

Notes:
- This is **UI-only** and does not start `wrangler dev`.

### 4) (Optional) Run Storybook In The Worker Host

If you need prod-parity (SSR, Worker asset binding, response headers), run:

```bash
cd apps/web
npm run storybook:worker
```

Open:

- `http://localhost:6006/__storybook`

## Using Storybook

### Manager UI

- Visit `GET /__storybook`
- Click a story in the sidebar to load it into the canvas iframe

### Canvas URLs

Each story is directly addressable:

- `GET /__storybook/canvas/:storyId`

This is what visual tests screenshot.

## Adding Stories

### Story Type

Stories are **pure render functions** that return an Effuse `TemplateResult`:

- no React
- no JSX
- no TanStack

See `apps/web/src/storybook/types.ts`.

### Add a New Story

1. Add it to a file under `apps/web/src/storybook/stories/` (or create a new file).
2. Export it from `apps/web/src/storybook/index.ts` via `allStories`.

Guidelines:
- **IDs must be stable**: `id` becomes the URL segment and snapshot filename.
- Keep the rendered output **deterministic**:
  - avoid `Date.now()`, random ids, “time ago” strings, etc.
  - prefer fixed example strings and fixed layout sizes
- Use `kind: "atom" | "molecule" | "organism"` to categorize.

## Visual Regression Tests

### Baseline Location

Committed baseline PNGs live in:

- `apps/web/tests/visual/storybook/*.png`

The filename is derived from `storyId` (sanitized).

### Run The Visual Suite

```bash
cd apps/web
npm run test:visual
```

What it does:
- Starts `wrangler dev` on a dedicated port (via `packages/effuse-test`)
- Fetches story ids from `GET /__storybook/api/stories`
- For each story:
  - loads `GET /__storybook/canvas/:storyId`
  - waits for `[data-story-ready="1"]`
  - waits for `document.fonts.ready` when available
  - screenshots at a fixed viewport
  - compares PNG to the baseline with `pixelmatch`

Artifacts on failures:
- `output/effuse-test/<runId>/<testId>/failure.png`
- `output/effuse-test/<runId>/<testId>/failure.html`
- plus per-story `.diff.png` in that test’s artifact directory when mismatches occur

### Update / Create Baselines

```bash
cd apps/web
npm run test:visual:update
```

This sets:

- `EFFUSE_TEST_UPDATE_SNAPSHOTS=1`

Behavior:
- Missing baselines are created
- Existing baselines are overwritten

### Running Against A Deployed URL (Optional)

If you want to test a deployed Worker (manual / operator workflow), you can point the runner at an existing base URL:

```bash
cd packages/effuse-test
bun run src/cli.ts run --project ../../apps/web --tag visual --base-url https://autopilot-web.openagents.workers.dev
```

Notes:
- By default, `effuse-test` does **not** run `prod`-tagged tests unless you pass `--tag prod`.
- Visual tests are opt-in via `--tag visual`.

## Troubleshooting

### Storybook loads but canvas is blank

- Open the canvas URL directly: `/__storybook/canvas/<id>`
- Check the browser console for runtime errors
- If running **Worker-hosted** (`npm run storybook:worker`), verify Effuse client assets are served:
  - `GET /effuse-client.css`
  - `GET /effuse-client.js`

### CSS looks missing

UI-only storybook (`npm run storybook`) serves CSS via Vite and should not depend on `dist/effuse-client`.

Worker-hosted storybook (`npm run storybook:worker`) depends on the Effuse client build being present in `dist/effuse-client/`.
If you started `wrangler dev` without `dev:client`, assets may be stale or missing.

Use:

```bash
cd apps/web
npm run storybook:worker
```

### Visual tests cannot find Chrome

Set an explicit path:

```bash
export EFFUSE_TEST_CHROME_PATH="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
```

### Flaky diffs due to fonts/animations

The visual suite already:
- waits for `document.fonts.ready` when available
- injects a “reduce motion” style to disable animations/transitions

If a story is still flaky, it probably contains time-based content or nondeterministic layout.
Fix the story to be deterministic rather than increasing thresholds.
