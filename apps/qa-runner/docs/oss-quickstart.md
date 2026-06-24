# OSS quick-start — autonomous QA against YOUR site, with YOUR model

> Issue #6191 / Rhys req #5: *"Must be OSS and runnable locally."*

This is the genuinely-OSS, local-first, runtime-agnostic path. You run an
autonomous end-to-end check **on your own machine, against your own dev server,
driven by any OpenAI-compatible model you bring**. It records a playable video +
a Playwright trace + per-step screenshots, and **distills the session into a
committed e2e test file** you keep in your repo.

**No OpenAgents account, login, or key is required.** Khala is just one optional
model endpoint. The OpenAgents-specific add-ons (Cloud VMs, `/pro`,
receipts-to-marketplace, settlement) are **not used by this path and are not
dependencies of it**.

It is MIT-licensed (`apps/qa-runner/LICENSE`).

---

## 0. The 10-second proof (no model key, no network, no login)

```sh
bun run --cwd apps/qa-runner demo:byo
# == qa run (BYO-model, OSS, local-first) ==
# MODE: --fake-model — deterministic, no network, NO OpenAgents login, NO model key.
# ...
# result: pass (./runs/byo-fake/result.json)
# emitted committed test: .../generated/<slug>.e2e.test.ts
```

`--fake-model` drives a canned `/login` scenario against a fake page with a
deterministic decision-maker. It still produces a real `result.json`, a video
artifact, and a committed e2e test — proving the whole pipeline (drive → record →
distill) works with **zero credentials and zero hosted dependency**. This is the
exact code path a real run takes; only the model/browser are swapped for fakes.

---

## 1. Install

The shipped `qa` CLI is a **single self-contained bundle** (`dist/qa.js`) — the
workspace deps (`@openagentsinc/probe-runtime` + `effect`) are inlined at build
time, so a standalone install needs **no monorepo, no workspace, and no
OpenAgents login**. The only runtime dependency that stays external is
`playwright` (it downloads its own browser).

### Standalone (no monorepo) — the real OSS path

```sh
# from the published registry once this is on npm (see §5):
npx  @openagentsinc/qa-runner run --fake-model --url https://example.test --out ./runs/qa
bunx @openagentsinc/qa-runner run --fake-model --url https://example.test --out ./runs/qa

# one-time, for the real-browser path (the fake path above needs none of this):
npx playwright install chromium
```

It runs on plain Node (`node dist/qa.js …`) or Bun — the bundle targets Node and
keeps a `#!/usr/bin/env node` shebang.

### From a local tarball (works today, before npm publish)

```sh
git clone https://github.com/OpenAgentsInc/openagents && cd openagents && bun install
bun run --cwd apps/qa-runner build          # produces apps/qa-runner/dist/qa.js
cd apps/qa-runner && bun pm pack            # -> openagentsinc-qa-runner-0.1.0.tgz (runs prepack -> build)

# now, in ANY clean dir OUTSIDE the monorepo, with no workspace:
mkdir /tmp/my-ci && cd /tmp/my-ci && npm init -y
npm install /path/to/openagentsinc-qa-runner-0.1.0.tgz
./node_modules/.bin/qa run --fake-model --url https://example.test --out ./runs/qa
npx playwright install chromium             # one-time, only for the real-browser path
```

This is genuinely standalone: the install pulls only `effect` + `playwright`
(NOT `@openagentsinc/probe-runtime`, which is inlined), and the run needs no
OpenAgents account.

### Dev (inside the monorepo, against source)

```sh
git clone https://github.com/OpenAgentsInc/openagents && cd openagents && bun install
bun run --cwd apps/qa-runner playwright:install     # one-time, real-browser path
bun run --cwd apps/qa-runner qa run --url http://localhost:3000 ...
```

---

## 2. Run a scenario against YOUR dev server with YOUR model

Bring any OpenAI-compatible endpoint — OpenAI, OpenRouter, a local `llama.cpp` /
`vLLM` / Ollama OpenAI shim, or `openagents/khala` if you happen to want it.

```sh
bun run --cwd apps/qa-runner qa run \
  --url   http://localhost:3000 \
  --goal  "open /login, confirm the sign-in form renders, and confirm the URL stays at /login" \
  --model gpt-4o-mini \
  --base-url https://api.openai.com/v1 \
  --api-key  "$OPENAI_API_KEY" \
  --out ./runs/my-app
```

Env equivalents (the de-facto OpenAI standard, so existing CI works unchanged):

```sh
export OPENAI_BASE_URL=https://api.openai.com/v1
export OPENAI_API_KEY=sk-...
export OPENAI_MODEL=gpt-4o-mini
bun run --cwd apps/qa-runner qa run --url http://localhost:3000
```

Neutral `QA_*` names (`QA_MODEL`, `QA_BASE_URL`, `QA_API_KEY`) are also accepted
and take precedence over the `OPENAI_*` names. Flags win over env. The credential
**value is never printed** — only its source label.

