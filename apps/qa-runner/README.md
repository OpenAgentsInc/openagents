# @openagentsinc/qa-runner

Khala autonomous-QA runner — the substrate + headline demo for the **Khala
autonomous-QA example flow** (epic #6174).

It executes a **computer-use session** (via the Probe computer-use tools, #6175)
against a **Target** (dev or prod), inside an **isolation backend**, and emits a
**dereferenceable, public-safe receipt**: a playable video + Playwright trace +
per-step screenshots + `result.json`. A reviewer confirms a run by reading the
result and watching the video — *no local run*.

> Origin: modeled as prior art on `projects/repos/executor/e2e` (Target / VM /
> artifact substrate) and on `apps/acceptance-runner` (the Bun+Effect+Playwright
> fakes-in-CI / real-for-proof discipline).

## What's real now vs owner-gated

| Piece | Status |
|---|---|
| `scriptedBrain` (deterministic decision-maker) | **real now** — used by the demo + tests |
| `khalaBrain` (drives via `openagents/khala` inference) | **seam, owner/flag-gated, inert** — throws "not armed" without an injected driver; no live inference in CI |
| `localBackend` (real chromium on this host) | **real now** — the default |
| `cloudVmBackend` (per-run OpenAgents Cloud firecracker / sek8s microVM) | **interface-only, owner-gated** — throws "not armed" unless an injected provisioner is supplied; `cloud` (`oa-node`/`oa-workroomd`) wires the real provisioner later |
| video (mp4 via ffmpeg, webm fallback) + trace + screenshots + `result.json` | **real now** |
| session → committed e2e test distiller | **follow-up** (not in this app yet) |
| run = verified receipt / settlement wrapper | **follow-up** |

There is **no fake green**: an un-armed cloud backend or khala brain throws; a
missing-chromium real run fails honestly; a failed assertion produces a `fail`
result with the failure visible in the video.

## One model

`openagents/khala` is the single model. There are no `khala-code`/`-mini`/`-pro`
variants. (`openagents/khala-oss-20b` is a separate served alias.)

## Commands

```sh
# Unit tests — fakes-in-CI, NO chromium, NO network (the default gate)
bun run --cwd apps/qa-runner test

# Install chromium for the real-browser paths below
bun run --cwd apps/qa-runner playwright:install   # or: bunx playwright install chromium

# Headline demo (#6177): Khala /login regression on https://openagents.com
bun run --cwd apps/qa-runner demo:login
bun run --cwd apps/qa-runner demo:login -- --out ./runs/login --headed
# Prove honest failure: point the same scenario at a deliberately-wrong assertion
bun run --cwd apps/qa-runner demo:login -- --wrong

# One-shot run against any target (real chromium)
bun run --cwd apps/qa-runner run-once -- --url https://openagents.com --out ./runs/manual

# Long-running daemon scaffold (inert without QA_JOB_LEASE_URL)
bun run --cwd apps/qa-runner serve
```

## Artifacts (`result.json` + receipt)

A run writes, into the artifact dir:

- `session.mp4` (or `session.webm` if ffmpeg is unavailable — reported in
  `result.json.artifacts.videoFormat`)
- `trace.zip` (Playwright trace — open with `npx playwright show-trace`)
- `NN-step.png` per-step screenshots
- `result.json` — `{ status, target, brain, backend, steps, artifacts, failure? }`

`result.json` and all artifact metadata are **public-safe**: a tripwire
(`assertPublicSafeResult`) rejects any forbidden field
(`token`/`secret`/`password`/`cookie`/`authorization`/`bearer`/`api_key`/
`prompt`/`credential`) before the result is written, and the surfaces withhold
typed text, command output, and file contents from the action timeline at the
source.

## Targets

A `Target` is a deployment seen from outside (`name`, `baseUrl`, `capabilities`).
Swap `baseUrl` to point the same scenario at dev or prod:

```sh
QA_TARGET_URL=https://staging.openagents.com bun run --cwd apps/qa-runner run-once
```

Default target is `https://openagents.com`.

## Cost / time envelope

The demo is one headless chromium session of a handful of steps: a few seconds
of compute plus the page-load time of the target, and a sub-second ffmpeg
transcode. It is cheap enough to run on demand / per PR. The hosted tier is
"more/faster VMs" via `cloudVmBackend`, never a lock-in for this OSS core.

## Docker

```sh
docker build -f apps/qa-runner/Dockerfile -t oa-qa-runner .
docker run --rm oa-qa-runner   # runs demo:login against openagents.com
```
