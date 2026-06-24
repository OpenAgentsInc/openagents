# OpenAgents Autonomous QA — standalone quickstart

Run an autonomous end-to-end check on **your own machine**, against **your own
server**, driven by **any OpenAI-compatible model you bring**. It records a
playable video + a Playwright trace + per-step screenshots, and **distills the
session into a committed e2e test file** you keep in your repo.

**No OpenAgents account, login, or key is required.** The runner is the
MIT-licensed package `@openagentsinc/qa-runner`. Khala / OpenAgents Cloud /
`/pro` / receipts are **optional add-ons, not dependencies of this path**.

- Docs page: <https://openagents.com/docs/autonomous-qa>
- Source: <https://github.com/OpenAgentsInc/openagents/tree/main/apps/qa-runner>
- Full OSS quick-start (in-repo): `apps/qa-runner/docs/oss-quickstart.md`

> **Publish status (be honest about install):** the package is **publish-ready
> but is NOT yet on the public npm registry** — `npm view @openagentsinc/qa-runner`
> currently 404s. Until it is published, use the **local-tarball** or
> **clone-and-build** paths below, which work today. The `bunx` / `npx` lines are
> shown for once it is published.

---

## 0. The 10-second proof — no model key, no network, no login

This proves the whole pipeline (drive → record → distill) with **zero
credentials and zero hosted dependency**. It drives a canned `/login` scenario
against a fake page with a deterministic decision-maker, and still produces a
real `result.json`, a video artifact, and a committed e2e test.

```sh
# Inside a clone of the monorepo:
git clone https://github.com/OpenAgentsInc/openagents && cd openagents
bun install
bun run --cwd apps/qa-runner demo:byo
# == qa run (BYO-model, OSS, local-first) ==
# MODE: --fake-model — deterministic, no network, NO OpenAgents login, NO model key.
# ...
# verdict: pass
# emitted committed test: .../generated/<slug>.e2e.test.ts
```

This is the exact code path a real run takes; only the model and browser are
swapped for fakes.

---

## 1. Install

The shipped `qa` CLI is a **single self-contained bundle** (`dist/qa.js`) — the
workspace deps are inlined at build time, so a standalone install needs **no
monorepo, no workspace, and no OpenAgents login**. The only runtime dependency
that stays external is `playwright`, which downloads its own browser.

### From a local tarball (works today, before npm publish)

```sh
git clone https://github.com/OpenAgentsInc/openagents && cd openagents
bun install
bun run --cwd apps/qa-runner build          # produces apps/qa-runner/dist/qa.js
cd apps/qa-runner && bun pm pack            # -> openagentsinc-qa-runner-0.1.0.tgz

# now, in ANY clean dir OUTSIDE the monorepo, with no workspace:
mkdir /tmp/my-ci && cd /tmp/my-ci && npm init -y
npm install /path/to/openagentsinc-qa-runner-0.1.0.tgz
./node_modules/.bin/qa run --fake-model --url https://example.test --out ./runs/qa

# one-time, only for the real-browser path:
npx playwright install chromium
```

The install pulls only `effect` + `playwright` (the heavy workspace packages are
inlined, not required at run time). The run needs no OpenAgents account.

### From the published registry (once it is on npm)

```sh
bunx @openagentsinc/qa-runner run --fake-model --url https://example.test --out ./runs/qa
npx  @openagentsinc/qa-runner run --fake-model --url https://example.test --out ./runs/qa
npx playwright install chromium             # one-time, real-browser path
```

It runs on plain Node (`node dist/qa.js …`) or Bun.

### Dev (inside the monorepo, against source)

```sh
git clone https://github.com/OpenAgentsInc/openagents && cd openagents && bun install
bun run --cwd apps/qa-runner playwright:install     # one-time, real-browser path
bun run --cwd apps/qa-runner qa run --url http://localhost:3000 ...
```

---

## 2. Run a scenario against YOUR dev server with YOUR model

Bring any OpenAI-compatible endpoint — OpenAI, OpenRouter, a local `llama.cpp` /
`vLLM` / Ollama OpenAI shim, or `openagents/khala` if you want it.

