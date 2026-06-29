# @openagentsinc/qa-runner

OSS, local-first, runtime-agnostic autonomous-QA runner — the substrate + the
headline demo for the **Khala autonomous-QA example flow** (epic #6174).

> **OSS + BYO-model + no login (issue #6191 / Rhys req #5).** The core path runs
> on YOUR machine, against YOUR target, driven by ANY OpenAI-compatible model you
> bring (model + base URL + key via flags/env) — **no OpenAgents account or login
> required**. The fake-model proof needs no key; Khala uses a free `oa_agent_…`
> key from `POST /api/keys/free`. It records a video + trace + screenshots and distills the
> session into a **committed e2e test**. Khala / OpenAgents Cloud / `/pro` /
> receipts / settlement are **optional add-ons, not dependencies of the core
> run**. MIT-licensed (`LICENSE`). Full walkthrough:
> [`docs/oss-quickstart.md`](docs/oss-quickstart.md).
>
> ```sh
> # 10-second proof: no key, no network, no login. Emits a video + a committed test.
> bun run --cwd apps/qa-runner demo:byo
>
> # Real Khala run against your dev server. Mint a free key first:
> export QA_API_KEY="$(curl -fsS -X POST https://openagents.com/api/keys/free | jq -r '.credential.token')"
> bun run --cwd apps/qa-runner qa run \
>   --url http://localhost:3000 --out ./runs/my-app
>
> # BYO override still works:
> bun run --cwd apps/qa-runner qa run \
>   --url http://localhost:3000 --model gpt-4o-mini \
>   --base-url https://api.openai.com/v1 --api-key "$OPENAI_API_KEY" --out ./runs/my-app-openai
> ```
>
> **Genuinely standalone.** The shipped `qa` CLI is a single self-contained
> bundle (`dist/qa.js`) built by `bun run build` — `@openagentsinc/probe-runtime`
> and `effect` are inlined, so a standalone install needs **no workspace and no
> login** (only `playwright` stays external). Install the packed tarball in any
> clean dir (`npm install ./openagentsinc-qa-runner-0.1.0.tgz` →
> `qa run --fake-model ...`), or `bunx @openagentsinc/qa-runner` once published.
> Mechanics + proof in the quick-start (§1, §5).

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
| `scriptedBrain` (deterministic decision-maker) | **real now** — deterministic CI + `demo:login` |
| Khala driver (`runKhalaSession`): Khala **autonomously** drives via `openagents/khala` | **real now** — `demo:khala`; a prompt-based ReAct/JSON-action loop over `/chat/completions` (plain `fetch`, no native function-calling) |
| `khalaBrain` (BrainStep seam for `runQaSession`) | **inert seam** — throws "not armed" without an injected driver; the live loop is `runKhalaSession`, not this seam |
| `KhalaSessionTrace` capture + `assertSessionTracePublicSafe` tripwire | **real now** — deterministic, replayable, public-safe (`session-trace.json`) |
| session → committed executor-style e2e scenario **distiller** (spec §E.2) | **real now** — `distill(trace)` emits `generated/<slug>.e2e.test.ts` |
| skill emitter (NIP-SKL marketplace candidate, spec §E.1) | **typed seam + TODO, FUTURE/owner-gated** |
| `localBackend` (real chromium on this host) | **real now** — the default |
| `cloudVmBackend` (per-run OpenAgents Cloud firecracker / sek8s microVM) | **interface-only, owner-gated** — throws "not armed" unless an injected provisioner is supplied |
| video (mp4 via ffmpeg, webm fallback) + trace + screenshots + `result.json` | **real now** |
| run = verified receipt / settlement wrapper | **follow-up, owner-gated (settlement INERT)** |

There is **no fake green**: an un-armed cloud backend or khala brain throws; a
missing-chromium real run fails honestly; a failed assertion produces a `fail`
result with the failure visible in the video; an unparseable model action is a
real failure (a bounded corrective re-prompt, never a fabricated action); a
session that never reaches a verdict is reported `incomplete`/`fail`.

## Credentials (no hardcoded secrets)

The `qa` CLI defaults to the Khala dogfood endpoint:

- `QA_MODEL` default: `openagents/khala`
- `QA_BASE_URL` default: `https://openagents.com/api/v1`
- `QA_API_KEY`: a free `oa_agent_…` key from
  `curl -X POST https://openagents.com/api/keys/free`
- `QA_DEMAND_KIND` default: `internal`, sent only to the OpenAgents endpoint so
  served-token analytics can distinguish first-party QA dogfood from external
  demand. Override with `external` or `unlabeled` for non-dogfood runs.

`demo:khala` resolves the model endpoint from env + `~/work/.secrets/`, in order:

1. `OPENAGENTS_API_KEY` env -> the real `openagents/khala` endpoint (preferred).
2. a discovered `OPENAGENTS_AGENT_TOKEN=` in any `~/work/.secrets/*.env` -> real
   `openagents/khala`.
3. `PROBE_OPENAI_API_KEY` (env or `~/work/.secrets/probe-openai.env`) -> an
   OpenAI-compatible **fallback** model, used ONLY to prove the loop runs for
   real and clearly labeled; pass `--no-fallback` to forbid it.

The credential VALUE is never printed; only its source LABEL is logged.

## One model

`openagents/khala` is the single model. There are no `khala-code`/`-mini`/`-pro`
variants. (`openagents/khala-oss-20b` is a separate served alias.)

## Commands

```sh
# Unit tests — fakes-in-CI, NO chromium, NO network (the default gate)
bun run --cwd apps/qa-runner test

# Build the self-contained standalone CLI bundle (dist/qa.js; inlines workspace deps)
bun run --cwd apps/qa-runner build
# Verify the publish-ready tarball contents (includes a freshly-built dist/qa.js via prepack)
cd apps/qa-runner && bun pm pack --dry-run

# Install chromium for the real-browser paths below
bun run --cwd apps/qa-runner playwright:install   # or: bunx playwright install chromium

# HEADLINE demo (epic #6174): Khala AUTONOMOUSLY drives the session, records it,
# and distills it into a COMMITTED executor-style e2e scenario.
bun run --cwd apps/qa-runner demo:khala
bun run --cwd apps/qa-runner demo:khala -- --goal "..." --url https://openagents.com --out ./runs/khala
#   -> writes session.mp4 + trace.zip + screenshots + result.json + session-trace.json,
#      and emits generated/<slug>.e2e.test.ts (the review artifact alongside the video).
# Run the generated scenario (a real, runnable test) against any target:
TARGET_URL=https://openagents.com bun test apps/qa-runner/generated/login-verify.e2e.test.ts

# Deterministic /login regression demo (scriptedBrain; no model, for CI):
bun run --cwd apps/qa-runner demo:login
bun run --cwd apps/qa-runner demo:login -- --out ./runs/login --headed
# Prove honest failure: point the same scenario at a deliberately-wrong assertion
bun run --cwd apps/qa-runner demo:login -- --wrong

# One-shot run against any target (real chromium)
bun run --cwd apps/qa-runner run-once -- --url https://openagents.com --out ./runs/manual

# Long-running daemon scaffold (inert without QA_JOB_LEASE_URL)
bun run --cwd apps/qa-runner serve

# QA CONTROL API (#6196): drive the full submit->run->fetch flow over HTTP, not
# just the CLI. Auth'd by a Khala agent bearer token; deterministic mock path
# (no Chrome/network/spend) by default; real runs gated by QA_CONTROL_ARM_REAL=1.
QA_CONTROL_TOKENS="raynor:tok_demo_secret" bun run --cwd apps/qa-runner api
#   curl quick-start a third party can follow: docs/control-api-quickstart.md
```

## QA Control API (#6196): everything over HTTP

The runner is also drivable **programmatically over HTTP** so the whole flow —
submit → run → fetch artifacts + verdict + `/pro` link — is API-first (for
`executor.sh`'s CI and "do everything via API"). It is a **qa-runner HTTP
daemon** (the runner drives a real Chrome, which can't run in a Cloudflare
Worker), running the existing engine **in-process** with an in-memory job store.
Auth is a **Khala agent bearer token**; a deterministic **mock path** runs with
no Chrome/network/spend; real runs are owner-gated. Full curl walkthrough:
[`docs/control-api-quickstart.md`](docs/control-api-quickstart.md).

## Artifacts (`result.json` + receipt)

A run writes, into the artifact dir:

- `session.mp4` (or `session.webm` if ffmpeg is unavailable — reported in
  `result.json.artifacts.videoFormat`)
- `trace.zip` (Playwright trace — open with `npx playwright show-trace`)
- `NN-step.png` per-step screenshots
- `result.json` — `{ status, target, brain, backend, steps, artifacts, failure?, verify? }`
- `session-trace.json` (Khala runs) — the deterministic, public-safe
  `KhalaSessionTrace` (`openagents.khala.session_trace.v1`): ordered beats
  (chat_turn/tool_call/browser/terminal/verdict) with raw text/secrets withheld
  (refs/hashes only), inferred typed inputs/outputs, receipts, and a content
  digest. This is the distiller's input.

### Verify verdict (commitments → CONFIRMED / REFUTED / INCONCLUSIVE, #6192)

A run may declare **commitments** (`RunInput.commitments`) — what it must PROVE,
and the evidence type. After the run, the **verify stage** (`src/verify.ts`)
checks the produced steps/status against them and writes an **additive**
`result.json.verify` field:

```jsonc
"verify": {
  "verdict": "REFUTED",            // CONFIRMED | REFUTED | INCONCLUSIVE
  "observed": true,
  "findings": [
    { "id": "claims-redirect", "claim": "...", "verdict": "REFUTED",
      "evidenceSummary": "observed step \"...\" = failed (contradicting evidence)" }
  ]
}
```

It is an **investigator** verdict with strict **anti-fabrication**: a false
claim is a valid **REFUTED** finding (never a fake pass); an
unobserved/expected-but-unran outcome is **INCONCLUSIVE**, never CONFIRMED; only
OBSERVED ok evidence yields CONFIRMED. The verdict is surfaced on the `/pro`
run + eval pages (a CONFIRMED-green / REFUTED-red / INCONCLUSIVE-amber pill +
the evidence) and led at the top of the PR-evidence comment (`pr-comment.ts`).
`verify` is **additive** and namespaced — it does not rename/remove existing
fields and is independent of the separate additive `receipt` field.

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

## Chill-evals: compare agents across MCP/config changes (#6183)

A **chill-eval** holds a scenario fixed and runs it across **N variants** (a
variant = a brain / model / config / MCP-set / tool-policy, or a before/after of
a change), then produces a **comparison**: per-variant pass-rate, latency
p50/p90, and behavior deltas vs the baseline, each with its run artifact
(video/result). It reuses the inference benchmark's percentile/mean math and
persists a public-safe `openagents.qa_runner.eval.v1` result (tripwire-checked).

```sh
# Deterministic, no-network/no-spend comparison (fixtures), prints the PR markdown:
bun run --cwd apps/qa-runner evals -- --fixtures --id login-mcp-compare --out ./runs/eval --md

# Real chromium against a target:
bun run --cwd apps/qa-runner evals -- --url https://openagents.com --out ./runs/eval
```

A variant supplies its own `brain` / `backend` factory, so "model A vs B" or
"MCP-on vs MCP-off" is two variant entries — it does **not** edit
`khala-config`/`openrouter` (a separate lane owns those). The default
fixture/CI path is `decisionGrade: false` (illustrative — proves the harness,
not the lanes); only an owner-armed real seam yields decision-grade numbers.
`not_measured` is honest (never a fabricated `0`).

The eval renders in `/pro` at a stable, shareable **`/pro/evals/<id>`** URL
(variant comparison + per-variant video + deltas) — the link the PR loop posts.

## PR-evidence + CI loop (gh-attach) (#6185)

On a PR, `.github/workflows/chill-eval.yml` diff-scopes the changed paths to
affected scenarios, runs the eval (**fixtures by default — no spend**), and posts
a PR comment with the comparison table, the per-variant video, the distilled-test
ref, and the **`/pro/evals/<id>` link**.

```sh
# Compose the PR comment locally (dry run; writes ./pr-comment.md):
bun run --cwd apps/qa-runner pr-comment -- \
  --changed "apps/openagents.com/...,apps/qa-runner/..." \
  --out ./runs/pr-eval --comment-out ./pr-comment.md
```

Videos are uploaded with [`ain3sh/gh-attach`](https://github.com/ain3sh/gh-attach)
— a Go binary that uses GitHub's **web** upload path the REST API lacks, and
prints embeddable markdown. **Install in CI** (armed runs only; it builds from
source so Go must be present):

```sh
curl -fsSL https://github.com/ain3sh/gh-attach/releases/latest/download/install.sh \
  | sh -s -- --bin-dir "$HOME/.local/bin"
```

gh-attach authenticates from local browser cookies, which CI does not have; when
the upload is unavailable/unauthenticated the comment falls back to the in-eval
**relative video ref** — honest, never a broken or fake embed. The composer
exits non-zero on a real regression, so the CI check is **RED on a failing
variant** (no fake green). The real-model path is gated behind
`vars.CHILL_EVAL_ARMED == 'true'` + a token cap and is OFF by default.
