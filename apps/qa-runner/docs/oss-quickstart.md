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

This package currently lives inside the OpenAgents monorepo. It runs on
[Bun](https://bun.sh).

```sh
git clone https://github.com/OpenAgentsInc/openagents
cd openagents
bun install
# one-time, for the real-browser path (the fake path above needs none of this):
bun run --cwd apps/qa-runner playwright:install   # installs chromium
```

> When this package is published to npm (see "Publishing & the probe-runtime
> caveat" below) the install will become `bunx @openagentsinc/qa-runner` /
> `npx @openagentsinc/qa-runner`. Today, run it from the monorepo via the `qa`
> script.

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

## 5. Honesty: the `@openagentsinc/probe-runtime` dependency

The QA core uses the computer-use surface (Playwright `BrowserSurface`,
`withBrowserSurface`, `acquirePlaywrightBrowser`) from
`@openagentsinc/probe-runtime`. **Right now that package is not independently
published**: in this monorepo it is `private: true`, `version 0.0.0`, and pulls
in further workspace packages (`@openagentsinc/blueprint-contracts`,
`@openagentsinc/provider-account-schema`). So today the genuinely-runnable OSS
path is **"clone the monorepo and run `qa`"** — which fully satisfies "OSS +
runnable locally with no login" — but a bare `npm i @openagentsinc/qa-runner`
will not yet resolve the runtime from the public registry.

Path to a fully-standalone npm package (tracked as a follow-up to #6191):

1. The QA core only needs a thin **computer-use port** — `BrowserSurface`,
   `WaitForCondition`, `TerminalCondition`, `withBrowserSurface`,
   `acquirePlaywrightBrowser`, and the Playwright artifact types. Extract just
   those into a small, dependency-light published package (e.g.
   `@openagentsinc/computer-use`) that depends only on `playwright` + `effect`,
   **without** the Blueprint/provider-account workspace coupling.
2. Repoint qa-runner's `import "@openagentsinc/probe-runtime"` sites
   (`brain.ts`, `backend.ts`, `runner.ts`, `khala-session.ts`,
   `terminal-backend.ts`, `terminal-scenario.ts`) at that port.
3. Publish qa-runner with that port as its only `@openagentsinc/*` dependency,
   then `bunx @openagentsinc/qa-runner` works against the public registry.

Until then, the OSS guarantee is honest and concrete: MIT-licensed source, a
local-first run with no OpenAgents login, and a published `package.json` that is
publishable in shape — see `bun pm pack --dry-run` in the README.