```sh
qa run \
  --url   http://localhost:3000 \
  --goal  "open /login, confirm the sign-in form renders, and confirm the URL stays at /login" \
  --model gpt-4o-mini \
  --base-url https://api.openai.com/v1 \
  --api-key  "$OPENAI_API_KEY" \
  --out ./runs/my-app
```

(Inside the monorepo, prefix the command with `bun run --cwd apps/qa-runner`.)

Env equivalents — the de-facto OpenAI standard, so existing CI works unchanged:

```sh
export OPENAI_BASE_URL=https://api.openai.com/v1
export OPENAI_API_KEY=sk-...
export OPENAI_MODEL=gpt-4o-mini
qa run --url http://localhost:3000
```

Neutral `QA_*` names (`QA_MODEL`, `QA_BASE_URL`, `QA_API_KEY`) are also accepted
and take precedence over the `OPENAI_*` names. Flags win over env. The
credential **value is never printed** — only its source label.

### Point it at a local, keyless model server

```sh
qa run \
  --url http://localhost:3000 \
  --model my-local-model \
  --base-url http://localhost:8080/v1 \
  --allow-keyless
```

### All flags

```
--url <url>          Target dev/prod server to drive (required for a real run).
--goal "<text>"      What the agent should verify (defaults to a /login check).
--model <id>         BYO model id (or env QA_MODEL / OPENAI_MODEL).
--base-url <url>     OpenAI-compatible base URL (or env QA_BASE_URL / OPENAI_BASE_URL).
--api-key <key>      Bearer key (or env QA_API_KEY / OPENAI_API_KEY). Never printed.
--allow-keyless      Permit a keyless local server (llama.cpp / vLLM / Ollama shim).
--out <dir>          Artifact dir for video/trace/screenshots/result (default ./runs/qa).
--emit <path>        Where to write the distilled e2e test (default generated/<slug>.e2e.test.ts).
--max-turns <n>      Hard cap on model turns (default 16).
--headed             Run a visible browser (default headless).
--fake-model         Deterministic, no-network, no-key, no-OpenAgents proof of the loop.
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
run against any target:

```sh
TARGET_URL=http://localhost:3000 bun test ./generated/<slug>.e2e.test.ts
```

---

## 4. Multiple targets — dev, staging, prod

A target is a deployment seen from outside (a name + a base URL). Swap the base
URL to point the same scenario at a different environment without rewriting it:

```sh
qa run --url https://staging.example.com --goal "verify the sign-in flow" --out ./runs/staging
qa run --url https://example.com         --goal "verify the sign-in flow" --out ./runs/prod
```

---

## 5. Drop it into CI

```yaml
# .github/workflows/qa.yml (sketch)
- run: bun install
- run: bun run --cwd apps/qa-runner playwright:install
- run: |
    qa run \
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
distilled test; a failed assertion, an unreachable verdict, or a config error is
a non-zero exit (never a fake green). So the CI check goes red on a real failure
and only on a real failure.

### Post the video to a PR

GitHub's REST API cannot natively attach media to a comment.
[`ain3sh/gh-attach`](https://github.com/ain3sh/gh-attach) drives the web upload
path that can, and prints embeddable markdown — so a CI job can post the run
video + screenshots into the PR comment alongside the distilled-test ref. When
the upload is unavailable or unauthenticated, fall back to a relative video ref;
never a broken or fake embed.

---

## 6. Optional hosted path (OpenAgents)

The standalone runner above is complete on its own. OpenAgents also offers an
**optional managed path**, clearly separate from the free local path and never a
requirement for it:

- Runs driven by `openagents/khala` on OpenAgents infrastructure.
- A QA control API to drive the full submit → run → fetch flow over HTTP.
- A `/pro` dashboard to watch runs and review distilled tests.

The hosted tier is "more and faster runners," not a lock-in for the open-source
core.

---

MIT-licensed. Source and the in-repo OSS quick-start:
<https://github.com/OpenAgentsInc/openagents/tree/main/apps/qa-runner>.