### Point it at a local, keyless model server

```sh
bun run --cwd apps/qa-runner qa run \
  --url http://localhost:3000 \
  --model my-local-model \
  --base-url http://localhost:8080/v1 \
  --allow-keyless
```

---

## 3. What you get

Into `--out`:

- `session.mp4` (or `session.webm` if `ffmpeg` is unavailable) — the playable run
- `trace.zip` — open with `npx playwright show-trace ./runs/my-app/trace.zip`
- `NN-step.png` — per-step screenshots
- `result.json` — `{ status, target, steps, artifacts, failure? }` (public-safe)
- `session-trace.json` — the deterministic trace the distiller consumes

And a **committed e2e test** at `generated/<slug>.e2e.test.ts` (override with
`--emit <path>`). It is a real, runnable check you can drop into your repo and
run in CI:

```sh
TARGET_URL=http://localhost:3000 bun test ./generated/<slug>.e2e.test.ts
```

---

## 4. Drop it into executor.sh's CI

```yaml
# .github/workflows/qa.yml (sketch)
- run: bun install
- run: bun run --cwd apps/qa-runner playwright:install
- run: |
    bun run --cwd apps/qa-runner qa run \
      --url "$DEV_URL" \
      --goal "verify the sign-in flow renders and stays on /login" \
      --out ./runs/ci
  env:
    OPENAI_BASE_URL: ${{ secrets.OPENAI_BASE_URL }}
    OPENAI_API_KEY:  ${{ secrets.OPENAI_API_KEY }}
    OPENAI_MODEL:    gpt-4o-mini
# upload ./runs/ci/* as build artifacts; commit ./generated/*.e2e.test.ts
```

The `qa` exit code is honest: `0` only on a clean pass **and** an admissible
distilled test; a failed assertion / unreachable verdict / config error is a
non-zero exit (never a fake green). So the CI check goes red on a real failure.

---

## 5. How standalone works: the bundled CLI (and publishing)

The QA core uses the computer-use surface (Playwright `BrowserSurface`,
`withBrowserSurface`, `acquirePlaywrightBrowser`) from
`@openagentsinc/probe-runtime`. That package is `private: true` / `version 0.0.0`
and pulls in further workspace packages, so it is **not** independently published
— a naive `npm i @openagentsinc/qa-runner` could never resolve it from source.

We fix that by **bundling**, not by waiting on an extraction:

- `bun run build` (`scripts/build.ts`) runs `Bun.build` on the BYO CLI
  (`src/byo.ts`). It aliases `@openagentsinc/probe-runtime` to that package's
  `computer-use` entry (the only surface the BYO path reaches) and **inlines**
  it together with `effect` into a single self-contained `dist/qa.js`. Only
  `playwright` is kept external (it is a real, published runtime dep that
  downloads its own browser).
- Because the heavy, unrelated probe-runtime modules (terminal `@opentui/core`
  native binaries, model backends, benchmark harnesses) are never reached by the
  BYO path, the bundle stays small and **needs no workspace packages at run
  time**.
- `package.json` points `bin.qa` and the `./qa` export at `dist/qa.js`,
  `@openagentsinc/probe-runtime` is a **devDependency** (inlined, not required by
  consumers), and a `prepack` hook rebuilds `dist/qa.js` fresh at pack time so
  the npm tarball always ships the current bundle. `dist/` is git-ignored.

Bundling happens at the JS/module-graph level, so the pre-existing
`packages/probe` **typecheck** errors do not block it — `bun build` does not
typecheck.

### Proof (standalone, no workspace, no login)

```sh
bun run --cwd apps/qa-runner build
cd apps/qa-runner && bun pm pack
cd /tmp && mkdir clean && cd clean && npm init -y
npm install --ignore-scripts /path/to/openagentsinc-qa-runner-0.1.0.tgz
# scrubbed env, no OPENAGENTS_*/OPENAI_*/QA_*, no workspace:
env -i PATH="$PATH" HOME="$HOME" ./node_modules/.bin/qa \
  run --fake-model --url https://example.test --out ./runs/proof
# -> verdict: pass, emits a video + a committed e2e test, exit 0.
# node_modules/@openagentsinc/ contains qa-runner only — NO probe-runtime.
```

### Publishing (do not run unless releasing)

The package is publish-ready and intentionally NOT published here. Per
`apps/pylon/docs/npm-publishing-runbook.md`, `bun publish` is broken against
npmjs — pack first, then publish the tarball with npm (scope `@openagentsinc/`,
token in workspace `.secrets/npm-publish.env`):

```sh
bun run --cwd apps/qa-runner build
cd apps/qa-runner && bun pm pack
npm publish ./openagentsinc-qa-runner-0.1.0.tgz --access public
```

CDN propagation can make a fresh publish look 404 to bun for minutes; the runbook
covers that. Until then the local-tarball path in §1 is the genuine, runnable
standalone OSS install.
